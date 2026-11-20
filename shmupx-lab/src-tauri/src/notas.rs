//! Notas da versão a partir do código: os commits do `shmupx-client` desde a versão anterior
//! viram linhas de Novidades (`feat`) e Correções (`fix`) no rascunho do Lab.
//!
//! "Versão anterior" é o último commit em que o `tauri.conf.json` passou a ter uma versão menor
//! que a do rascunho; sem nenhum, usa os commits mais recentes. Commits de manutenção (`docs`,
//! `chore`, `test`, `refactor`, `style`, `build`, `ci`) não entram.

use std::path::Path;

use serde::Serialize;
use tokio::process::Command;

use serde_json::{json, Value};

use crate::ai::anthropic;
use crate::instalador::pasta_client;

const LIMITE_SEM_BASE: usize = 15;
const CONFIG: &str = "src-tauri/tauri.conf.json";

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct NotasDoClient {
    pub features: Vec<String>,
    pub fixes: Vec<String>,
    /// Commit da versão anterior (abreviado), ou `None` se não houve base.
    pub since: Option<String>,
    pub commits: usize,
    /// As linhas foram reescritas pela IA para o jogador (senão, são os assuntos dos commits).
    pub rewritten: bool,
    /// Por que a reescrita não aconteceu (sem chave, falha de rede…).
    pub rewrite_error: Option<String>,
}

const INSTRUCOES: &str = "Você escreve as notas de atualização do ShmupX Client, um app desktop onde jogadores de shmup (jogos de nave) entram em salas, conversam, gravam runs e disputam rankings. Recebe a lista de commits do código desde a versão anterior e devolve o que o JOGADOR percebe, para ele bater o olho num aviso.
Regras:
- Português do Brasil, frases curtas (até 60 caracteres), sem ponto final, começando com letra maiúscula.
- No máximo 5 novidades e 4 correções; junte itens parecidos e fique com o que mais importa ao jogador.
- Nada técnico: sem nomes de ferramentas, bibliotecas, bancos de dados, serviços, arquitetura, decisões internas, IDs, siglas de engenharia (ex.: ADR, Firestore, FFmpeg, OAuth, PKCE, token, Rust, Superdesign).
- Ignore o que o jogador não vê: refatorações, infraestrutura, testes, instalação interna, sincronização de sessão.
- Fale do benefício: \"Nova tela de login\", \"Chat da sala em tempo real\", \"Atualizações automáticas do app\".
- Correção é algo que antes falhava e agora funciona. Na dúvida, é novidade.
- Sem nada relevante ao jogador, devolva listas vazias.";

fn esquema() -> Value {
    let lista = json!({ "type": "array", "items": { "type": "string" } });
    json!({
        "type": "object",
        "properties": { "features": lista, "fixes": lista },
        "required": ["features", "fixes"],
        "additionalProperties": false
    })
}

/// Reescreve os commits (técnicos) em notas curtas para o jogador.
async fn para_o_jogador(features: &[String], fixes: &[String]) -> Result<(Vec<String>, Vec<String>), String> {
    let config = anthropic::Config::do_ambiente()?;
    let entrada = format!(
        "Novidades nos commits:
{}

Correções nos commits:
{}",
        features.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("
"),
        fixes.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("
"),
    );
    let corpo = json!({
        "model": config.modelo,
        "max_tokens": 2000,
        "system": INSTRUCOES,
        "messages": [{ "role": "user", "content": entrada }],
        "output_config": { "effort": "low", "format": { "type": "json_schema", "schema": esquema() } },
    });
    let resposta = anthropic::enviar(&config, &corpo).await?;
    let notas: Value = serde_json::from_str(&anthropic::texto_da_resposta(&resposta))
        .map_err(|e| format!("resposta da IA em formato inesperado: {e}"))?;
    let lista = |campo: &str| -> Vec<String> {
        notas[campo]
            .as_array()
            .map(|itens| itens.iter().filter_map(|i| i.as_str()).map(|i| i.trim().trim_end_matches('.').to_string()).filter(|i| !i.is_empty()).collect())
            .unwrap_or_default()
    };
    Ok((lista("features"), lista("fixes")))
}

fn versao(texto: &str) -> Option<(u32, u32, u32)> {
    let mut partes = texto.trim().split('.').map(|p| p.parse::<u32>().ok());
    let v = (partes.next()??, partes.next()??, partes.next()??);
    partes.next().is_none().then_some(v)
}

async fn git(pasta: &Path, args: &[&str]) -> Result<String, String> {
    let saida = Command::new("git")
        .arg("-C")
        .arg(pasta)
        .args(args)
        .output()
        .await
        .map_err(|e| format!("git indisponível: {e}"))?;
    if !saida.status.success() {
        return Err(format!("git falhou: {}", String::from_utf8_lossy(&saida.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&saida.stdout).into_owned())
}

/// Linha do commit: o assunto (ou, num commit de várias áreas, os itens `- client: …` do corpo).
fn classificar(assunto: &str, corpo: &str) -> Vec<(&'static str, String)> {
    const TIPOS: [&str; 11] = ["feat", "fix", "perf", "docs", "chore", "test", "refactor", "style", "build", "ci", "revert"];
    let convencional = assunto.split_once(':').and_then(|(cabeca, resto)| {
        let tipo = cabeca.split('(').next().unwrap_or("").trim_end_matches('!').to_ascii_lowercase();
        TIPOS.contains(&tipo.as_str()).then(|| (tipo, resto.trim()))
    });
    // Commit sem tipo (histórico antigo): conta como novidade.
    let (tipo, texto) = convencional.unwrap_or_else(|| ("feat".to_string(), assunto.trim()));
    let secao = match tipo.as_str() {
        "feat" | "perf" => "features",
        "fix" => "fixes",
        _ => return vec![],
    };
    let itens_client: Vec<String> = corpo
        .lines()
        .filter_map(|l| l.trim().strip_prefix("- client:").or_else(|| l.trim().strip_prefix("- Client:")))
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let linhas = if itens_client.is_empty() { vec![texto.to_string()] } else { itens_client };
    linhas
        .into_iter()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let mut c = l.chars();
            let frase = c.next().map(|p| p.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
            (secao, frase)
        })
        .collect()
}

/// Último commit em que a versão do Client ficou menor que `alvo` (a base das notas).
async fn base(pasta: &Path, alvo: (u32, u32, u32)) -> Result<Option<String>, String> {
    let commits = git(pasta, &["log", "--format=%H", "-G\"version\"", "--", CONFIG]).await?;
    for hash in commits.lines().map(str::trim).filter(|h| !h.is_empty()) {
        let Ok(conteudo) = git(pasta, &["show", &format!("{hash}:./{CONFIG}")]).await else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&conteudo) else { continue };
        if json.get("version").and_then(|v| v.as_str()).and_then(versao).is_some_and(|v| v < alvo) {
            return Ok(Some(hash.to_string()));
        }
    }
    Ok(None)
}

/// Novidades e correções do Client para o rascunho da `versao`, lidas do git.
#[tauri::command]
pub async fn notas_do_client(versao_alvo: String) -> Result<NotasDoClient, String> {
    let alvo = versao(&versao_alvo).ok_or("versão inválida")?;
    let pasta = pasta_client()?;
    let base = base(&pasta, alvo).await?;
    let limite = format!("-{LIMITE_SEM_BASE}");
    let intervalo = base.as_ref().map(|b| format!("{b}..HEAD"));
    let mut args = vec!["log", "--no-merges", "--format=%s%x1f%b%x1e"];
    match &intervalo {
        Some(i) => args.push(i),
        None => args.push(&limite),
    }
    args.extend(["--", "."]);
    let log = git(&pasta, &args).await?;

    let mut notas = NotasDoClient { since: base.map(|b| b[..b.len().min(7)].to_string()), ..Default::default() };
    // git log vem do mais novo ao mais antigo; as notas leem melhor em ordem cronológica.
    for registro in log.split('\u{1e}').map(str::trim).filter(|r| !r.is_empty()).collect::<Vec<_>>().into_iter().rev() {
        notas.commits += 1;
        let (assunto, corpo) = registro.split_once('\u{1f}').unwrap_or((registro, ""));
        for (secao, linha) in classificar(assunto, corpo) {
            let lista = if secao == "fixes" { &mut notas.fixes } else { &mut notas.features };
            if !lista.contains(&linha) {
                lista.push(linha);
            }
        }
    }
    if notas.features.is_empty() && notas.fixes.is_empty() {
        return Ok(notas);
    }
    match para_o_jogador(&notas.features, &notas.fixes).await {
        Ok((features, fixes)) => {
            notas.features = features;
            notas.fixes = fixes;
            notas.rewritten = true;
        }
        Err(e) => notas.rewrite_error = Some(e),
    }
    Ok(notas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tipos_viram_secoes() {
        assert_eq!(classificar("feat(client): sala nova", ""), vec![("features", "Sala nova".to_string())]);
        assert_eq!(classificar("fix: login travado", ""), vec![("fixes", "Login travado".to_string())]);
        assert!(classificar("docs: readme", "").is_empty());
        assert!(classificar("chore(deps): bump", "").is_empty());
        assert_eq!(classificar("Runs: gravação confiável", ""), vec![("features", "Runs: gravação confiável".to_string())]);
    }

    #[test]
    fn commit_de_varias_areas_usa_os_itens_do_client() {
        let corpo = "- Lab: upload\n- client: capa nos cards\n- backend: campo";
        assert_eq!(classificar("feat: capa", corpo), vec![("features", "Capa nos cards".to_string())]);
    }

    #[test]
    fn versoes() {
        assert_eq!(versao("0.1.2"), Some((0, 1, 2)));
        assert!(versao("0.1").is_none());
        assert!(versao("0.1.1") < versao("0.1.2"));
    }
}
