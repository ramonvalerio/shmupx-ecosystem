//! Tempo conectado de cada jogador (Users Manager do Lab): a cada minuto, quem está online, ausente,
//! jogando ou ao vivo ganha o tempo desde a última contagem em `players/{uid}.connectedSeconds`.
//!
//! A contagem roda junto das requisições (`distribuicoes::acordar`), e o Cloud Scheduler
//! `shmupx-distribuicoes-agendadas` chama o backend a cada minuto. `metrics/connectedTime.lastTickAt`
//! garante uma contagem por vez entre instâncias; um intervalo maior que [`MAXIMO_POR_CONTAGEM`]
//! (backend parado) conta só esse máximo, para não somar tempo que não dá para confirmar.

use chrono::{DateTime, Duration, Utc};
use serde_json::json;

use crate::error::AppError;
use crate::firestore::valor::{data, ler_data};
use crate::firestore::Firestore;
use crate::services::usuarios;

const MARCADOR: &str = "metrics/connectedTime";
/// Duas contagens com menos que isso de intervalo: a segunda não conta (outra instância já contou).
const INTERVALO_MINIMO_SEGUNDOS: i64 = 45;
/// Teto de uma contagem (a presença vale por 75 s; o sinal do Client aberto, por 120 s).
pub const MAXIMO_POR_CONTAGEM: i64 = 90;

/// Segundos a somar nesta contagem, ou `None` se a última foi há pouco.
pub fn segundos_desde(ultima: Option<DateTime<Utc>>, agora: DateTime<Utc>) -> Option<i64> {
    match ultima {
        None => Some(60),
        Some(ultima) if agora - ultima < Duration::seconds(INTERVALO_MINIMO_SEGUNDOS) => None,
        Some(ultima) => Some((agora - ultima).num_seconds().clamp(0, MAXIMO_POR_CONTAGEM)),
    }
}

/// Ocupa a contagem deste minuto (transação no marcador) e devolve os segundos a somar.
async fn ocupar(fs: &Firestore, agora: DateTime<Utc>) -> Result<Option<i64>, AppError> {
    let tx = fs.iniciar_transacao().await?;
    let resultado: Result<Option<i64>, AppError> = async {
        let ultima = fs.ler(MARCADOR, &tx).await?.and_then(|c| ler_data(&c, "lastTickAt"));
        let Some(segundos) = segundos_desde(ultima, agora) else { return Ok(None) };
        let escrita = json!({
            "update": { "name": fs.nome(MARCADOR), "fields": { "lastTickAt": data(agora) } },
            "updateMask": { "fieldPaths": ["lastTickAt"] },
        });
        fs.confirmar(&tx, vec![escrita]).await?;
        Ok(Some(segundos))
    }
    .await;
    if !matches!(resultado, Ok(Some(_))) {
        fs.desfazer(&tx).await;
    }
    resultado
}

/// Soma o tempo desde a última contagem a quem está conectado agora. Devolve quantos jogadores.
pub async fn contabilizar(fs: &Firestore) -> Result<usize, AppError> {
    let Some(segundos) = ocupar(fs, Utc::now()).await? else { return Ok(0) };
    let uids = usuarios::conectados_agora(fs).await?;
    if segundos > 0 && !uids.is_empty() {
        let itens: Vec<(String, String, i64)> = uids.iter().map(|uid| (format!("players/{uid}"), "connectedSeconds".to_string(), segundos)).collect();
        fs.somar(&itens).await?;
    }
    Ok(uids.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segundos_por_contagem() {
        let agora = Utc::now();
        assert_eq!(segundos_desde(None, agora), Some(60));
        assert_eq!(segundos_desde(Some(agora - Duration::seconds(20)), agora), None, "outra instância acabou de contar");
        assert_eq!(segundos_desde(Some(agora - Duration::seconds(61)), agora), Some(61));
        assert_eq!(segundos_desde(Some(agora - Duration::hours(5)), agora), Some(MAXIMO_POR_CONTAGEM), "backend parado não soma horas");
    }

    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn soma_quem_esta_conectado_no_emulador() {
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let (on, off) = (format!("on-{}", uuid::Uuid::new_v4()), format!("off-{}", uuid::Uuid::new_v4()));
        let presenca = |quando: DateTime<Utc>| {
            let mut c = crate::firestore::Campos::new();
            c.insert("state".into(), crate::firestore::valor::texto("away"));
            c.insert("lastSeen".into(), data(quando));
            c
        };
        fs.gravar_novo(&format!("rooms/sala-t/presence/{on}"), presenca(Utc::now())).await.unwrap();
        fs.gravar_novo(&format!("rooms/sala-t/presence/{off}"), presenca(Utc::now() - Duration::minutes(10))).await.unwrap();
        // Última contagem há 70 s: esta soma 70 s.
        let mut marcador = crate::firestore::Campos::new();
        marcador.insert("lastTickAt".into(), data(Utc::now() - Duration::seconds(70)));
        fs.gravar(MARCADOR, marcador, &["lastTickAt"]).await.unwrap();

        assert!(contabilizar(&fs).await.unwrap() >= 1);
        assert_eq!(contabilizar(&fs).await.unwrap(), 0, "logo em seguida não conta de novo");
        let segundos = |uid: String| {
            let fs = &fs;
            async move { fs.ler_documento(&format!("players/{uid}")).await.unwrap().and_then(|c| crate::firestore::valor::ler_inteiro(&c, "connectedSeconds")) }
        };
        let somado = segundos(on.clone()).await.unwrap();
        assert!((70..=72).contains(&somado), "somou {somado}");
        assert_eq!(segundos(off).await, None, "offline não soma");
    }
}
