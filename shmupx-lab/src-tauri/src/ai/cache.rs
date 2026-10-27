//! Cache local das sugestões: o mesmo título não é pago duas vezes. Um arquivo JSON na pasta de
//! dados do app, limitado em tamanho; se ele sumir ou corromper, o cache simplesmente recomeça.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::sugestao::Sugestao;

const MAX_ENTRADAS: usize = 200;

#[derive(Debug, Serialize, Deserialize)]
struct Entrada {
    /// Veio de uma pesquisa na web (mais confiável que só o conhecimento do modelo).
    web: bool,
    /// Segundos desde 1970; serve para descartar as mais antigas.
    em: u64,
    sugestoes: Vec<Sugestao>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Arquivo {
    entradas: BTreeMap<String, Entrada>,
}

pub struct Cache {
    caminho: PathBuf,
}

/// Chave estável do título: minúsculas, só letras e números, espaços colapsados. "Dodonpachi!" e
/// "dodonpachi" caem na mesma entrada.
pub fn chave(titulo: &str) -> String {
    titulo
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn agora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Cache {
    pub fn new(caminho: PathBuf) -> Self {
        Self { caminho }
    }

    fn ler(&self) -> Arquivo {
        std::fs::read(&self.caminho)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Resposta guardada. Um pedido com busca na web só aceita entradas que vieram da web; um
    /// pedido sem ela aceita qualquer uma (a da web é melhor).
    pub fn buscar(&self, titulo: &str, web: bool) -> Option<Vec<Sugestao>> {
        let mut arquivo = self.ler();
        let entrada = arquivo.entradas.remove(&chave(titulo))?;
        (entrada.web || !web).then_some(entrada.sugestoes)
    }

    /// Guarda o resultado. Respostas vazias não entram: o jogo pode ser encontrado depois.
    pub fn guardar(&self, titulo: &str, web: bool, sugestoes: &[Sugestao]) -> Result<(), String> {
        let chave = chave(titulo);
        if chave.is_empty() || sugestoes.is_empty() {
            return Ok(());
        }
        let mut arquivo = self.ler();
        arquivo.entradas.insert(chave, Entrada { web, em: agora(), sugestoes: sugestoes.to_vec() });

        while arquivo.entradas.len() > MAX_ENTRADAS {
            let Some(mais_antiga) = arquivo.entradas.iter().min_by_key(|(_, e)| e.em).map(|(k, _)| k.clone()) else {
                break;
            };
            arquivo.entradas.remove(&mais_antiga);
        }

        if let Some(pasta) = self.caminho.parent() {
            std::fs::create_dir_all(pasta).map_err(|e| format!("falha ao criar a pasta do cache: {e}"))?;
        }
        // Grava num arquivo temporário e renomeia: um corte no meio não deixa o cache pela metade.
        let temporario = self.caminho.with_extension("json.tmp");
        let bytes = serde_json::to_vec(&arquivo).map_err(|e| format!("falha ao serializar o cache: {e}"))?;
        std::fs::write(&temporario, bytes).map_err(|e| format!("falha ao gravar o cache: {e}"))?;
        std::fs::rename(&temporario, &self.caminho).map_err(|e| format!("falha ao gravar o cache: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sugestao(nome: &str) -> Sugestao {
        Sugestao {
            name: nome.into(),
            developer: None,
            publisher: None,
            genre: None,
            year: None,
            platform: None,
            steam_app_id: None,
            modes: vec![],
        }
    }

    fn cache(nome: &str) -> Cache {
        let pasta = std::env::temp_dir().join(format!("shmupx-lab-cache-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        Cache::new(pasta.join("ai_cache.json"))
    }

    #[test]
    fn chave_ignora_caixa_pontuacao_e_espacos() {
        assert_eq!(chave("  DoDonPachi!! "), "dodonpachi");
        assert_eq!(chave("Mushihime-sama  Futari"), "mushihime sama futari");
        assert_eq!(chave("***"), "");
    }

    #[test]
    fn guarda_e_recupera() {
        let cache = cache("basico");
        assert!(cache.buscar("Earthion", false).is_none());
        cache.guardar("Earthion", false, &[sugestao("Earthion")]).unwrap();
        assert_eq!(cache.buscar("earthion!", false).unwrap()[0].name, "Earthion");
    }

    #[test]
    fn busca_na_web_nao_aceita_entrada_sem_web() {
        let cache = cache("web");
        cache.guardar("Ibara", false, &[sugestao("Ibara")]).unwrap();
        assert!(cache.buscar("Ibara", true).is_none());
        cache.guardar("Ibara", true, &[sugestao("Ibara (web)")]).unwrap();
        assert!(cache.buscar("Ibara", true).is_some());
        assert!(cache.buscar("Ibara", false).is_some(), "sem web aceita a da web");
    }

    #[test]
    fn resposta_vazia_nao_e_guardada() {
        let cache = cache("vazio");
        cache.guardar("Nada", false, &[]).unwrap();
        assert!(cache.buscar("Nada", false).is_none());
        assert!(!cache.caminho.exists());
    }

    #[test]
    fn arquivo_corrompido_recomeca_vazio() {
        let cache = cache("corrompido");
        std::fs::create_dir_all(cache.caminho.parent().unwrap()).unwrap();
        std::fs::write(&cache.caminho, b"{ isto nao e json").unwrap();
        assert!(cache.buscar("Qualquer", false).is_none());
        cache.guardar("Qualquer", false, &[sugestao("Qualquer")]).unwrap();
        assert!(cache.buscar("Qualquer", false).is_some());
    }

    #[test]
    fn descarta_as_entradas_mais_antigas_acima_do_limite() {
        let cache = cache("limite");
        for i in 0..(MAX_ENTRADAS + 5) {
            cache.guardar(&format!("jogo {i}"), false, &[sugestao("x")]).unwrap();
        }
        assert!(cache.ler().entradas.len() <= MAX_ENTRADAS);
    }
}
