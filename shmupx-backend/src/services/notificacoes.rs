//! Notificações do ShmupX Client e registro de jogadores para os avisos.
//!
//! - `notifications/{id}`: avisos para todos os jogadores (hoje, versões agendadas do Client:
//!   `release-{versão}`). Cancelar a versão esconde o aviso (`hidden`).
//! - `players/{uid}`: e-mail (do token do Firebase, verificado), preferência de aviso por e-mail
//!   (`emailUpdates`, ligada por padrão, desligável pelo jogador), versão do Client e as
//!   notificações já lidas (`readNotifications`). Criado/atualizado quando o Client consulta as
//!   notificações; é a lista de destinatários dos e-mails. `unsubscribeToken` assina o link
//!   "parar de receber" do rodapé dos e-mails, que funciona sem o Client.
//! - `players/{uid}/inbox/{id}`: avisos só daquele jogador. Todo e-mail enviado a um jogador
//!   específico (ex.: token do MiSTer) grava aqui o aviso correspondente, para aparecer também no
//!   sininho. Os e-mails de versão já têm o aviso geral em `notifications/release-*`.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::error::AppError;
use crate::firestore::valor::{booleano, data, ler_booleano, ler_data, ler_lista_de_textos, ler_texto, lista_de_textos, texto};
use crate::firestore::{Campos, Firestore};
use crate::middleware::firebase::FirebaseUser;
use crate::services::versoes::{ler_versao, Versao};

const LIDAS_MAXIMAS: usize = 50;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NotificacaoResumo {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub created_at: DateTime<Utc>,
    pub read: bool,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ListaNotificacoes {
    pub unread: usize,
    pub items: Vec<NotificacaoResumo>,
}

/// Conteúdo completo, para a tela de detalhe.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Notificacao {
    #[serde(flatten)]
    pub resumo: NotificacaoResumo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_at: Option<DateTime<Utc>>,
    pub mandatory: bool,
    pub features: Vec<String>,
    pub fixes: Vec<String>,
    pub rules: Vec<String>,
    pub scripts: Vec<String>,
}

/// Registra o jogador (ou atualiza e-mail e versão do Client). A preferência de e-mail nasce
/// ligada e nunca é sobrescrita aqui.
pub async fn registrar_jogador(fs: &Firestore, usuario: &FirebaseUser, versao_client: Option<&str>) -> Result<Campos, AppError> {
    let caminho = format!("players/{}", usuario.uid);
    let existente = fs.ler_documento(&caminho).await?;
    let mut campos = Campos::new();
    let mut mascara = vec!["emailVerified", "lastSeenAt"];
    campos.insert("emailVerified".into(), booleano(usuario.email_verified));
    campos.insert("lastSeenAt".into(), data(Utc::now()));
    if let Some(email) = &usuario.email {
        campos.insert("email".into(), texto(email));
        mascara.push("email");
    }
    if let Some(versao) = versao_client.filter(|v| ler_versao(v).is_some()) {
        campos.insert("clientVersion".into(), texto(versao));
        mascara.push("clientVersion");
    }
    if existente.as_ref().and_then(|c| ler_booleano(c, "emailUpdates")).is_none() {
        campos.insert("emailUpdates".into(), booleano(true));
        mascara.push("emailUpdates");
    }
    if existente.as_ref().and_then(|c| ler_texto(c, CAMPO_TOKEN)).is_none() {
        campos.insert(CAMPO_TOKEN.into(), texto(&novo_token()));
        mascara.push(CAMPO_TOKEN);
    }
    fs.gravar(&caminho, campos.clone(), &mascara).await?;
    let mut atual = existente.unwrap_or_default();
    atual.extend(campos);
    Ok(atual)
}

fn resumo_de(id: String, campos: &Campos, lidas: &[String]) -> NotificacaoResumo {
    NotificacaoResumo {
        read: lidas.contains(&id),
        kind: ler_texto(campos, "kind").unwrap_or_else(|| "info".into()),
        title: ler_texto(campos, "title").unwrap_or_default(),
        summary: ler_texto(campos, "summary").unwrap_or_default(),
        created_at: ler_data(campos, "createdAt").unwrap_or_default(),
        id,
    }
}

/// Avisos visíveis (gerais e os do próprio jogador), do mais novo para o mais antigo, com o
/// contador de não lidos.
pub async fn listar(fs: &Firestore, uid: &str, jogador: &Campos) -> Result<ListaNotificacoes, AppError> {
    let lidas = ler_lista_de_textos(jogador, "readNotifications");
    let mut documentos = fs.listar("notifications").await?;
    documentos.extend(fs.listar(&caixa_do_jogador(uid)).await?);
    let mut items: Vec<NotificacaoResumo> = documentos
        .into_iter()
        .filter(|(_, campos)| !ler_booleano(campos, "hidden").unwrap_or(false))
        .map(|(id, campos)| resumo_de(id, &campos, &lidas))
        .collect();
    items.sort_by_key(|n| std::cmp::Reverse(n.created_at));
    items.truncate(LIDAS_MAXIMAS);
    Ok(ListaNotificacoes { unread: items.iter().filter(|n| !n.read).count(), items })
}

fn caixa_do_jogador(uid: &str) -> String {
    format!("players/{uid}/inbox")
}

/// Grava um aviso só para o jogador `uid` (sininho) e devolve o id. Use junto de todo e-mail
/// mandado a um jogador: o que chega por e-mail também aparece no Client.
pub async fn notificar_jogador(fs: &Firestore, uid: &str, kind: &str, titulo: &str, resumo: &str) -> Result<String, AppError> {
    let id = format!("u-{}", uuid::Uuid::new_v4().simple());
    let mut campos = Campos::new();
    campos.insert("kind".into(), texto(kind));
    campos.insert("title".into(), texto(titulo));
    campos.insert("summary".into(), texto(resumo));
    campos.insert("createdAt".into(), data(Utc::now()));
    campos.insert("hidden".into(), booleano(false));
    fs.gravar(&format!("{}/{id}", caixa_do_jogador(uid)), campos, &["kind", "title", "summary", "createdAt", "hidden"]).await?;
    Ok(id)
}

fn id_seguro(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

pub async fn obter(fs: &Firestore, uid: &str, jogador: &Campos, id: &str) -> Result<Notificacao, AppError> {
    if !id_seguro(id) {
        return Err(AppError::bad_request("id de notificação inválido"));
    }
    // Avisos do jogador têm o prefixo `u-`; os gerais, não.
    let caminho = if id.starts_with("u-") { format!("{}/{id}", caixa_do_jogador(uid)) } else { format!("notifications/{id}") };
    let campos = fs
        .ler_documento(&caminho)
        .await?
        .filter(|c| !ler_booleano(c, "hidden").unwrap_or(false))
        .ok_or_else(|| AppError::not_found("notificação não encontrada"))?;
    let lidas = ler_lista_de_textos(jogador, "readNotifications");
    Ok(Notificacao {
        resumo: resumo_de(id.to_string(), &campos, &lidas),
        version: ler_texto(&campos, "version"),
        release_at: ler_data(&campos, "releaseAt"),
        mandatory: ler_booleano(&campos, "mandatory").unwrap_or(false),
        features: ler_lista_de_textos(&campos, "features"),
        fixes: ler_lista_de_textos(&campos, "fixes"),
        rules: ler_lista_de_textos(&campos, "rules"),
        scripts: ler_lista_de_textos(&campos, "scripts"),
    })
}

pub async fn marcar_lidas(fs: &Firestore, uid: &str, ids: &[String]) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    if ids.iter().any(|id| !id_seguro(id)) {
        return Err(AppError::bad_request("id de notificação inválido"));
    }
    fs.acrescentar_na_lista(&format!("players/{uid}"), "readNotifications", ids).await
}

pub async fn definir_preferencia_email(fs: &Firestore, uid: &str, ligada: bool) -> Result<(), AppError> {
    let mut campos = Campos::new();
    campos.insert("emailUpdates".into(), booleano(ligada));
    fs.gravar(&format!("players/{uid}"), campos, &["emailUpdates"]).await
}

/// Data no formato dos avisos (UTC; o Client mostra no fuso do jogador).
fn quando(data: DateTime<Utc>) -> String {
    data.format("%d/%m/%Y %H:%M UTC").to_string()
}

/// Título e resumo do aviso de uma versão agendada.
pub fn textos_da_versao(versao: &Versao) -> (String, String) {
    let titulo = format!("ShmupX Client {}: {}", versao.version, versao.title);
    let liberacao = match (versao.lead.as_deref(), versao.release_at) {
        (Some("urgent"), _) | (_, None) => "Disponível agora.".to_string(),
        (_, Some(data)) => format!("Chega em {}.", quando(data)),
    };
    let obrigatoria = if versao.mandatory { " Atualização obrigatória para continuar jogando." } else { "" };
    (titulo, format!("{liberacao}{obrigatoria}"))
}

/// Publica (ou republica) o aviso da versão agendada.
pub async fn publicar_versao(fs: &Firestore, versao: &Versao) -> Result<String, AppError> {
    let id = format!("release-{}", versao.version);
    let (titulo, resumo) = textos_da_versao(versao);
    let mut campos = Campos::new();
    campos.insert("kind".into(), texto("release"));
    campos.insert("title".into(), texto(&titulo));
    campos.insert("summary".into(), texto(&resumo));
    campos.insert("createdAt".into(), data(Utc::now()));
    campos.insert("version".into(), texto(&versao.version));
    campos.insert("mandatory".into(), booleano(versao.mandatory));
    campos.insert("features".into(), lista_de_textos(&versao.features));
    campos.insert("fixes".into(), lista_de_textos(&versao.fixes));
    campos.insert("rules".into(), lista_de_textos(&versao.rules));
    campos.insert("scripts".into(), lista_de_textos(&versao.scripts));
    campos.insert("hidden".into(), booleano(false));
    let mut mascara =
        vec!["kind", "title", "summary", "createdAt", "version", "mandatory", "features", "fixes", "rules", "scripts", "hidden"];
    if let Some(release_at) = versao.release_at {
        campos.insert("releaseAt".into(), data(release_at));
        mascara.push("releaseAt");
    }
    fs.gravar(&format!("notifications/{id}"), campos, &mascara).await?;
    Ok(id)
}

/// Esconde o aviso de uma versão cancelada.
pub async fn esconder_versao(fs: &Firestore, versao: &str) -> Result<(), AppError> {
    let mut campos = Campos::new();
    campos.insert("hidden".into(), booleano(true));
    fs.gravar(&format!("notifications/release-{versao}"), campos, &["hidden"]).await
}

/// Quem recebe os avisos por e-mail, com o token do link "parar de receber" do rodapé.
#[derive(Debug, Clone, PartialEq)]
pub struct Destinatario {
    pub uid: String,
    pub email: String,
    pub token: String,
}

/// Token aleatório do link de descadastro (`players/{uid}.unsubscribeToken`): o link do e-mail
/// desliga os avisos sem login e sem o Client instalado.
const CAMPO_TOKEN: &str = "unsubscribeToken";

fn novo_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// E-mails que aceitam avisos: verificados e com a preferência ligada. Jogadores registrados
/// antes do token ganham um agora.
pub async fn destinatarios(fs: &Firestore) -> Result<Vec<Destinatario>, AppError> {
    let mut lista = Vec::new();
    for (uid, c) in fs.listar("players").await? {
        if !(ler_booleano(&c, "emailVerified").unwrap_or(false) && ler_booleano(&c, "emailUpdates").unwrap_or(true)) {
            continue;
        }
        let Some(email) = ler_texto(&c, "email") else { continue };
        let token = match ler_texto(&c, CAMPO_TOKEN) {
            Some(t) => t,
            None => {
                let t = novo_token();
                let mut campos = Campos::new();
                campos.insert(CAMPO_TOKEN.into(), texto(&t));
                fs.gravar(&format!("players/{uid}"), campos, &[CAMPO_TOKEN]).await?;
                t
            }
        };
        lista.push(Destinatario { uid, email, token });
    }
    lista.sort_by(|a, b| a.email.cmp(&b.email));
    lista.dedup_by(|a, b| a.email == b.email);
    Ok(lista)
}

/// Comparação sem atalho, para o tempo de resposta não revelar o token.
fn tokens_iguais(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Link do rodapé: desliga os avisos por e-mail do jogador. `false` quando o link não confere.
pub async fn descadastrar(fs: &Firestore, uid: &str, token: &str) -> Result<bool, AppError> {
    let uid_valido = !uid.is_empty() && uid.len() <= 128 && uid.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
    if !uid_valido || token.is_empty() {
        return Ok(false);
    }
    let Some(jogador) = fs.ler_documento(&format!("players/{uid}")).await? else { return Ok(false) };
    let Some(guardado) = ler_texto(&jogador, CAMPO_TOKEN) else { return Ok(false) };
    if !tokens_iguais(&guardado, token) {
        return Ok(false);
    }
    definir_preferencia_email(fs, uid, false).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn versao(lead: &str, mandatory: bool) -> Versao {
        Versao {
            version: "0.2.0".into(),
            title: "Aero Fighters 2".into(),
            lead: Some(lead.into()),
            release_at: Some(Utc.with_ymd_and_hms(2026, 11, 2, 15, 0, 0).unwrap()),
            mandatory,
            ..Default::default()
        }
    }

    #[test]
    fn textos_do_aviso() {
        let (titulo, resumo) = textos_da_versao(&versao("48h", false));
        assert_eq!(titulo, "ShmupX Client 0.2.0: Aero Fighters 2");
        assert_eq!(resumo, "Chega em 02/11/2026 15:00 UTC.");
        assert_eq!(textos_da_versao(&versao("urgent", true)).1, "Disponível agora. Atualização obrigatória para continuar jogando.");
    }

    #[test]
    fn ids_de_notificacao_nao_viram_caminhos() {
        assert!(id_seguro("release-0.2.0"));
        assert!(id_seguro(&format!("u-{}", uuid::Uuid::new_v4().simple())));
        assert!(!id_seguro("../players/x"));
        assert!(!id_seguro(""));
    }

    /// Contra o emulador (nunca produção): versão agendada vira aviso, contador e lida.
    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn versao_vira_aviso_lido_e_bloqueio_no_emulador() {
        use crate::services::versoes::{self, Antecedencia, VersaoEntrada};
        assert!(std::env::var("FIRESTORE_EMULATOR_HOST").is_ok(), "este teste só roda no emulador");
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        // Versão única por execução, sempre acima das de execuções anteriores no mesmo emulador.
        let maior = versoes::listar(&fs).await.unwrap().iter().filter_map(|v| ler_versao(&v.version)).max().unwrap_or((0, 0, 0));
        let versao = format!("{}.0.0", maior.0 + 1);
        // E-mail único por execução: a lista de destinatários junta e-mails repetidos.
        let id = uuid::Uuid::new_v4().simple().to_string();
        let usuario = FirebaseUser { uid: format!("uid-{id}"), email: Some(format!("jogador-{id}@example.com")), email_verified: true };

        let entrada = VersaoEntrada { title: "Teste".into(), fixes: vec!["Correção".into()], mandatory: true, ..Default::default() };
        let rascunho = versoes::salvar_rascunho(&fs, &versao, entrada).await.unwrap();
        assert_eq!(rascunho.status, "draft");

        let agendada = versoes::agendar_com(&fs, &versao, Antecedencia::Urgente, Default::default()).await.unwrap();
        assert_eq!(agendada.status, "released", "urgente libera na hora");
        let id = publicar_versao(&fs, &agendada).await.unwrap();

        let jogador = registrar_jogador(&fs, &usuario, Some("0.1.0")).await.unwrap();
        assert_eq!(ler_booleano(&jogador, "emailUpdates"), Some(true));
        let lista = listar(&fs, &usuario.uid, &jogador).await.unwrap();
        assert!(lista.items.iter().any(|n| n.id == id && !n.read));
        let token = ler_texto(&jogador, CAMPO_TOKEN).expect("token de descadastro criado no registro");
        assert!(destinatarios(&fs).await.unwrap().iter().any(|d| d.uid == usuario.uid && d.token == token));
        assert!(!descadastrar(&fs, &usuario.uid, "errado").await.unwrap(), "token errado não descadastra");

        marcar_lidas(&fs, &usuario.uid, std::slice::from_ref(&id)).await.unwrap();
        let jogador = registrar_jogador(&fs, &usuario, Some("0.1.0")).await.unwrap();
        assert!(listar(&fs, &usuario.uid, &jogador).await.unwrap().items.iter().any(|n| n.id == id && n.read));
        assert_eq!(obter(&fs, &usuario.uid, &jogador, &id).await.unwrap().fixes, vec!["Correção"]);

        // Aviso só deste jogador (o que acompanha um e-mail individual): aparece no sininho dele,
        // abre pelo id e marca como lido como os gerais.
        let pessoal = notificar_jogador(&fs, &usuario.uid, "mister", "Token do MiSTer", "Enviado por e-mail.").await.unwrap();
        let lista = listar(&fs, &usuario.uid, &jogador).await.unwrap();
        assert!(lista.items.iter().any(|n| n.id == pessoal && !n.read && n.kind == "mister"));
        assert_eq!(obter(&fs, &usuario.uid, &jogador, &pessoal).await.unwrap().resumo.title, "Token do MiSTer");
        assert!(obter(&fs, "outro-uid", &jogador, &pessoal).await.is_err(), "aviso de outro jogador não abre");
        marcar_lidas(&fs, &usuario.uid, std::slice::from_ref(&pessoal)).await.unwrap();
        let jogador = registrar_jogador(&fs, &usuario, Some("0.1.0")).await.unwrap();
        assert!(listar(&fs, &usuario.uid, &jogador).await.unwrap().items.iter().any(|n| n.id == pessoal && n.read));

        // Obrigatória liberada: Client antigo não abre run; o atualizado abre.
        assert!(versoes::exigir_versao_em_dia(&fs, Some("0.1.0")).await.is_err());
        assert!(versoes::exigir_versao_em_dia(&fs, Some(&versao)).await.is_ok());

        // Preferência desligada sai da lista de e-mails e não volta ao registrar de novo.
        definir_preferencia_email(&fs, &usuario.uid, false).await.unwrap();
        registrar_jogador(&fs, &usuario, None).await.unwrap();
        let caminho = format!("players/{}", usuario.uid);
        assert_eq!(ler_booleano(&fs.ler_documento(&caminho).await.unwrap().unwrap(), "emailUpdates"), Some(false));
        // O link do e-mail também desliga, sem login.
        definir_preferencia_email(&fs, &usuario.uid, true).await.unwrap();
        assert!(descadastrar(&fs, &usuario.uid, &token).await.unwrap());
        assert!(!destinatarios(&fs).await.unwrap().iter().any(|d| d.uid == usuario.uid));

        // Cancelar só vale para agendadas ainda não liberadas.
        let futura = format!("{}.0.0", maior.0 + 2);
        let entrada = VersaoEntrada { title: "Futura".into(), features: vec!["x".into()], ..Default::default() };
        versoes::salvar_rascunho(&fs, &futura, entrada).await.unwrap();
        assert_eq!(versoes::agendar_com(&fs, &futura, Antecedencia::Dias7, Default::default()).await.unwrap().status, "scheduled");
        assert!(versoes::cancelar(&fs, &versao).await.is_err(), "já liberada");
        assert_eq!(versoes::cancelar(&fs, &futura).await.unwrap().status, "cancelled");
    }

    #[test]
    fn tokens_de_descadastro() {
        let t = novo_token();
        assert_eq!(t.len(), 32);
        assert_ne!(t, novo_token());
        assert!(tokens_iguais(&t, &t.clone()));
        assert!(!tokens_iguais(&t, &t[..31]));
        assert!(!tokens_iguais("a".repeat(32).as_str(), "b".repeat(32).as_str()));
    }

    #[test]
    fn resumo_marca_lida() {
        let mut campos = Campos::new();
        campos.insert("title".into(), texto("t"));
        assert!(resumo_de("a".into(), &campos, &["a".into()]).read);
        assert!(!resumo_de("b".into(), &campos, &["a".into()]).read);
    }
}
