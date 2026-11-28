//! Download automático dos ROMs dos jogos do catálogo. A lista é o `roms.json` da pasta do Client
//! (ao lado da pasta `roms`), que o próprio jogador mantém: para cada ROM, de onde baixar e de quais
//! outros ele depende (`require`, ex.: a BIOS `neogeo`). Sem o arquivo, o Client cria um com a lista
//! padrão (`src-tauri/roms.json`). Ao abrir o Client e ao atualizar a biblioteca, os ROMs da lista que
//! o jogador ainda não tem (em nenhuma pasta lida) são baixados para a pasta padrão `roms` do
//! Client, só os de jogos publicados pelo Lab (a interface informa quais) e os que eles exigem. Um
//! ROM já presente nunca é baixado de novo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Lista padrão: vira o `roms.json` da pasta do Client quando ele ainda não existe.
const MODELO: &str = include_str!("../../roms.json");

/// Tamanho máximo aceito para um ROM (os maiores do Neo Geo ficam bem abaixo disso).
const TAMANHO_MAXIMO: u64 = 512 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Entrada {
    download: String,
    #[serde(default)]
    require: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ResultadoDownload {
    /// ROMs baixados agora.
    pub baixados: Vec<String>,
    /// ROMs que não deu para baixar (o motivo fica no log).
    pub falhas: Vec<String>,
    /// Já havia um download em andamento: este pedido não fez nada.
    pub em_andamento: bool,
}

static EM_ANDAMENTO: AtomicBool = AtomicBool::new(false);

/// `roms.json` da pasta do Client: ao lado da pasta `roms`.
pub fn arquivo_da_lista(pasta_roms: &Path) -> PathBuf {
    pasta_roms.parent().unwrap_or(pasta_roms).join("roms.json")
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

/// Lê o `roms.json` da pasta do Client (cria com a lista padrão se ele não existe).
fn lista(arquivo: &Path) -> BTreeMap<String, Entrada> {
    let texto = match std::fs::read_to_string(arquivo) {
        Ok(texto) => texto,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Err(e) = std::fs::write(arquivo, MODELO) {
                log::warn!("roms: não deu para criar {}: {e}", arquivo.display());
            }
            MODELO.to_string()
        }
        Err(e) => {
            log::warn!("roms: não deu para ler {}: {e}", arquivo.display());
            return BTreeMap::new();
        }
    };
    interpretar(&texto).unwrap_or_else(|e| {
        log::warn!("roms: {} inválido: {e}", arquivo.display());
        BTreeMap::new()
    })
}

/// Só os ROMs de jogos publicados (`permitidos`, da interface) e os que eles exigem.
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

/// Nome curto de ROM aceito como nome de arquivo (`sonicwi2`, `kof2k4se`…).
fn nome_valido(nome: &str) -> bool {
    !nome.is_empty() && nome.len() <= 32 && nome.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// ROMs da lista que faltam, com os exigidos por outros (`require`) antes de quem depende deles.
fn faltando(lista: &BTreeMap<String, Entrada>, tem: &[String]) -> Vec<String> {
    let exigidos: Vec<&String> = lista.values().flat_map(|e| e.require.iter()).collect();
    for nome in &exigidos {
        if !lista.contains_key(nome.as_str()) && !tem.contains(nome) {
            log::warn!("roms: {nome} é exigido na lista, mas não tem endereço de download");
        }
    }
    let mut nomes: Vec<&String> = lista.keys().filter(|n| !tem.contains(n)).collect();
    nomes.sort_by_key(|n| !exigidos.contains(n));
    nomes.into_iter().cloned().collect()
}

async fn baixar(cliente: &reqwest::Client, url: &str, destino: PathBuf) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err(format!("endereço recusado (só https): {url}"));
    }
    let resposta = cliente.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
    if resposta.content_length().is_some_and(|t| t > TAMANHO_MAXIMO) {
        return Err("arquivo grande demais".into());
    }
    let bytes = resposta.bytes().await.map_err(|e| e.to_string())?;
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
/// ROMs já lidos neste PC), só os de jogos publicados (`permitidos`) e os que eles exigem.
pub async fn baixar_faltando(pasta: &Path, tem: &[String], permitidos: &[String]) -> ResultadoDownload {
    if EM_ANDAMENTO.swap(true, Ordering::SeqCst) {
        return ResultadoDownload { em_andamento: true, ..Default::default() };
    }
    let mut resultado = ResultadoDownload::default();
    let lista = so_publicados(lista(&arquivo_da_lista(pasta)), permitidos);
    let nomes = faltando(&lista, tem);
    if !nomes.is_empty() {
        match reqwest::Client::builder()
            .user_agent("ShmupX-Client")
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(15 * 60))
            .build()
        {
            Ok(cliente) => {
                for nome in nomes {
                    if !nome_valido(&nome) {
                        log::warn!("roms: nome inválido na lista: {nome:?}");
                        resultado.falhas.push(nome);
                        continue;
                    }
                    let destino = pasta.join(format!("{nome}.zip"));
                    log::warn!("roms: baixando {nome} para {}", destino.display());
                    match baixar(&cliente, &lista[&nome].download, destino).await {
                        Ok(()) => resultado.baixados.push(nome),
                        Err(e) => {
                            log::warn!("roms: falha ao baixar {nome}: {e}");
                            resultado.falhas.push(nome);
                        }
                    }
                }
            }
            Err(e) => log::warn!("roms: falha ao preparar o download: {e}"),
        }
    }
    EM_ANDAMENTO.store(false, Ordering::SeqCst);
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lista_padrao_e_valida() {
        let lista = interpretar(MODELO).unwrap();
        assert!(!lista.is_empty());
        for (nome, entrada) in &lista {
            assert!(nome_valido(nome), "{nome}");
            assert!(entrada.download.starts_with("https://"), "{nome}");
            for exigido in &entrada.require {
                assert!(lista.contains_key(exigido), "{nome} exige {exigido}, que não está na lista");
            }
        }
    }

    #[test]
    fn exigidos_vem_antes_e_presentes_ficam_de_fora() {
        let lista: BTreeMap<String, Entrada> = serde_json::from_str(
            r#"{"sonicwi2":{"download":"https://x/a.zip","require":["neogeo"]},"cawing":{"download":"https://x/b.zip"},"neogeo":{"download":"https://x/c.zip"}}"#,
        )
        .unwrap();
        assert_eq!(faltando(&lista, &[]), ["neogeo", "cawing", "sonicwi2"]);
        assert_eq!(faltando(&lista, &["neogeo".into(), "cawing".into()]), ["sonicwi2"]);
    }

    #[test]
    fn virgulas_esquecidas_ou_sobrando_sao_aceitas() {
        let texto = "\u{feff}{\n  \"neogeo\": { \"download\": \"https://x/n.zip\" }\n  \"androdun\": { \"download\": \"https://x/a.zip\", \"require\": [\"neogeo\",], }\n}";
        let lista = interpretar(texto).unwrap();
        assert_eq!(lista.keys().cloned().collect::<Vec<_>>(), ["androdun", "neogeo"]);
        assert_eq!(lista["androdun"].require, ["neogeo"]);
        assert!(interpretar("{ isso não é json").is_err());
    }

    #[test]
    fn so_baixa_jogos_publicados_e_o_que_eles_exigem() {
        let lista: BTreeMap<String, Entrada> = serde_json::from_str(
            r#"{"sonicwi2":{"download":"https://x/a.zip","require":["neogeo"]},"neogeo":{"download":"https://x/c.zip"},"kof98":{"download":"https://x/k.zip","require":["neogeo"]}}"#,
        )
        .unwrap();
        let ficam = so_publicados(lista, &["sonicwi2".into()]);
        assert_eq!(ficam.keys().cloned().collect::<Vec<_>>(), ["neogeo", "sonicwi2"]);
    }

    #[test]
    fn cria_o_arquivo_com_a_lista_padrao() {
        let pasta = std::env::temp_dir().join(format!("shmupx-roms-lista-{}", std::process::id()));
        let roms = pasta.join("roms");
        std::fs::create_dir_all(&roms).unwrap();
        let arquivo = arquivo_da_lista(&roms);
        assert_eq!(arquivo, pasta.join("roms.json"));
        assert_eq!(lista(&arquivo).len(), interpretar(MODELO).unwrap().len());
        assert_eq!(std::fs::read_to_string(&arquivo).unwrap(), MODELO);
        std::fs::remove_dir_all(&pasta).unwrap();
    }

    /// Baixa de verdade a lista padrão (usa a internet): `cargo test -- --ignored baixa_a_lista`.
    #[tokio::test]
    #[ignore]
    async fn baixa_a_lista() {
        let pasta = std::env::temp_dir().join(format!("shmupx-roms-teste-{}", std::process::id())).join("roms");
        std::fs::create_dir_all(&pasta).unwrap();
        let todos: Vec<String> = interpretar(MODELO).unwrap().keys().cloned().collect();
        let resultado = baixar_faltando(&pasta, &[], &todos).await;
        assert!(resultado.falhas.is_empty(), "falhas: {:?}", resultado.falhas);
        assert_eq!(resultado.baixados.len(), todos.len());
        let segundo = baixar_faltando(&pasta, &resultado.baixados, &todos).await;
        assert!(segundo.baixados.is_empty());
        std::fs::remove_dir_all(pasta.parent().unwrap()).unwrap();
    }
}
