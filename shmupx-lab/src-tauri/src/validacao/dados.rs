//! Banco de testes do validador de cada jogo: os vídeos, o gabarito que o admin informa e o
//! resultado do script. Um arquivo JSON por jogo na pasta de dados do Lab (material de trabalho
//! local; nada disso vai para o Firestore).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn agora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Identifica esta execução do Lab. Um estado transitório (baixando, na fila, analisando)
/// gravado por outra execução foi interrompido quando o Lab fechou.
pub fn sessao() -> u64 {
    static SESSAO: OnceLock<u64> = OnceLock::new();
    *SESSAO.get_or_init(|| {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
    })
}

/// O `gameId` vira nome de arquivo e de pasta: só o formato que o catálogo aceita.
pub fn id_seguro(id: &str) -> bool {
    (1..=128).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Origem {
    Youtube { url: String },
    Arquivo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "fase", rename_all = "snake_case")]
pub enum Estado {
    Baixando { progresso: f32, sessao: u64 },
    /// Vídeo disponível, ainda sem análise.
    Pronto,
    /// `ordem`: posição de chegada na fila (crescente); a fila sempre roda a menor primeiro.
    NaFila {
        sessao: u64,
        #[serde(default)]
        ordem: u64,
    },
    /// `progresso` de 0 a 100, publicado pelo script (`--progress`).
    Analisando {
        #[serde(default)]
        progresso: f32,
        sessao: u64,
    },
    /// Análise terminada: o resultado (ou o erro do script) está em `Video::resultado`.
    Concluido,
    /// O vídeo não ficou disponível (download falhou ou foi interrompido).
    Falhou { erro: String },
}

/// Valores reais da gameplay, informados pelo admin. Vidas e continues são opcionais.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Gabarito {
    pub placar: Option<i64>,
    pub vidas: Option<i32>,
    pub continues: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resultado {
    pub placar: Option<i64>,
    pub vidas: Option<i32>,
    pub continues: Option<i32>,
    pub game_over: bool,
    /// Jogadores na partida (2 = cooperativa); `None` em resultados antigos ou scripts que não sabem.
    #[serde(default)]
    pub jogadores: Option<u8>,
    /// A volta foi completada (tela de final ou de resultado); `None` em resultados antigos.
    #[serde(default)]
    pub zerado: Option<bool>,
    /// Bombas gastas, quando o script do jogo conta bombas; `None` nos demais.
    #[serde(default)]
    pub bombas: Option<i32>,
    pub confianca: String,
    pub revisao: bool,
    pub observacao: Option<String>,
    pub versao_validador: Option<String>,
    /// O Client aceitaria esta run sem revisão (mesma regra de `analysis/offline.rs`).
    pub aceito_automatico: bool,
    /// O script falhou (processo com erro ou relatório inválido).
    pub erro: Option<String>,
    /// Relatório completo do script, para inspeção.
    pub relatorio: Option<Value>,
    pub em: u64,
    pub duracao_segundos: u64,
}

impl Resultado {
    pub fn falha(erro: String, duracao_segundos: u64) -> Self {
        Self {
            placar: None,
            vidas: None,
            continues: None,
            game_over: false,
            confianca: String::new(),
            revisao: true,
            observacao: None,
            versao_validador: None,
            jogadores: None,
            zerado: None,
            bombas: None,
            aceito_automatico: false,
            erro: Some(erro),
            relatorio: None,
            em: agora(),
            duracao_segundos,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub titulo: String,
    pub origem: Origem,
    /// Arquivo de vídeo no disco (baixado, ou o original quando veio de arquivo local).
    pub caminho: Option<String>,
    pub adicionado_em: u64,
    /// Duração do vídeo, quando conhecida (metadados do YouTube ou leitura do arquivo).
    #[serde(default)]
    pub duracao_segundos: Option<u64>,
    #[serde(default)]
    pub gabarito: Gabarito,
    pub estado: Estado,
    pub resultado: Option<Resultado>,
}

impl Video {
    /// O placar lido bate com o gabarito (e vidas/continues, se informados). `None` quando falta
    /// gabarito ou análise. Um script que falhou conta como erro.
    pub fn acertou(&self) -> Option<bool> {
        let esperado = self.gabarito.placar?;
        let resultado = self.resultado.as_ref()?;
        if resultado.erro.is_some() {
            return Some(false);
        }
        let vidas_ok = self.gabarito.vidas.is_none_or(|vidas| resultado.vidas == Some(vidas));
        let continues_ok = self.gabarito.continues.is_none_or(|c| resultado.continues == Some(c));
        Some(resultado.placar == Some(esperado) && vidas_ok && continues_ok)
    }

    /// Estados transitórios gravados por outra execução do Lab foram interrompidos.
    fn recuperar_interrompido(&mut self, sessao_atual: u64) -> bool {
        let novo = match &self.estado {
            Estado::Baixando { sessao, .. } if *sessao != sessao_atual => {
                Estado::Falhou { erro: "download interrompido: o Lab foi fechado".into() }
            }
            Estado::NaFila { sessao, .. } | Estado::Analisando { sessao, .. } if *sessao != sessao_atual => {
                if self.resultado.is_some() { Estado::Concluido } else { Estado::Pronto }
            }
            _ => return false,
        };
        self.estado = novo;
        true
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    pub videos: Vec<Video>,
}

impl Dataset {
    /// Próximo vídeo da fila de análise (menor `ordem`) e o arquivo dele.
    pub fn proximo_da_fila(&self) -> Option<(String, String)> {
        self.videos
            .iter()
            .filter_map(|v| match v.estado {
                Estado::NaFila { ordem, .. } => Some((ordem, v)),
                _ => None,
            })
            .filter(|(_, v)| v.caminho.is_some())
            .min_by_key(|(ordem, _)| *ordem)
            .and_then(|(_, v)| Some((v.id.clone(), v.caminho.clone()?)))
    }
}

/// Números do topo da tela.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Resumo {
    pub total: usize,
    /// Vídeos com análise concluída (inclui scripts que falharam).
    pub analisados: usize,
    /// Analisados que têm gabarito: a base do percentual de acerto.
    pub com_gabarito: usize,
    pub acertos: usize,
    /// Analisados que o Client aceitaria sem revisão.
    pub aceitos: usize,
}

pub fn resumo(dataset: &Dataset) -> Resumo {
    let analisados: Vec<&Video> = dataset.videos.iter().filter(|v| v.resultado.is_some()).collect();
    let veredictos: Vec<bool> = analisados.iter().filter_map(|v| v.acertou()).collect();
    Resumo {
        total: dataset.videos.len(),
        analisados: analisados.len(),
        com_gabarito: veredictos.len(),
        acertos: veredictos.iter().filter(|&&ok| ok).count(),
        aceitos: analisados.iter().filter(|v| v.resultado.as_ref().is_some_and(|r| r.aceito_automatico)).count(),
    }
}

/// Arquivos do banco de testes dentro da pasta de dados do Lab.
pub struct Repositorio {
    raiz: PathBuf,
}

impl Repositorio {
    pub fn new(raiz: PathBuf) -> Self {
        Self { raiz }
    }

    fn arquivo(&self, jogo: &str) -> PathBuf {
        self.raiz.join("datasets").join(format!("{jogo}.json"))
    }

    pub fn pasta_videos(&self, jogo: &str, video: &str) -> PathBuf {
        self.raiz.join("videos").join(jogo).join(video)
    }

    /// Lê o banco do jogo e recupera estados interrompidos. Arquivo ausente = banco vazio; arquivo
    /// ilegível é erro (não apagamos o trabalho do admin em silêncio).
    pub fn carregar(&self, jogo: &str) -> Result<Dataset, String> {
        let caminho = self.arquivo(jogo);
        let mut dataset: Dataset = match std::fs::read(&caminho) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| format!("banco de testes ilegível em {}: {e}", caminho.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Dataset::default(),
            Err(e) => return Err(format!("falha ao ler o banco de testes: {e}")),
        };
        let atual = sessao();
        let mudou = dataset.videos.iter_mut().fold(false, |mudou, v| v.recuperar_interrompido(atual) | mudou);
        if mudou {
            self.salvar(jogo, &dataset)?;
        }
        Ok(dataset)
    }

    /// Grava num temporário e renomeia: um corte no meio não corrompe o banco.
    pub fn salvar(&self, jogo: &str, dataset: &Dataset) -> Result<(), String> {
        let caminho = self.arquivo(jogo);
        if let Some(pasta) = caminho.parent() {
            std::fs::create_dir_all(pasta).map_err(|e| format!("falha ao criar a pasta do banco: {e}"))?;
        }
        let temporario = caminho.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(dataset).map_err(|e| format!("falha ao serializar o banco: {e}"))?;
        std::fs::write(&temporario, bytes).map_err(|e| format!("falha ao gravar o banco: {e}"))?;
        std::fs::rename(&temporario, &caminho).map_err(|e| format!("falha ao gravar o banco: {e}"))
    }

    /// A pasta é nossa (vídeos baixados): pode ser apagada ao remover o vídeo.
    pub fn e_pasta_propria(&self, caminho: &Path) -> bool {
        caminho.starts_with(self.raiz.join("videos"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resultado(placar: Option<i64>, vidas: Option<i32>, aceito: bool) -> Resultado {
        Resultado {
            placar,
            vidas,
            continues: Some(0),
            game_over: false,
            confianca: "high".into(),
            revisao: false,
            observacao: None,
            versao_validador: Some("1.0.0".into()),
            jogadores: None,
            zerado: None,
            bombas: None,
            aceito_automatico: aceito,
            erro: None,
            relatorio: None,
            em: 0,
            duracao_segundos: 1,
        }
    }

    fn video(gabarito: Gabarito, resultado: Option<Resultado>) -> Video {
        Video {
            id: "v".into(),
            titulo: "t".into(),
            origem: Origem::Arquivo,
            caminho: Some("x.mp4".into()),
            adicionado_em: 0,
            duracao_segundos: None,
            gabarito,
            estado: if resultado.is_some() { Estado::Concluido } else { Estado::Pronto },
            resultado,
        }
    }

    #[test]
    fn fila_roda_primeiro_a_menor_ordem() {
        let na_fila = |id: &str, ordem: u64| Video { id: id.into(), estado: Estado::NaFila { sessao: 1, ordem }, ..video(Gabarito::default(), None) };
        let mut dataset = Dataset { videos: vec![na_fila("c", 7), video(Gabarito::default(), None), na_fila("a", 3), na_fila("b", 5)] };
        assert_eq!(dataset.proximo_da_fila(), Some(("a".into(), "x.mp4".into())));
        dataset.videos[2].estado = Estado::Concluido;
        assert_eq!(dataset.proximo_da_fila().map(|(id, _)| id).as_deref(), Some("b"));
        dataset.videos.retain(|v| !matches!(v.estado, Estado::NaFila { .. }));
        assert_eq!(dataset.proximo_da_fila(), None);
    }

    #[test]
    fn acerto_compara_placar_e_o_que_mais_foi_informado() {
        let g = |placar, vidas| Gabarito { placar: Some(placar), vidas, continues: None };
        assert_eq!(video(g(1000, None), Some(resultado(Some(1000), Some(3), true))).acertou(), Some(true));
        assert_eq!(video(g(1000, None), Some(resultado(Some(999), Some(3), true))).acertou(), Some(false));
        assert_eq!(video(g(1000, Some(2)), Some(resultado(Some(1000), Some(3), true))).acertou(), Some(false));
        assert_eq!(video(g(1000, Some(3)), Some(resultado(Some(1000), Some(3), true))).acertou(), Some(true));
    }

    #[test]
    fn sem_gabarito_ou_sem_analise_nao_ha_veredito_e_falha_do_script_e_erro() {
        assert_eq!(video(Gabarito::default(), Some(resultado(Some(1), None, true))).acertou(), None);
        assert_eq!(video(Gabarito { placar: Some(1), ..Default::default() }, None).acertou(), None);
        let falha = Resultado::falha("crash".into(), 0);
        assert_eq!(video(Gabarito { placar: Some(1), ..Default::default() }, Some(falha)).acertou(), Some(false));
    }

    #[test]
    fn resumo_conta_acertos_e_aceitacao() {
        let g = Gabarito { placar: Some(100), ..Default::default() };
        let dataset = Dataset {
            videos: vec![
                video(g.clone(), Some(resultado(Some(100), None, true))),
                video(g.clone(), Some(resultado(Some(90), None, false))),
                video(Gabarito::default(), Some(resultado(Some(5), None, true))),
                video(g, None),
            ],
        };
        assert_eq!(resumo(&dataset), Resumo { total: 4, analisados: 3, com_gabarito: 2, acertos: 1, aceitos: 2 });
    }

    #[test]
    fn estados_de_outra_execucao_sao_recuperados() {
        let mut baixando = video(Gabarito::default(), None);
        baixando.estado = Estado::Baixando { progresso: 40.0, sessao: 1 };
        assert!(baixando.recuperar_interrompido(2));
        assert!(matches!(baixando.estado, Estado::Falhou { .. }));

        let mut analisando = video(Gabarito::default(), Some(resultado(Some(1), None, true)));
        analisando.estado = Estado::Analisando { progresso: 30.0, sessao: 1 };
        assert!(analisando.recuperar_interrompido(2));
        assert_eq!(analisando.estado, Estado::Concluido);

        let mut na_fila = video(Gabarito::default(), None);
        na_fila.estado = Estado::NaFila { sessao: 2, ordem: 1 };
        assert!(!na_fila.recuperar_interrompido(2), "a execução atual não é tocada");
    }

    #[test]
    fn ids_seguros_para_caminho() {
        assert!(id_seguro("earthion"));
        assert!(!id_seguro("../x"));
        assert!(!id_seguro("a\\b"));
        assert!(!id_seguro(""));
    }

    #[test]
    fn repositorio_grava_le_e_recupera() {
        let raiz = std::env::temp_dir().join(format!("shmupx-lab-validacao-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let repo = Repositorio::new(raiz.clone());
        assert_eq!(repo.carregar("earthion").unwrap(), Dataset::default());

        let mut v = video(Gabarito::default(), None);
        v.estado = Estado::Baixando { progresso: 10.0, sessao: 0 };
        repo.salvar("earthion", &Dataset { videos: vec![v] }).unwrap();
        let lido = repo.carregar("earthion").unwrap();
        assert!(matches!(lido.videos[0].estado, Estado::Falhou { .. }), "sessão 0 é de outra execução");

        std::fs::write(raiz.join("datasets").join("quebrado.json"), b"{").unwrap();
        assert!(repo.carregar("quebrado").is_err());
        assert!(repo.e_pasta_propria(&repo.pasta_videos("earthion", "abc")));
        assert!(!repo.e_pasta_propria(Path::new("C:\\Users\\x\\video.mp4")));
    }
}
