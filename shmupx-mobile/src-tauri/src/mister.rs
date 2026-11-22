//! MiSTer FPGA pelo celular, com o mesmo código do Client (`shmupx-mister-link`): ler os jogos,
//! instalar/ligar o jogador à conta, listar os modos e abrir/parar o jogo na TV. Os comandos têm os
//! mesmos nomes dos do Client, então as telas e dados reaproveitados do Client funcionam iguais.

use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use shmupx_mister_link::{rede, ssh};
use tauri::{AppHandle, Emitter, State};

const BACKEND: &str = match option_env!("SHMUPX_BACKEND_URL") {
    Some(url) => url,
    None => "https://shmupx-backend-230979440400.us-central1.run.app",
};

/// ID token do Firebase do jogador (a interface manda a cada renovação): só para pedir o token do
/// MiSTer ao backend.
#[derive(Default)]
pub struct Sessao(pub Mutex<Option<String>>);

fn senha(senha: Option<String>) -> String {
    senha.filter(|s| !s.is_empty()).unwrap_or_else(|| "1".into())
}

async fn backend_post(sessao: &Sessao, caminho: &str) -> Result<Value, String> {
    let token = sessao.0.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or("entre na sua conta para conectar o MiSTer")?;
    let resposta = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("{BACKEND}{caminho}"))
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_LENGTH, 0)
        .body(Vec::<u8>::new())
        .send()
        .await
        .map_err(|e| format!("falha de rede com o ShmupX: {e}"))?;
    if !resposta.status().is_success() {
        let status = resposta.status();
        return Err(format!("o ShmupX respondeu HTTP {status}: {}", resposta.text().await.unwrap_or_default()));
    }
    resposta.json::<Value>().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn definir_token_autenticacao(sessao: State<'_, Sessao>, token: String) {
    *sessao.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(token);
}

#[tauri::command]
pub fn limpar_token_autenticacao(sessao: State<'_, Sessao>) {
    *sessao.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Jogos de arcade e ROMs do MiSTer (FTP).
#[tauri::command]
pub async fn mister_biblioteca(host: String, senha: Option<String>) -> Result<rede::BibliotecaMister, String> {
    tauri::async_runtime::spawn_blocking(move || rede::ler(&host, senha.as_deref())).await.map_err(|e| e.to_string())?
}

/// Jogadores do `shmupx.ini` do MiSTer (FTP).
#[tauri::command]
pub async fn mister_usuarios(host: String, senha: Option<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || rede::usuarios(&host, senha.as_deref())).await.map_err(|e| e.to_string())?
}

/// Instala/atualiza o ShmupX no MiSTer (SSH) e liga o jogador à conta com um token novo.
#[tauri::command]
pub async fn mister_conectar(app: AppHandle, sessao: State<'_, Sessao>, host: String, senha: Option<String>, username: String) -> Result<String, String> {
    if !rede::username_valido(&username) {
        return Err("nome de jogador inválido: use letras, números, _ e -".into());
    }
    let resposta = backend_post(&sessao, "/api/mister/tokens/direct").await?;
    let token = resposta["token"].as_str().filter(|t| t.starts_with("sxm_")).ok_or("o ShmupX não devolveu o token do MiSTer")?.to_string();
    let senha = self::senha(senha);
    ssh::instalar(host.trim(), &senha, &username, &token, BACKEND, |etapa| {
        let _ = app.emit("mister://etapa", etapa.to_string());
    })
    .await
}

/// Alternativa ao direto: o token vai para o e-mail da conta.
#[tauri::command]
pub async fn mister_enviar_token(sessao: State<'_, Sessao>) -> Result<Value, String> {
    backend_post(&sessao, "/api/mister/tokens").await
}

/// Modos em que o jogador tem cada jogo no menu ShmupX do MiSTer (SSH).
#[tauri::command]
pub async fn mister_jogos_do_jogador(host: String, senha: Option<String>, username: String) -> Result<Vec<ssh::JogoNoMenu>, String> {
    ssh::jogos_no_menu(host.trim(), &self::senha(senha), &username).await
}

/// Abre o jogo na TV (como escolher no menu do MiSTer).
#[tauri::command]
pub async fn mister_iniciar(host: String, senha: Option<String>, caminho: String) -> Result<(), String> {
    ssh::abrir(host.trim(), &self::senha(senha), &caminho).await
}

/// Para o jogo: o MiSTer volta ao menu e o agente grava a partida.
#[tauri::command]
pub async fn mister_parar(host: String, senha: Option<String>) -> Result<(), String> {
    ssh::voltar_ao_menu(host.trim(), &self::senha(senha)).await
}
