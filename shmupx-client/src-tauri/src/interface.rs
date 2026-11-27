//! Interface atualizada sem instalador.
//!
//! A interface (o `dist` do Vite) vai embutida no executável, e mudar só a interface exigiria um
//! instalador novo. Aqui o Client baixa uma interface mais nova publicada para a mesma versão do
//! programa (`scripts/publicar-interface.ps1`) e passa a servi-la no lugar da embutida:
//!
//! - O pacote é um manifesto (`/downloads/ui/<versão>/manifest.json`) com o SHA-256 de cada arquivo,
//!   assinado com a chave do updater (a mesma verificação do `tauri-plugin-updater`, com a chave
//!   pública do `tauri.conf.json`). Os arquivos ficam em `/downloads/ui/files/<sha256>`: só os que
//!   mudaram são baixados, e cada um é conferido pelo hash do manifesto assinado.
//! - Só vale para a versão exata do programa (`native`) e se for mais nova que a embutida (`built`):
//!   a interface nunca chama um comando que este programa não tem, e o instalador seguinte, com a
//!   interface dele, passa na frente.
//! - A troca usa a mesma origem (`tauri.localhost`): login, salas abertas e preferências continuam.
//! - Baixada, ela entra no próximo início do Client ou quando a interface pedir (`interface_aplicar`,
//!   seguido de um recarregar). Uma interface que não confirma que abriu (`interface_confirmar`) em 30
//!   segundos ou em dois inícios seguidos volta para a embutida e não é usada de novo.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::utils::assets::{AssetKey, AssetsIter, CspHash};
use tauri::{App, AppHandle, Assets, Emitter, Manager, Wry};

const BASE: &str = "https://www.shmupx.com/downloads/ui";

/// Endereço das interfaces publicadas; `SHMUPX_INTERFACE_BASE` troca para testes (a assinatura
/// continua obrigatória, então outro servidor não consegue entregar uma interface).
fn base() -> String {
    std::env::var("SHMUPX_INTERFACE_BASE").ok().filter(|b| b.starts_with("http")).unwrap_or_else(|| BASE.to_string())
}
const VERSAO_EMBUTIDA: &str = "/ui-version.json";
const PRIMEIRA_CHECAGEM: Duration = Duration::from_secs(20);
const CHECAR_A_CADA: Duration = Duration::from_secs(30 * 60);
const PRAZO_CONFIRMAR: Duration = Duration::from_secs(30);
const INICIOS_SEM_CONFIRMAR: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifesto {
    /// Versão do programa para a qual a interface foi gerada.
    native: String,
    /// Quando foi gerada (UTC, `AAAA-MM-DDTHH:MM:SSZ`): ordena as interfaces.
    built: String,
    #[serde(default)]
    commit: Option<String>,
    files: BTreeMap<String, ArquivoManifesto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArquivoManifesto {
    sha256: String,
    size: u64,
}

/// O que fica em disco: o manifesto como veio (é ele que a assinatura cobre) e se já abriu bem.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Registro {
    manifesto: String,
    assinatura: String,
    #[serde(default)]
    confirmada: bool,
    #[serde(default)]
    tentativas: u32,
}

#[derive(Debug, Clone)]
enum Fonte {
    Embutida,
    Cache(PathBuf),
}

struct Ativa {
    built: String,
    arquivos: HashMap<String, Fonte>,
}

struct Global {
    embutida: Arc<dyn Assets<Wry>>,
    pasta: OnceLock<PathBuf>,
    chave: OnceLock<String>,
    versao_app: OnceLock<String>,
    ativa: RwLock<Option<Arc<Ativa>>>,
    hashes_embutidos: OnceLock<HashMap<String, String>>,
    trabalho: Mutex<()>,
}

static GLOBAL: OnceLock<Global> = OnceLock::new();

fn global() -> Option<&'static Global> {
    GLOBAL.get()
}

/// Fonte dos arquivos da janela: a interface baixada, se houver uma ativa, ou a embutida.
pub struct InterfaceAssets;

/// Troca a fonte dos arquivos do contexto do Tauri (antes do `build`).
pub fn instalar(context: &mut tauri::Context<Wry>) {
    let embutida: Arc<dyn Assets<Wry>> = Arc::from(context.set_assets(Box::new(InterfaceAssets)));
    let _ = GLOBAL.set(Global {
        embutida,
        pasta: OnceLock::new(),
        chave: OnceLock::new(),
        versao_app: OnceLock::new(),
        ativa: RwLock::new(None),
        hashes_embutidos: OnceLock::new(),
        trabalho: Mutex::new(()),
    });
}

impl Assets<Wry> for InterfaceAssets {
    fn setup(&self, app: &App<Wry>) {
        if let Some(g) = global() {
            g.embutida.setup(app);
        }
    }

    fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        let g = global()?;
        let fonte = g.ativa.read().ok().and_then(|ativa| ativa.as_ref().map(|a| a.arquivos.get(key.as_ref()).cloned()));
        match fonte {
            // sem interface baixada: a embutida
            None => g.embutida.get(key),
            Some(Some(Fonte::Embutida)) => g.embutida.get(key),
            Some(Some(Fonte::Cache(caminho))) => match std::fs::read(&caminho) {
                Ok(bytes) => Some(Cow::Owned(bytes)),
                Err(e) => {
                    log::warn!("interface: {} sumiu do cache ({e}); usando a embutida", caminho.display());
                    g.embutida.get(key)
                }
            },
            // a interface baixada não tem este arquivo
            Some(None) => None,
        }
    }

    fn iter(&self) -> Box<AssetsIter<'_>> {
        match global() {
            Some(g) => g.embutida.iter(),
            None => Box::new(std::iter::empty()),
        }
    }

    fn csp_hashes(&self, html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        match global() {
            Some(g) => g.embutida.csp_hashes(html_path),
            None => Box::new(std::iter::empty()),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn pasta() -> Option<&'static PathBuf> {
    global()?.pasta.get()
}

fn caminho_arquivo(sha: &str) -> Option<PathBuf> {
    Some(pasta()?.join("arquivos").join(sha))
}

fn ler_registro(nome: &str) -> Option<Registro> {
    let texto = std::fs::read_to_string(pasta()?.join(nome)).ok()?;
    serde_json::from_str(&texto).ok()
}

fn gravar_registro(nome: &str, registro: &Registro) -> std::io::Result<()> {
    let pasta = pasta().ok_or_else(|| std::io::Error::other("sem pasta"))?;
    std::fs::create_dir_all(pasta)?;
    let temporario = pasta.join(format!("{nome}.tmp"));
    std::fs::write(&temporario, serde_json::to_vec_pretty(registro).map_err(std::io::Error::other)?)?;
    std::fs::rename(&temporario, pasta.join(nome))
}

fn apagar_registro(nome: &str) {
    if let Some(pasta) = pasta() {
        let _ = std::fs::remove_file(pasta.join(nome));
    }
}

fn recusadas() -> Vec<String> {
    pasta()
        .and_then(|p| std::fs::read_to_string(p.join("recusadas.txt")).ok())
        .map(|t| t.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

fn recusar(built: &str) {
    if let Some(pasta) = pasta() {
        let mut lista = recusadas();
        if !lista.iter().any(|b| b == built) {
            lista.push(built.to_string());
            let _ = std::fs::write(pasta.join("recusadas.txt"), lista.join("\n"));
        }
    }
}

/// Quando a interface embutida foi gerada (`ui-version.json` do build); vazio nas antigas.
fn built_embutida() -> String {
    #[derive(Deserialize)]
    struct Versao {
        built: String,
    }
    global()
        .and_then(|g| g.embutida.get(&AssetKey::from(VERSAO_EMBUTIDA)))
        .and_then(|bytes| serde_json::from_slice::<Versao>(&bytes).ok())
        .map(|v| v.built)
        .unwrap_or_default()
}

fn hashes_embutidos() -> Option<&'static HashMap<String, String>> {
    let g = global()?;
    // `iter` devolve os bytes comprimidos (brotli); o hash é do arquivo, como no manifesto
    Some(g.hashes_embutidos.get_or_init(|| {
        let chaves: Vec<String> = g.embutida.iter().map(|(chave, _)| chave.into_owned()).collect();
        chaves
            .into_iter()
            .filter_map(|chave| {
                let bytes = g.embutida.get(&AssetKey::from(chave.as_str()))?;
                let hash = sha256(&bytes);
                Some((chave, hash))
            })
            .collect()
    }))
}

/// Confere a assinatura (minisign, chave do updater) e lê o manifesto.
fn verificar(manifesto: &str, assinatura: &str) -> Result<Manifesto, String> {
    let chave = global().and_then(|g| g.chave.get()).ok_or("sem a chave pública do updater")?;
    let b64 = base64::engine::general_purpose::STANDARD;
    let texto = |s: &str| -> Result<String, String> {
        let bytes = b64.decode(s.trim()).map_err(|e| format!("base64: {e}"))?;
        String::from_utf8(bytes).map_err(|e| format!("utf8: {e}"))
    };
    let publica = minisign_verify::PublicKey::decode(&texto(chave)?).map_err(|e| format!("chave: {e}"))?;
    let sig = minisign_verify::Signature::decode(&texto(assinatura)?).map_err(|e| format!("assinatura: {e}"))?;
    publica.verify(manifesto.as_bytes(), &sig, true).map_err(|e| format!("assinatura inválida: {e}"))?;
    serde_json::from_str(manifesto).map_err(|e| format!("manifesto: {e}"))
}

/// A interface serve para este programa e é mais nova que a embutida (e não foi recusada).
fn compativel(m: &Manifesto) -> Result<(), String> {
    let versao_app = global().and_then(|g| g.versao_app.get()).ok_or("sem versão do app")?;
    if &m.native != versao_app {
        return Err(format!("gerada para a versão {} (esta é a {versao_app})", m.native));
    }
    if m.built <= built_embutida() {
        return Err(format!("{} não é mais nova que a embutida", m.built));
    }
    if recusadas().contains(&m.built) {
        return Err(format!("{} foi recusada antes", m.built));
    }
    Ok(())
}

/// Monta a tabela de arquivos; só vale se todo arquivo que não está embutido já estiver no cache.
fn montar(m: &Manifesto) -> Result<Ativa, String> {
    let embutidos = hashes_embutidos().ok_or("sem a interface embutida")?;
    let mut arquivos = HashMap::with_capacity(m.files.len());
    for (chave, arquivo) in &m.files {
        let fonte = if embutidos.get(chave).is_some_and(|h| h == &arquivo.sha256) {
            Fonte::Embutida
        } else {
            let caminho = caminho_arquivo(&arquivo.sha256).ok_or("sem pasta")?;
            match std::fs::metadata(&caminho) {
                Ok(meta) if meta.len() == arquivo.size => Fonte::Cache(caminho),
                _ => return Err(format!("falta {chave} no cache")),
            }
        };
        arquivos.insert(chave.clone(), fonte);
    }
    if !arquivos.contains_key("/index.html") {
        return Err("manifesto sem /index.html".into());
    }
    Ok(Ativa { built: m.built.clone(), arquivos })
}

fn ativar(ativa: Option<Ativa>) {
    if let Some(g) = global() {
        if let Ok(mut atual) = g.ativa.write() {
            *atual = ativa.map(Arc::new);
        }
    }
}

fn built_ativa() -> Option<String> {
    global()?.ativa.read().ok()?.as_ref().map(|a| a.built.clone())
}

/// Início do Client: a baixada que esperava entra agora; a ativa que não abriu bem duas vezes sai.
pub fn iniciar(app: &AppHandle) {
    let Some(g) = global() else { return };
    let Ok(dados) = app.path().app_data_dir() else { return };
    let _ = g.pasta.set(dados.join("interface"));
    let _ = g.versao_app.set(app.package_info().version.to_string());
    if let Some(chave) = app
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
    {
        let _ = g.chave.set(chave.to_string());
    }
    // `npm run dev` (Vite) e builds de depuração: sempre a interface local
    if cfg!(debug_assertions) {
        return;
    }

    if let Some(pendente) = ler_registro("pendente.json") {
        apagar_registro("pendente.json");
        let nova = Registro { confirmada: false, tentativas: 0, ..pendente };
        if let Err(e) = gravar_registro("ativa.json", &nova) {
            log::warn!("interface: não deu para ativar a baixada: {e}");
        }
    }
    if let Some(mut registro) = ler_registro("ativa.json") {
        let resultado = verificar(&registro.manifesto, &registro.assinatura).and_then(|m| {
            compativel(&m)?;
            montar(&m)
        });
        match resultado {
            Ok(ativa) if !registro.confirmada && registro.tentativas >= INICIOS_SEM_CONFIRMAR => {
                log::warn!("interface: {} não abriu em {} inícios; volta a embutida", ativa.built, registro.tentativas);
                recusar(&ativa.built);
                apagar_registro("ativa.json");
            }
            Ok(ativa) => {
                if !registro.confirmada {
                    registro.tentativas += 1;
                    let _ = gravar_registro("ativa.json", &registro);
                }
                ativar(Some(ativa));
            }
            Err(e) => {
                // ex.: o instalador trouxe uma versão nova do programa
                log::info!("interface: a baixada não vale mais ({e})");
                apagar_registro("ativa.json");
            }
        }
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(PRIMEIRA_CHECAGEM).await;
        loop {
            if let Err(e) = checar(&app).await {
                log::info!("interface: sem atualização agora ({e})");
            }
            tokio::time::sleep(CHECAR_A_CADA).await;
        }
    });
}

async fn baixar_texto(cliente: &reqwest::Client, url: &str) -> Result<String, String> {
    let resposta = cliente.get(url).send().await.map_err(|e| e.to_string())?;
    if !resposta.status().is_success() {
        return Err(format!("HTTP {}", resposta.status()));
    }
    resposta.text().await.map_err(|e| e.to_string())
}

/// Procura uma interface mais nova; se houver, baixa o que falta e deixa pronta (`pendente.json`).
async fn checar(app: &AppHandle) -> Result<(), String> {
    let versao_app = global().and_then(|g| g.versao_app.get()).ok_or("sem versão do app")?.clone();
    let cliente = reqwest::Client::builder().timeout(Duration::from_secs(60)).build().map_err(|e| e.to_string())?;
    let agora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let base = base();
    let manifesto = baixar_texto(&cliente, &format!("{base}/{versao_app}/manifest.json?t={agora}")).await?;
    let assinatura = baixar_texto(&cliente, &format!("{base}/{versao_app}/manifest.json.sig?t={agora}")).await?;
    let m = verificar(&manifesto, &assinatura)?;
    compativel(&m)?;
    if built_ativa().is_some_and(|b| m.built <= b) {
        return Ok(());
    }
    if ler_registro("pendente.json").is_some_and(|p| p.manifesto == manifesto) {
        return Ok(());
    }

    // só os arquivos que nem a embutida nem o cache têm
    let embutidos = hashes_embutidos().ok_or("sem a interface embutida")?;
    let faltam: Vec<&ArquivoManifesto> = m
        .files
        .iter()
        .filter(|(chave, a)| !embutidos.get(*chave).is_some_and(|h| h == &a.sha256))
        .map(|(_, a)| a)
        .filter(|a| caminho_arquivo(&a.sha256).and_then(|c| std::fs::metadata(c).ok()).is_none_or(|meta| meta.len() != a.size))
        .collect();
    let pasta_arquivos = pasta().ok_or("sem pasta")?.join("arquivos");
    std::fs::create_dir_all(&pasta_arquivos).map_err(|e| e.to_string())?;
    for arquivo in faltam {
        if !arquivo.sha256.chars().all(|c| c.is_ascii_hexdigit()) || arquivo.sha256.len() != 64 {
            return Err(format!("hash inválido no manifesto: {}", arquivo.sha256));
        }
        let resposta = cliente
            .get(format!("{base}/files/{}", arquivo.sha256))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resposta.status().is_success() {
            return Err(format!("arquivo {}: HTTP {}", arquivo.sha256, resposta.status()));
        }
        let bytes = resposta.bytes().await.map_err(|e| e.to_string())?;
        if sha256(&bytes) != arquivo.sha256 {
            return Err(format!("arquivo {} não confere com o manifesto", arquivo.sha256));
        }
        let destino = pasta_arquivos.join(&arquivo.sha256);
        let temporario = pasta_arquivos.join(format!("{}.tmp", arquivo.sha256));
        std::fs::write(&temporario, &bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&temporario, &destino).map_err(|e| e.to_string())?;
    }
    montar(&m)?;
    gravar_registro("pendente.json", &Registro { manifesto, assinatura, confirmada: false, tentativas: 0 }).map_err(|e| e.to_string())?;
    let _ = app.emit("interface://pronta", serde_json::json!({ "versao": m.built }));
    Ok(())
}

/// A interface pede para usar a baixada agora: troca a fonte (a interface recarrega em seguida). Se a
/// nova não confirmar que abriu em 30 s, volta a embutida e recarrega de novo.
#[tauri::command]
pub async fn interface_aplicar(app: AppHandle) -> Result<String, String> {
    let _trava = global().ok_or("indisponível")?.trabalho.lock().map_err(|e| e.to_string())?;
    let pendente = ler_registro("pendente.json").ok_or("nenhuma interface nova baixada")?;
    let m = verificar(&pendente.manifesto, &pendente.assinatura)?;
    compativel(&m)?;
    let ativa = montar(&m)?;
    let registro = Registro { confirmada: false, tentativas: 0, ..pendente };
    gravar_registro("ativa.json", &registro).map_err(|e| e.to_string())?;
    apagar_registro("pendente.json");
    let built = ativa.built.clone();
    ativar(Some(ativa));

    let vigia = app.clone();
    let esperado = built.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(PRAZO_CONFIRMAR).await;
        let confirmada = ler_registro("ativa.json").is_some_and(|r| r.confirmada);
        if !confirmada && built_ativa().as_deref() == Some(esperado.as_str()) {
            log::warn!("interface: {esperado} não confirmou que abriu; volta a embutida");
            recusar(&esperado);
            apagar_registro("ativa.json");
            ativar(None);
            if let Some(janela) = vigia.get_webview_window("main") {
                let _ = janela.eval("location.reload()");
            }
        }
    });
    Ok(built)
}

/// A interface abriu bem: a baixada passa a valer de vez.
#[tauri::command]
pub fn interface_confirmar() {
    if let Some(mut registro) = ler_registro("ativa.json") {
        if !registro.confirmada {
            registro.confirmada = true;
            registro.tentativas = 0;
            let _ = gravar_registro("ativa.json", &registro);
        }
    }
}

#[derive(Serialize)]
pub struct VersaoInterface {
    embutida: String,
    ativa: Option<String>,
}

/// Qual interface está na tela (para o rodapé e o suporte).
#[tauri::command]
pub fn interface_versao() -> VersaoInterface {
    VersaoInterface { embutida: built_embutida(), ativa: built_ativa() }
}
