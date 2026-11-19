//! Onde está o placar de cada jogo e como decodificá-lo.

use crate::genesis::{TAMANHO_WRAM, deslocamento_wram};
use serde::Deserialize;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codificacao {
    /// Um dígito decimal por nibble (comum em jogos de 16 bits).
    Bcd,
    /// Inteiro sem sinal, big-endian.
    Binario,
    /// Um dígito decimal (0–9) por palavra de 16 bits, do mais significativo ao menos
    /// (Earthion: o placar fica pronto para desenhar na tela).
    #[serde(rename = "digito_por_palavra")]
    DigitoPorPalavra,
}

impl Codificacao {
    /// Tamanhos (em bytes) que o `procurar` testa.
    fn tamanhos(self) -> &'static [usize] {
        match self {
            Codificacao::Bcd | Codificacao::Binario => &[2, 3, 4],
            Codificacao::DigitoPorPalavra => &[16],
        }
    }

    fn nome(self) -> &'static str {
        match self {
            Codificacao::Bcd => "bcd",
            Codificacao::Binario => "binario",
            Codificacao::DigitoPorPalavra => "digito_por_palavra",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampoPlacar {
    /// Deslocamento na WRAM, na ordem do 68000.
    pub deslocamento: u32,
    pub bytes: usize,
    pub codificacao: Codificacao,
    /// Muitos jogos guardam o placar sem os zeros fixos do fim (mostra 12300, guarda 1230).
    pub multiplicador: u64,
}

impl CampoPlacar {
    pub fn decodificar(&self, bytes: &[u8]) -> Option<u64> {
        decodificar(bytes, self.codificacao).and_then(|v| v.checked_mul(self.multiplicador))
    }
}

impl fmt::Display for CampoPlacar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "0xFF{:04X}  {} bytes  {}  x{}",
            self.deslocamento,
            self.bytes,
            self.codificacao.nome(),
            self.multiplicador
        )
    }
}

pub fn decodificar(bytes: &[u8], codificacao: Codificacao) -> Option<u64> {
    match codificacao {
        Codificacao::Binario => Some(bytes.iter().fold(0, |acc, b| (acc << 8) | u64::from(*b))),
        Codificacao::Bcd => bytes.iter().try_fold(0u64, |acc, b| {
            let (alto, baixo) = (b >> 4, b & 0x0F);
            (alto <= 9 && baixo <= 9).then(|| acc * 100 + u64::from(alto) * 10 + u64::from(baixo))
        }),
        Codificacao::DigitoPorPalavra => {
            let (palavras, sobra) = bytes.as_chunks::<2>();
            if !sobra.is_empty() {
                return None;
            }
            palavras.iter().try_fold(0u64, |acc, palavra| {
                let digito = u16::from_be_bytes(*palavra);
                (digito <= 9).then(|| acc * 10 + u64::from(digito))
            })
        }
    }
}

/// Perfil de um jogo (`perfis/<jogo>.json`).
#[derive(Debug, Deserialize)]
pub struct Perfil {
    pub jogo: String,
    /// Nome para mostrar na tela (`Earthion`); sem ele, usa o id.
    #[serde(default)]
    pub nome: Option<String>,
    pub sistema: String,
    /// Modos do ShmupX em que o jogo aparece. O primeiro vale para as partidas registradas.
    #[serde(default = "modos_padrao")]
    pub modos: Vec<String>,
    /// Id do jogo no catálogo do backend (`games/{id}`). Sem ele, a partida fica só local.
    #[serde(default)]
    pub jogo_id: Option<String>,
    /// Versão do jogo; o ranking junta partidas do mesmo jogo e versão, de qualquer plataforma.
    #[serde(default)]
    pub versao: Option<String>,
    pub placar: PlacarJson,
}

fn modos_padrao() -> Vec<String> {
    vec![crate::partida::MODO_PADRAO.to_owned()]
}

#[derive(Debug, Deserialize)]
pub struct PlacarJson {
    /// `null` enquanto o endereço não foi descoberto com `dump` + `procurar`.
    pub endereco: Option<String>,
    pub bytes: usize,
    pub codificacao: Codificacao,
    #[serde(default = "um")]
    pub multiplicador: u64,
}

fn um() -> u64 {
    1
}

impl Perfil {
    pub fn de_json(texto: &str) -> Result<Self, String> {
        serde_json::from_str(texto).map_err(|e| format!("perfil inválido: {e}"))
    }

    pub fn titulo(&self) -> &str {
        self.nome.as_deref().unwrap_or(&self.jogo)
    }

    pub fn modo_principal(&self) -> &str {
        self.modos.first().map_or(crate::partida::MODO_PADRAO, String::as_str)
    }

    pub fn campo(&self) -> Result<CampoPlacar, String> {
        if self.sistema != "megadrive" {
            return Err(format!("sistema '{}' ainda não suportado (só megadrive)", self.sistema));
        }
        let texto = self.placar.endereco.as_deref().ok_or_else(|| {
            format!(
                "o endereço do placar de '{}' ainda não foi descoberto; use `dump` e `procurar`",
                self.jogo
            )
        })?;
        let endereco = u32::from_str_radix(texto.trim_start_matches("0x").trim_start_matches("0X"), 16)
            .map_err(|_| format!("endereço inválido: {texto}"))?;
        let deslocamento = deslocamento_wram(endereco).ok_or_else(|| format!("{texto} está fora da WRAM"))?;
        let bytes = self.placar.bytes;
        let par_se_preciso = self.placar.codificacao != Codificacao::DigitoPorPalavra || bytes.is_multiple_of(2);
        if !(1..=32).contains(&bytes) || !par_se_preciso || deslocamento as usize + bytes > TAMANHO_WRAM {
            return Err(format!("tamanho do placar inválido: {bytes} bytes"));
        }
        Ok(CampoPlacar {
            deslocamento,
            bytes,
            codificacao: self.placar.codificacao,
            multiplicador: self.placar.multiplicador,
        })
    }
}

/// Só aceita um placar que se repete em dois quadros seguidos: evita ler no meio de uma
/// atualização do jogo. Devolve o valor quando ele muda.
#[derive(Debug, Default)]
pub struct Estabilizador {
    anterior: Option<u64>,
    aceito: Option<u64>,
}

impl Estabilizador {
    pub fn observar(&mut self, valor: Option<u64>) -> Option<u64> {
        let novo = match valor {
            Some(v) if valor == self.anterior && valor != self.aceito => {
                self.aceito = valor;
                Some(v)
            }
            _ => None,
        };
        self.anterior = valor;
        novo
    }
}

/// Perfis prontos (com endereço) de um sistema, e os jogos que ainda esperam a descoberta.
pub fn perfis_do_sistema(pasta: &Path, sistema: &str) -> (Vec<(Perfil, CampoPlacar)>, Vec<String>) {
    let mut prontos = Vec::new();
    let mut pendentes = Vec::new();
    let Ok(entradas) = std::fs::read_dir(pasta) else {
        return (prontos, pendentes);
    };
    let mut caminhos: Vec<_> = entradas
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    caminhos.sort();
    for caminho in caminhos {
        let lido = std::fs::read_to_string(&caminho).map_err(|e| e.to_string());
        let Ok(perfil) = lido.and_then(|texto| Perfil::de_json(&texto)) else {
            continue;
        };
        if perfil.sistema != sistema {
            continue;
        }
        match perfil.campo() {
            Ok(campo) => prontos.push((perfil, campo)),
            Err(_) => pendentes.push(perfil.jogo),
        }
    }
    (prontos, pendentes)
}

/// Uma cópia da WRAM tirada com o placar conhecido na tela.
pub struct Amostra {
    pub wram: Vec<u8>,
    pub placar: u64,
}

/// Todos os campos que decodificam exatamente o placar de cada amostra.
///
/// Com placares diferentes e não nulos, 3 ou 4 amostras costumam deixar um candidato só.
pub fn procurar(amostras: &[Amostra]) -> Vec<CampoPlacar> {
    let Some(menor) = amostras.iter().map(|a| a.wram.len()).min() else {
        return Vec::new();
    };
    let mut candidatos = Vec::new();
    for codificacao in [Codificacao::Bcd, Codificacao::Binario, Codificacao::DigitoPorPalavra] {
        for &bytes in codificacao.tamanhos() {
            for multiplicador in [1, 10, 100] {
                for deslocamento in 0..=menor.saturating_sub(bytes) {
                    let campo = CampoPlacar { deslocamento: deslocamento as u32, bytes, codificacao, multiplicador };
                    let confere = amostras.iter().all(|a| {
                        campo.decodificar(&a.wram[deslocamento..deslocamento + bytes]) == Some(a.placar)
                    });
                    if confere {
                        candidatos.push(campo);
                    }
                }
            }
        }
    }
    candidatos
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn decodifica_bcd_e_binario() {
        assert_eq!(decodificar(&[0x01, 0x23, 0x45], Codificacao::Bcd), Some(12345));
        assert_eq!(decodificar(&[0x1A], Codificacao::Bcd), None);
        assert_eq!(decodificar(&[0x01, 0x00], Codificacao::Binario), Some(256));
    }

    #[test]
    fn decodifica_um_digito_por_palavra_como_no_earthion() {
        // Trecho real da RAM do Earthion (0xFF1C48) com 230100 na tela.
        let ram = [0, 0, 0, 0, 0, 2, 0, 3, 0, 0, 0, 1, 0, 0, 0, 0];

        assert_eq!(decodificar(&ram, Codificacao::DigitoPorPalavra), Some(230_100));
        assert_eq!(decodificar(&[0, 10], Codificacao::DigitoPorPalavra), None);
        assert_eq!(decodificar(&[1, 2], Codificacao::DigitoPorPalavra), None);
        assert_eq!(decodificar(&[0, 1, 0], Codificacao::DigitoPorPalavra), None);
    }

    fn amostra(placar_bcd: [u8; 3], placar: u64, ruido: u8) -> Amostra {
        let mut wram: Vec<u8> = (0..TAMANHO_WRAM).map(|i| (i as u8).wrapping_mul(31).wrapping_add(ruido)).collect();
        wram[0x2410..0x2413].copy_from_slice(&placar_bcd);
        Amostra { wram, placar }
    }

    #[test]
    fn procurar_encontra_o_placar_plantado() {
        // O jogo mostra 1234500 e guarda 123450 em BCD (o último zero é fixo).
        let amostras = [
            amostra([0x12, 0x34, 0x50], 1_234_500, 1),
            amostra([0x00, 0x98, 0x70], 98_700, 2),
            amostra([0x05, 0x55, 0x10], 555_100, 3),
        ];

        let candidatos = procurar(&amostras);

        let esperado = CampoPlacar { deslocamento: 0x2410, bytes: 3, codificacao: Codificacao::Bcd, multiplicador: 10 };
        assert!(candidatos.contains(&esperado), "{candidatos:?}");
        assert!(candidatos.len() <= 3, "{candidatos:?}");
    }

    #[test]
    fn estabilizador_exige_dois_quadros_iguais() {
        let mut e = Estabilizador::default();

        assert_eq!(e.observar(Some(10)), None);
        assert_eq!(e.observar(Some(10)), Some(10));
        assert_eq!(e.observar(Some(10)), None); // sem mudança
        assert_eq!(e.observar(Some(19)), None); // lido no meio da atualização
        assert_eq!(e.observar(Some(20)), None);
        assert_eq!(e.observar(Some(20)), Some(20));
        assert_eq!(e.observar(None), None);
    }

    #[test]
    fn separa_perfis_prontos_e_pendentes() {
        let pasta = std::env::temp_dir().join(format!("shmupx-perfis-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let perfil = |jogo: &str, endereco: &str| {
            format!(
                r#"{{"jogo":"{jogo}","sistema":"megadrive","placar":{{"endereco":{endereco},"bytes":3,"codificacao":"bcd"}}}}"#
            )
        };
        std::fs::write(pasta.join("a.json"), perfil("pronto", r#""0xFF0100""#)).unwrap();
        std::fs::write(pasta.join("b.json"), perfil("earthion", "null")).unwrap();
        std::fs::write(pasta.join("c.txt"), "ignorado").unwrap();

        let (prontos, pendentes) = perfis_do_sistema(&pasta, "megadrive");

        assert_eq!(prontos.len(), 1);
        assert_eq!(prontos[0].0.jogo, "pronto");
        assert_eq!(pendentes, vec!["earthion".to_owned()]);
        std::fs::remove_dir_all(pasta).unwrap();
    }

    #[test]
    fn perfil_sem_endereco_explica_o_que_fazer() {
        let perfil = Perfil::de_json(
            r#"{"jogo":"earthion","sistema":"megadrive","placar":{"endereco":null,"bytes":4,"codificacao":"bcd"}}"#,
        )
        .unwrap();

        assert!(perfil.campo().unwrap_err().contains("procurar"));
    }

    #[test]
    fn perfil_com_endereco_do_68000() {
        let perfil = Perfil::de_json(
            r#"{"jogo":"x","sistema":"megadrive","placar":{"endereco":"0xFF2410","bytes":3,"codificacao":"bcd","multiplicador":10}}"#,
        )
        .unwrap();

        let campo = perfil.campo().unwrap();

        assert_eq!(campo.deslocamento, 0x2410);
        assert_eq!(campo.decodificar(&[0x12, 0x34, 0x50]), Some(1_234_500));
    }
}
