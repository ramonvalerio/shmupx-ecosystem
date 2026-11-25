//! Denúncias do chat (`chatReports/{messageId}__{reporterUid}`), feitas pelo jogador no Client ou no
//! Lite: o Users Manager do Lab lista, abre o jogador denunciado para moderar e marca cada denúncia
//! como resolvida (houve ação) ou dispensada. Só o servidor lê as denúncias dos outros (Rules).

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::AppError;
use crate::firestore::valor::{data, ler_data, ler_texto, texto};
use crate::firestore::{Campos, Firestore};
use crate::services::gameplays::id_valido;

const COLECAO: &str = "chatReports";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Denuncia {
    pub id: String,
    pub reporter_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporter_name: Option<String>,
    pub author_id: String,
    pub author_name: String,
    pub room_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_title: Option<String>,
    pub message_id: String,
    pub text: String,
    /// `abuse`, `spam`, `inappropriate` ou `other`.
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    /// `open`, `actioned` (houve moderação) ou `dismissed`.
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolucao {
    /// `actioned`, `dismissed` ou `open` (reabrir).
    pub resolution: String,
}

fn denuncia_de(id: String, c: &Campos) -> Denuncia {
    Denuncia {
        reporter_id: ler_texto(c, "reporterId").unwrap_or_default(),
        reporter_name: None,
        author_id: ler_texto(c, "authorId").unwrap_or_default(),
        author_name: ler_texto(c, "authorName").unwrap_or_default(),
        room_id: ler_texto(c, "roomId").unwrap_or_default(),
        room_title: None,
        message_id: ler_texto(c, "messageId").unwrap_or_default(),
        text: ler_texto(c, "text").unwrap_or_default(),
        reason: ler_texto(c, "reason").unwrap_or_else(|| "other".into()),
        created_at: ler_data(c, "createdAt"),
        status: ler_texto(c, "status").filter(|s| ["actioned", "dismissed"].contains(&s.as_str())).unwrap_or_else(|| "open".into()),
        resolved_at: ler_data(c, "resolvedAt"),
        resolved_by: ler_texto(c, "resolvedBy"),
        id,
    }
}

/// Denúncias sem os nomes (para contar por jogador no Users Manager).
pub async fn todas(fs: &Firestore) -> Result<Vec<Denuncia>, AppError> {
    Ok(fs.listar(COLECAO).await?.into_iter().map(|(id, c)| denuncia_de(id, &c)).collect())
}

/// Denúncias abertas por jogador denunciado.
pub fn abertas_por_autor(denuncias: &[Denuncia]) -> HashMap<String, i64> {
    let mut contagem = HashMap::new();
    for d in denuncias.iter().filter(|d| d.status == "open") {
        *contagem.entry(d.author_id.clone()).or_insert(0) += 1;
    }
    contagem
}

/// Todas as denúncias, abertas primeiro e depois as mais recentes, com o nome de quem denunciou e da sala.
pub async fn listar(fs: &Firestore) -> Result<Vec<Denuncia>, AppError> {
    let (mut lista, jogos) = tokio::try_join!(
        todas(fs),
        fs.consultar(json!({ "from": [{ "collectionId": "games" }], "select": { "fields": [{ "fieldPath": "title" }] } })),
    )?;
    let titulos: HashMap<String, String> = jogos.into_iter().filter_map(|(id, c)| Some((id, ler_texto(&c, "title")?))).collect();
    let quem: BTreeSet<&String> = lista.iter().map(|d| &d.reporter_id).filter(|uid| id_valido(uid)).collect();
    let caminhos: Vec<String> = quem.iter().flat_map(|uid| [format!("publicProfiles/{uid}"), format!("users/{uid}")]).collect();
    let perfis = if caminhos.is_empty() { HashMap::new() } else { fs.ler_varios(&caminhos).await? };
    for d in &mut lista {
        d.room_title = titulos.get(&d.room_id).cloned();
        d.reporter_name = [format!("users/{}", d.reporter_id), format!("publicProfiles/{}", d.reporter_id)]
            .iter()
            .find_map(|c| perfis.get(c).and_then(|p| ler_texto(p, "displayName")));
    }
    lista.sort_by(|a, b| (a.status != "open").cmp(&(b.status != "open")).then(b.created_at.cmp(&a.created_at)));
    Ok(lista)
}

/// Marca a denúncia como resolvida, dispensada ou aberta de novo.
pub async fn resolver(fs: &Firestore, id: &str, entrada: Resolucao, admin: &str) -> Result<Denuncia, AppError> {
    if !id_valido(id) {
        return Err(AppError::bad_request("denúncia inválida"));
    }
    if !["actioned", "dismissed", "open"].contains(&entrada.resolution.as_str()) {
        return Err(AppError::bad_request("resolução inválida: use actioned, dismissed ou open"));
    }
    let caminho = format!("{COLECAO}/{id}");
    if fs.ler_documento(&caminho).await?.is_none() {
        return Err(AppError::not_found("denúncia não encontrada"));
    }
    let mut campos = Campos::new();
    campos.insert("status".into(), texto(&entrada.resolution));
    if entrada.resolution != "open" {
        campos.insert("resolvedAt".into(), data(Utc::now()));
        campos.insert("resolvedBy".into(), texto(admin));
    }
    // Reabrir apaga quem resolveu (campo na máscara sem valor).
    fs.gravar(&caminho, campos, &["status", "resolvedAt", "resolvedBy"]).await?;
    let c = fs.ler_documento(&caminho).await?.unwrap_or_default();
    Ok(denuncia_de(id.to_string(), &c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(autor: &str, status: &str) -> Denuncia {
        let mut c = Campos::new();
        c.insert("authorId".into(), texto(autor));
        c.insert("status".into(), texto(status));
        denuncia_de(format!("m__{autor}"), &c)
    }

    #[test]
    fn status_desconhecido_e_aberta() {
        assert_eq!(d("a", "qualquer").status, "open");
        assert_eq!(d("a", "dismissed").status, "dismissed");
        assert_eq!(denuncia_de("x".into(), &Campos::new()).status, "open");
    }

    #[test]
    fn conta_so_as_abertas_por_autor() {
        let lista = [d("a", "open"), d("a", "open"), d("a", "dismissed"), d("b", "actioned"), d("c", "")];
        let contagem = abertas_por_autor(&lista);
        assert_eq!(contagem.get("a"), Some(&2));
        assert_eq!(contagem.get("b"), None);
        assert_eq!(contagem.get("c"), Some(&1));
    }

    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn lista_e_resolve_no_emulador() {
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let (autor, quem) = (format!("autor-{}", uuid::Uuid::new_v4()), format!("quem-{}", uuid::Uuid::new_v4()));
        let mut perfil = Campos::new();
        perfil.insert("displayName".into(), texto("Quem Denunciou"));
        fs.gravar_novo(&format!("users/{quem}"), perfil).await.unwrap();
        let id = format!("msg1__{quem}");
        let mut c = Campos::new();
        for (k, v) in [("reporterId", quem.as_str()), ("authorId", autor.as_str()), ("authorName", "Spammer"), ("roomId", "sala-x"), ("messageId", "msg1"), ("text", "compre já"), ("reason", "spam")] {
            c.insert(k.into(), texto(v));
        }
        c.insert("createdAt".into(), data(Utc::now()));
        fs.gravar_novo(&format!("chatReports/{id}"), c).await.unwrap();

        let lista = listar(&fs).await.unwrap();
        let minha = lista.iter().find(|x| x.id == id).unwrap();
        assert_eq!((minha.status.as_str(), minha.reporter_name.as_deref()), ("open", Some("Quem Denunciou")));
        assert_eq!(abertas_por_autor(&lista).get(&autor), Some(&1));

        let r = resolver(&fs, &id, Resolucao { resolution: "dismissed".into() }, "admin@x").await.unwrap();
        assert_eq!((r.status.as_str(), r.resolved_by.as_deref()), ("dismissed", Some("admin@x")));
        let r = resolver(&fs, &id, Resolucao { resolution: "open".into() }, "admin@x").await.unwrap();
        assert_eq!((r.status.as_str(), r.resolved_by), ("open", None));
        assert!(resolver(&fs, &id, Resolucao { resolution: "apagar".into() }, "admin@x").await.is_err());
        assert!(resolver(&fs, "nao-existe", Resolucao { resolution: "dismissed".into() }, "admin@x").await.is_err());
    }
}
