//! Envio dos avisos por e-mail pelo Amazon SES (API v2, `SendEmail`).
//!
//! Sem chave guardada: as credenciais vêm da mesma role da AWS usada para os segredos
//! (`AWS_SECRETS_ROLE_ARN`, ver `segredos_aws.rs`), renovadas antes de expirar. O remetente é
//! `EMAIL_FROM` (ex.: `ShmupX <noreply@shmupx.com>`, de um domínio verificado no SES; identidade e
//! DNS no shmupx-infra, `envs/backend-prod/ses.tf`). Região em `AWS_SES_REGION` (padrão: a mesma
//! dos segredos, `us-east-1`). Sem essa configuração, nada é enviado e o motivo fica registrado na
//! versão. Cada destinatário recebe o próprio e-mail, sem ver os endereços dos outros.

use std::time::Duration;

use serde_json::json;

use crate::segredos_aws::{cabecalhos_assinados, regiao_do_ambiente, SessaoAws};
use crate::services::versoes::Versao;
use crate::services::notificacoes::textos_da_versao;

const CAMINHO_ENVIO: &str = "/v2/email/outbound-emails";
/// Tentativas por destinatário quando o SES limita a taxa (HTTP 429) ou falha temporariamente.
const TENTATIVAS: u32 = 4;

pub struct Ses {
    http: reqwest::Client,
    sessao: SessaoAws,
    regiao: String,
    remetente: String,
}

impl Ses {
    /// `None` quando o envio não está configurado (sem `EMAIL_FROM` ou sem a role da AWS).
    pub fn do_ambiente() -> Option<Self> {
        let remetente = std::env::var("EMAIL_FROM").ok().filter(|v| !v.trim().is_empty())?;
        let sessao = SessaoAws::do_ambiente()?;
        let regiao = std::env::var("AWS_SES_REGION")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(regiao_do_ambiente);
        let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().ok()?;
        Some(Self { http, sessao, regiao, remetente })
    }

    /// Envia a todos; devolve quantos foram aceitos pelo SES.
    pub async fn enviar(&self, destinatarios: &[String], assunto: &str, html: &str, texto: &str) -> Result<usize, String> {
        let mut enviados = 0;
        for para in destinatarios {
            let corpo = corpo_do_envio(&self.remetente, para, assunto, html, texto);
            self.enviar_um(&corpo)
                .await
                .map_err(|e| format!("{e} ({enviados} enviados antes da falha)"))?;
            enviados += 1;
        }
        Ok(enviados)
    }

    async fn enviar_um(&self, corpo: &str) -> Result<(), String> {
        let host = format!("email.{}.amazonaws.com", self.regiao);
        let mut espera = Duration::from_millis(500);
        let mut tentativa = 1;
        loop {
            let cred = self.sessao.credenciais().await.map_err(|e| format!("credenciais da AWS: {e}"))?;
            let cabecalhos = cabecalhos_assinados(
                &cred,
                &self.regiao,
                "ses",
                &host,
                CAMINHO_ENVIO,
                "application/json",
                &[],
                corpo,
                chrono::Utc::now(),
            );
            let mut req = self.http.post(format!("https://{host}{CAMINHO_ENVIO}")).body(corpo.to_string());
            for (nome, valor) in cabecalhos {
                req = req.header(nome, valor);
            }
            let resposta = req.send().await.map_err(|e| format!("falha de rede com o SES: {e}"))?;
            let status = resposta.status();
            if status.is_success() {
                return Ok(());
            }
            let detalhe: String = resposta.text().await.unwrap_or_default().chars().take(300).collect();
            let token_recusado = status == reqwest::StatusCode::FORBIDDEN
                && (detalhe.contains("ExpiredToken") || detalhe.contains("security token"));
            let temporario = status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() || token_recusado;
            if token_recusado {
                self.sessao.invalidar().await;
            }
            if !temporario || tentativa >= TENTATIVAS {
                return Err(format!("SES respondeu HTTP {status}: {detalhe}"));
            }
            tokio::time::sleep(espera).await;
            espera *= 2;
            tentativa += 1;
        }
    }
}

/// JSON do `SendEmail` (SES v2) para um destinatário.
fn corpo_do_envio(remetente: &str, para: &str, assunto: &str, html: &str, texto: &str) -> String {
    json!({
        "FromEmailAddress": remetente,
        "Destination": { "ToAddresses": [para] },
        "Content": {
            "Simple": {
                "Subject": { "Data": assunto, "Charset": "UTF-8" },
                "Body": {
                    "Html": { "Data": html, "Charset": "UTF-8" },
                    "Text": { "Data": texto, "Charset": "UTF-8" }
                }
            }
        }
    })
    .to_string()
}

fn escapar(texto: &str) -> String {
    texto.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Assunto, HTML e texto puro do aviso de uma versão (o mesmo conteúdo da notificação do Client).
pub fn mensagem_da_versao(versao: &Versao) -> (String, String, String) {
    let (titulo, resumo) = textos_da_versao(versao);
    let secoes = [
        ("Novidades", &versao.features),
        ("Correções", &versao.fixes),
        ("Mudanças de regra", &versao.rules),
        ("Scripts de validação", &versao.scripts),
    ];
    let mut html = format!(
        "<div style=\"font-family:system-ui,sans-serif;max-width:560px\"><h2>{}</h2><p>{}</p>",
        escapar(&titulo),
        escapar(&resumo)
    );
    let mut texto = format!("{titulo}\n\n{resumo}\n");
    for (nome, itens) in secoes {
        if itens.is_empty() {
            continue;
        }
        html.push_str(&format!("<h3>{}</h3><ul>", escapar(nome)));
        texto.push_str(&format!("\n{nome}:\n"));
        for item in itens.iter() {
            html.push_str(&format!("<li>{}</li>", escapar(item)));
            texto.push_str(&format!("- {item}\n"));
        }
        html.push_str("</ul>");
    }
    let rodape = "Você recebe este aviso porque joga no ShmupX. Para parar de receber, desative os avisos por e-mail nas configurações do ShmupX Client.";
    html.push_str(&format!("<p style=\"color:#777;font-size:12px\">{}</p></div>", escapar(rodape)));
    texto.push_str(&format!("\n{rodape}\n"));
    (titulo, html, texto)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mensagem_escapa_o_conteudo_e_tem_as_secoes() {
        let versao = Versao {
            version: "0.2.0".into(),
            title: "Aero <Fighters> 2".into(),
            lead: Some("urgent".into()),
            features: vec!["Sala & ranking".into()],
            scripts: vec!["aero-fighters-2@0.2.0".into()],
            ..Default::default()
        };
        let (assunto, html, texto) = mensagem_da_versao(&versao);
        assert_eq!(assunto, "ShmupX Client 0.2.0: Aero <Fighters> 2");
        assert!(html.contains("Aero &lt;Fighters&gt; 2") && html.contains("Sala &amp; ranking"));
        assert!(!html.contains("Correções"), "seção vazia não aparece");
        assert!(texto.contains("Scripts de validação:\n- aero-fighters-2@0.2.0"));
        assert!(texto.contains("desative os avisos"));
    }

    #[test]
    fn corpo_do_envio_segue_o_formato_do_ses_v2() {
        let corpo: serde_json::Value =
            serde_json::from_str(&corpo_do_envio("ShmupX <noreply@shmupx.com>", "a@b.com", "Assunto", "<p>oi</p>", "oi")).unwrap();
        assert_eq!(corpo["FromEmailAddress"], "ShmupX <noreply@shmupx.com>");
        assert_eq!(corpo["Destination"]["ToAddresses"], serde_json::json!(["a@b.com"]));
        assert_eq!(corpo["Content"]["Simple"]["Subject"]["Data"], "Assunto");
        assert_eq!(corpo["Content"]["Simple"]["Body"]["Html"]["Data"], "<p>oi</p>");
        assert_eq!(corpo["Content"]["Simple"]["Body"]["Text"]["Charset"], "UTF-8");
    }
}
