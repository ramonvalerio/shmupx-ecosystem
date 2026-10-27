//! Scripts de validação do Lab (ADR 004): cada pasta `validators/games/<jogo>/<versão>/` é um
//! script (`script.json`, `config.json` e `templates/`). O analisador por HUD é genérico; o que é de
//! cada jogo é a calibração e as imagens. Este módulo cria o primeiro script de um jogo (0.1.0, a
//! partir da calibração do Earthion, sem as imagens dele) e as versões seguintes (cópia de uma
//! versão existente). Toda versão nova nasce rascunho (`draft`); nada existente é sobrescrito.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

use super::dados::id_seguro;

/// Jogo de referência: calibração de partida e estrutura das imagens.
const BASE: &str = "earthion";
const ANALISADOR: &str = "hud_score";
const PRIMEIRA_VERSAO: &str = "0.1.0";
/// Recortes brutos do `make_digit_templates.py`: não passam para a versão seguinte.
const PASTA_BRUTOS: &str = "raw_digits";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScriptGerado {
    pub versao: String,
    /// Pastas criadas, relativas à pasta `validators`.
    pub criados: Vec<String>,
    /// Imagens que faltam em `templates/` (estrutura do jogo base). Vazio numa versão copiada.
    pub imagens_pendentes: Vec<String>,
}

type Versao = (u32, u32, u32);

fn ler_versao(texto: &str) -> Option<Versao> {
    let mut partes = texto.split('.');
    let mut parte = || -> Option<u32> {
        let p = partes.next()?;
        let valida = !p.is_empty() && p.len() <= 4 && p.chars().all(|c| c.is_ascii_digit()) && (p == "0" || !p.starts_with('0'));
        if valida { p.parse().ok() } else { None }
    };
    let versao = (parte()?, parte()?, parte()?);
    partes.next().is_none().then_some(versao)
}

/// `1.2.3`: três números sem zeros à esquerda (o mesmo formato que o motor Python aceita).
pub fn versao_valida(texto: &str) -> bool {
    ler_versao(texto).is_some()
}

/// Versões do jogo que têm `script.json`, em ordem crescente.
fn versoes(raiz: &Path, jogo: &str) -> Vec<Versao> {
    let mut lista: Vec<Versao> = std::fs::read_dir(raiz.join("games").join(jogo))
        .map(|entradas| {
            entradas
                .filter_map(Result::ok)
                .filter(|e| e.path().join("script.json").is_file())
                .filter_map(|e| ler_versao(&e.file_name().to_string_lossy()))
                .collect()
        })
        .unwrap_or_default();
    lista.sort();
    lista
}

fn texto_versao((a, b, c): Versao) -> String {
    format!("{a}.{b}.{c}")
}

fn json_bonito(valor: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(valor).map(|t| t + "\n").map_err(|e| format!("falha ao montar o JSON: {e}"))
}

/// Imagens de referência, relativas a `templates/` (ex.: `digits/pc/0.png`), sem os recortes brutos.
fn imagens(templates: &Path) -> Result<Vec<String>, String> {
    fn percorrer(pasta: &Path, prefixo: &str, saida: &mut Vec<String>) -> std::io::Result<()> {
        let mut entradas: Vec<_> = std::fs::read_dir(pasta)?.filter_map(Result::ok).collect();
        entradas.sort_by_key(|e| e.file_name());
        for entrada in entradas {
            let nome = entrada.file_name().to_string_lossy().into_owned();
            let relativo = if prefixo.is_empty() { nome.clone() } else { format!("{prefixo}/{nome}") };
            if entrada.path().is_dir() {
                if nome != PASTA_BRUTOS {
                    percorrer(&entrada.path(), &relativo, saida)?;
                }
            } else if nome.to_ascii_lowercase().ends_with(".png") {
                saida.push(relativo);
            }
        }
        Ok(())
    }
    let mut saida = Vec::new();
    percorrer(templates, "", &mut saida).map_err(|e| format!("falha ao ler as imagens de {}: {e}", templates.display()))?;
    Ok(saida)
}

/// Calibração do jogo base como ponto de partida, marcada como rascunho.
fn calibracao(origem: &Path, titulo: &str) -> Result<String, String> {
    let texto = std::fs::read_to_string(origem.join("config.json")).map_err(|e| format!("falha ao ler a calibração do {BASE}: {e}"))?;
    let mut valor: Value = serde_json::from_str(&texto).map_err(|e| format!("calibração do {BASE} inválida: {e}"))?;
    let objeto = valor.as_object_mut().ok_or("calibração do jogo base não é um objeto")?;
    objeto.insert(
        "description".into(),
        json!(format!("{titulo}: RASCUNHO a partir do {BASE}. Ajuste as variantes do HUD (rótulos), a posição do placar e das vidas (dx/dy/pitch/count) e a tela de resultado.")),
    );
    objeto.insert(
        "_comment_rascunho".into(),
        json!(format!("Valores copiados do {BASE}: posições e escalas referem-se ao HUD dele (altura de referência 720) e precisam ser medidas neste jogo antes de recortar os dígitos.")),
    );
    json_bonito(&valor)
}

fn leia_me(jogo: &str, titulo: &str, versao: &str, imagens: &[String]) -> String {
    let lista: String = imagens.iter().map(|i| format!("- `{i}`\n")).collect();
    format!(
        "# Validação de {titulo} (`{jogo}` {versao})\n\n\
         O analisador é o genérico por HUD (`engine/analyzers/{ANALISADOR}.py`, o mesmo do {BASE}). O que é\n\
         deste jogo é a calibração `config.json` e as imagens desta pasta `templates/`.\n\n\
         ## Passo a passo (na pasta `shmupx-lab/validators`)\n\n\
         1. Recorte os rótulos do HUD de um quadro do jogo (placar, rótulo alternativo de chefe e o da tela\n   \
            de resultado) em `templates/labels/` e aponte-os no `config.json` (`hud_variants`, `results`).\n\
         2. Ajuste no `config.json` a posição do placar e das vidas (`score.dx/dy/pitch/count`, `lives`)\n   \
            em relação ao rótulo, na altura de referência de 720 px.\n\
         3. Recorte os dígitos com a ferramenta:\n\n   \
            ```\n   python tools/make_digit_templates.py VIDEO.mp4 --game {jogo} --version {versao}\n   ```\n\n   \
            Os grupos vão para `templates/{PASTA_BRUTOS}/`. Olhe cada `cluster_XX_nYYY.png`, renomeie para\n   \
            `0.png` … `9.png`, mova para `templates/digits/<nome_do_conjunto>/` e apague o resto.\n\
         4. Rode a análise: `python analyze.py VIDEO.mp4 --game {jogo} --version {versao} --player teste` — ou,\n   \
            no ShmupX Lab, adicione vídeos com gabarito em **Validação** e rode esta versão até o acerto ficar\n   \
            bom. Prefira vídeos com continue ou game over para testar essas regras também.\n\n\
         ## Estrutura de referência (do {BASE})\n\n{lista}"
    )
}

/// Copia a pasta sem seguir links simbólicos e sem os recortes brutos.
fn copiar_pasta(origem: &Path, destino: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destino)?;
    for entrada in std::fs::read_dir(origem)? {
        let entrada = entrada?;
        let tipo = std::fs::symlink_metadata(entrada.path())?.file_type();
        let alvo = destino.join(entrada.file_name());
        if tipo.is_dir() {
            if entrada.file_name() != PASTA_BRUTOS {
                copiar_pasta(&entrada.path(), &alvo)?;
            }
        } else if tipo.is_file() {
            std::fs::copy(entrada.path(), &alvo)?;
        }
    }
    Ok(())
}

/// Grava a pasta nova; se algo falhar no meio, apaga o que já foi criado.
fn criar(pasta: &Path, montar: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    if pasta.exists() {
        return Err(format!("já existe {}; o gerador não sobrescreve arquivos", pasta.display()));
    }
    let resultado = std::fs::create_dir_all(pasta)
        .map_err(|e| format!("falha ao criar {}: {e}", pasta.display()))
        .and_then(|_| montar(pasta));
    if resultado.is_err() {
        let _ = std::fs::remove_dir_all(pasta);
    }
    resultado
}

fn relativo(raiz: &Path, caminho: &Path) -> String {
    caminho.strip_prefix(raiz).unwrap_or(caminho).to_string_lossy().replace('\\', "/")
}

/// Cria o primeiro script do jogo (0.1.0, rascunho) a partir da calibração da versão mais nova do
/// Earthion. As imagens não são copiadas: as do Earthion fariam o jogo novo ser lido errado.
pub fn gerar(raiz: &Path, jogo: &str, titulo: &str) -> Result<ScriptGerado, String> {
    if !id_seguro(jogo) {
        return Err("id de jogo inválido".into());
    }
    let titulo = titulo.trim();
    if titulo.is_empty() || titulo.chars().count() > 80 {
        return Err("título do jogo inválido".into());
    }
    let raiz = raiz.canonicalize().map_err(|e| format!("pasta validators indisponível: {e}"))?;
    let pasta_jogo = raiz.join("games").join(jogo);
    if pasta_jogo.exists() {
        return Err(format!("o jogo {jogo} já tem scripts; crie uma nova versão a partir de uma existente"));
    }
    let base = versoes(&raiz, BASE).last().copied().ok_or_else(|| format!("nenhum script do {BASE} para usar como base"))?;
    let origem = raiz.join("games").join(BASE).join(texto_versao(base));

    // Tudo é montado em memória antes de gravar.
    let conteudo_manifesto = json_bonito(&json!({
        "game": jogo,
        "name": titulo,
        "version": PRIMEIRA_VERSAO,
        "status": "draft",
        "analyzer": ANALISADOR,
    }))?;
    let conteudo_config = calibracao(&origem, titulo)?;
    let referencia = imagens(&origem.join("templates"))?;

    let pasta = pasta_jogo.join(PRIMEIRA_VERSAO);
    let resultado = criar(&pasta, |pasta| {
        std::fs::write(pasta.join("script.json"), &conteudo_manifesto).map_err(|e| format!("falha ao gravar o script.json: {e}"))?;
        std::fs::write(pasta.join("config.json"), &conteudo_config).map_err(|e| format!("falha ao gravar a calibração: {e}"))?;
        let templates = pasta.join("templates");
        std::fs::create_dir_all(&templates).map_err(|e| format!("falha ao criar a pasta de imagens: {e}"))?;
        for sub in referencia.iter().filter_map(|i| i.rsplit_once('/').map(|(p, _)| p)) {
            std::fs::create_dir_all(templates.join(sub)).map_err(|e| format!("falha ao criar {sub}: {e}"))?;
        }
        std::fs::write(templates.join("LEIA-ME.md"), leia_me(jogo, titulo, PRIMEIRA_VERSAO, &referencia))
            .map_err(|e| format!("falha ao gravar o LEIA-ME: {e}"))
    });
    if let Err(erro) = resultado {
        // A pasta do jogo foi criada só para esta versão.
        let _ = std::fs::remove_dir(&pasta_jogo);
        return Err(erro);
    }
    Ok(ScriptGerado {
        versao: PRIMEIRA_VERSAO.into(),
        criados: vec![format!("{}/", relativo(&raiz, &pasta))],
        imagens_pendentes: referencia,
    })
}

/// Cria a próxima versão do script (minor seguinte à mais nova do jogo), copiando calibração e
/// imagens de `de_versao`. Nasce rascunho, com `based_on` apontando a origem.
pub fn nova_versao(raiz: &Path, jogo: &str, de_versao: &str) -> Result<ScriptGerado, String> {
    if !id_seguro(jogo) {
        return Err("id de jogo inválido".into());
    }
    if !versao_valida(de_versao) {
        return Err("versão de origem inválida".into());
    }
    let raiz = raiz.canonicalize().map_err(|e| format!("pasta validators indisponível: {e}"))?;
    let origem = raiz.join("games").join(jogo).join(de_versao);
    let texto = std::fs::read_to_string(origem.join("script.json")).map_err(|_| format!("o jogo {jogo} não tem o script {de_versao}"))?;
    let mut manifesto: Value = serde_json::from_str(&texto).map_err(|e| format!("script.json de {de_versao} inválido: {e}"))?;
    let (maior, menor, _) = versoes(&raiz, jogo).last().copied().ok_or("nenhuma versão encontrada")?;
    let versao = texto_versao((maior, menor + 1, 0));

    let campos = manifesto.as_object_mut().ok_or("script.json não é um objeto")?;
    campos.insert("version".into(), json!(versao));
    campos.insert("status".into(), json!("draft"));
    campos.insert("based_on".into(), json!(de_versao));
    let conteudo_manifesto = json_bonito(&manifesto)?;

    let pasta: PathBuf = raiz.join("games").join(jogo).join(&versao);
    criar(&pasta, |pasta| {
        copiar_pasta(&origem, pasta).map_err(|e| format!("falha ao copiar a versão {de_versao}: {e}"))?;
        std::fs::write(pasta.join("script.json"), &conteudo_manifesto).map_err(|e| format!("falha ao gravar o script.json: {e}"))
    })?;
    Ok(ScriptGerado { versao, criados: vec![format!("{}/", relativo(&raiz, &pasta))], imagens_pendentes: vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raiz_de_teste(nome: &str) -> PathBuf {
        let raiz = std::env::temp_dir().join(format!("shmupx-lab-gerador-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let base = raiz.join("games/earthion/1.0.0");
        std::fs::create_dir_all(base.join("templates/digits/pc")).unwrap();
        std::fs::create_dir_all(base.join("templates/labels")).unwrap();
        std::fs::create_dir_all(base.join("templates/raw_digits")).unwrap();
        std::fs::write(
            base.join("script.json"),
            r#"{"game": "earthion", "name": "Earthion", "version": "1.0.0", "status": "approved", "analyzer": "hud_score"}"#,
        )
        .unwrap();
        std::fs::write(base.join("config.json"), "{\"description\": \"x\", \"reference_height\": 720}").unwrap();
        std::fs::write(base.join("templates/digits/pc/0.png"), b"png").unwrap();
        std::fs::write(base.join("templates/labels/hud_score.png"), b"png").unwrap();
        std::fs::write(base.join("templates/raw_digits/cluster_00_n9.png"), b"png").unwrap();
        raiz
    }

    #[test]
    fn versoes_seguem_o_formato_do_motor() {
        assert!(versao_valida("0.1.0") && versao_valida("1.10.3"));
        for invalida in ["1.0", "1.0.0.0", "01.0.0", "../1.0.0", "1.a.0", "", "12345.0.0"] {
            assert!(!versao_valida(invalida), "{invalida}");
        }
    }

    #[test]
    fn primeiro_script_tem_calibracao_pastas_e_manifesto_sem_imagens_do_base() {
        let raiz = raiz_de_teste("primeiro");
        let gerado = gerar(&raiz, "ketsui", "Ketsui \"Kizuna\"").unwrap();
        assert_eq!(gerado.versao, "0.1.0");
        assert_eq!(gerado.criados, ["games/ketsui/0.1.0/"]);
        assert_eq!(gerado.imagens_pendentes, ["digits/pc/0.png", "labels/hud_score.png"]);

        let pasta = raiz.join("games/ketsui/0.1.0");
        let manifesto: Value = serde_json::from_str(&std::fs::read_to_string(pasta.join("script.json")).unwrap()).unwrap();
        assert_eq!(manifesto["game"], "ketsui");
        assert_eq!(manifesto["name"], "Ketsui \"Kizuna\"");
        assert_eq!(manifesto["status"], "draft");
        assert_eq!(manifesto["analyzer"], "hud_score");
        let config: Value = serde_json::from_str(&std::fs::read_to_string(pasta.join("config.json")).unwrap()).unwrap();
        assert_eq!(config["reference_height"], 720);

        assert!(pasta.join("templates/digits/pc").is_dir());
        assert!(!pasta.join("templates/digits/pc/0.png").exists(), "imagens do Earthion não são copiadas");
        let leia_me = std::fs::read_to_string(pasta.join("templates/LEIA-ME.md")).unwrap();
        assert!(leia_me.contains("tools/make_digit_templates.py VIDEO.mp4 --game ketsui --version 0.1.0"));
    }

    #[test]
    fn jogo_que_ja_tem_script_nao_e_gerado_de_novo() {
        let raiz = raiz_de_teste("repetido");
        gerar(&raiz, "dodonpachi", "DoDonPachi").unwrap();
        assert!(gerar(&raiz, "dodonpachi", "DoDonPachi").is_err());
        assert!(gerar(&raiz, "earthion", "Earthion").is_err());
    }

    #[test]
    fn nova_versao_copia_a_origem_sem_os_recortes_brutos() {
        let raiz = raiz_de_teste("nova-versao");
        let gerado = nova_versao(&raiz, "earthion", "1.0.0").unwrap();
        assert_eq!(gerado.versao, "1.1.0");
        let pasta = raiz.join("games/earthion/1.1.0");
        assert!(pasta.join("templates/digits/pc/0.png").is_file());
        assert!(!pasta.join("templates/raw_digits").exists());
        let manifesto: Value = serde_json::from_str(&std::fs::read_to_string(pasta.join("script.json")).unwrap()).unwrap();
        assert_eq!((manifesto["version"].as_str(), manifesto["status"].as_str()), (Some("1.1.0"), Some("draft")));
        assert_eq!(manifesto["based_on"], "1.0.0");
        assert_eq!(manifesto["name"], "Earthion");

        // A partir de uma versão antiga, o número segue a mais nova do jogo.
        assert_eq!(nova_versao(&raiz, "earthion", "1.0.0").unwrap().versao, "1.2.0");
        let original = std::fs::read_to_string(raiz.join("games/earthion/1.0.0/script.json")).unwrap();
        assert!(original.contains("\"approved\""), "a origem fica intacta");
    }

    #[test]
    fn nova_versao_de_origem_inexistente_e_erro() {
        let raiz = raiz_de_teste("sem-origem");
        assert!(nova_versao(&raiz, "earthion", "9.0.0").is_err());
        assert!(nova_versao(&raiz, "ketsui", "1.0.0").is_err());
        assert!(nova_versao(&raiz, "earthion", "../1.0.0").is_err());
    }

    #[test]
    fn falha_no_meio_nao_deixa_pastas_pela_metade() {
        let raiz = raiz_de_teste("falha");
        std::fs::write(raiz.join("games/earthion/1.0.0/config.json"), "não é json").unwrap();
        assert!(gerar(&raiz, "ibara", "Ibara").is_err());
        assert!(!raiz.join("games/ibara").exists());
    }

    #[test]
    fn ids_e_titulos_invalidos_sao_recusados() {
        let raiz = raiz_de_teste("invalidos");
        assert!(gerar(&raiz, "../x", "X").is_err());
        assert!(gerar(&raiz, "ok", "   ").is_err());
    }
}
