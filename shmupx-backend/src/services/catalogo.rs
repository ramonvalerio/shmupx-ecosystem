//! Catálogo de jogos: `games/{gameId}` no Firestore (ADR 006: um jogo, uma sala, um RobotX, um
//! validador, todos pelo mesmo `gameId`).
//!
//! Quem escreve é o admin, pelo Lab, através das rotas `/api/admin/games`. O Client só lê os
//! jogos `published` direto do Firestore (Security Rules); `gameplays::iniciar` exige o mesmo
//! campo `published`, então despublicar um jogo também impede novas runs.

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::firestore::valor::{
    booleano, data, inteiro, lista_de_textos, ler_booleano, ler_inteiro, ler_lista_de_textos, ler_texto, texto,
};
use crate::firestore::{Campos, DocumentoComDatas, Firestore};
use crate::services::gameplays::id_valido;
use crate::services::versoes::Mudanca;

const DURACAO_PADRAO_SEGUNDOS: i64 = 3600;
const DURACAO_MAXIMA_SEGUNDOS: i64 = 7200;
/// Tempo do Caravan quando o Lab não definiu (a caravana clássica, 5 min). O jogo fecha nesse tempo.
pub const CARAVAN_PADRAO_SEGUNDOS: i64 = 300;
/// Tempo do Caravan aceito no Lab: de 30 s a 1 h.
pub const CARAVAN_MINIMO_SEGUNDOS: i64 = 30;
pub const CARAVAN_MAXIMO_SEGUNDOS: i64 = 3600;
const TITULO_MAXIMO: usize = 80;
const TEXTO_MAXIMO: usize = 80;
const ANO_MINIMO: i64 = 1970;
const ANO_MAXIMO: i64 = 2100;
/// Capas do card no Client: só as que o Lab envia ao bucket do site (CloudFront).
const PREFIXO_CAPA: &str = "https://www.shmupx.com/catalog/covers/";
const URL_MAXIMA: usize = 300;

/// Plataformas do catálogo (as mesmas do Lab antigo). Qualquer outro valor é recusado.
const PLATAFORMAS: [&str; 9] =
    ["steam", "mister", "megadrive", "neogeo", "arcade", "xbox360", "playstation", "switch", "other"];
/// Onde o jogo pode ser jogado no ShmupX (mais de uma): Steam, emulador (Fightcade/FBNeo) e MiSTer.
const ONDE_JOGAR: [&str; 3] = ["steam", "emulator", "mister"];
/// Formatos competitivos que o jogo suporta: pontuação, 1 crédito e caravana.
const MODOS: [&str; 3] = ["highscore", "1cc", "caravan"];

/// Campos que o Lab define. O `gameId` vem da rota, nunca do corpo. Os metadados opcionais
/// (desenvolvedora em diante) substituem os anteriores a cada gravação: ausente = apagado.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JogoEntrada {
    pub title: String,
    /// `0` = jogo sem Steam (Fightcade, emulador): a detecção por Steam simplesmente não casa.
    pub steam_app_id: i64,
    pub max_duration_segundos: i64,
    pub published: bool,
    #[serde(default)]
    pub developer: Option<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub year: Option<i64>,
    #[serde(default)]
    pub platform: Option<String>,
    /// Plataformas em que se joga o jogo no ShmupX (`steam`, `emulator`, `mister`).
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub modes: Vec<String>,
    /// Imagem do card (16:9, WebP), enviada pelo Lab. Ausente = cartão tipográfico.
    #[serde(default)]
    pub cover_url: Option<String>,
    /// Ranked no MiSTer: o agente lê o placar da memória do jogo no MiSTer (hoje só o Earthion, no
    /// core de Mega Drive). Desligado, o jogo no MiSTer é só Training. Exige MiSTer em `platforms`.
    #[serde(default)]
    pub ranked_mister: bool,
    /// Sem proteção da Steam (SteamStub/DRM): o Client abre o executável do jogo direto, sem abrir a
    /// Steam. Exige Steam em `platforms` e um `steam_app_id` (é a instalação da Steam que é aberta).
    #[serde(default)]
    pub no_steam_protection: bool,
    /// Tempo do Caravan em segundos: na partida Caravan, o jogo fecha nesse tempo e o placar feito
    /// até ali vale. Ausente = `CARAVAN_PADRAO_SEGUNDOS`. Menor que `max_duration_segundos`.
    #[serde(default)]
    pub caravan_segundos: Option<i64>,
}

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct Jogo {
    pub id: String,
    pub title: String,
    pub steam_app_id: i64,
    pub max_duration_segundos: i64,
    pub published: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    pub platforms: Vec<String>,
    pub modes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    /// Ranked no MiSTer liberado (leitura de placar pela memória no agente do MiSTer).
    pub ranked_mister: bool,
    /// Jogo da Steam sem proteção: o Client abre o executável direto, sem a Steam.
    pub no_steam_protection: bool,
    /// Tempo do Caravan em segundos (o jogo fecha nesse tempo na partida Caravan).
    pub caravan_segundos: i64,
    /// Quando o jogo foi cadastrado (RFC 3339, do Firestore). Só na visão do admin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Última alteração do jogo, inclusive no RobotX (RFC 3339, do Firestore).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Texto opcional aparado: vazio vira `None`; acima do limite é erro.
fn texto_opcional(valor: Option<String>, campo: &str) -> Result<Option<String>, AppError> {
    let Some(valor) = valor.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    if valor.chars().count() > TEXTO_MAXIMO {
        return Err(AppError::bad_request(format!("{campo} deve ter no máximo {TEXTO_MAXIMO} caracteres")));
    }
    Ok(Some(valor))
}

/// Valida e normaliza a entrada (textos aparados, plataforma e modos conferidos).
fn validar(id: &str, entrada: JogoEntrada) -> Result<JogoEntrada, AppError> {
    if !id_valido(id) {
        return Err(AppError::bad_request("id do jogo inválido (use letras, números, - e _)"));
    }
    let title = entrada.title.trim().to_string();
    if title.is_empty() || title.chars().count() > TITULO_MAXIMO {
        return Err(AppError::bad_request("título deve ter de 1 a 80 caracteres"));
    }
    if !(0..=i64::from(i32::MAX)).contains(&entrada.steam_app_id) {
        return Err(AppError::bad_request("steam_app_id inválido"));
    }
    if !(1..=DURACAO_MAXIMA_SEGUNDOS).contains(&entrada.max_duration_segundos) {
        return Err(AppError::bad_request("max_duration_segundos deve ficar entre 1 e 7200"));
    }
    if let Some(caravan) = entrada.caravan_segundos {
        if !(CARAVAN_MINIMO_SEGUNDOS..=CARAVAN_MAXIMO_SEGUNDOS).contains(&caravan) {
            return Err(AppError::bad_request("caravan_segundos deve ficar entre 30 e 3600"));
        }
        if caravan >= entrada.max_duration_segundos {
            return Err(AppError::bad_request("o tempo do Caravan deve ser menor que a duração máxima da run"));
        }
    }
    if entrada.year.is_some_and(|ano| !(ANO_MINIMO..=ANO_MAXIMO).contains(&ano)) {
        return Err(AppError::bad_request("year deve ficar entre 1970 e 2100"));
    }
    let platform = texto_opcional(entrada.platform, "platform")?;
    if platform.as_deref().is_some_and(|p| !PLATAFORMAS.contains(&p)) {
        return Err(AppError::bad_request("platform desconhecida"));
    }
    let mut platforms: Vec<String> = Vec::new();
    for onde in entrada.platforms {
        if !ONDE_JOGAR.contains(&onde.as_str()) {
            return Err(AppError::bad_request("plataforma desconhecida (use steam, emulator ou mister)"));
        }
        if !platforms.contains(&onde) {
            platforms.push(onde);
        }
    }
    let mut modes: Vec<String> = Vec::new();
    for modo in entrada.modes {
        if !MODOS.contains(&modo.as_str()) {
            return Err(AppError::bad_request("modo desconhecido (use highscore, 1cc ou caravan)"));
        }
        if !modes.contains(&modo) {
            modes.push(modo);
        }
    }
    if entrada.ranked_mister && !platforms.iter().any(|p| p == "mister") {
        return Err(AppError::bad_request("Ranked no MiSTer exige MiSTer nas plataformas do jogo"));
    }
    if entrada.no_steam_protection && (!platforms.iter().any(|p| p == "steam") || entrada.steam_app_id == 0) {
        return Err(AppError::bad_request("No Steam Protection exige Steam nas plataformas e o app_id da Steam"));
    }
    let cover_url = entrada.cover_url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty());
    if let Some(url) = &cover_url {
        let nome = url.strip_prefix(PREFIXO_CAPA).unwrap_or("");
        let nome_ok = !nome.is_empty() && nome.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) && !nome.contains("..");
        if url.len() > URL_MAXIMA || !nome_ok {
            return Err(AppError::bad_request(format!("cover_url deve ser uma imagem em {PREFIXO_CAPA}")));
        }
    }
    Ok(JogoEntrada {
        title,
        cover_url,
        developer: texto_opcional(entrada.developer, "developer")?,
        publisher: texto_opcional(entrada.publisher, "publisher")?,
        genre: texto_opcional(entrada.genre, "genre")?,
        platform,
        platforms,
        modes,
        ..entrada
    })
}

/// Campos do catálogo que a gravação controla. Os opcionais ausentes na entrada estão na máscara
/// mas não nos valores, o que os apaga do documento.
const MASCARA: [&str; 16] = [
    "title",
    "steamAppId",
    "maxDurationSeconds",
    "published",
    "updatedAt",
    "developer",
    "publisher",
    "genre",
    "year",
    "platform",
    "platforms",
    "modes",
    "coverUrl",
    "rankedMister",
    "noSteamProtection",
    "caravanSeconds",
];

/// Cria ou atualiza `games/{id}`. Só toca os campos do catálogo, preservando o que outras
/// partes (ex.: dados do validador, no futuro) gravarem no mesmo documento.
pub async fn salvar(fs: &Firestore, id: &str, entrada: JogoEntrada) -> Result<Jogo, AppError> {
    gravar(fs, id, entrada, false).await
}

/// Cadastra um jogo novo. Recusa (`409`) um ID que já existe, em vez de sobrescrever o jogo.
pub async fn criar(fs: &Firestore, id: &str, entrada: JogoEntrada) -> Result<Jogo, AppError> {
    if let Some(existente) = obter(fs, id).await? {
        return Err(id_em_uso(id, &existente.title));
    }
    gravar(fs, id, entrada, true).await
}

fn id_em_uso(id: &str, titulo: &str) -> AppError {
    AppError::conflict(format!("já existe um jogo com o ID {id} ({titulo})"))
}

/// Remove `games/{id}` (com o RobotX, que fica no mesmo documento). Runs e rankings já gravados
/// com esse ID continuam onde estão. Devolve o jogo como estava; `404` se não existe.
pub async fn excluir(fs: &Firestore, id: &str) -> Result<Jogo, AppError> {
    let jogo = obter(fs, id).await?.ok_or_else(|| AppError::not_found(format!("jogo {id} não existe")))?;
    fs.apagar(&format!("games/{id}")).await?;
    Ok(jogo)
}

async fn gravar(fs: &Firestore, id: &str, entrada: JogoEntrada, novo: bool) -> Result<Jogo, AppError> {
    let entrada = validar(id, entrada)?;
    let mut campos = Campos::new();
    let mut inserir = |nome: &str, valor| {
        campos.insert(nome.to_string(), valor);
    };
    inserir("title", texto(&entrada.title));
    inserir("steamAppId", inteiro(entrada.steam_app_id));
    inserir("maxDurationSeconds", inteiro(entrada.max_duration_segundos));
    inserir("published", booleano(entrada.published));
    inserir("rankedMister", booleano(entrada.ranked_mister));
    inserir("noSteamProtection", booleano(entrada.no_steam_protection));
    inserir("updatedAt", data(Utc::now()));
    for (nome, valor) in [
        ("developer", &entrada.developer),
        ("publisher", &entrada.publisher),
        ("genre", &entrada.genre),
        ("platform", &entrada.platform),
        ("coverUrl", &entrada.cover_url),
    ] {
        if let Some(valor) = valor {
            inserir(nome, texto(valor));
        }
    }
    if let Some(ano) = entrada.year {
        inserir("year", inteiro(ano));
    }
    if let Some(caravan) = entrada.caravan_segundos {
        inserir("caravanSeconds", inteiro(caravan));
    }
    if !entrada.modes.is_empty() {
        inserir("modes", lista_de_textos(&entrada.modes));
    }
    if !entrada.platforms.is_empty() {
        inserir("platforms", lista_de_textos(&entrada.platforms));
    }
    let caminho = format!("games/{id}");
    if novo {
        // A checagem em `criar` dá a mensagem; esta pega quem criou o mesmo ID entre ela e aqui.
        fs.gravar_novo(&caminho, campos).await.map_err(|e| match e {
            AppError::Conflict(_) => id_em_uso(id, &entrada.title),
            outro => outro,
        })?;
    } else {
        fs.gravar(&caminho, campos, &MASCARA).await?;
    }
    // Relê só as datas, que quem define é o Firestore.
    let datas = fs.ler_documento_com_datas(&caminho).await?;
    Ok(Jogo {
        created_at: datas.as_ref().and_then(|d| d.criado_em.clone()),
        updated_at: datas.and_then(|d| d.atualizado_em),
        id: id.to_string(),
        title: entrada.title,
        steam_app_id: entrada.steam_app_id,
        max_duration_segundos: entrada.max_duration_segundos,
        published: entrada.published,
        developer: entrada.developer,
        publisher: entrada.publisher,
        genre: entrada.genre,
        year: entrada.year,
        platform: entrada.platform,
        platforms: entrada.platforms,
        modes: entrada.modes,
        cover_url: entrada.cover_url,
        ranked_mister: entrada.ranked_mister,
        no_steam_protection: entrada.no_steam_protection,
        caravan_segundos: entrada.caravan_segundos.unwrap_or(CARAVAN_PADRAO_SEGUNDOS),
    })
}

/// Jogo do catálogo, publicado ou não; `None` se ainda não existe.
pub async fn obter(fs: &Firestore, id: &str) -> Result<Option<Jogo>, AppError> {
    if !id_valido(id) {
        return Ok(None);
    }
    Ok(fs.ler_documento_com_datas(&format!("games/{id}")).await?.map(jogo_com_datas))
}

const NOMES_MODOS: [(&str, &str); 3] = [("highscore", "pontuação"), ("1cc", "1 crédito"), ("caravan", "caravana")];

fn nomes_dos_modos(modos: &[String]) -> String {
    let nomes: Vec<&str> =
        modos.iter().map(|m| NOMES_MODOS.iter().find(|(id, _)| id == m).map_or(m.as_str(), |(_, nome)| nome)).collect();
    if nomes.is_empty() { "nenhum".into() } else { nomes.join(", ") }
}

type DadosDoJogo<'a> =
    (&'a str, i64, Option<&'a str>, Option<&'a str>, Option<&'a str>, Option<i64>, Option<&'a str>, Option<&'a str>);

/// Informações exibidas do jogo, fora regras (duração e modos).
fn dados_do_jogo(j: &Jogo) -> DadosDoJogo<'_> {
    (&j.title, j.steam_app_id, j.developer.as_deref(), j.publisher.as_deref(), j.genre.as_deref(), j.year, j.platform.as_deref(), j.cover_url.as_deref())
}

/// O que uma gravação do jogo muda para os jogadores, em itens para o rascunho da próxima
/// versão. Jogo que não está (nem estava) publicado não aparece para ninguém: não gera nada.
/// Cada item tem chave estável por jogo e assunto, então gravar de novo só atualiza o texto.
pub fn mudancas_do_jogo(antes: Option<&Jogo>, depois: &Jogo) -> Vec<Mudanca> {
    let estava = antes.is_some_and(|a| a.published);
    let item = |assunto: &str, section: &str, text: Option<String>| Mudanca {
        key: format!("game:{}:{assunto}", depois.id),
        section: section.into(),
        text,
    };
    match (estava, depois.published) {
        (false, false) => vec![],
        (false, true) => vec![item("sala", "features", Some(format!("Nova sala: {}", depois.title)))],
        // Mudanças de catálogo valem na hora, então a retirada também é notícia. Mesma chave da
        // publicação: publicar e retirar antes da versão sair deixa só a linha final.
        (true, false) => vec![item("sala", "fixes", Some(format!("{} saiu do catálogo", depois.title)))],
        (true, true) => {
            let antes = antes.expect("estava publicado");
            let mut mudancas = Vec::new();
            if antes.max_duration_segundos != depois.max_duration_segundos {
                let minutos = (depois.max_duration_segundos + 59) / 60;
                mudancas.push(item("duracao", "rules", Some(format!("{}: duração máxima da run agora é de {minutos} min", depois.title))));
            }
            if antes.modes != depois.modes {
                mudancas.push(item("modos", "rules", Some(format!("{}: modos competitivos agora são {}", depois.title, nomes_dos_modos(&depois.modes)))));
            }
            if antes.caravan_segundos != depois.caravan_segundos && depois.modes.iter().any(|m| m == "caravan") {
                mudancas.push(item("caravan", "rules", Some(format!("{}: o Caravan agora tem {}", depois.title, duracao_legivel(depois.caravan_segundos)))));
            }
            if antes.ranked_mister != depois.ranked_mister {
                let texto = if depois.ranked_mister { "Ranked no MiSTer liberado" } else { "no MiSTer agora só Training" };
                mudancas.push(item("ranked-mister", "rules", Some(format!("{}: {texto}", depois.title))));
            }
            if antes.no_steam_protection != depois.no_steam_protection {
                let texto = if depois.no_steam_protection { "abre direto, sem precisar abrir a Steam" } else { "volta a abrir pela Steam" };
                mudancas.push(item("sem-steam", "fixes", Some(format!("{}: {texto}", depois.title))));
            }
            if dados_do_jogo(antes) != dados_do_jogo(depois) || antes.platforms != depois.platforms {
                mudancas.push(item("dados", "fixes", Some(format!("{}: informações do jogo atualizadas", depois.title))));
            }
            mudancas
        }
    }
}

/// Duração para as notas da versão: `5 min`, `2 min 30 s`, `45 s`.
fn duracao_legivel(segundos: i64) -> String {
    match (segundos / 60, segundos % 60) {
        (0, s) => format!("{s} s"),
        (m, 0) => format!("{m} min"),
        (m, s) => format!("{m} min {s} s"),
    }
}

/// Todos os jogos, publicados ou não (visão do admin), ordenados por id.
pub async fn listar(fs: &Firestore) -> Result<Vec<Jogo>, AppError> {
    let mut jogos: Vec<Jogo> = fs.listar_com_datas("games").await?.into_iter().map(jogo_com_datas).collect();
    jogos.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(jogos)
}

fn jogo_com_datas(documento: DocumentoComDatas) -> Jogo {
    Jogo {
        created_at: documento.criado_em,
        updated_at: documento.atualizado_em,
        ..jogo_de(documento.id, &documento.campos)
    }
}

fn jogo_de(id: String, campos: &Campos) -> Jogo {
    Jogo {
        title: ler_texto(campos, "title").unwrap_or_else(|| id.clone()),
        steam_app_id: ler_inteiro(campos, "steamAppId").unwrap_or(0),
        max_duration_segundos: ler_inteiro(campos, "maxDurationSeconds").unwrap_or(DURACAO_PADRAO_SEGUNDOS),
        published: ler_booleano(campos, "published").unwrap_or(false),
        developer: ler_texto(campos, "developer"),
        publisher: ler_texto(campos, "publisher"),
        genre: ler_texto(campos, "genre"),
        year: ler_inteiro(campos, "year"),
        platform: ler_texto(campos, "platform"),
        platforms: ler_lista_de_textos(campos, "platforms"),
        modes: ler_lista_de_textos(campos, "modes"),
        cover_url: ler_texto(campos, "coverUrl"),
        ranked_mister: ler_booleano(campos, "rankedMister").unwrap_or(false),
        no_steam_protection: ler_booleano(campos, "noSteamProtection").unwrap_or(false),
        caravan_segundos: ler_inteiro(campos, "caravanSeconds").unwrap_or(CARAVAN_PADRAO_SEGUNDOS),
        created_at: None,
        updated_at: None,
        id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::gameplays;
    use uuid::Uuid;

    fn entrada() -> JogoEntrada {
        JogoEntrada {
            title: "  Earthion ".into(),
            steam_app_id: 1234,
            max_duration_segundos: 1800,
            published: true,
            ..Default::default()
        }
    }

    #[test]
    fn entrada_valida_e_normalizada() {
        let ok = validar("earthion", entrada()).unwrap();
        assert_eq!(ok.title, "Earthion");
    }

    #[test]
    fn tempo_do_caravan_entre_30_s_e_1_h_e_menor_que_a_run() {
        let com = |s: i64| validar("earthion", JogoEntrada { caravan_segundos: Some(s), ..entrada() });
        assert_eq!(com(300).unwrap().caravan_segundos, Some(300));
        assert!(com(29).is_err());
        assert!(com(3601).is_err());
        // A run de 30 min (1800 s) precisa caber o Caravan com folga.
        assert!(com(1800).is_err());
        assert_eq!(validar("earthion", entrada()).unwrap().caravan_segundos, None);
        assert_eq!(duracao_legivel(300), "5 min");
        assert_eq!(duracao_legivel(150), "2 min 30 s");
        assert_eq!(duracao_legivel(45), "45 s");
    }

    #[test]
    fn capa_so_do_bucket_do_site() {
        let com = |u: &str| validar("earthion", JogoEntrada { cover_url: Some(u.into()), ..entrada() });
        assert_eq!(
            com("https://www.shmupx.com/catalog/covers/earthion-1.webp").unwrap().cover_url.as_deref(),
            Some("https://www.shmupx.com/catalog/covers/earthion-1.webp")
        );
        assert_eq!(com("  ").unwrap().cover_url, None);
        assert!(com("https://evil.example/x.webp").is_err());
        assert!(com("https://www.shmupx.com/catalog/covers/../x.webp").is_err());
        assert!(com("https://www.shmupx.com/catalog/covers/").is_err());
    }

    #[test]
    fn entradas_invalidas_sao_recusadas() {
        assert!(validar("../wallets/x", entrada()).is_err());
        assert!(validar("", entrada()).is_err());
        assert!(validar("earthion", JogoEntrada { title: "   ".into(), ..entrada() }).is_err());
        assert!(validar("earthion", JogoEntrada { title: "x".repeat(81), ..entrada() }).is_err());
        assert!(validar("earthion", JogoEntrada { steam_app_id: -1, ..entrada() }).is_err());
        assert!(validar("earthion", JogoEntrada { max_duration_segundos: 0, ..entrada() }).is_err());
        assert!(validar("earthion", JogoEntrada { max_duration_segundos: 7201, ..entrada() }).is_err());
    }

    #[test]
    fn metadados_sao_normalizados() {
        let ok = validar(
            "earthion",
            JogoEntrada {
                developer: Some("  Ratalaika  ".into()),
                publisher: Some("   ".into()),
                genre: Some("Shmup".into()),
                year: Some(2021),
                platform: Some("steam".into()),
                modes: vec!["highscore".into(), "1cc".into(), "highscore".into()],
                ..entrada()
            },
        )
        .unwrap();
        assert_eq!(ok.developer.as_deref(), Some("Ratalaika"));
        assert_eq!(ok.publisher, None);
        assert_eq!(ok.modes, vec!["highscore", "1cc"]);
        let varias = validar("g", JogoEntrada { platforms: vec!["mister".into(), "steam".into(), "mister".into()], ..entrada() }).unwrap();
        assert_eq!(varias.platforms, vec!["mister", "steam"]);
    }

    #[test]
    fn metadados_invalidos_sao_recusados() {
        assert!(validar("g", JogoEntrada { year: Some(1969), ..entrada() }).is_err());
        assert!(validar("g", JogoEntrada { year: Some(2101), ..entrada() }).is_err());
        assert!(validar("g", JogoEntrada { platform: Some("atari".into()), ..entrada() }).is_err());
        assert!(validar("g", JogoEntrada { modes: vec!["speedrun".into()], ..entrada() }).is_err());
        assert!(validar("g", JogoEntrada { platforms: vec!["xbox360".into()], ..entrada() }).is_err());
        assert!(validar("g", JogoEntrada { developer: Some("x".repeat(81)), ..entrada() }).is_err());
    }

    #[test]
    fn sem_protecao_da_steam_exige_steam_e_app_id() {
        let steam = || vec!["steam".to_string()];
        let ok = validar("g", JogoEntrada { platforms: steam(), no_steam_protection: true, ..entrada() }).unwrap();
        assert!(ok.no_steam_protection);
        assert!(validar("g", JogoEntrada { platforms: vec!["emulator".into()], no_steam_protection: true, ..entrada() }).is_err());
        assert!(validar("g", JogoEntrada { platforms: steam(), steam_app_id: 0, no_steam_protection: true, ..entrada() }).is_err());
        let corpo = r#"{"title":"A","steam_app_id":1,"max_duration_segundos":60,"published":true,"platforms":["steam"],"no_steam_protection":true}"#;
        assert!(serde_json::from_str::<JogoEntrada>(corpo).unwrap().no_steam_protection);
    }

    #[test]
    fn corpo_com_campo_desconhecido_e_recusado() {
        let corpo = r#"{"title":"A","steam_app_id":1,"max_duration_segundos":60,"published":true,"id":"x"}"#;
        assert!(serde_json::from_str::<JogoEntrada>(corpo).is_err());
    }

    #[test]
    fn corpo_sem_metadados_continua_valido() {
        let corpo = r#"{"title":"A","steam_app_id":1,"max_duration_segundos":60,"published":true}"#;
        let entrada: JogoEntrada = serde_json::from_str(corpo).unwrap();
        assert!(entrada.modes.is_empty() && entrada.developer.is_none());
    }

    fn jogo(published: bool) -> Jogo {
        Jogo { id: "earthion".into(), title: "Earthion".into(), max_duration_segundos: 1800, published, ..Default::default() }
    }

    #[test]
    fn mudancas_do_jogo_para_o_rascunho() {
        // Rascunho de jogo nunca publicado: invisível aos jogadores.
        assert!(mudancas_do_jogo(None, &jogo(false)).is_empty());

        let nova = mudancas_do_jogo(None, &jogo(true));
        assert_eq!(nova.len(), 1);
        assert_eq!((nova[0].key.as_str(), nova[0].section.as_str()), ("game:earthion:sala", "features"));
        assert_eq!(nova[0].text.as_deref(), Some("Nova sala: Earthion"));

        let saiu = mudancas_do_jogo(Some(&jogo(true)), &jogo(false));
        assert_eq!(saiu[0].key, "game:earthion:sala", "mesma chave da publicação");

        // Sem diferença visível: nada.
        assert!(mudancas_do_jogo(Some(&jogo(true)), &jogo(true)).is_empty());

        let regras = Jogo { max_duration_segundos: 3600, modes: vec!["1cc".into()], ..jogo(true) };
        let itens = mudancas_do_jogo(Some(&jogo(true)), &regras);
        let chaves: Vec<&str> = itens.iter().map(|m| m.key.as_str()).collect();
        assert_eq!(chaves, vec!["game:earthion:duracao", "game:earthion:modos"]);
        assert!(itens.iter().all(|m| m.section == "rules"));
        assert_eq!(itens[1].text.as_deref(), Some("Earthion: modos competitivos agora são 1 crédito"));

        let dados = Jogo { developer: Some("Yuzo Koshiro".into()), ..jogo(true) };
        assert_eq!(mudancas_do_jogo(Some(&jogo(true)), &dados)[0].section, "fixes");
    }

    #[test]
    fn documento_incompleto_ganha_padroes_seguros() {
        let jogo = jogo_de("velho".into(), &Campos::new());
        assert_eq!(
            jogo,
            Jogo {
                id: "velho".into(),
                title: "velho".into(),
                steam_app_id: 0,
                max_duration_segundos: 3600,
                published: false,
                caravan_segundos: CARAVAN_PADRAO_SEGUNDOS,
                ..Default::default()
            }
        );
    }

    /// Contra o emulador (nunca produção): publicar, listar e usar o jogo numa run.
    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn catalogo_publica_lista_e_controla_runs_no_emulador() {
        assert!(std::env::var("FIRESTORE_EMULATOR_HOST").is_ok(), "este teste só roda no emulador");
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();

        let id = format!("jogo-{}", Uuid::new_v4());
        let uid = format!("uid-{}", Uuid::new_v4());

        // Rascunho: aparece na listagem do admin, mas não abre run.
        salvar(&fs, &id, JogoEntrada { published: false, ..entrada() }).await.unwrap();
        let listado = listar(&fs).await.unwrap().into_iter().find(|j| j.id == id).unwrap();
        assert_eq!((listado.title.as_str(), listado.published, listado.max_duration_segundos), ("Earthion", false, 1800));
        assert!(gameplays::iniciar(&fs, &uid, &id, &gameplays::Detalhes::default()).await.is_err());

        // Publicado: abre run com a duração do catálogo.
        salvar(&fs, &id, entrada()).await.unwrap();
        let run = gameplays::iniciar(&fs, &uid, &id, &gameplays::Detalhes::default()).await.unwrap();
        assert_eq!(run.max_duration_segundos, 1800);

        // Despublicar fecha novas runs de novo.
        salvar(&fs, &id, JogoEntrada { published: false, ..entrada() }).await.unwrap();
        assert!(gameplays::iniciar(&fs, &uid, &id, &gameplays::Detalhes::default()).await.is_err());
    }

    /// Criar recusa ID repetido; as datas vêm do Firestore; excluir tira o jogo da listagem.
    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn criar_recusa_id_repetido_e_excluir_remove_no_emulador() {
        assert!(std::env::var("FIRESTORE_EMULATOR_HOST").is_ok(), "este teste só roda no emulador");
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let id = format!("jogo-{}", Uuid::new_v4());

        let criado = criar(&fs, &id, entrada()).await.unwrap();
        assert!(criado.created_at.is_some() && criado.updated_at.is_some());
        let repetido = criar(&fs, &id, JogoEntrada { title: "Outro".into(), ..entrada() }).await;
        assert!(matches!(repetido, Err(AppError::Conflict(_))));
        assert_eq!(obter(&fs, &id).await.unwrap().unwrap().title, "Earthion", "o original fica intacto");

        // A precondição do Firestore também barra a criação por cima, sem a checagem prévia.
        assert!(matches!(gravar(&fs, &id, entrada(), true).await, Err(AppError::Conflict(_))));

        let listado = listar(&fs).await.unwrap().into_iter().find(|j| j.id == id).unwrap();
        assert_eq!(listado.created_at, criado.created_at);

        let salvo = salvar(&fs, &id, JogoEntrada { title: "Earthion 2".into(), ..entrada() }).await.unwrap();
        assert_eq!(salvo.created_at, criado.created_at, "editar não muda a data de criação");
        assert!(salvo.updated_at >= criado.updated_at);

        assert_eq!(excluir(&fs, &id).await.unwrap().title, "Earthion 2");
        assert!(obter(&fs, &id).await.unwrap().is_none());
        assert!(matches!(excluir(&fs, &id).await, Err(AppError::NotFound(_))));
    }

    /// Metadados gravados voltam na listagem e são apagados quando saem da entrada.
    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn metadados_persistem_e_sao_apagados_no_emulador() {
        assert!(std::env::var("FIRESTORE_EMULATOR_HOST").is_ok(), "este teste só roda no emulador");
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let id = format!("jogo-{}", Uuid::new_v4());

        let completo = JogoEntrada {
            developer: Some("Ratalaika".into()),
            genre: Some("Shmup".into()),
            year: Some(2021),
            platform: Some("steam".into()),
            modes: vec!["highscore".into(), "caravan".into()],
            ..entrada()
        };
        salvar(&fs, &id, completo).await.unwrap();
        let lido = listar(&fs).await.unwrap().into_iter().find(|j| j.id == id).unwrap();
        assert_eq!(lido.developer.as_deref(), Some("Ratalaika"));
        assert_eq!((lido.year, lido.platform.as_deref()), (Some(2021), Some("steam")));
        assert_eq!(lido.modes, vec!["highscore", "caravan"]);

        // Regravar sem metadados os apaga, sem tocar nos campos do catálogo.
        salvar(&fs, &id, entrada()).await.unwrap();
        let lido = listar(&fs).await.unwrap().into_iter().find(|j| j.id == id).unwrap();
        assert!(lido.developer.is_none() && lido.year.is_none() && lido.modes.is_empty());
        assert_eq!(lido.title, "Earthion");
    }
}
