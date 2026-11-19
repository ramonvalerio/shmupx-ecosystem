//! Mega Drive / Genesis: RAM de trabalho do 68000 (64 KB em `0xFF0000`).
//!
//! O RetroAchievements numera essa RAM de 0 a `0xFFFF`, mas com os bytes de cada palavra
//! trocados (o 68000 é big-endian e os emuladores rodam em hosts little-endian). Aqui tudo
//! fica na ordem do 68000; a troca acontece só na fronteira com o espelho.

use crate::espelho::{Erro, Espelho, MAX_ENDERECOS};
use crate::memoria::Memoria;

pub const TAMANHO_WRAM: usize = 0x1_0000;
const INICIO_WRAM_68K: u32 = 0xFF_0000;

/// Endereço RA de um byte da WRAM (deslocamento na ordem do 68000), e vice-versa.
pub fn endereco_ra(deslocamento_68k: u32) -> u32 {
    deslocamento_68k ^ 1
}

/// Aceita o endereço como o 68000 vê (`0xFF1234`) ou já como deslocamento (`0x1234`).
pub fn deslocamento_wram(endereco: u32) -> Option<u32> {
    match endereco {
        e if e < TAMANHO_WRAM as u32 => Some(e),
        e if e >= INICIO_WRAM_68K => Some(e - INICIO_WRAM_68K),
        _ => None,
    }
}

/// Lê bytes consecutivos da WRAM, na ordem do 68000.
#[cfg(test)]
pub fn ler_bytes<M: Memoria>(espelho: &mut Espelho<M>, deslocamento: u32, quantidade: usize) -> Result<Vec<u8>, Erro> {
    let (enderecos, ordem) = enderecos_ordenados(deslocamento, quantidade);
    let leitura = espelho.consultar(&enderecos)?;
    Ok(ordem.iter().map(|&i| leitura.valores[i]).collect())
}

/// Endereços RA em ordem crescente (exigida pelo protocolo) e, para cada byte pedido, o seu
/// índice nessa lista.
pub fn enderecos_ordenados(deslocamento: u32, quantidade: usize) -> (Vec<u32>, Vec<usize>) {
    let ra: Vec<u32> = (0..quantidade as u32).map(|i| endereco_ra(deslocamento + i)).collect();
    let mut ordenados = ra.clone();
    ordenados.sort_unstable();
    let ordem = ra.iter().map(|e| ordenados.binary_search(e).unwrap()).collect();
    (ordenados, ordem)
}

/// Copia os 64 KB inteiros, 4096 bytes por quadro (16 quadros, cerca de 0,3 s).
pub fn despejar_wram<M: Memoria>(espelho: &mut Espelho<M>) -> Result<Vec<u8>, Erro> {
    let mut wram = vec![0; TAMANHO_WRAM];
    for pagina in (0..TAMANHO_WRAM as u32).step_by(MAX_ENDERECOS) {
        let enderecos: Vec<u32> = (pagina..pagina + MAX_ENDERECOS as u32).collect();
        let leitura = espelho.consultar(&enderecos)?;
        for (endereco, valor) in enderecos.iter().zip(leitura.valores) {
            wram[endereco_ra(*endereco) as usize] = valor;
        }
    }
    Ok(wram)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::espelho::simulador::FpgaSimulado;

    fn wram_de_teste() -> Vec<u8> {
        (0..TAMANHO_WRAM).map(|i| (i * 7 + i / 256) as u8).collect()
    }

    #[test]
    fn despejo_devolve_a_wram_na_ordem_do_68000() {
        let wram = wram_de_teste();
        let mut espelho = Espelho::novo(FpgaSimulado::novo(wram.clone()));

        assert_eq!(despejar_wram(&mut espelho).unwrap(), wram);
    }

    #[test]
    fn le_bytes_em_endereco_impar() {
        let wram = wram_de_teste();
        let mut espelho = Espelho::novo(FpgaSimulado::novo(wram.clone()));

        assert_eq!(ler_bytes(&mut espelho, 0x1233, 5).unwrap(), wram[0x1233..0x1238]);
    }

    #[test]
    fn aceita_endereco_do_68000_ou_deslocamento() {
        assert_eq!(deslocamento_wram(0xFF1234), Some(0x1234));
        assert_eq!(deslocamento_wram(0x1234), Some(0x1234));
        assert_eq!(deslocamento_wram(0x20_0000), None);
    }
}
