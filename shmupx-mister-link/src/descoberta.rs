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

/// Espera da conferência rápida antes de falar com o MiSTer (em vez dos 12 s do SSH).
const TEMPO_ALCANCE: Duration = Duration::from_millis(1500);

/// Por que o MiSTer está fora de alcance, para a interface explicar com palavras simples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemConexao {
    /// Este aparelho não está numa rede local.
    SemRede,
    /// Este aparelho e o MiSTer estão em redes diferentes.
    OutraRede,
    /// Mesma rede, mas o MiSTer não respondeu (desligado, fora da rede ou o roteador separa).
    SemResposta,
}

fn classificar(host: &str) -> SemConexao {
    let Some(proprio) = ip_local() else { return SemConexao::SemRede };
    match host.trim().parse::<Ipv4Addr>() {
        Ok(alvo) if alvo.octets()[..3] != proprio.octets()[..3] => SemConexao::OutraRede,
        _ => SemConexao::SemResposta,
    }
}

/// Conferência rápida (1,5 s) se o MiSTer em `host` responde na rede, antes de abrir um jogo.
pub async fn alcance(host: &str) -> Result<(), SemConexao> {
    let host = host.trim();
    if host.is_empty() {
        return Err(SemConexao::SemResposta);
    }
    match tokio::time::timeout(TEMPO_ALCANCE, TcpStream::connect((host, 22))).await {
        Ok(Ok(_)) => Ok(()),
        _ => Err(classificar(host)),
    }
}

/// Por que este aparelho não alcançou o MiSTer em `host`, para o log: fora de uma rede local, numa
/// sub-rede diferente da do MiSTer, ou na mesma (aí é o roteador ou o MiSTer).
pub fn motivo_sem_conexao(host: &str) -> String {
    let Some(proprio) = ip_local() else {
        return "este aparelho não está numa rede Wi-Fi local; ligue o Wi-Fi da mesma rede do MiSTer".into();
    };
    match host.trim().parse::<Ipv4Addr>() {
        Ok(alvo) if alvo.octets()[..3] != proprio.octets()[..3] => format!(
            "este aparelho está na rede {proprio}, e o MiSTer em {alvo} é de outra rede; ligue o Wi-Fi da mesma rede do MiSTer (redes de convidados e algumas redes 5 GHz/mesh ficam separadas)"
        ),
        _ => format!(
            "este aparelho ({proprio}) está na mesma rede, mas o MiSTer não respondeu; confira se ele está ligado e se o roteador não isola os aparelhos do Wi-Fi (\"isolamento de AP\"/\"client isolation\")"
        ),
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
