//! Captura da janela do jogo para a gravação da run, com o trabalho pesado na placa de vídeo.
//!
//! Um passo do processador de vídeo do Direct3D 11 faz o recorte da área interna (sem a barra de
//! título e as bordas), a redução para caber em 1280×720 (sem distorcer, com barras pretas) e a
//! conversão para NV12, o formato que o x264 codifica. A CPU só lê o quadro pronto (1,4 MB, contra
//! 8 MB do quadro inteiro em BGRA reduzido em Rust) e o FFmpeg codifica direto, sem converter. A
//! leitura alterna entre duas texturas: lê a do quadro anterior enquanto a GPU prepara a do atual,
//! sem esperar a placa.
//!
//! Sem o processador de vídeo (driver antigo, área de trabalho remota), cai para a CPU: o encaixe
//! da live (`live::janela::encaixar`) e a conversão para NV12 em Rust.

use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windows::core::Interface;
use windows::Win32::Foundation::{E_FAIL, RECT};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_NV12, DXGI_RATIONAL, DXGI_SAMPLE_DESC};
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

use crate::live::janela;

pub const LARGURA: u32 = janela::LARGURA as u32;
pub const ALTURA: u32 = janela::ALTURA as u32;
/// Bytes de um quadro NV12: o plano Y inteiro e o UV pela metade.
pub const BYTES: usize = (LARGURA * ALTURA * 3 / 2) as usize;

/// Quadro NV12 preto (Y 16, Cb e Cr 128): o vídeo antes do primeiro quadro do jogo.
pub fn quadro_preto() -> Vec<u8> {
    let mut quadro = vec![16u8; (LARGURA * ALTURA) as usize];
    quadro.resize(BYTES, 128);
    quadro
}

/// Onde o jogo (`l`×`a`) fica no quadro de saída: o maior retângulo com a proporção dele, centrado,
/// em coordenadas pares (o NV12 guarda a cor a cada 2×2 pixels).
fn destino(l: u32, a: u32) -> RECT {
    let escala = (f64::from(LARGURA) / f64::from(l)).min(f64::from(ALTURA) / f64::from(a));
    let par = |v: f64, max: u32| ((v.round() as u32) & !1).clamp(2, max);
    let (dl, da) = (par(f64::from(l) * escala, LARGURA), par(f64::from(a) * escala, ALTURA));
    let (x, y) = (((LARGURA - dl) / 2) & !1, ((ALTURA - da) / 2) & !1);
    RECT { left: x as i32, top: y as i32, right: (x + dl) as i32, bottom: (y + da) as i32 }
}

fn textura(device: &ID3D11Device, l: u32, a: u32, formato: DXGI_FORMAT, uso: D3D11_USAGE, ligacao: u32, cpu: u32) -> windows::core::Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: l,
        Height: a,
        MipLevels: 1,
        ArraySize: 1,
        Format: formato,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: uso,
        BindFlags: ligacao,
        CPUAccessFlags: cpu,
        MiscFlags: 0,
    };
    let mut t = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut t))? };
    t.ok_or_else(|| E_FAIL.into())
}

/// Processador de vídeo montado para um tamanho de recorte e um formato de quadro.
struct Processador {
    device: ID3D11Device,
    video_context: ID3D11VideoContext,
    processador: ID3D11VideoProcessor,
    /// Recorte da janela (área interna), copiado do quadro capturado.
    entrada: ID3D11Texture2D,
    vista_entrada: ID3D11VideoProcessorInputView,
    saida: ID3D11Texture2D,
    vista_saida: ID3D11VideoProcessorOutputView,
    /// Cópias legíveis pela CPU, alternadas a cada quadro.
    leitura: [ID3D11Texture2D; 2],
    tamanho: (u32, u32),
    formato: DXGI_FORMAT,
    quadros: u64,
}

impl Processador {
    fn novo(device: &ID3D11Device, contexto: &ID3D11DeviceContext, formato: DXGI_FORMAT, l: u32, a: u32) -> windows::core::Result<Self> {
        let video_device: ID3D11VideoDevice = device.cast()?;
        let video_context: ID3D11VideoContext = contexto.cast()?;
        let taxa = DXGI_RATIONAL { Numerator: 60, Denominator: 1 };
        let conteudo = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: taxa,
            InputWidth: l,
            InputHeight: a,
            OutputFrameRate: taxa,
            OutputWidth: LARGURA,
            OutputHeight: ALTURA,
            Usage: D3D11_VIDEO_USAGE_OPTIMAL_SPEED,
        };
        let enumerador = unsafe { video_device.CreateVideoProcessorEnumerator(&conteudo)? };
        let processador = unsafe { video_device.CreateVideoProcessor(&enumerador, 0)? };

        let alvo = (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32;
        let entrada = textura(device, l, a, formato, D3D11_USAGE_DEFAULT, alvo, 0)?;
        let saida = textura(device, LARGURA, ALTURA, DXGI_FORMAT_NV12, D3D11_USAGE_DEFAULT, D3D11_BIND_RENDER_TARGET.0 as u32, 0)?;
        let leitura = [
            textura(device, LARGURA, ALTURA, DXGI_FORMAT_NV12, D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ.0 as u32)?,
            textura(device, LARGURA, ALTURA, DXGI_FORMAT_NV12, D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ.0 as u32)?,
        ];

        let desc_entrada = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPIV { MipSlice: 0, ArraySlice: 0 } },
        };
        let mut vista_entrada = None;
        unsafe { video_device.CreateVideoProcessorInputView(&entrada, &enumerador, &desc_entrada, Some(&mut vista_entrada))? };
        let desc_saida = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
            ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 } },
        };
        let mut vista_saida = None;
        unsafe { video_device.CreateVideoProcessorOutputView(&saida, &enumerador, &desc_saida, Some(&mut vista_saida))? };

        unsafe {
            video_context.VideoProcessorSetStreamFrameFormat(&processador, 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
            // Sem "melhorias" automáticas do driver (nitidez, cor): o placar tem que sair como está.
            video_context.VideoProcessorSetStreamAutoProcessingMode(&processador, 0, false);
            video_context.VideoProcessorSetStreamSourceRect(&processador, 0, true, Some(&RECT { left: 0, top: 0, right: l as i32, bottom: a as i32 }));
            video_context.VideoProcessorSetStreamDestRect(&processador, 0, true, Some(&destino(l, a)));
            video_context.VideoProcessorSetOutputTargetRect(&processador, true, Some(&RECT { left: 0, top: 0, right: LARGURA as i32, bottom: ALTURA as i32 }));
            let preto = D3D11_VIDEO_COLOR { Anonymous: D3D11_VIDEO_COLOR_0 { YCbCr: D3D11_VIDEO_COLOR_YCbCrA { Y: 16.0 / 255.0, Cb: 0.5, Cr: 0.5, A: 1.0 } } };
            video_context.VideoProcessorSetOutputBackgroundColor(&processador, true, &preto);
            // Entrada: RGB 0–255. Saída: YCbCr BT.601 16–235, o mesmo que a conversão do FFmpeg fazia.
            video_context.VideoProcessorSetStreamColorSpace(&processador, 0, &D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: 0 });
            video_context.VideoProcessorSetOutputColorSpace(&processador, &D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: 1 << 4 });
        }
        Ok(Self {
            device: device.clone(),
            video_context,
            processador,
            entrada,
            vista_entrada: vista_entrada.ok_or(windows::core::Error::from(E_FAIL))?,
            saida,
            vista_saida: vista_saida.ok_or(windows::core::Error::from(E_FAIL))?,
            leitura,
            tamanho: (l, a),
            formato,
            quadros: 0,
        })
    }

    /// Processa o recorte `(x0, y0, x1, y1)` de `quadro` e, a partir do segundo quadro, escreve em
    /// `destino` o NV12 do quadro anterior (já pronto na GPU). `false`: ainda não há quadro pronto.
    fn processar(&mut self, contexto: &ID3D11DeviceContext, quadro: &ID3D11Texture2D, recorte: (u32, u32, u32, u32), destino: &mut [u8]) -> windows::core::Result<bool> {
        let (x0, y0, x1, y1) = recorte;
        let caixa = D3D11_BOX { left: x0, top: y0, front: 0, right: x1, bottom: y1, back: 1 };
        unsafe { contexto.CopySubresourceRegion(&self.entrada, 0, 0, 0, 0, quadro, 0, Some(&caixa)) };
        let mut fluxo = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            pInputSurface: ManuallyDrop::new(Some(self.vista_entrada.clone())),
            ..Default::default()
        };
        let resultado = unsafe { self.video_context.VideoProcessorBlt(&self.processador, &self.vista_saida, 0, std::slice::from_ref(&fluxo)) };
        unsafe { ManuallyDrop::drop(&mut fluxo.pInputSurface) };
        resultado?;

        let atual = (self.quadros % 2) as usize;
        unsafe { contexto.CopyResource(&self.leitura[atual], &self.saida) };
        self.quadros += 1;
        if self.quadros < 2 {
            return Ok(false);
        }
        let anterior = &self.leitura[1 - atual];
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe { contexto.Map(anterior, 0, D3D11_MAP_READ, 0, Some(&mut mapa))? };
        let (linha, l, a) = (mapa.RowPitch as usize, LARGURA as usize, ALTURA as usize);
        let base = mapa.pData as *const u8;
        // No NV12 legível pela CPU, o plano UV (a = altura/2 linhas) vem logo depois do Y.
        for y in 0..a + a / 2 {
            let origem = unsafe { std::slice::from_raw_parts(base.add(y * linha), l) };
            destino[y * l..(y + 1) * l].copy_from_slice(origem);
        }
        unsafe { contexto.Unmap(anterior, 0) };
        Ok(true)
    }
}

/// BGRA 1280×720 (já encaixado) para NV12, BT.601 16–235 (reserva sem o processador de vídeo).
fn bgra_para_nv12(bgra: &[u8], nv12: &mut [u8]) {
    let (l, a) = (LARGURA as usize, ALTURA as usize);
    let (plano_y, plano_uv) = nv12.split_at_mut(l * a);
    for y in (0..a).step_by(2) {
        for x in (0..l).step_by(2) {
            let (mut soma_u, mut soma_v) = (0i32, 0i32);
            for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let p = ((y + dy) * l + x + dx) * 4;
                let (b, g, r) = (i32::from(bgra[p]), i32::from(bgra[p + 1]), i32::from(bgra[p + 2]));
                plano_y[(y + dy) * l + x + dx] = (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16) as u8;
                soma_u += ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
                soma_v += ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
            }
            let i = (y / 2) * l + x;
            plano_uv[i] = (soma_u / 4) as u8;
            plano_uv[i + 1] = (soma_v / 4) as u8;
        }
    }
}

struct Captura {
    tela: janela::Tela,
    fechada: Arc<AtomicBool>,
    hwnd: isize,
    processador: Option<Processador>,
    /// O processador de vídeo falhou neste PC: o resto da gravação vai pela CPU.
    sem_gpu: bool,
    quadro: Vec<u8>,
    sem_padding: Vec<u8>,
    encaixado: Vec<u8>,
}

impl Captura {
    fn pela_gpu(&mut self, frame: &Frame) -> windows::core::Result<bool> {
        let (l, a) = (frame.width(), frame.height());
        let recorte = janela::area_interna(self.hwnd, l, a).unwrap_or((0, 0, l, a));
        let tamanho = (recorte.2 - recorte.0, recorte.3 - recorte.1);
        let formato = frame.desc().Format;
        let serve = self.processador.as_ref().is_some_and(|p| p.tamanho == tamanho && p.formato == formato && &p.device == frame.device());
        if !serve {
            self.processador = None;
            self.processador = Some(Processador::novo(frame.device(), frame.device_context(), formato, tamanho.0, tamanho.1)?);
        }
        let processador = self.processador.as_mut().ok_or(windows::core::Error::from(E_FAIL))?;
        processador.processar(frame.device_context(), frame.as_raw_texture(), recorte, &mut self.quadro)
    }

    fn pela_cpu(&mut self, frame: &mut Frame) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let buffer = match janela::area_interna(self.hwnd, frame.width(), frame.height()) {
            Some((x0, y0, x1, y1)) => frame.buffer_crop(x0, y0, x1, y1)?,
            None => frame.buffer()?,
        };
        let (l, a) = (buffer.width() as usize, buffer.height() as usize);
        let dados = buffer.as_nopadding_buffer(&mut self.sem_padding);
        janela::encaixar(dados, l, a, &mut self.encaixado);
        bgra_para_nv12(&self.encaixado, &mut self.quadro);
        Ok(())
    }
}

impl GraphicsCaptureApiHandler for Captura {
    type Flags = (janela::Tela, Arc<AtomicBool>, isize);
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            tela: ctx.flags.0,
            fechada: ctx.flags.1,
            hwnd: ctx.flags.2,
            processador: None,
            sem_gpu: false,
            quadro: quadro_preto(),
            sem_padding: Vec::new(),
            encaixado: vec![0; janela::BYTES],
        })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, _controle: InternalCaptureControl) -> Result<(), Self::Error> {
        if !self.sem_gpu {
            match self.pela_gpu(frame) {
                Ok(false) => return Ok(()),
                Ok(true) => {}
                Err(e) => {
                    log::warn!("gravação: sem o processador de vídeo da placa ({e}); reduzindo pela CPU");
                    self.sem_gpu = true;
                    self.processador = None;
                }
            }
        }
        if self.sem_gpu {
            self.pela_cpu(frame)?;
        }
        let mut tela = self.tela.lock().unwrap_or_else(|p| p.into_inner());
        std::mem::swap(&mut *tela, &mut self.quadro);
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.fechada.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Captura em andamento: para ao ser descartada.
pub struct CapturaJanela {
    /// Processo dono da janela (de onde vem o som da gravação).
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

/// Começa a capturar a janela para dentro de `tela` (quadros NV12 1280×720, [`BYTES`] cada). Sem
/// borda amarela quando o Windows permite (Windows 11).
pub fn iniciar(janela: Window, tela: janela::Tela) -> Result<CapturaJanela, String> {
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
        pid,
        parar: Some(Box::new(move || {
            let _ = controle.stop();
        })),
    })
}

/// Tela inicial da gravação (preta), compartilhada entre a captura e o escritor.
pub fn tela_inicial() -> janela::Tela {
    Arc::new(Mutex::new(quadro_preto()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jogo_fica_centrado_sem_distorcer() {
        // 16:9 ocupa o quadro todo; 4:3 (640×480) fica com barras laterais; tudo em coordenadas pares.
        let cheio = destino(1920, 1080);
        assert_eq!((cheio.left, cheio.top, cheio.right, cheio.bottom), (0, 0, 1280, 720));
        let r = destino(640, 480);
        assert_eq!((r.left, r.top, r.right, r.bottom), (160, 0, 1120, 720));
        let estranho = destino(1917, 1001);
        assert!(estranho.left % 2 == 0 && estranho.top % 2 == 0 && estranho.right % 2 == 0 && estranho.bottom % 2 == 0);
    }

    #[test]
    fn branco_e_preto_viram_nv12_de_estudio() {
        let mut nv12 = vec![0u8; BYTES];
        bgra_para_nv12(&vec![255u8; janela::BYTES], &mut nv12);
        assert_eq!((nv12[0], nv12[BYTES - 1]), (235, 128));
        bgra_para_nv12(&vec![0u8; janela::BYTES], &mut nv12);
        assert_eq!((nv12[0], nv12[(LARGURA * ALTURA) as usize]), (16, 128));
        assert_eq!(quadro_preto().len(), BYTES);
    }
}
