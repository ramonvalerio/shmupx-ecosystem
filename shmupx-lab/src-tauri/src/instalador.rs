//! Instalador do ShmupX Client para uma versão: o Lab acha o build local (assinado pelo updater
//! do Tauri) e o publica em `www.shmupx.com/downloads/`, no mesmo bucket e CloudFront do site.
//!
//! Credenciais da AWS vêm do OpenBao (`shmupx/infra/aws`), lidas na hora do envio e passadas só
//! ao processo do `aws`: nada é gravado em disco nem guardado pelo Lab.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::process::Command;

const BUCKET: &str = "shmupx-frontend-prod-364037626379";
const URL_DOWNLOADS: &str = "https://www.shmupx.com/downloads";

/// Pasta do `shmupx-client`: `SHMUPX_CLIENT_DIR`, senão (em dev) a vizinha do Lab no ecosystem.
fn pasta_client() -> Result<PathBuf, String> {
    let pasta = match std::env::var_os("SHMUPX_CLIENT_DIR") {
        Some(pasta) => PathBuf::from(pasta),
        None if cfg!(debug_assertions) => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shmupx-client"),
        None => return Err("defina SHMUPX_CLIENT_DIR com a pasta do shmupx-client".into()),
    };
    pasta.canonicalize().map_err(|e| format!("pasta do shmupx-client indisponível: {e}"))
}

fn versao_valida(versao: &str) -> bool {
    let partes: Vec<&str> = versao.split('.').collect();
    partes.len() == 3 && partes.iter().all(|p| !p.is_empty() && p.len() <= 4 && p.chars().all(|c| c.is_ascii_digit()))
}

/// Saída do `tauri build --bundles nsis` (com `createUpdaterArtifacts`): o `.exe` e o `.sig`.
fn arquivos_do_build(versao: &str) -> Result<(PathBuf, PathBuf), String> {
    if !versao_valida(versao) {
        return Err("versão inválida".into());
    }
    let pasta = pasta_client()?.join("src-tauri/target/release/bundle/nsis");
    let exe = pasta.join(format!("ShmupX_{versao}_x64-setup.exe"));
    let sig = pasta.join(format!("ShmupX_{versao}_x64-setup.exe.sig"));
    Ok((exe, sig))
}

#[derive(Debug, Serialize)]
pub struct InstaladorLocal {
    pub found: bool,
    pub signed: bool,
    pub file: String,
    pub size: Option<u64>,
    /// Modificação do `.exe` em ms desde 1970, para o admin conferir se é o build certo.
    pub built_at_ms: Option<u64>,
}

/// O que existe no build local para a versão (sem enviar nada).
#[tauri::command]
pub async fn instalador_local(versao: String) -> Result<InstaladorLocal, String> {
    let (exe, sig) = arquivos_do_build(&versao)?;
    let meta = std::fs::metadata(&exe).ok();
    let built_at_ms = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    Ok(InstaladorLocal {
        found: meta.is_some(),
        signed: sig.is_file(),
        file: exe.to_string_lossy().into_owned(),
        size: meta.map(|m| m.len()),
        built_at_ms,
    })
}

#[derive(Debug, Serialize)]
pub struct InstaladorPublicado {
    pub url: String,
    pub signature: String,
    pub size: i64,
}

#[derive(Deserialize)]
struct SegredoBao {
    data: SegredoDados,
}
#[derive(Deserialize)]
struct SegredoDados {
    data: CredenciaisAws,
}
#[derive(Deserialize)]
#[allow(non_snake_case)]
struct CredenciaisAws {
    AWS_ACCESS_KEY_ID: String,
    AWS_SECRET_ACCESS_KEY: String,
}

async fn credenciais_aws() -> Result<CredenciaisAws, String> {
    let saida = Command::new("bao")
        .args(["kv", "get", "-format=json", "-mount=shmupx", "infra/aws"])
        .env("BAO_ADDR", std::env::var("BAO_ADDR").unwrap_or_else(|_| "http://127.0.0.1:8200".into()))
        .output()
        .await
        .map_err(|e| format!("OpenBao (bao) indisponível: {e}"))?;
    if !saida.status.success() {
        return Err(format!("OpenBao recusou shmupx/infra/aws: {}", String::from_utf8_lossy(&saida.stderr).trim()));
    }
    let segredo: SegredoBao = serde_json::from_slice(&saida.stdout).map_err(|e| format!("resposta inesperada do OpenBao: {e}"))?;
    Ok(segredo.data.data)
}

/// Envia o instalador assinado da versão ao S3 e devolve URL e assinatura para o backend.
#[tauri::command]
pub async fn publicar_instalador(versao: String) -> Result<InstaladorPublicado, String> {
    let (exe, sig) = arquivos_do_build(&versao)?;
    let tamanho = std::fs::metadata(&exe)
        .map_err(|_| format!("instalador da versão {versao} não encontrado: gere o build do Client ({})", exe.display()))?
        .len();
    let assinatura = std::fs::read_to_string(&sig)
        .map_err(|_| format!("assinatura {} não encontrada: gere o build com a chave do updater", sig.display()))?;

    let nome = format!("ShmupX-Setup-{versao}.exe");
    let credenciais = credenciais_aws().await?;
    let saida = Command::new("aws")
        .arg("s3")
        .arg("cp")
        .arg(&exe)
        .arg(format!("s3://{BUCKET}/downloads/{nome}"))
        .args(["--content-type", "application/octet-stream"])
        .args(["--content-disposition", &format!("attachment; filename=\"{nome}\"")])
        // Nome com a versão: o arquivo nunca muda, então pode ficar em cache.
        .args(["--cache-control", "public,max-age=31536000,immutable"])
        .arg("--only-show-errors")
        .env("AWS_ACCESS_KEY_ID", credenciais.AWS_ACCESS_KEY_ID)
        .env("AWS_SECRET_ACCESS_KEY", credenciais.AWS_SECRET_ACCESS_KEY)
        .env("AWS_DEFAULT_REGION", "us-east-1")
        .output()
        .await
        .map_err(|e| format!("AWS CLI (aws) indisponível: {e}"))?;
    if !saida.status.success() {
        return Err(format!("falha ao enviar ao S3: {}", String::from_utf8_lossy(&saida.stderr).trim()));
    }
    Ok(InstaladorPublicado {
        url: format!("{URL_DOWNLOADS}/{nome}"),
        signature: assinatura.trim().to_string(),
        size: i64::try_from(tamanho).unwrap_or(i64::MAX),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_versoes_no_formato_certo() {
        assert!(versao_valida("0.1.1"));
        assert!(!versao_valida("0.1"));
        assert!(!versao_valida("../0.1.1"));
        assert!(!versao_valida("0.1.x"));
        assert!(arquivos_do_build("..\\x").is_err());
    }
}
