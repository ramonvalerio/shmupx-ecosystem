//! Cliente mínimo da API REST do Cloud Firestore para o backend (ADR 005).
//!
//! O backend escreve o que as Security Rules proíbem ao cliente (fichas, gameplays). Credenciais:
//! - `FIRESTORE_EMULATOR_HOST` definido: fala com o emulador local, sem credencial real;
//! - senão, conta de serviço em `GOOGLE_APPLICATION_CREDENTIALS` (JSON fora do repositório),
//!   trocada por um access token OAuth 2.0 (fluxo JWT bearer) com escopo do Datastore.
//!
//! Acesso administrativo ignora as Rules: toda validação de escrita fica no backend.

pub mod valor;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::sync::Mutex;

use crate::error::AppError;

const ESCOPO: &str = "https://www.googleapis.com/auth/datastore";

pub type Campos = Map<String, Value>;

pub struct Firestore {
    http: reqwest::Client,
    /// `projects/<id>/databases/(default)/documents`
    raiz: String,
    base_url: String,
    credencial: Credencial,
}

enum Credencial {
    Emulador,
    ContaServico { conta: ContaServico, token: Mutex<Option<(String, u64)>> },
}

#[derive(Deserialize)]
struct ContaServico {
    client_email: String,
    private_key: String,
    token_uri: String,
}

impl Firestore {
    /// Lê a configuração do ambiente. `None` em `FIREBASE_PROJECT_ID` é erro de configuração.
    pub fn do_ambiente() -> Result<Self, String> {
        let projeto = std::env::var("FIREBASE_PROJECT_ID")
            .map_err(|_| "FIREBASE_PROJECT_ID não configurada".to_string())?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("falha ao construir cliente HTTP: {e}"))?;
        let raiz = format!("projects/{projeto}/databases/(default)/documents");

        if let Ok(host) = std::env::var("FIRESTORE_EMULATOR_HOST") {
            return Ok(Self { http, raiz, base_url: format!("http://{host}/v1"), credencial: Credencial::Emulador });
        }

        let caminho = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").map_err(|_| {
            "GOOGLE_APPLICATION_CREDENTIALS não configurada (caminho do JSON da conta de serviço)".to_string()
        })?;
        let conta: ContaServico = std::fs::read_to_string(&caminho)
            .map_err(|e| format!("falha ao ler a conta de serviço: {e}"))
            .and_then(|texto| serde_json::from_str(&texto).map_err(|e| format!("conta de serviço inválida: {e}")))?;
        Ok(Self {
            http,
            raiz,
            base_url: "https://firestore.googleapis.com/v1".to_string(),
            credencial: Credencial::ContaServico { conta, token: Mutex::new(None) },
        })
    }

    /// Nome completo de um documento, usado nas escritas: `projects/.../documents/<caminho>`.
    pub fn nome(&self, caminho: &str) -> String {
        format!("{}/{caminho}", self.raiz)
    }

    pub async fn iniciar_transacao(&self) -> Result<String, AppError> {
        #[derive(Deserialize)]
        struct Resposta {
            transaction: String,
        }
        let resposta: Resposta = self
            .post(&format!("{}:beginTransaction", self.raiz), &json!({ "options": { "readWrite": {} } }))
            .await?;
        Ok(resposta.transaction)
    }

    /// Lê um documento dentro da transação. `None` quando não existe.
    ///
    /// Usa `:batchGet`, como os SDKs oficiais: o `GET ?transaction=` trava no emulador.
    pub async fn ler(&self, caminho: &str, transacao: &str) -> Result<Option<Campos>, AppError> {
        #[derive(Deserialize)]
        struct Documento {
            #[serde(default)]
            fields: Campos,
        }
        #[derive(Deserialize)]
        struct Item {
            found: Option<Documento>,
        }
        let itens: Vec<Item> = self
            .post(
                &format!("{}:batchGet", self.raiz),
                &json!({ "documents": [self.nome(caminho)], "transaction": transacao }),
            )
            .await?;
        Ok(itens.into_iter().find_map(|item| item.found).map(|documento| documento.fields))
    }

    pub async fn confirmar(&self, transacao: &str, escritas: Vec<Value>) -> Result<(), AppError> {
        let _: Value = self
            .post(&format!("{}:commit", self.raiz), &json!({ "writes": escritas, "transaction": transacao }))
            .await?;
        Ok(())
    }

    /// Libera a transação quando a operação desiste antes do commit. Falha aqui é inofensiva:
    /// a transação expira sozinha no servidor.
    pub async fn desfazer(&self, transacao: &str) {
        let _: Result<Value, _> = self.post(&format!("{}:rollback", self.raiz), &json!({ "transaction": transacao })).await;
    }

    async fn post<T: for<'de> Deserialize<'de>>(&self, recurso: &str, corpo: &impl Serialize) -> Result<T, AppError> {
        let resposta = self
            .http
            .post(format!("{}/{recurso}", self.base_url))
            .bearer_auth(self.access_token().await?)
            .json(corpo)
            .send()
            .await
            .map_err(|e| AppError::internal(format!("falha de rede com o Firestore: {e}")))?;
        Self::corpo(resposta).await
    }

    async fn corpo<T: for<'de> Deserialize<'de>>(resposta: reqwest::Response) -> Result<T, AppError> {
        let status = resposta.status();
        // Contenção de transação (ABORTED) chega como 409.
        if status == reqwest::StatusCode::CONFLICT {
            return Err(AppError::conflict("operação concorrente; tente novamente"));
        }
        if !status.is_success() {
            let texto = resposta.text().await.unwrap_or_default();
            return Err(AppError::internal(format!("Firestore respondeu HTTP {status}: {texto}")));
        }
        resposta
            .json()
            .await
            .map_err(|e| AppError::internal(format!("resposta do Firestore em formato inesperado: {e}")))
    }

    async fn access_token(&self) -> Result<String, AppError> {
        let Credencial::ContaServico { conta, token } = &self.credencial else {
            // O emulador aceita este token fixo como administrador.
            return Ok("owner".to_string());
        };

        let mut cache = token.lock().await;
        let agora = agora();
        if let Some((valor, expira)) = cache.as_ref() {
            if agora + 60 < *expira {
                return Ok(valor.clone());
            }
        }

        #[derive(Serialize)]
        struct Asserção<'a> {
            iss: &'a str,
            scope: &'a str,
            aud: &'a str,
            iat: u64,
            exp: u64,
        }
        let chave = EncodingKey::from_rsa_pem(conta.private_key.as_bytes())
            .map_err(|e| AppError::internal(format!("chave da conta de serviço inválida: {e}")))?;
        let assercao = jsonwebtoken::encode(
            &Header::new(Algorithm::RS256),
            &Asserção { iss: &conta.client_email, scope: ESCOPO, aud: &conta.token_uri, iat: agora, exp: agora + 3600 },
            &chave,
        )
        .map_err(|e| AppError::internal(format!("falha ao assinar a asserção OAuth: {e}")))?;

        #[derive(Deserialize)]
        struct Resposta {
            access_token: String,
            expires_in: u64,
        }
        let resposta: Resposta = self
            .http
            .post(&conta.token_uri)
            .form(&[("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"), ("assertion", &assercao)])
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| AppError::internal(format!("falha ao autenticar a conta de serviço: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::internal(format!("resposta OAuth em formato inesperado: {e}")))?;

        *cache = Some((resposta.access_token.clone(), agora + resposta.expires_in));
        Ok(resposta.access_token)
    }
}

fn agora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("relógio do sistema antes de 1970").as_secs()
}
