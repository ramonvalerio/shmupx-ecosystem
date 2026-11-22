//! Procura o MiSTer na rede local, para o jogador não precisar saber o IP: testa a porta do SSH
//! em todos os endereços da mesma sub-rede (/24) e, nos que respondem, entra com o usuário e a
//! senha (padrão `root` / `1`) e confere se é um MiSTer.
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::task::JoinSet;

/// Espera por endereço da varredura (todos em paralelo).
const TEMPO_PORTA: Duration = Duration::from_millis(900);

/// IPv4 deste aparelho na rede local. Um socket UDP "conectado" não envia nada: só escolhe a
/// interface de saída.
pub fn ip_local() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.168.0.1:9").or_else(|_| socket.connect("10.0.0.1:9")).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if ip.is_private() => Some(ip),
        _ => None,
    }
}

/// Endereços da mesma /24, sem o deste aparelho.
fn vizinhos(ip: Ipv4Addr) -> Vec<Ipv4Addr> {
    let [a, b, c, proprio] = ip.octets();
    (1..=254u8).filter(|&d| d != proprio).map(|d| Ipv4Addr::new(a, b, c, d)).collect()
}

/// IPs dos MiSTers encontrados na rede (normalmente um).
pub async fn procurar(usuario: &str, senha: &str) -> Result<Vec<String>, String> {
    let ip = ip_local().ok_or("este aparelho não está numa rede Wi-Fi local; ligue o Wi-Fi da mesma rede do MiSTer")?;
    let mut tarefas = JoinSet::new();
    for vizinho in vizinhos(ip) {
        tarefas.spawn(async move {
            let endereco = SocketAddr::new(IpAddr::V4(vizinho), 22);
            matches!(tokio::time::timeout(TEMPO_PORTA, TcpStream::connect(endereco)).await, Ok(Ok(_))).then_some(vizinho)
        });
    }
    let mut com_ssh = Vec::new();
    while let Some(resultado) = tarefas.join_next().await {
        if let Ok(Some(vizinho)) = resultado {
            com_ssh.push(vizinho);
        }
    }
    com_ssh.sort();
    let mut misters = Vec::new();
    for host in com_ssh {
        let host = host.to_string();
        if crate::ssh::eh_mister(&host, usuario, senha).await {
            misters.push(host);
        }
    }
    Ok(misters)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vizinhos_sao_a_mesma_sub_rede_sem_o_proprio() {
        let lista = vizinhos(Ipv4Addr::new(192, 168, 0, 40));
        assert_eq!(lista.len(), 253);
        assert!(!lista.contains(&Ipv4Addr::new(192, 168, 0, 40)));
        assert_eq!(lista[0], Ipv4Addr::new(192, 168, 0, 1));
    }

    /// `cargo test -- --ignored procura_na_rede --nocapture`.
    #[tokio::test]
    #[ignore = "depende de um MiSTer na rede"]
    async fn procura_na_rede() {
        println!("{:?}", procurar("root", "1").await);
    }
}
