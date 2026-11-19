//! Notificações do ShmupX Client e registro de jogadores para os avisos.
//!
//! - `notifications/{id}`: avisos para todos os jogadores (hoje, versões agendadas do Client:
//!   `release-{versão}`). Cancelar a versão esconde o aviso (`hidden`).
//! - `players/{uid}`: e-mail (do token do Firebase, verificado), preferência de aviso por e-mail
//!   (`emailUpdates`, ligada por padrão, desligável pelo jogador), versão do Client e as
//!   notificações já lidas (`readNotifications`). Criado/atualizado quando o Client consulta as
//!   notificações; é a lista de destinatários dos e-mails.

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

/// Avisos visíveis, do mais novo para o mais antigo, com o contador de não lidos do jogador.
pub async fn listar(fs: &Firestore, jogador: &Campos) -> Result<ListaNotificacoes, AppError> {
    let lidas = ler_lista_de_textos(jogador, "readNotifications");
    let mut items: Vec<NotificacaoResumo> = fs
        .listar("notifications")
        .await?
        .into_iter()
        .filter(|(_, campos)| !ler_booleano(campos, "hidden").unwrap_or(false))
        .map(|(id, campos)| resumo_de(id, &campos, &lidas))
        .collect();
    items.sort_by_key(|n| std::cmp::Reverse(n.created_at));
    items.truncate(LIDAS_MAXIMAS);
    Ok(ListaNotificacoes { unread: items.iter().filter(|n| !n.read).count(), items })
}

fn id_seguro(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

pub async fn obter(fs: &Firestore, jogador: &Campos, id: &str) -> Result<Notificacao, AppError> {
    if !id_seguro(id) {
        return Err(AppError::bad_request("id de notificação inválido"));
    }
    let campos = fs
        .ler_documento(&format!("notifications/{id}"))
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

/// E-mails que aceitam avisos: verificados e com a preferência ligada.
pub async fn destinatarios(fs: &Firestore) -> Result<Vec<String>, AppError> {
    let mut emails: Vec<String> = fs
        .listar("players")
        .await?
        .into_iter()
        .filter(|(_, c)| ler_booleano(c, "emailVerified").unwrap_or(false) && ler_booleano(c, "emailUpdates").unwrap_or(true))
        .filter_map(|(_, c)| ler_texto(&c, "email"))
        .collect();
    emails.sort();
    emails.dedup();
    Ok(emails)
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
        let usuario = FirebaseUser { uid: format!("uid-{}", uuid::Uuid::new_v4()), email: Some("jogador@example.com".into()), email_verified: true };

        let entrada = VersaoEntrada { title: "Teste".into(), fixes: vec!["Correção".into()], mandatory: true, ..Default::default() };
        let rascunho = versoes::salvar_rascunho(&fs, &versao, entrada).await.unwrap();
        assert_eq!(rascunho.status, "draft");

        let agendada = versoes::agendar(&fs, &versao, Antecedencia::Urgente).await.unwrap();
        assert_eq!(agendada.status, "released", "urgente libera na hora");
        let id = publicar_versao(&fs, &agendada).await.unwrap();

        let jogador = registrar_jogador(&fs, &usuario, Some("0.1.0")).await.unwrap();
        assert_eq!(ler_booleano(&jogador, "emailUpdates"), Some(true));
        let lista = listar(&fs, &jogador).await.unwrap();
        assert!(lista.items.iter().any(|n| n.id == id && !n.read));
        assert!(destinatarios(&fs).await.unwrap().contains(&"jogador@example.com".to_string()));

        marcar_lidas(&fs, &usuario.uid, std::slice::from_ref(&id)).await.unwrap();
        let jogador = registrar_jogador(&fs, &usuario, Some("0.1.0")).await.unwrap();
        assert!(listar(&fs, &jogador).await.unwrap().items.iter().any(|n| n.id == id && n.read));
        assert_eq!(obter(&fs, &jogador, &id).await.unwrap().fixes, vec!["Correção"]);

        // Obrigatória liberada: Client antigo não abre run; o atualizado abre.
        assert!(versoes::exigir_versao_em_dia(&fs, Some("0.1.0")).await.is_err());
        assert!(versoes::exigir_versao_em_dia(&fs, Some(&versao)).await.is_ok());

        // Preferência desligada sai da lista de e-mails e não volta ao registrar de novo.
        definir_preferencia_email(&fs, &usuario.uid, false).await.unwrap();
        registrar_jogador(&fs, &usuario, None).await.unwrap();
        let caminho = format!("players/{}", usuario.uid);
        assert_eq!(ler_booleano(&fs.ler_documento(&caminho).await.unwrap().unwrap(), "emailUpdates"), Some(false));

        // Cancelar só vale para agendadas ainda não liberadas.
        let futura = format!("{}.0.0", maior.0 + 2);
        let entrada = VersaoEntrada { title: "Futura".into(), features: vec!["x".into()], ..Default::default() };
        versoes::salvar_rascunho(&fs, &futura, entrada).await.unwrap();
        assert_eq!(versoes::agendar(&fs, &futura, Antecedencia::Dias7).await.unwrap().status, "scheduled");
        assert!(versoes::cancelar(&fs, &versao).await.is_err(), "já liberada");
        assert_eq!(versoes::cancelar(&fs, &futura).await.unwrap().status, "cancelled");
    }

    #[test]
    fn resumo_marca_lida() {
        let mut campos = Campos::new();
        campos.insert("title".into(), texto("t"));
        assert!(resumo_de("a".into(), &campos, &["a".into()]).read);
        assert!(!resumo_de("b".into(), &campos, &["a".into()]).read);
    }
}
