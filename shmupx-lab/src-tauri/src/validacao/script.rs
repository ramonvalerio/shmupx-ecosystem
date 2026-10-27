//! Execução do validador Python de cada jogo (`shmupx_offline`, ADR 004), com o mesmo contrato
//! `client-v1` que o Client usa. Diferente do Client, o Lab não recusa resultados duvidosos:
//! guarda o relatório inteiro e calcula se o Client o aceitaria sem revisão.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

use super::dados::{agora, Resultado};

const TIMEOUT_ANALISE: Duration = Duration::from_secs(900);
const TIMEOUT_CONSULTA: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptInfo {
    pub id: String,
    pub name: String,
    pub version: String,
}

/// Pasta do `shmupx_offline`: `SHMUPX_OFFLINE_ROOT`, ou a do repositório em desenvolvimento.
pub fn raiz_offline() -> Result<PathBuf, String> {
    let raiz = match std::env::var_os("SHMUPX_OFFLINE_ROOT") {
        Some(raiz) => PathBuf::from(raiz),
        None if cfg!(debug_assertions) => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shmupx_offline"),
        None => return Err("defina SHMUPX_OFFLINE_ROOT com a pasta do shmupx_offline".into()),
    };
    let raiz = raiz.canonicalize().map_err(|e| format!("pasta do shmupx_offline indisponível: {e}"))?;
    if !raiz.join("analyze.py").is_file() {
        return Err(format!("analyze.py não encontrado em {}", raiz.display()));
    }
    Ok(raiz)
}

/// Python do ambiente do script: `SHMUPX_PYTHON`, senão o `.venv` do `shmupx_offline`, senão o do PATH.
pub fn python(raiz: &Path) -> OsString {
    if let Some(python) = std::env::var_os("SHMUPX_PYTHON") {
        return python;
    }
    let local = raiz.join(if cfg!(windows) { ".venv/Scripts/python.exe" } else { ".venv/bin/python" });
    if local.is_file() { local.into_os_string() } else { "python".into() }
}

fn comando(raiz: &Path) -> Command {
    let mut comando = Command::new(python(raiz));
    comando
        .current_dir(raiz)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    comando.creation_flags(0x08000000); // CREATE_NO_WINDOW
    comando
}

/// Lê um pipe inteiro sem deixar o processo travar, guardando no máximo `limite` bytes.
async fn ler_limitado(mut leitor: impl AsyncRead + Unpin, limite: usize) -> Vec<u8> {
    let mut saida = Vec::new();
    let mut buffer = [0u8; 8192];
    while let Ok(lidos) = leitor.read(&mut buffer).await {
        if lidos == 0 {
            break;
        }
        let cabe = lidos.min(limite.saturating_sub(saida.len()));
        saida.extend_from_slice(&buffer[..cabe]);
    }
    saida
}

/// Roda o Python com os argumentos e devolve (sucesso, stdout, stderr).
async fn executar(raiz: &Path, argumentos: &[OsString], timeout: Duration) -> Result<(bool, Vec<u8>, Vec<u8>), String> {
    let mut filho = comando(raiz)
        .args(argumentos)
        .spawn()
        .map_err(|e| format!("não foi possível iniciar o Python: {e}"))?;
    let stdout = filho.stdout.take().ok_or("stdout indisponível")?;
    let stderr = filho.stderr.take().ok_or("stderr indisponível")?;
    let processo = async {
        tokio::join!(filho.wait(), ler_limitado(stdout, 1_048_576), ler_limitado(stderr, 65_536))
    };
    match tokio::time::timeout(timeout, processo).await {
        Ok((status, saida, erro)) => {
            let status = status.map_err(|e| format!("falha ao aguardar o Python: {e}"))?;
            Ok((status.success(), saida, erro))
        }
        Err(_) => {
            let _ = filho.kill().await;
            Err(format!("o script passou de {} minutos e foi encerrado", timeout.as_secs() / 60))
        }
    }
}

fn ultima_linha(bytes: &[u8]) -> String {
    let texto = String::from_utf8_lossy(bytes);
    texto
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().chars().take(500).collect())
        .unwrap_or_else(|| "sem detalhe".to_string())
}

/// Validadores registrados (`analyze.py --list-games`; não exige OpenCV).
pub async fn listar_scripts(raiz: &Path) -> Result<Vec<ScriptInfo>, String> {
    let argumentos = [OsString::from("analyze.py"), OsString::from("--list-games")];
    let (ok, saida, erro) = executar(raiz, &argumentos, TIMEOUT_CONSULTA).await?;
    if !ok {
        return Err(format!("o Python falhou ao listar os scripts: {}", ultima_linha(&erro)));
    }
    serde_json::from_slice(&saida).map_err(|e| format!("lista de scripts em formato inesperado: {e}"))
}

/// As bibliotecas que a análise usa estão instaladas no Python escolhido.
pub async fn dependencias_ok(raiz: &Path) -> Result<(), String> {
    let argumentos = [OsString::from("-c"), OsString::from("import cv2, numpy")];
    let (ok, _, erro) = executar(raiz, &argumentos, TIMEOUT_CONSULTA).await?;
    if ok { Ok(()) } else { Err(ultima_linha(&erro)) }
}

/// Analisa o vídeo com o script do jogo. Falhas do script viram um `Resultado` com `erro`
/// (contam no percentual); só falhas para sequer iniciar o processo são `Err`.
pub async fn analisar(raiz: &Path, jogo: &str, video: &Path) -> Resultado {
    let inicio = Instant::now();
    let argumentos: Vec<OsString> = vec![
        "analyze.py".into(),
        video.as_os_str().to_owned(),
        "--game".into(),
        jogo.into(),
        "--player".into(),
        "lab".into(),
        "--format".into(),
        "client-v1".into(),
        "--quiet".into(),
    ];
    let execucao = executar(raiz, &argumentos, TIMEOUT_ANALISE).await;
    let duracao = inicio.elapsed().as_secs();
    match execucao {
        Ok((true, saida, _)) => interpretar(&saida, jogo, duracao),
        Ok((false, _, erro)) => Resultado::falha(format!("o script falhou: {}", ultima_linha(&erro)), duracao),
        Err(erro) => Resultado::falha(erro, duracao),
    }
}

#[derive(Deserialize)]
struct Relatorio {
    schema_version: u32,
    game_id: String,
    validator_version: String,
    confidence: String,
    needs_review: bool,
    gameplay: Gameplay,
}

#[derive(Deserialize)]
struct Gameplay {
    #[serde(rename = "hi-score")]
    score: Option<i64>,
    #[serde(rename = "lives-lost")]
    lives: i32,
    continues: i32,
    #[serde(rename = "game-over")]
    game_over: bool,
    observation: Option<String>,
}

/// Lê o relatório `client-v1`. `aceito_automatico` segue a regra do Client
/// (`shmupx-client/src-tauri/src/analysis/offline.rs`): confiança alta, sem revisão, sem game
/// over, sem continue e com placar.
fn interpretar(bytes: &[u8], jogo: &str, duracao_segundos: u64) -> Resultado {
    let bruto: Value = match serde_json::from_slice(bytes) {
        Ok(valor) => valor,
        Err(e) => return Resultado::falha(format!("relatório inválido do script: {e}"), duracao_segundos),
    };
    let relatorio: Relatorio = match serde_json::from_value(bruto.clone()) {
        Ok(relatorio) => relatorio,
        Err(e) => return Resultado::falha(format!("relatório fora do formato client-v1: {e}"), duracao_segundos),
    };
    if relatorio.schema_version != 1 || relatorio.game_id != jogo {
        return Resultado::falha("relatório de outro formato ou de outro jogo".into(), duracao_segundos);
    }
    let g = &relatorio.gameplay;
    let contadores_validos = g.score.is_none_or(|s| s >= 0) && g.lives >= 0 && g.continues >= 0;
    let aceito_automatico = contadores_validos
        && relatorio.confidence == "high"
        && !relatorio.needs_review
        && !g.game_over
        && g.continues == 0
        && g.score.is_some();
    Resultado {
        placar: g.score,
        vidas: Some(g.lives),
        continues: Some(g.continues),
        game_over: g.game_over,
        confianca: relatorio.confidence.clone(),
        revisao: relatorio.needs_review,
        observacao: g.observation.clone(),
        versao_validador: Some(relatorio.validator_version.clone()),
        aceito_automatico,
        erro: None,
        relatorio: Some(bruto),
        em: agora(),
        duracao_segundos,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn relatorio(confidence: &str, review: bool, score: Option<i64>, continues: i32, game_over: bool) -> Vec<u8> {
        json!({
            "schema_version": 1, "game_id": "earthion", "validator_version": "1.0.0",
            "confidence": confidence, "needs_review": review,
            "gameplay": { "hi-score": score, "lives-lost": 2, "continues": continues, "game-over": game_over, "observation": "ok" }
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn relatorio_limpo_e_aceito_automaticamente() {
        let r = interpretar(&relatorio("high", false, Some(123_400), 0, false), "earthion", 10);
        assert!(r.aceito_automatico && r.erro.is_none());
        assert_eq!((r.placar, r.vidas, r.continues), (Some(123_400), Some(2), Some(0)));
        assert_eq!(r.versao_validador.as_deref(), Some("1.0.0"));
        assert!(r.relatorio.is_some());
    }

    #[test]
    fn mesma_regra_do_client_para_ir_a_revisao() {
        for (confianca, revisao, placar, continues, game_over) in [
            ("medium", false, Some(1), 0, false),
            ("high", true, Some(1), 0, false),
            ("high", false, None, 0, false),
            ("high", false, Some(1), 1, false),
            ("high", false, Some(1), 0, true),
        ] {
            let r = interpretar(&relatorio(confianca, revisao, placar, continues, game_over), "earthion", 1);
            assert!(!r.aceito_automatico, "{confianca} {revisao} {placar:?} {continues} {game_over}");
            assert!(r.erro.is_none(), "leitura duvidosa não é erro do script");
        }
    }

    #[test]
    fn saida_invalida_ou_de_outro_jogo_vira_falha() {
        assert!(interpretar(b"nao e json", "earthion", 1).erro.is_some());
        assert!(interpretar(b"{\"schema_version\":1}", "earthion", 1).erro.is_some());
        assert!(interpretar(&relatorio("high", false, Some(1), 0, false), "outro", 1).erro.is_some());
    }

    /// Contra o shmupx_offline real do repositório (só precisa do Python, não do OpenCV).
    /// `cargo test -- --ignored scripts_reais`
    #[tokio::test]
    #[ignore = "exige Python instalado"]
    async fn scripts_reais_incluem_o_earthion() {
        let raiz = raiz_offline().unwrap();
        let scripts = listar_scripts(&raiz).await.unwrap();
        let earthion = scripts.iter().find(|s| s.id == "earthion").expect("earthion registrado");
        assert_eq!(earthion.version, "1.0.0");
    }

    #[test]
    fn ultima_linha_ignora_vazias_e_limita_o_tamanho() {
        assert_eq!(ultima_linha(b"Traceback\n  x\nValueError: ruim\n\n"), "ValueError: ruim");
        assert_eq!(ultima_linha(b""), "sem detalhe");
        assert_eq!(ultima_linha("x".repeat(900).as_bytes()).len(), 500);
    }
}
