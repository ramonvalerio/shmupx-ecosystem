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
    #[serde(rename = "game-over")]
    game_over: bool,
    observation: Option<String>,
}

fn parse_report(bytes: &[u8], game: &str) -> Result<AnalysisResult, String> {
    let report: Report = serde_json::from_slice(bytes)
        .map_err(|e| format!("resposta inválida do validador offline: {e}"))?;
    if report.schema_version != 1 || report.game_id != game || report.validator_version != "1.0.0" {
        return Err("versão ou jogo incompatível no resultado offline".into());
    }
    if report.needs_review
        || report.confidence != "high"
        || report.gameplay.game_over
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

fn offline_root(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(root) = std::env::var_os("SHMUPX_OFFLINE_ROOT") {
        return Ok(PathBuf::from(root));
    }
    if cfg!(debug_assertions) {
        return Ok(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shmupx_offline"));
    }
    app.path()
        .resource_dir()
        .map(|path| path.join("shmupx_offline"))
        .map_err(|e| e.to_string())
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
    // Mapeamento explícito. IDs de banco deverão ser associados ao slug pelo catálogo real.
    if game != "earthion" {
        return Err(format!(
            "não há validador offline cadastrado no cliente para {game}"
        ));
    }
    let root = offline_root(app)?
        .canonicalize()
        .map_err(|e| format!("pasta do validador indisponível: {e}"))?;
    let script = root.join("analyze.py");
    if !script.is_file() {
        return Err("validador offline não instalado; configure SHMUPX_OFFLINE_ROOT".into());
    }
    let video = video
        .canonicalize()
        .map_err(|e| format!("vídeo indisponível: {e}"))?;
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
    let mut command = Command::new(python);
    command
        .arg(script)
        .arg(video)
        .args([
            "--game",
            game,
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
        .map_err(|e| format!("não foi possível iniciar Python: {e}"))?;
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
    parse_report(&stdout, game)
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
        parse_report(&serde_json::to_vec(value).unwrap(), "earthion")
    }
    #[test]
    fn accepts_measured_score() {
        assert_eq!(parse(&valid()).unwrap().score_bruto, 1234);
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
            ("game-over", serde_json::json!(true)),
        ] {
            let mut report = valid();
            report["gameplay"][key] = value;
            assert!(parse(&report).is_err());
        }
        assert!(parse_report(b"not json", "earthion").is_err());
    }
    #[tokio::test]
    async fn bounds_output() {
        assert_eq!(bounded_read(&b"abc"[..], 3).await.unwrap(), b"abc");
        assert!(bounded_read(&b"abcd"[..], 3).await.is_err());
    }
}
