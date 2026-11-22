//! Perfil público do jogador: `publicProfiles/{uid}` (leitura de qualquer jogador logado, escrita
//! só pelo servidor — Security Rules), com o **nick** (`@handle`) e o **avatar**.
//!
//! - Nick único: `handles/{nick}` guarda o dono e é criado com `currentDocument.exists = false`,
//!   então dois jogadores nunca ficam com o mesmo. Todo jogador ganha um nick padrão (do nome);
//!   pode trocá-lo **uma vez** (`handleLocked` passa a `true`). Depois, só por pedido ao suporte:
//!   o pedido vai por e-mail e fica `handleRequest.status = "pending"` até a análise.
//! - Avatar: imagem pequena já recortada pelo Client (160x160, WebP/JPEG/PNG em data URL, até
//!   ~60 KB), gravada no próprio documento: aparece para os outros no chat e na sala.

use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::firestore::valor::{booleano, data, ler_booleano, ler_mapa, ler_texto, mapa, texto};
use crate::firestore::{Campos, Firestore};

/// Para onde vão os pedidos de troca de nick.
pub const SUPORTE: &str = "shmupx-support@googlegroups.com";
const AVATAR_MAXIMO: usize = 80_000;

#[derive(Debug, Serialize, PartialEq)]
pub struct PedidoDeNick {
    pub status: String,
    pub requested: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Perfil {
    pub handle: String,
    /// Já usou a troca livre: outra troca só pelo suporte.
    pub handle_locked: bool,
    pub handle_request: Option<PedidoDeNick>,
    pub avatar: Option<String>,
}

/// Nick: 3 a 20 caracteres, letras minúsculas, dígitos e `_`.
pub fn nick_valido(nick: &str) -> bool {
    (3..=20).contains(&nick.len()) && nick.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Nick sugerido a partir do nome (ou do e-mail): sem acento, minúsculas, só letras/dígitos/_.
pub fn sugestao(base: &str) -> String {
    let sem_acento: String = base
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            outro => outro,
        })
        .collect();
    let mut nick: String = sem_acento
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
        .take(16)
        .collect();
    if nick.len() < 3 {
        nick = format!("jogador{nick}");
    }
    nick
}

fn perfil_de(campos: &Campos) -> Option<Perfil> {
    let handle = ler_texto(campos, "handle")?;
    let handle_request = ler_mapa(campos, "handleRequest").and_then(|pedido| {
        Some(PedidoDeNick { status: ler_texto(&pedido, "status")?, requested: ler_texto(&pedido, "requested").unwrap_or_default() })
    });
    Some(Perfil {
        handle,
        handle_locked: ler_booleano(campos, "handleLocked").unwrap_or(false),
        handle_request,
        avatar: ler_texto(campos, "avatar"),
    })
}

fn caminho_perfil(uid: &str) -> String {
    format!("publicProfiles/{uid}")
}

fn criar_nick(fs: &Firestore, nick: &str, uid: &str) -> Value {
    let campos: Campos = [("uid".to_string(), texto(uid)), ("createdAt".to_string(), data(Utc::now()))].into_iter().collect();
    json!({ "update": { "name": fs.nome(&format!("handles/{nick}")), "fields": campos }, "currentDocument": { "exists": false } })
}

fn atualizar_perfil(fs: &Firestore, uid: &str, campos: Campos, mascara: &[&str]) -> Value {
    json!({ "update": { "name": fs.nome(&caminho_perfil(uid)), "fields": campos }, "updateMask": { "fieldPaths": mascara } })
}

/// Perfil público do jogador; na primeira vez, cria com um nick padrão livre (do nome ou e-mail).
pub async fn garantir(fs: &Firestore, uid: &str, nome: Option<&str>, email: Option<&str>) -> Result<Perfil, AppError> {
    if let Some(perfil) = fs.ler_documento(&caminho_perfil(uid)).await?.as_ref().and_then(perfil_de) {
        return Ok(perfil);
    }
    let base = sugestao(nome.filter(|n| !n.trim().is_empty()).or(email.and_then(|e| e.split('@').next())).unwrap_or("jogador"));
    for tentativa in 0..50 {
        let nick = if tentativa == 0 { base.clone() } else { format!("{}{}", &base[..base.len().min(16)], tentativa + 1) };
        let tx = fs.iniciar_transacao().await?;
        if let Some(dono) = fs.ler(&format!("handles/{nick}"), &tx).await? {
            fs.desfazer(&tx).await;
            if ler_texto(&dono, "uid").as_deref() == Some(uid) {
                break;
            }
            continue;
        }
        let campos: Campos = [
            ("handle".to_string(), texto(&nick)),
            ("handleLocked".to_string(), booleano(false)),
            ("updatedAt".to_string(), data(Utc::now())),
        ]
        .into_iter()
        .collect();
        let escritas = vec![criar_nick(fs, &nick, uid), atualizar_perfil(fs, uid, campos, &["handle", "handleLocked", "updatedAt"])];
        if fs.confirmar(&tx, escritas).await.is_ok() {
            break;
        }
    }
    fs.ler_documento(&caminho_perfil(uid))
        .await?
        .as_ref()
        .and_then(perfil_de)
        .ok_or_else(|| AppError::conflict("não foi possível criar o nick agora; tente de novo"))
}

/// O nick está livre (ou já é deste jogador)?
pub async fn disponivel(fs: &Firestore, nick: &str, uid: &str) -> Result<bool, AppError> {
    if !nick_valido(nick) {
        return Ok(false);
    }
    Ok(match fs.ler_documento(&format!("handles/{nick}")).await? {
        None => true,
        Some(dono) => ler_texto(&dono, "uid").as_deref() == Some(uid),
    })
}

/// A troca livre (uma vez): reserva o nick novo, libera o antigo e trava a troca.
pub async fn trocar_nick(fs: &Firestore, uid: &str, novo: &str) -> Result<Perfil, AppError> {
    if !nick_valido(novo) {
        return Err(AppError::bad_request("nick inválido: de 3 a 20 caracteres, letras minúsculas, números e _"));
    }
    let tx = fs.iniciar_transacao().await?;
    let atual = fs.ler(&caminho_perfil(uid), &tx).await?.as_ref().and_then(perfil_de);
    let Some(atual) = atual else {
        fs.desfazer(&tx).await;
        return Err(AppError::conflict("perfil ainda não criado; abra o perfil de novo"));
    };
    if atual.handle_locked {
        fs.desfazer(&tx).await;
        return Err(AppError::conflict("o nick já foi trocado uma vez; peça a troca ao suporte"));
    }
    if atual.handle == novo {
        fs.desfazer(&tx).await;
        return Err(AppError::bad_request("esse já é o seu nick"));
    }
    if fs.ler(&format!("handles/{novo}"), &tx).await?.is_some() {
        fs.desfazer(&tx).await;
        return Err(AppError::conflict("esse nick já está em uso"));
    }
    let campos: Campos = [
        ("handle".to_string(), texto(novo)),
        ("handleLocked".to_string(), booleano(true)),
        ("updatedAt".to_string(), data(Utc::now())),
    ]
    .into_iter()
    .collect();
    let escritas = vec![
        criar_nick(fs, novo, uid),
        json!({ "delete": fs.nome(&format!("handles/{}", atual.handle)) }),
        atualizar_perfil(fs, uid, campos, &["handle", "handleLocked", "updatedAt"]),
    ];
    fs.confirmar(&tx, escritas).await.map_err(|_| AppError::conflict("esse nick acabou de ser usado; escolha outro"))?;
    Ok(Perfil { handle: novo.to_string(), handle_locked: true, ..atual })
}

/// Marca o pedido de troca como aguardando análise (o e-mail ao suporte já saiu).
pub async fn registrar_pedido(fs: &Firestore, uid: &str, pedido: &str) -> Result<(), AppError> {
    let campos: Campos = [
        (
            "handleRequest".to_string(),
            mapa([("status".to_string(), texto("pending")), ("requested".to_string(), texto(pedido)), ("createdAt".to_string(), data(Utc::now()))].into_iter().collect()),
        ),
        ("updatedAt".to_string(), data(Utc::now())),
    ]
    .into_iter()
    .collect();
    fs.gravar(&caminho_perfil(uid), campos, &["handleRequest", "updatedAt"]).await
}

/// Avatar já recortado pelo Client: data URL de imagem, pequena.
pub fn avatar_valido(avatar: &str) -> bool {
    let tipos = ["data:image/webp;base64,", "data:image/jpeg;base64,", "data:image/png;base64,"];
    avatar.len() <= AVATAR_MAXIMO
        && tipos.iter().any(|t| avatar.starts_with(t))
        && avatar.split_once(',').is_some_and(|(_, b64)| !b64.is_empty() && b64.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b)))
}

pub async fn definir_avatar(fs: &Firestore, uid: &str, avatar: Option<&str>) -> Result<(), AppError> {
    let mut campos: Campos = [("updatedAt".to_string(), data(Utc::now()))].into_iter().collect();
    if let Some(avatar) = avatar {
        if !avatar_valido(avatar) {
            return Err(AppError::bad_request("avatar inválido: envie a imagem recortada (até ~60 KB)"));
        }
        campos.insert("avatar".to_string(), texto(avatar));
    }
    // Sem avatar no corpo: o campo sai da máscara com valor ausente, o que o apaga.
    fs.gravar(&caminho_perfil(uid), campos, &["avatar", "updatedAt"]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nick_e_sugestao() {
        assert!(nick_valido("ramonvalerio") && nick_valido("kw_2") && !nick_valido("ab") && !nick_valido("Ramon") && !nick_valido("a b c"));
        assert_eq!(sugestao("Ramon Valério"), "ramonvalerio");
        assert_eq!(sugestao("Ãé"), "jogadorae");
        assert!(nick_valido(&sugestao("Um Nome Muito Comprido Demais Para Nick")));
    }

    #[test]
    fn avatar_so_imagem_pequena() {
        assert!(avatar_valido("data:image/webp;base64,UklGRg=="));
        assert!(!avatar_valido("data:text/html;base64,PGh0bWw+"));
        assert!(!avatar_valido("https://evil.example/x.png"));
        assert!(!avatar_valido(&format!("data:image/png;base64,{}", "A".repeat(90_000))));
        assert!(!avatar_valido("data:image/png;base64,<script>"));
    }
}
