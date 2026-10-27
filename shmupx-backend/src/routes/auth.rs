use std::sync::Arc;
use std::time::Duration;

use axum::routing::post;
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::middleware::firebase::FirebaseUser;

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Cliente OAuth "Desktop" do Google, com o segredo que só o servidor conhece.
struct Cliente {
    id: String,
    secret: String,
    /// Client de uma ferramenta interna (Lab): o login só é concluído para admins (`ADMIN_EMAILS`).
    so_admins: bool,
}

/// Clientes OAuth "Desktop" do Google: o do app do jogador (`GOOGLE_DESKTOP_*`) e, opcional, o
/// do Lab (`GOOGLE_LAB_*`). O código de login só se troca com o mesmo client que o iniciou, então
/// cada app informa o seu `client_id`. Os segredos ficam só no servidor: o app distribuído nunca
/// os carrega, e uma rotação não exige atualizar o app.
pub struct GoogleOAuth {
    jogador: Option<Cliente>,
    lab: Option<Cliente>,
    http: reqwest::Client,
}

impl GoogleOAuth {
    /// `None` quando o ambiente não configura nenhum cliente: a rota responde 500 com o motivo no log.
    pub fn do_ambiente() -> Option<Self> {
        let http = reqwest::Client::builder().timeout(Duration::from_secs(15)).build().ok()?;
        let jogador = cliente_do_ambiente("GOOGLE_DESKTOP_CLIENT_ID", "GOOGLE_DESKTOP_CLIENT_SECRET", false);
        let lab = cliente_do_ambiente("GOOGLE_LAB_CLIENT_ID", "GOOGLE_LAB_CLIENT_SECRET", true);
        if jogador.is_none() && lab.is_none() {
            return None;
        }
        Some(Self { jogador, lab, http })
    }

    /// Sem `client_id` no pedido vale o client do jogador (apps já distribuídos não o enviam).
    /// Um `client_id` desconhecido não é aceito.
    fn cliente(&self, client_id: Option<&str>) -> Option<&Cliente> {
        match client_id {
            None => self.jogador.as_ref(),
            Some(id) => [self.jogador.as_ref(), self.lab.as_ref()].into_iter().flatten().find(|c| c.id == id),
        }
    }
}

fn cliente_do_ambiente(variavel_id: &str, variavel_secret: &str, so_admins: bool) -> Option<Cliente> {
    // `trim`: segredos montados do Secret Manager costumam carregar uma quebra de linha final.
    let id = std::env::var(variavel_id).ok()?.trim().to_string();
    let secret = std::env::var(variavel_secret).ok()?.trim().to_string();
    (!id.is_empty() && !secret.is_empty()).then_some(Cliente { id, secret, so_admins })
}

/// Identidade declarada no `id_token` que o Google entregou direto no endpoint de token, por
/// TLS: nesse caminho o OIDC dispensa conferir a assinatura (o Firebase a confere depois).
fn usuario_do_id_token(id_token: &str) -> Option<FirebaseUser> {
    use base64::Engine;
    #[derive(Deserialize)]
    struct Claims {
        sub: String,
        email: Option<String>,
        #[serde(default)]
        email_verified: bool,
    }
    let payload = id_token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: Claims = serde_json::from_slice(&bytes).ok()?;
    Some(FirebaseUser { uid: claims.sub, email: claims.email, email_verified: claims.email_verified })
}

/// Rota pública: o login acontece antes de existir sessão. O `code` só vale com o
/// `code_verifier` gerado pelo app (PKCE), então quem não iniciou o login não consegue trocá-lo.
pub fn router<S>(google: Option<Arc<GoogleOAuth>>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/auth/google/token", post(trocar_codigo))
        .layer(Extension(google))
}

#[derive(Debug, Deserialize)]
struct TrocaPayload {
    code: String,
    code_verifier: String,
    redirect_uri: String,
    /// Client OAuth que iniciou o login (Lab); ausente = client do jogador.
    #[serde(default)]
    client_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct TrocaResposta {
    id_token: String,
}

async fn trocar_codigo(
    Extension(google): Extension<Option<Arc<GoogleOAuth>>>,
    Json(payload): Json<TrocaPayload>,
) -> Result<Json<TrocaResposta>, AppError> {
    let google = google.ok_or_else(|| {
        AppError::internal("nenhum client OAuth configurado (GOOGLE_DESKTOP_* ou GOOGLE_LAB_*)")
    })?;
    let cliente = google
        .cliente(payload.client_id.as_deref())
        .ok_or_else(|| AppError::bad_request("client_id desconhecido ou não configurado neste servidor"))?;
    if !redirect_loopback(&payload.redirect_uri) {
        return Err(AppError::bad_request("redirect_uri deve ser http://127.0.0.1:<porta>"));
    }
    if payload.code.is_empty() || payload.code_verifier.is_empty() {
        return Err(AppError::bad_request("code e code_verifier são obrigatórios"));
    }

    let resposta = google
        .http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", cliente.id.as_str()),
            ("client_secret", cliente.secret.as_str()),
            ("code", payload.code.as_str()),
            ("code_verifier", payload.code_verifier.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", payload.redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|e| AppError::internal(format!("falha de rede com o Google: {e}")))?;

    let status = resposta.status();
    if status.is_client_error() {
        // Código expirado, reutilizado ou verifier errado: culpa da requisição, não do servidor.
        let corpo = resposta.text().await.unwrap_or_default();
        eprintln!("Google recusou a troca do código (HTTP {status}): {corpo}");
        return Err(AppError::unauthorized("o Google recusou o código de login; tente entrar novamente"));
    }
    if !status.is_success() {
        return Err(AppError::internal(format!("Google respondeu HTTP {status} na troca do código")));
    }

    #[derive(Deserialize)]
    struct Token {
        id_token: Option<String>,
    }
    let token: Token = resposta
        .json()
        .await
        .map_err(|e| AppError::internal(format!("resposta do Google em formato inesperado: {e}")))?;
    let id_token = token.id_token.ok_or_else(|| AppError::internal("Google não devolveu id_token"))?;

    // Ferramenta interna: quem não é admin não recebe o token, então nem chega a ter sessão.
    if cliente.so_admins && !usuario_do_id_token(&id_token).is_some_and(|usuario| usuario.e_admin()) {
        return Err(AppError::forbidden("esta conta não é administradora; o Lab é restrito a admins"));
    }
    Ok(Json(TrocaResposta { id_token }))
}

/// Só o loopback do app (RFC 8252): impede usar esta rota como troca genérica para outros destinos.
fn redirect_loopback(uri: &str) -> bool {
    uri.strip_prefix("http://127.0.0.1:")
        .and_then(|porta| porta.parse::<u16>().ok())
        .is_some_and(|porta| porta != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn google(jogador: bool, lab: bool) -> GoogleOAuth {
        let cliente = |id: &str, so_admins| Cliente { id: id.into(), secret: "s".into(), so_admins };
        GoogleOAuth {
            jogador: jogador.then(|| cliente("jogador", false)),
            lab: lab.then(|| cliente("lab", true)),
            http: reqwest::Client::new(),
        }
    }

    #[test]
    fn client_sem_id_usa_o_do_jogador() {
        assert_eq!(google(true, true).cliente(None).map(|c| c.id.as_str()), Some("jogador"));
        assert!(google(false, true).cliente(None).is_none());
    }

    #[test]
    fn client_id_conhecido_e_resolvido_e_desconhecido_nao() {
        let g = google(true, true);
        assert_eq!(g.cliente(Some("lab")).map(|c| c.id.as_str()), Some("lab"));
        assert_eq!(g.cliente(Some("jogador")).map(|c| c.id.as_str()), Some("jogador"));
        assert!(g.cliente(Some("intruso")).is_none());
        assert!(google(true, false).cliente(Some("lab")).is_none());
    }

    #[test]
    fn identidade_vem_do_payload_do_id_token() {
        use base64::Engine;
        let payload = br#"{"sub":"uid-1","email":"adm@example.com","email_verified":true}"#;
        let token = format!("h.{}.s", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload));
        let usuario = usuario_do_id_token(&token).unwrap();
        assert_eq!(
            usuario,
            FirebaseUser { uid: "uid-1".into(), email: Some("adm@example.com".into()), email_verified: true }
        );
        assert!(usuario_do_id_token("sem-pontos").is_none());
        assert!(usuario_do_id_token("a.%%%.c").is_none());
    }

    #[test]
    fn aceita_so_loopback_com_porta() {
        assert!(redirect_loopback("http://127.0.0.1:58858"));
        assert!(!redirect_loopback("http://127.0.0.1:0"));
        assert!(!redirect_loopback("http://127.0.0.1:58858/cb"));
        assert!(!redirect_loopback("http://localhost:58858"));
        assert!(!redirect_loopback("https://evil.example/"));
        assert!(!redirect_loopback("http://127.0.0.1:58858@evil.example"));
    }
}
