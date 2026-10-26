use crate::error::AppError;

/// Extrai o token de `Authorization: Bearer <token>`. A validação do token em si fica em
/// `middleware::firebase` (Firebase Authentication substituiu o JWT HS256 do Supabase Auth).
pub(crate) fn extrair_bearer_token(header_value: Option<&str>) -> Result<&str, AppError> {
    let header = header_value
        .ok_or_else(|| AppError::unauthorized("cabeçalho Authorization ausente"))?;

    header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::unauthorized("cabeçalho Authorization deve ser \"Bearer <token>\""))
}

#[cfg(test)]
mod tests {
    use super::*;

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
