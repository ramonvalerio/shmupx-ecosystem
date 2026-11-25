use std::sync::Arc;

use axum::extract::Path;
use axum::middleware;
use axum::routing::{delete, get, post, put};
use axum::{Extension, Json, Router};
use serde::Deserialize;

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};
use crate::services::catalogo::{self, Jogo, JogoEntrada};
use crate::services::denuncias::{self, Denuncia, Resolucao};
use crate::services::distribuicoes::{self, Distribuicao, DistribuicaoEntrada};
use crate::services::usuarios::{self, Moderacao, ModeracaoEntrada, UsuarioAdmin};
use crate::services::email::{mensagem_da_versao, Ses};
use crate::services::notificacoes;
use crate::services::robotx::{self, ConhecimentoEntrada, ItemRobotx};
use crate::services::versoes::{self, Instalador, Mudanca, OpcoesAgendamento, Versao, VersaoEntrada};

/// Rotas do Lab. Exigem ID token do Firebase e, além disso, admin (`ADMIN_EMAILS`): o servidor
/// decide a permissão a cada chamada; o app do Lab não carrega credencial nem segredo.
pub fn router<S>(firestore: Arc<Firestore>, email: Option<Arc<Ses>>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/admin/games", get(listar_jogos))
        .route("/api/admin/games/{id}", post(criar_jogo).put(salvar_jogo).delete(excluir_jogo))
        .route("/api/admin/games/{id}/robotx", get(obter_robotx).put(salvar_robotx))
        .route("/api/admin/releases", get(listar_versoes))
        .route("/api/admin/releases/draft", get(rascunho_aberto))
        .route("/api/admin/releases/draft/changes", post(registrar_mudanca))
        .route("/api/admin/releases/{version}", put(salvar_versao))
        .route("/api/admin/releases/{version}/auto/{key}", delete(remover_auto))
        .route("/api/admin/releases/{version}/installer", put(definir_instalador))
        .route("/api/admin/releases/{version}/schedule", post(agendar_versao))
        .route("/api/admin/releases/{version}/cancel", post(cancelar_versao))
        .route("/api/admin/coins/grants", get(listar_distribuicoes).post(criar_distribuicao))
        .route("/api/admin/coins/grants/{id}/cancel", post(cancelar_distribuicao))
        .route("/api/admin/coins/grants/{id}/retry", post(repetir_distribuicao))
        .route("/api/admin/users", get(listar_usuarios))
        .route("/api/admin/users/{uid}/moderation", put(moderar_usuario))
        .route("/api/admin/reports", get(listar_denuncias))
        .route("/api/admin/reports/{id}/resolve", post(resolver_denuncia))
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

/// `POST /api/admin/games/{id}`: cadastra um jogo novo. ID já usado = `409`, nada é sobrescrito.
async fn criar_jogo(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
    Json(entrada): Json<JogoEntrada>,
) -> Result<Json<Jogo>, AppError> {
    exigir_admin(&usuario)?;
    let jogo = catalogo::criar(&firestore, &id, entrada).await?;
    println!("catálogo: {} criou o jogo {} (published={})", usuario.uid, jogo.id, jogo.published);
    registrar_no_rascunho(&firestore, None, &jogo).await;
    Ok(Json(jogo))
}

/// `DELETE /api/admin/games/{id}`: tira o jogo do catálogo (some do Client e fecha novas runs).
/// Runs e rankings já gravados com o ID ficam guardados.
async fn excluir_jogo(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    exigir_admin(&usuario)?;
    let jogo = catalogo::excluir(&firestore, &id).await?;
    println!("catálogo: {} excluiu o jogo {} (published={})", usuario.uid, jogo.id, jogo.published);
    // Para os jogadores, excluir um jogo publicado é o mesmo que retirá-lo do catálogo.
    registrar_no_rascunho(&firestore, Some(&jogo), &Jogo { published: false, ..jogo.clone() }).await;
    Ok(Json(serde_json::json!({ "id": jogo.id })))
}

/// A alteração entra no rascunho da próxima versão. Falhar aqui não desfaz o catálogo, que já
/// vale para os jogadores: só fica registrado no log.
async fn registrar_no_rascunho(firestore: &Firestore, antes: Option<&Jogo>, depois: &Jogo) {
    for mudanca in catalogo::mudancas_do_jogo(antes, depois) {
        if let Err(e) = versoes::registrar_mudanca(firestore, mudanca).await {
            eprintln!("versões: falha ao registrar a mudança do jogo {} no rascunho: {e:?}", depois.id);
        }
    }
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
    let antes = catalogo::obter(&firestore, &id).await?;
    let jogo = catalogo::salvar(&firestore, &id, entrada).await?;
    println!("catálogo: {} salvou o jogo {} (published={})", usuario.uid, jogo.id, jogo.published);
    registrar_no_rascunho(&firestore, antes.as_ref(), &jogo).await;
    Ok(Json(jogo))
}

/// `GET /api/admin/games/{id}/robotx`: perguntas e respostas curadas do RobotX do jogo.
async fn obter_robotx(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ItemRobotx>>, AppError> {
    exigir_admin(&usuario)?;
    Ok(Json(robotx::obter(&firestore, &id).await?))
}

/// `PUT /api/admin/games/{id}/robotx`: substitui as perguntas e respostas do jogo.
async fn salvar_robotx(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
    Json(entrada): Json<ConhecimentoEntrada>,
) -> Result<Json<Vec<ItemRobotx>>, AppError> {
    exigir_admin(&usuario)?;
    let (antes, depois) = robotx::salvar(&firestore, &id, entrada).await?;
    println!("robotx: {} salvou {} perguntas do jogo {id}", usuario.uid, depois.len());
    if let Some(jogo) = catalogo::obter(&firestore, &id).await?
        && let Some(m) = robotx::mudanca(&jogo, &antes, &depois)
        && let Err(e) = versoes::registrar_mudanca(&firestore, m).await
    {
        eprintln!("versões: falha ao registrar o RobotX do jogo {id} no rascunho: {e:?}");
    }
    Ok(Json(depois))
}

/// `GET /api/admin/releases/draft`: rascunho aberto da próxima versão (criado se não houver).
async fn rascunho_aberto(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Versao>, AppError> {
    exigir_admin(&usuario)?;
    Ok(Json(versoes::rascunho_aberto(&firestore).await?))
}

/// `POST /api/admin/releases/draft/changes`: alteração feita no Lab fora do catálogo (ex.: script
/// de validação aprovado) entra no rascunho aberto.
async fn registrar_mudanca(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(mudanca): Json<Mudanca>,
) -> Result<Json<Versao>, AppError> {
    exigir_admin(&usuario)?;
    let chave = mudanca.key.clone();
    let rascunho = versoes::registrar_mudanca(&firestore, mudanca).await?;
    println!("versões: {} registrou {chave} no rascunho {}", usuario.uid, rascunho.version);
    Ok(Json(rascunho))
}

/// `DELETE /api/admin/releases/{version}/auto/{key}`: tira um item automático do rascunho.
async fn remover_auto(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path((versao, chave)): Path<(String, String)>,
) -> Result<Json<Versao>, AppError> {
    exigir_admin(&usuario)?;
    Ok(Json(versoes::remover_auto(&firestore, &versao, &chave).await?))
}

/// `PUT /api/admin/releases/{version}/installer`: liga o instalador (já enviado ao S3 pelo Lab)
/// e a assinatura do updater ao rascunho.
async fn definir_instalador(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(versao): Path<String>,
    Json(instalador): Json<Instalador>,
) -> Result<Json<Versao>, AppError> {
    exigir_admin(&usuario)?;
    let salva = versoes::definir_instalador(&firestore, &versao, instalador).await?;
    println!("versões: {} ligou o instalador de {}", usuario.uid, salva.version);
    Ok(Json(salva))
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
    /// Clients baixam e instalam sozinhos ao liberar (exige instalador). Ausente = não.
    #[serde(default)]
    auto_update: bool,
    /// E-mail aos jogadores que aceitam avisos. Ausente = sim (comportamento anterior).
    #[serde(default = "sim")]
    send_email: bool,
}

fn sim() -> bool {
    true
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
    let opcoes = OpcoesAgendamento { auto_update: payload.auto_update };
    let agendada = versoes::agendar_com(&firestore, &versao, antecedencia, opcoes).await?;
    notificacoes::publicar_versao(&firestore, &agendada).await?;
    println!(
        "versões: {} agendou {} (antecedência {}, auto_update={}, e-mail={})",
        usuario.uid, agendada.version, payload.lead, payload.auto_update, payload.send_email
    );

    if !payload.send_email {
        versoes::registrar_emails(&firestore, &agendada.version, "skipped", 0).await?;
        return Ok(Json(versoes::obter(&firestore, &agendada.version).await?));
    }
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

fn quem(usuario: &FirebaseUser) -> String {
    usuario.email.clone().unwrap_or_else(|| usuario.uid.clone())
}

/// `GET /api/admin/coins/grants`: distribuições de fichas. Antes, credita as que já venceram.
async fn listar_distribuicoes(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Vec<Distribuicao>>, AppError> {
    exigir_admin(&usuario)?;
    if let Err(e) = distribuicoes::processar_vencidas(&firestore).await {
        eprintln!("fichas: falha ao conferir as agendadas ao listar: {e:?}");
    }
    Ok(Json(distribuicoes::listar(&firestore).await?))
}

/// `POST /api/admin/coins/grants`: nova distribuição, para agora (credita na hora) ou agendada.
async fn criar_distribuicao(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(entrada): Json<DistribuicaoEntrada>,
) -> Result<Json<Distribuicao>, AppError> {
    exigir_admin(&usuario)?;
    let criada = distribuicoes::criar(&firestore, entrada, &quem(&usuario)).await?;
    println!(
        "fichas: {} criou a distribuição {} ({} x {} fichas, {}, para {})",
        quem(&usuario), criada.id, criada.target, criada.amount, criada.reason, criada.scheduled_at
    );
    Ok(Json(criada))
}

/// `POST /api/admin/coins/grants/{id}/cancel`: cancela uma distribuição ainda agendada.
async fn cancelar_distribuicao(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
) -> Result<Json<Distribuicao>, AppError> {
    exigir_admin(&usuario)?;
    let cancelada = distribuicoes::cancelar(&firestore, &id).await?;
    println!("fichas: {} cancelou a distribuição {id}", quem(&usuario));
    Ok(Json(cancelada))
}

/// `POST /api/admin/coins/grants/{id}/retry`: roda de novo uma distribuição que falhou.
async fn repetir_distribuicao(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
) -> Result<Json<Distribuicao>, AppError> {
    exigir_admin(&usuario)?;
    println!("fichas: {} repetiu a distribuição {id}", quem(&usuario));
    Ok(Json(distribuicoes::repetir(&firestore, &id).await?))
}

/// `GET /api/admin/users`: todos os jogadores, com situação (online, ausente...) e moderação.
async fn listar_usuarios(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Vec<UsuarioAdmin>>, AppError> {
    exigir_admin(&usuario)?;
    Ok(Json(usuarios::listar(&firestore).await?))
}

/// `PUT /api/admin/users/{uid}/moderation`: banir, mutar, desativar o acesso ou tirar do chat
/// (só os campos enviados mudam).
async fn moderar_usuario(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(uid): Path<String>,
    Json(entrada): Json<ModeracaoEntrada>,
) -> Result<Json<Moderacao>, AppError> {
    exigir_admin(&usuario)?;
    Ok(Json(usuarios::moderar(&firestore, &uid, entrada, &usuario).await?))
}

/// `GET /api/admin/reports`: denúncias do chat, abertas primeiro.
async fn listar_denuncias(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Vec<Denuncia>>, AppError> {
    exigir_admin(&usuario)?;
    Ok(Json(denuncias::listar(&firestore).await?))
}

/// `POST /api/admin/reports/{id}/resolve`: marca a denúncia como resolvida, dispensada ou reaberta.
async fn resolver_denuncia(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
    Json(entrada): Json<Resolucao>,
) -> Result<Json<Denuncia>, AppError> {
    exigir_admin(&usuario)?;
    let r = denuncias::resolver(&firestore, &id, entrada, &quem(&usuario)).await?;
    println!("denúncias: {} marcou {id} como {}", quem(&usuario), r.status);
    Ok(Json(r))
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
