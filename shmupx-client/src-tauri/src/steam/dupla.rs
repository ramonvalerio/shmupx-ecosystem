//! Treino em dupla online: o Client prepara a porta UDP do ShmupX Engine e descobre por onde o
//! parceiro chega até ela (rede local e o endereço público visto por um servidor STUN). Os dois
//! Clients trocam esses endereços pela sessão da dupla no Firestore e abrem o motor com
//! `-shmupx_netplay host|guest` (ver shmupx-engine/docs/NETPLAY.md).
//!
//! O STUN sai da mesma porta que o motor usa logo depois: o roteador (NAT) guarda o caminho aberto
//! por alguns segundos e os pacotes do parceiro entram por ele ("hole punching").

use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// Servidores STUN públicos (só respondem o endereço de onde o pacote veio).
const SERVIDORES_STUN: &[&str] = &["stun.l.google.com:19302", "stun1.l.google.com:19302", "stun.cloudflare.com:3478"];
const ESPERA_STUN: Duration = Duration::from_millis(1500);
const MAGIC_COOKIE: u32 = 0x2112_A442;

/// O que o Client publica na sessão da dupla: a porta do motor e os endereços para chegar nela.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparoDupla {
    pub porta: u16,
    pub candidatos: Vec<String>,
    /// Endereço público visto pelo STUN (sem ele, só a rede local funciona).
    pub publico: Option<String>,
}

/// Como abrir o motor na dupla (vem da interface).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dupla {
    /// `host` (quem convidou, jogador 1) ou `guest` (jogador 2).
    pub modo: String,
    pub porta: u16,
    pub parceiros: Vec<String>,
    pub chave: String,
}

impl Dupla {
    /// Só o que o motor aceita: modo conhecido, endereços `ip:porta` e chave hexadecimal.
    pub fn validar(&self) -> Result<(), String> {
        if self.modo != "host" && self.modo != "guest" {
            return Err("modo da dupla inválido".into());
        }
        if self.chave.len() < 16 || self.chave.len() > 128 || !self.chave.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("chave da dupla inválida".into());
        }
        if self.parceiros.is_empty() || self.parceiros.len() > 8 || self.parceiros.iter().any(|p| p.parse::<SocketAddrV4>().is_err()) {
            return Err("endereços do parceiro inválidos".into());
        }
        Ok(())
    }

    pub fn argumentos(&self) -> Vec<String> {
        vec![
            "-shmupx_netplay".into(),
            self.modo.clone(),
            "-shmupx_netplay_port".into(),
            self.porta.to_string(),
            "-shmupx_netplay_peer".into(),
            self.parceiros.join(","),
            "-shmupx_netplay_key".into(),
            self.chave.clone(),
        ]
    }
}

/// Abre uma porta UDP, pergunta o endereço público ao STUN e devolve a porta (já fechada, para o
/// motor abrir) com os endereços: rede local, público e a própria máquina (teste no mesmo PC).
#[tauri::command]
pub async fn dupla_preparar() -> Result<PreparoDupla, String> {
    tauri::async_runtime::spawn_blocking(preparar).await.map_err(|e| e.to_string())?
}

fn preparar() -> Result<PreparoDupla, String> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).map_err(|e| format!("não foi possível abrir uma porta UDP: {e}"))?;
    let porta = socket.local_addr().map_err(|e| e.to_string())?.port();
    let publico = stun(&socket);
    drop(socket);

    let mut candidatos: Vec<String> = Vec::new();
    if let Some(local) = ip_da_rede_local() {
        candidatos.push(format!("{local}:{porta}"));
    }
    if let Some(publico) = &publico {
        if !candidatos.contains(publico) {
            candidatos.push(publico.clone());
        }
    }
    candidatos.push(format!("127.0.0.1:{porta}"));
    log::info!("dupla: porta {porta}, endereços {}", candidatos.join(", "));
    Ok(PreparoDupla { porta, candidatos, publico })
}

/// IPv4 da rede local pelo qual o PC sai para a internet (nenhum pacote é enviado).
fn ip_da_rede_local() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()? {
        SocketAddr::V4(endereco) if !endereco.ip().is_loopback() && !endereco.ip().is_unspecified() => Some(*endereco.ip()),
        _ => None,
    }
}

/// Binding Request do STUN (RFC 5389) pelos servidores da lista; o primeiro que responder vale.
fn stun(socket: &UdpSocket) -> Option<String> {
    let mut transacao = [0u8; 12];
    getrandom::getrandom(&mut transacao).ok()?;
    let mut pedido = Vec::with_capacity(20);
    pedido.extend_from_slice(&0x0001u16.to_be_bytes());
    pedido.extend_from_slice(&0u16.to_be_bytes());
    pedido.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
    pedido.extend_from_slice(&transacao);

    let servidores: Vec<SocketAddr> = SERVIDORES_STUN
        .iter()
        .filter_map(|s| s.to_socket_addrs().ok())
        .flat_map(|enderecos| enderecos.filter(SocketAddr::is_ipv4).take(1))
        .collect();
    if servidores.is_empty() {
        log::warn!("dupla: nenhum servidor STUN resolvido");
        return None;
    }
    socket.set_read_timeout(Some(Duration::from_millis(250))).ok()?;
    let limite = Instant::now() + ESPERA_STUN;
    let mut resposta = [0u8; 512];
    while Instant::now() < limite {
        for servidor in &servidores {
            let _ = socket.send_to(&pedido, servidor);
        }
        let fim_da_rodada = Instant::now() + Duration::from_millis(400);
        while Instant::now() < fim_da_rodada {
            let Ok((n, _)) = socket.recv_from(&mut resposta) else { break };
            if let Some(endereco) = ler_resposta_stun(&resposta[..n], &transacao) {
                return Some(endereco.to_string());
            }
        }
    }
    log::warn!("dupla: o STUN não respondeu; só a rede local vai funcionar");
    None
}

/// XOR-MAPPED-ADDRESS (ou MAPPED-ADDRESS) IPv4 de uma Binding Success Response.
fn ler_resposta_stun(dados: &[u8], transacao: &[u8; 12]) -> Option<SocketAddrV4> {
    if dados.len() < 20 || u16::from_be_bytes([dados[0], dados[1]]) != 0x0101 {
        return None;
    }
    if u32::from_be_bytes([dados[4], dados[5], dados[6], dados[7]]) != MAGIC_COOKIE || &dados[8..20] != transacao {
        return None;
    }
    let tamanho = usize::from(u16::from_be_bytes([dados[2], dados[3]]));
    let fim = (20 + tamanho).min(dados.len());
    let mut pos = 20;
    let mut mapeado = None;
    while pos + 4 <= fim {
        let tipo = u16::from_be_bytes([dados[pos], dados[pos + 1]]);
        let len = usize::from(u16::from_be_bytes([dados[pos + 2], dados[pos + 3]]));
        let valor = dados.get(pos + 4..pos + 4 + len)?;
        if (tipo == 0x0020 || tipo == 0x0001) && len >= 8 && valor[1] == 0x01 {
            let mut porta = u16::from_be_bytes([valor[2], valor[3]]);
            let mut ip = u32::from_be_bytes([valor[4], valor[5], valor[6], valor[7]]);
            if tipo == 0x0020 {
                porta ^= (MAGIC_COOKIE >> 16) as u16;
                ip ^= MAGIC_COOKIE;
                return Some(SocketAddrV4::new(Ipv4Addr::from(ip), porta));
            }
            mapeado = Some(SocketAddrV4::new(Ipv4Addr::from(ip), porta));
        }
        pos += 4 + len.div_ceil(4) * 4;
    }
    mapeado
}

/// Eventos do motor na dupla (`SHMUPX_NETPLAY {...}`), repassados à interface como `dupla://evento`.
#[derive(Clone, Serialize)]
struct EventoDupla {
    pid: u32,
    evento: serde_json::Value,
}

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Guarda o app para repassar os eventos da dupla (a primeira chamada vale).
pub fn registrar_app(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Linha `SHMUPX_NETPLAY {...}` da saída do motor `pid`.
pub fn repassar_evento(pid: u32, linha: &str) {
    let Some(json) = linha.strip_prefix("SHMUPX_NETPLAY ") else { return };
    let Ok(evento) = serde_json::from_str::<serde_json::Value>(json.trim()) else { return };
    if let Some(app) = APP.get() {
        let _ = app.emit("dupla://evento", EventoDupla { pid, evento });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_xor_mapped_address() {
        let transacao = [1u8; 12];
        let mut resposta = vec![0x01, 0x01, 0x00, 0x0c];
        resposta.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        resposta.extend_from_slice(&transacao);
        // XOR-MAPPED-ADDRESS de 203.0.113.5:40000
        let porta = 40000u16 ^ (MAGIC_COOKIE >> 16) as u16;
        let ip = u32::from(Ipv4Addr::new(203, 0, 113, 5)) ^ MAGIC_COOKIE;
        resposta.extend_from_slice(&[0x00, 0x20, 0x00, 0x08, 0x00, 0x01]);
        resposta.extend_from_slice(&porta.to_be_bytes());
        resposta.extend_from_slice(&ip.to_be_bytes());
        assert_eq!(ler_resposta_stun(&resposta, &transacao), Some(SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 5), 40000)));
        // outra transação não vale
        assert_eq!(ler_resposta_stun(&resposta, &[2u8; 12]), None);
    }

    #[test]
    fn valida_a_dupla() {
        let boa = Dupla { modo: "host".into(), porta: 5000, parceiros: vec!["192.168.0.2:6000".into()], chave: "0123456789abcdef0123".into() };
        assert!(boa.validar().is_ok());
        assert!(Dupla { modo: "x".into(), ..boa.clone() }.validar().is_err());
        assert!(Dupla { chave: "naohex!!naohex!!".into(), ..boa.clone() }.validar().is_err());
        assert!(Dupla { parceiros: vec!["-shmupx_ranked".into()], ..boa.clone() }.validar().is_err());
        assert_eq!(boa.argumentos()[1], "host");
    }
}
