//! Títulos da live do jogador no YouTube.
//!
//! - Durante a live: só o modo competitivo (a tentativa ainda está em andamento).
//!   `Ketsui · HI-SCORE — ramonvalerio | ShmupX`
//! - No VOD, depois que o backend registra o resultado: o placar e, no Hi-Score zerado com 1
//!   crédito (a run entra também na tabela 1CC), o selo `[1CC]` logo depois do jogo.
//!   `Ketsui [1CC] · HI-SCORE · 245,890,120 — ramonvalerio | ShmupX`
//!   `Ketsui · CARAVAN · 48,910,000 — ramonvalerio | ShmupX`
//!
//! O YouTube aceita até 100 caracteres no título: se passar, quem encurta é o nome do jogo (o modo,
//! o placar e o jogador ficam inteiros).

use crate::services::placar::Tabela;

/// Limite do título de vídeo no YouTube.
const MAXIMO: usize = 100;

/// Modo como aparece no título: Caravan é Caravan; o resto (inclusive o `1cc` de um Lite antigo,
/// que é uma run de Hi-Score) é Hi-Score.
fn rotulo(modo: Option<&str>) -> &'static str {
    match Tabela::do_modo(modo) {
        Tabela::Caravan => "CARAVAN",
        Tabela::HiScore | Tabela::UmCredito => "HI-SCORE",
    }
}

/// Placar com vírgula nos milhares (`245,890,120`).
fn placar(n: i64) -> String {
    let digitos = n.unsigned_abs().to_string();
    let mut saida = String::with_capacity(digitos.len() + digitos.len() / 3);
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i) % 3 == 0 {
            saida.push(',');
        }
        saida.push(c);
    }
    if n < 0 { format!("-{saida}") } else { saida }
}

/// `{jogo}{selo} · {resto} — {jogador} | ShmupX`, com o jogo encurtado se o todo passar de 100.
fn montar(jogo: &str, selo: &str, resto: &str, jogador: &str) -> String {
    let jogo = jogo.trim();
    let jogador = jogador.trim();
    let depois = format!("{selo} · {resto} — {jogador} | ShmupX");
    let cabe = MAXIMO.saturating_sub(depois.chars().count());
    let jogo: String = if jogo.chars().count() > cabe {
        let mut curto: String = jogo.chars().take(cabe.saturating_sub(1)).collect();
        curto = curto.trim_end().to_string();
        curto.push('…');
        curto
    } else {
        jogo.to_string()
    };
    format!("{jogo}{depois}")
}

/// Título da live no ar: jogo, modo e jogador.
pub fn da_live(jogo: &str, modo: Option<&str>, jogador: &str) -> String {
    montar(jogo, "", rotulo(modo), jogador)
}

/// Título do VOD com o resultado registrado. `um_credito`: zerou sem continue; o selo `[1CC]` só
/// entra no Hi-Score (o Caravan não tem tabela 1CC).
pub fn do_vod(jogo: &str, modo: Option<&str>, jogador: &str, score: i64, um_credito: bool) -> String {
    let rotulo = rotulo(modo);
    let selo = if um_credito && rotulo == "HI-SCORE" { " [1CC]" } else { "" };
    montar(jogo, selo, &format!("{rotulo} · {}", placar(score)), jogador)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titulo_da_live_so_com_o_modo() {
        assert_eq!(da_live("Ketsui", Some("highscore"), "ramonvalerio"), "Ketsui · HI-SCORE — ramonvalerio | ShmupX");
        assert_eq!(da_live("Ketsui", None, "ramonvalerio"), "Ketsui · HI-SCORE — ramonvalerio | ShmupX");
        assert_eq!(da_live("Ketsui", Some("1cc"), "ramonvalerio"), "Ketsui · HI-SCORE — ramonvalerio | ShmupX");
        assert_eq!(da_live("Ketsui", Some("caravan-classic"), "ramonvalerio"), "Ketsui · CARAVAN — ramonvalerio | ShmupX");
    }

    #[test]
    fn titulo_do_vod_com_placar_e_selo_1cc() {
        assert_eq!(
            do_vod("Ketsui", Some("highscore"), "ramonvalerio", 245_890_120, true),
            "Ketsui [1CC] · HI-SCORE · 245,890,120 — ramonvalerio | ShmupX"
        );
        assert_eq!(
            do_vod("Ketsui", Some("highscore"), "ramonvalerio", 142_300_500, false),
            "Ketsui · HI-SCORE · 142,300,500 — ramonvalerio | ShmupX"
        );
        // Caravan não tem 1CC, mesmo que o jogo tenha sido zerado.
        assert_eq!(
            do_vod("Ketsui", Some("caravan-classic"), "ramonvalerio", 48_910_000, true),
            "Ketsui · CARAVAN · 48,910,000 — ramonvalerio | ShmupX"
        );
    }

    #[test]
    fn placar_com_virgulas() {
        assert_eq!(placar(0), "0");
        assert_eq!(placar(999), "999");
        assert_eq!(placar(1000), "1,000");
        assert_eq!(placar(1_234_567), "1,234,567");
    }

    #[test]
    fn titulo_longo_encurta_o_jogo() {
        let jogo = "A".repeat(120);
        let titulo = do_vod(&jogo, Some("highscore"), "ramonvalerio", 1_000_000, true);
        assert_eq!(titulo.chars().count(), MAXIMO);
        assert!(titulo.ends_with("… [1CC] · HI-SCORE · 1,000,000 — ramonvalerio | ShmupX"));
    }
}
