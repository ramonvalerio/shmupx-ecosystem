//! Preenchimento por IA pela assinatura do Claude: o Lab roda o Claude Code instalado no PC do
//! admin (`claude -p`), já logado na conta claude.ai, em vez de chamar a API com chave.
//!
//! - A chave do ambiente (`ANTHROPIC_API_KEY`/`ANTHROPIC_AUTH_TOKEN`) sai do processo filho: com
//!   ela definida, o Claude Code usaria a API em vez da assinatura.
//! - Sem configurações, hooks, MCP nem sessão salva; só a ferramenta de busca, e só quando pedida.
//! - O título, os dados da Steam e o esquema vão como entrada; nada passa por shell (`cmd`).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Caminho do executável do Claude Code, quando não está no lugar de sempre.
pub const VARIAVEL_CAMINHO: &str = "SHMUPX_LAB_CLAUDE";
/// A pesquisa na web leva alguns turnos; ainda assim é finita.
const TIMEOUT: Duration = Duration::from_secs(240);
/// Variáveis que fariam o Claude Code usar a API (ou se achar dentro de outra sessão dele).
const VARIAVEIS_REMOVIDAS: [&str; 3] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDECODE"];

/// Acha o executável: `SHMUPX_LAB_CLAUDE`, depois o PATH (`claude.exe`, ou o `claude.exe` que o
/// `claude.cmd` do npm chama) e o instalador nativo (`~/.local/bin`).
fn executavel() -> Result<PathBuf, String> {
    if let Some(caminho) = std::env::var_os(VARIAVEL_CAMINHO).map(PathBuf::from).filter(|p| p.is_file()) {
        return Ok(caminho);
    }
    let nome = if cfg!(windows) { "claude.exe" } else { "claude" };
    let mut candidatos: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for pasta in std::env::split_paths(&path) {
            candidatos.push(pasta.join(nome));
            // npm: `claude.cmd` repassa para o executável nativo do pacote.
            candidatos.push(pasta.join("node_modules").join("@anthropic-ai").join("claude-code").join("bin").join(nome));
        }
    }
    if let Some(casa) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        candidatos.push(Path::new(&casa).join(".local").join("bin").join(nome));
    }
    candidatos.into_iter().find(|c| c.is_file()).ok_or_else(|| {
        format!("Não achei o Claude Code neste PC. Instale e entre com a sua conta (claude, depois /login), ou aponte {VARIAVEL_CAMINHO} para o executável.")
    })
}

/// Argumentos do `claude -p`: resposta em JSON validada pelo esquema; só a busca na web, e só
/// quando `web` (sem ela, nenhuma ferramenta).
fn argumentos(modelo: &str, sistema: &str, esquema: &Value, web: bool) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p", "--output-format", "json", "--model", modelo, "--system-prompt", sistema,
        "--json-schema", &esquema.to_string(), "--setting-sources", "", "--strict-mcp-config",
        "--no-session-persistence",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if web {
        args.extend(["--tools", "WebSearch", "--allowedTools", "WebSearch", "--permission-mode", "dontAsk"].map(String::from));
    } else {
        args.extend(["--tools", ""].map(String::from));
    }
    args
}

/// Resposta do `claude -p --output-format json`: o objeto do esquema em `structured_output`.
fn resultado(stdout: &str, stderr: &str) -> Result<Value, String> {
    let resposta: Value = serde_json::from_str(stdout.trim()).map_err(|_| {
        let motivo = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("sem resposta");
        format!("O Claude Code não respondeu como esperado: {motivo}")
    })?;
    if resposta["is_error"].as_bool() == Some(true) || resposta["subtype"].as_str() != Some("success") {
        let texto = resposta["result"].as_str().unwrap_or("erro sem detalhe");
        return Err(if texto.to_lowercase().contains("login") || texto.to_lowercase().contains("log in") {
            "O Claude Code não está logado: abra um terminal, rode claude e entre com a sua conta (/login).".to_string()
        } else {
            format!("O Claude Code recusou o pedido: {texto}")
        });
    }
    resposta.get("structured_output").filter(|v| v.is_object()).cloned()
        .ok_or_else(|| "A IA respondeu em formato inesperado; tente de novo.".to_string())
}

/// Roda o Claude Code com o prompt de sistema, o conteúdo (pela entrada padrão) e o esquema, e
/// devolve o JSON do esquema. `pasta` é onde ele roda (a pasta de dados do Lab, sem projeto).
pub async fn estruturado(modelo: &str, sistema: &str, conteudo: &str, esquema: &Value, web: bool, pasta: &Path) -> Result<Value, String> {
    let mut comando = Command::new(executavel()?);
    comando
        .args(argumentos(modelo, sistema, esquema, web))
        .current_dir(pasta)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for variavel in VARIAVEIS_REMOVIDAS {
        comando.env_remove(variavel);
    }
    #[cfg(windows)]
    comando.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: sem janela de console piscando

    let mut filho = comando.spawn().map_err(|e| format!("não foi possível abrir o Claude Code: {e}"))?;
    if let Some(mut entrada) = filho.stdin.take() {
        entrada.write_all(conteudo.as_bytes()).await.map_err(|e| format!("falha ao enviar o pedido ao Claude Code: {e}"))?;
    }
    let saida = tokio::time::timeout(TIMEOUT, filho.wait_with_output())
        .await
        .map_err(|_| "O Claude Code demorou demais para responder; tente de novo.".to_string())?
        .map_err(|e| format!("falha ao ler a resposta do Claude Code: {e}"))?;
    resultado(&String::from_utf8_lossy(&saida.stdout), &String::from_utf8_lossy(&saida.stderr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sem_web_nao_libera_ferramentas() {
        let args = argumentos("claude-sonnet-5-5", "s", &json!({"type": "object"}), false);
        let tools = args.iter().position(|a| a == "--tools").unwrap();
        assert_eq!(args[tools + 1], "");
        assert!(!args.iter().any(|a| a == "WebSearch"));
        assert!(args.iter().any(|a| a == "--no-session-persistence"));
        assert!(!args.iter().any(|a| a == "--bare"), "--bare só autentica por chave, não pela assinatura");
    }

    #[test]
    fn com_web_libera_so_a_busca() {
        let args = argumentos("claude-sonnet-5-5", "s", &json!({"type": "object"}), true);
        let tools = args.iter().position(|a| a == "--tools").unwrap();
        assert_eq!(args[tools + 1], "WebSearch");
        let permitidas = args.iter().position(|a| a == "--allowedTools").unwrap();
        assert_eq!(args[permitidas + 1], "WebSearch");
    }

    #[test]
    fn le_o_objeto_do_esquema() {
        let stdout = json!({ "type": "result", "subtype": "success", "is_error": false, "structured_output": { "suggestions": [] } }).to_string();
        assert_eq!(resultado(&stdout, "").unwrap(), json!({ "suggestions": [] }));
    }

    #[test]
    fn erro_de_login_vira_instrucao() {
        let stdout = json!({ "type": "result", "subtype": "success", "is_error": true, "result": "Not logged in · Please run /login" }).to_string();
        assert!(resultado(&stdout, "").unwrap_err().contains("/login"));
    }

    #[test]
    fn saida_que_nao_e_json_mostra_o_stderr() {
        assert!(resultado("", "algo deu errado\n").unwrap_err().contains("algo deu errado"));
    }
}
