//! Conhecimento curado do RobotX (ADR 006): perguntas e respostas por jogo, escritas pelo admin
//! no Lab. Fica no próprio `games/{gameId}` (campo `robotxQa`), que o Client já lê em tempo real
//! para os jogos publicados: o RobotX responde no PC do jogador, sem IA e sem custo, buscando a
//! pergunta mais parecida. Gravar só toca esse campo (máscara), preservando o resto do catálogo.

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::firestore::valor::{ler_lista_de_mapas, ler_texto, lista_de_mapas, texto};
use crate::firestore::{Campos, Firestore};
use crate::services::catalogo;
use crate::services::versoes::Mudanca;

const CAMPO: &str = "robotxQa";
const MAXIMO_ITENS: usize = 300;
const PERGUNTA_MAXIMA: usize = 200;
const RESPOSTA_MAXIMA: usize = 1000;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemRobotx {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConhecimentoEntrada {
    pub items: Vec<ItemRobotx>,
}

fn validar(itens: Vec<ItemRobotx>) -> Result<Vec<ItemRobotx>, AppError> {
    if itens.len() > MAXIMO_ITENS {
        return Err(AppError::bad_request(format!("no máximo {MAXIMO_ITENS} perguntas por jogo")));
    }
    let mut validos: Vec<ItemRobotx> = Vec::new();
    for (i, item) in itens.into_iter().enumerate() {
        let question = item.question.trim().to_string();
        let answer = item.answer.trim().to_string();
        // Linha totalmente vazia (o admin apagou o texto): ignorada em vez de erro.
        if question.is_empty() && answer.is_empty() {
            continue;
        }
        if question.is_empty() || answer.is_empty() {
            return Err(AppError::bad_request(format!("item {}: preencha a pergunta e a resposta", i + 1)));
        }
        if question.chars().count() > PERGUNTA_MAXIMA || answer.chars().count() > RESPOSTA_MAXIMA {
            return Err(AppError::bad_request(format!(
                "item {}: pergunta até {PERGUNTA_MAXIMA} e resposta até {RESPOSTA_MAXIMA} caracteres",
                i + 1
            )));
        }
        if !validos.iter().any(|v| v.question.to_lowercase() == question.to_lowercase()) {
            validos.push(ItemRobotx { question, answer });
        }
    }
    Ok(validos)
}

pub fn ler(campos: &Campos) -> Vec<ItemRobotx> {
    ler_lista_de_mapas(campos, CAMPO)
        .iter()
        .filter_map(|m| Some(ItemRobotx { question: ler_texto(m, "q")?, answer: ler_texto(m, "a")? }))
        .collect()
}

/// Perguntas e respostas do jogo; erro 404 se o jogo não existe.
pub async fn obter(fs: &Firestore, game_id: &str) -> Result<Vec<ItemRobotx>, AppError> {
    if catalogo::obter(fs, game_id).await?.is_none() {
        return Err(AppError::not_found("jogo não encontrado no catálogo"));
    }
    Ok(fs.ler_documento(&format!("games/{game_id}")).await?.map(|c| ler(&c)).unwrap_or_default())
}

/// Substitui o conhecimento do jogo. Devolve (antes, depois) para o rascunho da versão.
pub async fn salvar(
    fs: &Firestore,
    game_id: &str,
    entrada: ConhecimentoEntrada,
) -> Result<(Vec<ItemRobotx>, Vec<ItemRobotx>), AppError> {
    let antes = obter(fs, game_id).await?;
    let itens = validar(entrada.items)?;
    let mapas = itens
        .iter()
        .map(|i| Campos::from_iter([("q".to_string(), texto(&i.question)), ("a".to_string(), texto(&i.answer))]))
        .collect();
    let mut campos = Campos::new();
    campos.insert(CAMPO.to_string(), lista_de_mapas(mapas));
    fs.gravar(&format!("games/{game_id}"), campos, &[CAMPO]).await?;
    Ok((antes, itens))
}

/// Linha do rascunho da versão quando o RobotX passa a responder numa sala publicada.
pub fn mudanca(jogo: &catalogo::Jogo, antes: &[ItemRobotx], depois: &[ItemRobotx]) -> Option<Mudanca> {
    (jogo.published && antes.is_empty() && !depois.is_empty()).then(|| Mudanca {
        key: format!("game:{}:robotx", jogo.id),
        section: "features".into(),
        text: Some(format!("RobotX agora responde na sala de {}", jogo.title)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(q: &str, a: &str) -> ItemRobotx {
        ItemRobotx { question: q.into(), answer: a.into() }
    }

    #[test]
    fn valida_e_normaliza() {
        let ok = validar(vec![item(" Controles? ", " Z atira "), item("", ""), item("controles?", "dup")]).unwrap();
        assert_eq!(ok, vec![item("Controles?", "Z atira")]);
        assert!(validar(vec![item("Sem resposta", "")]).is_err());
        assert!(validar(vec![item(&"x".repeat(201), "a")]).is_err());
    }

    #[test]
    fn mudanca_so_na_primeira_vez_em_jogo_publicado() {
        let jogo = catalogo::Jogo { id: "earthion".into(), title: "Earthion".into(), published: true, ..Default::default() };
        assert!(mudanca(&jogo, &[], &[item("a", "b")]).is_some());
        assert!(mudanca(&jogo, &[item("a", "b")], &[item("a", "c")]).is_none());
        let rascunho = catalogo::Jogo { published: false, ..jogo };
        assert!(mudanca(&rascunho, &[], &[item("a", "b")]).is_none());
    }
}
