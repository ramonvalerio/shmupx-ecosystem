//! Ranking online só com o melhor de cada jogador. Toda run válida fica no ranking local, no Client
//! do jogador; o banco guarda só o recorde. Assim o banco não acumula as runs abaixo do recorde de
//! ninguém, e as leituras do ranking não crescem com o número de partidas jogadas.
//!
//! Depois do envio de uma run, as gameplays com resultado do jogador naquele jogo são comparadas:
//! fica só o recorde de cada tabela (maior placar; no Caravan, o maior placar e depois o tempo
//! vivido, as mortes e as bombas), e as outras saem do banco (o lançamento da ficha na
//! carteira continua). A comparação é sempre da run atual com o que está no banco: mexer no
//! ranking local não muda nada aqui.
//!
//! Tabela = modo (Hi-Score, 1CC, cada Caravan) × versão do jogo × evento. Uma run de Hi-Score
//! zerada com 1 crédito vale também na tabela 1CC; ela fica enquanto for o recorde de qualquer
//! uma das tabelas em que entra. Empate: fica a mais antiga (quem chegou primeiro).

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::error::AppError;
use crate::firestore::valor::{ler_booleano, ler_data, ler_inteiro, ler_mapa, ler_texto};
use crate::firestore::{Campos, Firestore};
use crate::services::placar;

/// Estados de uma gameplay com resultado (os do ranking).
const COM_RESULTADO: [&str; 2] = ["needs_review", "approved"];
/// Teto de gameplays lidas de um jogador num jogo (com a poda, ficam poucas).
const MAX_LIDAS: usize = 2000;

/// Uma gameplay com resultado, do jeito que a poda precisa.
#[derive(Debug, Clone, PartialEq)]
struct Registro {
    id: String,
    score: i64,
    modo: Option<String>,
    versao: Option<String>,
    evento: Option<String>,
    zerou: bool,
    continues: i64,
    /// Vidas perdidas (`result.livesLost`): o 3º critério do Caravan.
    vidas_perdidas: Option<i64>,
    /// Caravan: tempo vivido (ms) e bombas usadas, os critérios depois do placar e das mortes.
    sobrevivencia_ms: Option<i64>,
    bombas_usadas: Option<i64>,
    enviada_em: Option<DateTime<Utc>>,
}

/// As tabelas em que a run entra.
fn tabelas(r: &Registro) -> Vec<String> {
    let modo = match r.modo.as_deref() {
        None | Some("highscore") => "highscore",
        Some(m) => m,
    };
    let resto = format!("{}|{}", r.versao.as_deref().unwrap_or(""), r.evento.as_deref().unwrap_or(""));
    let mut lista = vec![format!("{modo}|{resto}")];
    if modo == "highscore" && r.zerou && r.continues == 0 {
        lista.push(format!("1cc|{resto}"));
    }
    lista
}

/// A run `a` é melhor que `b` na tabela: no Caravan, a regra de `placar::ordem_caravan` (placar, tempo
/// vivido, mortes, bombas); nos outros modos, o maior placar. Empate: a mais antiga.
fn melhor_que(a: &Registro, b: &Registro, tabela: &str) -> bool {
    let principal = if tabela.starts_with("caravan") {
        let marca = |r: &Registro| (r.score, r.sobrevivencia_ms, r.vidas_perdidas, r.bombas_usadas);
        placar::ordem_caravan(&marca(a), &marca(b))
    } else {
        b.score.cmp(&a.score)
    };
    principal.then((a.enviada_em, &a.id).cmp(&(b.enviada_em, &b.id))).is_lt()
}

/// Ids que ficam: o recorde de cada tabela.
fn recordes(registros: &[Registro]) -> HashSet<String> {
    let mut melhor: HashMap<String, &Registro> = HashMap::new();
    for r in registros {
        for tabela in tabelas(r) {
            let atual = melhor.entry(tabela.clone()).or_insert(r);
            if melhor_que(r, atual, &tabela) {
                *atual = r;
            }
        }
    }
    melhor.into_values().map(|r| r.id.clone()).collect()
}

fn registro(id: String, campos: &Campos) -> Option<Registro> {
    if !COM_RESULTADO.contains(&ler_texto(campos, "status")?.as_str()) {
        return None;
    }
    let resultado = ler_mapa(campos, "result")?;
    Some(Registro {
        id,
        score: ler_inteiro(&resultado, "score")?,
        modo: ler_texto(campos, "mode"),
        versao: ler_texto(campos, "gameVersion"),
        evento: ler_texto(campos, "eventId"),
        zerou: ler_booleano(&resultado, "cleared") == Some(true),
        continues: ler_inteiro(&resultado, "continuesUsed").unwrap_or(0),
        vidas_perdidas: ler_inteiro(&resultado, "livesLost"),
        sobrevivencia_ms: ler_inteiro(&resultado, "survivalMs"),
        bombas_usadas: ler_inteiro(&resultado, "bombsUsed"),
        enviada_em: ler_data(&resultado, "submittedAt"),
    })
}

/// Deixa no banco só os recordes do jogador `uid` no jogo `jogo` e diz se a run `run_id` ficou
/// (é recorde de alguma tabela). Uma falha ao apagar só vai para o log: o envio já valeu.
pub async fn manter_so_os_recordes(fs: &Firestore, uid: &str, jogo: &str, run_id: &str) -> Result<bool, AppError> {
    let igual = |campo: &str, valor: &str| json!({ "fieldFilter": { "field": { "fieldPath": campo }, "op": "EQUAL", "value": { "stringValue": valor } } });
    let campos_lidos: Vec<Value> = ["status", "mode", "gameVersion", "eventId", "result"].iter().map(|c| json!({ "fieldPath": c })).collect();
    let consulta = json!({
        "from": [{ "collectionId": "gameplays" }],
        "where": { "compositeFilter": { "op": "AND", "filters": [igual("ownerId", uid), igual("gameId", jogo)] } },
        "select": { "fields": campos_lidos },
        "limit": MAX_LIDAS,
    });
    let registros: Vec<Registro> = fs.consultar(consulta).await?.into_iter().filter_map(|(id, campos)| registro(id, &campos)).collect();
    let ficam = recordes(&registros);
    for r in registros.iter().filter(|r| !ficam.contains(&r.id)) {
        if let Err(e) = fs.apagar(&format!("gameplays/{}", r.id)).await {
            eprintln!("recordes: não apagou a gameplay {} ({uid}, {jogo}): {e:?}", r.id);
        }
    }
    Ok(ficam.contains(run_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn r(id: &str, score: i64, modo: Option<&str>, zerou: bool, minuto: u32) -> Registro {
        Registro {
            id: id.into(),
            score,
            modo: modo.map(str::to_string),
            versao: None,
            evento: None,
            zerou,
            continues: 0,
            vidas_perdidas: None,
            sobrevivencia_ms: None,
            bombas_usadas: None,
            enviada_em: Some(Utc.with_ymd_and_hms(2026, 11, 26, 10, minuto, 0).unwrap()),
        }
    }

    #[test]
    fn fica_so_o_maior_placar_de_cada_jogador() {
        let ficam = recordes(&[r("a", 100, None, false, 1), r("b", 300, Some("highscore"), false, 2), r("c", 200, None, false, 3)]);
        assert_eq!(ficam, HashSet::from(["b".to_string()]));
    }

    #[test]
    fn a_zerada_fica_pela_tabela_1cc_mesmo_abaixo_do_recorde() {
        let ficam = recordes(&[r("alto", 500, None, false, 1), r("zerou", 400, None, true, 2), r("zerou-baixo", 300, None, true, 3)]);
        assert_eq!(ficam, HashSet::from(["alto".to_string(), "zerou".to_string()]));
    }

    #[test]
    fn modos_versoes_e_eventos_sao_tabelas_separadas() {
        let mut evento = r("evento", 50, None, false, 1);
        evento.evento = Some("copa".into());
        let mut outra_versao = r("versao", 60, None, false, 2);
        outra_versao.versao = Some("pc-1.0".into());
        let ficam = recordes(&[r("hs", 900, None, false, 3), r("caravan", 10, Some("caravan-sprint"), false, 4), evento, outra_versao]);
        assert_eq!(ficam.len(), 4);
    }

    #[test]
    fn no_caravan_fica_o_maior_placar_e_depois_tempo_mortes_e_bombas() {
        let marca = |id: &str, score: i64, tempo: Option<i64>, mortes: Option<i64>, bombas: Option<i64>, minuto: u32| Registro {
            vidas_perdidas: mortes,
            sobrevivencia_ms: tempo,
            bombas_usadas: bombas,
            ..r(id, score, Some("caravan-classic"), false, minuto)
        };
        // O maior placar vence, mesmo com mais mortes.
        let ficam = recordes(&[marca("mais-pontos", 900, Some(100_000), Some(2), Some(0), 1), marca("menos-vidas", 500, Some(300_000), Some(0), Some(0), 2)]);
        assert_eq!(ficam, HashSet::from(["mais-pontos".to_string()]));
        // Mesmo placar: mais tempo vivido, depois menos mortes, depois menos bombas.
        let ficam = recordes(&[marca("a", 400, Some(200_000), Some(0), Some(0), 1), marca("b", 400, Some(300_000), Some(2), Some(5), 2)]);
        assert_eq!(ficam, HashSet::from(["b".to_string()]));
        let ficam = recordes(&[marca("a", 400, Some(300_000), Some(1), Some(0), 1), marca("b", 400, Some(300_000), Some(0), Some(3), 2)]);
        assert_eq!(ficam, HashSet::from(["b".to_string()]));
        let ficam = recordes(&[marca("a", 400, Some(300_000), Some(0), Some(2), 1), marca("b", 400, Some(300_000), Some(0), Some(1), 2)]);
        assert_eq!(ficam, HashSet::from(["b".to_string()]));
    }

    #[test]
    fn empate_fica_o_mais_antigo() {
        let ficam = recordes(&[r("depois", 100, None, false, 5), r("antes", 100, None, false, 1)]);
        assert_eq!(ficam, HashSet::from(["antes".to_string()]));
    }
}
