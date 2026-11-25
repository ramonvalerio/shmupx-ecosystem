//! Captura da janela do jogo para a live (spec 033/036 do Client antigo): Windows Graphics Capture
//! só da janela, nunca da tela inteira. Cada quadro é encaixado num quadro fixo 1280×720 (sem
//! distorcer, com barras pretas) — a janela pode mudar de tamanho ou ir para tela cheia sem a live
//! cair: a biblioteca recria o buffer no novo tamanho e o encaixe se ajusta.
//!
//! Só a área interna da janela vai para a live: a barra de título e as bordas do modo janela ficam
//! de fora (em tela cheia a área interna é a janela inteira).
//!
//! Quem escreve no FFmpeg é um relógio de 60 quadros/s, numa thread própria com timer de alta
//! resolução, que repete o último quadro quando a janela não manda um novo (pausa, tela parada),
//! então o vídeo nunca para.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use std::io::Write;

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
};
pub use windows_capture::window::Window;

pub const LARGURA: usize = 1280;
pub const ALTURA: usize = 720;
pub const QUADROS_POR_SEGUNDO: u32 = 60;
const BYTES: usize = LARGURA * ALTURA * 4;

/// Janela visível do processo (a maior, se houver várias), entre os `pids` dados.
pub fn janela_dos_processos(pids: &[u32]) -> Option<Window> {
    if pids.is_empty() {
        return None;
    }
    Window::enumerate()
        .ok()?
        .into_iter()
        .filter(|w| w.process_id().is_ok_and(|pid| pids.contains(&pid)))
        .filter_map(|w| {
            let area = i64::from(w.width().ok()?) * i64::from(w.height().ok()?);
            (area > 160 * 120).then_some((area, w))
        })
        .max_by_key(|(area, _)| *area)
        .map(|(_, w)| w)
}

/// Encaixa o quadro BGRA `origem` (l×a) no quadro fixo, mantendo a proporção. Cada pixel de saída
/// é a média de 2×2 pixels da origem (suaviza a redução sem custar muito).
fn encaixar(origem: &[u8], l: usize, a: usize, destino: &mut [u8]) {
    destino.fill(0);
    if l == 0 || a == 0 || origem.len() < l * a * 4 {
        return;
    }
    let escala = (LARGURA as f32 / l as f32).min(ALTURA as f32 / a as f32);
    let (dl, da) = (((l as f32 * escala) as usize).clamp(1, LARGURA), ((a as f32 * escala) as usize).clamp(1, ALTURA));
    let (ox, oy) = ((LARGURA - dl) / 2, (ALTURA - da) / 2);
    let mapa_x: Vec<(usize, usize)> = (0..dl)
        .map(|x| {
            let sx = ((x as f32 + 0.5) / escala - 0.5).max(0.0) as usize;
            let sx = sx.min(l - 1);
            (sx * 4, (sx + 1).min(l - 1) * 4)
        })
        .collect();
    for y in 0..da {
        let sy = (((y as f32 + 0.5) / escala - 0.5).max(0.0) as usize).min(a - 1);
        let (l0, l1) = (sy * l * 4, (sy + 1).min(a - 1) * l * 4);
        let linha = &mut destino[((oy + y) * LARGURA + ox) * 4..((oy + y) * LARGURA + ox + dl) * 4];
        for (x, &(a0, a1)) in mapa_x.iter().enumerate() {
            for c in 0..3 {
                let soma = u16::from(origem[l0 + a0 + c]) + u16::from(origem[l0 + a1 + c]) + u16::from(origem[l1 + a0 + c]) + u16::from(origem[l1 + a1 + c]);
                linha[x * 4 + c] = (soma / 4) as u8;
            }
            linha[x * 4 + 3] = 255;
        }
    }
}

/// Recorte `(x0, y0, x1, y1)` da área interna da janela dentro do quadro capturado (`l`×`a`).
/// O quadro do Windows Graphics Capture cobre a moldura visível (`DWMWA_EXTENDED_FRAME_BOUNDS`);
/// a área interna começa onde o canto do cliente cai nela. `None`: usar o quadro inteiro.
fn area_interna(hwnd: isize, l: u32, a: u32) -> Option<(u32, u32, u32, u32)> {
    let hwnd = HWND(hwnd as *mut std::ffi::c_void);
    let mut moldura = RECT::default();
    let mut cliente = RECT::default();
    let mut origem = POINT::default();
    unsafe {
        DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, (&raw mut moldura).cast(), size_of::<RECT>() as u32).ok()?;
        GetClientRect(hwnd, &mut cliente).ok()?;
        if !ClientToScreen(hwnd, &mut origem).as_bool() {
            return None;
        }
    }
    recorte(moldura, origem, cliente.right, cliente.bottom, l, a)
}

fn recorte(moldura: RECT, origem: POINT, largura: i32, altura: i32, l: u32, a: u32) -> Option<(u32, u32, u32, u32)> {
    let (l, a) = (l as i32, a as i32);
    let x0 = (origem.x - moldura.left).clamp(0, l);
    let y0 = (origem.y - moldura.top).clamp(0, a);
    let x1 = (x0 + largura).min(l);
    let y1 = (y0 + altura).min(a);
    // Janela minimizada ou área vazia: melhor o quadro inteiro do que nada.
    if x1 - x0 < 16 || y1 - y0 < 16 || (x0, y0, x1, y1) == (0, 0, l, a) {
        return None;
    }
    Some((x0 as u32, y0 as u32, x1 as u32, y1 as u32))
}

/// Último quadro encaixado (compartilhado entre a captura e o escritor).
pub type Tela = Arc<Mutex<Vec<u8>>>;

struct Captura {
    tela: Tela,
    fechada: Arc<AtomicBool>,
    hwnd: isize,
    sem_padding: Vec<u8>,
    encaixado: Vec<u8>,
}

impl GraphicsCaptureApiHandler for Captura {
    type Flags = (Tela, Arc<AtomicBool>, isize);
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { tela: ctx.flags.0, fechada: ctx.flags.1, hwnd: ctx.flags.2, sem_padding: Vec::new(), encaixado: vec![0; BYTES] })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, _controle: InternalCaptureControl) -> Result<(), Self::Error> {
        let buffer = match area_interna(self.hwnd, frame.width(), frame.height()) {
            Some((x0, y0, x1, y1)) => frame.buffer_crop(x0, y0, x1, y1)?,
            None => frame.buffer()?,
        };
        let (l, a) = (buffer.width() as usize, buffer.height() as usize);
        let dados = buffer.as_nopadding_buffer(&mut self.sem_padding);
        encaixar(dados, l, a, &mut self.encaixado);
        let mut tela = self.tela.lock().unwrap_or_else(|p| p.into_inner());
        std::mem::swap(&mut *tela, &mut self.encaixado);
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.fechada.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Captura em andamento: para ao ser descartada.
pub struct CapturaJanela {
    pub tela: Tela,
    /// A janela do jogo fechou (o jogo saiu).
    pub fechada: Arc<AtomicBool>,
    /// Processo dono da janela (de onde vem o som da live).
    pub pid: u32,
    parar: Option<Box<dyn FnOnce() + Send>>,
}

impl Drop for CapturaJanela {
    fn drop(&mut self) {
        if let Some(parar) = self.parar.take() {
            parar();
        }
    }
}

/// Começa a capturar a janela para dentro de `tela` (a mesma que o FFmpeg já recebe: a live troca
/// a tela de espera pelo jogo sem parar). Sem borda amarela quando o Windows permite (Windows 11).
pub fn iniciar(janela: Window, tela: Tela) -> Result<CapturaJanela, String> {
    let fechada = Arc::new(AtomicBool::new(false));
    let pid = janela.process_id().unwrap_or(0);
    let hwnd = janela.as_raw_hwnd() as isize;
    let tentar = |borda: DrawBorderSettings| {
        Captura::start_free_threaded(Settings::new(
            janela,
            CursorCaptureSettings::WithoutCursor,
            borda,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            (tela.clone(), fechada.clone(), hwnd),
        ))
    };
    let controle = tentar(DrawBorderSettings::WithoutBorder)
        .or_else(|_| tentar(DrawBorderSettings::Default))
        .map_err(|e| format!("não foi possível capturar a janela do jogo: {e}"))?;
    Ok(CapturaJanela {
        tela,
        fechada,
        pid,
        parar: Some(Box::new(move || {
            let _ = controle.stop();
        })),
    })
}

/// Escreve o quadro atual no FFmpeg (BGRA 1280×720) a 60 quadros/s pelo relógio, repetindo o
/// último quando não chega um novo, até `parar` ou erro de escrita. Roda numa thread própria: o
/// `sleep` do Rust no Windows usa timer de alta resolução (o do tokio anda em degraus de ~15 ms,
/// o que fazia o vídeo engasgar).
pub fn alimentar(mut destino: std::fs::File, tela: Tela, parar: Arc<AtomicBool>) {
    let resultado = std::thread::Builder::new().name("live-video".into()).spawn(move || {
        let inicio = Instant::now();
        let fps = f64::from(QUADROS_POR_SEGUNDO);
        let mut escritos: u64 = 0;
        let mut quadro = vec![0u8; BYTES];
        while !parar.load(Ordering::SeqCst) {
            let alvo = inicio + Duration::from_secs_f64(escritos as f64 / fps);
            let agora = Instant::now();
            if alvo > agora {
                std::thread::sleep(alvo - agora);
            }
            let devidos = (inicio.elapsed().as_secs_f64() * fps) as u64 + 1;
            quadro.copy_from_slice(&tela.lock().unwrap_or_else(|p| p.into_inner()));
            // Um quadro por quadro devido, para o vídeo acompanhar o relógio (e o áudio). Um atraso
            // longo (o FFmpeg ainda abrindo, máquina travada) é pulado, não despejado de uma vez.
            let atraso = devidos.saturating_sub(escritos).max(1);
            for _ in 0..if atraso > 30 { 1 } else { atraso } {
                if destino.write_all(&quadro).is_err() {
                    return;
                }
            }
            escritos = devidos.max(escritos + 1);
        }
        let _ = destino.flush();
    });
    if let Err(e) = resultado {
        log::warn!("live: falha ao criar a thread de vídeo: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorta_a_barra_de_titulo() {
        // Moldura em (100,100); cliente começa 31 px abaixo (barra de título), 800×600.
        let moldura = RECT { left: 100, top: 100, right: 900, bottom: 731 };
        let origem = POINT { x: 100, y: 131 };
        assert_eq!(recorte(moldura, origem, 800, 600, 800, 631), Some((0, 31, 800, 631)));
        // Tela cheia: cliente = janela inteira.
        assert_eq!(recorte(moldura, POINT { x: 100, y: 100 }, 800, 631, 800, 631), None);
    }

    #[test]
    fn encaixa_sem_distorcer_com_barras() {
        // 4:3 (640×480) num 16:9: largura 960 centralizada, barras pretas nas laterais.
        let origem = vec![200u8; 640 * 480 * 4];
        let mut destino = vec![0u8; BYTES];
        encaixar(&origem, 640, 480, &mut destino);
        let pixel = |x: usize, y: usize| destino[(y * LARGURA + x) * 4];
        assert_eq!(pixel(10, 360), 0, "barra preta à esquerda");
        assert_eq!(pixel(640, 360), 200, "imagem no centro");
        assert_eq!(pixel(1270, 360), 0, "barra preta à direita");
    }
}

#[cfg(test)]
mod neste_pc {
    use super::*;

    /// Captura 1 s da maior janela aberta: `cargo test -- --ignored captura_janela --nocapture`.
    #[test]
    #[ignore = "depende de uma janela aberta"]
    fn captura_janela() {
        let janela = Window::enumerate()
            .unwrap()
            .into_iter()
            .filter(|w| w.title().is_ok_and(|t| t.contains("Visual Studio Code") || t.contains("Chrome") || t.contains("ShmupX")))
            .filter_map(|w| Some((i64::from(w.width().ok()?) * i64::from(w.height().ok()?), w)))
            .max_by_key(|(a, _)| *a)
            .unwrap()
            .1;
        println!("janela: {:?} {}x{}", janela.title(), janela.width().unwrap(), janela.height().unwrap());
        println!("área interna: {:?}", area_interna(janela.as_raw_hwnd() as isize, janela.width().unwrap() as u32, janela.height().unwrap() as u32));
        let captura = iniciar(janela, Arc::new(Mutex::new(vec![0; BYTES]))).unwrap();
        std::thread::sleep(Duration::from_millis(1500));
        let tela = captura.tela.lock().unwrap();
        let acesos = tela.chunks(4).filter(|p| p[0] > 10 || p[1] > 10 || p[2] > 10).count();
        println!("pixels com imagem: {acesos} de {}", LARGURA * ALTURA);
        assert!(acesos > 1000);
    }
}
