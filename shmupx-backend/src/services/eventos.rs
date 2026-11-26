//! Eventos e torneios: `events/{eventId}` no Firestore. Quem escreve é o admin, pelo Lab
//! (`/api/admin/events`). Um evento tem datas, os jogos que valem, o modo, as regras, a premiação,
//! o custo da inscrição em fichas, o banner (carrossel da sala no Client) e o link da página.
//!
//! Inscrição (`/api/events/{id}/join`): numa transação, debita as fichas do evento da carteira e
//! grava `events/{id}/participants/{uid}`. Inscrito, o jogador joga partidas valendo no evento
//! (`/api/runs/start` com `evento_id`), que gastam 1 ficha como o Ranked e levam o `eventId`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::firestore::valor::{
    booleano, data, inteiro, ler_booleano, ler_data, ler_inteiro, ler_lista_de_textos, ler_texto, lista_de_textos, texto,
};
use crate::firestore::{Campos, DocumentoComDatas, Firestore};
use crate::services::gameplays::{criar, debito_na_carteira, id_valido};

const TITULO_MAXIMO: usize = 80;
const DESCRICAO_MAXIMA: usize = 600;
const JOGOS_MAXIMOS: usize = 10;
const ITENS_MAXIMOS: usize = 20;
const ITEM_MAXIMO: usize = 200;
/// Modo que vale no evento (as tabelas do ranking).
const MODOS: [&str; 3] = ["highscore", "1cc", "caravan"];
const CAMPOS: [&str; 12] =
    ["title", "description", "gameIds", "mode", "startsAt", "endsAt", "rules", "prizes", "published", "entryFee", "bannerUrl", "pageUrl"];
/// Inscrição mais cara aceita, em fichas.
const CUSTO_MAXIMO: i64 = 1000;
/// Banners enviados pelo Lab ao site (`events/banners/` no bucket do site).
const PREFIXO_BANNER: &str = "https://www.shmupx.com/events/banners/";
const LINK_MAXIMO: usize = 300;

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
    /// Fichas para se inscrever (0 = grátis).
    #[serde(default)]
    pub entry_fee: i64,
    /// Banner do carrossel na sala do Client (enviado pelo Lab ao site); vazio = só o título.
    #[serde(default)]
    pub banner_url: String,
    /// Página com tudo sobre o evento (regulamento completo, transmissões…); vazio = sem link.
    #[serde(default)]
    pub page_url: String,
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
    pub entry_fee: i64,
    pub banner_url: String,
    pub page_url: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// O evento como o jogador vê no Client (carrossel e janela de inscrição), e se ele já se inscreveu.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EventoDoJogador {
    pub id: String,
    pub title: String,
    pub description: String,
    pub game_ids: Vec<String>,
    /// Nomes dos jogos do evento (na ordem de `game_ids`), para o Client mostrar sem o catálogo.
    pub game_titles: Vec<String>,
    pub mode: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub rules: Vec<String>,
    pub prizes: Vec<String>,
    pub entry_fee: i64,
    pub banner_url: String,
    pub page_url: String,
    pub joined: bool,
}

impl EventoDoJogador {
    fn de(e: Evento, joined: bool, titulos: &std::collections::HashMap<String, Campos>) -> Self {
        let game_titles = e.game_ids.iter().map(|g| titulos.get(&format!("games/{g}")).and_then(|c| ler_texto(c, "title")).unwrap_or_else(|| g.clone())).collect();
        Self {
            id: e.id,
            title: e.title,
            description: e.description,
            game_titles,
            game_ids: e.game_ids,
            mode: e.mode,
            starts_at: e.starts_at,
            ends_at: e.ends_at,
            rules: e.rules,
            prizes: e.prizes,
            entry_fee: e.entry_fee,
            banner_url: e.banner_url,
            page_url: e.page_url,
            joined,
        }
    }
}

/// Resultado da inscrição: o evento (já inscrito) e o saldo depois do débito.
#[derive(Debug, Serialize, PartialEq)]
pub struct Inscricao {
    pub event: EventoDoJogador,
    /// Saldo de fichas depois da inscrição; `None` quando nada foi cobrado (grátis ou já inscrito).
    pub balance: Option<i64>,
}

/// Link da página do evento: `https://`, sem espaços nem aspas.
fn link_valido(url: &str) -> bool {
    url.len() > "https://".len()
        && url.len() <= LINK_MAXIMO
        && url.starts_with("https://")
        && !url.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '<' | '>'))
}

/// Banner enviado pelo Lab: `events/banners/<arquivo>.webp` no site.
fn banner_valido(url: &str) -> bool {
    url.strip_prefix(PREFIXO_BANNER).is_some_and(|nome| {
        nome.ends_with(".webp")
            && nome.len() <= 120
            && !nome.contains("..")
            && nome.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    })
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
    if !(0..=CUSTO_MAXIMO).contains(&e.entry_fee) {
        return Err(AppError::bad_request(format!("o custo da inscrição vai de 0 a {CUSTO_MAXIMO} fichas")));
    }
    e.banner_url = e.banner_url.trim().to_string();
    e.page_url = e.page_url.trim().to_string();
    if !e.banner_url.is_empty() && !banner_valido(&e.banner_url) {
        return Err(AppError::bad_request("banner inválido: envie a imagem pelo Lab"));
    }
    if !e.page_url.is_empty() && !link_valido(&e.page_url) {
        return Err(AppError::bad_request("link da página inválido: use um endereço https://"));
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
        ("entryFee", inteiro(e.entry_fee)),
        ("bannerUrl", texto(&e.banner_url)),
        ("pageUrl", texto(&e.page_url)),
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
        entry_fee: ler_inteiro(c, "entryFee").unwrap_or(0).max(0),
        banner_url: ler_texto(c, "bannerUrl").unwrap_or_default(),
        page_url: ler_texto(c, "pageUrl").unwrap_or_default(),
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

fn inscricao(evento: &str, uid: &str) -> String {
    format!("events/{evento}/participants/{uid}")
}

/// Lê o evento publicado dentro de uma transação.
async fn publicado_na_transacao(fs: &Firestore, tx: &str, id: &str) -> Result<Evento, AppError> {
    let campos = fs.ler(&format!("events/{id}"), tx).await?.filter(|c| ler_booleano(c, "published") == Some(true));
    let campos = campos.ok_or_else(|| AppError::not_found("evento não encontrado"))?;
    Ok(evento(DocumentoComDatas { id: id.to_string(), campos, criado_em: None, atualizado_em: None }))
}

/// Eventos publicados que ainda não terminaram, do que começa primeiro ao último, com a inscrição
/// do jogador.
pub async fn listar_para_jogador(fs: &Firestore, uid: &str) -> Result<Vec<EventoDoJogador>, AppError> {
    let agora = Utc::now();
    let mut abertos: Vec<Evento> =
        listar(fs).await?.into_iter().filter(|e| e.published && e.ends_at.is_some_and(|fim| fim > agora)).collect();
    abertos.sort_by(|a, b| a.starts_at.cmp(&b.starts_at).then_with(|| a.title.cmp(&b.title)));
    let mut caminhos: Vec<String> = abertos.iter().map(|e| inscricao(&e.id, uid)).collect();
    caminhos.extend(abertos.iter().flat_map(|e| e.game_ids.iter().map(|g| format!("games/{g}"))));
    caminhos.sort();
    caminhos.dedup();
    let lidos = fs.ler_varios(&caminhos).await?;
    Ok(abertos
        .into_iter()
        .map(|e| {
            let joined = lidos.contains_key(&inscricao(&e.id, uid));
            EventoDoJogador::de(e, joined, &lidos)
        })
        .collect())
}

/// Inscreve o jogador no evento: debita as fichas da inscrição e grava a inscrição, numa transação.
/// Já inscrito: não cobra de novo. Evento encerrado ou não publicado: recusa.
pub async fn participar(fs: &Firestore, uid: &str, id: &str) -> Result<Inscricao, AppError> {
    if !id_valido(id) {
        return Err(AppError::not_found("evento não encontrado"));
    }
    let tx = fs.iniciar_transacao().await?;
    let resultado = participar_na_transacao(fs, &tx, uid, id).await;
    // Sem escrita (erro ou já inscrito), a transação é só liberada.
    if !matches!(resultado, Ok((_, true))) {
        fs.desfazer(&tx).await;
    }
    let (e, joined, saldo) = resultado.map(|(inscricao, _)| inscricao)?;
    let titulos = fs.ler_varios(&e.game_ids.iter().map(|g| format!("games/{g}")).collect::<Vec<_>>()).await?;
    Ok(Inscricao { event: EventoDoJogador::de(e, joined, &titulos), balance: saldo })
}

/// O evento, a inscrição (sempre `true`), o saldo cobrado e se a inscrição foi gravada nesta transação.
async fn participar_na_transacao(fs: &Firestore, tx: &str, uid: &str, id: &str) -> Result<((Evento, bool, Option<i64>), bool), AppError> {
    let e = publicado_na_transacao(fs, tx, id).await?;
    let agora = Utc::now();
    if e.ends_at.is_none_or(|fim| fim <= agora) {
        return Err(AppError::bad_request("este evento já terminou"));
    }
    if fs.ler(&inscricao(id, uid), tx).await?.is_some() {
        return Ok(((e, true, None), false));
    }
    let mut escritas = Vec::new();
    let mut saldo = None;
    if e.entry_fee > 0 {
        let carteira = fs.ler(&format!("wallets/{uid}"), tx).await?;
        let (debito, saldo_final) = debito_na_carteira(fs, uid, carteira.as_ref(), e.entry_fee, "event_entry", ("eventId", id), agora)
            .map_err(|_| AppError::bad_request(format!("fichas insuficientes: a inscrição custa {} fichas", e.entry_fee)))?;
        escritas.extend(debito);
        saldo = Some(saldo_final);
    }
    let mut inscrito = Campos::new();
    inscrito.insert("uid".into(), texto(uid));
    inscrito.insert("fee".into(), inteiro(e.entry_fee));
    inscrito.insert("joinedAt".into(), data(agora));
    escritas.push(criar(fs, &inscricao(id, uid), inscrito));
    fs.confirmar(tx, escritas).await?;
    Ok(((e, true, saldo), true))
}

/// O modo do evento aceita o modo da partida: Hi-Score e 1CC são partidas de Hi-Score (o 1CC sai da
/// run zerada sem continue); Caravan, as de caravana.
fn modo_do_evento_aceita(evento: &str, partida: Option<&str>) -> bool {
    let partida = partida.unwrap_or("highscore");
    match evento {
        "caravan" => partida.starts_with("caravan"),
        _ => partida == "highscore" || partida == "1cc",
    }
}

/// Partida valendo no evento (`/api/runs/start` com `evento_id`), dentro da transação da run:
/// evento publicado e em andamento, o jogo e o modo são do evento e o jogador se inscreveu.
pub(crate) async fn exigir_partida_de_evento(
    fs: &Firestore,
    tx: &str,
    uid: &str,
    id: &str,
    game_id: &str,
    modo: Option<&str>,
    agora: DateTime<Utc>,
) -> Result<(), AppError> {
    let e = publicado_na_transacao(fs, tx, id).await?;
    if e.starts_at.is_some_and(|inicio| agora < inicio) {
        return Err(AppError::bad_request("o evento ainda não começou"));
    }
    if e.ends_at.is_none_or(|fim| agora >= fim) {
        return Err(AppError::bad_request("este evento já terminou"));
    }
    if !e.game_ids.iter().any(|g| g == game_id) {
        return Err(AppError::bad_request("este jogo não vale neste evento"));
    }
    if !modo_do_evento_aceita(&e.mode, modo) {
        return Err(AppError::bad_request("este modo de jogo não vale neste evento"));
    }
    if fs.ler(&inscricao(id, uid), tx).await?.is_none() {
        return Err(AppError::forbidden("você não está inscrito neste evento"));
    }
    Ok(())
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
            entry_fee: 10,
            banner_url: String::new(),
            page_url: String::new(),
        }
    }

    #[test]
    fn custo_banner_e_link() {
        assert!(validar("copa", &mut EventoEntrada { entry_fee: -1, ..entrada() }).is_err());
        assert!(validar("copa", &mut EventoEntrada { entry_fee: 1001, ..entrada() }).is_err());
        assert!(validar("copa", &mut EventoEntrada { entry_fee: 0, ..entrada() }).is_ok());
        let banner = format!("{PREFIXO_BANNER}copa-vertical-1795.webp");
        assert!(validar("copa", &mut EventoEntrada { banner_url: banner, ..entrada() }).is_ok());
        assert!(validar("copa", &mut EventoEntrada { banner_url: "https://evil.example/x.webp".into(), ..entrada() }).is_err());
        assert!(validar("copa", &mut EventoEntrada { banner_url: format!("{PREFIXO_BANNER}../x.webp"), ..entrada() }).is_err());
        assert!(validar("copa", &mut EventoEntrada { page_url: "https://www.shmupx.com/eventos/copa".into(), ..entrada() }).is_ok());
        assert!(validar("copa", &mut EventoEntrada { page_url: "http://inseguro.com".into(), ..entrada() }).is_err());
        assert!(validar("copa", &mut EventoEntrada { page_url: "https://a b".into(), ..entrada() }).is_err());
    }

    #[test]
    fn modo_do_evento() {
        assert!(modo_do_evento_aceita("highscore", Some("highscore")));
        assert!(modo_do_evento_aceita("1cc", Some("highscore")));
        assert!(modo_do_evento_aceita("highscore", None));
        assert!(!modo_do_evento_aceita("highscore", Some("caravan-classic")));
        assert!(modo_do_evento_aceita("caravan", Some("caravan-sprint")));
        assert!(!modo_do_evento_aceita("caravan", Some("highscore")));
    }

    /// Contra o emulador (nunca produção): inscrição paga, sem cobrar duas vezes, e partida de evento.
    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn inscricao_e_partida_de_evento_no_emulador() {
        assert!(std::env::var("FIRESTORE_EMULATOR_HOST").is_ok(), "este teste só roda no emulador");
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let sufixo = uuid::Uuid::new_v4().simple().to_string();
        let (jogo, evento_id, uid) = (format!("jogo-{sufixo}"), format!("copa-{sufixo}"), format!("uid-{sufixo}"));
        let mut campos_jogo = Campos::new();
        campos_jogo.insert("title".into(), texto("Jogo"));
        campos_jogo.insert("published".into(), booleano(true));
        fs.gravar(&format!("games/{jogo}"), campos_jogo, &["title", "published"]).await.unwrap();
        let agora = Utc::now();
        let e = EventoEntrada {
            game_ids: vec![jogo.clone()],
            starts_at: agora - chrono::Duration::hours(1),
            ends_at: agora + chrono::Duration::days(1),
            published: true,
            entry_fee: 5,
            ..entrada()
        };
        salvar(&fs, &evento_id, e).await.unwrap();

        // Antes de se inscrever: aparece sem inscrição e a partida de evento é recusada.
        let lista = listar_para_jogador(&fs, &uid).await.unwrap();
        assert!(lista.iter().any(|e| e.id == evento_id && !e.joined && e.entry_fee == 5));
        let detalhes = crate::services::gameplays::Detalhes { origem: "pc", evento_id: Some(evento_id.clone()), ..Default::default() };
        assert!(crate::services::gameplays::iniciar(&fs, &uid, &jogo, &detalhes).await.is_err());

        // Inscrição: carteira nova (50) menos 5; de novo, não cobra.
        let feita = participar(&fs, &uid, &evento_id).await.unwrap();
        assert!(feita.event.joined);
        assert_eq!(feita.balance, Some(crate::services::gameplays::FICHAS_INICIAIS - 5));
        assert_eq!(participar(&fs, &uid, &evento_id).await.unwrap().balance, None);
        assert!(listar_para_jogador(&fs, &uid).await.unwrap().iter().any(|e| e.id == evento_id && e.joined));

        // Inscrito: a partida vale no evento e leva o eventId; modo de outro tipo é recusado.
        let run = crate::services::gameplays::iniciar(&fs, &uid, &jogo, &detalhes).await.unwrap();
        let gameplay = fs.ler_documento(&format!("gameplays/{}", run.run_id)).await.unwrap().unwrap();
        assert_eq!(ler_texto(&gameplay, "eventId"), Some(evento_id.clone()));
        let caravana = crate::services::gameplays::Detalhes { modo: Some("caravan-sprint".into()), ..detalhes.clone() };
        assert!(crate::services::gameplays::iniciar(&fs, &uid, &jogo, &caravana).await.is_err());
        let saldo = fs.ler_documento(&format!("wallets/{uid}")).await.unwrap().unwrap();
        assert_eq!(ler_inteiro(&saldo, "balance"), Some(crate::services::gameplays::FICHAS_INICIAIS - 6));
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
