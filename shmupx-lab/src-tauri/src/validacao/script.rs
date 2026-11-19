//! Execução dos scripts de validação do Lab (`shmupx-lab/validators`, ADR 004): o motor Python
//! genérico roda o script `games/<jogo>/<versão>/` com o mesmo contrato `client-v1` que o Client usa. Diferente do Client, o Lab não recusa resultados duvidosos:
//! guarda o relatório inteiro e calcula se o Client o aceitaria sem revisão.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};
use tokio::sync::watch;
use tokio::process::Command;

use super::dados::{agora, Resultado};

const TIMEOUT_ANALISE: Duration = Duration::from_secs(900);
const TIMEOUT_CONSULTA: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    /// `draft` (rascunho) ou `approved`.
    pub status: String,
    /// Imagens e arquivos que o script pede e ainda não existem; vazio = pronto para rodar.
    #[serde(default)]
    pub missing: Vec<String>,
}

/// Pasta `validators` do Lab: `SHMUPX_VALIDATORS_ROOT`, ou a do repositório em desenvolvimento.
pub fn raiz_validadores() -> Result<PathBuf, String> {
    let raiz = match std::env::var_os("SHMUPX_VALIDATORS_ROOT") {
        Some(raiz) => PathBuf::from(raiz),
        None if cfg!(debug_assertions) => Path::new(env!("CARGO_MANIFEST_DIR")).join("../validators"),
        None => return Err("defina SHMUPX_VALIDATORS_ROOT com a pasta validators do shmupx-lab".into()),
    };
    let raiz = raiz.canonicalize().map_err(|e| format!("pasta validators indisponível: {e}"))?;
    if !raiz.join("analyze.py").is_file() {
        return Err(format!("analyze.py não encontrado em {}", raiz.display()));
    }
    Ok(raiz)
}

/// Python do ambiente dos scripts: `SHMUPX_PYTHON`, senão o `.venv` da pasta `validators`, senão o do PATH.
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

/// Scripts de todos os jogos, um por versão (`analyze.py --list-games`; não exige OpenCV).
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

/// Duração do vídeo em segundos, lida pelo OpenCV do ambiente do script. `None` se não der.
pub async fn duracao_video(raiz: &Path, video: &Path) -> Option<u64> {
    let codigo = "import cv2,sys\nc=cv2.VideoCapture(sys.argv[1])\nf=c.get(cv2.CAP_PROP_FPS)\nn=c.get(cv2.CAP_PROP_FRAME_COUNT)\nprint(n/f if f and n else 0)";
    let argumentos = [OsString::from("-c"), OsString::from(codigo), video.as_os_str().to_owned()];
    let (ok, saida, _) = executar(raiz, &argumentos, TIMEOUT_CONSULTA).await.ok()?;
    if !ok {
        return None;
    }
    String::from_utf8_lossy(&saida).trim().parse::<f64>().ok().filter(|d| d.is_finite() && *d > 0.0).map(|d| d.round() as u64)
}

/// `SHMUPX_PROGRESS 45` -> 45.0 (linha que o script publica no stderr com `--progress`).
fn progresso_da_linha(linha: &str) -> Option<f32> {
    linha
        .trim()
        .strip_prefix("SHMUPX_PROGRESS")?
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|p| (0.0..=100.0).contains(p))
}

/// Lê o stderr do script: as linhas de progresso vão para `progresso`; das demais, guarda as
/// últimas (para explicar uma falha).
async fn ler_stderr(leitor: impl AsyncRead + Unpin, progresso: &watch::Sender<f32>) -> Vec<u8> {
    let mut linhas = BufReader::new(leitor).lines();
    let mut ultimas: VecDeque<String> = VecDeque::new();
    while let Ok(Some(linha)) = linhas.next_line().await {
        if let Some(p) = progresso_da_linha(&linha) {
            let _ = progresso.send(p);
        } else {
            if ultimas.len() == 50 {
                ultimas.pop_front();
            }
            ultimas.push_back(linha.chars().take(500).collect());
        }
    }
    ultimas.into_iter().collect::<Vec<_>>().join("\n").into_bytes()
}

/// Analisa o vídeo com a versão pedida do script do jogo, publicando o progresso (0 a 100). Falhas do script
/// viram um `Resultado` com `erro` (contam no percentual). `None` = cancelado pelo admin.
pub async fn analisar(
    raiz: &Path,
    jogo: &str,
    versao: &str,
    video: &Path,
    progresso: watch::Sender<f32>,
    cancelar: watch::Receiver<bool>,
) -> Option<Resultado> {
    let inicio = Instant::now();
    let argumentos: Vec<OsString> = vec![
        "analyze.py".into(),
        video.as_os_str().to_owned(),
        "--game".into(),
        jogo.into(),
        "--version".into(),
        versao.into(),
        "--player".into(),
        "lab".into(),
        "--format".into(),
        "client-v1".into(),
        "--quiet".into(),
        "--progress".into(),
    ];
    let mut filho = match comando(raiz).args(&argumentos).spawn() {
        Ok(filho) => filho,
        Err(e) => return Some(Resultado::falha(format!("não foi possível iniciar o Python: {e}"), 0)),
    };
    let (Some(stdout), Some(stderr)) = (filho.stdout.take(), filho.stderr.take()) else {
        return Some(Resultado::falha("saídas do script indisponíveis".into(), 0));
    };
    let processo = async {
        tokio::join!(filho.wait(), ler_limitado(stdout, 1_048_576), ler_stderr(stderr, &progresso))
    };
    // Cancelar descarta o futuro; o processo do Python morre junto (kill_on_drop).
    let execucao = tokio::select! {
        espera = tokio::time::timeout(TIMEOUT_ANALISE, processo) => espera,
        _ = super::esperar_cancelamento(cancelar) => return None,
    };
    let duracao = inicio.elapsed().as_secs();
    Some(match execucao {
        Ok((Ok(status), saida, _)) if status.success() => interpretar(&saida, jogo, versao, duracao),
        Ok((Ok(_), _, erro)) => Resultado::falha(format!("o script falhou: {}", ultima_linha(&erro)), duracao),
        Ok((Err(e), _, _)) => Resultado::falha(format!("falha ao aguardar o Python: {e}"), duracao),
        Err(_) => Resultado::falha(format!("o script passou de {} minutos e foi encerrado", TIMEOUT_ANALISE.as_secs() / 60), duracao),
    })
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
    #[serde(default)]
    players: Option<u8>,
    #[serde(default)]
    cleared: Option<bool>,
    #[serde(default, rename = "bombs-used")]
    bombs_used: Option<i32>,
}

/// Lê o relatório `client-v1`. `aceito_automatico` segue a regra do Client
/// (`shmupx-client/src-tauri/src/analysis/offline.rs`): confiança alta, sem revisão, sem game
/// over, sem continue e com placar.
fn interpretar(bytes: &[u8], jogo: &str, versao: &str, duracao_segundos: u64) -> Resultado {
    let bruto: Value = match serde_json::from_slice(bytes) {
        Ok(valor) => valor,
        Err(e) => return Resultado::falha(format!("relatório inválido do script: {e}"), duracao_segundos),
    };
    let relatorio: Relatorio = match serde_json::from_value(bruto.clone()) {
        Ok(relatorio) => relatorio,
        Err(e) => return Resultado::falha(format!("relatório fora do formato client-v1: {e}"), duracao_segundos),
    };
    if relatorio.schema_version != 1 || relatorio.game_id != jogo || relatorio.validator_version != versao {
        return Resultado::falha("relatório de outro formato, jogo ou versão do script".into(), duracao_segundos);
    }
    let g = &relatorio.gameplay;
    let contadores_validos = g.score.is_none_or(|s| s >= 0) && g.lives >= 0 && g.continues >= 0;
    // Partida a 2 também vai para revisão (o ranking é de 1 jogador), como o relatório já marca.
    let aceito_automatico = contadores_validos
        && g.players.is_none_or(|p| p <= 1)
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
        jogadores: g.players,
        zerado: g.cleared,
        bombas: g.bombs_used,
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
        let r = interpretar(&relatorio("high", false, Some(123_400), 0, false), "earthion", "1.0.0", 10);
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
            let r = interpretar(&relatorio(confianca, revisao, placar, continues, game_over), "earthion", "1.0.0", 1);
            assert!(!r.aceito_automatico, "{confianca} {revisao} {placar:?} {continues} {game_over}");
            assert!(r.erro.is_none(), "leitura duvidosa não é erro do script");
        }
    }

    #[test]
    fn saida_invalida_ou_de_outro_jogo_vira_falha() {
        assert!(interpretar(b"nao e json", "earthion", "1.0.0", 1).erro.is_some());
        assert!(interpretar(b"{\"schema_version\":1}", "earthion", "1.0.0", 1).erro.is_some());
        assert!(interpretar(&relatorio("high", false, Some(1), 0, false), "outro", "1.0.0", 1).erro.is_some());
        assert!(interpretar(&relatorio("high", false, Some(1), 0, false), "earthion", "1.1.0", 1).erro.is_some(), "outra versão");
    }

    /// Contra a pasta validators real do Lab (só precisa do Python, não do OpenCV).
    /// `cargo test -- --ignored scripts_reais`
    #[tokio::test]
    #[ignore = "exige Python instalado"]
    async fn scripts_reais_incluem_o_earthion() {
        let raiz = raiz_validadores().unwrap();
        let scripts = listar_scripts(&raiz).await.unwrap();
        let earthion = scripts.iter().find(|s| s.id == "earthion" && s.version == "1.0.0").expect("earthion 1.0.0");
        assert_eq!(earthion.status, "approved");
    }

    #[test]
    fn le_o_progresso_publicado_pelo_script() {
        assert_eq!(progresso_da_linha("SHMUPX_PROGRESS 45"), Some(45.0));
        assert_eq!(progresso_da_linha("  SHMUPX_PROGRESS 100\r"), Some(100.0));
        assert_eq!(progresso_da_linha("SHMUPX_PROGRESS 140"), None);
        assert_eq!(progresso_da_linha("[calibração] variante=pc_pt"), None);
    }

    #[tokio::test]
    async fn stderr_separa_progresso_do_texto() {
        let (tx, rx) = watch::channel(0.0f32);
        let entrada: &[u8] = b"SHMUPX_PROGRESS 10\nlog qualquer\nSHMUPX_PROGRESS 55\nValueError: ruim\n";
        let resto = ler_stderr(entrada, &tx).await;
        assert_eq!(*rx.borrow(), 55.0);
        assert_eq!(String::from_utf8(resto).unwrap(), "log qualquer\nValueError: ruim");
    }

    #[test]
    fn ultima_linha_ignora_vazias_e_limita_o_tamanho() {
        assert_eq!(ultima_linha(b"Traceback\n  x\nValueError: ruim\n\n"), "ValueError: ruim");
        assert_eq!(ultima_linha(b""), "sem detalhe");
        assert_eq!(ultima_linha("x".repeat(900).as_bytes()).len(), 500);
    }
}
