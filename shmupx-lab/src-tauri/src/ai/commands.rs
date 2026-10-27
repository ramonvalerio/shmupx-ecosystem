use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::anthropic::Config;
use super::cache::Cache;
use super::steam::{self, JogoSteam};
use super::sugestao::{self, Sugestao};

#[derive(Debug, Serialize)]
pub struct RespostaSugestoes {
    pub sugestoes: Vec<Sugestao>,
    /// `true` quando veio do cache local: nenhuma chamada à API foi feita nem cobrada.
    pub do_cache: bool,
    /// Modelo usado (`None` quando veio do cache).
    pub modelo: Option<String>,
    /// A loja da Steam foi consultada e conferiu o resultado: um `steam_app_id` presente é confirmado.
    pub steam_consultada: bool,
    /// Quantos jogos a loja da Steam devolveu para o título.
    pub jogos_na_steam: usize,
    /// Avisos para mostrar ao admin (ex.: a Steam não respondeu e a IA foi usada sozinha).
    pub avisos: Vec<String>,
}

/// Sugere metadados do jogo a partir do título. Consulta o cache local primeiro. `steam` busca
/// o jogo na loja da Steam (gratuito) e usa os dados dela como fonte confiável; `web` ancora a
/// resposta numa pesquisa na internet (mais cara); `ignorar_cache` força uma consulta nova.
#[tauri::command]
pub async fn sugerir_jogo(
    app: AppHandle,
    titulo: String,
    web: bool,
    steam: bool,
    ignorar_cache: bool,
) -> Result<RespostaSugestoes, String> {
    let pasta = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))?;
    // A versão no nome invalida respostas guardadas antes de uma mudança no tratamento delas
    // (v2: a versão da Steam passou a entrar mesmo quando a IA não a devolve).
    let cache = Cache::new(pasta.join("ai_cache_v2.json"));

    if !ignorar_cache {
        if let Some(sugestoes) = cache.buscar(&titulo, web, steam) {
            return Ok(RespostaSugestoes {
                sugestoes,
                do_cache: true,
                modelo: None,
                // O cache só atende um pedido com Steam se a entrada foi conferida por ela.
                steam_consultada: steam,
                jogos_na_steam: 0,
                avisos: vec![],
            });
        }
    }

    // A chave é exigida antes de consultar a Steam: sem ela nada do fluxo adianta.
    let config = Config::do_ambiente()?;

    // A Steam é opcional: se ela falhar, a IA segue sozinha e o admin é avisado.
    let mut avisos = Vec::new();
    let mut jogos_steam: Option<Vec<JogoSteam>> = None;
    if steam {
        match steam::buscar(&titulo).await {
            Ok(jogos) => jogos_steam = Some(jogos),
            Err(erro) => avisos.push(format!("A Steam não respondeu ({erro}); usei só a IA.")),
        }
    }

    let sugestoes = sugestao::sugerir(&config, &titulo, web, jogos_steam.as_deref()).await?;
    // Falha ao gravar o cache não invalida uma resposta que já foi paga. Se a Steam falhou, a
    // entrada fica marcada sem Steam, e uma consulta futura com ela tenta de novo.
    if let Err(erro) = cache.guardar(&titulo, web, jogos_steam.is_some(), &sugestoes) {
        eprintln!("cache de sugestões: {erro}");
    }
    Ok(RespostaSugestoes {
        sugestoes,
        do_cache: false,
        modelo: Some(config.modelo),
        steam_consultada: jogos_steam.is_some(),
        jogos_na_steam: jogos_steam.as_ref().map_or(0, Vec::len),
        avisos,
    })
}
