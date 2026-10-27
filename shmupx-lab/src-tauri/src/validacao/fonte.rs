//! Leitura, só para visualização, dos arquivos de uma versão do script de um jogo
//! (`validators/games/<jogo>/<versão>/`, ADR 004): o manifesto, a calibração, o passo a passo das
//! imagens e o analisador genérico que o manifesto escolhe. Caminhos fixos, montados a partir de
//! id e versão já validados; nada fora da pasta `validators` é lido.

use std::io::Read;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use super::dados::id_seguro;
use super::gerador::versao_valida;

/// Teto por arquivo: o visualizador não precisa de mais que isso, e um arquivo enorme travaria a tela.
const LIMITE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArquivoFonte {
    /// Rótulo da aba (ex.: `config.json`).
    pub nome: String,
    /// Caminho relativo à pasta `validators`.
    pub caminho: String,
    pub linguagem: &'static str,
    pub conteudo: String,
    /// O arquivo passou do limite e foi cortado.
    pub truncado: bool,
}

/// Nome do analisador no `script.json` (ex.: `hud_score`). Só letras minúsculas, dígitos e `_`.
pub fn analisador(manifesto: &str) -> Option<String> {
    let valor: Value = serde_json::from_str(manifesto).ok()?;
    let nome = valor.get("analyzer")?.as_str()?;
    let valido = !nome.is_empty() && nome.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    valido.then(|| nome.to_string())
}

/// Lê um arquivo de dentro da raiz, com o teto de tamanho. `None` se não existir.
fn ler_arquivo(raiz: &Path, relativo: &str, linguagem: &'static str) -> Result<Option<ArquivoFonte>, String> {
    let Ok(real) = raiz.join(relativo).canonicalize() else { return Ok(None) };
    // Defesa extra: um link simbólico não pode levar para fora da pasta validators.
    if !real.starts_with(raiz) || !real.is_file() {
        return Ok(None);
    }
    let arquivo = std::fs::File::open(&real).map_err(|e| format!("falha ao abrir {relativo}: {e}"))?;
    let mut bytes = Vec::new();
    arquivo
        .take(LIMITE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("falha ao ler {relativo}: {e}"))?;
    let truncado = bytes.len() as u64 > LIMITE_BYTES;
    bytes.truncate(LIMITE_BYTES as usize);
    Ok(Some(ArquivoFonte {
        nome: relativo.rsplit('/').next().unwrap_or(relativo).to_string(),
        caminho: relativo.to_string(),
        linguagem,
        conteudo: String::from_utf8_lossy(&bytes).into_owned(),
        truncado,
    }))
}

/// Lê os arquivos da versão do script, na ordem das abas: manifesto, calibração, passo a passo
/// (se houver) e analisador. Erro quando a versão não existe.
pub fn ler(raiz: &Path, jogo: &str, versao: &str) -> Result<Vec<ArquivoFonte>, String> {
    if !id_seguro(jogo) {
        return Err("id de jogo inválido".into());
    }
    if !versao_valida(versao) {
        return Err("versão de script inválida".into());
    }
    let raiz = raiz.canonicalize().map_err(|e| format!("pasta validators indisponível: {e}"))?;
    let pasta = format!("games/{jogo}/{versao}");
    let manifesto = ler_arquivo(&raiz, &format!("{pasta}/script.json"), "json")?
        .ok_or_else(|| format!("o jogo {jogo} não tem o script {versao}"))?;
    let nome = analisador(&manifesto.conteudo).ok_or("script.json sem um analisador válido")?;

    let mut arquivos = vec![manifesto];
    if let Some(calibracao) = ler_arquivo(&raiz, &format!("{pasta}/config.json"), "json")? {
        arquivos.push(calibracao);
    }
    if let Some(leia_me) = ler_arquivo(&raiz, &format!("{pasta}/templates/LEIA-ME.md"), "markdown")? {
        arquivos.push(leia_me);
    }
    let codigo = ler_arquivo(&raiz, &format!("engine/analyzers/{nome}.py"), "python")?
        .ok_or_else(|| format!("analisador engine/analyzers/{nome}.py não encontrado"))?;
    arquivos.push(codigo);
    Ok(arquivos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raiz_de_teste(nome: &str) -> std::path::PathBuf {
        let raiz = std::env::temp_dir().join(format!("shmupx-lab-fonte-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(raiz.join("engine/analyzers")).unwrap();
        std::fs::create_dir_all(raiz.join("games/ketsui/0.1.0/templates")).unwrap();
        std::fs::write(raiz.join("engine/analyzers/hud_score.py"), "def create_analyzer():\n    pass\n").unwrap();
        std::fs::write(
            raiz.join("games/ketsui/0.1.0/script.json"),
            r#"{"game": "ketsui", "version": "0.1.0", "status": "draft", "analyzer": "hud_score"}"#,
        )
        .unwrap();
        std::fs::write(raiz.join("games/ketsui/0.1.0/config.json"), "{\"reference_height\": 720}").unwrap();
        std::fs::write(raiz.join("games/ketsui/0.1.0/templates/LEIA-ME.md"), "# Ketsui\n").unwrap();
        raiz
    }

    #[test]
    fn analisador_vem_do_manifesto_e_nao_vira_caminho() {
        assert_eq!(analisador(r#"{"analyzer": "hud_score"}"#).as_deref(), Some("hud_score"));
        assert_eq!(analisador(r#"{"analyzer": "../os"}"#), None);
        assert_eq!(analisador(r#"{"analyzer": "engine.analyzers.x"}"#), None);
        assert_eq!(analisador("{}"), None);
    }

    #[test]
    fn le_manifesto_calibracao_passo_a_passo_e_analisador_na_ordem_das_abas() {
        let raiz = raiz_de_teste("ordem");
        let arquivos = ler(&raiz, "ketsui", "0.1.0").unwrap();
        let caminhos: Vec<&str> = arquivos.iter().map(|a| a.caminho.as_str()).collect();
        assert_eq!(
            caminhos,
            [
                "games/ketsui/0.1.0/script.json",
                "games/ketsui/0.1.0/config.json",
                "games/ketsui/0.1.0/templates/LEIA-ME.md",
                "engine/analyzers/hud_score.py"
            ]
        );
        assert!(arquivos[3].conteudo.contains("create_analyzer"));
    }

    #[test]
    fn versao_inexistente_e_erro() {
        let raiz = raiz_de_teste("sem-versao");
        assert!(ler(&raiz, "ketsui", "0.2.0").is_err());
        assert!(ler(&raiz, "dodonpachi", "1.0.0").is_err());
    }

    #[test]
    fn id_ou_versao_que_tentam_sair_da_pasta_sao_recusados() {
        let raiz = raiz_de_teste("travessia");
        assert!(ler(&raiz, "../../etc", "0.1.0").is_err());
        assert!(ler(&raiz, "ketsui", "../0.1.0").is_err());
        assert!(ler(&raiz, "", "0.1.0").is_err());
    }

    #[test]
    fn arquivo_grande_e_cortado_no_limite() {
        let raiz = raiz_de_teste("grande");
        std::fs::write(raiz.join("games/ketsui/0.1.0/config.json"), "x".repeat(LIMITE_BYTES as usize + 10)).unwrap();
        let arquivos = ler(&raiz, "ketsui", "0.1.0").unwrap();
        assert!(arquivos[1].truncado);
        assert_eq!(arquivos[1].conteudo.len(), LIMITE_BYTES as usize);
    }
}
