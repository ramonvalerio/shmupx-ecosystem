//! O FFmpeg do Client: onde ele está (o empacotado ou o do PATH), se executa, o motivo de uma falha
//! (última linha do log) e a parada graciosa. A gravação da run em si fica em `gravacao`.

use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Child;

static EMPACOTADO: OnceLock<PathBuf> = OnceLock::new();

/// Registra o FFmpeg que vem com o app (`ffmpeg/ffmpeg.exe` nos recursos), se existir. Em
/// desenvolvimento ele não existe e vale o do PATH.
pub fn usar_empacotado(recursos: Option<PathBuf>) {
    if let Some(exe) = recursos
        .map(|dir| dir.join("ffmpeg").join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }))
        .filter(|p| p.is_file())
    {
        let _ = EMPACOTADO.set(exe);
    }
}

/// Programa a executar: o empacotado ou, sem ele, `ffmpeg` do PATH.
pub fn programa() -> PathBuf {
    EMPACOTADO
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

/// Verifica se o FFmpeg (empacotado ou do PATH) executa.
pub fn is_ffmpeg_available() -> bool {
    let mut cmd = std::process::Command::new(programa());
    cmd.arg("-version").stdout(Stdio::null()).stderr(Stdio::null());
    // Sem `CREATE_NO_WINDOW`, o Client instalado (sem console) abre uma janela de console para o
    // FFmpeg: ela toma o primeiro plano e o jogo em tela cheia, que acabou de abrir, minimiza.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.status().map(|status| status.success()).unwrap_or(false)
}

/// Última linha útil do log do FFmpeg (as anteriores costumam ser banner e configuração).
pub fn ultima_linha(log: &str) -> String {
    log.lines()
        .rev()
        .map(str::trim)
        .find(|linha| !linha.is_empty())
        .map(|linha| linha.chars().take(300).collect())
        .unwrap_or_else(|| "sem detalhe no log do FFmpeg".to_string())
}

/// Parada graciosa de um FFmpeg que lê comandos do stdin: envia `q` (fecha o arquivo
/// corretamente) e aguarda a saída do processo; se não finalizar dentro do timeout, força o
/// encerramento (`kill`).
#[cfg_attr(windows, allow(dead_code))]
pub async fn graceful_stop(child: &mut Child) -> io::Result<()> {
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"q\n").await;
        let _ = stdin.shutdown().await;
    }

    match tokio::time::timeout(Duration::from_secs(3), child.wait()).await {
        Ok(_) => Ok(()),
        Err(_) => child.kill().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ultima_linha_do_log_explica_a_falha() {
        let log = "ffmpeg version 8.0\n  built with gcc\nError opening input file audio=virtual-audio-capturer.\nError opening input files: I/O error\n\n";
        assert_eq!(ultima_linha(log), "Error opening input files: I/O error");
        assert_eq!(ultima_linha(""), "sem detalhe no log do FFmpeg");
    }
}
