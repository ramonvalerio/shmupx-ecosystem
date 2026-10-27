//! "Gerar script": prepara a validação de um jogo novo no `shmupx_offline` (ADR 004). O analisador
//! por HUD é genérico (vídeo, HUD, dígitos, vidas, continues, attempts); o que é de cada jogo é a
//! calibração `games/<jogo>.json` e as imagens de `templates/<jogo>/`. O gerador cria a calibração
//! a partir da do Earthion, as pastas das imagens com o passo a passo do recorte
//! (`tools/make_digit_templates.py`) e registra o jogo com o analisador genérico na versão `0.1.0`
//! (rascunho). Não copia imagens do Earthion: elas fariam o jogo novo ser lido errado. O rascunho
//! não chega aos jogadores: o Client só aceita validadores aprovados.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

use super::dados::id_seguro;

/// Jogo de referência: calibração de partida e estrutura das imagens.
const BASE: &str = "earthion";
/// Analisador genérico por HUD (nasceu com o Earthion).
const ANALISADOR: &str = "shmupx.validators.earthion";
const ARQUIVO_ANALISADOR: &str = "shmupx/validators/earthion.py";
const VERSAO_RASCUNHO: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScriptGerado {
    pub versao: String,
    /// Arquivos e pastas criados ou alterados, relativos à raiz do `shmupx_offline`.
    pub criados: Vec<String>,
    /// Imagens que faltam em `templates/<jogo>/` (mesma estrutura do jogo base).
    pub imagens_pendentes: Vec<String>,
}

fn texto_python(valor: &str) -> String {
    valor.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Imagens de referência do jogo base, relativas a `templates/<base>/` (ex.: `digits/pc/0.png`).
fn imagens_do_base(raiz: &Path) -> Result<Vec<String>, String> {
    fn percorrer(pasta: &Path, prefixo: &str, saida: &mut Vec<String>) -> std::io::Result<()> {
        let mut entradas: Vec<_> = std::fs::read_dir(pasta)?.filter_map(Result::ok).collect();
        entradas.sort_by_key(|e| e.file_name());
        for entrada in entradas {
            let nome = entrada.file_name().to_string_lossy().into_owned();
            let relativo = if prefixo.is_empty() { nome.clone() } else { format!("{prefixo}/{nome}") };
            if entrada.path().is_dir() {
                percorrer(&entrada.path(), &relativo, saida)?;
            } else if nome.to_ascii_lowercase().ends_with(".png") {
                saida.push(relativo);
            }
        }
        Ok(())
    }
    let mut saida = Vec::new();
    percorrer(&raiz.join("templates").join(BASE), "", &mut saida).map_err(|e| format!("falha ao ler as imagens do {BASE}: {e}"))?;
    Ok(saida)
}

/// O analisador genérico precisa aceitar o id do jogo (`game=`) para carregar a calibração dele.
fn conferir_analisador(raiz: &Path) -> Result<(), String> {
    let codigo = std::fs::read_to_string(raiz.join(ARQUIVO_ANALISADOR)).map_err(|e| format!("falha ao ler {ARQUIVO_ANALISADOR}: {e}"))?;
    if codigo.contains("def create_analyzer(") && codigo.contains("GameConfig(root, game)") {
        Ok(())
    } else {
        Err(format!("{ARQUIVO_ANALISADOR} ainda não recebe o id do jogo; atualize o shmupx_offline"))
    }
}

/// Calibração do jogo base como ponto de partida, marcada como rascunho.
fn calibracao(raiz: &Path, jogo: &str, titulo: &str) -> Result<String, String> {
    let texto = std::fs::read_to_string(raiz.join("games").join(format!("{BASE}.json")))
        .map_err(|e| format!("falha ao ler a calibração do {BASE}: {e}"))?;
    let mut valor: Value = serde_json::from_str(&texto).map_err(|e| format!("calibração do {BASE} inválida: {e}"))?;
    let objeto = valor.as_object_mut().ok_or("calibração do jogo base não é um objeto")?;
    objeto.insert("game".into(), json!(jogo));
    objeto.insert(
        "description".into(),
        json!(format!("{titulo}: RASCUNHO a partir do {BASE}. Ajuste as variantes do HUD (rótulos), a posição do placar e das vidas (dx/dy/pitch/count) e a tela de resultado.")),
    );
    objeto.insert(
        "_comment_rascunho".into(),
        json!(format!("Valores copiados do {BASE}: posições e escalas referem-se ao HUD dele (altura de referência 720) e precisam ser medidas neste jogo antes de recortar os dígitos.")),
    );
    serde_json::to_string_pretty(&valor).map(|t| t + "\n").map_err(|e| format!("falha ao montar a calibração: {e}"))
}

fn leia_me(jogo: &str, titulo: &str, imagens: &[String]) -> String {
    let lista: String = imagens.iter().map(|i| format!("- `{i}`\n")).collect();
    format!(
        "# Validação de {titulo} (`{jogo}`)\n\n\
         O analisador é o genérico por HUD (o mesmo do {BASE}). O que é deste jogo é a calibração\n\
         `games/{jogo}.json` e as imagens desta pasta.\n\n\
         ## Passo a passo\n\n\
         1. Recorte os rótulos do HUD de um quadro do jogo (placar, rótulo alternativo de chefe e o da tela\n   \
            de resultado) em `labels/` e aponte-os em `games/{jogo}.json` (`hud_variants`, `results`).\n\
         2. Ajuste em `games/{jogo}.json` a posição do placar e das vidas (`score.dx/dy/pitch/count`,\n   \
            `lives`) em relação ao rótulo, na altura de referência de 720 px.\n\
         3. Recorte os dígitos com a ferramenta (rode na pasta `shmupx_offline`):\n\n   \
            ```\n   python tools/make_digit_templates.py VIDEO.mp4 --game {jogo} --out templates/{jogo}/digits/meu_set_raw\n   ```\n\n   \
            Olhe cada `cluster_XX_nYYY.png`, renomeie para `0.png` … `9.png` e apague o que não for dígito.\n\
         4. Rode a análise: `python analyze.py VIDEO.mp4 --game {jogo} --player teste` — ou, no ShmupX Lab,\n   \
            adicione vídeos com gabarito em **Validação** e rode o script até o acerto ficar bom.\n\
            Prefira vídeos com continue ou game over para testar essas regras também.\n\n\
         ## Estrutura de referência (do {BASE})\n\n{lista}"
    )
}

/// Insere a entrada do jogo na tupla `VALIDATORS` do `registry.py`.
fn registrar(registro: &str, jogo: &str, titulo: &str) -> Result<String, String> {
    if registro.contains(&format!("ValidatorSpec(\"{jogo}\"")) {
        return Err(format!("o jogo {jogo} já está registrado"));
    }
    let inicio = registro.find("VALIDATORS = (").ok_or("registry.py sem a tupla VALIDATORS")?;
    let fim = registro[inicio..].find("\n)").map(|p| inicio + p).ok_or("registry.py com a tupla VALIDATORS sem fechamento")?;
    let linha = format!(
        "\n    ValidatorSpec(\"{jogo}\", \"{titulo}\", \"{VERSAO_RASCUNHO}\", \"{ANALISADOR}\"),",
        titulo = texto_python(titulo),
    );
    Ok(format!("{}{linha}{}", &registro[..fim], &registro[fim..]))
}

/// Prepara a validação do jogo. Recusa se a calibração, a pasta de imagens ou o registro já existirem.
pub fn gerar(raiz: &Path, jogo: &str, titulo: &str) -> Result<ScriptGerado, String> {
    if !id_seguro(jogo) {
        return Err("id de jogo inválido".into());
    }
    let titulo = titulo.trim();
    if titulo.is_empty() || titulo.chars().count() > 80 {
        return Err("título do jogo inválido".into());
    }
    let raiz = raiz.canonicalize().map_err(|e| format!("pasta do shmupx_offline indisponível: {e}"))?;
    let config = raiz.join("games").join(format!("{jogo}.json"));
    let templates = raiz.join("templates").join(jogo);
    let caminho_registro = raiz.join("shmupx/registry.py");
    for existente in [&config, &templates] {
        if existente.exists() {
            return Err(format!("já existe {}; o gerador não sobrescreve arquivos", existente.strip_prefix(&raiz).unwrap_or(existente).display()));
        }
    }

    // Tudo é montado em memória antes de gravar: um erro aqui não deixa nada pela metade.
    conferir_analisador(&raiz)?;
    let conteudo_config = calibracao(&raiz, jogo, titulo)?;
    let imagens = imagens_do_base(&raiz)?;
    let registro = std::fs::read_to_string(&caminho_registro).map_err(|e| format!("falha ao ler registry.py: {e}"))?;
    let novo_registro = registrar(&registro, jogo, titulo)?;

    let mut criados: Vec<PathBuf> = Vec::new();
    let resultado = (|| -> Result<(), String> {
        std::fs::write(&config, &conteudo_config).map_err(|e| format!("falha ao gravar a calibração: {e}"))?;
        criados.push(config.clone());
        std::fs::create_dir_all(&templates).map_err(|e| format!("falha ao criar a pasta de imagens: {e}"))?;
        criados.push(templates.clone());
        for pasta in imagens.iter().filter_map(|i| i.rsplit_once('/').map(|(p, _)| p)) {
            std::fs::create_dir_all(templates.join(pasta)).map_err(|e| format!("falha ao criar {pasta}: {e}"))?;
        }
        std::fs::write(templates.join("LEIA-ME.md"), leia_me(jogo, titulo, &imagens)).map_err(|e| format!("falha ao gravar o LEIA-ME: {e}"))?;
        std::fs::write(&caminho_registro, &novo_registro).map_err(|e| format!("falha ao registrar: {e}"))
    })();
    if let Err(erro) = resultado {
        // Desfaz o que foi criado: o registro é o último passo, então não precisa ser revertido.
        for caminho in criados.iter().rev() {
            let _ = if caminho.is_dir() { std::fs::remove_dir_all(caminho) } else { std::fs::remove_file(caminho) };
        }
        return Err(erro);
    }

    let relativo = |p: &Path| p.strip_prefix(&raiz).unwrap_or(p).to_string_lossy().replace('\\', "/");
    Ok(ScriptGerado {
        versao: VERSAO_RASCUNHO.into(),
        criados: vec![relativo(&config), format!("{}/", relativo(&templates)), "shmupx/registry.py".into()],
        imagens_pendentes: imagens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANALISADOR_TESTE: &str = "def create_analyzer(root, video_path, *, debug_dir=None, quiet=False, game=\"earthion\"):\n    return Analyzer(GameConfig(root, game), video_path)\n";
    const REGISTRO_BASE: &str = "VALIDATORS = (\n    ValidatorSpec(\"earthion\", \"Earthion\", \"1.0.0\", \"shmupx.validators.earthion\"),\n)\n\n\ndef get_validator(game_id):\n    pass\n";

    fn raiz_de_teste(nome: &str) -> PathBuf {
        let raiz = std::env::temp_dir().join(format!("shmupx-lab-gerador-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(raiz.join("shmupx/validators")).unwrap();
        std::fs::create_dir_all(raiz.join("games")).unwrap();
        std::fs::create_dir_all(raiz.join("templates/earthion/digits/pc")).unwrap();
        std::fs::create_dir_all(raiz.join("templates/earthion/labels")).unwrap();
        std::fs::write(raiz.join(ARQUIVO_ANALISADOR), ANALISADOR_TESTE).unwrap();
        std::fs::write(raiz.join("games/earthion.json"), "{\"game\": \"earthion\", \"description\": \"x\", \"reference_height\": 720}").unwrap();
        std::fs::write(raiz.join("templates/earthion/digits/pc/0.png"), b"png").unwrap();
        std::fs::write(raiz.join("templates/earthion/labels/hud_score.png"), b"png").unwrap();
        std::fs::write(raiz.join("shmupx/registry.py"), REGISTRO_BASE).unwrap();
        raiz
    }

    #[test]
    fn gera_calibracao_pastas_e_registro_com_o_analisador_generico() {
        let raiz = raiz_de_teste("completo");
        let gerado = gerar(&raiz, "ketsui", "Ketsui \"Kizuna\"").unwrap();
        assert_eq!(gerado.versao, "0.1.0");
        assert_eq!(gerado.imagens_pendentes, ["digits/pc/0.png", "labels/hud_score.png"]);
        assert_eq!(gerado.criados, ["games/ketsui.json", "templates/ketsui/", "shmupx/registry.py"]);

        assert!(!raiz.join("shmupx/validators/ketsui.py").exists(), "a lógica é genérica: nada de cópia por jogo");
        let config: Value = serde_json::from_str(&std::fs::read_to_string(raiz.join("games/ketsui.json")).unwrap()).unwrap();
        assert_eq!(config["game"], "ketsui");
        assert_eq!(config["reference_height"], 720);

        assert!(raiz.join("templates/ketsui/digits/pc").is_dir());
        assert!(!raiz.join("templates/ketsui/digits/pc/0.png").exists(), "imagens do Earthion não são copiadas");
        let leia_me = std::fs::read_to_string(raiz.join("templates/ketsui/LEIA-ME.md")).unwrap();
        assert!(leia_me.contains("tools/make_digit_templates.py VIDEO.mp4 --game ketsui"));

        let registro = std::fs::read_to_string(raiz.join("shmupx/registry.py")).unwrap();
        assert!(registro.contains("ValidatorSpec(\"earthion\", \"Earthion\", \"1.0.0\""), "o Earthion continua primeiro");
        assert!(registro.contains("ValidatorSpec(\"ketsui\", \"Ketsui \\\"Kizuna\\\"\", \"0.1.0\", \"shmupx.validators.earthion\"),\n)"));
    }

    #[test]
    fn nao_sobrescreve_nem_registra_duas_vezes() {
        let raiz = raiz_de_teste("repetido");
        gerar(&raiz, "dodonpachi", "DoDonPachi").unwrap();
        assert!(gerar(&raiz, "dodonpachi", "DoDonPachi").is_err());
        assert!(gerar(&raiz, "earthion", "Earthion").is_err(), "o jogo base já existe");
    }

    #[test]
    fn analisador_que_nao_recebe_o_jogo_impede_a_geracao() {
        let raiz = raiz_de_teste("analisador-antigo");
        std::fs::write(raiz.join(ARQUIVO_ANALISADOR), "def create_analyzer(root, video_path):\n    return Analyzer(GameConfig(root, \"earthion\"))\n").unwrap();
        assert!(gerar(&raiz, "ibara", "Ibara").is_err());
        assert!(!raiz.join("games/ibara.json").exists());
    }

    #[test]
    fn falha_no_meio_nao_deixa_arquivos_pela_metade() {
        let raiz = raiz_de_teste("falha");
        std::fs::write(raiz.join("shmupx/registry.py"), "sem tupla\n").unwrap();
        assert!(gerar(&raiz, "ibara", "Ibara").is_err());
        assert!(!raiz.join("games/ibara.json").exists());
        assert!(!raiz.join("templates/ibara").exists());
    }

    #[test]
    fn ids_e_titulos_invalidos_sao_recusados() {
        let raiz = raiz_de_teste("invalidos");
        assert!(gerar(&raiz, "../x", "X").is_err());
        assert!(gerar(&raiz, "ok", "   ").is_err());
    }
}
