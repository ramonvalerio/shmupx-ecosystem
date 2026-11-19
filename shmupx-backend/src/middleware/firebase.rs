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
    /// O Google confirmou a posse do e-mail. Só um e-mail verificado vale para a lista de admins.
    pub email_verified: bool,
}

impl FirebaseUser {
    /// Admin = e-mail verificado presente em `ADMIN_EMAILS` (separado por vírgulas, sem
    /// diferenciar maiúsculas). Sem a variável, ninguém é admin. A decisão é sempre do servidor.
    pub fn e_admin(&self) -> bool {
        let lista = std::env::var("ADMIN_EMAILS").unwrap_or_default();
        self.e_admin_em(&lista)
    }

    fn e_admin_em(&self, lista: &str) -> bool {
        let Some(email) = self.email.as_deref().filter(|_| self.email_verified) else {
            return false;
        };
        lista.split(',').map(str::trim).any(|admin| !admin.is_empty() && admin.eq_ignore_ascii_case(email))
    }
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    auth_time: u64,
    email: Option<String>,
    #[serde(default)]
    email_verified: bool,
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
        if emulador_de_login_ativo() {
            return validar_token_do_emulador(token, &self.project_id, agora());
        }
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

/// Ambiente de desenvolvimento: `FIREBASE_AUTH_EMULATOR_HOST` aponta para o emulador do
/// Authentication (Docker local), que emite ID tokens **sem assinatura**.
pub fn emulador_de_login_ativo() -> bool {
    std::env::var("FIREBASE_AUTH_EMULATOR_HOST").is_ok_and(|v| !v.trim().is_empty())
}

/// Recusa subir com o emulador de login num lugar que não é desenvolvimento: aceitar tokens sem
/// assinatura em produção deixaria qualquer um se passar por qualquer jogador ou admin.
pub fn conferir_ambiente_do_emulador() -> Result<(), String> {
    if !emulador_de_login_ativo() {
        return Ok(());
    }
    let projeto = std::env::var("FIREBASE_PROJECT_ID").unwrap_or_default();
    if !projeto.starts_with("demo-") {
        return Err(format!("FIREBASE_AUTH_EMULATOR_HOST só vale com um projeto demo-* (atual: {projeto:?})"));
    }
    if std::env::var("FIRESTORE_EMULATOR_HOST").is_err() {
        return Err("FIREBASE_AUTH_EMULATOR_HOST exige também FIRESTORE_EMULATOR_HOST (dev é todo no emulador)".into());
    }
    if std::env::var("K_SERVICE").is_ok() {
        return Err("emulador de login configurado no Cloud Run: recusado".into());
    }
    Ok(())
}

/// Token do emulador: `alg: none`, sem assinatura. Confere o resto como o Firebase faz (emissor,
/// público, validade, `sub`), para o fluxo de dev se comportar como o de produção.
fn validar_token_do_emulador(token: &str, project_id: &str, agora: u64) -> Result<FirebaseUser, AppError> {
    use base64::Engine;
    let invalido = || AppError::unauthorized("token inválido ou expirado");
    let mut partes = token.split('.');
    let (Some(cabecalho), Some(corpo)) = (partes.next(), partes.next()) else { return Err(invalido()) };
    let decodificar = |parte: &str| -> Result<serde_json::Value, AppError> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parte.trim_end_matches('=')).map_err(|_| invalido())?;
        serde_json::from_slice(&bytes).map_err(|_| invalido())
    };
    if decodificar(cabecalho)?.get("alg").and_then(|a| a.as_str()) != Some("none") {
        return Err(invalido());
    }
    let corpo = decodificar(corpo)?;
    let texto = |nome: &str| corpo.get(nome).and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let numero = |nome: &str| corpo.get(nome).and_then(|v| v.as_u64()).unwrap_or(0);
    let valido = texto("aud") == project_id
        && texto("iss") == format!("https://securetoken.google.com/{project_id}")
        && numero("exp") + 60 >= agora
        && numero("iat") <= agora + 60
        && !texto("sub").is_empty()
        && texto("sub").len() <= 128;
    if !valido {
        return Err(invalido());
    }
    Ok(FirebaseUser {
        uid: texto("sub"),
        email: corpo.get("email").and_then(|v| v.as_str()).map(str::to_string),
        email_verified: corpo.get("email_verified").and_then(|v| v.as_bool()).unwrap_or(false),
    })
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

    Ok(FirebaseUser { uid: claims.sub, email: claims.email, email_verified: claims.email_verified })
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
        email_verified: bool,
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
            email_verified: true,
        }
    }

    fn usuario(email: Option<&str>, verificado: bool) -> FirebaseUser {
        FirebaseUser { uid: "uid".into(), email: email.map(str::to_string), email_verified: verificado }
    }

    fn token_do_emulador(alg: &str, corpo: serde_json::Value) -> String {
        use base64::Engine;
        let b64 = |v: serde_json::Value| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v.to_string());
        format!("{}.{}.", b64(serde_json::json!({ "alg": alg, "typ": "JWT" })), b64(corpo))
    }

    #[test]
    fn token_do_emulador_valido_vira_usuario() {
        let corpo = serde_json::json!({
            "aud": "demo-shmupx", "iss": "https://securetoken.google.com/demo-shmupx",
            "iat": 1_000, "exp": 4_600, "sub": "uid-dev", "email": "eu@example.com", "email_verified": true
        });
        let usuario = validar_token_do_emulador(&token_do_emulador("none", corpo), "demo-shmupx", 2_000).unwrap();
        assert_eq!(usuario, FirebaseUser { uid: "uid-dev".into(), email: Some("eu@example.com".into()), email_verified: true });
    }

    #[test]
    fn token_do_emulador_errado_e_recusado() {
        let base = serde_json::json!({
            "aud": "demo-shmupx", "iss": "https://securetoken.google.com/demo-shmupx", "iat": 1_000, "exp": 4_600, "sub": "uid"
        });
        assert!(validar_token_do_emulador(&token_do_emulador("RS256", base.clone()), "demo-shmupx", 2_000).is_err(), "só alg none");
        assert!(validar_token_do_emulador(&token_do_emulador("none", base.clone()), "outro", 2_000).is_err(), "outro projeto");
        assert!(validar_token_do_emulador(&token_do_emulador("none", base.clone()), "demo-shmupx", 9_000).is_err(), "expirado");
        let mut sem_sub = base.clone();
        sem_sub["sub"] = serde_json::json!("");
        assert!(validar_token_do_emulador(&token_do_emulador("none", sem_sub), "demo-shmupx", 2_000).is_err());
        assert!(validar_token_do_emulador("lixo", "demo-shmupx", 2_000).is_err());
    }

    #[test]
    fn admin_exige_email_verificado_na_lista() {
        let lista = " Dono@Example.com , outro@example.com ";
        assert!(usuario(Some("dono@example.com"), true).e_admin_em(lista));
        assert!(usuario(Some("outro@example.com"), true).e_admin_em(lista));
        assert!(!usuario(Some("dono@example.com"), false).e_admin_em(lista));
        assert!(!usuario(Some("intruso@example.com"), true).e_admin_em(lista));
        assert!(!usuario(None, true).e_admin_em(lista));
    }

    #[test]
    fn lista_vazia_nao_tem_admin() {
        assert!(!usuario(Some("dono@example.com"), true).e_admin_em(""));
        assert!(!usuario(Some(""), true).e_admin_em(","));
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
        assert_eq!(
            usuario,
            FirebaseUser { uid: "uid-abc123".into(), email: Some("jogador@example.com".into()), email_verified: true }
        );
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
