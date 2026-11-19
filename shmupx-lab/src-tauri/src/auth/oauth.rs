use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::net::backend;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";

/// Só identidade: permissão de upload no YouTube é pedida separadamente, ao conectar o canal.
const SCOPES: &str = "openid email profile";

/// Tempo para o jogador concluir o consentimento no navegador.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

const PAGINA_OK: &str = "<!doctype html><meta charset=utf-8><title>ShmupX</title>\
<body style=\"font-family:sans-serif;background:#1c1c1c;color:#eee;text-align:center;padding-top:20vh\">\
<h1>Login concluído</h1><p>Você já pode fechar esta aba e voltar ao ShmupX.</p>";
const PAGINA_ERRO: &str = "<!doctype html><meta charset=utf-8><title>ShmupX</title>\
<body style=\"font-family:sans-serif;background:#1c1c1c;color:#eee;text-align:center;padding-top:20vh\">\
<h1>Login não concluído</h1><p>Volte ao ShmupX e tente novamente.</p>";

pub async fn login(client_id: &str, backend_url: Option<&str>) -> Result<String, String> {
    if client_id.trim().is_empty() {
        return Err("cliente OAuth do Google não configurado".to_string());
    }

    let verifier = aleatorio_base64(32)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = aleatorio_base64(16)?;

    // Porta efêmera só em 127.0.0.1: nada fica exposto na rede local.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("falha ao abrir a porta local do login: {e}"))?;
    let porta = listener
        .local_addr()
        .map_err(|e| format!("falha ao ler a porta local do login: {e}"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{porta}");

    let mut url = url::Url::parse(AUTH_URL).expect("URL de autorização constante");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPES)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state)
        .append_pair("prompt", "select_account");
    abrir_navegador(url.as_str())?;

    let code = tokio::time::timeout(LOGIN_TIMEOUT, aguardar_codigo(&listener, &state))
        .await
        .map_err(|_| "tempo do login esgotado".to_string())??;

    // A troca exige o `client_secret`, que só o backend conhece; o app nunca o carrega.
    backend::trocar_codigo_google(backend_url, &code, &verifier, &redirect_uri, client_id).await
}

/// Aceita conexões até chegar o redirecionamento do Google. Requisições alheias (favicon,
/// sondagens) são ignoradas; um `state` divergente encerra o login.
async fn aguardar_codigo(listener: &TcpListener, state: &str) -> Result<String, String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| format!("falha ao receber o retorno do login: {e}"))?;
        let Some(query) = ler_query(&mut stream).await else {
            responder(&mut stream, "404 Not Found", "").await;
            continue;
        };

        let pares: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();
        let valor = |chave: &str| pares.iter().find(|(k, _)| k == chave).map(|(_, v)| v.clone());

        if valor("state").as_deref() != Some(state) {
            responder(&mut stream, "400 Bad Request", PAGINA_ERRO).await;
            return Err("retorno do login com state inválido".to_string());
        }
        if let Some(erro) = valor("error") {
            responder(&mut stream, "200 OK", PAGINA_ERRO).await;
            return Err(if erro == "access_denied" {
                "login cancelado".to_string()
            } else {
                format!("Google recusou o login: {erro}")
            });
        }
        let Some(code) = valor("code") else {
            responder(&mut stream, "400 Bad Request", PAGINA_ERRO).await;
            return Err("retorno do login sem código".to_string());
        };
        responder(&mut stream, "200 OK", PAGINA_OK).await;
        return Ok(code);
    }
}

/// Lê só a linha de requisição: `GET /?...` devolve a query; qualquer outro caminho, `None`.
async fn ler_query(stream: &mut TcpStream) -> Option<String> {
    let mut buffer = vec![0u8; 8192];
    let lidos = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buffer))
        .await
        .ok()?
        .ok()?;
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

fn aleatorio_base64(bytes: usize) -> Result<String, String> {
    let mut buffer = vec![0u8; bytes];
    getrandom::getrandom(&mut buffer).map_err(|e| format!("falha ao gerar valor aleatório: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(buffer))
}

fn abrir_navegador(url: &str) -> Result<(), String> {
    // `rundll32` recebe a URL como um argumento só; `cmd /C start` quebraria nos `&` da query.
    #[cfg(target_os = "windows")]
    let mut comando = {
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut comando = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut comando = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };

    comando
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("falha ao abrir o navegador do sistema: {e}"))
}
