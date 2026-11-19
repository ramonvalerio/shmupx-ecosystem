//! Envio das partidas ao backend do ShmupX (ADR 008), pelo mesmo caminho do Client de PC:
//! `POST /api/runs/start` (gasta 1 ficha) -> `POST /api/runs/end` -> `POST /api/runs`.
//!
//! O HTTPS sai pelo `curl` do próprio MiSTer, com o pacote de certificados do sistema
//! (`/etc/ssl/certs/cacert.pem`): sem ele o `curl` do MiSTer não verifica o servidor, e a
//! verificação nunca é desligada. Assim o binário não precisa de biblioteca de TLS.
//!
//! Partida que não pôde ser encerrada ou enviada (sem internet) vai para `pendentes/` e é
//! reenviada depois. O início não é refeito: sem o início no servidor, a partida fica local.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CERTIFICADOS: &str = "/etc/ssl/certs/cacert.pem";

/// Modo local (`partida.modo`) -> modo da API.
pub fn modo_da_api(modo: &str) -> Option<&'static str> {
    match modo {
        "hi-score" => Some("highscore"),
        "1cc" => Some("1cc"),
        "caravan-sprint" => Some("caravan-sprint"),
        "caravan-classic" => Some("caravan-classic"),
        _ => None,
    }
}

/// Credencial e destino de um jogador.
#[derive(Debug, Clone)]
pub struct Api {
    pub base: String,
    pub token: String,
}

/// Resposta do backend: código HTTP e JSON (ou o texto, se não for JSON).
#[derive(Debug, PartialEq)]
pub struct Resposta {
    pub status: u16,
    pub corpo: Value,
}

impl Resposta {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Mensagem de erro do backend (`{"error": "..."}`) ou o código.
    pub fn erro(&self) -> String {
        self.corpo
            .get("error")
            .or_else(|| self.corpo.get("message"))
            .and_then(Value::as_str)
            .map_or_else(|| format!("HTTP {}", self.status), str::to_owned)
    }
}

/// Saída do curl com `-w "\n%{http_code}"`: corpo, quebra de linha, código.
pub fn ler_saida(saida: &str) -> Option<Resposta> {
    let (corpo, codigo) = saida.trim_end().rsplit_once('\n').unwrap_or(("", saida.trim_end()));
    let status: u16 = codigo.trim().parse().ok()?;
    let corpo = serde_json::from_str(corpo).unwrap_or_else(|_| Value::String(corpo.to_owned()));
    Some(Resposta { status, corpo })
}

impl Api {
    /// `POST` com corpo JSON. `Err` = não houve resposta (sem rede, DNS, certificado).
    pub fn post(&self, caminho: &str, corpo: &Value) -> Result<Resposta, String> {
        let mut comando = Command::new("curl");
        comando.args(["-sS", "-m", "20", "-X", "POST", "-H", "Content-Type: application/json"]);
        if Path::new(CERTIFICADOS).is_file() {
            comando.args(["--cacert", CERTIFICADOS]);
        }
        // A entrada padrão leva o corpo; o token vai como argumento do curl, visível só para
        // quem já é root no próprio MiSTer (onde ele também está no shmupx.ini).
        comando
            .args(["-H", &format!("Authorization: Bearer {}", self.token)])
            .args(["--data-binary", "@-", "-w", "\n%{http_code}"])
            .arg(format!("{}{caminho}", self.base))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut processo = comando.spawn().map_err(|e| format!("não consegui rodar o curl: {e}"))?;
        if let Some(mut entrada) = processo.stdin.take() {
            entrada.write_all(corpo.to_string().as_bytes()).map_err(|e| format!("falha ao enviar ao curl: {e}"))?;
        }
        let saida = processo.wait_with_output().map_err(|e| format!("falha no curl: {e}"))?;
        if !saida.status.success() {
            return Err(String::from_utf8_lossy(&saida.stderr).trim().to_owned());
        }
        ler_saida(&String::from_utf8_lossy(&saida.stdout)).ok_or_else(|| "resposta do curl ilegível".to_owned())
    }

    /// Abre a partida no servidor (gasta 1 ficha). Devolve o `run_id`.
    pub fn iniciar(&self, jogo_id: &str, modo: Option<&str>, versao: Option<&str>) -> Result<String, String> {
        let resposta = self.post("/api/runs/start", &json!({ "jogo_id": jogo_id, "modo": modo, "versao": versao }))?;
        if !resposta.ok() {
            return Err(resposta.erro());
        }
        resposta.corpo.get("run_id").and_then(Value::as_str).map(str::to_owned).ok_or_else(|| "resposta sem run_id".to_owned())
    }
}

impl Api {
    /// Confere o token sem gastar ficha: encerrar uma partida que não existe responde 404 a
    /// quem está autenticado e 401 a quem não está.
    pub fn verificar(&self) -> Result<(), String> {
        let resposta = self.post("/api/runs/end", &json!({ "run_id": "verificacao-de-token" }))?;
        match resposta.status {
            404 => Ok(()),
            401 => Err("token inválido ou revogado".to_owned()),
            _ => Err(resposta.erro()),
        }
    }
}

/// O que falta mandar de uma partida aberta no servidor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pendente {
    pub usuario: String,
    pub run_id: String,
    pub jogo_id: String,
    pub placar: u64,
    /// O `/api/runs/end` já foi aceito.
    pub encerrada: bool,
}

/// Resultado de uma tentativa de concluir o envio.
#[derive(Debug, PartialEq, Eq)]
pub enum Conclusao {
    Enviada,
    /// Sem rede: tentar de novo depois.
    Tentar(String),
    /// O servidor recusou (expirou, token revogado...): não adianta repetir.
    Recusada(String),
}

/// Encerra (se preciso) e manda o placar.
pub fn concluir(api: &Api, pendente: &mut Pendente) -> Conclusao {
    if !pendente.encerrada {
        match api.post("/api/runs/end", &json!({ "run_id": pendente.run_id })) {
            Err(erro) => return Conclusao::Tentar(erro),
            Ok(r) if r.ok() => pendente.encerrada = true,
            Ok(r) if r.status >= 500 => return Conclusao::Tentar(r.erro()),
            Ok(r) => return Conclusao::Recusada(r.erro()),
        }
    }
    let corpo = json!({ "run_id": pendente.run_id, "jogo_id": pendente.jogo_id, "score_bruto": pendente.placar });
    match api.post("/api/runs", &corpo) {
        Err(erro) => Conclusao::Tentar(erro),
        Ok(r) if r.ok() => Conclusao::Enviada,
        Ok(r) if r.status >= 500 => Conclusao::Tentar(r.erro()),
        Ok(r) => Conclusao::Recusada(r.erro()),
    }
}

/// Tudo o que uma partida precisa para ir ao servidor.
#[derive(Debug, Clone)]
pub struct Envio {
    pub api: Api,
    pub usuario: String,
    pub jogo_id: String,
    pub versao: Option<String>,
    pub modo: Option<&'static str>,
}

/// Monta o envio da partida, ou diz por que ela fica só no ranking local.
pub fn preparar(
    pasta: &Path,
    usuario: &crate::config::Usuario,
    perfil: &crate::placar::Perfil,
    modo: &str,
) -> Result<Envio, String> {
    let base = crate::config::api(pasta).ok_or("sem api no shmupx.ini")?;
    let token = crate::config::token(pasta, usuario).ok_or_else(|| format!("sem token.{} no shmupx.ini", usuario.id))?;
    let jogo_id = perfil.jogo_id.clone().ok_or_else(|| format!("perfil {} sem jogo_id", perfil.jogo))?;
    Ok(Envio {
        api: Api { base, token },
        usuario: usuario.id.clone(),
        jogo_id,
        versao: perfil.versao.clone(),
        modo: modo_da_api(modo),
    })
}

/// Fim da partida aberta no servidor: encerra e manda o placar, ou guarda para depois.
/// Devolve a situação para gravar na partida.
pub fn finalizar(pasta: &Path, envio: &Envio, run_id: &str, placar: u64) -> String {
    let mut pendente = Pendente {
        usuario: envio.usuario.clone(),
        run_id: run_id.to_owned(),
        jogo_id: envio.jogo_id.clone(),
        placar,
        encerrada: false,
    };
    match concluir(&envio.api, &mut pendente) {
        Conclusao::Enviada => "enviada".to_owned(),
        Conclusao::Recusada(motivo) => format!("recusada: {motivo}"),
        Conclusao::Tentar(motivo) => match guardar_pendente(pasta, &pendente) {
            Ok(()) => format!("pendente: {motivo}"),
            Err(e) => format!("perdida: {motivo} ({e})"),
        },
    }
}

/// Tenta de novo as partidas guardadas. Devolve quantas foram enviadas.
pub fn reenviar_pendentes(pasta: &Path) -> usize {
    let Some(base) = crate::config::api(pasta) else {
        return 0;
    };
    let usuarios = crate::config::usuarios(pasta);
    let mut enviadas = 0;
    for (arquivo, mut pendente) in listar_pendentes(pasta) {
        let Some(usuario) = usuarios.iter().find(|u| u.id == pendente.usuario) else {
            continue;
        };
        let Some(token) = crate::config::token(pasta, usuario) else {
            continue;
        };
        let api = Api { base: base.clone(), token };
        match concluir(&api, &mut pendente) {
            Conclusao::Enviada => {
                enviadas += 1;
                let _ = std::fs::remove_file(&arquivo);
            }
            Conclusao::Recusada(motivo) => {
                eprintln!("partida {} recusada pelo servidor: {motivo}", pendente.run_id);
                let _ = std::fs::rename(&arquivo, arquivo.with_extension("recusada"));
            }
            // Guarda o progresso (encerrada) para a próxima tentativa.
            Conclusao::Tentar(_) => {
                let _ = guardar_pendente(pasta, &pendente);
            }
        }
    }
    enviadas
}

fn pasta_pendentes(pasta: &Path) -> PathBuf {
    pasta.join("pendentes")
}

pub fn guardar_pendente(pasta: &Path, pendente: &Pendente) -> std::io::Result<()> {
    let destino = pasta_pendentes(pasta);
    std::fs::create_dir_all(&destino)?;
    let json = serde_json::to_string_pretty(pendente).expect("Pendente sempre serializa");
    std::fs::write(destino.join(format!("{}.json", pendente.run_id)), json)
}

pub fn listar_pendentes(pasta: &Path) -> Vec<(PathBuf, Pendente)> {
    let Ok(entradas) = std::fs::read_dir(pasta_pendentes(pasta)) else {
        return Vec::new();
    };
    entradas
        .flatten()
        .map(|e| e.path())
        .filter_map(|p| Some((p.clone(), serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?)))
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_a_saida_do_curl() {
        assert_eq!(
            ler_saida("{\"run_id\":\"abc\"}\n201"),
            Some(Resposta { status: 201, corpo: json!({ "run_id": "abc" }) })
        );
        let erro = ler_saida("{\"error\":\"saldo de fichas insuficiente\"}\n400\n").unwrap();
        assert!(!erro.ok());
        assert_eq!(erro.erro(), "saldo de fichas insuficiente");
        assert_eq!(ler_saida("\n502").unwrap().erro(), "HTTP 502");
        assert_eq!(ler_saida("lixo"), None);
    }

    #[test]
    fn traduz_o_modo_para_a_api() {
        assert_eq!(modo_da_api("hi-score"), Some("highscore"));
        assert_eq!(modo_da_api("caravan-classic"), Some("caravan-classic"));
        assert_eq!(modo_da_api("outro"), None);
    }

    #[test]
    fn prepara_o_envio_ou_explica_por_que_fica_local() {
        let pasta = std::env::temp_dir().join(format!("shmupx-envio-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let perfil = crate::placar::Perfil::de_json(
            r#"{"jogo":"earthion","sistema":"megadrive","jogo_id":"earthion","versao":"megadrive-4.0.0",
                "placar":{"endereco":"0xFF1C48","bytes":16,"codificacao":"digito_por_palavra"}}"#,
        )
        .unwrap();
        let escrever = |ini: &str| std::fs::write(pasta.join("shmupx.ini"), ini).unwrap();
        let usuario = |nome: &str| crate::config::identificar(&pasta, &format!("SX_{nome}_hs")).0;

        escrever("username = [ramonvalerio, convidado]\n");
        assert_eq!(preparar(&pasta, &usuario("ramonvalerio"), &perfil, "hi-score").unwrap_err(), "sem api no shmupx.ini");

        escrever("username = [ramonvalerio, convidado]\napi = https://api.exemplo\ntoken.ramonvalerio = sxm_1\n");
        let envio = preparar(&pasta, &usuario("ramonvalerio"), &perfil, "caravan-sprint").unwrap();
        assert_eq!((envio.jogo_id.as_str(), envio.modo), ("earthion", Some("caravan-sprint")));
        assert_eq!(envio.versao.as_deref(), Some("megadrive-4.0.0"));
        assert_eq!(
            preparar(&pasta, &usuario("convidado"), &perfil, "hi-score").unwrap_err(),
            "sem token.convidado no shmupx.ini"
        );
        std::fs::remove_dir_all(pasta).unwrap();
    }

    #[test]
    fn guarda_e_lista_pendentes() {
        let pasta = std::env::temp_dir().join(format!("shmupx-pendentes-{}", std::process::id()));
        let pendente = Pendente {
            usuario: "ramonvalerio".into(),
            run_id: "run-1".into(),
            jogo_id: "earthion".into(),
            placar: 1_101_200,
            encerrada: false,
        };

        guardar_pendente(&pasta, &pendente).unwrap();

        let lista = listar_pendentes(&pasta);
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].1, pendente);
        std::fs::remove_dir_all(pasta).unwrap();
    }
}
