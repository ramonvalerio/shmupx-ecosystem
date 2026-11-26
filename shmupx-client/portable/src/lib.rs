//! Formato do ShmupX Portable: o lançador (`shmupx-portable.exe`) com o Client inteiro anexado no
//! fim do próprio `.exe`, num `.zip`:
//!
//! ```text
//! [lançador .exe][pacote .zip][versão em texto][u32: tamanho da versão][u64: tamanho do zip][SHMXPRT1]
//! ```
//!
//! O Windows ignora o que vem depois da imagem do `.exe`; o lançador lê o rodapé do próprio arquivo
//! para achar o `.zip` e a versão.

use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// Assinatura no fim do arquivo: sem ela, o `.exe` não traz pacote (download cortado ou lançador puro).
pub const MAGICO: [u8; 8] = *b"SHMXPRT1";
/// Tamanho fixo do fim do rodapé: tamanho da versão (u32), tamanho do zip (u64) e assinatura.
const CAUDA: u64 = 4 + 8 + 8;

/// Arquivo que marca a pasta extraída como completa (gravado por último, antes de renomear).
pub const PRONTO: &str = ".pronto";
/// Arquivo dentro do pacote que diz ao Client que ele roda como versão portátil.
pub const MARCA_PORTATIL: &str = ".portable";

/// Versão `maior.menor.patch`, comparável.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Versao(pub u32, pub u32, pub u32);

impl Versao {
    pub fn ler(texto: &str) -> Option<Self> {
        let mut partes = texto.trim().split('.').map(|p| p.parse::<u32>().ok());
        let versao = Versao(partes.next()??, partes.next()??, partes.next()??);
        partes.next().is_none().then_some(versao)
    }
}

impl fmt::Display for Versao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Rodapé a gravar depois do `.zip`.
pub fn rodape(versao: Versao, tamanho_zip: u64) -> Vec<u8> {
    let texto = versao.to_string().into_bytes();
    let mut saida = texto.clone();
    saida.extend_from_slice(&(texto.len() as u32).to_le_bytes());
    saida.extend_from_slice(&tamanho_zip.to_le_bytes());
    saida.extend_from_slice(&MAGICO);
    saida
}

/// O pacote anexado a um `.exe`.
#[derive(Debug)]
pub struct Pacote {
    pub versao: Versao,
    arquivo: File,
    inicio: u64,
    tamanho: u64,
}

fn invalido(motivo: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, motivo.to_string())
}

impl Pacote {
    pub fn abrir(exe: &Path) -> io::Result<Self> {
        let mut arquivo = File::open(exe)?;
        let total = arquivo.metadata()?.len();
        if total < CAUDA {
            return Err(invalido("arquivo pequeno demais para ter pacote"));
        }
        let mut cauda = [0u8; CAUDA as usize];
        arquivo.seek(SeekFrom::Start(total - CAUDA))?;
        arquivo.read_exact(&mut cauda)?;
        if cauda[12..] != MAGICO {
            return Err(invalido("sem pacote no fim do arquivo (download incompleto?)"));
        }
        let tamanho_versao = u64::from(u32::from_le_bytes(cauda[..4].try_into().unwrap()));
        let tamanho = u64::from_le_bytes(cauda[4..12].try_into().unwrap());
        let fim_zip = total
            .checked_sub(CAUDA + tamanho_versao)
            .ok_or_else(|| invalido("rodapé inconsistente"))?;
        let inicio = fim_zip.checked_sub(tamanho).ok_or_else(|| invalido("rodapé inconsistente"))?;
        if tamanho_versao > 64 {
            return Err(invalido("rodapé inconsistente"));
        }
        let mut texto = vec![0u8; tamanho_versao as usize];
        arquivo.seek(SeekFrom::Start(fim_zip))?;
        arquivo.read_exact(&mut texto)?;
        let versao = std::str::from_utf8(&texto).ok().and_then(Versao::ler).ok_or_else(|| invalido("versão ilegível no rodapé"))?;
        Ok(Pacote { versao, arquivo, inicio, tamanho })
    }

    /// Leitor só do trecho do `.zip`, com posições a partir do começo dele.
    pub fn zip(&self) -> io::Result<Trecho> {
        Ok(Trecho { arquivo: self.arquivo.try_clone()?, inicio: self.inicio, tamanho: self.tamanho, posicao: 0 })
    }
}

/// Uma faixa de um arquivo vista como arquivo inteiro.
pub struct Trecho {
    arquivo: File,
    inicio: u64,
    tamanho: u64,
    posicao: u64,
}

impl Read for Trecho {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let resta = self.tamanho.saturating_sub(self.posicao);
        if resta == 0 {
            return Ok(0);
        }
        let n = buf.len().min(usize::try_from(resta).unwrap_or(usize::MAX));
        self.arquivo.seek(SeekFrom::Start(self.inicio + self.posicao))?;
        let lidos = self.arquivo.read(&mut buf[..n])?;
        self.posicao += lidos as u64;
        Ok(lidos)
    }
}

impl Seek for Trecho {
    fn seek(&mut self, de: SeekFrom) -> io::Result<u64> {
        let nova = match de {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(d) => self.tamanho.checked_add_signed(d),
            SeekFrom::Current(d) => self.posicao.checked_add_signed(d),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "posição antes do começo do pacote"))?;
        self.posicao = nova;
        Ok(nova)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn versoes() {
        assert_eq!(Versao::ler("0.1.18"), Some(Versao(0, 1, 18)));
        assert!(Versao::ler("0.1").is_none());
        assert!(Versao::ler("0.1.x").is_none());
        assert!(Versao::ler("0.1.2.3").is_none());
        assert!(Versao(0, 1, 18) > Versao(0, 1, 9));
        assert_eq!(Versao(1, 2, 3).to_string(), "1.2.3");
    }

    #[test]
    fn le_o_pacote_anexado() {
        let caminho = std::env::temp_dir().join(format!("shmupx-portable-teste-{}.exe", std::process::id()));
        let zip = b"PK-conteudo-do-zip";
        let mut arquivo = File::create(&caminho).unwrap();
        arquivo.write_all(b"MZ-lancador").unwrap();
        arquivo.write_all(zip).unwrap();
        arquivo.write_all(&rodape(Versao(0, 1, 18), zip.len() as u64)).unwrap();
        drop(arquivo);

        let pacote = Pacote::abrir(&caminho).unwrap();
        assert_eq!(pacote.versao, Versao(0, 1, 18));
        let mut trecho = pacote.zip().unwrap();
        let mut lido = Vec::new();
        trecho.read_to_end(&mut lido).unwrap();
        assert_eq!(lido, zip);
        trecho.seek(SeekFrom::End(-3)).unwrap();
        let mut fim = Vec::new();
        trecho.read_to_end(&mut fim).unwrap();
        assert_eq!(fim, b"zip");
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn exe_sem_pacote_e_recusado() {
        let caminho = std::env::temp_dir().join(format!("shmupx-portable-vazio-{}.exe", std::process::id()));
        std::fs::write(&caminho, b"MZ-lancador-sem-nada-anexado-ao-fim").unwrap();
        assert!(Pacote::abrir(&caminho).is_err());
        std::fs::remove_file(&caminho).unwrap();
    }
}
