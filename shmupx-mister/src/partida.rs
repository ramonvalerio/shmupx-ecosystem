//! Registro de uma partida: o que o serviço grava ao fim de cada jogo.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MODO_PADRAO: &str = "hi-score";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Partida {
    pub jogo: String,
    /// Modo do ShmupX: `hi-score`, `1cc` ou `caravan`.
    #[serde(default = "modo_padrao")]
    pub modo: String,
    /// Nome configurado no MiSTer quando a partida foi jogada. Partidas antigas não têm.
    #[serde(default)]
    pub jogador: Option<String>,
    pub core: String,
    /// Segundos desde 1970 (UTC), pelo relógio do MiSTer.
    pub inicio: u64,
    pub fim: u64,
    pub placar_final: Option<u64>,
    pub placar_maximo: Option<u64>,
    /// Quadros do FPGA em que o placar foi lido.
    pub quadros: u32,
    /// Partida aberta no backend (`/api/runs/start`), se houve envio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Situação do envio: `enviada`, `pendente`, `recusada: ...` ou `local: <motivo>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envio: Option<String>,
}

fn modo_padrao() -> String {
    MODO_PADRAO.to_owned()
}

/// Quadros do FPGA que o modo Caravan permite (60 por segundo). Outros modos não têm limite.
pub fn limite_em_quadros(modo: &str) -> Option<u32> {
    match modo {
        "caravan-sprint" => Some(2 * 60 * 60),
        "caravan-classic" => Some(5 * 60 * 60),
        _ => None,
    }
}

impl Partida {
    pub fn nova(jogo: &str, modo: &str, jogador: &str, core: &str) -> Self {
        let agora = agora();
        Self {
            jogo: jogo.to_owned(),
            modo: modo.to_owned(),
            jogador: Some(jogador.to_owned()),
            core: core.to_owned(),
            inicio: agora,
            fim: agora,
            placar_final: None,
            placar_maximo: None,
            quadros: 0,
            run_id: None,
            envio: None,
        }
    }

    pub fn registrar(&mut self, placar: u64) {
        self.placar_final = Some(placar);
        self.placar_maximo = self.placar_maximo.max(Some(placar));
    }

    /// Partida que vale guardar: chegou a pontuar. Abrir o jogo e voltar ao menu na
    /// tela-título não conta.
    pub fn tem_placar(&self) -> bool {
        self.placar_maximo.is_some_and(|p| p > 0)
    }

    pub fn encerrar(&mut self) {
        self.fim = agora();
    }

    /// Grava em `<pasta>/<jogo>-<inicio>.json`.
    pub fn salvar(&self, pasta: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(pasta)?;
        let caminho = pasta.join(format!("{}-{}.json", self.jogo, self.inicio));
        let json = serde_json::to_string_pretty(self).expect("Partida sempre serializa");
        std::fs::write(caminho, json)
    }

    /// Todas as partidas gravadas na pasta (arquivos ilegíveis são ignorados).
    pub fn carregar_todas(pasta: &Path) -> Vec<Partida> {
        let Ok(entradas) = std::fs::read_dir(pasta) else {
            return Vec::new();
        };
        entradas
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .filter_map(|texto| serde_json::from_str(&texto).ok())
            .collect()
    }
}

fn agora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn guarda_o_ultimo_e_o_maior_placar() {
        let mut partida = Partida::nova("earthion", "hi-score", "RAMON", "MegaDrive");
        assert!(!partida.tem_placar());
        partida.registrar(0);
        assert!(!partida.tem_placar()); // só a tela-título

        partida.registrar(5_000);
        partida.registrar(12_000);
        partida.registrar(0); // voltou para a tela-título

        assert_eq!(partida.placar_final, Some(0));
        assert_eq!(partida.placar_maximo, Some(12_000));
        assert!(partida.tem_placar());
    }

    #[test]
    fn caravan_tem_tempo_e_os_outros_modos_nao() {
        assert_eq!(limite_em_quadros("caravan-sprint"), Some(7_200));
        assert_eq!(limite_em_quadros("caravan-classic"), Some(18_000));
        assert_eq!(limite_em_quadros("hi-score"), None);
        assert_eq!(limite_em_quadros("1cc"), None);
    }

    #[test]
    fn salva_e_carrega_as_partidas() {
        let pasta = std::env::temp_dir().join(format!("shmupx-mister-teste-{}", std::process::id()));
        let mut partida = Partida::nova("earthion", "hi-score", "RAMON", "MegaDrive");
        partida.registrar(300);

        partida.salvar(&pasta).unwrap();

        assert_eq!(Partida::carregar_todas(&pasta), vec![partida]);
        std::fs::remove_dir_all(pasta).unwrap();
    }

    #[test]
    fn le_partida_antiga_sem_jogador_nem_modo() {
        // Formato gravado antes de existir jogador e modo (a primeira partida do MiSTer).
        let antiga = r#"{"jogo":"earthion","core":"MegaDrive","inicio":1,"fim":2,
            "placar_final":1101200,"placar_maximo":1101200,"quadros":37940}"#;

        let partida: Partida = serde_json::from_str(antiga).unwrap();

        assert_eq!(partida.modo, "hi-score");
        assert_eq!(partida.jogador, None);
    }
}
