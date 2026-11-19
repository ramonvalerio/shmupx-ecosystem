use std::sync::Arc;

use axum::extract::Path;
use axum::middleware;
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use serde::Deserialize;

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};
use crate::services::catalogo::{self, Jogo, JogoEntrada};
use crate::services::email::{mensagem_da_versao, Ses};
use crate::services::notificacoes;
use crate::services::versoes::{self, Versao, VersaoEntrada};

/// Rotas do Lab. Exigem ID token do Firebase e, além disso, admin (`ADMIN_EMAILS`): o servidor
/// decide a permissão a cada chamada; o app do Lab não carrega credencial nem segredo.
pub fn router<S>(firestore: Arc<Firestore>, email: Option<Arc<Ses>>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/admin/games", get(listar_jogos))
        .route("/api/admin/games/{id}", put(salvar_jogo))
        .route("/api/admin/releases", get(listar_versoes))
        .route("/api/admin/releases/{version}", put(salvar_versao))
        .route("/api/admin/releases/{version}/schedule", post(agendar_versao))
        .route("/api/admin/releases/{version}/cancel", post(cancelar_versao))
        .route_layer(middleware::from_fn(exigir_usuario_firebase))
        .layer(Extension(firestore))
        .layer(Extension(email))
}

fn exigir_admin(usuario: &FirebaseUser) -> Result<(), AppError> {
    if usuario.e_admin() {
        Ok(())
    } else {
        Err(AppError::forbidden("acesso restrito a administradores"))
    }
}

/// `GET /api/admin/games`: todos os jogos do catálogo, inclusive rascunhos.
async fn listar_jogos(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Vec<Jogo>>, AppError> {
    exigir_admin(&usuario)?;
    Ok(Json(catalogo::listar(&firestore).await?))
}

/// `PUT /api/admin/games/{id}`: cria ou atualiza o jogo. `published: true` o torna visível ao
/// Client e liberado para runs; `false` o esconde e bloqueia novas runs.
async fn salvar_jogo(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
    Json(entrada): Json<JogoEntrada>,
) -> Result<Json<Jogo>, AppError> {
    exigir_admin(&usuario)?;
    let jogo = catalogo::salvar(&firestore, &id, entrada).await?;
    println!("catálogo: {} salvou o jogo {} (published={})", usuario.uid, jogo.id, jogo.published);
    Ok(Json(jogo))
}

/// `GET /api/admin/releases`: versões do Client, da mais nova para a mais antiga.
async fn listar_versoes(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Vec<Versao>>, AppError> {
    exigir_admin(&usuario)?;
    Ok(Json(versoes::listar(&firestore).await?))
}

/// `PUT /api/admin/releases/{version}`: cria ou edita o rascunho da versão.
async fn salvar_versao(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(versao): Path<String>,
    Json(entrada): Json<VersaoEntrada>,
) -> Result<Json<Versao>, AppError> {
    exigir_admin(&usuario)?;
    let salva = versoes::salvar_rascunho(&firestore, &versao, entrada).await?;
    println!("versões: {} salvou o rascunho {}", usuario.uid, salva.version);
    Ok(Json(salva))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgendarPayload {
    /// `urgent`, `24h`, `48h` ou `7d`.
    lead: String,
}

/// `POST /api/admin/releases/{version}/schedule`: agenda a versão com a antecedência escolhida,
/// publica o aviso no Client e dispara os e-mails em segundo plano.
async fn agendar_versao(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(email): Extension<Option<Arc<Ses>>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(versao): Path<String>,
    Json(payload): Json<AgendarPayload>,
) -> Result<Json<Versao>, AppError> {
    exigir_admin(&usuario)?;
    let antecedencia =
        versoes::antecedencia_de(&payload.lead).ok_or_else(|| AppError::bad_request("antecedência inválida (urgent, 24h, 48h ou 7d)"))?;
    let agendada = versoes::agendar(&firestore, &versao, antecedencia).await?;
    notificacoes::publicar_versao(&firestore, &agendada).await?;
    println!("versões: {} agendou {} (antecedência {})", usuario.uid, agendada.version, payload.lead);

    let Some(email) = email else {
        versoes::registrar_emails(&firestore, &agendada.version, "not_configured", 0).await?;
        return Ok(Json(versoes::obter(&firestore, &agendada.version).await?));
    };
    versoes::registrar_emails(&firestore, &agendada.version, "sending", 0).await?;
    let (fs, versao_enviada) = (firestore.clone(), agendada.clone());
    tokio::spawn(async move {
        let (status, enviados) = match notificacoes::destinatarios(&fs).await {
            Err(e) => (format!("failed: {e:?}"), 0),
            Ok(lista) if lista.is_empty() => ("sent".to_string(), 0),
            Ok(lista) => {
                let (assunto, html, texto) = mensagem_da_versao(&versao_enviada);
                match email.enviar(&lista, &assunto, &html, &texto).await {
                    Ok(n) => ("sent".to_string(), n as i64),
                    Err(e) => (format!("failed: {e}"), 0),
                }
            }
        };
        println!("versões: e-mails de {}: {status} ({enviados})", versao_enviada.version);
        if let Err(e) = versoes::registrar_emails(&fs, &versao_enviada.version, &status, enviados).await {
            eprintln!("versões: falha ao registrar o envio de e-mails: {e:?}");
        }
    });
    Ok(Json(versoes::obter(&firestore, &agendada.version).await?))
}

/// `POST /api/admin/releases/{version}/cancel`: cancela um agendamento ainda não liberado e
/// esconde o aviso no Client.
async fn cancelar_versao(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(versao): Path<String>,
) -> Result<Json<Versao>, AppError> {
    exigir_admin(&usuario)?;
    let cancelada = versoes::cancelar(&firestore, &versao).await?;
    notificacoes::esconder_versao(&firestore, &cancelada.version).await?;
    println!("versões: {} cancelou {}", usuario.uid, cancelada.version);
    Ok(Json(cancelada))
}
