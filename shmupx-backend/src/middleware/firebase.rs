use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::error::AppError;
use crate::middleware::auth::extrair_bearer_token;

/// Chaves públicas que assinam os ID tokens do Firebase Authentication, em formato JWK.
const JWKS_URL: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";

/// Usado quando o Google não manda `Cache-Control: max-age` (ele costuma mandar ~6 h).
const CACHE_PADRAO: Duration = Duration::from_secs(3600);

/// Identidade de um jogador autenticado pelo Firebase. Os handlers a leem via
/// `Extension<FirebaseUser>`, nunca de um campo do corpo da requisição.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirebaseUser {
    pub uid: String,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    auth_time: u64,
    email: Option<String>,
}

/// Middleware: exige `Authorization: Bearer <ID token do Firebase>` e injeta `FirebaseUser`.
///
/// Segue a verificação documentada pelo Firebase para backends sem Admin SDK: RS256 com `kid`
/// entre as chaves públicas vigentes, `aud` = ID do projeto, `iss` =
/// `https://securetoken.google.com/<projeto>`, `exp`/`iat` válidos, `auth_time` no passado e
/// `sub` não vazio (o UID). Revogação de sessão não é checada aqui: exigiria o Admin SDK.
pub async fn exigir_usuario_firebase(mut req: Request, next: Next) -> Result<Response, AppError> {
    let usuario = verificar_requisicao(req.headers()).await?;
    req.extensions_mut().insert(usuario);
    Ok(next.run(req).await)
}

/// Valida o `Authorization: Bearer` da requisição e devolve a identidade Firebase.
pub async fn verificar_requisicao(headers: &HeaderMap) -> Result<FirebaseUser, AppError> {
    let header_value = headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok());
    let token = extrair_bearer_token(header_value)?;
    verificador()?.verificar(token).await
}

fn verificador() -> Result<&'static Verificador, AppError> {
    static VERIFICADOR: OnceLock<Option<Verificador>> = OnceLock::new();
    VERIFICADOR
        .get_or_init(|| std::env::var("FIREBASE_PROJECT_ID").ok().filter(|id| !id.is_empty()).map(Verificador::new))
        .as_ref()
        .ok_or_else(|| AppError::internal("FIREBASE_PROJECT_ID não configurada no servidor"))
}

struct Verificador {
    project_id: String,
    http: reqwest::Client,
    chaves: RwLock<Option<(HashMap<String, DecodingKey>, Instant)>>,
}

impl Verificador {
    fn new(project_id: String) -> Self {
        Self {
            project_id,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("cliente HTTP com configuração fixa"),
            chaves: RwLock::new(None),
        }
    }

    async fn verificar(&self, token: &str) -> Result<FirebaseUser, AppError> {
        let kid = jsonwebtoken::decode_header(token)
            .ok()
            .and_then(|header| header.kid)
            .ok_or_else(|| AppError::unauthorized("token inválido ou expirado"))?;
        let chave = self.chave(&kid).await?;
        validar_com_chave(token, &chave, &self.project_id, agora())
    }

    /// Chave do `kid`, renovando o cache quando expira ou quando o `kid` é desconhecido
    /// (o Google rotaciona as chaves; um token novo pode chegar antes do cache vencer).
    async fn chave(&self, kid: &str) -> Result<DecodingKey, AppError> {
        if let Some((chaves, validade)) = self.chaves.read().await.as_ref() {
            if Instant::now() < *validade {
                if let Some(chave) = chaves.get(kid) {
                    return Ok(chave.clone());
                }
            }
        }

        let mut cache = self.chaves.write().await;
        let (chaves, max_age) = self.baixar_chaves().await?;
        let chave = chaves.get(kid).cloned();
        *cache = Some((chaves, Instant::now() + max_age));
        chave.ok_or_else(|| AppError::unauthorized("token inválido ou expirado"))
    }

    async fn baixar_chaves(&self) -> Result<(HashMap<String, DecodingKey>, Duration), AppError> {
        let response = self
            .http
            .get(JWKS_URL)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| AppError::internal(format!("falha ao obter as chaves do Firebase: {e}")))?;
        let max_age = max_age(response.headers().get(reqwest::header::CACHE_CONTROL)).unwrap_or(CACHE_PADRAO);
        let jwks: JwkSet = response
            .json()
            .await
            .map_err(|e| AppError::internal(format!("chaves do Firebase em formato inesperado: {e}")))?;

        let chaves = jwks
            .keys
            .iter()
            .filter_map(|jwk| Some((jwk.common.key_id.clone()?, DecodingKey::from_jwk(jwk).ok()?)))
            .collect();
        Ok((chaves, max_age))
    }
}

fn max_age(header: Option<&reqwest::header::HeaderValue>) -> Option<Duration> {
    header?
        .to_str()
        .ok()?
        .split(',')
        .find_map(|diretiva| diretiva.trim().strip_prefix("max-age=")?.parse().ok())
        .map(Duration::from_secs)
}

fn agora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("relógio do sistema antes de 1970")
        .as_secs()
}

/// Núcleo puro da validação (chave e relógio recebidos), separado para os testes não
/// dependerem de rede nem das chaves reais do Google.
fn validar_com_chave(
    token: &str,
    chave: &DecodingKey,
    project_id: &str,
    agora: u64,
) -> Result<FirebaseUser, AppError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[project_id]);
    validation.set_issuer(&[format!("https://securetoken.google.com/{project_id}")]);
    validation.set_required_spec_claims(&["exp", "iat", "aud", "iss", "sub"]);

    let claims = jsonwebtoken::decode::<Claims>(token, chave, &validation)
        .map_err(|_| AppError::unauthorized("token inválido ou expirado"))?
        .claims;

    // 60 s de folga, a mesma que `Validation` aplica a `exp`, para relógios levemente adiantados.
    if claims.sub.is_empty() || claims.sub.len() > 128 || claims.auth_time > agora + 60 {
        return Err(AppError::unauthorized("token inválido ou expirado"));
    }

    Ok(FirebaseUser { uid: claims.sub, email: claims.email })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;

    // Par de chaves descartável, gerado só para estes testes; não assina nada real.
    const PRIVADA: &[u8] = include_bytes!("testdata/firebase_test_private.pem");
    const PUBLICA: &[u8] = include_bytes!("testdata/firebase_test_public.pem");
    const OUTRA_PUBLICA: &[u8] = include_bytes!("testdata/outra_chave_public.pem");
    const PROJETO: &str = "shmupx-teste";

    #[derive(Serialize)]
    struct ClaimsDeTeste<'a> {
        sub: &'a str,
        aud: &'a str,
        iss: String,
        iat: u64,
        exp: u64,
        auth_time: u64,
        email: &'a str,
    }

    fn claims(sub: &str) -> ClaimsDeTeste<'_> {
        let agora = agora();
        ClaimsDeTeste {
            sub,
            aud: PROJETO,
            iss: format!("https://securetoken.google.com/{PROJETO}"),
            iat: agora - 10,
            exp: agora + 3600,
            auth_time: agora - 10,
            email: "jogador@example.com",
        }
    }

    fn assinar(claims: &ClaimsDeTeste) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("chave-teste".to_string());
        encode(&header, claims, &EncodingKey::from_rsa_pem(PRIVADA).unwrap()).unwrap()
    }

    fn validar(token: &str) -> Result<FirebaseUser, AppError> {
        validar_com_chave(token, &DecodingKey::from_rsa_pem(PUBLICA).unwrap(), PROJETO, agora())
    }

    #[test]
    fn token_valido_extrai_uid_e_email() {
        let usuario = validar(&assinar(&claims("uid-abc123"))).unwrap();
        assert_eq!(usuario, FirebaseUser { uid: "uid-abc123".into(), email: Some("jogador@example.com".into()) });
    }

    #[test]
    fn assinatura_de_outra_chave_e_rejeitada() {
        let token = assinar(&claims("uid"));
        let outra = DecodingKey::from_rsa_pem(OUTRA_PUBLICA).unwrap();
        assert!(validar_com_chave(&token, &outra, PROJETO, agora()).is_err());
    }

    #[test]
    fn token_de_outro_projeto_e_rejeitado() {
        let mut c = claims("uid");
        c.aud = "outro-projeto";
        assert!(validar(&assinar(&c)).is_err());
    }

    #[test]
    fn emissor_errado_e_rejeitado() {
        let mut c = claims("uid");
        c.iss = "https://accounts.google.com".into();
        assert!(validar(&assinar(&c)).is_err());
    }

    #[test]
    fn token_expirado_e_rejeitado() {
        let mut c = claims("uid");
        c.exp = agora() - 3600;
        assert!(validar(&assinar(&c)).is_err());
    }

    #[test]
    fn auth_time_no_futuro_e_rejeitado() {
        let mut c = claims("uid");
        c.auth_time = agora() + 3600;
        assert!(validar(&assinar(&c)).is_err());
    }

    #[test]
    fn sub_vazio_e_rejeitado() {
        assert!(validar(&assinar(&claims(""))).is_err());
    }

    #[test]
    fn token_hs256_e_rejeitado() {
        // Um token do Supabase (HS256) não pode passar por um token Firebase.
        let token = encode(&Header::default(), &claims("uid"), &EncodingKey::from_secret(b"segredo")).unwrap();
        assert!(validar(&token).is_err());
    }

    #[test]
    fn max_age_e_lido_do_cache_control() {
        let header = reqwest::header::HeaderValue::from_static("public, max-age=19800, must-revalidate");
        assert_eq!(max_age(Some(&header)), Some(Duration::from_secs(19800)));
        assert_eq!(max_age(None), None);
    }
}
