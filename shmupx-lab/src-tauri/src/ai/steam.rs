//! Consulta à loja da Steam (API pública, sem chave e sem custo). Os dados dela são a fonte mais
//! confiável para o que vem da Steam (app id, desenvolvedoras, publicadora e data): servem de
//! contexto para a IA e para conferir o que ela devolve.

use std::time::Duration;

use serde_json::Value;

const BUSCA: &str = "https://store.steampowered.com/api/storesearch/";
const DETALHES: &str = "https://store.steampowered.com/api/appdetails";
const MAX_JOGOS: usize = 3;
const TIMEOUT: Duration = Duration::from_secs(10);
const ANO_MINIMO: i64 = 1970;
const ANO_MAXIMO: i64 = 2100;

#[derive(Debug, Clone, PartialEq)]
pub struct JogoSteam {
    pub app_id: i64,
    pub nome: String,
    pub desenvolvedoras: Vec<String>,
    pub publicadoras: Vec<String>,
    /// Texto da loja, ex.: "31 jul. 2025" (ou "Em breve").
    pub lancamento: String,
    pub ano: Option<i64>,
}

/// Procura o título na loja e traz os detalhes dos primeiros resultados. Um jogo cujos detalhes
/// falharem ainda entra só com id e nome, o que basta para conferir um `app_id`.
pub async fn buscar(titulo: &str) -> Result<Vec<JogoSteam>, String> {
    let http = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("falha ao construir cliente HTTP: {e}"))?;

    let busca: Value = http
        .get(BUSCA)
        .query(&[("term", titulo), ("cc", "br"), ("l", "portuguese")])
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("falha ao consultar a loja da Steam: {e}"))?
        .json()
        .await
        .map_err(|e| format!("resposta da Steam em formato inesperado: {e}"))?;

    let mut jogos = Vec::new();
    for (app_id, nome) in ids_da_busca(&busca) {
        let detalhes = http
            .get(DETALHES)
            .query(&[("appids", app_id.to_string().as_str()), ("cc", "br"), ("l", "portuguese")])
            .send()
            .await
            .ok()
            .filter(|r| r.status().is_success());
        let detalhes = match detalhes {
            Some(resposta) => resposta.json::<Value>().await.ok(),
            None => None,
        };
        jogos.push(jogo_de(app_id, &nome, detalhes.as_ref()));
    }
    Ok(jogos)
}

/// `(app_id, nome)` dos primeiros resultados que são jogos (ignora DLC, trilhas, pacotes).
fn ids_da_busca(busca: &Value) -> Vec<(i64, String)> {
    busca["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["type"] == "app")
        .filter_map(|item| Some((item["id"].as_i64()?, item["name"].as_str()?.to_string())))
        .take(MAX_JOGOS)
        .collect()
}

fn textos(valor: &Value) -> Vec<String> {
    valor
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|texto| texto.trim().to_string())
        .filter(|texto| !texto.is_empty())
        .collect()
}

/// Monta o jogo a partir da resposta de `appdetails` (`{"<id>": {"success": true, "data": {...}}}`).
fn jogo_de(app_id: i64, nome_da_busca: &str, detalhes: Option<&Value>) -> JogoSteam {
    let dados = detalhes
        .map(|corpo| &corpo[app_id.to_string()])
        .filter(|entrada| entrada["success"] == true)
        .map(|entrada| &entrada["data"]);
    let Some(dados) = dados else {
        return JogoSteam {
            app_id,
            nome: nome_da_busca.to_string(),
            desenvolvedoras: vec![],
            publicadoras: vec![],
            lancamento: String::new(),
            ano: None,
        };
    };
    let lancamento = dados["release_date"]["date"].as_str().unwrap_or_default().to_string();
    JogoSteam {
        app_id,
        nome: dados["name"].as_str().unwrap_or(nome_da_busca).to_string(),
        desenvolvedoras: textos(&dados["developers"]),
        publicadoras: textos(&dados["publishers"]),
        ano: ano_da_data(&lancamento),
        lancamento,
    }
}

/// Ano de uma data da loja em qualquer formato ("31 jul. 2025", "Jul 31, 2025", "2025").
fn ano_da_data(data: &str) -> Option<i64> {
    data.split(|c: char| !c.is_ascii_digit())
        .filter(|parte| parte.len() == 4)
        .filter_map(|parte| parte.parse::<i64>().ok())
        .find(|ano| (ANO_MINIMO..=ANO_MAXIMO).contains(ano))
}

/// Resumo dos jogos para o prompt da IA, um por linha.
pub fn resumo(jogos: &[JogoSteam]) -> String {
    jogos
        .iter()
        .map(|jogo| {
            format!(
                "- app_id {}: \"{}\" | developers: {} | publishers: {} | release date: {}",
                jogo.app_id,
                jogo.nome,
                lista_ou_desconhecido(&jogo.desenvolvedoras),
                lista_ou_desconhecido(&jogo.publicadoras),
                if jogo.lancamento.is_empty() { "unknown" } else { &jogo.lancamento },
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn lista_ou_desconhecido(itens: &[String]) -> String {
    if itens.is_empty() { "unknown".to_string() } else { itens.join(" / ") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn busca_ignora_o_que_nao_e_jogo_e_limita_o_numero() {
        let busca = json!({ "items": [
            { "type": "app", "id": 1, "name": "A" },
            { "type": "dlc", "id": 2, "name": "A DLC" },
            { "type": "app", "id": 3, "name": "B" },
            { "type": "app", "id": 4, "name": "C" },
            { "type": "app", "id": 5, "name": "D" },
            { "type": "app", "name": "sem id" },
        ]});
        let ids = ids_da_busca(&busca);
        assert_eq!(ids, vec![(1, "A".into()), (3, "B".into()), (4, "C".into())]);
        assert!(ids_da_busca(&json!({})).is_empty());
    }

    #[test]
    fn detalhes_trazem_desenvolvedoras_publicadora_e_ano() {
        let corpo = json!({ "3597580": { "success": true, "data": {
            "name": "Earthion",
            "developers": ["Ancient Corporation", "Bitwave Games"],
            "publishers": ["Limited Run Games, Inc."],
            "release_date": { "coming_soon": false, "date": "31 jul. 2025" },
        }}});
        let jogo = jogo_de(3597580, "busca", Some(&corpo));
        assert_eq!(jogo.nome, "Earthion");
        assert_eq!(jogo.desenvolvedoras, vec!["Ancient Corporation", "Bitwave Games"]);
        assert_eq!(jogo.publicadoras, vec!["Limited Run Games, Inc."]);
        assert_eq!(jogo.ano, Some(2025));
    }

    #[test]
    fn detalhes_ausentes_ou_com_falha_ficam_so_com_id_e_nome() {
        for corpo in [None, Some(json!({ "7": { "success": false } })), Some(json!({}))] {
            let jogo = jogo_de(7, "Nome da busca", corpo.as_ref());
            assert_eq!((jogo.app_id, jogo.nome.as_str()), (7, "Nome da busca"));
            assert!(jogo.desenvolvedoras.is_empty() && jogo.ano.is_none());
        }
    }

    #[test]
    fn ano_vem_de_varios_formatos() {
        assert_eq!(ano_da_data("31 jul. 2025"), Some(2025));
        assert_eq!(ano_da_data("Jul 31, 2025"), Some(2025));
        assert_eq!(ano_da_data("2025"), Some(2025));
        assert_eq!(ano_da_data("Em breve"), None);
        assert_eq!(ano_da_data("31 jul. 20255"), None);
        assert_eq!(ano_da_data(""), None);
    }

    /// Contra a loja real da Steam (rede, sem custo): confere o parser com o JSON verdadeiro.
    /// `cargo test -- --ignored steam`
    #[tokio::test]
    #[ignore = "exige acesso à rede"]
    async fn loja_real_devolve_o_earthion() {
        let jogos = buscar("earthion").await.unwrap();
        let jogo = jogos.iter().find(|jogo| jogo.nome == "Earthion").expect("Earthion nos resultados");
        assert_eq!(jogo.app_id, 3597580);
        assert!(jogo.desenvolvedoras.iter().any(|d| d.contains("Ancient")), "{jogo:?}");
        assert!(!jogo.publicadoras.is_empty() && jogo.ano.is_some(), "{jogo:?}");
    }

    #[test]
    fn resumo_lista_um_jogo_por_linha() {
        let jogo = JogoSteam {
            app_id: 9,
            nome: "X".into(),
            desenvolvedoras: vec!["A".into(), "B".into()],
            publicadoras: vec![],
            lancamento: String::new(),
            ano: None,
        };
        let texto = resumo(&[jogo]);
        assert_eq!(texto, "- app_id 9: \"X\" | developers: A / B | publishers: unknown | release date: unknown");
    }
}
