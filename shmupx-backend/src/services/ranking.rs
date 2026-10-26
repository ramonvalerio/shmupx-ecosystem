//! Normalização de score (Spec 002, seção 3): converte o score bruto de um jogo em uma nota
//! 0..10 comparável entre jogos diferentes, usando a média/desvio padrão dos scores válidos
//! daquele jogo. Lógica pura — sem I/O, sem axum, sem sqlx — para ficar trivial de testar.

/// Nota devolvida quando não há dados suficientes para calcular um Z-score de verdade (jogo
/// novo sem histórico, ou só uma run válida até agora — desvio padrão indefinido). Nota neutra,
/// nem penaliza nem favorece quem joga um jogo ainda sem massa de comparação.
pub const NOTA_BASE_SEM_DADOS: f64 = 5.0;

/// Corte aplicado à nota quando a run usou pelo menos 1 continue: o ranking global de shmups
/// segue a cultura 1-Credit Clear (1CC), então uma run com continue ainda é registrada (para
/// histórico/estatística), mas praticamente não compete no ranking.
pub const FATOR_PENALIDADE_CONTINUE: f64 = 0.1;

const NOTA_MIN: f64 = 0.0;
const NOTA_MAX: f64 = 10.0;
/// Piso da nota *antes* da penalidade de continue: mesmo a run mais fraca já registrada (bem
/// abaixo da média) fica perto de 1.0, não de 0 — o 0 só aparece depois da penalidade de continue.
const NOTA_PISO_NATURAL: f64 = 1.0;

/// Converte um Z-score em nota 0..10. `z = 0` (score na média) -> nota 5.0; a partir daí, cada
/// desvio padrão vale ~1.67 pontos, então `z = +3` satura em 10.0 (performance excepcional) e
/// `z = -3` satura em 1.0 (run bem abaixo da média, mas ainda registrada).
fn z_score_para_nota(z: f64) -> f64 {
    let nota = 5.0 + z * (5.0 / 3.0);
    nota.clamp(NOTA_PISO_NATURAL, NOTA_MAX)
}

/// Calcula a nota normalizada de um `score_bruto` dado a média/desvio padrão dos scores válidos
/// anteriores daquele jogo. `mu`/`sigma` vêm de `AVG`/`STDDEV` em SQL, que devolvem `NULL`
/// (`None` aqui) quando não há linhas (jogo novo) — e `STDDEV` também devolve `NULL` com uma
/// única linha, já que desvio padrão de amostra única é indefinido. Em qualquer um desses casos,
/// ou se `sigma` vier `0.0` (todas as runs anteriores com o mesmo score, sem variância real),
/// a divisão por zero é evitada retornando a nota base em vez de propagar `NaN`/`inf`.
pub fn calcular_nota(score_bruto: i64, mu: Option<f64>, sigma: Option<f64>) -> f64 {
    match (mu, sigma) {
        (Some(mu), Some(sigma)) if sigma > 0.0 => {
            let z = (score_bruto as f64 - mu) / sigma;
            z_score_para_nota(z)
        }
        _ => NOTA_BASE_SEM_DADOS,
    }
}

/// Aplica a penalidade de continue sobre uma nota já calculada. Diferente do piso natural de
/// 1.0 em [`calcular_nota`], aqui a nota pode cair até 0 — é o ponto da penalidade: sinalizar
/// que a run praticamente não conta para o ranking, sem descartar o registro por completo.
pub fn aplicar_penalidade_continue(nota: f64, continues_usados: i32) -> f64 {
    if continues_usados > 0 {
        (nota * FATOR_PENALIDADE_CONTINUE).clamp(NOTA_MIN, NOTA_MAX)
    } else {
        nota
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_na_media_vira_nota_cinco() {
        let nota = calcular_nota(100_000, Some(100_000.0), Some(50_000.0));
        assert!((nota - 5.0).abs() < 1e-9);
    }

    #[test]
    fn score_tres_desvios_acima_satura_em_dez() {
        let nota = calcular_nota(250_000, Some(100_000.0), Some(50_000.0));
        assert!((nota - 10.0).abs() < 1e-9);
    }

    #[test]
    fn score_tres_desvios_abaixo_satura_no_piso_de_um() {
        let nota = calcular_nota(0, Some(150_000.0), Some(50_000.0));
        assert!((nota - 1.0).abs() < 1e-9);
    }

    #[test]
    fn score_entre_media_e_um_desvio_acima() {
        // z = 1.5 -> nota = 5 + 1.5 * (5/3) = 7.5 (mesmo valor validado manualmente em SQL).
        let nota = calcular_nota(175_000, Some(100_000.0), Some(50_000.0));
        assert!((nota - 7.5).abs() < 1e-9);
    }

    #[test]
    fn sem_media_cadastrada_usa_nota_base() {
        assert_eq!(calcular_nota(999_999, None, None), NOTA_BASE_SEM_DADOS);
    }

    #[test]
    fn desvio_padrao_indefinido_usa_nota_base() {
        // Uma única run anterior: STDDEV é NULL (não dá para calcular variância de amostra 1).
        assert_eq!(calcular_nota(50_000, Some(50_000.0), None), NOTA_BASE_SEM_DADOS);
    }

    #[test]
    fn desvio_padrao_zero_usa_nota_base_em_vez_de_dividir_por_zero() {
        // Todas as runs anteriores com o mesmo score: sigma = 0, não dá pra dividir por ele.
        assert_eq!(calcular_nota(100_000, Some(100_000.0), Some(0.0)), NOTA_BASE_SEM_DADOS);
    }

    #[test]
    fn continue_usado_corta_noventa_por_cento_da_nota() {
        let nota = aplicar_penalidade_continue(7.5, 1);
        assert!((nota - 0.75).abs() < 1e-9);
    }

    #[test]
    fn sem_continue_nota_fica_intacta() {
        assert_eq!(aplicar_penalidade_continue(7.5, 0), 7.5);
    }

    #[test]
    fn penalidade_nunca_resulta_em_nota_negativa() {
        let nota = aplicar_penalidade_continue(NOTA_PISO_NATURAL, 3);
        assert!(nota >= 0.0);
    }
}
