//! Fora do Windows ainda não há captura da janela do jogo (no Windows ela usa o Windows Graphics
//! Capture): a live fica indisponível, com a mesma interface para o resto do módulo compilar.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub const LARGURA: usize = 1280;
pub const ALTURA: usize = 720;
pub const QUADROS_POR_SEGUNDO: u32 = 60;

pub type Tela = Arc<Mutex<Vec<u8>>>;

pub struct Window;

pub struct CapturaJanela {
    pub tela: Tela,
    pub fechada: Arc<AtomicBool>,
    pub pid: u32,
}

pub fn janela_dos_processos(_pids: &[u32]) -> Option<Window> {
    None
}

pub fn iniciar(_janela: Window) -> Result<CapturaJanela, String> {
    Err("a live ainda não está disponível no Linux".to_string())
}

pub fn alimentar(_destino: std::fs::File, _tela: Tela, _parar: Arc<AtomicBool>) {}
