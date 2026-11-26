//! Ranking público montado a partir das gameplays no Firestore (Spec 002, ADR 005). Lógica pura:
//! recebe as partidas já lidas e devolve as classificações, sem I/O, para ficar simples de testar.
//!
//! - Por jogo (= sala, ADR 006): o maior placar de cada jogador, com a nota normalizada (Z-score
//!   sobre os placares do próprio jogo, `services::ranking`).
//! - Global: soma das notas máximas dos 5 jogos mais jogados de cada jogador (mesma regra da rota
//!   legada em PostgreSQL, `routes::rankings`).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::services::ranking::{aplicar_penalidade_continue, calcular_nota};

/// Jogos considerados por jogador no Ranking Global (Spec 002).
pub const MAX_JOGOS_GLOBAL: usize = 5;

/// Uma gameplay com resultado, já filtrada pelos estados que contam para o ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct Partida {
    /// Id da gameplay (`gameplays/{run_id}`): liga a partida ao destaque dela no canal oficial.
    pub run_id: String,
    pub jogador: String,
    pub jogo: String,
    pub score: i64,
    pub continues: i32,
    pub vidas_perdidas: Option<i32>,
    /// Rota percorrida (Darius Gaiden: `ABDHLQV`) e bombas usadas, quando o mapa de memória lê.
    pub rota: Option<String>,
    pub bombas_usadas: Option<i32>,
    /// Nave ou personagem escolhida (o rótulo do mapa de memória) e, no Caravan, o tempo vivido em ms.
    pub nave: Option<String>,
    pub sobrevivencia_ms: Option<i32>,
    /// O jogador zerou o jogo (`result.cleared`).
    pub zerou: bool,
    pub modo: Option<String>,
    pub versao: Option<String>,
    pub origem: Option<String>,
    /// `true` só para gameplays aprovadas; `needs_review` aparece como pendente de revisão.
    pub verificada: bool,
    pub enviada_em: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EntradaJogo {
    pub posicao: usize,
    pub jogador_id: String,
    pub nome: String,
    pub score: i64,
    pub nota: f64,
    pub continues: i32,
    pub vidas_perdidas: Option<i32>,
    pub rota: Option<String>,
    pub bombas_usadas: Option<i32>,
    pub nave: Option<String>,
    pub sobrevivencia_ms: Option<i32>,
    pub runs: usize,
    pub modo: Option<String>,
    pub versao: Option<String>,
    pub origem: Option<String>,
    pub verificada: bool,
    pub conquistado_em: Option<DateTime<Utc>>,
    /// Gameplay do registro (o melhor placar do jogador): não vai na resposta.
    #[serde(skip)]
    pub run_id: String,
    /// Vídeo da live da run no canal oficial da ShmupX (só quem chegou ao top 3 com live tem; o
    /// registro guarda o vídeo mesmo se cair de posição depois).
    pub youtube_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct JogoContado {
    pub jogo_id: String,
    pub titulo: String,
    pub nota: f64,
    pub runs: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EntradaGlobal {
    pub posicao: usize,
    pub jogador_id: String,
    pub nome: String,
    pub pontos: f64,
    pub jogos_contados: usize,
    pub runs: usize,
    pub jogos: Vec<JogoContado>,
}

/// Filtro de modo da tela: `highscore` inclui partidas sem modo (Clients antigos); `caravan`
/// junta `caravan-sprint` e `caravan-classic`; os demais comparam o valor exato.
pub fn modo_combina(filtro: Option<&str>, modo: Option<&str>) -> bool {
    match filtro {
        None => true,
        Some("highscore") => matches!(modo, None | Some("highscore")),
        Some("caravan") => modo.is_some_and(|m| m.starts_with("caravan")),
        Some(f) => modo == Some(f),
    }
}

/// A partida entra na tabela do filtro. 1CC não se escolhe ao jogar: a tabela 1CC tem as runs de
/// Hi-Score em que o jogador zerou com 1 crédito (0 continues), além das antigas marcadas `1cc`.
/// Elas continuam também na tabela Hi-Score.
pub fn entra_na_tabela(filtro: Option<&str>, p: &Partida) -> bool {
    match filtro {
        Some("1cc") => p.modo.as_deref() == Some("1cc") || (modo_combina(Some("highscore"), p.modo.as_deref()) && p.zerou && p.continues == 0),
        _ => modo_combina(filtro, p.modo.as_deref()),
    }
}

/// Média e desvio padrão amostral dos placares de cada jogo (como `AVG`/`STDDEV` do SQL).
fn estatisticas(partidas: &[Partida]) -> HashMap<&str, (Option<f64>, Option<f64>)> {
    let mut por_jogo: HashMap<&str, Vec<f64>> = HashMap::new();
    for p in partidas {
        por_jogo.entry(p.jogo.as_str()).or_default().push(p.score as f64);
    }
    por_jogo
        .into_iter()
        .map(|(jogo, scores)| {
            let n = scores.len() as f64;
            let media = scores.iter().sum::<f64>() / n;
            let desvio = (scores.len() > 1)
                .then(|| (scores.iter().map(|s| (s - media).powi(2)).sum::<f64>() / (n - 1.0)).sqrt());
            (jogo, (Some(media), desvio))
        })
        .collect()
}

fn nota_da_partida(p: &Partida, stats: &HashMap<&str, (Option<f64>, Option<f64>)>) -> f64 {
    let (media, desvio) = stats.get(p.jogo.as_str()).copied().unwrap_or((None, None));
    aplicar_penalidade_continue(calcular_nota(p.score, media, desvio), p.continues)
}

fn arredondar(valor: f64) -> f64 {
    (valor * 100.0).round() / 100.0
}

/// O que decide a posição no Caravan: placar, tempo sobrevivido (ms), mortes (vidas perdidas) e bombas.
/// Campo não lido (run antiga, jogo sem o dado) fica depois de quem tem o valor.
pub type MarcaCaravan = (i64, Option<i64>, Option<i64>, Option<i64>);

/// Regra padrão do Caravan Mode: 1º maior placar (mesmo que tenha morrido antes), 2º mais tempo
/// sobrevivido dentro do limite, 3º menos mortes, 4º menos bombas. O 5º critério é quem registrou a
/// marca primeiro (`enviada_em`), aplicado por quem chama. `Ordering::Less` = `a` na frente de `b`.
pub fn ordem_caravan(a: &MarcaCaravan, b: &MarcaCaravan) -> std::cmp::Ordering {
    let menos = |v: Option<i64>| v.unwrap_or(i64::MAX);
    b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(menos(a.2).cmp(&menos(b.2))).then(menos(a.3).cmp(&menos(b.3)))
}

fn marca(p: &Partida) -> MarcaCaravan {
    (p.score, p.sobrevivencia_ms.map(i64::from), p.vidas_perdidas.map(i64::from), p.bombas_usadas.map(i64::from))
}

/// Qual partida fica na frente: no Caravan, a regra de `ordem_caravan`; nos outros modos, o maior
/// placar. `Ordering::Less` = `a` na frente de `b`.
pub fn ordem_no_ranking(a: &Partida, b: &Partida, caravana: bool) -> std::cmp::Ordering {
    if caravana { ordem_caravan(&marca(a), &marca(b)) } else { b.score.cmp(&a.score) }
}

/// Classificação de um jogo: o melhor de cada jogador (`ordem_no_ranking`: no Caravan, menos vidas
/// perdidas e depois o placar). Empate: quem chegou primeiro fica na frente, mas a posição é a mesma
/// (1, 2, 2, 4).
pub fn ranking_do_jogo(partidas: &[Partida], jogo: &str, modo: Option<&str>, versao: Option<&str>) -> Vec<EntradaJogo> {
    let caravana = modo == Some("caravan");
    let stats = estatisticas(partidas);
    let mut melhores: HashMap<&str, (&Partida, usize)> = HashMap::new();
    for p in partidas.iter().filter(|p| {
        p.jogo == jogo && entra_na_tabela(modo, p) && versao.is_none_or(|v| p.versao.as_deref() == Some(v))
    }) {
        let atual = melhores.entry(p.jogador.as_str()).or_insert((p, 0));
        atual.1 += 1;
        let melhor = atual.0;
        if ordem_no_ranking(p, melhor, caravana).then(p.enviada_em.cmp(&melhor.enviada_em)).is_lt() {
            atual.0 = p;
        }
    }
    let mut lista: Vec<(&Partida, usize)> = melhores.into_values().collect();
    lista.sort_by(|(a, _), (b, _)| {
        ordem_no_ranking(a, b, caravana).then(a.enviada_em.cmp(&b.enviada_em)).then(a.jogador.cmp(&b.jogador))
    });

    let mut entradas: Vec<EntradaJogo> = Vec::with_capacity(lista.len());
    for (indice, (p, runs)) in lista.into_iter().enumerate() {
        let posicao = match entradas.last() {
            Some(anterior) if anterior.score == p.score && (!caravana || (anterior.sobrevivencia_ms, anterior.vidas_perdidas, anterior.bombas_usadas) == (p.sobrevivencia_ms, p.vidas_perdidas, p.bombas_usadas)) => anterior.posicao,
            _ => indice + 1,
        };
        entradas.push(EntradaJogo {
            posicao,
            jogador_id: p.jogador.clone(),
            nome: String::new(),
            score: p.score,
            nota: arredondar(nota_da_partida(p, &stats)),
            continues: p.continues,
            vidas_perdidas: p.vidas_perdidas,
            rota: p.rota.clone(),
            bombas_usadas: p.bombas_usadas,
            nave: p.nave.clone(),
            sobrevivencia_ms: p.sobrevivencia_ms,
            runs,
            modo: p.modo.clone(),
            versao: p.versao.clone(),
            origem: p.origem.clone(),
            verificada: p.verificada,
            conquistado_em: p.enviada_em,
            run_id: p.run_id.clone(),
            youtube_url: None,
        });
    }
    entradas
}

/// Ranking Global (Spec 002): por jogador, a nota máxima de cada jogo; entram só os 5 jogos com
/// mais partidas dele (desempate: nota, depois id do jogo), somados.
pub fn ranking_global(partidas: &[Partida]) -> Vec<EntradaGlobal> {
    let stats = estatisticas(partidas);
    // jogador -> jogo -> (nota máxima, partidas)
    let mut por_jogador: HashMap<&str, HashMap<&str, (f64, usize)>> = HashMap::new();
    for p in partidas {
        let nota = nota_da_partida(p, &stats);
        let jogo = por_jogador.entry(p.jogador.as_str()).or_default().entry(p.jogo.as_str()).or_insert((nota, 0));
        jogo.0 = jogo.0.max(nota);
        jogo.1 += 1;
    }

    let mut entradas: Vec<EntradaGlobal> = por_jogador
        .into_iter()
        .map(|(jogador, jogos)| {
            let runs = jogos.values().map(|(_, n)| n).sum();
            let mut jogos: Vec<(&str, f64, usize)> = jogos.into_iter().map(|(j, (nota, n))| (j, nota, n)).collect();
            jogos.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.total_cmp(&a.1)).then(a.0.cmp(b.0)));
            jogos.truncate(MAX_JOGOS_GLOBAL);
            let pontos = arredondar(jogos.iter().map(|(_, nota, _)| nota).sum());
            EntradaGlobal {
                posicao: 0,
                jogador_id: jogador.to_string(),
                nome: String::new(),
                pontos,
                jogos_contados: jogos.len(),
                runs,
                jogos: jogos
                    .into_iter()
                    .map(|(j, nota, n)| JogoContado { jogo_id: j.to_string(), titulo: String::new(), nota: arredondar(nota), runs: n })
                    .collect(),
            }
        })
        .collect();
    entradas.sort_by(|a, b| b.pontos.total_cmp(&a.pontos).then(a.jogador_id.cmp(&b.jogador_id)));
    for indice in 0..entradas.len() {
        entradas[indice].posicao = match indice.checked_sub(1).map(|i| &entradas[i]) {
            Some(anterior) if anterior.pontos == entradas[indice].pontos => anterior.posicao,
            _ => indice + 1,
        };
    }
    entradas
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partida(jogador: &str, jogo: &str, score: i64) -> Partida {
        Partida {
            run_id: format!("{jogador}-{score}"),
            jogador: jogador.into(),
            jogo: jogo.into(),
            score,
            continues: 0,
            vidas_perdidas: None,
            rota: None,
            bombas_usadas: None,
            nave: None,
            sobrevivencia_ms: None,
            zerou: false,
            modo: None,
            versao: None,
            origem: Some("pc".into()),
            verificada: false,
            enviada_em: None,
        }
    }

    #[test]
    fn sem_partidas_os_rankings_ficam_vazios() {
        assert!(ranking_do_jogo(&[], "earthion", None, None).is_empty());
        assert!(ranking_global(&[]).is_empty());
    }

    #[test]
    fn caravan_ordena_por_placar_sobrevivencia_mortes_e_bombas() {
        let caravan = |jogador: &str, score: i64, tempo: Option<i32>, vidas: Option<i32>, bombas: Option<i32>| Partida {
            modo: Some("caravan-classic".into()),
            vidas_perdidas: vidas,
            sobrevivencia_ms: tempo,
            bombas_usadas: bombas,
            ..partida(jogador, "j", score)
        };
        let partidas = [
            caravan("ana", 900, Some(300_000), Some(3), Some(0)),
            caravan("bia", 500, Some(300_000), Some(0), Some(0)),
            // Mesmo placar da ana: mais tempo vivido vence, mesmo com mais mortes.
            caravan("caio", 900, Some(222_000), Some(0), Some(0)),
            caravan("dani", 900, Some(300_000), Some(1), Some(2)),
            caravan("edu", 900, Some(300_000), Some(1), Some(0)),
            // Sem tempo lido (run antiga): depois de quem tem o tempo, no mesmo placar.
            caravan("fabi", 900, None, Some(0), Some(0)),
        ];
        let ordem: Vec<String> = ranking_do_jogo(&partidas, "j", Some("caravan"), None).into_iter().map(|e| e.jogador_id).collect();
        // Placar 900: tempo 300s (edu 1 morte/0 bombas, dani 1/2, ana 3 mortes), depois caio (222s), fabi
        // (sem tempo); por fim a bia, de placar menor.
        assert_eq!(ordem, vec!["edu", "dani", "ana", "caio", "fabi", "bia"]);
        // No Hi-Score continua só o placar.
        let hs: Vec<String> = ranking_do_jogo(&partidas.clone().map(|p| Partida { modo: None, ..p }), "j", Some("highscore"), None).into_iter().map(|e| e.jogador_id).collect();
        assert_eq!(hs.last().map(String::as_str), Some("bia"));
    }

    #[test]
    fn caravan_empatado_em_tudo_divide_a_posicao_e_o_primeiro_a_registrar_fica_na_frente() {
        use chrono::TimeZone;
        let caravan = |jogador: &str, minuto: u32| Partida {
            modo: Some("caravan-classic".into()),
            vidas_perdidas: Some(0),
            sobrevivencia_ms: Some(300_000),
            bombas_usadas: Some(0),
            enviada_em: Some(Utc.with_ymd_and_hms(2026, 11, 26, 10, minuto, 0).unwrap()),
            ..partida(jogador, "j", 500)
        };
        let r = ranking_do_jogo(&[caravan("depois", 9), caravan("antes", 1)], "j", Some("caravan"), None);
        assert_eq!(r.iter().map(|e| (e.jogador_id.as_str(), e.posicao)).collect::<Vec<_>>(), vec![("antes", 1), ("depois", 1)]);
    }

    #[test]
    fn ranking_do_jogo_usa_o_melhor_placar_de_cada_jogador() {
        let partidas = [
            partida("ana", "earthion", 100),
            partida("ana", "earthion", 300),
            partida("bia", "earthion", 200),
            partida("caio", "outro", 999),
        ];
        let r = ranking_do_jogo(&partidas, "earthion", None, None);
        assert_eq!(r.iter().map(|e| (e.jogador_id.as_str(), e.score, e.posicao, e.runs)).collect::<Vec<_>>(),
            vec![("ana", 300, 1, 2), ("bia", 200, 2, 1)]);
    }

    #[test]
    fn empate_no_placar_divide_a_posicao() {
        let partidas = [partida("ana", "j", 500), partida("bia", "j", 500), partida("caio", "j", 100)];
        let posicoes: Vec<usize> = ranking_do_jogo(&partidas, "j", None, None).iter().map(|e| e.posicao).collect();
        assert_eq!(posicoes, vec![1, 1, 3]);
    }

    #[test]
    fn hi_score_zerado_com_1_credito_entra_tambem_no_1cc() {
        let mut zerou = partida("ana", "j", 900);
        zerou.zerou = true;
        let mut zerou_com_continue = partida("bia", "j", 800);
        zerou_com_continue.zerou = true;
        zerou_com_continue.continues = 1;
        let sem_zerar = partida("caio", "j", 950);
        let partidas = [zerou, zerou_com_continue, sem_zerar];
        let ids = |modo| ranking_do_jogo(&partidas, "j", modo, None).into_iter().map(|e| e.jogador_id).collect::<Vec<_>>();
        assert_eq!(ids(Some("1cc")), vec!["ana"]);
        assert_eq!(ids(Some("highscore")), vec!["caio", "ana", "bia"]);
    }

    #[test]
    fn filtro_de_modo_e_versao() {
        let mut um_cc = partida("ana", "j", 50);
        um_cc.modo = Some("1cc".into());
        let mut caravana = partida("bia", "j", 70);
        caravana.modo = Some("caravan-sprint".into());
        let mut versao = partida("caio", "j", 90);
        versao.versao = Some("arcade".into());
        let partidas = [um_cc, caravana, versao, partida("dani", "j", 10)];
        let ids = |modo, v| ranking_do_jogo(&partidas, "j", modo, v).into_iter().map(|e| e.jogador_id).collect::<Vec<_>>();
        assert_eq!(ids(Some("1cc"), None), vec!["ana"]);
        assert_eq!(ids(Some("caravan"), None), vec!["bia"]);
        assert_eq!(ids(Some("highscore"), None), vec!["caio", "dani"]);
        assert_eq!(ids(None, Some("arcade")), vec!["caio"]);
    }

    #[test]
    fn global_soma_so_os_cinco_jogos_mais_jogados() {
        let mut partidas = Vec::new();
        for jogo in ["a", "b", "c", "d", "e", "f"] {
            partidas.push(partida("ana", jogo, 100));
        }
        // "f" é o mais jogado; "a" tem só uma partida e empata em volume com b..e.
        partidas.push(partida("ana", "f", 100));
        let r = ranking_global(&partidas);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].jogos_contados, MAX_JOGOS_GLOBAL);
        assert_eq!(r[0].jogos[0].jogo_id, "f");
        assert_eq!(r[0].runs, 7);
        // Sem variação de placar, cada jogo vale a nota base (5.0).
        assert!((r[0].pontos - 25.0).abs() < 1e-9);
    }

    #[test]
    fn continue_derruba_a_nota_no_global() {
        let mut com_continue = partida("bia", "j", 100);
        com_continue.continues = 1;
        let r = ranking_global(&[partida("ana", "j", 100), com_continue]);
        assert_eq!(r[0].jogador_id, "ana");
        assert!(r[0].pontos > r[1].pontos);
        assert_eq!((r[0].posicao, r[1].posicao), (1, 2));
    }
}
