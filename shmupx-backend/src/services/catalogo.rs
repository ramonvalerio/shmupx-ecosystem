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
use crate::firestore::{Campos, Firestore};
use crate::services::gameplays::id_valido;

const DURACAO_PADRAO_SEGUNDOS: i64 = 3600;
const DURACAO_MAXIMA_SEGUNDOS: i64 = 7200;
const TITULO_MAXIMO: usize = 80;
const TEXTO_MAXIMO: usize = 80;
const ANO_MINIMO: i64 = 1970;
const ANO_MAXIMO: i64 = 2100;

/// Plataformas do catálogo (as mesmas do Lab antigo). Qualquer outro valor é recusado.
const PLATAFORMAS: [&str; 8] =
    ["steam", "megadrive", "neogeo", "arcade", "xbox360", "playstation", "switch", "other"];
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
    #[serde(default)]
    pub modes: Vec<String>,
}

#[derive(Debug, Default, Serialize, PartialEq)]
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
    pub modes: Vec<String>,
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
    if entrada.year.is_some_and(|ano| !(ANO_MINIMO..=ANO_MAXIMO).contains(&ano)) {
        return Err(AppError::bad_request("year deve ficar entre 1970 e 2100"));
    }
    let platform = texto_opcional(entrada.platform, "platform")?;
    if platform.as_deref().is_some_and(|p| !PLATAFORMAS.contains(&p)) {
        return Err(AppError::bad_request("platform desconhecida"));
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
    Ok(JogoEntrada {
        title,
        developer: texto_opcional(entrada.developer, "developer")?,
        publisher: texto_opcional(entrada.publisher, "publisher")?,
        genre: texto_opcional(entrada.genre, "genre")?,
        platform,
        modes,
        ..entrada
    })
}

/// Campos do catálogo que a gravação controla. Os opcionais ausentes na entrada estão na máscara
/// mas não nos valores, o que os apaga do documento.
const MASCARA: [&str; 11] = [
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
    "modes",
];

/// Cria ou atualiza `games/{id}`. Só toca os campos do catálogo, preservando o que outras
/// partes (ex.: dados do validador, no futuro) gravarem no mesmo documento.
pub async fn salvar(fs: &Firestore, id: &str, entrada: JogoEntrada) -> Result<Jogo, AppError> {
    let entrada = validar(id, entrada)?;
    let mut campos = Campos::new();
    let mut inserir = |nome: &str, valor| {
        campos.insert(nome.to_string(), valor);
    };
    inserir("title", texto(&entrada.title));
    inserir("steamAppId", inteiro(entrada.steam_app_id));
    inserir("maxDurationSeconds", inteiro(entrada.max_duration_segundos));
    inserir("published", booleano(entrada.published));
    inserir("updatedAt", data(Utc::now()));
    for (nome, valor) in [
        ("developer", &entrada.developer),
        ("publisher", &entrada.publisher),
        ("genre", &entrada.genre),
        ("platform", &entrada.platform),
    ] {
        if let Some(valor) = valor {
            inserir(nome, texto(valor));
        }
    }
    if let Some(ano) = entrada.year {
        inserir("year", inteiro(ano));
    }
    if !entrada.modes.is_empty() {
        inserir("modes", lista_de_textos(&entrada.modes));
    }
    fs.gravar(&format!("games/{id}"), campos, &MASCARA).await?;
    Ok(Jogo {
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
        modes: entrada.modes,
    })
}

/// Todos os jogos, publicados ou não (visão do admin), ordenados por id.
pub async fn listar(fs: &Firestore) -> Result<Vec<Jogo>, AppError> {
    let mut jogos: Vec<Jogo> = fs
        .listar("games")
        .await?
        .into_iter()
        .map(|(id, campos)| jogo_de(id, &campos))
        .collect();
    jogos.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(jogos)
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
        modes: ler_lista_de_textos(campos, "modes"),
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
    }

    #[test]
    fn metadados_invalidos_sao_recusados() {
        assert!(validar("g", JogoEntrada { year: Some(1969), ..entrada() }).is_err());
        assert!(validar("g", JogoEntrada { year: Some(2101), ..entrada() }).is_err());
        assert!(validar("g", JogoEntrada { platform: Some("atari".into()), ..entrada() }).is_err());
        assert!(validar("g", JogoEntrada { modes: vec!["speedrun".into()], ..entrada() }).is_err());
        assert!(validar("g", JogoEntrada { developer: Some("x".repeat(81)), ..entrada() }).is_err());
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
