//! Leitura, só para visualização, dos arquivos do validador de um jogo no `shmupx_offline`
//! (ADR 004): a calibração do jogo, o analisador que o registro aponta para ele e o próprio
//! registro. O analisador por HUD é genérico; o que é de cada jogo é `games/<jogo>.json` e as
//! imagens de `templates/<jogo>/`. Caminhos fixos, montados a partir de um id já validado e do
//! módulo registrado; nada fora da raiz do `shmupx_offline` é lido.

use std::io::Read;
use std::path::Path;

use serde::Serialize;

use super::dados::id_seguro;

/// Teto por arquivo: o visualizador não precisa de mais que isso, e um arquivo enorme travaria a tela.
const LIMITE_BYTES: u64 = 512 * 1024;
const REGISTRO: &str = "shmupx/registry.py";
const PREFIXO_MODULO: &str = "shmupx.validators.";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArquivoFonte {
    /// Rótulo da aba (ex.: `earthion.json`).
    pub nome: String,
    /// Caminho relativo à raiz do `shmupx_offline`.
    pub caminho: String,
    pub linguagem: &'static str,
    pub conteudo: String,
    /// O arquivo passou do limite e foi cortado.
    pub truncado: bool,
}

/// Módulo do analisador registrado para o jogo, ex.: `ValidatorSpec("ketsui", "Ketsui", "0.1.0",
/// "shmupx.validators.earthion")` -> `earthion`. Só aceita módulos dentro de `shmupx.validators`.
pub fn modulo_registrado(registro: &str, jogo: &str) -> Option<String> {
    let linha = registro.lines().find(|l| l.trim_start().starts_with(&format!("ValidatorSpec(\"{jogo}\",")))?;
    let modulo = linha.rsplit('"').nth(1)?.strip_prefix(PREFIXO_MODULO)?;
    let valido = !modulo.is_empty() && modulo.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    valido.then(|| modulo.to_string())
}

/// Lê um arquivo de dentro da raiz, com o teto de tamanho. `None` se não existir.
fn ler_arquivo(raiz: &Path, relativo: &str, linguagem: &'static str) -> Result<Option<ArquivoFonte>, String> {
    let Ok(real) = raiz.join(relativo).canonicalize() else { return Ok(None) };
    // Defesa extra: um link simbólico não pode levar para fora do shmupx_offline.
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

/// Lê os arquivos do validador do jogo, na ordem das abas: calibração do jogo, analisador e
/// registro. Erro quando o jogo não está registrado.
pub fn ler(raiz: &Path, jogo: &str) -> Result<Vec<ArquivoFonte>, String> {
    if !id_seguro(jogo) {
        return Err("id de jogo inválido".into());
    }
    let raiz = raiz.canonicalize().map_err(|e| format!("pasta do shmupx_offline indisponível: {e}"))?;
    let registro = ler_arquivo(&raiz, REGISTRO, "python")?.ok_or("registry.py não encontrado no shmupx_offline")?;
    let modulo = modulo_registrado(&registro.conteudo, jogo)
        .ok_or_else(|| format!("o jogo {jogo} não tem script de validação registrado"))?;

    let mut arquivos = Vec::new();
    if let Some(calibracao) = ler_arquivo(&raiz, &format!("games/{jogo}.json"), "json")? {
        arquivos.push(calibracao);
    }
    let analisador = ler_arquivo(&raiz, &format!("shmupx/validators/{modulo}.py"), "python")?
        .ok_or_else(|| format!("analisador shmupx/validators/{modulo}.py não encontrado"))?;
    arquivos.push(analisador);
    arquivos.push(registro);
    Ok(arquivos)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRO_TESTE: &str = "VALIDATORS = (\n    ValidatorSpec(\"earthion\", \"Earthion\", \"1.0.0\", \"shmupx.validators.earthion\"),\n    ValidatorSpec(\"ketsui\", \"Ketsui\", \"0.1.0\", \"shmupx.validators.earthion\"),\n    ValidatorSpec(\"fora\", \"Fora\", \"0.1.0\", \"os.path\"),\n)\n";

    fn raiz_de_teste(nome: &str) -> std::path::PathBuf {
        let raiz = std::env::temp_dir().join(format!("shmupx-lab-fonte-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(raiz.join("shmupx/validators")).unwrap();
        std::fs::create_dir_all(raiz.join("games")).unwrap();
        std::fs::write(raiz.join("shmupx/validators/earthion.py"), "def create_analyzer():\n    pass\n").unwrap();
        std::fs::write(raiz.join("games/earthion.json"), "{\"game\": \"earthion\"}").unwrap();
        std::fs::write(raiz.join("games/ketsui.json"), "{\"game\": \"ketsui\"}").unwrap();
        std::fs::write(raiz.join(REGISTRO), REGISTRO_TESTE).unwrap();
        raiz
    }

    #[test]
    fn modulo_vem_do_registro_e_fica_em_shmupx_validators() {
        assert_eq!(modulo_registrado(REGISTRO_TESTE, "earthion").as_deref(), Some("earthion"));
        assert_eq!(modulo_registrado(REGISTRO_TESTE, "ketsui").as_deref(), Some("earthion"));
        assert_eq!(modulo_registrado(REGISTRO_TESTE, "fora"), None);
        assert_eq!(modulo_registrado(REGISTRO_TESTE, "dodonpachi"), None);
    }

    #[test]
    fn le_calibracao_analisador_e_registro_na_ordem_das_abas() {
        let raiz = raiz_de_teste("ordem");
        let arquivos = ler(&raiz, "ketsui").unwrap();
        let caminhos: Vec<&str> = arquivos.iter().map(|a| a.caminho.as_str()).collect();
        assert_eq!(caminhos, ["games/ketsui.json", "shmupx/validators/earthion.py", "shmupx/registry.py"]);
        assert!(arquivos[1].conteudo.contains("create_analyzer"));
    }

    #[test]
    fn jogo_sem_registro_e_erro() {
        let raiz = raiz_de_teste("sem-registro");
        assert!(ler(&raiz, "dodonpachi").is_err());
    }

    #[test]
    fn id_que_tenta_sair_da_pasta_e_recusado() {
        let raiz = raiz_de_teste("travessia");
        assert!(ler(&raiz, "../../etc").is_err());
        assert!(ler(&raiz, "").is_err());
    }

    #[test]
    fn arquivo_grande_e_cortado_no_limite() {
        let raiz = raiz_de_teste("grande");
        std::fs::write(raiz.join("games/earthion.json"), "x".repeat(LIMITE_BYTES as usize + 10)).unwrap();
        let arquivos = ler(&raiz, "earthion").unwrap();
        assert!(arquivos[0].truncado);
        assert_eq!(arquivos[0].conteudo.len(), LIMITE_BYTES as usize);
    }
}
