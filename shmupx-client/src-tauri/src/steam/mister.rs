//! Biblioteca do MiSTer FPGA pela rede: lê por FTP (o MiSTer vem com o ProFTPD, usuário `root`)
//! os jogos de arcade (`.mra` em `/media/fat/_*`, um nível de subpastas) e os ROMs de MAME
//! (`games/mame` e `games/hbmame`). Só lista nomes; nada é baixado nem alterado no MiSTer. Quem
//! cruza com o catálogo é a interface.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BibliotecaMister {
    /// Endereço lido (IP ou nome), para mostrar e sincronizar de novo.
    pub host: String,
    /// Nomes dos `.mra` (sem a extensão), sem repetição.
    pub mras: Vec<String>,
    /// ROMs de MAME (`ket`, `dariusg`…), sem `.zip`, em minúsculas.
    pub roms: Vec<String>,
}

const TEMPO: Duration = Duration::from_secs(8);
const RAIZ: &str = "/media/fat";

/// IP ou nome de rede: só letras, dígitos, `.`, `-` e `:` (vai para a conexão, nunca para um shell).
fn host_valido(host: &str) -> bool {
    !host.is_empty() && host.len() <= 253 && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
}

struct Ftp {
    controle: BufReader<TcpStream>,
    endereco: SocketAddr,
}

impl Ftp {
    fn conectar(host: &str, usuario: &str, senha: &str) -> Result<Self, String> {
        let endereco = (host, 21)
            .to_socket_addrs()
            .map_err(|e| format!("endereço do MiSTer inválido: {e}"))?
            .next()
            .ok_or("endereço do MiSTer inválido")?;
        let fluxo = TcpStream::connect_timeout(&endereco, TEMPO)
            .map_err(|_| format!("não foi possível conectar ao MiSTer em {host}: confira o IP e se ele está ligado na mesma rede"))?;
        fluxo.set_read_timeout(Some(TEMPO)).ok();
        fluxo.set_write_timeout(Some(TEMPO)).ok();
        let mut ftp = Ftp { controle: BufReader::new(fluxo), endereco };
        ftp.esperar(&[220])?;
        let (codigo, _) = ftp.comando(&format!("USER {usuario}"), &[230, 331])?;
        if codigo == 331 {
            ftp.comando(&format!("PASS {senha}"), &[230])
                .map_err(|_| "o MiSTer recusou o usuário e a senha do FTP (padrão: root / 1)".to_string())?;
        }
        ftp.comando("TYPE A", &[200])?;
        Ok(ftp)
    }

    /// Lê uma resposta (inclusive as de várias linhas, `123-` … `123 `).
    fn resposta(&mut self) -> Result<(u16, String), String> {
        let mut linha = String::new();
        self.controle.read_line(&mut linha).map_err(|e| format!("o MiSTer parou de responder: {e}"))?;
        let codigo: u16 = linha.get(..3).and_then(|c| c.parse().ok()).ok_or_else(|| format!("resposta inesperada do FTP: {}", linha.trim()))?;
        if linha.as_bytes().get(3) == Some(&b'-') {
            let fim = format!("{codigo} ");
            loop {
                let mut outra = String::new();
                if self.controle.read_line(&mut outra).map_err(|e| e.to_string())? == 0 || outra.starts_with(&fim) {
                    break;
                }
            }
        }
        Ok((codigo, linha.trim().to_string()))
    }

    fn esperar(&mut self, ok: &[u16]) -> Result<(u16, String), String> {
        let (codigo, texto) = self.resposta()?;
        if ok.contains(&codigo) { Ok((codigo, texto)) } else { Err(format!("o MiSTer respondeu: {texto}")) }
    }

    fn comando(&mut self, comando: &str, ok: &[u16]) -> Result<(u16, String), String> {
        let fluxo = self.controle.get_mut();
        fluxo.write_all(format!("{comando}\r\n").as_bytes()).map_err(|e| format!("falha ao falar com o MiSTer: {e}"))?;
        self.esperar(ok)
    }

    /// Porta de dados (modo passivo): EPSV `(|||porta|)` ou PASV `(h1,h2,h3,h4,p1,p2)`.
    fn porta_de_dados(&mut self) -> Result<SocketAddr, String> {
        if let Ok((_, texto)) = self.comando("EPSV", &[229]) {
            let porta = texto.split("|||").nth(1).and_then(|r| r.split('|').next()).and_then(|p| p.parse::<u16>().ok());
            if let Some(porta) = porta {
                return Ok(SocketAddr::new(self.endereco.ip(), porta));
            }
        }
        let (_, texto) = self.comando("PASV", &[227])?;
        let numeros: Vec<u16> = texto
            .split(['(', ')'])
            .nth(1)
            .unwrap_or("")
            .split(',')
            .filter_map(|n| n.trim().parse().ok())
            .collect();
        if numeros.len() != 6 {
            return Err(format!("resposta PASV inesperada: {texto}"));
        }
        Ok(SocketAddr::new(self.endereco.ip(), numeros[4] * 256 + numeros[5]))
    }

    /// Nomes dentro de uma pasta (só o nome, sem o caminho). Pasta que não existe: lista vazia.
    fn listar(&mut self, caminho: &str) -> Result<Vec<String>, String> {
        let dados = self.porta_de_dados()?;
        let mut canal = TcpStream::connect_timeout(&dados, TEMPO).map_err(|e| format!("falha no canal de dados do FTP: {e}"))?;
        canal.set_read_timeout(Some(TEMPO)).ok();
        let (codigo, _) = self.comando(&format!("NLST {caminho}"), &[125, 150, 450, 550])?;
        if codigo >= 400 {
            return Ok(Vec::new());
        }
        let mut texto = Vec::new();
        canal.read_to_end(&mut texto).map_err(|e| format!("falha ao ler a lista do MiSTer: {e}"))?;
        drop(canal);
        self.esperar(&[226, 250])?;
        Ok(String::from_utf8_lossy(&texto)
            .lines()
            .map(|l| l.trim().rsplit('/').next().unwrap_or("").to_string())
            .filter(|n| !n.is_empty() && n != "." && n != "..")
            .collect())
    }
}

/// Pasta (sem extensão) de jogos/cores do MiSTer: `_Arcade`, `_Shmups`, `_Vertical`…
fn pasta_de_jogos(nome: &str) -> bool {
    nome.starts_with('_') && !nome.contains('.')
}

pub fn ler(host: &str, senha: Option<&str>) -> Result<BibliotecaMister, String> {
    let host = host.trim();
    if !host_valido(host) {
        return Err("informe o IP do MiSTer (ex.: 192.168.0.23)".into());
    }
    let mut ftp = Ftp::conectar(host, "root", senha.filter(|s| !s.is_empty()).unwrap_or("1"))?;
    let mut mras: Vec<String> = Vec::new();
    let guardar = |nomes: &[String], mras: &mut Vec<String>| {
        for nome in nomes {
            if let Some(jogo) = nome.strip_suffix(".mra").or_else(|| nome.strip_suffix(".MRA")) {
                if !mras.iter().any(|m| m == jogo) {
                    mras.push(jogo.to_string());
                }
            }
        }
    };
    for pasta in ftp.listar(RAIZ)?.into_iter().filter(|n| pasta_de_jogos(n)) {
        let caminho = format!("{RAIZ}/{pasta}");
        let nomes = ftp.listar(&caminho)?;
        guardar(&nomes, &mut mras);
        for sub in nomes.iter().filter(|n| pasta_de_jogos(n)) {
            let dentro = ftp.listar(&format!("{caminho}/{sub}"))?;
            guardar(&dentro, &mut mras);
        }
    }
    let mut roms: Vec<String> = Vec::new();
    for pasta in ["games/mame", "games/hbmame"] {
        for nome in ftp.listar(&format!("{RAIZ}/{pasta}"))? {
            let nome = nome.to_lowercase();
            if let Some(rom) = nome.strip_suffix(".zip").filter(|r| !r.is_empty()) {
                if !roms.iter().any(|r| r == rom) {
                    roms.push(rom.to_string());
                }
            }
        }
    }
    let _ = ftp.comando("QUIT", &[221]);
    mras.sort();
    roms.sort();
    Ok(BibliotecaMister { host: host.to_string(), mras, roms })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_enderecos_de_rede_viram_host() {
        assert!(host_valido("192.168.0.23"));
        assert!(host_valido("mister.local"));
        assert!(!host_valido(""));
        assert!(!host_valido("1.2.3.4; rm -rf"));
    }

    #[test]
    fn pastas_de_jogos_sao_as_com_sublinhado_e_sem_extensao() {
        assert!(pasta_de_jogos("_Arcade"));
        assert!(pasta_de_jogos("_Shmups"));
        assert!(!pasta_de_jogos("games"));
        assert!(!pasta_de_jogos("_x.rbf"));
    }

    /// Lê o MiSTer da rede: `MISTER=192.168.0.23 cargo test -- --ignored mister_da_rede --nocapture`.
    #[test]
    #[ignore = "depende de um MiSTer na rede"]
    fn mister_da_rede() {
        let host = std::env::var("MISTER").unwrap_or_else(|_| "192.168.0.23".into());
        let b = ler(&host, None).unwrap();
        println!("mras={} roms={} darius={:?}", b.mras.len(), b.roms.len(), b.mras.iter().filter(|m| m.contains("Darius")).collect::<Vec<_>>());
    }
}
