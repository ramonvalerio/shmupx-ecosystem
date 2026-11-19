//! Ranking local: as partidas gravadas neste MiSTer, para a janela do menu Scripts.
//!
//! A janela tem ~28 colunas e mostra só as primeiras ~14 linhas (não rola), então o texto
//! é montado para caber: título, filtro e top 10.

use crate::partida::Partida;

pub const TOP: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linha {
    pub posicao: usize,
    pub jogador: String,
    pub placar: u64,
}

/// Top `n` de um jogo e modo, maior placar primeiro; no empate, quem fez antes.
/// Partidas antigas, sem jogador, ficam com o nome configurado agora.
pub fn top(partidas: &[Partida], jogo: &str, modo: &str, jogador_atual: &str, n: usize) -> Vec<Linha> {
    let mut validas: Vec<&Partida> =
        partidas.iter().filter(|p| p.jogo == jogo && p.modo == modo && p.tem_placar()).collect();
    validas.sort_by(|a, b| b.placar_maximo.cmp(&a.placar_maximo).then(a.inicio.cmp(&b.inicio)));
    validas
        .into_iter()
        .take(n)
        .enumerate()
        .map(|(i, p)| Linha {
            posicao: i + 1,
            jogador: p.jogador.clone().unwrap_or_else(|| jogador_atual.to_owned()),
            placar: p.placar_maximo.unwrap_or(0),
        })
        .collect()
}

/// 1101200 -> "1.101.200"
pub fn milhar(valor: u64) -> String {
    let digitos = valor.to_string();
    let mut saida = String::new();
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            saida.push('.');
        }
        saida.push(c);
    }
    saida
}

/// Tela do ranking local, no máximo 28 colunas.
pub fn tela_local(titulo: &str, modo: &str, linhas: &[Linha]) -> String {
    let mut tela = cabecalho(titulo, modo, "[LOCAL]  online ");
    if linhas.is_empty() {
        tela.push_str(" nenhuma partida ainda\n");
    }
    for linha in linhas {
        tela.push_str(&format!("{:>2} {:<12} {:>11}\n", linha.posicao, linha.jogador, milhar(linha.placar)));
    }
    tela
}

/// Tela do ranking online. O MiSTer ainda não fala com o backend.
pub fn tela_online(titulo: &str, modo: &str) -> String {
    let mut tela = cabecalho(titulo, modo, " local  [ONLINE]");
    tela.push_str(" Ranking online ainda nao\n disponivel no MiSTer.\n\n Por enquanto as partidas\n ficam so no ranking local.\n");
    tela
}

fn cabecalho(titulo: &str, modo: &str, filtro: &str) -> String {
    let titulo = format!("{} - {}", titulo, nome_do_modo(modo));
    format!("{:^28}\n{:^28}\n", titulo.chars().take(28).collect::<String>(), filtro)
}

pub fn nome_do_modo(modo: &str) -> &str {
    match modo {
        "hi-score" => "Hi-Score",
        "1cc" => "1CC",
        "caravan-sprint" => "Sprint 2min",
        "caravan-classic" => "Classic 5min",
        outro => outro,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn partida(jogo: &str, modo: &str, jogador: Option<&str>, inicio: u64, placar: Option<u64>) -> Partida {
        Partida {
            jogo: jogo.into(),
            modo: modo.into(),
            jogador: jogador.map(Into::into),
            core: "MegaDrive".into(),
            inicio,
            fim: inicio,
            placar_final: placar,
            placar_maximo: placar,
            quadros: 1,
            run_id: None,
            envio: None,
        }
    }

    #[test]
    fn ordena_filtra_e_limita_o_top() {
        let mut partidas = vec![
            partida("earthion", "hi-score", Some("ANA"), 1, Some(500)),
            partida("earthion", "hi-score", None, 2, Some(900)),
            partida("earthion", "hi-score", Some("BIA"), 3, Some(500)),
            partida("earthion", "caravan", Some("ANA"), 4, Some(9_999)),
            partida("outro", "hi-score", Some("ANA"), 5, Some(9_999)),
            partida("earthion", "hi-score", Some("ANA"), 6, None),
            partida("earthion", "hi-score", Some("ANA"), 7, Some(0)),
        ];
        partidas.extend((0..20).map(|i| partida("earthion", "hi-score", Some("X"), 10 + i, Some(i))));

        let top = top(&partidas, "earthion", "hi-score", "RAMON", TOP);

        assert_eq!(top.len(), TOP);
        assert_eq!(top[0], Linha { posicao: 1, jogador: "RAMON".into(), placar: 900 });
        assert_eq!((top[1].jogador.as_str(), top[2].jogador.as_str()), ("ANA", "BIA")); // empate: quem fez antes
        assert_eq!(top[9].posicao, 10);
    }

    #[test]
    fn formata_milhar() {
        assert_eq!(milhar(0), "0");
        assert_eq!(milhar(999), "999");
        assert_eq!(milhar(1_000), "1.000");
        assert_eq!(milhar(1_101_200), "1.101.200");
        assert_eq!(milhar(99_999_999), "99.999.999");
    }

    #[test]
    fn telas_cabem_na_janela_do_menu_scripts() {
        let linhas: Vec<Linha> = (1..=10)
            .map(|i| Linha { posicao: i, jogador: "ABCDEFGHIJKL".into(), placar: 99_999_999 })
            .collect();

        for tela in [tela_local("Earthion", "hi-score", &linhas), tela_online("Earthion", "hi-score")] {
            assert!(tela.lines().all(|l| l.chars().count() <= 28), "{tela}");
            assert!(tela.lines().count() <= 13, "{tela}");
        }
    }
}
