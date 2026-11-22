//! Instalação do ShmupX no MiSTer FPGA pelo Client, por SSH (o MiSTer vem com o SSH ligado,
//! usuário `root`). Faz o mesmo que o `shmupx-mister/scripts/implantar.ps1`, sem o jogador abrir
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

struct Ssh {
    sessao: client::Handle<Cliente>,
}

impl Ssh {
    async fn conectar(host: &str, senha: &str) -> Result<Self, String> {
        let config = Arc::new(client::Config { inactivity_timeout: Some(Duration::from_secs(180)), ..Default::default() });
        let mut sessao = tokio::time::timeout(Duration::from_secs(12), client::connect(config, (host, 22), Cliente))
            .await
            .map_err(|_| format!("o MiSTer em {host} não respondeu ao SSH: confira o IP e se ele está ligado"))?
            .map_err(|e| format!("não foi possível abrir o SSH no MiSTer em {host}: {e}"))?;
        let resultado = sessao.authenticate_password("root", senha).await.map_err(|e| format!("falha no login SSH do MiSTer: {e}"))?;
        if !resultado.success() {
            return Err("o MiSTer recusou a senha do root (a padrão é 1)".into());
        }
        Ok(Self { sessao })
    }

    /// Roda um comando no MiSTer (com `entrada` no stdin) e devolve o código de saída e a saída.
    async fn executar(&self, comando: &str, entrada: Option<&[u8]>) -> Result<(u32, String), String> {
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
    async fn exigir(&self, comando: &str, entrada: Option<&[u8]>, passo: &str) -> Result<String, String> {
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
pub async fn instalar(host: &str, senha: &str, username: &str, token: &str, api: &str, etapa: impl Fn(&str)) -> Result<String, String> {
    etapa("Conectando ao MiSTer…");
    let ssh = Ssh::conectar(host, senha).await?;

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
    let novo = super::mister::ini_com_token(&atual, username, token, api);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Instala no MiSTer da rede mantendo o token que já está lá (não pede um novo ao backend):
    /// `MISTER=192.168.0.23 MISTER_JOGADOR=ramonvalerio cargo test -- --ignored instalar_no_mister_da_rede --nocapture`.
    #[tokio::test]
    #[ignore = "grava no MiSTer da rede"]
    async fn instalar_no_mister_da_rede() {
        let host = std::env::var("MISTER").unwrap_or_else(|_| "192.168.0.23".into());
        let jogador = std::env::var("MISTER_JOGADOR").unwrap_or_else(|_| "ramonvalerio".into());
        let ssh = Ssh::conectar(&host, "1").await.unwrap();
        let (_, ini) = ssh.executar(&format!("cat {PASTA}/shmupx.ini"), None).await.unwrap();
        let chave = format!("token.{jogador}");
        let token = ini
            .lines()
            .find_map(|l| l.split_once('=').filter(|(c, _)| c.trim() == chave).map(|(_, v)| v.trim().to_string()))
            .expect("o jogador não tem token no shmupx.ini");
        drop(ssh);
        let api = "https://shmupx-backend-230979440400.us-central1.run.app";
        let saida = instalar(&host, "1", &jogador, &token, api, |etapa| println!("etapa: {etapa}")).await.unwrap();
        println!("--- instalar.sh ---\n{saida}");
    }
}
