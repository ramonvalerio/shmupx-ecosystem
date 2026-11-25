//! Planos de assinatura: `plans/{planId}` no Firestore. Quem escreve é o admin, pelo Lab
//! (`/api/admin/plans`). Por enquanto só o cadastro: o que cada plano inclui e quanto custa; a
//! cobrança e quem assina chegam com o pagamento.

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::firestore::valor::{booleano, inteiro, lista_de_textos, ler_booleano, ler_inteiro, ler_lista_de_textos, ler_texto, texto};
use crate::firestore::{Campos, DocumentoComDatas, Firestore};
use crate::services::gameplays::id_valido;

const NOME_MAXIMO: usize = 40;
const DESCRICAO_MAXIMA: usize = 200;
const BENEFICIOS_MAXIMOS: usize = 10;
const BENEFICIO_MAXIMO: usize = 100;
/// R$ 10.000,00 em centavos: acima disso é erro de digitação.
const PRECO_MAXIMO_CENTAVOS: i64 = 1_000_000;
const FICHAS_MAXIMAS: i64 = 10_000;
/// Período da cobrança.
const PERIODOS: [&str; 4] = ["monthly", "quarterly", "semiannual", "yearly"];
const CAMPOS: [&str; 8] = ["name", "description", "priceCents", "period", "coinsPerPeriod", "benefits", "active", "order"];

/// Campos que o Lab define. O `planId` vem da rota.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanoEntrada {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Preço por período, em centavos de real (`0` = grátis).
    pub price_cents: i64,
    pub period: String,
    /// Fichas que o assinante recebe a cada período.
    #[serde(default)]
    pub coins_per_period: i64,
    #[serde(default)]
    pub benefits: Vec<String>,
    /// Desligado, o plano fica guardado mas não é oferecido.
    #[serde(default)]
    pub active: bool,
    /// Ordem na lista (menor primeiro).
    #[serde(default)]
    pub order: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Plano {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price_cents: i64,
    pub period: String,
    pub coins_per_period: i64,
    pub benefits: Vec<String>,
    pub active: bool,
    pub order: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

fn validar(id: &str, e: &mut PlanoEntrada) -> Result<(), AppError> {
    if !id_valido(id) {
        return Err(AppError::bad_request("ID do plano inválido: use letras minúsculas, números e hífen"));
    }
    e.name = e.name.trim().to_string();
    e.description = e.description.trim().to_string();
    e.benefits = e.benefits.iter().map(|b| b.trim().to_string()).filter(|b| !b.is_empty()).collect();
    if e.name.is_empty() || e.name.chars().count() > NOME_MAXIMO {
        return Err(AppError::bad_request(format!("o nome do plano precisa ter de 1 a {NOME_MAXIMO} caracteres")));
    }
    if e.description.chars().count() > DESCRICAO_MAXIMA {
        return Err(AppError::bad_request(format!("a descrição passa de {DESCRICAO_MAXIMA} caracteres")));
    }
    if !(0..=PRECO_MAXIMO_CENTAVOS).contains(&e.price_cents) {
        return Err(AppError::bad_request("preço inválido (de R$ 0,00 a R$ 10.000,00)"));
    }
    if !PERIODOS.contains(&e.period.as_str()) {
        return Err(AppError::bad_request("período inválido: use monthly, quarterly, semiannual ou yearly"));
    }
    if !(0..=FICHAS_MAXIMAS).contains(&e.coins_per_period) {
        return Err(AppError::bad_request(format!("fichas por período: de 0 a {FICHAS_MAXIMAS}")));
    }
    if e.benefits.len() > BENEFICIOS_MAXIMOS || e.benefits.iter().any(|b| b.chars().count() > BENEFICIO_MAXIMO) {
        return Err(AppError::bad_request(format!("até {BENEFICIOS_MAXIMOS} benefícios, cada um com até {BENEFICIO_MAXIMO} caracteres")));
    }
    Ok(())
}

fn campos(e: &PlanoEntrada) -> Campos {
    [
        ("name", texto(&e.name)),
        ("description", texto(&e.description)),
        ("priceCents", inteiro(e.price_cents)),
        ("period", texto(&e.period)),
        ("coinsPerPeriod", inteiro(e.coins_per_period)),
        ("benefits", lista_de_textos(&e.benefits)),
        ("active", booleano(e.active)),
        ("order", inteiro(e.order)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

fn plano(d: DocumentoComDatas) -> Plano {
    let c = &d.campos;
    Plano {
        name: ler_texto(c, "name").unwrap_or_default(),
        description: ler_texto(c, "description").unwrap_or_default(),
        price_cents: ler_inteiro(c, "priceCents").unwrap_or(0),
        period: ler_texto(c, "period").unwrap_or_else(|| "monthly".into()),
        coins_per_period: ler_inteiro(c, "coinsPerPeriod").unwrap_or(0),
        benefits: ler_lista_de_textos(c, "benefits"),
        active: ler_booleano(c, "active").unwrap_or(false),
        order: ler_inteiro(c, "order").unwrap_or(0),
        created_at: d.criado_em,
        updated_at: d.atualizado_em,
        id: d.id,
    }
}

/// Todos os planos, na ordem do Lab (depois pelo nome).
pub async fn listar(fs: &Firestore) -> Result<Vec<Plano>, AppError> {
    let mut planos: Vec<Plano> = fs.listar_com_datas("plans").await?.into_iter().map(plano).collect();
    planos.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.name.cmp(&b.name)));
    Ok(planos)
}

/// Cria ou atualiza o plano (todos os campos; o que não vier fica com o padrão).
pub async fn salvar(fs: &Firestore, id: &str, mut entrada: PlanoEntrada) -> Result<Plano, AppError> {
    validar(id, &mut entrada)?;
    let caminho = format!("plans/{id}");
    fs.gravar(&caminho, campos(&entrada), &CAMPOS).await?;
    fs.ler_documento_com_datas(&caminho).await?.map(plano).ok_or_else(|| AppError::internal("o plano não foi gravado"))
}

pub async fn excluir(fs: &Firestore, id: &str) -> Result<(), AppError> {
    if !id_valido(id) {
        return Err(AppError::not_found("plano não encontrado"));
    }
    fs.apagar(&format!("plans/{id}")).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrada() -> PlanoEntrada {
        PlanoEntrada { name: " Pro ".into(), price_cents: 1990, period: "monthly".into(), coins_per_period: 30, benefits: vec![" Live ".into(), "".into()], ..Default::default() }
    }

    #[test]
    fn valida_e_limpa() {
        let mut e = entrada();
        assert!(validar("pro", &mut e).is_ok());
        assert_eq!(e.name, "Pro");
        assert_eq!(e.benefits, vec!["Live"]);
    }

    #[test]
    fn recusa_periodo_preco_e_id_invalidos() {
        assert!(validar("pro", &mut PlanoEntrada { period: "weekly".into(), ..entrada() }).is_err());
        assert!(validar("pro", &mut PlanoEntrada { price_cents: -1, ..entrada() }).is_err());
        assert!(validar("Pro Plan", &mut entrada()).is_err());
        assert!(validar("pro", &mut PlanoEntrada { name: "  ".into(), ..entrada() }).is_err());
    }
}
