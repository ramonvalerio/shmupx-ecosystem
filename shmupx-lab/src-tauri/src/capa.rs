//! Capa do card do jogo no Client: o Lab recorta e comprime a imagem (WebP 16:9) no front e
//! este comando a envia a `www.shmupx.com/catalog/covers/`, no mesmo bucket e CloudFront do site.
//! O banner de um evento (WebP 3:1) vai pelo mesmo caminho para `www.shmupx.com/events/banners/`.
//!
//! O nome leva o id do jogo e o instante do envio: cada versão da capa é um arquivo novo, então
//! pode ficar em cache para sempre e trocar a capa nunca mostra a antiga por engano.

use std::time::{SystemTime, UNIX_EPOCH};

use tokio::process::Command;

use crate::instalador::{credenciais_aws, BUCKET};

/// Pastas aceitas no site: a capa do card do jogo e o banner do carrossel de eventos.
fn pasta_no_site(pasta: Option<&str>) -> Option<&'static str> {
    match pasta.unwrap_or("covers") {
        "covers" => Some("catalog/covers"),
        "banners" => Some("events/banners"),
        _ => None,
    }
}
/// Bem acima de um WebP 1280×720 (~100–300 KB); barra envio de arquivo errado.
const TAMANHO_MAXIMO: usize = 3 * 1024 * 1024;

fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// RIFF....WEBP: só aceita o que o editor do Lab produz.
fn e_webp(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

/// Envia a capa (WebP) do jogo ao S3 e devolve a URL pública para gravar no catálogo.
#[tauri::command]
pub async fn publicar_capa(game_id: String, bytes: Vec<u8>, pasta: Option<String>) -> Result<String, String> {
    if !id_valido(&game_id) {
        return Err("id inválido".into());
    }
    let destino = pasta_no_site(pasta.as_deref()).ok_or("pasta de imagem inválida")?;
    if !e_webp(&bytes) {
        return Err("a capa precisa estar em WebP".into());
    }
    if bytes.len() > TAMANHO_MAXIMO {
        return Err("capa acima de 3 MB".into());
    }
    let instante = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_millis();
    let nome = format!("{game_id}-{instante}.webp");
    let temporario = std::env::temp_dir().join(format!("shmupx-capa-{nome}"));
    tokio::fs::write(&temporario, &bytes).await.map_err(|e| format!("não foi possível preparar a capa: {e}"))?;

    let credenciais = credenciais_aws().await;
    let resultado = match credenciais {
        Err(e) => Err(e),
        Ok(credenciais) => Command::new("aws")
            .arg("s3")
            .arg("cp")
            .arg(&temporario)
            .arg(format!("s3://{BUCKET}/{destino}/{nome}"))
            .args(["--content-type", "image/webp"])
            .args(["--cache-control", "public,max-age=31536000,immutable"])
            .arg("--only-show-errors")
            .env("AWS_ACCESS_KEY_ID", credenciais.AWS_ACCESS_KEY_ID)
            .env("AWS_SECRET_ACCESS_KEY", credenciais.AWS_SECRET_ACCESS_KEY)
            .env("AWS_DEFAULT_REGION", "us-east-1")
            .output()
            .await
            .map_err(|e| format!("AWS CLI (aws) indisponível: {e}")),
    };
    let _ = tokio::fs::remove_file(&temporario).await;
    let saida = resultado?;
    if !saida.status.success() {
        return Err(format!("falha ao enviar a capa ao S3: {}", String::from_utf8_lossy(&saida.stderr).trim()));
    }
    Ok(format!("https://www.shmupx.com/{destino}/{nome}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_ids_e_webp_validos() {
        assert!(id_valido("earthion"));
        assert!(!id_valido("../x"));
        assert!(e_webp(b"RIFF\0\0\0\0WEBPVP8 "));
        assert_eq!(pasta_no_site(None), Some("catalog/covers"));
        assert_eq!(pasta_no_site(Some("banners")), Some("events/banners"));
        assert_eq!(pasta_no_site(Some("../downloads")), None);
        assert!(!e_webp(b"\x89PNG\r\n\x1a\n...."));
    }
}
