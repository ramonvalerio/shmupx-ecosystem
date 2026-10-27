use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Erro de borda da API: qualquer falha vira uma resposta HTTP limpa em JSON em vez de um
/// panic que derrubaria o processo inteiro (um handler que retorna `Err` nunca panica — axum
/// só chama `into_response` nele).
#[derive(Debug)]
pub enum AppError {
    /// Falha de banco (conexão, query, etc.) — sempre 500; o detalhe fica só no log do servidor.
    Database(sqlx::Error),
    /// Entrada inválida do cliente (payload malformado, regra de negócio violada) — 400.
    BadRequest(String),
    /// Recurso referenciado não existe (jogador, jogo, ficha ativa) — 404.
    NotFound(String),
    /// Estado válido mas em conflito com a regra de negócio (ex.: janela de envio expirada) — 409.
    Conflict(String),
    /// Identidade válida, mas sem permissão para a operação (ex.: rota de admin) — 403.
    Forbidden(String),
    /// Token ausente, malformado, com assinatura/expiração inválida — 401. O detalhe é seguro
    /// para o cliente ver (nunca contém o segredo nem o conteúdo do token).
    Unauthorized(String),
    /// Erro do nosso lado que não é culpa do cliente (ex.: variável de ambiente obrigatória
    /// ausente) — 500. Mensagem detalhada só no log do servidor, igual a `Database`.
    Internal(String),
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Database(err) => {
                eprintln!("erro de banco de dados: {err}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "falha ao consultar o banco de dados".to_string(),
                )
            }
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            AppError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            AppError::Conflict(message) => (StatusCode::CONFLICT, message),
            AppError::Forbidden(message) => (StatusCode::FORBIDDEN, message),
            AppError::Unauthorized(message) => (StatusCode::UNAUTHORIZED, message),
            AppError::Internal(message) => {
                eprintln!("erro interno: {message}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "erro interno do servidor".to_string(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
