//! Eventos e torneios: `events/{eventId}` no Firestore. Quem escreve é o admin, pelo Lab
//! (`/api/admin/events`); o Client lê só os `published` direto do Firestore (Security Rules).
//! Um evento tem datas, os jogos que valem, o modo, as regras e a premiação.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::firestore::valor::{booleano, data, lista_de_textos, ler_booleano, ler_data, ler_lista_de_textos, ler_texto, texto};
use crate::firestore::{Campos, DocumentoComDatas, Firestore};
use crate::services::gameplays::id_valido;

const TITULO_MAXIMO: usize = 80;
const DESCRICAO_MAXIMA: usize = 600;
const JOGOS_MAXIMOS: usize = 10;
const ITENS_MAXIMOS: usize = 20;
const ITEM_MAXIMO: usize = 200;
/// Modo que vale no evento (as tabelas do ranking).
const MODOS: [&str; 3] = ["highscore", "1cc", "caravan"];
const CAMPOS: [&str; 9] = ["title", "description", "gameIds", "mode", "startsAt", "endsAt", "rules", "prizes", "published"];

/// Campos que o Lab define. O `eventId` vem da rota.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EventoEntrada {
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Jogos do catálogo que valem no evento.
    pub game_ids: Vec<String>,
    pub mode: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    #[serde(default)]
    pub rules: Vec<String>,
    /// Premiação, uma linha por colocação ou prêmio.
    #[serde(default)]
    pub prizes: Vec<String>,
    /// Ligado, o evento aparece no Client.
    #[serde(default)]
    pub published: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Evento {
    pub id: String,
    pub title: String,
    pub description: String,
    pub game_ids: Vec<String>,
    pub mode: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub rules: Vec<String>,
    pub prizes: Vec<String>,
    pub published: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

fn limpar(itens: &[String]) -> Vec<String> {
    itens.iter().map(|i| i.trim().to_string()).filter(|i| !i.is_empty()).collect()
}

fn validar(id: &str, e: &mut EventoEntrada) -> Result<(), AppError> {
    if !id_valido(id) {
        return Err(AppError::bad_request("ID do evento inválido: use letras minúsculas, números e hífen"));
    }
    e.title = e.title.trim().to_string();
    e.description = e.description.trim().to_string();
    e.rules = limpar(&e.rules);
    e.prizes = limpar(&e.prizes);
    e.game_ids = limpar(&e.game_ids);
    e.game_ids.dedup();
    if e.title.is_empty() || e.title.chars().count() > TITULO_MAXIMO {
        return Err(AppError::bad_request(format!("o título precisa ter de 1 a {TITULO_MAXIMO} caracteres")));
    }
    if e.description.chars().count() > DESCRICAO_MAXIMA {
        return Err(AppError::bad_request(format!("a descrição passa de {DESCRICAO_MAXIMA} caracteres")));
    }
    if e.game_ids.is_empty() || e.game_ids.len() > JOGOS_MAXIMOS || e.game_ids.iter().any(|g| !id_valido(g)) {
        return Err(AppError::bad_request(format!("escolha de 1 a {JOGOS_MAXIMOS} jogos do catálogo")));
    }
    if !MODOS.contains(&e.mode.as_str()) {
        return Err(AppError::bad_request("modo inválido: use highscore, 1cc ou caravan"));
    }
    if e.ends_at <= e.starts_at {
        return Err(AppError::bad_request("o fim do evento precisa ser depois do começo"));
    }
    for (nome, itens) in [("regras", &e.rules), ("prêmios", &e.prizes)] {
        if itens.len() > ITENS_MAXIMOS || itens.iter().any(|i| i.chars().count() > ITEM_MAXIMO) {
            return Err(AppError::bad_request(format!("até {ITENS_MAXIMOS} {nome}, cada um com até {ITEM_MAXIMO} caracteres")));
        }
    }
    Ok(())
}

fn campos(e: &EventoEntrada) -> Campos {
    [
        ("title", texto(&e.title)),
        ("description", texto(&e.description)),
        ("gameIds", lista_de_textos(&e.game_ids)),
        ("mode", texto(&e.mode)),
        ("startsAt", data(e.starts_at)),
        ("endsAt", data(e.ends_at)),
        ("rules", lista_de_textos(&e.rules)),
        ("prizes", lista_de_textos(&e.prizes)),
        ("published", booleano(e.published)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

fn evento(d: DocumentoComDatas) -> Evento {
    let c = &d.campos;
    Evento {
        title: ler_texto(c, "title").unwrap_or_default(),
        description: ler_texto(c, "description").unwrap_or_default(),
        game_ids: ler_lista_de_textos(c, "gameIds"),
        mode: ler_texto(c, "mode").unwrap_or_else(|| "highscore".into()),
        starts_at: ler_data(c, "startsAt"),
        ends_at: ler_data(c, "endsAt"),
        rules: ler_lista_de_textos(c, "rules"),
        prizes: ler_lista_de_textos(c, "prizes"),
        published: ler_booleano(c, "published").unwrap_or(false),
        created_at: d.criado_em,
        updated_at: d.atualizado_em,
        id: d.id,
    }
}

/// Todos os eventos, do que começa por último ao mais antigo.
pub async fn listar(fs: &Firestore) -> Result<Vec<Evento>, AppError> {
    let mut eventos: Vec<Evento> = fs.listar_com_datas("events").await?.into_iter().map(evento).collect();
    eventos.sort_by(|a, b| b.starts_at.cmp(&a.starts_at).then_with(|| a.title.cmp(&b.title)));
    Ok(eventos)
}

/// Cria ou atualiza o evento. Os jogos precisam existir no catálogo.
pub async fn salvar(fs: &Firestore, id: &str, mut entrada: EventoEntrada) -> Result<Evento, AppError> {
    validar(id, &mut entrada)?;
    for jogo in &entrada.game_ids {
        if fs.ler_documento(&format!("games/{jogo}")).await?.is_none() {
            return Err(AppError::bad_request(format!("o jogo {jogo} não está no catálogo")));
        }
    }
    let caminho = format!("events/{id}");
    fs.gravar(&caminho, campos(&entrada), &CAMPOS).await?;
    fs.ler_documento_com_datas(&caminho).await?.map(evento).ok_or_else(|| AppError::internal("o evento não foi gravado"))
}

pub async fn excluir(fs: &Firestore, id: &str) -> Result<(), AppError> {
    if !id_valido(id) {
        return Err(AppError::not_found("evento não encontrado"));
    }
    fs.apagar(&format!("events/{id}")).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrada() -> EventoEntrada {
        EventoEntrada {
            title: " Copa Vertical ".into(),
            description: String::new(),
            game_ids: vec!["ketsui".into(), "ketsui".into()],
            mode: "highscore".into(),
            starts_at: "2026-12-01T20:00:00Z".parse().unwrap(),
            ends_at: "2026-12-31T23:59:00Z".parse().unwrap(),
            rules: vec!["Sem continue".into(), " ".into()],
            prizes: vec![],
            published: false,
        }
    }

    #[test]
    fn valida_e_limpa() {
        let mut e = entrada();
        assert!(validar("copa-vertical", &mut e).is_ok());
        assert_eq!(e.title, "Copa Vertical");
        assert_eq!(e.game_ids, vec!["ketsui"]);
        assert_eq!(e.rules, vec!["Sem continue"]);
    }

    #[test]
    fn recusa_datas_modo_e_jogos_invalidos() {
        let mut e = entrada();
        e.ends_at = e.starts_at;
        assert!(validar("copa", &mut e).is_err());
        assert!(validar("copa", &mut EventoEntrada { mode: "speedrun".into(), ..entrada() }).is_err());
        assert!(validar("copa", &mut EventoEntrada { game_ids: vec![], ..entrada() }).is_err());
    }
}
