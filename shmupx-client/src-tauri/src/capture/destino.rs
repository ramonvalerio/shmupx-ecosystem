//! Pasta de destino dos vídeos das runs (Configurações › Armazenamento). Padrão: `recordings`
//! na pasta onde o Client foi instalado (o instalador a cria com escrita liberada aos usuários);
//! se ela não aceitar escrita, e em desenvolvimento, `recordings` na pasta de dados do app. A escolha do jogador fica em `<pasta de dados>/gravacoes.json`; se a pasta escolhida
//! deixar de existir ou de aceitar escrita (disco removido), a gravação volta ao padrão.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Default)]
struct Config {
    pasta: Option<String>,
}

#[derive(Serialize)]
pub struct Destino {
    /// Pasta onde as próximas runs vão ser gravadas.
    pub pasta: String,
    /// É a pasta padrão (dados do app)?
    pub padrao: bool,
    /// Havia uma pasta escolhida, mas ela não está disponível agora (usando o padrão).
    pub indisponivel: Option<String>,
    /// Espaço do disco da pasta (bytes), quando o sistema informa.
    pub disco_livre: Option<u64>,
    pub disco_total: Option<u64>,
    /// Vídeos das runs (`shmupx_run_*.mp4`) já nessa pasta: quantos e quanto ocupam.
    pub gravacoes: u64,
    pub gravacoes_bytes: u64,
}

/// Disco que contém a pasta: o de ponto de montagem mais longo que é prefixo do caminho.
fn espaco_do_disco(pasta: &Path) -> (Option<u64>, Option<u64>) {
    let caminho = pasta.to_string_lossy().to_lowercase();
    let discos = sysinfo::Disks::new_with_refreshed_list();
    discos
        .list()
        .iter()
        .filter(|d| caminho.starts_with(&d.mount_point().to_string_lossy().to_lowercase()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| (Some(d.available_space()), Some(d.total_space())))
        .unwrap_or((None, None))
}

fn gravacoes_na_pasta(pasta: &Path) -> (u64, u64) {
    let Ok(entradas) = std::fs::read_dir(pasta) else { return (0, 0) };
    entradas
        .flatten()
        .filter(|e| { let nome = e.file_name().to_string_lossy().into_owned(); nome.starts_with("shmupx_run_") && nome.ends_with(".mp4") })
        .filter_map(|e| e.metadata().ok())
        .fold((0, 0), |(qtd, bytes), m| (qtd + 1, bytes + m.len()))
}

fn destino(pasta: String, padrao: bool, indisponivel: Option<String>) -> Destino {
    let caminho = PathBuf::from(&pasta);
    let (disco_livre, disco_total) = espaco_do_disco(&caminho);
    let (gravacoes, gravacoes_bytes) = gravacoes_na_pasta(&caminho);
    Destino { pasta, padrao, indisponivel, disco_livre, disco_total, gravacoes, gravacoes_bytes }
}

fn pasta_de_dados(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))
}

fn arquivo(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(pasta_de_dados(app)?.join("gravacoes.json"))
}

fn ler_config(app: &AppHandle) -> Config {
    arquivo(app).ok().and_then(|a| std::fs::read(a).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// A pasta aceita gravar? Cria (se faltar) e escreve um arquivo de teste.
fn gravavel(pasta: &Path) -> bool {
    if std::fs::create_dir_all(pasta).is_err() {
        return false;
    }
    let teste = pasta.join(".shmupx-teste");
    let ok = std::fs::write(&teste, b"ok").is_ok();
    let _ = std::fs::remove_file(teste);
    ok
}

/// Pasta padrão: `recordings` ao lado do Client instalado (junto de `validator`, `games` e
/// `ffmpeg`); se não der para gravar nela, ou em desenvolvimento, a dos dados do app.
fn pasta_padrao(app: &AppHandle) -> Result<PathBuf, String> {
    if !cfg!(debug_assertions) {
        if let Some(instalacao) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)) {
            let pasta = instalacao.join("recordings");
            if gravavel(&pasta) {
                return Ok(pasta);
            }
        }
    }
    Ok(pasta_de_dados(app)?.join("recordings"))
}

/// Destino atual (a escolha do jogador, se disponível; senão a pasta padrão).
pub fn atual(app: &AppHandle) -> Result<Destino, String> {
    let padrao = pasta_padrao(app)?;
    std::fs::create_dir_all(&padrao).map_err(|e| e.to_string())?;
    if let Some(escolhida) = ler_config(app).pasta {
        let caminho = PathBuf::from(&escolhida);
        if caminho.is_absolute() && gravavel(&caminho) {
            return Ok(destino(escolhida, false, None));
        }
        return Ok(destino(padrao.to_string_lossy().into_owned(), true, Some(escolhida)));
    }
    Ok(destino(padrao.to_string_lossy().into_owned(), true, None))
}

/// Pasta onde a próxima run grava o vídeo.
pub fn pasta_das_gravacoes(app: &AppHandle) -> Result<PathBuf, String> {
    let pasta = PathBuf::from(atual(app)?.pasta);
    std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
    Ok(pasta)
}

/// Destino das gravações: `{ pasta, padrao, indisponivel }`.
#[tauri::command]
pub fn gravacoes_destino(app: AppHandle) -> Result<Destino, String> {
    atual(&app)
}

/// Escolhe a pasta (`None` volta ao padrão). Confere se ela aceita gravar.
#[tauri::command]
pub fn gravacoes_definir_destino(app: AppHandle, pasta: Option<String>) -> Result<Destino, String> {
    let config = match pasta.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()) {
        None => Config::default(),
        Some(pasta) => {
            let caminho = PathBuf::from(&pasta);
            if !caminho.is_absolute() {
                return Err("escolha uma pasta completa (ex.: D:\\ShmupX\\Runs)".into());
            }
            if !gravavel(&caminho) {
                return Err("não é possível gravar nessa pasta: escolha outra".into());
            }
            Config { pasta: Some(pasta) }
        }
    };
    std::fs::create_dir_all(pasta_de_dados(&app)?).map_err(|e| e.to_string())?;
    std::fs::write(arquivo(&app)?, serde_json::to_vec(&config).map_err(|e| e.to_string())?).map_err(|e| format!("não foi possível salvar a escolha: {e}"))?;
    atual(&app)
}

/// Abre a pasta das gravações no gerenciador de arquivos.
#[tauri::command]
pub fn gravacoes_abrir_pasta(app: AppHandle) -> Result<(), String> {
    let pasta = pasta_das_gravacoes(&app)?;
    #[cfg(target_os = "windows")]
    let programa = "explorer";
    #[cfg(target_os = "macos")]
    let programa = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let programa = "xdg-open";
    std::process::Command::new(programa).arg(&pasta).spawn().map(|_| ()).map_err(|e| format!("não foi possível abrir a pasta: {e}"))
}
