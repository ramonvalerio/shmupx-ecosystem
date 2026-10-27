use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::anthropic::Config;
use super::cache::Cache;
use super::sugestao::{self, Sugestao};

#[derive(Debug, Serialize)]
pub struct RespostaSugestoes {
    pub sugestoes: Vec<Sugestao>,
    /// `true` quando veio do cache local: nenhuma chamada à API foi feita nem cobrada.
    pub do_cache: bool,
    /// Modelo usado (`None` quando veio do cache).
    pub modelo: Option<String>,
}

/// Sugere metadados do jogo a partir do título. Consulta o cache local primeiro; `web` ancora a
/// resposta numa pesquisa na internet (mais cara) e `ignorar_cache` força uma consulta nova.
#[tauri::command]
pub async fn sugerir_jogo(
    app: AppHandle,
    titulo: String,
    web: bool,
    ignorar_cache: bool,
) -> Result<RespostaSugestoes, String> {
    let pasta = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))?;
    let cache = Cache::new(pasta.join("ai_cache.json"));

    if !ignorar_cache {
        if let Some(sugestoes) = cache.buscar(&titulo, web) {
            return Ok(RespostaSugestoes { sugestoes, do_cache: true, modelo: None });
        }
    }

    let config = Config::do_ambiente()?;
    let sugestoes = sugestao::sugerir(&config, &titulo, web).await?;
    // Falha ao gravar o cache não invalida uma resposta que já foi paga.
    if let Err(erro) = cache.guardar(&titulo, web, &sugestoes) {
        eprintln!("cache de sugestões: {erro}");
    }
    Ok(RespostaSugestoes { sugestoes, do_cache: false, modelo: Some(config.modelo) })
}
