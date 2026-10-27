//! Conversão entre valores Rust e o formato tipado da API REST do Firestore
//! (`{"integerValue": "42"}`, `{"stringValue": "..."}`, ...).

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{json, Value};

use super::Campos;

pub fn texto(valor: &str) -> Value {
    json!({ "stringValue": valor })
}

pub fn inteiro(valor: i64) -> Value {
    // A API representa int64 como string para não perder precisão em JSON.
    json!({ "integerValue": valor.to_string() })
}

pub fn data(valor: DateTime<Utc>) -> Value {
    json!({ "timestampValue": valor.to_rfc3339_opts(SecondsFormat::Micros, true) })
}

pub fn mapa(campos: Campos) -> Value {
    json!({ "mapValue": { "fields": campos } })
}

pub fn ler_texto(campos: &Campos, nome: &str) -> Option<String> {
    campos.get(nome)?.get("stringValue")?.as_str().map(str::to_string)
}

pub fn ler_inteiro(campos: &Campos, nome: &str) -> Option<i64> {
    campos.get(nome)?.get("integerValue")?.as_str()?.parse().ok()
}

pub fn ler_booleano(campos: &Campos, nome: &str) -> Option<bool> {
    campos.get(nome)?.get("booleanValue")?.as_bool()
}

pub fn ler_data(campos: &Campos, nome: &str) -> Option<DateTime<Utc>> {
    let texto = campos.get(nome)?.get("timestampValue")?.as_str()?;
    DateTime::parse_from_rfc3339(texto).ok().map(|data| data.with_timezone(&Utc))
}

pub fn ler_mapa(campos: &Campos, nome: &str) -> Option<Campos> {
    campos.get(nome)?.get("mapValue")?.get("fields")?.as_object().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn campos(pares: &[(&str, Value)]) -> Campos {
        pares.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn inteiro_ida_e_volta() {
        let c = campos(&[("saldo", inteiro(49))]);
        assert_eq!(ler_inteiro(&c, "saldo"), Some(49));
    }

    #[test]
    fn data_ida_e_volta_preserva_microssegundos() {
        let instante = DateTime::parse_from_rfc3339("2026-10-26T18:30:00.123456Z").unwrap().with_timezone(&Utc);
        let c = campos(&[("criado", data(instante))]);
        assert_eq!(ler_data(&c, "criado"), Some(instante));
    }

    #[test]
    fn mapa_aninhado() {
        let interno = campos(&[("max", inteiro(3600))]);
        let c = campos(&[("capture", mapa(interno))]);
        assert_eq!(ler_inteiro(&ler_mapa(&c, "capture").unwrap(), "max"), Some(3600));
    }

    #[test]
    fn tipo_divergente_nao_e_lido() {
        let c = campos(&[("saldo", texto("49"))]);
        assert_eq!(ler_inteiro(&c, "saldo"), None);
    }
}
