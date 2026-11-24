//! Execução dos scripts locais por jogo (ADR 004). Nenhum fallback de pontuação.
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

#[derive(Debug)]
pub struct AnalysisResult {
    pub score_bruto: i64,
    pub vidas_perdidas: i32,
    pub continues_usados: i32,
}

#[derive(Deserialize)]
struct Report {
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
    // `game-over` também vem no relatório, mas não decide o envio: é o fim normal da ficha.
    observation: Option<String>,
}

fn parse_report(bytes: &[u8], game: &str, version: &str) -> Result<AnalysisResult, String> {
    let report: Report = serde_json::from_slice(bytes)
        .map_err(|e| format!("resposta inválida do validador offline: {e}"))?;
    if report.schema_version != 1 || report.game_id != game || report.validator_version != version {
        return Err("versão ou jogo incompatível no resultado offline".into());
    }
    // Game over (morreu na última vida, sem continue) é o fim normal de uma partida de 1 ficha:
    // o placar final do HUD vale. Continue ou leitura duvidosa ficam de fora.
    if report.needs_review
        || report.confidence != "high"
        || report.gameplay.continues > 0
    {
        return Err(format!(
            "gameplay precisa de revisão: {}",
            report
                .gameplay
                .observation
                .as_deref()
                .unwrap_or("leitura inconclusiva ou regra heurística")
        ));
    }
    let score = report
        .gameplay
        .score
        .ok_or("placar não identificado; revisão necessária")?;
    if score < 0 || report.gameplay.lives < 0 || report.gameplay.continues < 0 {
        return Err("o validador retornou contadores negativos".into());
    }
    Ok(AnalysisResult {
        score_bruto: score,
        vidas_perdidas: report.gameplay.lives,
        continues_usados: report.gameplay.continues,
    })
}

#[derive(Deserialize)]
struct ScriptAceito {
    game: String,
    version: String,
}

/// Scripts de validação que este Client aceita: jogo → versão aprovada no Lab. Enquanto a
/// publicação de scripts (ADR 006/007) não existe, a lista acompanha a versão do Client; o
/// `scripts/prepare-bundle.ps1` lê o mesmo arquivo para empacotar só esses scripts.
pub(crate) fn versao_aceita(game: &str) -> Option<String> {
    let aceitos: Vec<ScriptAceito> =
        serde_json::from_str(include_str!("../../scripts-aceitos.json"))
            .expect("scripts-aceitos.json inválido");
    aceitos
        .into_iter()
        .find(|s| s.game == game)
        .map(|s| s.version)
}

/// Como rodar o motor de validação.
enum Motor {
    /// `shmupx-validator.exe` compilado (sem Python na máquina do jogador).
    Exe(PathBuf),
    /// `python analyze.py`, só em desenvolvimento, direto da pasta do Lab.
    Python {
        python: std::ffi::OsString,
        script: PathBuf,
    },
}

/// Motor e pasta `games/<jogo>/<versão>/`. App instalado: os dois vêm dos recursos do pacote
/// (`validator/` e `games/`). Desenvolvimento: a pasta `validators` do Lab no repositório (ou
/// `SHMUPX_VALIDATORS_ROOT`) com Python; `SHMUPX_VALIDATOR_EXE` força o `.exe` para testá-lo.
fn localizar_motor(app: &AppHandle) -> Result<(Motor, PathBuf), String> {
    let exe_forcado = std::env::var_os("SHMUPX_VALIDATOR_EXE").map(PathBuf::from);
    let root = if let Some(root) = std::env::var_os("SHMUPX_VALIDATORS_ROOT") {
        Some(PathBuf::from(root))
    } else if cfg!(debug_assertions) {
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shmupx-lab/validators"))
    } else {
        None
    };
    let Some(root) = root else {
        let recursos = app.path().resource_dir().map_err(|e| e.to_string())?;
        let exe = exe_forcado.unwrap_or_else(|| recursos.join(if cfg!(windows) { "validator/shmupx-validator.exe" } else { "validator/shmupx-validator" }));
        return Ok((Motor::Exe(exe), recursos.join("games")));
    };
    let root = sem_prefixo_estendido(
        root.canonicalize()
            .map_err(|e| format!("pasta do validador indisponível: {e}"))?,
    );
    let games = root.join("games");
    if let Some(exe) = exe_forcado {
        return Ok((Motor::Exe(exe), games));
    }
    let python = std::env::var_os("SHMUPX_PYTHON").unwrap_or_else(|| {
        let local = root.join(if cfg!(windows) {
            ".venv/Scripts/python.exe"
        } else {
            ".venv/bin/python"
        });
        if local.is_file() {
            local.into_os_string()
        } else {
            "python".into()
        }
    });
    Ok((
        Motor::Python {
            python,
            script: root.join("analyze.py"),
        },
        games,
    ))
}

/// `canonicalize` no Windows devolve `\\?\D:\...`: nesse formato o Windows não aceita `/`, e o
/// Python monta caminhos de imagens com `/` (ex.: `labels/hud_score.png`). Tira o prefixo
/// quando o caminho é de unidade comum (`\\?\D:\`); caminhos de rede (`\\?\UNC\`) ficam como estão.
fn sem_prefixo_estendido(caminho: PathBuf) -> PathBuf {
    let texto = caminho.to_string_lossy();
    match texto.strip_prefix(r"\\?\") {
        Some(resto) if resto.as_bytes().get(1) == Some(&b':') => PathBuf::from(resto),
        _ => caminho,
    }
}

/// Drena o pipe inteiro sem deixar o processo bloquear nem acumular logs ilimitados.
async fn bounded_read(
    mut reader: impl AsyncRead + Unpin,
    limit: usize,
) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0; 8192];
    let mut exceeded = false;
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        let keep = count.min(limit.saturating_sub(output.len()));
        output.extend_from_slice(&buffer[..keep]);
        exceeded |= keep < count;
    }
    if exceeded {
        return Err(std::io::Error::other("saída do validador excedeu o limite"));
    }
    Ok(output)
}

pub async fn analisar_run(
    app: &AppHandle,
    video: &Path,
    game: &str,
) -> Result<AnalysisResult, String> {
    let Some(version) = versao_aceita(game) else {
        return Err(format!(
            "não há validador offline cadastrado no cliente para {game}"
        ));
    };
    let version = version.as_str();
    let (motor, games) = localizar_motor(app)?;
    let games = sem_prefixo_estendido(
        games
            .canonicalize()
            .map_err(|e| format!("scripts dos jogos indisponíveis ({}): {e}", games.display()))?,
    );
    let video = sem_prefixo_estendido(
        video
            .canonicalize()
            .map_err(|e| format!("vídeo indisponível: {e}"))?,
    );
    let mut command = match &motor {
        Motor::Exe(exe) => {
            if !exe.is_file() {
                return Err(format!(
                    "motor de validação não instalado: {}",
                    exe.display()
                ));
            }
            Command::new(exe)
        }
        Motor::Python { python, script } => {
            if !script.is_file() {
                return Err(
                    "validador offline não instalado; configure SHMUPX_VALIDATORS_ROOT".into(),
                );
            }
            let mut command = Command::new(python);
            command.arg(script);
            command
        }
    };
    command
        .arg(video)
        .arg("--games-dir")
        .arg(&games)
        .args([
            "--game",
            game,
            "--version",
            version,
            "--player",
            "local",
            "--format",
            "client-v1",
            "--quiet",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    // Sem janela e com prioridade abaixo do normal: a análise pode rodar enquanto o jogador já
    // está em outra run, e não deve roubar CPU do jogo nem da gravação.
    command.creation_flags(0x08000000 | 0x00004000); // CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS
    let mut child = command
        .spawn()
        .map_err(|e| format!("não foi possível iniciar o motor de validação: {e}"))?;
    let stdout = child.stdout.take().ok_or("stdout indisponível")?;
    let stderr = child.stderr.take().ok_or("stderr indisponível")?;
    let process = async {
        tokio::try_join!(
            child.wait(),
            bounded_read(stdout, 1_048_576),
            bounded_read(stderr, 65_536)
        )
    };
    let (status, stdout, stderr) =
        match tokio::time::timeout(Duration::from_secs(900), process).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                let _ = child.kill().await;
                return Err(format!("falha ao ler resultado offline: {error}"));
            }
            Err(_) => {
                let _ = child.kill().await;
                return Err("validação excedeu 15 minutos; processo encerrado".into());
            }
        };
    if !status.success() {
        return Err(format!(
            "validação offline falhou: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    parse_report(&stdout, game, version)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid() -> serde_json::Value {
        serde_json::json!({"schema_version":1,"game_id":"earthion","validator_version":"1.0.0",
            "confidence":"high","needs_review":false,"gameplay":{"hi-score":1234,
            "lives-lost":2,"continues":0,"game-over":false,"observation":null}})
    }
    fn parse(value: &serde_json::Value) -> Result<AnalysisResult, String> {
        parse_report(&serde_json::to_vec(value).unwrap(), "earthion", "1.0.0")
    }
    #[test]
    fn accepted_scripts_file_is_valid() {
        assert_eq!(versao_aceita("earthion").as_deref(), Some("1.0.0"));
        assert_eq!(versao_aceita("desconhecido"), None);
    }
    #[test]
    fn strips_extended_prefix_only_for_drive_paths() {
        assert_eq!(
            sem_prefixo_estendido(PathBuf::from(r"\\?\D:\repos\x")),
            PathBuf::from(r"D:\repos\x")
        );
        assert_eq!(
            sem_prefixo_estendido(PathBuf::from(r"\\?\UNC\srv\x")),
            PathBuf::from(r"\\?\UNC\srv\x")
        );
        assert_eq!(
            sem_prefixo_estendido(PathBuf::from(r"D:\x")),
            PathBuf::from(r"D:\x")
        );
    }
    #[test]
    fn accepts_measured_score() {
        assert_eq!(parse(&valid()).unwrap().score_bruto, 1234);
    }
    #[test]
    fn accepts_game_over_without_continue() {
        let mut report = valid();
        report["gameplay"]["game-over"] = serde_json::json!(true);
        report["gameplay"]["hi-score"] = serde_json::json!(918_200);
        assert_eq!(parse(&report).unwrap().score_bruto, 918_200);
    }
    #[test]
    fn rejects_mismatched_or_uncertain_results() {
        for (key, value) in [
            ("game_id", serde_json::json!("other")),
            ("schema_version", serde_json::json!(2)),
            ("validator_version", serde_json::json!("2.0.0")),
            ("confidence", serde_json::json!("low")),
            ("needs_review", serde_json::json!(true)),
        ] {
            let mut report = valid();
            report[key] = value;
            assert!(parse(&report).is_err());
        }
    }
    #[test]
    fn rejects_missing_negative_or_heuristic_metrics() {
        for (key, value) in [
            ("hi-score", serde_json::Value::Null),
            ("hi-score", serde_json::json!(-1)),
            ("lives-lost", serde_json::json!(-1)),
            ("continues", serde_json::json!(-1)),
            ("continues", serde_json::json!(1)),
        ] {
            let mut report = valid();
            report["gameplay"][key] = value;
            assert!(parse(&report).is_err());
        }
        assert!(parse_report(b"not json", "earthion", "1.0.0").is_err());
    }
    #[tokio::test]
    async fn bounds_output() {
        assert_eq!(bounded_read(&b"abc"[..], 3).await.unwrap(), b"abc");
        assert!(bounded_read(&b"abcd"[..], 3).await.is_err());
    }
}
