//! Totais do jogador por jogo, num documento só (`playerStats/{uid}_{gameId}`): fichas gastas (runs
//! valendo ranking começadas), tempo de jogo e o começo da última run. A sala do Client mostra esses
//! números. Eles não vêm mais das gameplays porque, com o ranking online só de recordes
//! (`recordes`), as runs abaixo do recorde saem do banco.
//!
//! Nada aqui derruba a run: uma falha vira só log.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::error::AppError;
use crate::firestore::valor::{data, inteiro, ler_data, ler_inteiro, ler_mapa, texto};
use crate::firestore::{Campos, Firestore};

/// Teto de gameplays lidas para montar os totais de antes desta mudança.
const MAX_LIDAS: usize = 5000;

pub fn caminho(uid: &str, jogo: &str) -> String {
    format!("playerStats/{uid}_{jogo}")
}

/// Totais de um conjunto de gameplays: quantas, os segundos jogados (início ao fim registrados
/// pelo servidor, no máximo a duração da ficha) e o começo da última.
fn totais(gameplays: &[Campos]) -> (i64, i64, Option<DateTime<Utc>>) {
    let mut segundos = 0;
    let mut ultima: Option<DateTime<Utc>> = None;
    for g in gameplays {
        let captura = ler_mapa(g, "capture").unwrap_or_default();
        let inicio = ler_data(&captura, "serverStartedAt");
        let fim = ler_data(&captura, "serverEndedAt").or_else(|| ler_data(g, "updatedAt"));
        let max = ler_inteiro(&captura, "maxDurationSeconds").unwrap_or(i64::MAX);
        if let (Some(inicio), Some(fim)) = (inicio, fim) {
            segundos += (fim - inicio).num_seconds().clamp(0, max);
        }
        let comecou = inicio.or_else(|| ler_data(g, "createdAt"));
        if comecou > ultima {
            ultima = comecou;
        }
    }
    (gameplays.len() as i64, segundos, ultima)
}

/// Cria os totais do jogador neste jogo, se ainda não existem, com o que as gameplays dele já
/// registram (as de antes desta mudança). Chamar antes de começar a run nova.
pub async fn garantir(fs: &Firestore, uid: &str, jogo: &str) {
    if let Err(e) = garantir_ou_erro(fs, uid, jogo).await {
        eprintln!("estatisticas: {uid}/{jogo}: {e:?}");
    }
}

async fn garantir_ou_erro(fs: &Firestore, uid: &str, jogo: &str) -> Result<(), AppError> {
    let doc = caminho(uid, jogo);
    if fs.ler_documento(&doc).await?.is_some() {
        return Ok(());
    }
    let igual = |campo: &str, valor: &str| json!({ "fieldFilter": { "field": { "fieldPath": campo }, "op": "EQUAL", "value": { "stringValue": valor } } });
    let campos_lidos: Vec<Value> = ["capture", "createdAt", "updatedAt"].iter().map(|c| json!({ "fieldPath": c })).collect();
    let consulta = json!({
        "from": [{ "collectionId": "gameplays" }],
        "where": { "compositeFilter": { "op": "AND", "filters": [igual("ownerId", uid), igual("gameId", jogo)] } },
        "select": { "fields": campos_lidos },
        "limit": MAX_LIDAS,
    });
    let gameplays: Vec<Campos> = fs.consultar(consulta).await?.into_iter().map(|(_, c)| c).collect();
    let (fichas, segundos, ultima) = totais(&gameplays);
    let mut campos: Campos = [("ownerId", texto(uid)), ("gameId", texto(jogo)), ("credits", inteiro(fichas)), ("seconds", inteiro(segundos))]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let mut mascara = vec!["ownerId", "gameId", "credits", "seconds"];
    if let Some(ultima) = ultima {
        campos.insert("lastStartedAt".into(), data(ultima));
        mascara.push("lastStartedAt");
    }
    fs.gravar(&doc, campos, &mascara).await
}

/// Run valendo ranking começou: mais uma ficha gasta neste jogo.
pub async fn run_comecou(fs: &Firestore, uid: &str, jogo: &str, quando: DateTime<Utc>) {
    let doc = caminho(uid, jogo);
    let campos: Campos = [("ownerId", texto(uid)), ("gameId", texto(jogo)), ("lastStartedAt", data(quando))]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let feito = async {
        fs.gravar(&doc, campos, &["ownerId", "gameId", "lastStartedAt"]).await?;
        fs.somar(&[(doc.clone(), "credits".to_string(), 1)]).await
    }
    .await;
    if let Err(e) = feito {
        eprintln!("estatisticas: run começou ({uid}/{jogo}): {e:?}");
    }
}

/// Run terminou (STOP, jogo fechado ou limite da ficha): soma o tempo jogado.
pub async fn run_terminou(fs: &Firestore, uid: &str, jogo: &str, segundos: i64) {
    if segundos <= 0 {
        return;
    }
    if let Err(e) = fs.somar(&[(caminho(uid, jogo), "seconds".to_string(), segundos)]).await {
        eprintln!("estatisticas: run terminou ({uid}/{jogo}): {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firestore::valor::mapa;
    use chrono::TimeZone;

    fn gameplay(inicio_min: u32, fim_min: Option<u32>, max: i64) -> Campos {
        let hora = |m: u32| Utc.with_ymd_and_hms(2026, 11, 26, 10, m, 0).unwrap();
        let mut captura: Campos = [("serverStartedAt".to_string(), data(hora(inicio_min))), ("maxDurationSeconds".to_string(), inteiro(max))].into_iter().collect();
        if let Some(fim) = fim_min {
            captura.insert("serverEndedAt".into(), data(hora(fim)));
        }
        [("capture".to_string(), mapa(captura))].into_iter().collect()
    }

    #[test]
    fn totais_das_gameplays_antigas() {
        // 10 min; 30 min limitados aos 20 da ficha; uma sem fim registrado.
        let (fichas, segundos, ultima) = totais(&[gameplay(0, Some(10), 3600), gameplay(20, Some(50), 1200), gameplay(55, None, 3600)]);
        assert_eq!(fichas, 3);
        assert_eq!(segundos, 600 + 1200);
        assert_eq!(ultima, Some(Utc.with_ymd_and_hms(2026, 11, 26, 10, 55, 0).unwrap()));
    }
}
