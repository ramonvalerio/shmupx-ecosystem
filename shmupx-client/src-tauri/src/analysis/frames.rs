use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

/// Quantos segundos do FINAL do vídeo amostrar — a tela de placar/Game Over fatalmente está
/// nesse intervalo, e ler só isso evita decodificar a gravação inteira (que pode ter até 60min,
/// Spec 002).
const JANELA_FINAL_SEGUNDOS: u32 = 5;
/// Taxa de amostragem dentro da janela final.
const FPS_AMOSTRAGEM: u32 = 2;

/// Extrai frames do final de `video_path` como PNGs numerados em `output_dir` (criado se não
/// existir). Usa `-sseof` (seek a partir do fim do arquivo) para não precisar decodificar o
/// vídeo inteiro até ali — essa é a "amostragem eficiente" em vez de ler frame a frame do
/// começo.
pub async fn extrair_frames_finais(
    video_path: &Path,
    output_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    tokio::fs::create_dir_all(output_dir).await.map_err(|e| {
        format!(
            "falha ao criar o diretório de frames ({}): {e}",
            output_dir.display()
        )
    })?;

    let padrao_saida = output_dir.join("frame_%03d.png");

    let mut comando = Command::new(crate::capture::ffmpeg::programa());
    comando
        .args(["-y", "-sseof", &format!("-{JANELA_FINAL_SEGUNDOS}")])
        .arg("-i")
        .arg(video_path)
        .args(["-vf", &format!("fps={FPS_AMOSTRAGEM}")])
        .arg(&padrao_saida)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Sem janela de console (ela tomaria o foco do jogo da run seguinte, que pode estar em tela
    // cheia) e abaixo do normal, como o motor de validação.
    #[cfg(windows)]
    comando.creation_flags(0x0800_0000 | 0x0000_4000); // CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS
    let status = comando
        .status()
        .await
        .map_err(|e| format!("falha ao executar o ffmpeg para extrair frames: {e}"))?;

    if !status.success() {
        return Err(format!(
            "ffmpeg terminou com erro ao extrair frames de {} (vídeo corrompido, vazio, ou \
             mais curto que a janela de {JANELA_FINAL_SEGUNDOS}s amostrada?)",
            video_path.display()
        ));
    }

    let mut entradas = tokio::fs::read_dir(output_dir)
        .await
        .map_err(|e| format!("falha ao listar os frames extraídos: {e}"))?;

    let mut frames = Vec::new();
    while let Some(entrada) = entradas
        .next_entry()
        .await
        .map_err(|e| format!("falha ao listar os frames extraídos: {e}"))?
    {
        let caminho = entrada.path();
        if caminho.extension().and_then(|ext| ext.to_str()) == Some("png") {
            frames.push(caminho);
        }
    }
    frames.sort();

    if frames.is_empty() {
        return Err(format!(
            "nenhum frame extraído de {} — vídeo vazio ou corrompido",
            video_path.display()
        ));
    }

    Ok(frames)
}
