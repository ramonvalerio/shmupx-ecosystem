//! Instalador do ShmupX Client para uma versão: o Lab acha o build local (assinado pelo updater
//! do Tauri) e o publica em `www.shmupx.com/downloads/`, no mesmo bucket e CloudFront do site,
//! junto com o ShmupX Portable (um `.exe` só, sem instalação) gerado pelo mesmo `npm run bundle`.
//!
//! Credenciais da AWS vêm do OpenBao (`shmupx/infra/aws`), lidas na hora do envio e passadas só
//! ao processo do `aws`: nada é gravado em disco nem guardado pelo Lab.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub(crate) const BUCKET: &str = "shmupx-frontend-prod-364037626379";
const URL_DOWNLOADS: &str = "https://www.shmupx.com/downloads";

/// Pasta do `shmupx-client`: `SHMUPX_CLIENT_DIR`, senão (em dev) a vizinha do Lab no ecosystem.
pub(crate) fn pasta_client() -> Result<PathBuf, String> {
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
    let client = pasta_client()?;
    let raiz = client.parent().ok_or("pasta do shmupx-client sem pasta acima")?;
    let pasta = raiz.join("target/release/bundle/nsis");
    let exe = pasta.join(format!("ShmupX_{versao}_x64-setup.exe"));
    let sig = pasta.join(format!("ShmupX_{versao}_x64-setup.exe.sig"));
    Ok((exe, sig))
}

/// Saída do `scripts/make-portable.ps1`: o ShmupX Portable e o SHA-256 dele.
fn portatil_do_build(versao: &str) -> Result<(PathBuf, PathBuf), String> {
    if !versao_valida(versao) {
        return Err("versão inválida".into());
    }
    let client = pasta_client()?;
    let raiz = client.parent().ok_or("pasta do shmupx-client sem pasta acima")?;
    let exe = raiz.join(format!("target/release/bundle/portable/ShmupX-Portable-{versao}.exe"));
    let sha = exe.with_extension("exe.sha256");
    Ok((exe, sha))
}

#[derive(Debug, Serialize)]
pub struct InstaladorLocal {
    pub found: bool,
    pub signed: bool,
    pub file: String,
    pub size: Option<u64>,
    /// Modificação do `.exe` em ms desde 1970, para o admin conferir se é o build certo.
    pub built_at_ms: Option<u64>,
    /// O ShmupX Portable da mesma versão (com o `.sha256`) também está no build.
    pub portable_found: bool,
    pub portable_size: Option<u64>,
}

/// O que existe no build local para a versão (sem enviar nada).
#[tauri::command]
pub async fn instalador_local(versao: String) -> Result<InstaladorLocal, String> {
    let (exe, sig) = arquivos_do_build(&versao)?;
    let (portatil, sha) = portatil_do_build(&versao)?;
    let meta_portatil = std::fs::metadata(&portatil).ok().filter(|_| sha.is_file());
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
        portable_found: meta_portatil.is_some(),
        portable_size: meta_portatil.map(|m| m.len()),
    })
}

#[derive(Debug, Serialize)]
pub struct InstaladorPublicado {
    pub url: String,
    pub signature: String,
    pub size: i64,
    /// O ShmupX Portable enviado junto (vai ao backend em `PUT .../portable`).
    pub portable: PortatilPublicado,
}

#[derive(Debug, Serialize)]
pub struct PortatilPublicado {
    pub url: String,
    pub sha256: String,
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
pub(crate) struct CredenciaisAws {
    pub AWS_ACCESS_KEY_ID: String,
    pub AWS_SECRET_ACCESS_KEY: String,
}

pub(crate) async fn credenciais_aws() -> Result<CredenciaisAws, String> {
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

/// Envia o instalador assinado e o ShmupX Portable da versão ao S3 e devolve URLs, assinatura e
/// SHA-256 para o backend.
#[tauri::command]
pub async fn publicar_instalador(versao: String) -> Result<InstaladorPublicado, String> {
    let (exe, sig) = arquivos_do_build(&versao)?;
    let tamanho = std::fs::metadata(&exe)
        .map_err(|_| format!("instalador da versão {versao} não encontrado: gere o build do Client ({})", exe.display()))?
        .len();
    let assinatura = std::fs::read_to_string(&sig)
        .map_err(|_| format!("assinatura {} não encontrada: gere o build com a chave do updater", sig.display()))?;
    let (portatil, sha) = portatil_do_build(&versao)?;
    let tamanho_portatil = std::fs::metadata(&portatil)
        .map_err(|_| format!("ShmupX Portable da versão {versao} não encontrado: gere o instalador de novo ({})", portatil.display()))?
        .len();
    let sha256 = std::fs::read_to_string(&sha)
        .map_err(|_| format!("SHA-256 {} não encontrado: gere o instalador de novo", sha.display()))?
        .trim()
        .to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("SHA-256 inválido em {}", sha.display()));
    }

    let credenciais = credenciais_aws().await?;
    let nome = format!("ShmupX-Setup-{versao}.exe");
    enviar_ao_s3(&exe, &nome, &credenciais).await?;
    let nome_portatil = format!("ShmupX-Portable-{versao}.exe");
    enviar_ao_s3(&portatil, &nome_portatil, &credenciais).await?;
    Ok(InstaladorPublicado {
        url: format!("{URL_DOWNLOADS}/{nome}"),
        signature: assinatura.trim().to_string(),
        size: i64::try_from(tamanho).unwrap_or(i64::MAX),
        portable: PortatilPublicado {
            url: format!("{URL_DOWNLOADS}/{nome_portatil}"),
            sha256,
            size: i64::try_from(tamanho_portatil).unwrap_or(i64::MAX),
        },
    })
}

/// Um arquivo em `downloads/<nome>` no bucket do site.
async fn enviar_ao_s3(arquivo: &Path, nome: &str, credenciais: &CredenciaisAws) -> Result<(), String> {
    let saida = Command::new("aws")
        .arg("s3")
        .arg("cp")
        .arg(arquivo)
        .arg(format!("s3://{BUCKET}/downloads/{nome}"))
        .args(["--content-type", "application/octet-stream"])
        .args(["--content-disposition", &format!("attachment; filename=\"{nome}\"")])
        // Nome com a versão: o arquivo nunca muda, então pode ficar em cache.
        .args(["--cache-control", "public,max-age=31536000,immutable"])
        .arg("--only-show-errors")
        .env("AWS_ACCESS_KEY_ID", &credenciais.AWS_ACCESS_KEY_ID)
        .env("AWS_SECRET_ACCESS_KEY", &credenciais.AWS_SECRET_ACCESS_KEY)
        .env("AWS_DEFAULT_REGION", "us-east-1")
        .output()
        .await
        .map_err(|e| format!("AWS CLI (aws) indisponível: {e}"))?;
    if !saida.status.success() {
        return Err(format!("falha ao enviar {nome} ao S3: {}", String::from_utf8_lossy(&saida.stderr).trim()));
    }
    Ok(())
}

/// Endereço do backend de produção, embutido no Client quando o OpenBao não traz outro.
const BACKEND_PRODUCAO: &str = "https://shmupx-backend-230979440400.us-central1.run.app";
/// Evento com o andamento do build do instalador (uma linha da saída, ou o fim).
const EVENTO_BUILD: &str = "instalador://build";
/// Um build por vez: dois `tauri build` na mesma pasta brigam pelos mesmos arquivos.
static GERANDO: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize)]
struct BuildEvento {
    versao: String,
    /// `rodando` (com `linha`), `pronto` ou `falhou` (com `erro`).
    fase: &'static str,
    linha: Option<String>,
    erro: Option<String>,
}

/// Todas as chaves de um caminho do OpenBao (mount `shmupx`), sem gravar nada em disco.
async fn segredo(caminho: &str) -> Result<Vec<(String, String)>, String> {
    let saida = Command::new("bao")
        .args(["kv", "get", "-format=json", "-mount=shmupx", caminho])
        .env("BAO_ADDR", std::env::var("BAO_ADDR").unwrap_or_else(|_| "http://127.0.0.1:8200".into()))
        .output()
        .await
        .map_err(|e| format!("OpenBao (bao) indisponível: {e}"))?;
    if !saida.status.success() {
        return Err(format!("OpenBao recusou shmupx/{caminho}: {}", String::from_utf8_lossy(&saida.stderr).trim()));
    }
    let valor: serde_json::Value = serde_json::from_slice(&saida.stdout).map_err(|e| format!("resposta inesperada do OpenBao: {e}"))?;
    let dados = valor.pointer("/data/data").and_then(|d| d.as_object()).ok_or_else(|| format!("shmupx/{caminho} vazio no OpenBao"))?;
    Ok(dados.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect())
}

/// Troca a versão do Client (`tauri.conf.json` e o `[package]` do `Cargo.toml`): só a linha da versão muda.
fn ajustar_versao(client: &Path, versao: &str) -> Result<(), String> {
    fn trocar(arquivo: &Path, comeco: &str, nova: String) -> Result<(), String> {
        let texto = std::fs::read_to_string(arquivo).map_err(|e| format!("falha ao ler {}: {e}", arquivo.display()))?;
        let mut trocou = false;
        let linhas: Vec<String> = texto
            .split('\n')
            .map(|linha| {
                if !trocou && linha.trim_start().starts_with(comeco) {
                    trocou = true;
                    let recuo = &linha[..linha.len() - linha.trim_start().len()];
                    let fim = if linha.ends_with('\r') { "\r" } else { "" };
                    format!("{recuo}{nova}{fim}")
                } else {
                    linha.to_string()
                }
            })
            .collect();
        if !trocou {
            return Err(format!("versão não encontrada em {}", arquivo.display()));
        }
        std::fs::write(arquivo, linhas.join("\n")).map_err(|e| format!("falha ao gravar {}: {e}", arquivo.display()))
    }
    trocar(&client.join("src-tauri/tauri.conf.json"), "\"version\":", format!("\"version\": \"{versao}\","))?;
    trocar(&client.join("src-tauri/Cargo.toml"), "version = ", format!("version = \"{versao}\""))
}

/// Gera o instalador assinado da versão (`npm run bundle` no shmupx-client): ajusta a versão do
/// Client, lê do OpenBao a configuração (`shmupx/client`) e a chave do updater (`shmupx/updater`)
/// e roda o build, avisando cada linha em `instalador://build`. Volta logo; o fim chega no evento.
#[tauri::command]
pub async fn gerar_instalador(app: AppHandle, versao: String) -> Result<(), String> {
    if !versao_valida(&versao) {
        return Err("versão inválida".into());
    }
    let client = pasta_client()?;
    let chave = segredo("updater").await.map_err(|e| {
        format!("{e}. Guarde a chave do updater do Tauri em shmupx/updater (TAURI_SIGNING_PRIVATE_KEY e TAURI_SIGNING_PRIVATE_KEY_PASSWORD) para assinar o instalador.")
    })?;
    if !chave.iter().any(|(k, _)| k == "TAURI_SIGNING_PRIVATE_KEY") {
        return Err("shmupx/updater no OpenBao não tem TAURI_SIGNING_PRIVATE_KEY".into());
    }
    let mut ambiente = segredo("client").await?;
    ambiente.extend(chave);
    if GERANDO.swap(true, Ordering::SeqCst) {
        return Err("já há um instalador sendo gerado".into());
    }
    if let Err(e) = ajustar_versao(&client, &versao) {
        GERANDO.store(false, Ordering::SeqCst);
        return Err(e);
    }

    tauri::async_runtime::spawn(async move {
        let avisar = |fase: &'static str, linha: Option<String>, erro: Option<String>| {
            let _ = app.emit(EVENTO_BUILD, BuildEvento { versao: versao.clone(), fase, linha, erro });
        };
        let resultado = rodar_build(&client, &ambiente, |linha| avisar("rodando", Some(linha), None)).await;
        let resultado = resultado.and_then(|()| match arquivos_do_build(&versao) {
            Ok((exe, sig)) if exe.is_file() && sig.is_file() => Ok(()),
            Ok(_) => Err("o build terminou, mas o instalador assinado não apareceu na pasta do bundle".into()),
            Err(e) => Err(e),
        });
        let resultado = resultado.and_then(|()| match portatil_do_build(&versao) {
            Ok((exe, sha)) if exe.is_file() && sha.is_file() => Ok(()),
            Ok(_) => Err("o build terminou, mas o ShmupX Portable não apareceu em target/release/bundle/portable".into()),
            Err(e) => Err(e),
        });
        match resultado {
            Ok(()) => avisar("pronto", None, None),
            Err(erro) => avisar("falhou", None, Some(erro)),
        }
        GERANDO.store(false, Ordering::SeqCst);
    });
    Ok(())
}

/// `npm run bundle` com o ambiente de produção, repassando cada linha da saída.
async fn rodar_build(client: &Path, ambiente: &[(String, String)], mut linha: impl FnMut(String)) -> Result<(), String> {
    let mut comando = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "npm", "run", "bundle"]);
        c
    } else {
        let mut c = Command::new("npm");
        c.args(["run", "bundle"]);
        c
    };
    // O Lab roda dentro do próprio `npm run tauri dev`: não passa ao build do Client as variáveis
    // dessa sessão (npm, Tauri, Vite e Node do Lab), nem a pasta de programas do Lab no PATH.
    for (nome, _) in std::env::vars_os() {
        let texto = nome.to_string_lossy().to_uppercase();
        if ["TAURI_", "NPM_", "VITE_", "NODE_", "INIT_CWD"].iter().any(|prefixo| texto.starts_with(prefixo)) {
            comando.env_remove(&nome);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        let limpo: Vec<PathBuf> = std::env::split_paths(&path).filter(|p| !p.to_string_lossy().to_lowercase().contains("shmupx-lab")).collect();
        if let Ok(path) = std::env::join_paths(limpo) {
            comando.env("PATH", path);
        }
    }
    comando
        .current_dir(client)
        .envs(ambiente.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .env("VITE_SHMUPX_ENV", "prd")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if !ambiente.iter().any(|(k, _)| k == "SHMUPX_BACKEND_URL") {
        comando.env("SHMUPX_BACKEND_URL", BACKEND_PRODUCAO);
    }
    #[cfg(windows)]
    comando.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let mut filho = comando.spawn().map_err(|e| format!("não foi possível rodar o npm: {e}"))?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    for leitor in [filho.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>), filho.stderr.take().map(|s| Box::new(s) as _)]
        .into_iter()
        .flatten()
    {
        let tx = tx.clone();
        // Lê bytes, não `lines()`: o PowerShell 5.1 escreve na página de código do console (cp850),
        // e um "í" fora de UTF-8 fazia `next_line` falhar, o leitor parar e fechar o pipe — a próxima
        // escrita do build quebrava sem mensagem nenhuma.
        tokio::spawn(async move {
            let mut leitor = BufReader::new(leitor);
            let mut bytes = Vec::new();
            loop {
                bytes.clear();
                match leitor.read_until(b'\n', &mut bytes).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let _ = tx.send(String::from_utf8_lossy(&bytes).into_owned());
                    }
                }
            }
        });
    }
    drop(tx);
    // Saída inteira num log (para conferir depois) e, se falhar, as linhas que falam de erro.
    let caminho_log = std::env::temp_dir().join("shmupx-lab-instalador.log");
    let mut log = std::fs::File::create(&caminho_log).ok();
    let mut ultimas: Vec<String> = Vec::new();
    let mut erros: Vec<String> = Vec::new();
    while let Some(l) = rx.recv().await {
        let l = l.trim_end().to_string();
        if let Some(arquivo) = log.as_mut() {
            use std::io::Write;
            let _ = writeln!(arquivo, "{l}");
        }
        if l.is_empty() {
            continue;
        }
        if linha_de_erro(&l) && erros.len() < 40 {
            erros.push(l.clone());
        }
        ultimas.push(l.clone());
        if ultimas.len() > 12 {
            ultimas.remove(0);
        }
        linha(l);
    }
    let status = filho.wait().await.map_err(|e| format!("falha ao esperar o build: {e}"))?;
    if status.success() {
        return Ok(());
    }
    let motivo = if erros.is_empty() { ultimas } else { erros.into_iter().take(12).collect() };
    Err(format!("o build falhou:\n{}\n\nLog completo: {}", motivo.join("\n"), caminho_log.display()))
}

/// Linha da saída do build que explica a falha (TypeScript, npm, Rust, NSIS…), sem o eco do Tauri.
fn linha_de_erro(linha: &str) -> bool {
    let minusculas = linha.to_lowercase();
    let eco = minusculas.contains("beforebuildcommand") || minusculas.contains("looking up installed tauri packages");
    !eco && ["error", "failed", "err!", "panicked", "não encontrad", "exception"].iter().any(|termo| minusculas.contains(termo))
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

    #[test]
    fn ajusta_so_a_linha_da_versao() {
        let pasta = std::env::temp_dir().join(format!("shmupx-lab-versao-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(pasta.join("src-tauri")).unwrap();
        std::fs::write(pasta.join("src-tauri/tauri.conf.json"), "{\r\n  \"productName\": \"ShmupX\",\r\n  \"version\": \"0.1.9\",\r\n  \"x\": { \"version\": \"9\" }\r\n}\r\n").unwrap();
        std::fs::write(pasta.join("src-tauri/Cargo.toml"), "[package]\nname = \"shmupx-client\"\nversion = \"0.1.9\"\n\n[dependencies]\nlog = { version = \"0.4\" }\n").unwrap();
        ajustar_versao(&pasta, "0.1.10").unwrap();
        assert_eq!(std::fs::read_to_string(pasta.join("src-tauri/tauri.conf.json")).unwrap(), "{\r\n  \"productName\": \"ShmupX\",\r\n  \"version\": \"0.1.10\",\r\n  \"x\": { \"version\": \"9\" }\r\n}\r\n");
        assert_eq!(std::fs::read_to_string(pasta.join("src-tauri/Cargo.toml")).unwrap(), "[package]\nname = \"shmupx-client\"\nversion = \"0.1.10\"\n\n[dependencies]\nlog = { version = \"0.4\" }\n");
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
