//! MiSTer FPGA pela rede, por FTP (o MiSTer vem com o ProFTPD, usuário `root`):
//! - Biblioteca: os jogos de arcade (`.mra` em `/media/fat/_*`, um nível de subpastas) e os ROMs
//!   de MAME (`games/mame` e `games/hbmame`). Só lista nomes; quem cruza com o catálogo é a
//!   interface.
//! - Conexão com a conta: grava `token.<username> = sxm_...` no `/media/fat/shmupx/shmupx.ini`
//!   (o mesmo arquivo do agente `shmupx-mister`), com cópia de segurança antes. É a única escrita.

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

    /// Conteúdo de um arquivo; `None` se ele não existe.
    fn baixar(&mut self, caminho: &str) -> Result<Option<Vec<u8>>, String> {
        self.comando("TYPE I", &[200])?;
        let dados = self.porta_de_dados()?;
        let mut canal = TcpStream::connect_timeout(&dados, TEMPO).map_err(|e| format!("falha no canal de dados do FTP: {e}"))?;
        canal.set_read_timeout(Some(TEMPO)).ok();
        let (codigo, _) = self.comando(&format!("RETR {caminho}"), &[125, 150, 450, 550])?;
        if codigo >= 400 {
            return Ok(None);
        }
        let mut conteudo = Vec::new();
        canal.read_to_end(&mut conteudo).map_err(|e| format!("falha ao ler {caminho} do MiSTer: {e}"))?;
        drop(canal);
        self.esperar(&[226, 250])?;
        Ok(Some(conteudo))
    }

    /// Grava (substitui) um arquivo.
    fn enviar(&mut self, caminho: &str, conteudo: &[u8]) -> Result<(), String> {
        self.comando("TYPE I", &[200])?;
        let dados = self.porta_de_dados()?;
        let mut canal = TcpStream::connect_timeout(&dados, TEMPO).map_err(|e| format!("falha no canal de dados do FTP: {e}"))?;
        self.comando(&format!("STOR {caminho}"), &[125, 150])
            .map_err(|e| format!("o MiSTer não deixou gravar {caminho}: {e}"))?;
        canal.write_all(conteudo).map_err(|e| format!("falha ao gravar {caminho} no MiSTer: {e}"))?;
        drop(canal);
        self.esperar(&[226, 250])?;
        Ok(())
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

const INI: &str = "/media/fat/shmupx/shmupx.ini";
const INI_BACKUP: &str = "/media/fat/shmupx/shmupx.ini.bak";

/// Nome de jogador do MiSTer: letras, dígitos, `_` e `-` (vira chave no `shmupx.ini`).
pub fn username_valido(nome: &str) -> bool {
    !nome.is_empty() && nome.len() <= 32 && nome.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Jogadores da linha `username = [a, b, c]` do `shmupx.ini`.
pub fn usuarios_do_ini(texto: &str) -> Vec<String> {
    texto
        .lines()
        .find_map(|linha| {
            let (chave, valor) = linha.split_once('=')?;
            (chave.trim() == "username").then(|| valor.trim().trim_start_matches('[').trim_end_matches(']').to_string())
        })
        .map(|lista| lista.split(',').map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect())
        .unwrap_or_default()
}

/// `shmupx.ini` com o jogador na lista `username`, o endereço do ShmupX (`api`, sem ele o agente
/// guarda as partidas só no ranking local) e a linha `token.<username>` nova (substitui a anterior
/// do mesmo jogador). O resto do arquivo fica como estava.
pub fn ini_com_token(texto: &str, username: &str, token: &str, api: &str) -> String {
    let mut usuarios = usuarios_do_ini(texto);
    if !usuarios.iter().any(|u| u == username) {
        usuarios.push(username.to_string());
    }
    // CONVIDADO (sem token): o MODO TREINO abre o jogo por ele, sem ficha e sem ranking online.
    if !usuarios.iter().any(|u| u == "convidado") {
        usuarios.push("convidado".to_string());
    }
    let linha_usuarios = format!("username = [{}]", usuarios.join(", "));
    let linha_token = format!("token.{username} = {token}");
    let linha_api = format!("api = {api}");
    let chave_token = format!("token.{username}");
    let mut linhas: Vec<String> = Vec::new();
    let (mut tem_usuarios, mut tem_token, mut tem_api) = (false, false, false);
    for linha in texto.lines() {
        let chave = linha.split_once('=').map(|(c, _)| c.trim()).unwrap_or("");
        if chave == "username" {
            linhas.push(linha_usuarios.clone());
            tem_usuarios = true;
        } else if chave == "api" {
            if !tem_api {
                linhas.push(linha_api.clone());
                tem_api = true;
            }
        } else if chave == chave_token {
            if !tem_token {
                linhas.push(linha_token.clone());
                tem_token = true;
            }
        } else {
            linhas.push(linha.to_string());
        }
    }
    if !tem_usuarios {
        linhas.insert(0, linha_usuarios);
    }
    if !tem_api {
        linhas.insert(1, linha_api);
    }
    if !tem_token {
        linhas.push(linha_token);
    }
    let mut saida = linhas.join("\n");
    saida.push('\n');
    saida
}

/// Jogadores já configurados no MiSTer (para escolher qual ligar à conta).
pub fn usuarios(host: &str, senha: Option<&str>) -> Result<Vec<String>, String> {
    let host = host.trim();
    if !host_valido(host) {
        return Err("informe o IP do MiSTer (ex.: 192.168.0.23)".into());
    }
    let mut ftp = Ftp::conectar(host, "root", senha.filter(|s| !s.is_empty()).unwrap_or("1"))?;
    let texto = ftp.baixar(INI)?.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let _ = ftp.comando("QUIT", &[221]);
    Ok(usuarios_do_ini(&texto))
}

/// Grava o token do jogador no `shmupx.ini` do MiSTer (guarda a versão anterior em `.bak`).
pub fn gravar_token(host: &str, senha: Option<&str>, username: &str, token: &str, api: &str) -> Result<(), String> {
    let host = host.trim();
    if !host_valido(host) {
        return Err("informe o IP do MiSTer (ex.: 192.168.0.23)".into());
    }
    if !username_valido(username) {
        return Err("nome de jogador inválido: use letras, números, _ e -".into());
    }
    let mut ftp = Ftp::conectar(host, "root", senha.filter(|s| !s.is_empty()).unwrap_or("1"))?;
    let atual = ftp.baixar(INI)?;
    if ftp.listar("/media/fat/shmupx")?.is_empty() && atual.is_none() {
        return Err("o ShmupX não está instalado neste MiSTer (falta /media/fat/shmupx)".into());
    }
    let texto = atual.as_deref().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    if let Some(original) = &atual {
        ftp.enviar(INI_BACKUP, original)?;
    }
    ftp.enviar(INI, ini_com_token(&texto, username, token, api).as_bytes())?;
    let _ = ftp.comando("QUIT", &[221]);
    Ok(())
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
    fn ini_ganha_o_token_e_mantem_o_resto() {
        let ini = "username = [ramonvalerio, kwayd]\ntoken.ramonvalerio = sxm_velho\n# comentario\n";
        assert_eq!(usuarios_do_ini(ini), vec!["ramonvalerio", "kwayd"]);
        let api = "https://api.shmupx";
        let novo = ini_com_token(ini, "ramonvalerio", "sxm_novo", api);
        assert_eq!(novo, "username = [ramonvalerio, kwayd, convidado]\napi = https://api.shmupx\ntoken.ramonvalerio = sxm_novo\n# comentario\n");
        let outro = ini_com_token(&novo, "dantrius", "sxm_d", api);
        assert!(outro.starts_with("username = [ramonvalerio, kwayd, convidado, dantrius]\napi = https://api.shmupx\n"));
        assert!(outro.contains("token.ramonvalerio = sxm_novo") && outro.ends_with("token.dantrius = sxm_d\n"));
        assert_eq!(outro.matches("api =").count(), 1);
        assert_eq!(ini_com_token("", "ramon", "sxm_x", api), "username = [ramon, convidado]\napi = https://api.shmupx\ntoken.ramon = sxm_x\n");
        assert!(!username_valido("a b") && !username_valido("") && username_valido("ramon_valerio"));
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
