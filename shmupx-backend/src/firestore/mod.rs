//! Cliente mínimo da API REST do Cloud Firestore para o backend (ADR 005).
//!
//! O backend escreve o que as Security Rules proíbem ao cliente (fichas, gameplays). Credenciais:
//! - `FIRESTORE_EMULATOR_HOST` definido: fala com o emulador local, sem credencial real;
//! - conta de serviço em `GOOGLE_APPLICATION_CREDENTIALS` (JSON fora do repositório),
//!   trocada por um access token OAuth 2.0 (fluxo JWT bearer) com escopo do Datastore;
//! - no Cloud Run, sem JSON: token da conta de serviço do serviço, pelo servidor de metadados.
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
    /// Cloud Run: a conta de serviço do próprio serviço, via servidor de metadados. Sem JSON.
    Metadados { token: Mutex<Option<(String, u64)>> },
}

const METADADOS_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

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

        let base_url = "https://firestore.googleapis.com/v1".to_string();
        let Ok(caminho) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") else {
            // `K_SERVICE` é definida pelo Cloud Run, que oferece o servidor de metadados.
            if std::env::var("K_SERVICE").is_ok() {
                return Ok(Self { http, raiz, base_url, credencial: Credencial::Metadados { token: Mutex::new(None) } });
            }
            return Err("GOOGLE_APPLICATION_CREDENTIALS não configurada (caminho do JSON da conta de serviço)".to_string());
        };
        let conta: ContaServico = std::fs::read_to_string(&caminho)
            .map_err(|e| format!("falha ao ler a conta de serviço: {e}"))
            .and_then(|texto| serde_json::from_str(&texto).map_err(|e| format!("conta de serviço inválida: {e}")))?;
        Ok(Self { http, raiz, base_url, credencial: Credencial::ContaServico { conta, token: Mutex::new(None) } })
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

    /// Grava só os campos da `mascara` do documento, criando-o se não existir. Sem transação:
    /// serve a escritas administrativas independentes, em que o último valor vence.
    pub async fn gravar(&self, caminho: &str, campos: Campos, mascara: &[&str]) -> Result<(), AppError> {
        let escrita = json!({
            "update": { "name": self.nome(caminho), "fields": campos },
            "updateMask": { "fieldPaths": mascara },
        });
        let _: Value = self.post(&format!("{}:commit", self.raiz), &json!({ "writes": [escrita] })).await?;
        Ok(())
    }

    /// Cria o documento só se ele ainda não existir (`currentDocument.exists = false`): duas
    /// criações com o mesmo caminho não se sobrescrevem; a segunda recebe `409`.
    pub async fn gravar_novo(&self, caminho: &str, campos: Campos) -> Result<(), AppError> {
        let escrita = json!({
            "update": { "name": self.nome(caminho), "fields": campos },
            "currentDocument": { "exists": false },
        });
        let _: Value = self.post(&format!("{}:commit", self.raiz), &json!({ "writes": [escrita] })).await?;
        Ok(())
    }

    /// Apaga o documento (as subcoleções, se houver, continuam). Apagar o que não existe não é erro.
    pub async fn apagar(&self, caminho: &str) -> Result<(), AppError> {
        let escrita = json!({ "delete": self.nome(caminho) });
        let _: Value = self.post(&format!("{}:commit", self.raiz), &json!({ "writes": [escrita] })).await?;
        Ok(())
    }

    /// Como [`Self::ler_documento`], mas com as datas de criação e da última escrita do documento.
    pub async fn ler_documento_com_datas(&self, caminho: &str) -> Result<Option<DocumentoComDatas>, AppError> {
        #[derive(Deserialize)]
        struct Item {
            found: Option<DocumentoBruto>,
        }
        let itens: Vec<Item> =
            self.post(&format!("{}:batchGet", self.raiz), &json!({ "documents": [self.nome(caminho)] })).await?;
        Ok(itens.into_iter().find_map(|item| item.found).map(DocumentoBruto::com_datas))
    }

    /// Lê um documento fora de transação (leitura simples, sem garantia de consistência com
    /// escritas concorrentes). `None` quando não existe.
    pub async fn ler_documento(&self, caminho: &str) -> Result<Option<Campos>, AppError> {
        #[derive(Deserialize)]
        struct Documento {
            #[serde(default)]
            fields: Campos,
        }
        #[derive(Deserialize)]
        struct Item {
            found: Option<Documento>,
        }
        let itens: Vec<Item> =
            self.post(&format!("{}:batchGet", self.raiz), &json!({ "documents": [self.nome(caminho)] })).await?;
        Ok(itens.into_iter().find_map(|item| item.found).map(|documento| documento.fields))
    }

    /// Acrescenta textos a uma lista do documento sem repetir os que já estão lá (`arrayUnion`),
    /// criando o documento se preciso. Atômico no servidor: duas chamadas juntas não se perdem.
    pub async fn acrescentar_na_lista(&self, caminho: &str, campo: &str, valores: &[String]) -> Result<(), AppError> {
        let escrita = json!({
            "transform": {
                "document": self.nome(caminho),
                "fieldTransforms": [{
                    "fieldPath": campo,
                    "appendMissingElements": { "values": valores.iter().map(|v| valor::texto(v)).collect::<Vec<_>>() },
                }],
            },
        });
        let _: Value = self.post(&format!("{}:commit", self.raiz), &json!({ "writes": [escrita] })).await?;
        Ok(())
    }

    /// Lista os documentos de uma coleção de primeiro nível, como `(id, campos)`.
    pub async fn listar(&self, colecao: &str) -> Result<Vec<(String, Campos)>, AppError> {
        Ok(self.listar_com_datas(colecao).await?.into_iter().map(|d| (d.id, d.campos)).collect())
    }

    /// Como [`Self::listar`], mas com as datas de criação e da última escrita de cada documento.
    pub async fn listar_com_datas(&self, colecao: &str) -> Result<Vec<DocumentoComDatas>, AppError> {
        #[derive(Deserialize)]
        struct Pagina {
            #[serde(default)]
            documents: Vec<DocumentoBruto>,
            #[serde(default, rename = "nextPageToken")]
            proxima: Option<String>,
        }

        // Teto de segurança: o catálogo é pequeno, e uma coleção enorme não deve travar a rota.
        const LIMITE: usize = 1000;
        let mut itens = Vec::new();
        let mut pagina: Option<String> = None;
        loop {
            let mut pedido = self
                .http
                .get(format!("{}/{}/{colecao}", self.base_url, self.raiz))
                .bearer_auth(self.access_token().await?)
                .query(&[("pageSize", "300")]);
            if let Some(token) = &pagina {
                pedido = pedido.query(&[("pageToken", token.as_str())]);
            }
            let resposta = pedido
                .send()
                .await
                .map_err(|e| AppError::internal(format!("falha de rede com o Firestore: {e}")))?;
            let lote: Pagina = Self::corpo(resposta).await?;
            itens.extend(lote.documents.into_iter().map(DocumentoBruto::com_datas));
            match lote.proxima {
                Some(token) if itens.len() < LIMITE => pagina = Some(token),
                _ => break,
            }
        }
        Ok(itens)
    }

    /// Consulta (`:runQuery`) fora de transação, com uma `structuredQuery` da API REST. Devolve os
    /// documentos encontrados como `(id, campos)`; o teto de documentos vem na própria consulta.
    pub async fn consultar(&self, consulta: Value) -> Result<Vec<(String, Campos)>, AppError> {
        Ok(self
            .consultar_caminhos(consulta)
            .await?
            .into_iter()
            .map(|(caminho, campos)| (caminho.rsplit('/').next().unwrap_or_default().to_string(), campos))
            .collect())
    }

    /// Como [`Self::consultar`], mas com o caminho relativo de cada documento (`rooms/x/presence/uid`):
    /// serve às consultas de grupo de coleções (`allDescendants`), em que o id sozinho não basta.
    pub async fn consultar_caminhos(&self, consulta: Value) -> Result<Vec<(String, Campos)>, AppError> {
        #[derive(Deserialize)]
        struct Documento {
            name: String,
            #[serde(default)]
            fields: Campos,
        }
        #[derive(Deserialize)]
        struct Item {
            document: Option<Documento>,
        }
        let itens: Vec<Item> =
            self.post(&format!("{}:runQuery", self.raiz), &json!({ "structuredQuery": consulta })).await?;
        let prefixo = format!("{}/", self.raiz);
        Ok(itens
            .into_iter()
            .filter_map(|item| item.document)
            .map(|d| (d.name.strip_prefix(&prefixo).unwrap_or(&d.name).to_string(), d.fields))
            .collect())
    }

    /// Lê vários documentos de uma vez (`:batchGet`), fora de transação. Devolve só os que
    /// existem, indexados pelo caminho relativo (`colecao/id`).
    pub async fn ler_varios(&self, caminhos: &[String]) -> Result<std::collections::HashMap<String, Campos>, AppError> {
        #[derive(Deserialize)]
        struct Documento {
            name: String,
            #[serde(default)]
            fields: Campos,
        }
        #[derive(Deserialize)]
        struct Item {
            found: Option<Documento>,
        }
        let mut lidos = std::collections::HashMap::new();
        let prefixo = format!("{}/", self.raiz);
        // O batchGet aceita lotes grandes, mas lotes menores mantêm cada resposta pequena.
        for lote in caminhos.chunks(100) {
            let nomes: Vec<String> = lote.iter().map(|c| self.nome(c)).collect();
            let itens: Vec<Item> = self.post(&format!("{}:batchGet", self.raiz), &json!({ "documents": nomes })).await?;
            for documento in itens.into_iter().filter_map(|item| item.found) {
                let caminho = documento.name.strip_prefix(&prefixo).unwrap_or(&documento.name).to_string();
                lidos.insert(caminho, documento.fields);
            }
        }
        Ok(lidos)
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
        let (conta, token) = match &self.credencial {
            // O emulador aceita este token fixo como administrador.
            Credencial::Emulador => return Ok("owner".to_string()),
            Credencial::Metadados { token } => return self.token_metadados(token).await,
            Credencial::ContaServico { conta, token } => (conta, token),
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

    async fn token_metadados(&self, token: &Mutex<Option<(String, u64)>>) -> Result<String, AppError> {
        let mut cache = token.lock().await;
        let agora = agora();
        if let Some((valor, expira)) = cache.as_ref() {
            if agora + 60 < *expira {
                return Ok(valor.clone());
            }
        }

        #[derive(Deserialize)]
        struct Resposta {
            access_token: String,
            expires_in: u64,
        }
        let resposta: Resposta = self
            .http
            .get(METADADOS_TOKEN_URL)
            .header("Metadata-Flavor", "Google")
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| AppError::internal(format!("falha ao obter token do servidor de metadados: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::internal(format!("token de metadados em formato inesperado: {e}")))?;

        *cache = Some((resposta.access_token.clone(), agora + resposta.expires_in));
        Ok(resposta.access_token)
    }
}

/// Documento lido com os metadados do Firestore: `criado_em`/`atualizado_em` em RFC 3339, como o
/// servidor devolve (`createTime`/`updateTime`); qualquer escrita no documento muda o segundo.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentoComDatas {
    pub id: String,
    pub campos: Campos,
    pub criado_em: Option<String>,
    pub atualizado_em: Option<String>,
}

#[derive(Deserialize)]
struct DocumentoBruto {
    name: String,
    #[serde(default)]
    fields: Campos,
    #[serde(default, rename = "createTime")]
    criado_em: Option<String>,
    #[serde(default, rename = "updateTime")]
    atualizado_em: Option<String>,
}

impl DocumentoBruto {
    fn com_datas(self) -> DocumentoComDatas {
        DocumentoComDatas {
            id: self.name.rsplit('/').next().unwrap_or_default().to_string(),
            campos: self.fields,
            criado_em: self.criado_em,
            atualizado_em: self.atualizado_em,
        }
    }
}

fn agora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("relógio do sistema antes de 1970").as_secs()
}
