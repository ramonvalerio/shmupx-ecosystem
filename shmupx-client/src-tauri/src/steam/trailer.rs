//! Trailer da loja da Steam para o gabinete arcade (a tela do gabinete mostra o jogo em movimento).
//! A API da loja (`appdetails`) não libera CORS para a página, então a consulta passa pelo Rust.
//! A loja hoje anuncia só HLS/DASH; o MP4 de 480p continua no endereço clássico pelo id do vídeo.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

/// Resultado por app_id (inclusive "sem trailer"), para não consultar a loja a cada abertura.
static CACHE: Mutex<Option<HashMap<u32, Option<String>>>> = Mutex::new(None);

fn mp4_do_video(id: u64) -> String {
    format!("https://cdn.akamai.steamstatic.com/steam/apps/{id}/movie480.mp4")
}

/// Primeiro trailer do jogo (o destacado, se houver) a partir da resposta de `appdetails`.
fn escolher(resposta: &Value, app_id: u32) -> Option<String> {
    let filmes = resposta[app_id.to_string()]["data"]["movies"].as_array()?;
    let filme = filmes.iter().find(|f| f["highlight"].as_bool() == Some(true)).or_else(|| filmes.first())?;
    // Formato antigo da API: o MP4 vem pronto; o atual só traz o id.
    if let Some(url) = filme["mp4"]["480"].as_str() {
        return Some(url.replace("http://", "https://"));
    }
    filme["id"].as_u64().map(mp4_do_video)
}

/// URL de um MP4 do trailer do jogo na Steam, ou `None` se a loja não tiver vídeo.
#[tauri::command]
pub async fn steam_trailer(app_id: u32) -> Option<String> {
    if app_id == 0 {
        return None;
    }
    if let Some(guardado) = CACHE.lock().ok().and_then(|c| c.as_ref().and_then(|m| m.get(&app_id).cloned())) {
        return guardado;
    }
    let url = format!("https://store.steampowered.com/api/appdetails?appids={app_id}&filters=movies");
    let resposta: Option<Value> = async {
        let cliente = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().ok()?;
        cliente.get(url).send().await.ok()?.json().await.ok()
    }
    .await;
    let trailer = resposta.as_ref().and_then(|r| escolher(r, app_id));
    // Só guarda quando a loja respondeu (uma falha de rede tenta de novo na próxima vez).
    if resposta.is_some() {
        if let Ok(mut cache) = CACHE.lock() {
            cache.get_or_insert_with(HashMap::new).insert(app_id, trailer.clone());
        }
    }
    trailer
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn usa_o_id_do_video_destacado() {
        let r = json!({ "2590180": { "success": true, "data": { "movies": [
            { "id": 1, "name": "Outro", "highlight": false },
            { "id": 257007023, "name": "Gameplay", "highlight": true, "hls_h264": "x.m3u8" },
        ] } } });
        assert_eq!(escolher(&r, 2590180).unwrap(), "https://cdn.akamai.steamstatic.com/steam/apps/257007023/movie480.mp4");
    }

    #[test]
    fn aceita_o_formato_antigo_com_mp4() {
        let r = json!({ "10": { "data": { "movies": [{ "id": 5, "mp4": { "480": "http://cdn/x/movie480.mp4" } }] } } });
        assert_eq!(escolher(&r, 10).unwrap(), "https://cdn/x/movie480.mp4");
    }

    #[test]
    fn sem_filmes_e_none() {
        assert!(escolher(&json!({ "10": { "success": false } }), 10).is_none());
        assert!(escolher(&json!({ "10": { "data": { "movies": [] } } }), 10).is_none());
    }
}
