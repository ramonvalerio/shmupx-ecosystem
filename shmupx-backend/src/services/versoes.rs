//! Versões do ShmupX Client: `releases/{versão}` no Firestore.
//!
//! O admin escreve a versão no Lab (rascunho), com as novidades, correções, mudanças de regra e
//! os scripts de validação que ela leva. Ao **agendar**, escolhe a antecedência do aviso (urgente,
//! 24 h, 48 h ou 7 dias): o servidor fixa `releaseAt`, publica a notificação para os jogadores e
//! dispara os e-mails. O status depois disso é calculado pelo relógio do servidor: `scheduled`
//! até `releaseAt`, `released` a partir dele. Uma versão marcada como obrigatória bloqueia novas
//! runs de Clients mais antigos assim que é liberada (`exige_atualizacao`).
//!
//! Rascunho automático: cada alteração feita no Lab (jogo publicado, regra alterada, script
//! aprovado) vira um item em `auto` do rascunho aberto, criado na hora se não houver. Os itens
//! automáticos ficam separados do que o admin escreve à mão, então regenerá-los nunca apaga
//! texto manual. Ao agendar, os dois se juntam nas listas da versão.
//!
//! Instalador e atualização automática: a versão pode levar o instalador assinado (URL e
//! assinatura do updater do Tauri). Agendada com `autoUpdate`, o Client a baixa sozinho a partir
//! de `releaseAt` (`atualizacao`). Sem instalador, a versão só traz notas (`notesOnly`): mudanças
//! de catálogo, que já valem na hora, e não pede atualização do Client.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::firestore::valor::{
    booleano, data, inteiro, ler_booleano, ler_data, ler_inteiro, ler_lista_de_textos, ler_mapa, ler_texto, lista_de_textos, mapa, texto,
};
use crate::firestore::{Campos, Firestore};
use crate::services::gameplays::id_valido;

const TITULO_MAXIMO: usize = 80;
const ITEM_MAXIMO: usize = 300;
const ITENS_MAXIMOS: usize = 40;

/// Antecedência entre o aviso (agendamento) e a liberação no Client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Antecedencia {
    /// Libera na hora (correção urgente), como era antes dos avisos.
    Urgente,
    #[serde(rename = "24h")]
    Horas24,
    #[serde(rename = "48h")]
    Horas48,
    #[serde(rename = "7d")]
    Dias7,
}

impl Antecedencia {
    fn duracao(self) -> Duration {
        match self {
            Antecedencia::Urgente => Duration::zero(),
            Antecedencia::Horas24 => Duration::hours(24),
            Antecedencia::Horas48 => Duration::hours(48),
            Antecedencia::Dias7 => Duration::days(7),
        }
    }

    fn texto(self) -> &'static str {
        match self {
            Antecedencia::Urgente => "urgent",
            Antecedencia::Horas24 => "24h",
            Antecedencia::Horas48 => "48h",
            Antecedencia::Dias7 => "7d",
        }
    }

    fn de_texto(valor: &str) -> Option<Self> {
        [Antecedencia::Urgente, Antecedencia::Horas24, Antecedencia::Horas48, Antecedencia::Dias7]
            .into_iter()
            .find(|a| a.texto() == valor)
    }
}

/// O que o Lab grava num rascunho. A versão vem da rota, nunca do corpo.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VersaoEntrada {
    pub title: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub fixes: Vec<String>,
    #[serde(default)]
    pub rules: Vec<String>,
    /// Scripts de validação incluídos, como `jogo@versão` (ex.: `aero-fighters-2@0.2.0`).
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub mandatory: bool,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Versao {
    pub version: String,
    pub title: String,
    pub features: Vec<String>,
    pub fixes: Vec<String>,
    pub rules: Vec<String>,
    pub scripts: Vec<String>,
    pub mandatory: bool,
    /// `draft`, `scheduled`, `released` ou `cancelled` (os dois do meio pelo relógio do servidor).
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lead: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_at: Option<DateTime<Utc>>,
    /// Resumo do envio de e-mails (`not_configured`, `sending`, `sent N`, `failed: ...`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emails_sent: Option<i64>,
    /// Itens gerados pelas alterações no Lab, ainda separados do texto manual (só em rascunho).
    pub auto: Vec<ItemAuto>,
    /// Instalador do Windows (NSIS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer: Option<Instalador>,
    /// Pacotes Linux por formato (`appimage`, `deb`), cada um com a assinatura do updater.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub linux_installers: BTreeMap<String, Instalador>,
    /// ShmupX Portable do Windows (um `.exe` só, sem instalação).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portable: Option<Portatil>,
    /// Liberada, o Client baixa e instala sozinho (exige `installer`).
    pub auto_update: bool,
    /// Versão sem instalador: só notas, não pede atualização do Client.
    pub notes_only: bool,
}

/// Seções da versão onde um item pode entrar.
const SECOES: [&str; 4] = ["features", "fixes", "rules", "scripts"];

/// Item automático do rascunho. `key` identifica a origem (ex.: `game:earthion:sala`): a mesma
/// alteração repetida substitui o item em vez de duplicá-lo.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ItemAuto {
    pub key: String,
    pub section: String,
    pub text: String,
}

/// Mudança registrada por uma alteração no Lab. `text: None` retira o item (ex.: jogo publicado
/// e despublicado antes da versão sair).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Mudanca {
    pub key: String,
    pub section: String,
    #[serde(default)]
    pub text: Option<String>,
}

/// Instalador assinado da versão, já publicado em `www.shmupx.com/downloads/`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Instalador {
    pub url: String,
    /// Conteúdo do `.sig` gerado pelo `tauri build` com a chave do updater.
    pub signature: String,
    #[serde(default)]
    pub size: Option<i64>,
}

/// ShmupX Portable da versão, já publicado em `www.shmupx.com/downloads/`. Sem assinatura do
/// updater: o Client portátil confere o `sha256` antes de abrir a versão nova.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Portatil {
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub size: Option<i64>,
}

/// Escolhas do admin ao agendar.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpcoesAgendamento {
    pub auto_update: bool,
}

/// `1.2.3` → (1, 2, 3). Três números sem zeros à esquerda.
pub fn ler_versao(texto: &str) -> Option<(u32, u32, u32)> {
    let mut partes = texto.split('.');
    let mut parte = || -> Option<u32> {
        let p = partes.next()?;
        let ok = !p.is_empty() && p.len() <= 4 && p.chars().all(|c| c.is_ascii_digit()) && (p == "0" || !p.starts_with('0'));
        if ok { p.parse().ok() } else { None }
    };
    let versao = (parte()?, parte()?, parte()?);
    partes.next().is_none().then_some(versao)
}

fn lista(itens: Vec<String>, campo: &str) -> Result<Vec<String>, AppError> {
    let itens: Vec<String> = itens.into_iter().map(|i| i.trim().to_string()).filter(|i| !i.is_empty()).collect();
    if itens.len() > ITENS_MAXIMOS {
        return Err(AppError::bad_request(format!("{campo}: no máximo {ITENS_MAXIMOS} itens")));
    }
    if itens.iter().any(|i| i.chars().count() > ITEM_MAXIMO) {
        return Err(AppError::bad_request(format!("{campo}: cada item com no máximo {ITEM_MAXIMO} caracteres")));
    }
    Ok(itens)
}

fn validar(versao: &str, entrada: VersaoEntrada) -> Result<VersaoEntrada, AppError> {
    if ler_versao(versao).is_none() {
        return Err(AppError::bad_request("versão inválida (use o formato 1.2.3)"));
    }
    let title = entrada.title.trim().to_string();
    if title.is_empty() || title.chars().count() > TITULO_MAXIMO {
        return Err(AppError::bad_request("título deve ter de 1 a 80 caracteres"));
    }
    let scripts = lista(entrada.scripts, "scripts")?;
    for script in &scripts {
        let valido = script.split_once('@').is_some_and(|(jogo, v)| id_valido(jogo) && ler_versao(v).is_some());
        if !valido {
            return Err(AppError::bad_request(format!("script inválido: {script} (use jogo@1.2.3)")));
        }
    }
    let features = lista(entrada.features, "features")?;
    let fixes = lista(entrada.fixes, "fixes")?;
    let rules = lista(entrada.rules, "rules")?;
    // Rascunho pode ficar vazio (os itens automáticos chegam depois); a exigência de conteúdo
    // vale ao agendar.
    Ok(VersaoEntrada { title, features, fixes, rules, scripts, mandatory: entrada.mandatory })
}

/// Itens automáticos gravados em `auto` (`{chave: {section, text}}`), na ordem das chaves.
fn ler_auto(campos: &Campos) -> Vec<ItemAuto> {
    ler_mapa(campos, "auto")
        .map(|mapa| {
            mapa.into_iter()
                .filter_map(|(key, valor)| {
                    let item = valor.get("mapValue")?.get("fields")?.as_object()?.clone();
                    Some(ItemAuto { key, section: ler_texto(&item, "section")?, text: ler_texto(&item, "text")? })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn gravar_auto(itens: &[ItemAuto]) -> serde_json::Value {
    let mut campos = Campos::new();
    for item in itens {
        let mut interno = Campos::new();
        interno.insert("section".into(), texto(&item.section));
        interno.insert("text".into(), texto(&item.text));
        campos.insert(item.key.clone(), mapa(interno));
    }
    mapa(campos)
}

fn instalador_de(mapa: &Campos) -> Option<Instalador> {
    Some(Instalador { url: ler_texto(mapa, "url")?, signature: ler_texto(mapa, "signature")?, size: ler_inteiro(mapa, "size") })
}

fn ler_instalador(campos: &Campos) -> Option<Instalador> {
    instalador_de(&ler_mapa(campos, "installer")?)
}

fn ler_portatil(campos: &Campos) -> Option<Portatil> {
    let mapa = ler_mapa(campos, "portable")?;
    Some(Portatil { url: ler_texto(&mapa, "url")?, sha256: ler_texto(&mapa, "sha256")?, size: ler_inteiro(&mapa, "size") })
}

/// `linuxInstallers` (`{formato: {url, signature, size}}`).
fn ler_instaladores_linux(campos: &Campos) -> BTreeMap<String, Instalador> {
    ler_mapa(campos, "linuxInstallers")
        .map(|mapa| {
            mapa.into_iter()
                .filter_map(|(formato, valor)| {
                    let interno = valor.get("mapValue")?.get("fields")?.as_object()?.clone();
                    Some((formato, instalador_de(&interno)?))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Junta os itens automáticos às listas manuais, sem repetir texto já presente.
fn juntar_auto(mut versao: Versao) -> Versao {
    for item in std::mem::take(&mut versao.auto) {
        let lista = match item.section.as_str() {
            "features" => &mut versao.features,
            "fixes" => &mut versao.fixes,
            "rules" => &mut versao.rules,
            _ => &mut versao.scripts,
        };
        if !lista.contains(&item.text) {
            lista.push(item.text);
        }
    }
    versao
}

/// Status efetivo: um agendamento vira `released` quando chega a hora.
fn status_efetivo(guardado: &str, release_at: Option<DateTime<Utc>>, agora: DateTime<Utc>) -> String {
    match (guardado, release_at) {
        ("scheduled", Some(quando)) if agora >= quando => "released".into(),
        (outro, _) => outro.into(),
    }
}

fn versao_de(version: String, campos: &Campos, agora: DateTime<Utc>) -> Versao {
    let release_at = ler_data(campos, "releaseAt");
    Versao {
        title: ler_texto(campos, "title").unwrap_or_else(|| version.clone()),
        features: ler_lista_de_textos(campos, "features"),
        fixes: ler_lista_de_textos(campos, "fixes"),
        rules: ler_lista_de_textos(campos, "rules"),
        scripts: ler_lista_de_textos(campos, "scripts"),
        mandatory: ler_booleano(campos, "mandatory").unwrap_or(false),
        status: status_efetivo(&ler_texto(campos, "status").unwrap_or_else(|| "draft".into()), release_at, agora),
        lead: ler_texto(campos, "lead"),
        scheduled_at: ler_data(campos, "scheduledAt"),
        release_at,
        email_status: ler_texto(campos, "emailStatus"),
        emails_sent: ler_inteiro(campos, "emailsSent"),
        auto: ler_auto(campos),
        installer: ler_instalador(campos),
        linux_installers: ler_instaladores_linux(campos),
        portable: ler_portatil(campos),
        auto_update: ler_booleano(campos, "autoUpdate").unwrap_or(false),
        notes_only: ler_booleano(campos, "notesOnly").unwrap_or(false),
        version,
    }
}

/// Todas as versões, da mais nova para a mais antiga.
pub async fn listar(fs: &Firestore) -> Result<Vec<Versao>, AppError> {
    let agora = Utc::now();
    let mut versoes: Vec<Versao> =
        fs.listar("releases").await?.into_iter().map(|(id, campos)| versao_de(id, &campos, agora)).collect();
    versoes.sort_by_key(|v| std::cmp::Reverse(ler_versao(&v.version)));
    Ok(versoes)
}

pub async fn obter(fs: &Firestore, versao: &str) -> Result<Versao, AppError> {
    if ler_versao(versao).is_none() {
        return Err(AppError::bad_request("versão inválida"));
    }
    let campos = fs.ler_documento(&format!("releases/{versao}")).await?.ok_or_else(|| AppError::not_found("versão não encontrada"))?;
    Ok(versao_de(versao.to_string(), &campos, Utc::now()))
}

const MASCARA_RASCUNHO: [&str; 9] = ["title", "features", "fixes", "rules", "scripts", "mandatory", "status", "updatedAt", "version"];

/// Cria ou atualiza o rascunho. Versões já agendadas não mudam (o aviso já saiu com aquele texto).
pub async fn salvar_rascunho(fs: &Firestore, versao: &str, entrada: VersaoEntrada) -> Result<Versao, AppError> {
    let entrada = validar(versao, entrada)?;
    if let Some(campos) = fs.ler_documento(&format!("releases/{versao}")).await? {
        let atual = ler_texto(&campos, "status").unwrap_or_else(|| "draft".into());
        if atual != "draft" {
            return Err(AppError::conflict("só rascunhos podem ser editados; crie uma nova versão"));
        }
    }
    let mut campos = Campos::new();
    campos.insert("version".into(), texto(versao));
    campos.insert("title".into(), texto(&entrada.title));
    campos.insert("features".into(), lista_de_textos(&entrada.features));
    campos.insert("fixes".into(), lista_de_textos(&entrada.fixes));
    campos.insert("rules".into(), lista_de_textos(&entrada.rules));
    campos.insert("scripts".into(), lista_de_textos(&entrada.scripts));
    campos.insert("mandatory".into(), booleano(entrada.mandatory));
    campos.insert("status".into(), texto("draft"));
    campos.insert("updatedAt".into(), data(Utc::now()));
    fs.gravar(&format!("releases/{versao}"), campos, &MASCARA_RASCUNHO).await?;
    obter(fs, versao).await
}

/// Agenda o rascunho: fixa `releaseAt = agora + antecedência`. A versão precisa ser maior que
/// todas as já agendadas ou liberadas (o Client só anda para a frente).
/// Agenda juntando os itens automáticos ao texto manual. Com `auto_update`, exige o instalador.
pub async fn agendar_com(
    fs: &Firestore,
    versao: &str,
    antecedencia: Antecedencia,
    opcoes: OpcoesAgendamento,
) -> Result<Versao, AppError> {
    let atual = obter(fs, versao).await?;
    if atual.status != "draft" {
        return Err(AppError::conflict("só rascunhos podem ser agendados"));
    }
    if opcoes.auto_update && atual.installer.is_none() {
        return Err(AppError::bad_request("envie o instalador antes de agendar com atualização automática"));
    }
    let completa = juntar_auto(atual);
    if completa.features.is_empty() && completa.fixes.is_empty() && completa.rules.is_empty() && completa.scripts.is_empty() {
        return Err(AppError::bad_request("a versão precisa de ao menos uma novidade, correção, regra ou script"));
    }
    if !completa.scripts.is_empty() && completa.installer.is_none() {
        // Scripts de validação vão dentro do instalador: sem ele, ninguém os recebe.
        return Err(AppError::bad_request("esta versão leva scripts de validação: envie o instalador antes de agendar"));
    }
    let nova = ler_versao(versao);
    let maior_ativa = listar(fs)
        .await?
        .into_iter()
        .filter(|v| v.status == "scheduled" || v.status == "released")
        .filter_map(|v| ler_versao(&v.version))
        .max();
    if maior_ativa.is_some_and(|maior| nova <= Some(maior)) {
        return Err(AppError::conflict("a versão precisa ser maior que todas as já agendadas ou liberadas"));
    }
    let agora = Utc::now();
    let mut campos = Campos::new();
    campos.insert("status".into(), texto("scheduled"));
    campos.insert("lead".into(), texto(antecedencia.texto()));
    campos.insert("scheduledAt".into(), data(agora));
    campos.insert("releaseAt".into(), data(agora + antecedencia.duracao()));
    // O texto que sai no aviso, no e-mail e nas notas do updater fica congelado aqui.
    campos.insert("features".into(), lista_de_textos(&completa.features));
    campos.insert("fixes".into(), lista_de_textos(&completa.fixes));
    campos.insert("rules".into(), lista_de_textos(&completa.rules));
    campos.insert("scripts".into(), lista_de_textos(&completa.scripts));
    campos.insert("auto".into(), gravar_auto(&[]));
    campos.insert("autoUpdate".into(), booleano(opcoes.auto_update));
    campos.insert("notesOnly".into(), booleano(completa.installer.is_none()));
    let mascara =
        ["status", "lead", "scheduledAt", "releaseAt", "features", "fixes", "rules", "scripts", "auto", "autoUpdate", "notesOnly"];
    fs.gravar(&format!("releases/{versao}"), campos, &mascara).await?;
    obter(fs, versao).await
}

/// `1.2.3` → `1.2.4`.
fn proximo_patch((maior, menor, patch): (u32, u32, u32)) -> String {
    format!("{maior}.{menor}.{}", patch + 1)
}

/// Rascunho aberto (o de maior versão). Sem nenhum, cria o da próxima versão de patch depois da
/// maior já usada, para as alterações do Lab terem onde entrar.
pub async fn rascunho_aberto(fs: &Firestore) -> Result<Versao, AppError> {
    let versoes = listar(fs).await?;
    if let Some(rascunho) = versoes.iter().filter(|v| v.status == "draft").max_by_key(|v| ler_versao(&v.version)) {
        return Ok(rascunho.clone());
    }
    let base = versoes.iter().filter_map(|v| ler_versao(&v.version)).max().unwrap_or((0, 1, 0));
    let versao = proximo_patch(base);
    let mut campos = Campos::new();
    campos.insert("version".into(), texto(&versao));
    campos.insert("title".into(), texto(&format!("Versão {versao}")));
    campos.insert("status".into(), texto("draft"));
    campos.insert("updatedAt".into(), data(Utc::now()));
    fs.gravar(&format!("releases/{versao}"), campos, &["version", "title", "status", "updatedAt"]).await?;
    obter(fs, &versao).await
}

fn validar_mudanca(mudanca: &Mudanca) -> Result<(), AppError> {
    let chave_ok = !mudanca.key.is_empty()
        && mudanca.key.len() <= 120
        && mudanca.key.chars().all(|c| c.is_ascii_alphanumeric() || "-_:.@".contains(c));
    if !chave_ok {
        return Err(AppError::bad_request("chave da mudança inválida"));
    }
    if !SECOES.contains(&mudanca.section.as_str()) {
        return Err(AppError::bad_request("seção inválida (features, fixes, rules ou scripts)"));
    }
    if let Some(texto) = &mudanca.text {
        if texto.trim().is_empty() || texto.chars().count() > ITEM_MAXIMO {
            return Err(AppError::bad_request(format!("texto da mudança deve ter de 1 a {ITEM_MAXIMO} caracteres")));
        }
        if mudanca.section == "scripts" {
            let valido = texto.split_once('@').is_some_and(|(jogo, v)| id_valido(jogo) && ler_versao(v).is_some());
            if !valido {
                return Err(AppError::bad_request("script inválido (use jogo@1.2.3)"));
            }
        }
    }
    Ok(())
}

/// Registra no rascunho aberto o item de uma alteração do Lab (substitui o da mesma chave).
pub async fn registrar_mudanca(fs: &Firestore, mudanca: Mudanca) -> Result<Versao, AppError> {
    validar_mudanca(&mudanca)?;
    let rascunho = rascunho_aberto(fs).await?;
    let mut itens: BTreeMap<String, ItemAuto> = rascunho.auto.into_iter().map(|i| (i.key.clone(), i)).collect();
    match mudanca.text {
        Some(texto) => {
            itens.insert(mudanca.key.clone(), ItemAuto { key: mudanca.key, section: mudanca.section, text: texto.trim().to_string() });
        }
        None => {
            itens.remove(&mudanca.key);
        }
    }
    gravar_itens_auto(fs, &rascunho.version, &itens.into_values().collect::<Vec<_>>()).await?;
    obter(fs, &rascunho.version).await
}

/// Retira um item automático do rascunho (o admin não quer aquela linha na versão).
pub async fn remover_auto(fs: &Firestore, versao: &str, chave: &str) -> Result<Versao, AppError> {
    let atual = obter(fs, versao).await?;
    if atual.status != "draft" {
        return Err(AppError::conflict("só rascunhos podem ser editados"));
    }
    let itens: Vec<ItemAuto> = atual.auto.into_iter().filter(|i| i.key != chave).collect();
    gravar_itens_auto(fs, versao, &itens).await?;
    obter(fs, versao).await
}

async fn gravar_itens_auto(fs: &Firestore, versao: &str, itens: &[ItemAuto]) -> Result<(), AppError> {
    let mut campos = Campos::new();
    campos.insert("auto".into(), gravar_auto(itens));
    campos.insert("updatedAt".into(), data(Utc::now()));
    fs.gravar(&format!("releases/{versao}"), campos, &["auto", "updatedAt"]).await
}

/// Onde os instaladores ficam publicados (mesmo bucket e CloudFront do site).
const PREFIXO_DOWNLOADS: &str = "https://www.shmupx.com/downloads/";

/// Arquivo publicado em `www.shmupx.com/downloads/`, com nome simples (sem subpastas nem `?`).
fn url_de_download_valida(url: &str) -> bool {
    let nome = url.strip_prefix(PREFIXO_DOWNLOADS).unwrap_or_default();
    !nome.is_empty() && nome.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

/// Liga o instalador já publicado ao rascunho.
pub async fn definir_instalador(fs: &Firestore, versao: &str, instalador: Instalador) -> Result<Versao, AppError> {
    let atual = obter(fs, versao).await?;
    if atual.status != "draft" {
        return Err(AppError::conflict("só rascunhos podem receber instalador"));
    }
    if !url_de_download_valida(&instalador.url) {
        return Err(AppError::bad_request(format!("o instalador deve estar em {PREFIXO_DOWNLOADS}")));
    }
    let assinatura = instalador.signature.trim();
    if assinatura.is_empty() || assinatura.len() > 4096 {
        return Err(AppError::bad_request("assinatura do instalador inválida"));
    }
    let mut interno = Campos::new();
    interno.insert("url".into(), texto(&instalador.url));
    interno.insert("signature".into(), texto(assinatura));
    if let Some(tamanho) = instalador.size {
        interno.insert("size".into(), inteiro(tamanho));
    }
    let mut campos = Campos::new();
    campos.insert("installer".into(), mapa(interno));
    campos.insert("updatedAt".into(), data(Utc::now()));
    fs.gravar(&format!("releases/{versao}"), campos, &["installer", "updatedAt"]).await?;
    obter(fs, versao).await
}

/// Liga o ShmupX Portable já publicado ao rascunho (enviado pelo Lab junto com o instalador).
pub async fn definir_portatil(fs: &Firestore, versao: &str, portatil: Portatil) -> Result<Versao, AppError> {
    let atual = obter(fs, versao).await?;
    if atual.status != "draft" {
        return Err(AppError::conflict("só rascunhos podem receber a versão portátil"));
    }
    if !url_de_download_valida(&portatil.url) {
        return Err(AppError::bad_request(format!("a versão portátil deve estar em {PREFIXO_DOWNLOADS}")));
    }
    let sha256 = portatil.sha256.trim().to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::bad_request("SHA-256 da versão portátil inválido"));
    }
    let mut interno = Campos::new();
    interno.insert("url".into(), texto(&portatil.url));
    interno.insert("sha256".into(), texto(&sha256));
    if let Some(tamanho) = portatil.size {
        interno.insert("size".into(), inteiro(tamanho));
    }
    let mut campos = Campos::new();
    campos.insert("portable".into(), mapa(interno));
    campos.insert("updatedAt".into(), data(Utc::now()));
    fs.gravar(&format!("releases/{versao}"), campos, &["portable", "updatedAt"]).await?;
    obter(fs, versao).await
}

/// Resposta do updater do Tauri (formato de servidor dinâmico).
#[derive(Debug, Serialize, PartialEq)]
pub struct Atualizacao {
    pub version: String,
    pub notes: String,
    pub pub_date: DateTime<Utc>,
    pub url: String,
    pub signature: String,
}

/// Sistema e formato do Client que pede a atualização (`{{target}}` e `{{bundle_type}}` do updater).
#[derive(Debug, Clone, PartialEq)]
pub enum Plataforma {
    Windows,
    /// Formato do pacote instalado: `appimage` ou `deb`.
    Linux(String),
}

impl Plataforma {
    /// `None` para sistemas e formatos sem pacote publicado (ex.: macOS, rpm).
    pub fn de(alvo: &str, formato: Option<&str>) -> Option<Self> {
        match (alvo, formato) {
            ("windows", _) => Some(Plataforma::Windows),
            ("linux", Some(f @ ("appimage" | "deb"))) => Some(Plataforma::Linux(f.to_string())),
            _ => None,
        }
    }

    fn instalador<'a>(&self, versao: &'a Versao) -> Option<&'a Instalador> {
        match self {
            Plataforma::Windows => versao.installer.as_ref(),
            Plataforma::Linux(formato) => versao.linux_installers.get(formato),
        }
    }
}

/// Versão que o Client na versão `atual` deve baixar sozinho: a mais nova liberada, com
/// atualização automática e pacote para a plataforma dele, maior que a dele. `None` = nada a fazer
/// (HTTP 204).
pub fn atualizacao(versoes: &[Versao], atual: &str, plataforma: &Plataforma) -> Option<Atualizacao> {
    let atual = ler_versao(atual).unwrap_or((0, 0, 0));
    let (_, versao, instalador) = versoes
        .iter()
        .filter(|v| v.status == "released" && v.auto_update)
        .filter_map(|v| Some((ler_versao(&v.version)?, v, plataforma.instalador(v)?)))
        .filter(|(n, _, _)| *n > atual)
        .max_by_key(|(n, _, _)| *n)?;
    let mut notas = vec![versao.title.clone()];
    for (rotulo, itens) in [("Novidades", &versao.features), ("Correções", &versao.fixes), ("Regras", &versao.rules)] {
        if !itens.is_empty() {
            notas.push(format!("{rotulo}:\n{}", itens.iter().map(|i| format!("• {i}")).collect::<Vec<_>>().join("\n")));
        }
    }
    Some(Atualizacao {
        version: versao.version.clone(),
        notes: notas.join("\n\n"),
        pub_date: versao.release_at.unwrap_or_else(Utc::now),
        url: instalador.url.clone(),
        signature: instalador.signature.clone(),
    })
}

/// Instalador para baixar no site.
#[derive(Debug, Serialize, PartialEq)]
pub struct Download {
    pub version: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    /// Só na versão portátil: o Client portátil confere o arquivo antes de abrir.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// O que a página de Download do site oferece: por sistema, o instalador da versão mais nova já
/// liberada que tem pacote para ele. Muda sozinho quando um agendamento do Lab é liberado.
#[derive(Debug, Serialize, PartialEq)]
pub struct Downloads {
    pub windows: Option<Download>,
    pub appimage: Option<Download>,
    pub deb: Option<Download>,
    /// ShmupX Portable (Windows): a versão mais nova liberada que tem o `.exe` portátil.
    pub portable: Option<Download>,
}

pub fn downloads(versoes: &[Versao]) -> Downloads {
    let mais_nova = |plataforma: Plataforma| {
        versoes
            .iter()
            .filter(|v| v.status == "released")
            .filter_map(|v| Some((ler_versao(&v.version)?, v, plataforma.instalador(v)?)))
            .max_by_key(|(n, _, _)| *n)
            .map(|(_, v, i)| Download { version: v.version.clone(), url: i.url.clone(), size: i.size, sha256: None })
    };
    let portable = versoes
        .iter()
        .filter(|v| v.status == "released")
        .filter_map(|v| Some((ler_versao(&v.version)?, v, v.portable.as_ref()?)))
        .max_by_key(|(n, _, _)| *n)
        .map(|(_, v, p)| Download { version: v.version.clone(), url: p.url.clone(), size: p.size, sha256: Some(p.sha256.clone()) });
    Downloads {
        windows: mais_nova(Plataforma::Windows),
        appimage: mais_nova(Plataforma::Linux("appimage".into())),
        deb: mais_nova(Plataforma::Linux("deb".into())),
        portable,
    }
}

/// Cancela um agendamento que ainda não foi liberado.
pub async fn cancelar(fs: &Firestore, versao: &str) -> Result<Versao, AppError> {
    let atual = obter(fs, versao).await?;
    if atual.status != "scheduled" {
        return Err(AppError::conflict("só versões agendadas (ainda não liberadas) podem ser canceladas"));
    }
    let mut campos = Campos::new();
    campos.insert("status".into(), texto("cancelled"));
    campos.insert("updatedAt".into(), data(Utc::now()));
    fs.gravar(&format!("releases/{versao}"), campos, &["status", "updatedAt"]).await?;
    obter(fs, versao).await
}

/// Grava o resultado do envio de e-mails na versão (para o Lab mostrar).
pub async fn registrar_emails(fs: &Firestore, versao: &str, status: &str, enviados: i64) -> Result<(), AppError> {
    let mut campos = Campos::new();
    campos.insert("emailStatus".into(), texto(status));
    campos.insert("emailsSent".into(), inteiro(enviados));
    fs.gravar(&format!("releases/{versao}"), campos, &["emailStatus", "emailsSent"]).await
}

/// Situação do Client que chama: a versão mais nova liberada e se ele precisa atualizar antes de
/// abrir uma run (existe versão obrigatória liberada maior que a dele).
#[derive(Debug, Serialize, PartialEq)]
pub struct SituacaoClient {
    pub latest: Option<String>,
    pub update_available: bool,
    pub update_required: bool,
    /// Próxima versão agendada (aviso antecipado), se houver.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_release_at: Option<DateTime<Utc>>,
}

pub fn situacao(versoes: &[Versao], atual: Option<&str>) -> SituacaoClient {
    // Client sem versão informada (anterior aos avisos) conta como 0.0.0.
    let atual = atual.and_then(ler_versao).unwrap_or((0, 0, 0));
    // Versões só de notas (sem instalador) não pedem atualização do Client.
    let liberadas: Vec<&Versao> = versoes.iter().filter(|v| v.status == "released" && !v.notes_only).collect();
    let latest = liberadas.iter().filter_map(|v| ler_versao(&v.version).map(|n| (n, *v))).max_by_key(|(n, _)| *n);
    let update_required = liberadas.iter().any(|v| v.mandatory && ler_versao(&v.version).is_some_and(|n| n > atual));
    let proxima = versoes
        .iter()
        .filter(|v| v.status == "scheduled")
        .filter_map(|v| ler_versao(&v.version).map(|n| (n, v)))
        .min_by_key(|(n, _)| *n);
    SituacaoClient {
        update_available: latest.is_some_and(|(n, _)| n > atual),
        latest: latest.map(|(_, v)| v.version.clone()),
        update_required,
        next: proxima.map(|(_, v)| v.version.clone()),
        next_release_at: proxima.and_then(|(_, v)| v.release_at),
    }
}

/// Recusa a run se o Client está abaixo de uma versão obrigatória já liberada.
pub async fn exigir_versao_em_dia(fs: &Firestore, atual: Option<&str>) -> Result<(), AppError> {
    let situacao = situacao(&listar(fs).await?, atual);
    if situacao.update_required {
        return Err(AppError::conflict(format!(
            "atualize o ShmupX Client para a versão {} antes de jogar",
            situacao.latest.unwrap_or_default()
        )));
    }
    Ok(())
}

pub fn antecedencia_de(valor: &str) -> Option<Antecedencia> {
    Antecedencia::de_texto(valor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrada() -> VersaoEntrada {
        VersaoEntrada { title: " Aero Fighters 2 ".into(), features: vec!["Sala do Aero Fighters 2".into()], ..Default::default() }
    }

    fn versao(v: &str, status: &str, mandatory: bool) -> Versao {
        Versao { version: v.into(), status: status.into(), mandatory, ..Default::default() }
    }

    #[test]
    fn entrada_valida_e_normalizada() {
        let ok = validar("0.2.0", VersaoEntrada { fixes: vec!["  ".into(), " Gravação ".into()], ..entrada() }).unwrap();
        assert_eq!(ok.title, "Aero Fighters 2");
        assert_eq!(ok.fixes, vec!["Gravação"]);
    }

    #[test]
    fn entradas_invalidas_sao_recusadas() {
        assert!(validar("0.2", entrada()).is_err());
        assert!(validar("../0.2.0", entrada()).is_err());
        assert!(validar("0.2.0", VersaoEntrada { title: " ".into(), ..entrada() }).is_err());
        // Rascunho vazio é aceito (os itens automáticos chegam depois); a exigência vale ao agendar.
        assert!(validar("0.2.0", VersaoEntrada { features: vec![], ..entrada() }).is_ok(), "rascunho vazio");
        assert!(validar("0.2.0", VersaoEntrada { scripts: vec!["aero-fighters-2".into()], ..entrada() }).is_err());
        assert!(validar("0.2.0", VersaoEntrada { scripts: vec!["../x@0.1.0".into()], ..entrada() }).is_err());
        assert!(validar("0.2.0", VersaoEntrada { features: vec!["x".repeat(301)], ..entrada() }).is_err());
        assert!(validar("0.2.0", VersaoEntrada { scripts: vec!["aero-fighters-2@0.2.0".into()], ..entrada() }).is_ok());
    }

    #[test]
    fn corpo_com_campo_desconhecido_e_recusado() {
        assert!(serde_json::from_str::<VersaoEntrada>(r#"{"title":"x","status":"released"}"#).is_err());
    }

    #[test]
    fn agendada_vira_liberada_na_hora_marcada() {
        let agora = Utc::now();
        assert_eq!(status_efetivo("scheduled", Some(agora + Duration::hours(1)), agora), "scheduled");
        assert_eq!(status_efetivo("scheduled", Some(agora), agora), "released");
        assert_eq!(status_efetivo("cancelled", Some(agora - Duration::hours(1)), agora), "cancelled");
        assert_eq!(status_efetivo("draft", None, agora), "draft");
    }

    #[test]
    fn antecedencias_aceitas() {
        assert_eq!(antecedencia_de("urgent"), Some(Antecedencia::Urgente));
        assert_eq!(antecedencia_de("48h").map(|a| a.duracao()), Some(Duration::hours(48)));
        assert_eq!(antecedencia_de("1h"), None);
    }

    fn instalador() -> Instalador {
        Instalador { url: "https://www.shmupx.com/downloads/ShmupX-Setup-0.2.0.exe".into(), signature: "sig".into(), size: None }
    }

    #[test]
    fn itens_automaticos_entram_sem_repetir_o_manual() {
        let rascunho = Versao {
            features: vec!["Nova sala: Earthion".into()],
            auto: vec![
                ItemAuto { key: "game:earthion:sala".into(), section: "features".into(), text: "Nova sala: Earthion".into() },
                ItemAuto { key: "game:earthion:duracao".into(), section: "rules".into(), text: "Earthion: 30 min".into() },
                ItemAuto { key: "script:earthion".into(), section: "scripts".into(), text: "earthion@1.1.0".into() },
            ],
            ..Default::default()
        };
        let completa = juntar_auto(rascunho);
        assert_eq!(completa.features, vec!["Nova sala: Earthion"], "texto igual não duplica");
        assert_eq!(completa.rules, vec!["Earthion: 30 min"]);
        assert_eq!(completa.scripts, vec!["earthion@1.1.0"]);
        assert!(completa.auto.is_empty());
    }

    #[test]
    fn mudancas_invalidas_sao_recusadas() {
        let ok = Mudanca { key: "game:earthion:sala".into(), section: "features".into(), text: Some("Nova sala".into()) };
        assert!(validar_mudanca(&ok).is_ok());
        assert!(validar_mudanca(&Mudanca { key: "a/b".into(), ..ok.clone() }).is_err(), "chave com barra");
        assert!(validar_mudanca(&Mudanca { section: "status".into(), ..ok.clone() }).is_err());
        assert!(validar_mudanca(&Mudanca { text: Some(" ".into()), ..ok.clone() }).is_err());
        assert!(validar_mudanca(&Mudanca { section: "scripts".into(), text: Some("earthion".into()), ..ok.clone() }).is_err());
        assert!(validar_mudanca(&Mudanca { section: "scripts".into(), text: Some("earthion@1.1.0".into()), ..ok.clone() }).is_ok());
        assert!(validar_mudanca(&Mudanca { text: None, ..ok }).is_ok(), "retirar o item");
    }

    #[test]
    fn proxima_versao_de_patch() {
        assert_eq!(proximo_patch((0, 1, 0)), "0.1.1");
        assert_eq!(proximo_patch((1, 9, 9)), "1.9.10");
    }

    #[test]
    fn atualizacao_automatica_so_de_versao_liberada_com_instalador() {
        let liberada = Versao {
            title: "Aero Fighters 2".into(),
            features: vec!["Sala do Aero Fighters 2".into()],
            installer: Some(instalador()),
            auto_update: true,
            ..versao("0.2.0", "released", false)
        };
        let win = Plataforma::Windows;
        let a = atualizacao(std::slice::from_ref(&liberada), "0.1.0", &win).unwrap();
        assert_eq!((a.version.as_str(), a.signature.as_str()), ("0.2.0", "sig"));
        assert!(a.notes.contains("• Sala do Aero Fighters 2"));

        assert!(atualizacao(std::slice::from_ref(&liberada), "0.2.0", &win).is_none(), "já em dia");
        let agendada = Versao { status: "scheduled".into(), ..liberada };
        assert!(atualizacao(std::slice::from_ref(&agendada), "0.1.0", &win).is_none(), "antes de releaseAt");
        let manual = Versao { status: "released".into(), auto_update: false, ..agendada };
        assert!(atualizacao(&[manual], "0.1.0", &win).is_none(), "admin não liberou o automático");
    }

    #[test]
    fn cada_plataforma_recebe_o_proprio_pacote() {
        let linux = |url: &str| Instalador { url: url.into(), signature: format!("sig-{url}"), size: None };
        let so_windows = Versao { installer: Some(instalador()), auto_update: true, ..versao("0.2.0", "released", false) };
        let com_linux = Versao {
            installer: Some(instalador()),
            linux_installers: [("appimage".to_string(), linux("a.AppImage")), ("deb".to_string(), linux("a.deb"))].into(),
            auto_update: true,
            ..versao("0.1.5", "released", false)
        };
        let versoes = [so_windows, com_linux];
        let appimage = Plataforma::de("linux", Some("appimage")).unwrap();
        let deb = Plataforma::de("linux", Some("deb")).unwrap();

        assert_eq!(atualizacao(&versoes, "0.1.4", &Plataforma::Windows).unwrap().version, "0.2.0");
        // A 0.2.0 não tem pacote Linux: o Linux fica na mais nova que tem.
        let a = atualizacao(&versoes, "0.1.4", &appimage).unwrap();
        assert_eq!((a.version.as_str(), a.url.as_str()), ("0.1.5", "a.AppImage"));
        assert_eq!(atualizacao(&versoes, "0.1.4", &deb).unwrap().url, "a.deb");
        assert!(atualizacao(&versoes, "0.1.5", &deb).is_none());

        assert_eq!(Plataforma::de("linux", None), None, "Linux sem formato (Client antigo)");
        assert_eq!(Plataforma::de("linux", Some("rpm")), None);
        assert_eq!(Plataforma::de("darwin", Some("app")), None);
    }

    #[test]
    fn download_do_site_e_a_mais_nova_liberada_com_pacote_por_sistema() {
        let linux = |url: &str| Instalador { url: url.into(), signature: "s".into(), size: Some(10) };
        let agendada = Versao { installer: Some(instalador()), ..versao("0.3.0", "scheduled", false) };
        let so_windows = Versao { installer: Some(instalador()), ..versao("0.2.0", "released", false) };
        let com_linux = Versao {
            installer: Some(instalador()),
            linux_installers: [("appimage".to_string(), linux("a.AppImage")), ("deb".to_string(), linux("a.deb"))].into(),
            ..versao("0.1.5", "released", false)
        };
        let d = downloads(&[agendada, so_windows, com_linux]);
        assert_eq!(d.windows.unwrap().version, "0.2.0", "a agendada ainda não aparece");
        assert_eq!(d.appimage.unwrap().url, "a.AppImage");
        assert_eq!(d.deb.unwrap().version, "0.1.5");
        assert_eq!(downloads(&[]), Downloads { windows: None, appimage: None, deb: None, portable: None });
    }

    #[test]
    fn portatil_do_site_e_o_da_mais_nova_liberada_que_tem_portatil() {
        let portatil = |url: &str| Portatil { url: url.into(), sha256: "ab".repeat(32), size: Some(20) };
        let agendada = Versao { portable: Some(portatil("p3.exe")), ..versao("0.3.0", "scheduled", false) };
        let sem_portatil = Versao { installer: Some(instalador()), ..versao("0.2.0", "released", false) };
        let com_portatil = Versao { installer: Some(instalador()), portable: Some(portatil("p1.exe")), ..versao("0.1.5", "released", false) };
        let d = downloads(&[agendada, sem_portatil, com_portatil]);
        assert_eq!(d.windows.unwrap().version, "0.2.0");
        let p = d.portable.unwrap();
        assert_eq!((p.version.as_str(), p.url.as_str(), p.size), ("0.1.5", "p1.exe", Some(20)));
        assert_eq!(p.sha256, Some("ab".repeat(32)));
    }

    #[test]
    fn so_downloads_do_site_sao_aceitos() {
        assert!(url_de_download_valida("https://www.shmupx.com/downloads/ShmupX-Portable-0.1.18.exe"));
        assert!(!url_de_download_valida("https://www.shmupx.com/downloads/"));
        assert!(!url_de_download_valida("https://www.shmupx.com/downloads/a/b.exe"));
        assert!(!url_de_download_valida("https://evil.example/downloads/x.exe"));
    }

    #[test]
    fn versao_so_de_notas_nao_pede_atualizacao() {
        let notas = Versao { notes_only: true, ..versao("0.2.0", "released", true) };
        let s = situacao(&[notas], Some("0.1.0"));
        assert!(!s.update_available && !s.update_required);
    }

    #[test]
    fn situacao_do_client() {
        let versoes = vec![
            versao("0.2.0", "released", false),
            versao("0.3.0", "released", true),
            versao("0.4.0", "scheduled", true),
            versao("0.5.0", "draft", true),
            versao("0.9.0", "cancelled", true),
        ];
        let antigo = situacao(&versoes, Some("0.1.0"));
        assert_eq!(antigo.latest.as_deref(), Some("0.3.0"));
        assert!(antigo.update_available && antigo.update_required);
        assert_eq!(antigo.next.as_deref(), Some("0.4.0"));

        let em_dia = situacao(&versoes, Some("0.3.0"));
        assert!(!em_dia.update_available && !em_dia.update_required, "agendada ainda não obriga");

        assert!(situacao(&versoes, None).update_required, "Client sem versão conta como 0.0.0");
        assert!(!situacao(&[versao("0.2.0", "released", false)], Some("0.1.0")).update_required);
    }
}
