//! Login Google pelo navegador do sistema (loopback + PKCE), como no Client do PC: o app escuta
//! em 127.0.0.1 enquanto o jogador consente no navegador do celular; a troca do código usa o
//! backend (o `client_secret` nunca vem para o app).

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const SCOPES: &str = "openid email profile";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
const BACKEND_BASE_URL: &str = match option_env!("SHMUPX_BACKEND_URL") {
    Some(url) => url,
    None => "https://shmupx-backend-230979440400.us-central1.run.app",
};

const PAGINA_OK: &str = "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'><title>ShmupX</title>\
<body style=\"font-family:sans-serif;background:#1c1c1c;color:#eee;text-align:center;padding-top:20vh\">\
<h1>Login concluído</h1><p>Volte ao ShmupX Lite.</p>";
const PAGINA_ERRO: &str = "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'><title>ShmupX</title>\
<body style=\"font-family:sans-serif;background:#1c1c1c;color:#eee;text-align:center;padding-top:20vh\">\
<h1>Login não concluído</h1><p>Volte ao ShmupX Lite e tente novamente.</p>";

#[derive(Serialize)]
struct TrocaCodigo<'a> {
    code: &'a str,
    code_verifier: &'a str,
    redirect_uri: &'a str,
}

#[derive(Deserialize)]
struct TrocaResposta {
    id_token: String,
}

pub async fn login(app: &tauri::AppHandle, client_id: &str) -> Result<String, String> {
    if client_id.trim().is_empty() {
        return Err("cliente OAuth do Google não configurado".into());
    }
    let verifier = aleatorio(32)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = aleatorio(16)?;

    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| format!("falha ao abrir a porta local do login: {e}"))?;
    let porta = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{porta}");

    let mut url = url::Url::parse(AUTH_URL).expect("URL constante");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPES)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state)
        .append_pair("prompt", "select_account");
    app.opener().open_url(url.as_str(), None::<&str>).map_err(|e| format!("falha ao abrir o navegador: {e}"))?;

    let code = tokio::time::timeout(LOGIN_TIMEOUT, aguardar_codigo(&listener, &state))
        .await
        .map_err(|_| "tempo do login esgotado".to_string())??;

    let resposta = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("{BACKEND_BASE_URL}/api/auth/google/token"))
        .json(&TrocaCodigo { code: &code, code_verifier: &verifier, redirect_uri: &redirect_uri })
        .send()
        .await
        .map_err(|e| format!("falha de rede ao concluir o login: {e}"))?;
    if !resposta.status().is_success() {
        let status = resposta.status();
        return Err(format!("backend respondeu HTTP {status}: {}", resposta.text().await.unwrap_or_default()));
    }
    Ok(resposta.json::<TrocaResposta>().await.map_err(|e| e.to_string())?.id_token)
}

async fn aguardar_codigo(listener: &TcpListener, state: &str) -> Result<String, String> {
    loop {
        let (mut stream, _) = listener.accept().await.map_err(|e| format!("falha ao receber o retorno do login: {e}"))?;
        let Some(query) = ler_query(&mut stream).await else {
            responder(&mut stream, "404 Not Found", "").await;
            continue;
        };
        let pares: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
        let valor = |chave: &str| pares.iter().find(|(k, _)| k == chave).map(|(_, v)| v.clone());
        if valor("state").as_deref() != Some(state) {
            responder(&mut stream, "400 Bad Request", PAGINA_ERRO).await;
            return Err("retorno do login com state inválido".into());
        }
        if let Some(erro) = valor("error") {
            responder(&mut stream, "200 OK", PAGINA_ERRO).await;
            return Err(if erro == "access_denied" { "login cancelado".into() } else { format!("Google recusou o login: {erro}") });
        }
        let Some(code) = valor("code") else {
            responder(&mut stream, "400 Bad Request", PAGINA_ERRO).await;
            return Err("retorno do login sem código".into());
        };
        responder(&mut stream, "200 OK", PAGINA_OK).await;
        return Ok(code);
    }
}

async fn ler_query(stream: &mut TcpStream) -> Option<String> {
    let mut buffer = vec![0u8; 8192];
    let lidos = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buffer)).await.ok()?.ok()?;
    let texto = String::from_utf8_lossy(&buffer[..lidos]);
    let alvo = texto.lines().next()?.strip_prefix("GET ")?.split(' ').next()?;
    alvo.strip_prefix("/?").map(str::to_string)
}

async fn responder(stream: &mut TcpStream, status: &str, corpo: &str) {
    let resposta = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    );
    let _ = stream.write_all(resposta.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn aleatorio(bytes: usize) -> Result<String, String> {
    let mut buffer = vec![0u8; bytes];
    getrandom::getrandom(&mut buffer).map_err(|e| format!("falha ao gerar valor aleatório: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(buffer))
}
