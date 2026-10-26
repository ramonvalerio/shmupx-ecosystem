use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Deserialize)]
struct Claims {
    sub: Uuid,
}

/// Middleware de autenticação: exige um JWT válido do Supabase Auth
/// (`Authorization: Bearer <token>`), valida a assinatura com `SUPABASE_JWT_SECRET` e injeta o
/// `usuario_id` (claim `sub`) nas extensions da requisição — os handlers o leem via
/// `Extension<Uuid>` em vez de confiar num `usuario_id` solto no corpo do JSON, que qualquer
/// cliente poderia forjar para gastar fichas ou submeter runs em nome de outro jogador.
///
/// Só suporta HS256 (segredo compartilhado, igual ao `SUPABASE_JWT_SECRET` legado do painel do
/// Supabase). Projetos novos do Supabase podem assinar com RS256 (chave assimétrica, via JWKS)
/// — isso não é tratado aqui; ver TASKS.md.
pub async fn exigir_usuario_autenticado(
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let header_value = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    let token = extrair_bearer_token(header_value)?;
    let usuario_id = validar_token(token)?;

    req.extensions_mut().insert(usuario_id);

    Ok(next.run(req).await)
}

fn extrair_bearer_token(header_value: Option<&str>) -> Result<&str, AppError> {
    let header = header_value
        .ok_or_else(|| AppError::unauthorized("cabeçalho Authorization ausente"))?;

    header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::unauthorized("cabeçalho Authorization deve ser \"Bearer <token>\""))
}

fn validar_token(token: &str) -> Result<Uuid, AppError> {
    let secret = std::env::var("SUPABASE_JWT_SECRET").map_err(|_| {
        AppError::internal("SUPABASE_JWT_SECRET não configurada no servidor")
    })?;

    validar_token_com_segredo(token, &secret)
}

/// Núcleo puro da validação (sem ler env), separado só para os testes não precisarem mexer em
/// variável de ambiente global compartilhada entre testes rodando em paralelo.
fn validar_token_com_segredo(token: &str, secret: &str) -> Result<Uuid, AppError> {
    let decoding_key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::new(Algorithm::HS256);

    let token_data = jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|_| AppError::unauthorized("token inválido ou expirado"))?;

    Ok(token_data.claims.sub)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;
    use std::time::{SystemTime, UNIX_EPOCH};

    const SEGREDO_TESTE: &str = "segredo-de-teste-bem-longo-para-hs256-nao-usar-em-producao";

    #[derive(Serialize)]
    struct ClaimsDeTeste {
        sub: Uuid,
        exp: usize,
    }

    fn gerar_token(sub: Uuid, offset_segundos: i64, secret: &str) -> String {
        let agora = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("relógio do sistema antes de 1970")
            .as_secs() as i64;
        let exp = (agora + offset_segundos) as usize;

        encode(
            &Header::default(),
            &ClaimsDeTeste { sub, exp },
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("falha ao gerar token de teste")
    }

    #[test]
    fn token_valido_extrai_o_sub_correto() {
        let usuario_id = Uuid::new_v4();
        let token = gerar_token(usuario_id, 3600, SEGREDO_TESTE);

        let resultado = validar_token_com_segredo(&token, SEGREDO_TESTE);

        assert!(matches!(resultado, Ok(id) if id == usuario_id));
    }

    #[test]
    fn token_assinado_com_segredo_errado_e_rejeitado() {
        let token = gerar_token(Uuid::new_v4(), 3600, "um-segredo-completamente-diferente-123");

        assert!(validar_token_com_segredo(&token, SEGREDO_TESTE).is_err());
    }

    #[test]
    fn token_expirado_e_rejeitado() {
        let token = gerar_token(Uuid::new_v4(), -3600, SEGREDO_TESTE);

        assert!(validar_token_com_segredo(&token, SEGREDO_TESTE).is_err());
    }

    #[test]
    fn token_malformado_e_rejeitado() {
        assert!(validar_token_com_segredo("isto-nao-e-um-jwt", SEGREDO_TESTE).is_err());
    }

    #[test]
    fn sem_header_authorization_e_rejeitado() {
        assert!(extrair_bearer_token(None).is_err());
    }

    #[test]
    fn header_sem_prefixo_bearer_e_rejeitado() {
        assert!(extrair_bearer_token(Some("Token abc123")).is_err());
    }

    #[test]
    fn header_com_bearer_extrai_o_token() {
        assert_eq!(
            extrair_bearer_token(Some("Bearer meu.jwt.token")).unwrap(),
            "meu.jwt.token"
        );
    }
}
