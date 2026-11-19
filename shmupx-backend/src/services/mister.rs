//! Tokens do MiSTer FPGA (ADR 008): o agente `shmupx-mister` não tem como fazer login com
//! Google, então envia partidas com um token longo, ligado a uma conta.
//!
//! - O jogador logado no Client pede um token; o servidor gera, guarda só o **hash** em
//!   `misterTokens/{sha256}` e manda o token **por e-mail** para o endereço verificado da conta.
//!   O token nunca volta na resposta da API nem aparece na tela.
//! - O jogador cola o token no `shmupx.ini` do MiSTer (`token.<username> = sxm_...`).
//! - Nas rotas de partida, `Authorization: Bearer sxm_...` vale como o jogador dono do token.
//!   Revogar = `revoked: true` no documento.

use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::AppError;
use crate::firestore::valor::{booleano, data, ler_booleano, ler_texto, texto};
use crate::firestore::{Campos, Firestore};

/// Prefixo que distingue o token do MiSTer de um ID token do Firebase (JWT).
pub const PREFIXO: &str = "sxm_";

/// `sxm_` + 64 caracteres hexadecimais (dois UUID v4: 244 bits aleatórios).
pub fn gerar_token() -> String {
    format!("{PREFIXO}{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn e_token_do_mister(token: &str) -> bool {
    token.starts_with(PREFIXO)
}

fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn caminho(token: &str) -> String {
    format!("misterTokens/{}", hash(token))
}

/// Guarda o hash do token novo, ligado ao jogador.
pub async fn registrar(fs: &Firestore, uid: &str, token: &str) -> Result<(), AppError> {
    let campos: Campos = [
        ("ownerId".to_string(), texto(uid)),
        ("revoked".to_string(), booleano(false)),
        ("createdAt".to_string(), data(Utc::now())),
    ]
    .into_iter()
    .collect();
    fs.gravar(&caminho(token), campos, &["ownerId", "revoked", "createdAt"]).await
}

/// Invalida o token (ex.: o e-mail não pôde ser enviado).
pub async fn revogar(fs: &Firestore, token: &str) -> Result<(), AppError> {
    let campos: Campos = [("revoked".to_string(), booleano(true))].into_iter().collect();
    fs.gravar(&caminho(token), campos, &["revoked"]).await
}

/// Dono de um token válido e não revogado.
pub async fn dono(fs: &Firestore, token: &str) -> Result<Option<String>, AppError> {
    if !e_token_do_mister(token) || token.len() > 128 {
        return Ok(None);
    }
    let Some(documento) = fs.ler_documento(&caminho(token)).await? else {
        return Ok(None);
    };
    if ler_booleano(&documento, "revoked") == Some(true) {
        return Ok(None);
    }
    Ok(ler_texto(&documento, "ownerId"))
}

/// `ramonvalerios@gmail.com` -> `ra***@gmail.com`, para confirmar o destino sem expor o endereço.
pub fn mascarar_email(email: &str) -> String {
    match email.split_once('@') {
        Some((nome, dominio)) => format!("{}***@{dominio}", nome.chars().take(2).collect::<String>()),
        None => "***".to_string(),
    }
}

/// Assunto, HTML e texto do e-mail com o token.
pub fn mensagem(token: &str) -> (String, String, String) {
    let assunto = "Seu token do ShmupX para o MiSTer".to_string();
    let linha = format!("token.<seu username> = {token}");
    let html = format!(
        "<div style=\"font-family:system-ui,sans-serif;max-width:560px\">\
         <h2>Conectar o MiSTer ao ShmupX</h2>\
         <p>Copie a linha abaixo para o arquivo <code>/media/fat/shmupx/shmupx.ini</code> do seu MiSTer, \
         trocando <code>&lt;seu username&gt;</code> pelo seu nome na lista <code>username</code>:</p>\
         <pre style=\"background:#111;color:#7ff;padding:12px;border-radius:6px\">{linha}</pre>\
         <p>Com ele, as partidas que você jogar pelo menu ShmupX do MiSTer entram no seu perfil e no ranking, \
         como as do PC.</p>\
         <p style=\"color:#777;font-size:12px\">Não compartilhe este token: quem tiver ele joga em seu nome. \
         Se não foi você que pediu, ignore este e-mail.</p></div>",
        linha = linha.replace('<', "&lt;").replace('>', "&gt;")
    );
    let texto_puro = format!(
        "Conectar o MiSTer ao ShmupX\n\nCopie a linha abaixo para /media/fat/shmupx/shmupx.ini, trocando <seu username> \
         pelo seu nome na lista username:\n\n{linha}\n\nNão compartilhe este token: quem tiver ele joga em seu nome. \
         Se não foi você que pediu, ignore este e-mail.\n"
    );
    (assunto, html, texto_puro)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_tem_prefixo_e_e_unico() {
        let (a, b) = (gerar_token(), gerar_token());
        assert!(e_token_do_mister(&a));
        assert_eq!(a.len(), PREFIXO.len() + 64);
        assert_ne!(a, b);
        assert!(!e_token_do_mister("eyJhbGciOiJSUzI1NiJ9.x.y"));
    }

    #[test]
    fn so_o_hash_vira_caminho() {
        let token = gerar_token();
        let caminho = caminho(&token);
        assert!(caminho.starts_with("misterTokens/"));
        assert!(!caminho.contains(&token[PREFIXO.len()..]));
        assert_eq!(caminho.len(), "misterTokens/".len() + 64);
    }

    #[test]
    fn mascara_o_email() {
        assert_eq!(mascarar_email("ramonvalerios@gmail.com"), "ra***@gmail.com");
        assert_eq!(mascarar_email("x"), "***");
    }

    #[test]
    fn mensagem_traz_o_token_escapado_no_html() {
        let (_, html, texto) = mensagem("sxm_abc");
        assert!(html.contains("token.&lt;seu username&gt; = sxm_abc"));
        assert!(texto.contains("token.<seu username> = sxm_abc"));
    }
}
