//! Jogo aberto na conta (`remoteLaunch/{uid}.playing`) avisado pelo agente do MiSTer: ao abrir um
//! jogo pelo menu ShmupX (por qualquer caminho: Client, Lite ou o próprio controle do MiSTer), o
//! agente avisa `aberto` e renova a cada minuto; ao voltar ao menu, avisa `fechado` e a conta fica
//! livre. Funciona com o Client fechado. O Client e o Lite leem o mesmo campo (um jogo Ranked por
//! vez na conta; Training não trava).
//!
//! Ranked ou Training é decisão do servidor: Ranked só nos jogos com `rankedMister` no catálogo
//! (leitura de placar pela memória no MiSTer).

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::firestore::valor::{inteiro, ler_inteiro, ler_mapa, ler_texto, mapa, texto};
use crate::firestore::{Campos, Firestore};
use crate::services::catalogo::{self, Jogo};
use crate::services::gameplays::id_valido;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvisoMister {
    /// `aberto` (também serve de renovação) ou `fechado`.
    pub estado: String,
    /// Nome do jogo no menu do MiSTer (o mesmo título do catálogo, na maioria).
    #[serde(default)]
    pub titulo: Option<String>,
    /// Id do catálogo, quando o agente sabe (perfil do jogo).
    #[serde(default)]
    pub jogo_id: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct JogoAberto {
    pub game_id: String,
    pub title: String,
    /// `ranked` ou `training`.
    pub kind: String,
}

/// Título comparável: minúsculas, só letras e dígitos (o menu do MiSTer tira pontuação do nome).
fn comparavel(titulo: &str) -> String {
    titulo.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Jogo do catálogo pelo id ou pelo título do menu.
fn achar<'a>(jogos: &'a [Jogo], jogo_id: Option<&str>, titulo: Option<&str>) -> Option<&'a Jogo> {
    jogo_id
        .and_then(|id| jogos.iter().find(|j| j.id == id))
        .or_else(|| titulo.map(comparavel).filter(|t| !t.is_empty()).and_then(|t| jogos.iter().find(|j| comparavel(&j.title) == t)))
}

/// Id seguro para um jogo fora do catálogo (aparece só como "jogo aberto").
fn id_de(titulo: &str) -> String {
    let id: String = titulo.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let id = id.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if id.is_empty() { "mister".into() } else { id.chars().take(64).collect() }
}

/// Aplica o aviso do agente. `aberto` devolve o jogo marcado na conta; `fechado` devolve `None`.
pub async fn avisar(fs: &Firestore, uid: &str, aviso: AvisoMister) -> Result<Option<JogoAberto>, AppError> {
    let caminho = format!("remoteLaunch/{uid}");
    let atual = fs.ler_documento(&caminho).await?.and_then(|c| ler_mapa(&c, "playing"));
    match aviso.estado.as_str() {
        "fechado" => {
            // Só libera o que é do MiSTer: um jogo aberto depois no PC fica como está.
            if atual.as_ref().is_some_and(|p| ler_texto(p, "platform").as_deref() == Some("mister")) {
                fs.gravar(&caminho, Campos::new(), &["playing"]).await?;
            }
            Ok(None)
        }
        "aberto" => {
            let titulo = aviso.titulo.as_deref().map(str::trim).filter(|t| !t.is_empty() && t.chars().count() <= 160);
            let jogo_id = aviso.jogo_id.as_deref().filter(|id| id_valido(id));
            if titulo.is_none() && jogo_id.is_none() {
                return Err(AppError::bad_request("informe o jogo (titulo ou jogo_id)"));
            }
            let jogos = catalogo::listar(fs).await?;
            let jogo = achar(&jogos, jogo_id, titulo);
            let aberto = match jogo {
                Some(j) => JogoAberto { game_id: j.id.clone(), title: j.title.clone(), kind: if j.ranked_mister { "ranked" } else { "training" }.into() },
                None => {
                    let titulo = titulo.or(jogo_id).unwrap_or("MiSTer").to_string();
                    JogoAberto { game_id: id_de(&titulo), title: titulo, kind: "training".into() }
                }
            };
            let agora = Utc::now().timestamp_millis();
            // Renovação do mesmo jogo pelo MiSTer: mantém a hora em que começou.
            let comecou = atual
                .as_ref()
                .filter(|p| ler_texto(p, "gameId").as_deref() == Some(aberto.game_id.as_str()) && ler_texto(p, "device").as_deref() == Some("mister"))
                .and_then(|p| ler_inteiro(p, "startedAt"))
                .unwrap_or(agora);
            let mut jogando = Campos::new();
            jogando.insert("gameId".into(), texto(&aberto.game_id));
            jogando.insert("title".into(), texto(&aberto.title));
            jogando.insert("platform".into(), texto("mister"));
            jogando.insert("kind".into(), texto(&aberto.kind));
            jogando.insert("device".into(), texto("mister"));
            jogando.insert("startedAt".into(), inteiro(comecou));
            jogando.insert("beatAt".into(), inteiro(agora));
            let mut campos = Campos::new();
            campos.insert("playing".into(), mapa(jogando));
            fs.gravar(&caminho, campos, &["playing"]).await?;
            Ok(Some(aberto))
        }
        _ => Err(AppError::bad_request("estado inválido: use aberto ou fechado")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jogo(id: &str, titulo: &str, ranked: bool) -> Jogo {
        Jogo { id: id.into(), title: titulo.into(), ranked_mister: ranked, ..Default::default() }
    }

    #[test]
    fn acha_pelo_id_ou_pelo_titulo_do_menu() {
        let jogos = [jogo("earthion", "Earthion", true), jogo("aero-fighters-2", "Aero Fighters 2", false), jogo("darius-gaiden", "Darius Gaiden", false)];
        assert_eq!(achar(&jogos, Some("earthion"), None).unwrap().id, "earthion");
        assert_eq!(achar(&jogos, None, Some("Aero Fighters 2")).unwrap().id, "aero-fighters-2");
        assert_eq!(achar(&jogos, None, Some("darius gaiden")).unwrap().id, "darius-gaiden");
        assert!(achar(&jogos, Some("nao-existe"), Some("Ketsui")).is_none());
    }

    #[test]
    fn id_seguro_para_jogo_fora_do_catalogo() {
        assert_eq!(id_de("Ketsui: Kizuna Jigoku Tachi"), "ketsui-kizuna-jigoku-tachi");
        assert_eq!(id_de("???"), "mister");
        assert!(id_valido(&id_de("Mushihime-sama Futari 1.5")));
    }

    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn marca_renova_e_libera_no_emulador() {
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let mut jogo = Campos::new();
        jogo.insert("title".into(), texto("Earthion Teste"));
        jogo.insert("rankedMister".into(), crate::firestore::valor::booleano(true));
        fs.gravar("games/earthion-teste", jogo, &["title", "rankedMister"]).await.unwrap();
        let uid = format!("u-{}", uuid::Uuid::new_v4());
        let aviso = |estado: &str, titulo: &str| AvisoMister { estado: estado.into(), titulo: Some(titulo.into()), jogo_id: None };

        let aberto = avisar(&fs, &uid, aviso("aberto", "Earthion Teste")).await.unwrap().unwrap();
        assert_eq!((aberto.game_id.as_str(), aberto.kind.as_str()), ("earthion-teste", "ranked"));
        let ler = || async { fs.ler_documento(&format!("remoteLaunch/{uid}")).await.unwrap().and_then(|c| ler_mapa(&c, "playing")) };
        let primeiro = ler().await.unwrap();
        avisar(&fs, &uid, aviso("aberto", "Earthion Teste")).await.unwrap();
        let renovado = ler().await.unwrap();
        assert_eq!(ler_inteiro(&primeiro, "startedAt"), ler_inteiro(&renovado, "startedAt"), "renovar mantém o começo");
        assert_eq!(ler_texto(&renovado, "device").as_deref(), Some("mister"));

        // Fora do catálogo: só Training.
        let outro = avisar(&fs, &uid, aviso("aberto", "Jogo Sem Cadastro")).await.unwrap().unwrap();
        assert_eq!(outro.kind, "training");

        avisar(&fs, &uid, aviso("fechado", "")).await.unwrap();
        assert!(ler().await.is_none(), "voltou ao menu: conta livre");
        assert!(avisar(&fs, &uid, aviso("pausado", "x")).await.is_err());
    }
}
