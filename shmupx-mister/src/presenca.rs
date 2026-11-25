//! Jogo aberto na conta do ShmupX (`POST /api/mister/playing`). O MiSTer grava o nome do core em
//! `/tmp/CORENAME`; os itens do menu ShmupX levam `<código do jogo>_<jogador>_<modo>` (o `menu.sh`
//! gera os códigos em `codigos.txt`). Ao abrir um jogo do menu, o agente avisa `aberto` com o token
//! do jogador e renova a cada minuto; ao voltar ao menu (ou trocar de core), avisa `fechado`.
//! Assim a conta fica livre mesmo com o Client do PC fechado, e qualquer caminho de abrir o jogo
//! (Client, Lite ou o controle no próprio MiSTer) marca o jogo na conta.
//!
//! Corre numa thread própria, separada da leitura do placar (que fica presa no jogo enquanto ele roda).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::config::{self, Usuario};
use crate::envio::Api;

const RENOVAR_A_CADA: Duration = Duration::from_secs(60);
const TENTAR_DE_NOVO_EM: Duration = Duration::from_secs(15);
const OLHAR_A_CADA: Duration = Duration::from_secs(5);
/// Um `fechado` que não saiu (sem internet) é tentado de novo por este tempo.
const DESISTIR_DO_FECHADO: Duration = Duration::from_secs(10 * 60);

/// Código do jogo -> nome no menu (`codigos.txt`: `codigo|nome`, gravado pelo `menu.sh`).
pub fn codigos(pasta: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(pasta.join("codigos.txt"))
        .unwrap_or_default()
        .lines()
        .filter_map(|linha| {
            let (codigo, nome) = linha.split_once('|')?;
            let (codigo, nome) = (codigo.trim().to_ascii_lowercase(), nome.trim().to_owned());
            (!codigo.is_empty() && !nome.is_empty()).then_some((codigo, nome))
        })
        .collect()
}

/// O que avisar ao backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aviso {
    Aberto { usuario: Usuario, titulo: String },
    Fechado { usuario: Usuario },
}

#[derive(Debug, Clone)]
struct Ativo {
    core: String,
    usuario: Usuario,
    titulo: String,
    ultimo_aviso: Option<Instant>,
}

/// Estado do vigia: o jogo do menu ShmupX que está aberto agora.
#[derive(Debug, Default)]
pub struct Vigia {
    ativo: Option<Ativo>,
    /// `fechado` que ainda não saiu, com a hora do primeiro pedido.
    fechar: Option<(Usuario, Instant)>,
}

impl Vigia {
    /// Olha o core atual e diz o que avisar agora. `identificar` traduz o core no jogador e no jogo.
    pub fn passo(&mut self, core: &str, agora: Instant, identificar: impl Fn(&str) -> Option<(Usuario, String)>) -> Vec<Aviso> {
        let mut avisos = Vec::new();
        if let Some((usuario, desde)) = &self.fechar {
            if agora.duration_since(*desde) > DESISTIR_DO_FECHADO {
                self.fechar = None;
            } else {
                avisos.push(Aviso::Fechado { usuario: usuario.clone() });
            }
        }
        let novo = if config::core_do_shmupx(core) { identificar(core) } else { None };
        match (self.ativo.as_mut(), novo) {
            (Some(ativo), Some(_)) if ativo.core == core => {
                if ativo.ultimo_aviso.is_none_or(|ultimo| agora.duration_since(ultimo) >= RENOVAR_A_CADA) {
                    avisos.push(Aviso::Aberto { usuario: ativo.usuario.clone(), titulo: ativo.titulo.clone() });
                }
            }
            (ativo, novo) => {
                if let Some(antigo) = ativo.cloned() {
                    // Outro jogador (ou voltou ao menu): libera a conta de quem estava jogando.
                    if novo.as_ref().is_none_or(|(u, _)| u.id != antigo.usuario.id) {
                        avisos.push(Aviso::Fechado { usuario: antigo.usuario.clone() });
                        self.fechar = Some((antigo.usuario, agora));
                    }
                }
                self.ativo = novo.map(|(usuario, titulo)| {
                    avisos.push(Aviso::Aberto { usuario: usuario.clone(), titulo: titulo.clone() });
                    Ativo { core: core.to_owned(), usuario, titulo, ultimo_aviso: None }
                });
            }
        }
        avisos
    }

    /// Resultado do envio: o aviso saiu (`true`) ou vai ser tentado de novo.
    pub fn enviado(&mut self, aviso: &Aviso, ok: bool, agora: Instant) {
        match aviso {
            Aviso::Aberto { usuario, .. } => {
                if let Some(ativo) = self.ativo.as_mut().filter(|a| a.usuario.id == usuario.id) {
                    // Falhou: tenta de novo em 15 s (e não espera o minuto inteiro).
                    ativo.ultimo_aviso = Some(if ok { agora } else { agora - RENOVAR_A_CADA + TENTAR_DE_NOVO_EM });
                }
            }
            Aviso::Fechado { usuario } => {
                if ok && self.fechar.as_ref().is_some_and(|(u, _)| u.id == usuario.id) {
                    self.fechar = None;
                }
            }
        }
    }
}

/// Manda o aviso com o token do jogador. Sem `api` ou sem token (convidado), não há conta a marcar.
fn enviar(pasta: &Path, aviso: &Aviso) -> Result<(), String> {
    let usuario = match aviso {
        Aviso::Aberto { usuario, .. } | Aviso::Fechado { usuario } => usuario,
    };
    let (Some(base), Some(token)) = (config::api(pasta), config::token(pasta, usuario)) else { return Ok(()) };
    let corpo = match aviso {
        Aviso::Aberto { titulo, .. } => json!({ "estado": "aberto", "titulo": titulo }),
        Aviso::Fechado { .. } => json!({ "estado": "fechado" }),
    };
    let resposta = (Api { base, token }).post("/api/mister/playing", &corpo)?;
    if resposta.ok() || resposta.status == 401 || resposta.status == 400 {
        // 401/400 não melhoram tentando de novo (token revogado, jogo inválido).
        Ok(())
    } else {
        Err(resposta.erro())
    }
}

/// Laço da thread: olha o core a cada 5 s e avisa o backend.
pub fn vigiar(pasta: PathBuf) {
    let mut vigia = Vigia::default();
    loop {
        let core = std::fs::read_to_string("/tmp/CORENAME").map(|n| n.trim().to_owned()).unwrap_or_default();
        let tabela = codigos(&pasta);
        let avisos = vigia.passo(&core, Instant::now(), |core| {
            let codigo = core.split('_').next()?.to_ascii_lowercase();
            let titulo = tabela.get(&codigo)?.clone();
            Some((config::identificar(&pasta, core).0, titulo))
        });
        for aviso in avisos {
            let resultado = enviar(&pasta, &aviso);
            if let Err(motivo) = &resultado {
                eprintln!("jogo aberto: aviso ao ShmupX falhou ({motivo}); tento de novo");
            }
            vigia.enviado(&aviso, resultado.is_ok(), Instant::now());
        }
        std::thread::sleep(OLHAR_A_CADA);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usuario(id: &str) -> Usuario {
        Usuario { id: id.into(), nome: id.to_ascii_uppercase() }
    }

    fn identificar(core: &str) -> Option<(Usuario, String)> {
        let mut partes = core.split('_');
        let jogo = match partes.next()? { "earthion" => "Earthion", "dariusg" => "Darius Gaiden", _ => return None };
        Some((usuario(partes.next()?), jogo.to_owned()))
    }

    #[test]
    fn abre_renova_e_fecha() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        assert!(v.passo("MENU", t0, identificar).is_empty());
        let avisos = v.passo("earthion_kwayd_hs", t0, identificar);
        assert_eq!(avisos, vec![Aviso::Aberto { usuario: usuario("kwayd"), titulo: "Earthion".into() }]);
        v.enviado(&avisos[0], true, t0);
        assert!(v.passo("earthion_kwayd_hs", t0 + Duration::from_secs(30), identificar).is_empty(), "renova só a cada minuto");
        assert_eq!(v.passo("earthion_kwayd_hs", t0 + Duration::from_secs(61), identificar).len(), 1);
        let fechar = v.passo("MENU", t0 + Duration::from_secs(70), identificar);
        assert_eq!(fechar, vec![Aviso::Fechado { usuario: usuario("kwayd") }]);
        v.enviado(&fechar[0], true, t0 + Duration::from_secs(70));
        assert!(v.passo("MENU", t0 + Duration::from_secs(75), identificar).is_empty());
    }

    #[test]
    fn falha_tenta_de_novo() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        let aberto = v.passo("dariusg_ana_spr", t0, identificar);
        v.enviado(&aberto[0], false, t0);
        assert!(v.passo("dariusg_ana_spr", t0 + Duration::from_secs(5), identificar).is_empty());
        assert_eq!(v.passo("dariusg_ana_spr", t0 + Duration::from_secs(16), identificar).len(), 1, "tenta de novo em 15 s");
        // Fechado sem internet: repete até sair.
        let fechar = v.passo("MENU", t0 + Duration::from_secs(20), identificar);
        v.enviado(&fechar[0], false, t0 + Duration::from_secs(20));
        assert_eq!(v.passo("MENU", t0 + Duration::from_secs(25), identificar), vec![Aviso::Fechado { usuario: usuario("ana") }]);
        assert!(v.passo("MENU", t0 + Duration::from_secs(20 * 60), identificar).is_empty(), "desiste depois de 10 min");
    }

    #[test]
    fn troca_de_jogo_e_de_jogador() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        v.passo("earthion_ana_hs", t0, identificar);
        // Mesmo jogador, outro jogo: só o aberto novo (o backend troca o jogo).
        assert_eq!(v.passo("dariusg_ana_hs", t0, identificar), vec![Aviso::Aberto { usuario: usuario("ana"), titulo: "Darius Gaiden".into() }]);
        // Outro jogador: libera a conta da Ana e marca a do Beto.
        let avisos = v.passo("dariusg_beto_hs", t0, identificar);
        assert_eq!(avisos, vec![Aviso::Fechado { usuario: usuario("ana") }, Aviso::Aberto { usuario: usuario("beto"), titulo: "Darius Gaiden".into() }]);
    }

    #[test]
    fn core_fora_do_menu_shmupx_nao_marca() {
        let mut v = Vigia::default();
        assert!(v.passo("MegaDrive", Instant::now(), identificar).is_empty());
        assert!(v.passo("SX_ana_hs", Instant::now(), identificar).is_empty(), "menu antigo sem código: sem aviso");
    }

    #[test]
    fn le_a_tabela_de_codigos() {
        let pasta = std::env::temp_dir().join(format!("shmupx-codigos-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("codigos.txt"), "earthion|Earthion\nDARIUSG|Darius Gaiden\nlixo\n|x\n").unwrap();
        let tabela = codigos(&pasta);
        assert_eq!(tabela.get("earthion").map(String::as_str), Some("Earthion"));
        assert_eq!(tabela.get("dariusg").map(String::as_str), Some("Darius Gaiden"));
        assert_eq!(tabela.len(), 2);
        std::fs::remove_dir_all(&pasta).unwrap();
    }
}
