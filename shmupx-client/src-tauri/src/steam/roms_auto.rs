//! ROMs dos jogos do catálogo: o `roms.json` dentro da pasta `roms` liga o
//! **id do jogo cadastrado no Lab** (a chave) ao ROM dele. Para cada jogo: de onde baixar
//! (`download`, o `.zip`; o nome do arquivo é o nome do ROM: `.../sonicwi2.zip` → `sonicwi2`) e de
//! quais outras entradas ele depende (`require`, ex.: a BIOS `neogeo`). Jogo de console tem também
//! `system`, o sistema do MAME que abre o cartucho (`snes`, `genesis`, `megadrij`, `megadriv`); o
//! cartucho é gravado com o sistema na frente (`snes_axelayu.zip`). O Client não tem nenhum nome de
//! ROM no código: jogo fora da lista não tem ROM.
//!
//! A lista padrão (`src-tauri/roms.json`) vale sempre; o arquivo da pasta do Client (criado com ela
//! se não existe) acrescenta ou substitui entradas. Ao abrir o Client e ao atualizar a biblioteca,
//! os ROMs da lista que o jogador ainda não tem (em nenhuma pasta lida) são baixados para a pasta
//! padrão `roms` do Client, só os de jogos publicados pelo Lab (a interface informa os ids) e os que
//! eles exigem. Um ROM já presente nunca é baixado de novo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Lista padrão: vale sempre e vira o `roms.json` da pasta do Client quando ele ainda não existe.
const MODELO: &str = include_str!("../../roms.json");

/// Tamanho máximo aceito para um ROM (os maiores do Neo Geo ficam bem abaixo disso).
const TAMANHO_MAXIMO: u64 = 512 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Entrada {
    download: String,
    #[serde(default)]
    require: Vec<String>,
    /// Jogo de console: o sistema do MAME que abre o cartucho (`snes`, `genesis`, `megadrij`,
    /// `megadriv`). Ausente: jogo de arcade, aberto pelo nome do ROM.
    #[serde(default)]
    system: Option<String>,
}

/// ROM de um jogo e, se for de console, o sistema do MAME que abre o cartucho.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RomDoJogo {
    pub rom: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sistema: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ResultadoDownload {
    /// ROMs baixados agora.
    pub baixados: Vec<String>,
    /// ROMs que não deu para baixar (o motivo fica no log).
    pub falhas: Vec<String>,
    /// Já havia um download em andamento: este pedido não fez nada.
    pub em_andamento: bool,
    /// A pasta escolhida não tem `roms.json`: nada a baixar.
    pub sem_lista: bool,
}

/// Andamento do download, para a barra de progresso da interface (evento `roms://progresso`).
/// `fase`: `inicio` (já se sabe quantos arquivos), `baixando` (bytes do arquivo `indice`),
/// `arquivo` (o arquivo `indice` terminou: `ok`) e `fim`.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Progresso {
    pub fase: &'static str,
    /// Arquivos a baixar nesta vez.
    pub total: usize,
    /// Arquivo atual (a partir de 0).
    pub indice: usize,
    /// Id do jogo (chave do `roms.json`) e ROM do arquivo atual.
    pub jogo: String,
    pub rom: String,
    /// Bytes recebidos do arquivo atual e o tamanho dele, se o servidor informou.
    pub recebidos: u64,
    pub tamanho: Option<u64>,
    pub ok: Option<bool>,
}

/// Intervalo mínimo entre dois avisos de bytes recebidos (não inunda a interface).
const INTERVALO_PROGRESSO: Duration = Duration::from_millis(120);

static EM_ANDAMENTO: AtomicBool = AtomicBool::new(false);

/// `roms.json` dentro da pasta `roms` (a padrão ou a da instalação portátil).
pub fn arquivo_da_lista(pasta_roms: &Path) -> PathBuf {
    pasta_roms.join("roms.json")
}

/// Arquivo editado à mão: vírgula esquecida entre dois blocos (`}` seguido de `"`) e vírgula
/// sobrando antes de `}` ou `]`.
fn consertar(texto: &str) -> String {
    let chars: Vec<char> = texto.chars().collect();
    let proximo = |i: usize| chars[i + 1..].iter().copied().find(|c| !c.is_whitespace());
    let mut saida = String::with_capacity(texto.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        if c == ',' && matches!(proximo(i), Some('}') | Some(']')) {
            continue;
        }
        saida.push(c);
        if c == '}' && proximo(i) == Some('"') {
            saida.push(',');
        }
    }
    saida
}

fn interpretar(texto: &str) -> Result<BTreeMap<String, Entrada>, String> {
    let texto = texto.trim_start_matches('\u{feff}');
    serde_json::from_str(texto).or_else(|erro| {
        let lista = serde_json::from_str(&consertar(texto)).map_err(|_| erro.to_string())?;
        log::warn!("roms: roms.json com vírgula faltando ou sobrando; lido assim mesmo ({erro})");
        Ok(lista)
    })
}

/// A lista padrão com as entradas do `roms.json` por cima. Sem o arquivo, ilegível ou inválido:
/// só a lista padrão. Ler nunca cria o arquivo.
fn lista(arquivo: &Path) -> BTreeMap<String, Entrada> {
    let mut lista = interpretar(MODELO).unwrap_or_default();
    let texto = match std::fs::read_to_string(arquivo) {
        Ok(texto) => texto,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return lista,
        Err(e) => {
            log::warn!("roms: não deu para ler {}: {e}", arquivo.display());
            return lista;
        }
    };
    match interpretar(&texto) {
        Ok(do_arquivo) => lista.extend(do_arquivo),
        Err(e) => log::warn!("roms: {} inválido: {e}", arquivo.display()),
    }
    lista
}

/// Nome do ROM pelo endereço de download: o nome do `.zip` (`.../sonicwi2.zip` → `sonicwi2`).
fn rom_da_url(url: &str) -> Option<String> {
    let caminho = url.split(['?', '#']).next()?;
    let arquivo = caminho.rsplit('/').next()?.to_ascii_lowercase();
    let rom = arquivo.strip_suffix(".zip")?;
    nome_valido(rom).then(|| rom.to_string())
}

/// Nome curto de ROM aceito como nome de arquivo (`sonicwi2`, `kof2k4se`…).
fn nome_valido(nome: &str) -> bool {
    !nome.is_empty() && nome.len() <= 32 && nome.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Id do jogo → ROM (e sistema, se for de console), para a interface reconhecer e abrir os jogos.
/// Entrada com endereço que não termina num `.zip` com nome de ROM fica de fora (com aviso no log);
/// sistema com nome inválido é ignorado (o jogo abre como arcade).
fn roms_da_lista(lista: &BTreeMap<String, Entrada>) -> BTreeMap<String, RomDoJogo> {
    lista
        .iter()
        .filter_map(|(id, entrada)| {
            let rom = rom_da_url(&entrada.download);
            if rom.is_none() {
                log::warn!("roms: {id}: o download não termina num .zip com nome de ROM: {}", entrada.download);
            }
            let sistema = entrada.system.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_ascii_lowercase);
            let sistema = match sistema {
                Some(s) if !nome_valido(&s) => {
                    log::warn!("roms: {id}: sistema inválido: {s:?}");
                    None
                }
                outro => outro,
            };
            // Cartucho de console fica com o sistema na frente (`snes_axelayu.zip`): nomes de
            // cartucho repetem os de arcade (`truxton`, `zerowing`) e iam sobrescrever o ROM deles.
            let rom = rom.map(|rom| match &sistema {
                Some(s) => format!("{s}_{rom}"),
                None => rom,
            });
            rom.filter(|r| nome_valido(r)).map(|rom| (id.clone(), RomDoJogo { rom, sistema }))
        })
        .collect()
}

/// Id do jogo → ROM (e sistema) pelo `roms.json` da pasta do Client (com a lista padrão).
pub fn roms_dos_jogos(pasta_roms: &Path) -> BTreeMap<String, RomDoJogo> {
    roms_da_lista(&lista(&arquivo_da_lista(pasta_roms)))
}

/// Só os jogos publicados (`permitidos`: ids do catálogo, da interface) e as entradas que eles exigem.
fn so_publicados(mut lista: BTreeMap<String, Entrada>, permitidos: &[String]) -> BTreeMap<String, Entrada> {
    let mut ficam: Vec<String> = lista.keys().filter(|n| permitidos.contains(n)).cloned().collect();
    let exigidos: Vec<String> = ficam.iter().flat_map(|n| lista[n].require.clone()).collect();
    ficam.extend(exigidos);
    let fora: Vec<&String> = lista.keys().filter(|n| !ficam.contains(n)).collect();
    if !fora.is_empty() {
        // Aviso (o log do Client instalado só guarda avisos e erros): o jogador vê por que não baixou.
        log::warn!("roms: fora do download (sem jogo publicado no ShmupX): {}", fora.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", "));
    }
    lista.retain(|n, _| ficam.contains(n));
    lista
}

/// Entradas da lista cujo ROM falta (`tem`: os ROMs já lidos neste PC), com as exigidas por outras
/// (`require`) antes de quem depende delas: `(id, rom)`.
fn faltando(lista: &BTreeMap<String, Entrada>, tem: &[String]) -> Vec<(String, String)> {
    let exigidos: Vec<&String> = lista.values().flat_map(|e| e.require.iter()).collect();
    for nome in &exigidos {
        if !lista.contains_key(nome.as_str()) {
            log::warn!("roms: {nome} é exigido na lista, mas não tem endereço de download");
        }
    }
    let mut nomes: Vec<(String, String)> = roms_da_lista(lista).into_iter().map(|(id, r)| (id, r.rom)).filter(|(_, rom)| !tem.contains(rom)).collect();
    nomes.sort_by_key(|(id, _)| !exigidos.contains(&id));
    nomes
}

/// Baixa `url` para `destino`, avisando os bytes recebidos (`recebidos`, `tamanho`) em `andamento`.
async fn baixar(cliente: &reqwest::Client, url: &str, destino: PathBuf, andamento: &(dyn Fn(u64, Option<u64>) + Sync)) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err(format!("endereço recusado (só https): {url}"));
    }
    let mut resposta = cliente.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
    let tamanho = resposta.content_length();
    if tamanho.is_some_and(|t| t > TAMANHO_MAXIMO) {
        return Err("arquivo grande demais".into());
    }
    let mut bytes: Vec<u8> = Vec::with_capacity(tamanho.unwrap_or(0).min(TAMANHO_MAXIMO) as usize);
    andamento(0, tamanho);
    let mut ultimo = std::time::Instant::now();
    while let Some(parte) = resposta.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() as u64 + parte.len() as u64 > TAMANHO_MAXIMO {
            return Err("arquivo grande demais".into());
        }
        bytes.extend_from_slice(&parte);
        if ultimo.elapsed() >= INTERVALO_PROGRESSO {
            ultimo = std::time::Instant::now();
            andamento(bytes.len() as u64, tamanho);
        }
    }
    andamento(bytes.len() as u64, tamanho);
    // Um .zip começa com "PK": evita gravar uma página de erro ou de login no lugar do ROM.
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err("o arquivo baixado não é um .zip".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let parcial = destino.with_extension("zip.part");
        std::fs::write(&parcial, &bytes).and_then(|_| std::fs::rename(&parcial, &destino)).map_err(|e| {
            let _ = std::fs::remove_file(&parcial);
            format!("não foi possível gravar {}: {e}", destino.display())
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Baixa para `pasta` os ROMs da lista (`roms.json` da pasta do Client) que não estão em `tem` (os
/// ROMs já lidos neste PC), só os de jogos publicados (`permitidos`: ids) e os que eles exigem.
/// Sem `roms.json` na pasta: com `criar_lista` (a pasta padrão), cria com a lista padrão; senão (a
/// pasta escolhida pelo jogador), não baixa nada (`sem_lista`).
/// O andamento vai para `progresso` (sempre termina com a fase `fim`, mesmo sem nada a baixar).
pub async fn baixar_faltando(pasta: &Path, tem: &[String], permitidos: &[String], criar_lista: bool, progresso: &(dyn Fn(Progresso) + Sync)) -> ResultadoDownload {
    // Lista e ROMs baixados ficam na mesma pasta: a do `roms.json`.
    let arquivo = arquivo_da_lista(pasta);
    if !arquivo.is_file() {
        if !criar_lista {
            log::warn!("roms: sem roms.json em {}; nada a baixar", pasta.display());
            progresso(Progresso { fase: "inicio", ..Default::default() });
            progresso(Progresso { fase: "fim", ..Default::default() });
            return ResultadoDownload { sem_lista: true, ..Default::default() };
        }
        if let Err(e) = std::fs::write(&arquivo, MODELO) {
            log::warn!("roms: não deu para criar {}: {e}", arquivo.display());
        }
    }
    if EM_ANDAMENTO.swap(true, Ordering::SeqCst) {
        return ResultadoDownload { em_andamento: true, ..Default::default() };
    }
    let mut resultado = ResultadoDownload::default();
    let lista = so_publicados(lista(&arquivo), permitidos);
    let nomes = faltando(&lista, tem);
    let total = nomes.len();
    progresso(Progresso { fase: "inicio", total, ..Default::default() });
    if !nomes.is_empty() {
        match reqwest::Client::builder()
            .user_agent("ShmupX-Client")
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(15 * 60))
            .build()
        {
            Ok(cliente) => {
                for (indice, (id, rom)) in nomes.into_iter().enumerate() {
                    let destino = pasta.join(format!("{rom}.zip"));
                    log::warn!("roms: baixando {rom} ({id}) para {}", destino.display());
                    let base = Progresso { total, indice, jogo: id.clone(), rom: rom.clone(), ..Default::default() };
                    let andamento = |recebidos: u64, tamanho: Option<u64>| progresso(Progresso { fase: "baixando", recebidos, tamanho, ..base.clone() });
                    let ok = match baixar(&cliente, &lista[&id].download, destino, &andamento).await {
                        Ok(()) => {
                            resultado.baixados.push(rom);
                            true
                        }
                        Err(e) => {
                            log::warn!("roms: falha ao baixar {rom} ({id}): {e}");
                            resultado.falhas.push(rom);
                            false
                        }
                    };
                    progresso(Progresso { fase: "arquivo", ok: Some(ok), ..base });
                }
            }
            Err(e) => log::warn!("roms: falha ao preparar o download: {e}"),
        }
    }
    EM_ANDAMENTO.store(false, Ordering::SeqCst);
    progresso(Progresso { fase: "fim", total, indice: total, ..Default::default() });
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;

    fn de(texto: &str) -> BTreeMap<String, Entrada> {
        serde_json::from_str(texto).unwrap()
    }

    #[test]
    fn lista_padrao_e_valida() {
        let lista = interpretar(MODELO).unwrap();
        assert!(!lista.is_empty());
        for (id, entrada) in &lista {
            assert!(entrada.download.starts_with("https://"), "{id}");
            assert!(rom_da_url(&entrada.download).is_some(), "{id}: {}", entrada.download);
            for exigido in &entrada.require {
                assert!(lista.contains_key(exigido), "{id} exige {exigido}, que não está na lista");
            }
        }
    }

    #[test]
    fn rom_vem_do_nome_do_zip() {
        assert_eq!(rom_da_url("https://archive.org/download/fightcaderoms/sonicwi2.zip").as_deref(), Some("sonicwi2"));
        assert_eq!(rom_da_url("https://x/y/DDonPach.ZIP?download=1#a").as_deref(), Some("ddonpach"));
        assert_eq!(rom_da_url("https://x/y/aero-fighters-2.zip"), None);
        assert_eq!(rom_da_url("https://x/y/sonicwi2.7z"), None);
        assert_eq!(rom_da_url("https://x/"), None);
    }

    #[test]
    fn id_do_jogo_liga_ao_rom() {
        let lista = de(
            r#"{"aero-fighters-2":{"download":"https://x/sonicwi2.zip","require":["neogeo"]},"neogeo":{"download":"https://x/neogeo.zip"},"ruim":{"download":"https://x/pagina"},"axelay":{"download":"https://x/axelayu.zip","system":" SNES "},"x":{"download":"https://x/x.zip","system":"../a"}}"#,
        );
        let roms = roms_da_lista(&lista);
        assert_eq!(roms["aero-fighters-2"], RomDoJogo { rom: "sonicwi2".into(), sistema: None });
        assert_eq!(roms["neogeo"].rom, "neogeo");
        assert!(!roms.contains_key("ruim"));
        assert_eq!(roms["axelay"], RomDoJogo { rom: "snes_axelayu".into(), sistema: Some("snes".into()) });
        assert_eq!(roms["x"].sistema, None, "sistema inválido é ignorado");
    }

    #[test]
    fn exigidos_vem_antes_e_presentes_ficam_de_fora() {
        let lista = de(
            r#"{"aero-fighters-2":{"download":"https://x/sonicwi2.zip","require":["neogeo"]},"carrier-air-wing":{"download":"https://x/cawing.zip"},"neogeo":{"download":"https://x/neogeo.zip"}}"#,
        );
        let par = |id: &str, rom: &str| (id.to_string(), rom.to_string());
        assert_eq!(faltando(&lista, &[]), [par("neogeo", "neogeo"), par("aero-fighters-2", "sonicwi2"), par("carrier-air-wing", "cawing")]);
        assert_eq!(faltando(&lista, &["neogeo".into(), "cawing".into()]), [par("aero-fighters-2", "sonicwi2")]);
    }

    #[test]
    fn virgulas_esquecidas_ou_sobrando_sao_aceitas() {
        let texto = "\u{feff}{\n  \"neogeo\": { \"download\": \"https://x/n.zip\" }\n  \"andro-dunos\": { \"download\": \"https://x/a.zip\", \"require\": [\"neogeo\",], }\n}";
        let lista = interpretar(texto).unwrap();
        assert_eq!(lista.keys().cloned().collect::<Vec<_>>(), ["andro-dunos", "neogeo"]);
        assert_eq!(lista["andro-dunos"].require, ["neogeo"]);
        assert!(interpretar("{ isso não é json").is_err());
    }

    #[test]
    fn so_baixa_jogos_publicados_e_o_que_eles_exigem() {
        let lista = de(
            r#"{"aero-fighters-2":{"download":"https://x/sonicwi2.zip","require":["neogeo"]},"neogeo":{"download":"https://x/neogeo.zip"},"kof98":{"download":"https://x/kof98.zip","require":["neogeo"]}}"#,
        );
        let ficam = so_publicados(lista, &["aero-fighters-2".into()]);
        assert_eq!(ficam.keys().cloned().collect::<Vec<_>>(), ["aero-fighters-2", "neogeo"]);
    }

    #[test]
    fn sem_arquivo_vale_a_lista_padrao_e_o_arquivo_acrescenta() {
        let pasta = std::env::temp_dir().join(format!("shmupx-roms-lista-{}", std::process::id()));
        let roms = pasta.join("roms");
        std::fs::create_dir_all(&roms).unwrap();
        let arquivo = arquivo_da_lista(&roms);
        assert_eq!(arquivo, roms.join("roms.json"));
        assert_eq!(lista(&arquivo).len(), interpretar(MODELO).unwrap().len());
        assert!(!arquivo.exists(), "ler a lista não cria o arquivo");
        // Arquivo antigo (chaves pelo ROM) ou do jogador: a lista padrão continua valendo.
        std::fs::write(&arquivo, r#"{"meu-jogo":{"download":"https://x/meujogo.zip"}}"#).unwrap();
        let roms_dos = roms_dos_jogos(&roms);
        assert_eq!(roms_dos.get("meu-jogo").map(|r| r.rom.as_str()), Some("meujogo"));
        assert_eq!(roms_dos.len(), interpretar(MODELO).unwrap().len() + 1);
        std::fs::remove_dir_all(&pasta).unwrap();
    }

    /// Baixa de verdade a lista padrão (usa a internet): `cargo test -- --ignored baixa_a_lista`.
    #[tokio::test]
    #[ignore]
    async fn baixa_a_lista() {
        let pasta = std::env::temp_dir().join(format!("shmupx-roms-teste-{}", std::process::id())).join("roms");
        std::fs::create_dir_all(&pasta).unwrap();
        let todos: Vec<String> = interpretar(MODELO).unwrap().keys().cloned().collect();
        let fases = std::sync::Mutex::new(Vec::new());
        let resultado = baixar_faltando(&pasta, &[], &todos, true, &|p| fases.lock().unwrap().push(p.fase)).await;
        assert!(resultado.falhas.is_empty(), "falhas: {:?}", resultado.falhas);
        assert_eq!(resultado.baixados.len(), todos.len());
        let fases = fases.into_inner().unwrap();
        assert_eq!(fases.first(), Some(&"inicio"));
        assert_eq!(fases.last(), Some(&"fim"));
        assert_eq!(fases.iter().filter(|f| **f == "arquivo").count(), todos.len());
        assert!(fases.contains(&"baixando"));
        let segundo = baixar_faltando(&pasta, &resultado.baixados, &todos, true, &|_| {}).await;
        assert!(segundo.baixados.is_empty());
        // Pasta escolhida sem roms.json: não baixa nada nem cria o arquivo.
        let escolhida = pasta.parent().unwrap().join("escolhida");
        std::fs::create_dir_all(&escolhida).unwrap();
        let sem = baixar_faltando(&escolhida, &[], &todos, false, &|_| {}).await;
        assert!(sem.sem_lista && sem.baixados.is_empty());
        assert!(!arquivo_da_lista(&escolhida).exists());
        std::fs::remove_dir_all(pasta.parent().unwrap()).unwrap();
    }
}
