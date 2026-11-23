//! Instalação do ShmupX no MiSTer FPGA pelo Client, por SSH (o MiSTer vem com o SSH ligado,
//! padrão `root` / `1`). Faz o mesmo que o `shmupx-mister/scripts/implantar.ps1`, sem o jogador abrir
//! terminal nem mexer no MiSTer:
//!
//! 1. baixa o pacote do agente publicado no site (`downloads/mister/latest.json`, conferido pelo
//!    SHA-256) e o extrai em `/media/fat/shmupx`;
//! 2. se faltar, baixa o core MegaDrive com espelho de RAM (odelot, v1.1, o testado) para
//!    `shmupx/cores/` (o Earthion precisa dele para o placar);
//! 3. grava o `shmupx.ini` com o jogador, o endereço do ShmupX e o token (cópia em `.bak`);
//! 4. roda o `instalar.sh` no MiSTer: menu ShmupX por jogador e modo, telas no menu Scripts e o
//!    serviço no boot. Rodar de novo não duplica nada nem sobrescreve perfis.

use std::sync::Arc;
use std::time::Duration;

use russh::ChannelMsg;
use russh::client;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const MANIFESTO: &str = "https://www.shmupx.com/downloads/mister/latest.json";
const PASTA: &str = "/media/fat/shmupx";
/// Core MegaDrive com espelho de RAM (fork de RetroAchievements de odelot), versão testada com o
/// agente. Fica fora dos menus, em shmupx/cores, com o nome que o instalador procura.
const CORE_URL: &str = "https://github.com/odelot/MegaDrive_MiSTer/releases/download/v1.1/MegaDrive.rbf";
const CORE_NOME: &str = "MegaDriveRA_20260815.rbf";

#[derive(Deserialize)]
struct Manifesto {
    version: String,
    url: String,
    sha256: String,
}

struct Cliente;

impl client::Handler for Cliente {
    type Error = russh::Error;

    /// MiSTer na rede local do jogador, sem autoridade de chaves: aceita a chave do aparelho.
    async fn check_server_key(&mut self, _chave: &russh::keys::PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

pub(crate) struct Ssh {
    sessao: client::Handle<Cliente>,
}

impl Ssh {
    pub(crate) async fn conectar(host: &str, usuario: &str, senha: &str) -> Result<Self, String> {
        let config = Arc::new(client::Config { inactivity_timeout: Some(Duration::from_secs(180)), ..Default::default() });
        let mut sessao = tokio::time::timeout(Duration::from_secs(12), client::connect(config, (host, 22), Cliente))
            .await
            .map_err(|_| format!("o MiSTer em {host} não respondeu ao SSH: {}", crate::descoberta::motivo_sem_conexao(host)))?
            .map_err(|e| format!("não foi possível abrir o SSH no MiSTer em {host} ({e}): {}", crate::descoberta::motivo_sem_conexao(host)))?;
        let resultado = sessao.authenticate_password(usuario, senha).await.map_err(|e| format!("falha no login SSH do MiSTer: {e}"))?;
        if !resultado.success() {
            return Err(format!("o MiSTer recusou o usuário {usuario} e a senha (padrão: root / 1)"));
        }
        Ok(Self { sessao })
    }

    /// Roda um comando no MiSTer (com `entrada` no stdin) e devolve o código de saída e a saída.
    pub(crate) async fn executar(&self, comando: &str, entrada: Option<&[u8]>) -> Result<(u32, String), String> {
        let mut canal = self.sessao.channel_open_session().await.map_err(|e| format!("falha no SSH: {e}"))?;
        canal.exec(true, comando).await.map_err(|e| format!("falha no SSH: {e}"))?;
        if let Some(dados) = entrada {
            canal.data(dados).await.map_err(|e| format!("falha ao enviar para o MiSTer: {e}"))?;
        }
        canal.eof().await.map_err(|e| format!("falha no SSH: {e}"))?;
        let mut saida = Vec::new();
        let mut codigo = None;
        while let Some(mensagem) = canal.wait().await {
            match mensagem {
                ChannelMsg::Data { data } => saida.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, .. } => saida.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => codigo = Some(exit_status),
                _ => {}
            }
        }
        Ok((codigo.unwrap_or(0), String::from_utf8_lossy(&saida).into_owned()))
    }

    /// Como `executar`, mas código diferente de zero vira erro com o fim da saída.
    pub(crate) async fn exigir(&self, comando: &str, entrada: Option<&[u8]>, passo: &str) -> Result<String, String> {
        let (codigo, saida) = self.executar(comando, entrada).await?;
        if codigo != 0 {
            let fim: Vec<&str> = saida.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
            return Err(format!("{passo} falhou no MiSTer (código {codigo}): {}", fim.join(" | ")));
        }
        Ok(saida)
    }
}

async fn baixar(url: &str) -> Result<Vec<u8>, String> {
    let resposta = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("falha ao baixar {url}: {e}"))?;
    if !resposta.status().is_success() {
        return Err(format!("{url} respondeu HTTP {}", resposta.status()));
    }
    Ok(resposta.bytes().await.map_err(|e| format!("falha ao baixar {url}: {e}"))?.to_vec())
}

/// Instala (ou atualiza) o ShmupX no MiSTer e liga o jogador `username` à conta com `token`.
/// `etapa` recebe o andamento para a tela. Devolve a saída do instalador.
pub async fn instalar(host: &str, usuario: &str, senha: &str, username: &str, token: &str, api: &str, etapa: impl Fn(&str)) -> Result<String, String> {
    etapa("Conectando ao MiSTer…");
    let ssh = Ssh::conectar(host, usuario, senha).await?;

    etapa("Baixando o ShmupX para o MiSTer…");
    let manifesto: Manifesto = serde_json::from_slice(&baixar(MANIFESTO).await?).map_err(|e| format!("pacote do MiSTer inválido: {e}"))?;
    let pacote = baixar(&manifesto.url).await?;
    let hash: String = Sha256::digest(&pacote).iter().map(|b| format!("{b:02x}")).collect();
    if !hash.eq_ignore_ascii_case(&manifesto.sha256) {
        return Err("o pacote do MiSTer baixado não confere (SHA-256); tente de novo".into());
    }

    etapa("Copiando o ShmupX para o MiSTer…");
    ssh.exigir(&format!("mkdir -p {PASTA} && tar -xf - -C {PASTA}"), Some(&pacote), "copiar o ShmupX").await?;

    let (_, cores) = ssh.executar(&format!("ls {PASTA}/cores/MegaDriveRA_*.rbf 2>/dev/null"), None).await?;
    if cores.trim().is_empty() {
        etapa("Baixando o core MegaDrive (odelot)…");
        let core = baixar(CORE_URL).await?;
        ssh.exigir(&format!("mkdir -p {PASTA}/cores && cat > {PASTA}/cores/{CORE_NOME}"), Some(&core), "copiar o core MegaDrive").await?;
    }

    etapa("Ligando o jogador à sua conta…");
    let (_, atual) = ssh.executar(&format!("cat {PASTA}/shmupx.ini 2>/dev/null"), None).await?;
    if !atual.trim().is_empty() {
        ssh.exigir(&format!("cp {PASTA}/shmupx.ini {PASTA}/shmupx.ini.bak"), None, "guardar o shmupx.ini").await?;
    }
    let novo = crate::rede::ini_com_token(&atual, username, token, api);
    ssh.exigir(&format!("cat > {PASTA}/shmupx.ini"), Some(novo.as_bytes()), "gravar o shmupx.ini").await?;

    etapa("Montando o menu ShmupX no MiSTer…");
    let comando = format!(
        "cd {PASTA} && sed -i 's/\\r$//' *.sh Scripts/*.sh && mkdir -p perfis && \
         for p in perfis-novos/*.json; do [ -e \"perfis/$(basename \"$p\")\" ] || cp \"$p\" perfis/; done; ./instalar.sh"
    );
    let saida = ssh.exigir(&comando, None, "o instalador").await?;
    etapa(&format!("Pronto: ShmupX {} instalado no MiSTer.", manifesto.version));
    Ok(saida)
}

/// Um jogo no menu ShmupX do MiSTer, para um jogador e um modo.
#[derive(Debug, serde::Serialize, PartialEq)]
pub struct JogoNoMenu {
    /// Nome no menu (`Earthion`, `Darius Gaiden`): o título do jogo no catálogo.
    pub jogo: String,
    /// Modo como aparece no menu (`Hi-Score`, `1CC`, `Caravan · Sprint (2 min)`).
    pub modo: String,
    /// Caminho do `.mgl`/`.mra` no MiSTer.
    pub caminho: String,
}

const MENU: &str = "/media/fat/_ShmupX";

/// Pasta do jogador no menu (`_1 RAMONVALERIO`, `_02 KWAYD`) é deste `username`?
fn pasta_do_jogador(pasta: &str, username: &str) -> bool {
    let Some(resto) = pasta.strip_prefix('_') else { return false };
    let Some((numero, nome)) = resto.split_once(' ') else { return false };
    numero.chars().all(|c| c.is_ascii_digit()) && !nome.is_empty() && username.to_uppercase().starts_with(&nome.to_uppercase())
}

/// Lê as linhas do `find` no menu e fica com os jogos do jogador, com o modo de cada um.
fn jogos_do_jogador(listagem: &str, username: &str) -> Vec<JogoNoMenu> {
    let mut jogos: Vec<JogoNoMenu> = listagem
        .lines()
        .filter_map(|caminho| {
            let relativo = caminho.trim().strip_prefix(MENU)?.strip_prefix('/')?;
            let partes: Vec<&str> = relativo.split('/').collect();
            if partes.len() < 3 || !pasta_do_jogador(partes[0], username) {
                return None;
            }
            let arquivo = partes[partes.len() - 1];
            let jogo = arquivo.strip_suffix(".mgl").or_else(|| arquivo.strip_suffix(".mra"))?;
            let modo = partes[1..partes.len() - 1].iter().map(|p| p.trim_start_matches('_')).collect::<Vec<_>>().join(" · ");
            Some(JogoNoMenu { jogo: jogo.to_string(), modo, caminho: caminho.trim().to_string() })
        })
        .collect();
    jogos.sort_by(|a, b| (&a.jogo, &a.modo).cmp(&(&b.jogo, &b.modo)));
    jogos
}

/// Jogos e modos do menu ShmupX do MiSTer para o jogador `username`.
pub async fn jogos_no_menu(host: &str, usuario: &str, senha: &str, username: &str) -> Result<Vec<JogoNoMenu>, String> {
    let ssh = Ssh::conectar(host, usuario, senha).await?;
    let (_, listagem) = ssh
        .executar(&format!("find {MENU} -mindepth 3 -maxdepth 4 -type f \\( -name '*.mgl' -o -name '*.mra' \\) 2>/dev/null"), None)
        .await?;
    Ok(jogos_do_jogador(&listagem, username))
}

/// Abre um jogo do menu ShmupX no MiSTer (como escolher no menu da TV): o MiSTer carrega o core
/// e o agente vê o jogador e o modo pelo nome do core.
pub async fn abrir(host: &str, usuario: &str, senha: &str, caminho: &str) -> Result<(), String> {
    let valido = caminho.starts_with(&format!("{MENU}/"))
        && (caminho.ends_with(".mgl") || caminho.ends_with(".mra"))
        && !caminho.contains(['\'', '\n', '\r'])
        && !caminho.contains("..");
    if !valido {
        return Err("jogo do MiSTer inválido".into());
    }
    let ssh = Ssh::conectar(host, usuario, senha).await?;
    ssh.exigir(&format!("echo 'load_core {caminho}' > /dev/MiSTer_cmd"), None, "abrir o jogo").await?;
    Ok(())
}

/// Volta o MiSTer ao menu principal (como apertar o menu no controle): o agente vê o jogo fechar
/// e grava a partida.
pub async fn voltar_ao_menu(host: &str, usuario: &str, senha: &str) -> Result<(), String> {
    let ssh = Ssh::conectar(host, usuario, senha).await?;
    ssh.exigir("echo 'load_core /media/fat/menu.rbf' > /dev/MiSTer_cmd", None, "voltar ao menu").await?;
    Ok(())
}

/// O aparelho em `host` é um MiSTer que aceita este usuário e senha (tem `/media/fat`)?
pub async fn eh_mister(host: &str, usuario: &str, senha: &str) -> bool {
    let Ok(ssh) = Ssh::conectar(host, usuario, senha).await else { return false };
    matches!(ssh.executar("test -d /media/fat/_ShmupX -o -d /media/fat/config && echo mister", None).await,
        Ok((0, saida)) if saida.contains("mister"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jogos_do_menu_ficam_com_o_jogador_e_o_modo() {
        let listagem = "/media/fat/_ShmupX/_1 RAMONVALERIO/_Hi-Score/Earthion.mgl\n\
            /media/fat/_ShmupX/_1 RAMONVALERIO/_Caravan/_Sprint (2 min)/Darius Gaiden.mra\n\
            /media/fat/_ShmupX/_2 KWAYD/_Hi-Score/Earthion.mgl\n\
            /media/fat/_ShmupX/cores/x.mra\n";
        let jogos = jogos_do_jogador(listagem, "ramonvalerio");
        assert_eq!(jogos.len(), 2);
        assert_eq!((jogos[0].jogo.as_str(), jogos[0].modo.as_str()), ("Darius Gaiden", "Caravan · Sprint (2 min)"));
        assert_eq!((jogos[1].jogo.as_str(), jogos[1].modo.as_str()), ("Earthion", "Hi-Score"));
        assert!(pasta_do_jogador("_10 KWAYD", "kwayd") && !pasta_do_jogador("cores", "kwayd") && !pasta_do_jogador("_1 KWAYD", "ramon"));
    }

    /// Só lê: `cargo test -- --ignored jogos_do_mister_da_rede --nocapture`.
    #[tokio::test]
    #[ignore = "depende de um MiSTer na rede"]
    async fn jogos_do_mister_da_rede() {
        let host = std::env::var("MISTER").unwrap_or_else(|_| "192.168.0.23".into());
        for jogo in jogos_no_menu(&host, "root", "1", "ramonvalerio").await.unwrap() {
            println!("{} | {} | {}", jogo.jogo, jogo.modo, jogo.caminho);
        }
    }

    /// Instala no MiSTer da rede mantendo o token que já está lá (não pede um novo ao backend):
    /// `MISTER=192.168.0.23 MISTER_JOGADOR=ramonvalerio cargo test -- --ignored instalar_no_mister_da_rede --nocapture`.
    #[tokio::test]
    #[ignore = "grava no MiSTer da rede"]
    async fn instalar_no_mister_da_rede() {
        let host = std::env::var("MISTER").unwrap_or_else(|_| "192.168.0.23".into());
        let jogador = std::env::var("MISTER_JOGADOR").unwrap_or_else(|_| "ramonvalerio".into());
        let ssh = Ssh::conectar(&host, "root", "1").await.unwrap();
        let (_, ini) = ssh.executar(&format!("cat {PASTA}/shmupx.ini"), None).await.unwrap();
        let chave = format!("token.{jogador}");
        let token = ini
            .lines()
            .find_map(|l| l.split_once('=').filter(|(c, _)| c.trim() == chave).map(|(_, v)| v.trim().to_string()))
            .expect("o jogador não tem token no shmupx.ini");
        drop(ssh);
        let api = "https://shmupx-backend-230979440400.us-central1.run.app";
        let saida = instalar(&host, "root", "1", &jogador, &token, api, |etapa| println!("etapa: {etapa}")).await.unwrap();
        println!("--- instalar.sh ---\n{saida}");
    }
}
