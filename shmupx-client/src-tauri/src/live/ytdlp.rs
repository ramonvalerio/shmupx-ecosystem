//! yt-dlp: baixa do YouTube o vídeo de uma live que entrou no top 3, para o envio ao canal oficial
//! da ShmupX (a live não grava nada no PC). Vem com o app (`ytdlp/yt-dlp.exe` nos recursos); em
//! desenvolvimento, o do PATH.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use crate::capture::ffmpeg;

static EMPACOTADO: OnceLock<PathBuf> = OnceLock::new();

/// Quantas vezes tentar baixar, e o intervalo: logo depois do fim da live, o YouTube ainda está
/// preparando o vídeo dela (até 30 min).
const TENTATIVAS: u32 = 30;
const INTERVALO: Duration = Duration::from_secs(60);
/// Tempo máximo de um download (a live dura no máximo 45 min, em 720p).
const LIMITE_DO_DOWNLOAD: Duration = Duration::from_secs(20 * 60);

/// Registra o yt-dlp que vem com o app, se existir.
pub fn usar_empacotado(recursos: Option<PathBuf>) {
    if let Some(exe) = recursos
        .map(|dir| dir.join("ytdlp").join(if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" }))
        .filter(|p| p.is_file())
    {
        let _ = EMPACOTADO.set(exe);
    }
}

fn programa() -> PathBuf {
    EMPACOTADO.get().cloned().unwrap_or_else(|| PathBuf::from("yt-dlp"))
}

/// Id de vídeo do YouTube (11 caracteres `A-Z a-z 0-9 - _`): vai num endereço e num argumento.
fn id_valido(video_id: &str) -> bool {
    video_id.len() == 11 && video_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Argumentos do download: MP4 até 720p (a resolução da live), com o FFmpeg do app para juntar
/// vídeo e áudio, em `pasta/live.mp4`.
fn argumentos(video_id: &str, pasta: &Path) -> Vec<String> {
    let mut args: Vec<String> = ["--no-progress", "--no-playlist", "--no-warnings", "--force-overwrites"].map(String::from).to_vec();
    args.extend(["-f".into(), "bv*[height<=720]+ba/b[height<=720]/b".into(), "--merge-output-format".into(), "mp4".into()]);
    let ffmpeg = ffmpeg::programa();
    if ffmpeg.is_absolute() {
        args.extend(["--ffmpeg-location".into(), ffmpeg.to_string_lossy().into_owned()]);
    }
    args.extend(["-o".into(), pasta.join("live.%(ext)s").to_string_lossy().into_owned()]);
    args.push(format!("https://www.youtube.com/watch?v={video_id}"));
    args
}

async fn tentar(video_id: &str, pasta: &Path) -> Result<PathBuf, String> {
    let mut cmd = tokio::process::Command::new(programa());
    cmd.args(argumentos(video_id, pasta)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).kill_on_drop(true);
    // Sem janela de console (tiraria o foco de um jogo em tela cheia) e abaixo do normal.
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000 | 0x0000_4000); // CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS
    let saida = tokio::time::timeout(LIMITE_DO_DOWNLOAD, cmd.output())
        .await
        .map_err(|_| "o download do vídeo da live demorou demais".to_string())?
        .map_err(|e| format!("yt-dlp indisponível: {e}"))?;
    let arquivo = pasta.join("live.mp4");
    if saida.status.success() && arquivo.is_file() {
        return Ok(arquivo);
    }
    let erro = String::from_utf8_lossy(&saida.stderr);
    Err(ffmpeg::ultima_linha(&erro))
}

/// Baixa o vídeo da live `video_id` em `pasta` e devolve o arquivo. Enquanto o YouTube ainda está
/// preparando o vídeo, tenta de novo a cada minuto.
pub async fn baixar(video_id: &str, pasta: &Path) -> Result<PathBuf, String> {
    if !id_valido(video_id) {
        return Err(format!("id de vídeo do YouTube inválido: {video_id}"));
    }
    tokio::fs::create_dir_all(pasta).await.map_err(|e| format!("sem pasta para o vídeo da live: {e}"))?;
    let mut ultimo = String::new();
    for tentativa in 1..=TENTATIVAS {
        match tentar(video_id, pasta).await {
            Ok(arquivo) => return Ok(arquivo),
            Err(e) if e.starts_with("yt-dlp indisponível") => return Err(e),
            Err(e) => {
                log::info!("destaque: vídeo da live {video_id} ainda não baixou (tentativa {tentativa}): {e}");
                ultimo = e;
            }
        }
        tokio::time::sleep(INTERVALO).await;
    }
    Err(format!("o vídeo da live não ficou disponível no YouTube: {ultimo}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_ids_do_youtube() {
        assert!(id_valido("dQw4w9WgXcQ") && id_valido("a-b_c-d_e-1"));
        assert!(!id_valido("curto") && !id_valido("dQw4w9WgXc&") && !id_valido("dQw4w9WgXcQx"));
    }

    #[test]
    fn baixa_em_mp4_ate_720p_no_endereco_do_video() {
        let args = argumentos("dQw4w9WgXcQ", Path::new("C:/tmp/destaque"));
        assert_eq!(args.last().unwrap(), "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert!(args.windows(2).any(|w| w[0] == "--merge-output-format" && w[1] == "mp4"));
        assert!(args.windows(2).any(|w| w[0] == "-o" && w[1].ends_with("live.%(ext)s")));
    }
}
