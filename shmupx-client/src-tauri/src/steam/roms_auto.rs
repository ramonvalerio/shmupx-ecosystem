//! Download automático dos ROMs dos jogos do catálogo. A lista fica em `roms.json` (na raiz do
//! `src-tauri`, embutida no Client): para cada ROM, de onde baixar e de quais outros ele depende
//! (`require`, ex.: a BIOS `neogeo`). Ao abrir o Client e ao atualizar a biblioteca do emulador, os
//! ROMs da lista que o jogador ainda não tem (em nenhuma pasta lida) são baixados para a pasta
//! padrão `roms` do Client. Um ROM já presente nunca é baixado de novo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};

const LISTA: &str = include_str!("../../roms.json");

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

fn lista() -> BTreeMap<String, Entrada> {
    serde_json::from_str(LISTA).unwrap_or_else(|e| {
        log::warn!("roms: roms.json inválido: {e}");
        BTreeMap::new()
    })
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

/// Baixa para `pasta` os ROMs da lista que não estão em `tem` (os ROMs já lidos neste PC).
pub async fn baixar_faltando(pasta: &Path, tem: &[String]) -> ResultadoDownload {
    if EM_ANDAMENTO.swap(true, Ordering::SeqCst) {
        return ResultadoDownload { em_andamento: true, ..Default::default() };
    }
    let mut resultado = ResultadoDownload::default();
    let lista = lista();
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
                    log::info!("roms: baixando {nome} para {}", destino.display());
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
    fn lista_embutida_e_valida() {
        let lista = lista();
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

    /// Baixa de verdade a lista embutida (usa a internet): `cargo test -- --ignored baixa_a_lista`.
    #[tokio::test]
    #[ignore]
    async fn baixa_a_lista() {
        let pasta = std::env::temp_dir().join(format!("shmupx-roms-teste-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let resultado = baixar_faltando(&pasta, &[]).await;
        assert!(resultado.falhas.is_empty(), "falhas: {:?}", resultado.falhas);
        assert_eq!(resultado.baixados.len(), lista().len());
        let segundo = baixar_faltando(&pasta, &resultado.baixados).await;
        assert!(segundo.baixados.is_empty());
        std::fs::remove_dir_all(&pasta).unwrap();
    }
}
