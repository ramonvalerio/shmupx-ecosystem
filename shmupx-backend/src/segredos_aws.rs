//! Acesso à AWS sem chave guardada: segredos do Secrets Manager ao subir e credenciais para o SES.
//!
//! No Cloud Run não há chave da AWS: o serviço pega um ID token do Google no servidor de metadados,
//! troca por credenciais temporárias com `sts:AssumeRoleWithWebIdentity` (role criada no
//! shmupx-infra, `envs/backend-prod`) e assina as chamadas com SigV4. Ao subir, lê o segredo JSON
//! e cada chave vira uma variável de ambiente, sem sobrescrever as que já existem. Sem
//! `AWS_SECRETS_ROLE_ARN`/`AWS_SECRET_ID` (desenvolvimento local), nada acontece.
//!
//! [`SessaoAws`] guarda as credenciais temporárias em cache e renova antes de expirarem, para
//! quem precisa chamar a AWS durante a vida do processo (envio de e-mail pelo SES).

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const METADADOS_ID_TOKEN: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity";
const AUDIENCIA: &str = "sts.amazonaws.com";
/// Duração pedida ao STS (o máximo padrão da role é 1 h).
const DURACAO_SESSAO: Duration = Duration::from_secs(3600);
/// Renova as credenciais quando faltar menos que isto para expirarem.
const MARGEM_RENOVACAO: Duration = Duration::from_secs(300);

pub struct Configuracao {
    role_arn: String,
    secret_id: String,
    regiao: String,
}

impl Configuracao {
    pub fn do_ambiente() -> Option<Self> {
        let role_arn = role_do_ambiente()?;
        let secret_id = std::env::var("AWS_SECRET_ID").ok().filter(|v| !v.is_empty())?;
        let regiao = regiao_do_ambiente();
        Some(Self { role_arn, secret_id, regiao })
    }
}

fn role_do_ambiente() -> Option<String> {
    std::env::var("AWS_SECRETS_ROLE_ARN").ok().filter(|v| !v.trim().is_empty())
}

/// Região padrão das chamadas à AWS (`AWS_SECRETS_REGION`, padrão `us-east-1`).
pub fn regiao_do_ambiente() -> String {
    std::env::var("AWS_SECRETS_REGION").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| "us-east-1".to_string())
}

/// Credenciais temporárias devolvidas pelo STS.
#[derive(Clone)]
pub struct Credenciais {
    pub access_key: String,
    pub secret_key: String,
    pub token: String,
}

/// Sessão na AWS pela role do Cloud Run, com as credenciais temporárias em cache.
pub struct SessaoAws {
    http: reqwest::Client,
    role_arn: String,
    cache: tokio::sync::Mutex<Option<(Credenciais, Instant)>>,
}

impl SessaoAws {
    /// `None` quando `AWS_SECRETS_ROLE_ARN` não está definida (desenvolvimento local).
    pub fn do_ambiente() -> Option<Self> {
        Self::nova(role_do_ambiente()?).ok()
    }

    pub fn nova(role_arn: String) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("cliente HTTP: {e}"))?;
        Ok(Self { http, role_arn, cache: tokio::sync::Mutex::new(None) })
    }

    /// Credenciais válidas por pelo menos [`MARGEM_RENOVACAO`]; renova pelo STS quando preciso.
    pub async fn credenciais(&self) -> Result<Credenciais, String> {
        let mut cache = self.cache.lock().await;
        if let Some((cred, expira)) = cache.as_ref() {
            if Instant::now() + MARGEM_RENOVACAO < *expira {
                return Ok(cred.clone());
            }
        }
        let pedido_em = Instant::now();
        let id_token = id_token_do_google(&self.http).await?;
        let cred = assumir_role(&self.http, &self.role_arn, &id_token, DURACAO_SESSAO).await?;
        *cache = Some((cred.clone(), pedido_em + DURACAO_SESSAO));
        Ok(cred)
    }

    /// Esquece as credenciais em cache (ex.: a AWS recusou o token); a próxima chamada renova.
    pub async fn invalidar(&self) {
        *self.cache.lock().await = None;
    }
}

/// Busca o segredo e devolve os pares NOME=valor. Erro aqui impede o backend de subir.
pub async fn carregar(cfg: &Configuracao) -> Result<BTreeMap<String, String>, String> {
    let sessao = SessaoAws::nova(cfg.role_arn.clone())?;
    let cred = sessao.credenciais().await?;
    let texto = ler_segredo(&sessao.http, &cred, &cfg.regiao, &cfg.secret_id).await?;
    let valores: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&texto).map_err(|e| format!("o segredo não é um objeto JSON: {e}"))?;
    Ok(valores
        .into_iter()
        .filter_map(|(k, v)| match v {
            serde_json::Value::String(s) => Some((k, s)),
            serde_json::Value::Null => None,
            outro => Some((k, outro.to_string())),
        })
        .collect())
}

async fn id_token_do_google(http: &reqwest::Client) -> Result<String, String> {
    let resposta = http
        .get(METADADOS_ID_TOKEN)
        .query(&[("audience", AUDIENCIA), ("format", "full")])
        .header("Metadata-Flavor", "Google")
        .send()
        .await
        .map_err(|e| format!("servidor de metadados do Google: {e}"))?;
    if !resposta.status().is_success() {
        return Err(format!("servidor de metadados do Google respondeu {}", resposta.status()));
    }
    resposta.text().await.map_err(|e| format!("ID token: {e}"))
}

async fn assumir_role(
    http: &reqwest::Client,
    role_arn: &str,
    id_token: &str,
    duracao: Duration,
) -> Result<Credenciais, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Raiz {
        assume_role_with_web_identity_response: Resposta,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Resposta {
        assume_role_with_web_identity_result: Resultado,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Resultado {
        credentials: Cred,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Cred {
        access_key_id: String,
        secret_access_key: String,
        session_token: String,
    }

    let segundos = duracao.as_secs().to_string();
    let resposta = http
        .post("https://sts.amazonaws.com/")
        .header("Accept", "application/json")
        .form(&[
            ("Action", "AssumeRoleWithWebIdentity"),
            ("Version", "2011-06-15"),
            ("RoleArn", role_arn),
            ("RoleSessionName", "shmupx-backend"),
            ("DurationSeconds", segundos.as_str()),
            ("WebIdentityToken", id_token),
        ])
        .send()
        .await
        .map_err(|e| format!("AWS STS: {e}"))?;
    let status = resposta.status();
    let corpo = resposta.text().await.map_err(|e| format!("AWS STS: {e}"))?;
    if !status.is_success() {
        return Err(format!("AWS STS respondeu {status}: {corpo}"));
    }
    let raiz: Raiz = serde_json::from_str(&corpo).map_err(|e| format!("resposta do STS inesperada: {e}"))?;
    let c = raiz.assume_role_with_web_identity_response.assume_role_with_web_identity_result.credentials;
    Ok(Credenciais { access_key: c.access_key_id, secret_key: c.secret_access_key, token: c.session_token })
}

async fn ler_segredo(http: &reqwest::Client, cred: &Credenciais, regiao: &str, secret_id: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Resposta {
        #[serde(rename = "SecretString")]
        secret_string: Option<String>,
    }

    let host = format!("secretsmanager.{regiao}.amazonaws.com");
    let corpo = serde_json::json!({ "SecretId": secret_id }).to_string();
    let cabecalhos = cabecalhos_assinados(
        cred,
        regiao,
        "secretsmanager",
        &host,
        "/",
        "application/x-amz-json-1.1",
        &[("x-amz-target", "secretsmanager.GetSecretValue")],
        &corpo,
        chrono::Utc::now(),
    );
    let mut req = http.post(format!("https://{host}/")).body(corpo);
    for (nome, valor) in cabecalhos {
        req = req.header(nome, valor);
    }
    let resposta = req.send().await.map_err(|e| format!("AWS Secrets Manager: {e}"))?;
    let status = resposta.status();
    let texto = resposta.text().await.map_err(|e| format!("AWS Secrets Manager: {e}"))?;
    if !status.is_success() {
        return Err(format!("AWS Secrets Manager respondeu {status}: {texto}"));
    }
    let r: Resposta = serde_json::from_str(&texto).map_err(|e| format!("resposta do Secrets Manager inesperada: {e}"))?;
    r.secret_string.ok_or_else(|| "o segredo não tem SecretString (valor binário não é suportado)".to_string())
}

/// Cabeçalhos de um POST assinado com SigV4 usando credenciais temporárias: devolve
/// `content-type`, `x-amz-date`, `x-amz-security-token`, os `extras` e `authorization`
/// (o `host` entra na assinatura, e o próprio cliente HTTP o envia).
#[allow(clippy::too_many_arguments)]
pub fn cabecalhos_assinados(
    cred: &Credenciais,
    regiao: &str,
    servico: &str,
    host: &str,
    caminho: &str,
    tipo: &str,
    extras: &[(&str, &str)],
    corpo: &str,
    agora: chrono::DateTime<chrono::Utc>,
) -> Vec<(String, String)> {
    let amz_data = agora.format("%Y%m%dT%H%M%SZ").to_string();
    let mut cabecalhos = BTreeMap::new();
    cabecalhos.insert("content-type".to_string(), tipo.to_string());
    cabecalhos.insert("host".to_string(), host.to_string());
    cabecalhos.insert("x-amz-date".to_string(), amz_data.clone());
    cabecalhos.insert("x-amz-security-token".to_string(), cred.token.clone());
    for (nome, valor) in extras {
        cabecalhos.insert(nome.to_ascii_lowercase(), valor.to_string());
    }
    let autorizacao =
        autorizacao_sigv4(&cred.access_key, &cred.secret_key, regiao, servico, "POST", caminho, &cabecalhos, corpo, &amz_data);
    cabecalhos.remove("host");
    let mut saida: Vec<(String, String)> = cabecalhos.into_iter().collect();
    saida.push(("authorization".to_string(), autorizacao));
    saida
}

/// Cabeçalho `Authorization` AWS SigV4 (https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_sigv.html).
/// `cabecalhos` já em minúsculas (o `BTreeMap` mantém a ordem exigida); caminho sem query string.
#[allow(clippy::too_many_arguments)]
fn autorizacao_sigv4(
    access_key: &str,
    secret_key: &str,
    regiao: &str,
    servico: &str,
    metodo: &str,
    caminho: &str,
    cabecalhos: &BTreeMap<String, String>,
    corpo: &str,
    amz_data: &str,
) -> String {
    let dia = &amz_data[..8];
    let canonicos: String = cabecalhos.iter().map(|(k, v)| format!("{k}:{}\n", v.trim())).collect();
    let assinados = cabecalhos.keys().cloned().collect::<Vec<_>>().join(";");
    let requisicao_canonica = format!(
        "{metodo}\n{caminho}\n\n{canonicos}\n{assinados}\n{}",
        hex(&Sha256::digest(corpo.as_bytes()))
    );
    let escopo = format!("{dia}/{regiao}/{servico}/aws4_request");
    let texto_assinar = format!(
        "AWS4-HMAC-SHA256\n{amz_data}\n{escopo}\n{}",
        hex(&Sha256::digest(requisicao_canonica.as_bytes()))
    );
    let chave = [dia, regiao, servico, "aws4_request"]
        .iter()
        .fold(format!("AWS4{secret_key}").into_bytes(), |k, parte| hmac_sha256(&k, parte.as_bytes()));
    let assinatura = hex(&hmac_sha256(&chave, texto_assinar.as_bytes()));
    format!("AWS4-HMAC-SHA256 Credential={access_key}/{escopo}, SignedHeaders={assinados}, Signature={assinatura}")
}

fn hmac_sha256(chave: &[u8], dados: &[u8]) -> Vec<u8> {
    const BLOCO: usize = 64;
    let mut k = if chave.len() > BLOCO { Sha256::digest(chave).to_vec() } else { chave.to_vec() };
    k.resize(BLOCO, 0);
    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    let interno = Sha256::new().chain_update(&ipad).chain_update(dados).finalize();
    Sha256::new().chain_update(&opad).chain_update(interno).finalize().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn hmac_confere_com_rfc4231() {
        // RFC 4231, caso 2.
        let r = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(hex(&r), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    }

    #[test]
    fn sigv4_confere_com_a_suite_da_aws() {
        // Suíte oficial de testes do SigV4, caso "post-vanilla".
        let mut cabecalhos = BTreeMap::new();
        cabecalhos.insert("host".to_string(), "example.amazonaws.com".to_string());
        cabecalhos.insert("x-amz-date".to_string(), "20150830T123600Z".to_string());
        let autorizacao = autorizacao_sigv4(
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "service",
            "POST",
            "/",
            &cabecalhos,
            "",
            "20150830T123600Z",
        );
        assert_eq!(
            autorizacao,
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
             SignedHeaders=host;x-amz-date, \
             Signature=5da7c1a2acd57cee7505fc6676e4e544621c30862966e37dddb68e92efbe5d6b"
        );
    }

    #[test]
    fn cabecalhos_assinados_incluem_token_e_nao_o_host() {
        let cred = Credenciais { access_key: "AKID".into(), secret_key: "segredo".into(), token: "tok".into() };
        let agora = chrono::DateTime::parse_from_rfc3339("2026-11-19T08:00:00Z").unwrap().with_timezone(&chrono::Utc);
        let c = cabecalhos_assinados(&cred, "us-east-1", "ses", "email.us-east-1.amazonaws.com", "/v2/email/outbound-emails", "application/json", &[], "{}", agora);
        let nomes: Vec<&str> = c.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(nomes, ["content-type", "x-amz-date", "x-amz-security-token", "authorization"]);
        assert!(c[3].1.contains("/20261119/us-east-1/ses/aws4_request"));
        assert!(c[3].1.contains("SignedHeaders=content-type;host;x-amz-date;x-amz-security-token,"));
    }
}
