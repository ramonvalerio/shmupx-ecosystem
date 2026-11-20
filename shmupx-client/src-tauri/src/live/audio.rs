//! Som do jogo para a live, sem microfone nem driver extra. Primeiro o loopback *do processo do
//! jogo* (Windows 10 2004+): só o som do jogo, qualquer que seja a saída — inclusive dispositivos
//! virtuais como o THX Spatial, cujo loopback de saída chega sempre mudo. Se não der, o loopback
//! WASAPI da saída padrão (tudo o que toca no PC).
//!
//! Lições do Client antigo (spec 033, D-03309/D-03310): o loopback não entrega nada enquanto o
//! jogo está em silêncio, e o FFmpeg trava o vídeo se o áudio parar de chegar. Por isso quem
//! escreve no FFmpeg é um relógio de parede: a cada 10 ms ele entrega exatamente as amostras
//! devidas até ali — as capturadas, completando com silêncio, ou descartando o excesso. Sem
//! loopback disponível, a live sai com uma trilha de silêncio (o YouTube exige áudio).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use tokio::io::AsyncWriteExt;

/// Taxa usada quando não há loopback (só silêncio).
pub const TAXA_PADRAO: u32 = 48_000;
/// Acima disto (em segundos) de áudio acumulado, o mais antigo é descartado para não atrasar.
const FOLGA_MAXIMA: f64 = 0.25;

pub struct Loopback {
    pub taxa: u32,
    amostras: Arc<Mutex<VecDeque<f32>>>,
    parar: Arc<AtomicBool>,
}

impl Drop for Loopback {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::SeqCst);
    }
}

fn travar<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Som do processo `pid` (e dos filhos); sem ele, o da saída padrão.
pub fn iniciar(pid: u32) -> Result<Loopback, String> {
    if pid != 0 {
        match processo::iniciar(pid) {
            Ok(l) => return Ok(l),
            Err(e) => log::warn!("live: sem o som do processo do jogo ({e}); usando a saída padrão"),
        }
    }
    iniciar_saida()
}

/// Liga o loopback numa thread própria (o `cpal::Stream` não pode mudar de thread). Devolve a
/// taxa do dispositivo; o áudio sai sempre em estéreo `f32`.
fn iniciar_saida() -> Result<Loopback, String> {
    let amostras = Arc::new(Mutex::new(VecDeque::<f32>::new()));
    let parar = Arc::new(AtomicBool::new(false));
    let (pronto_tx, pronto_rx) = std::sync::mpsc::channel::<Result<u32, String>>();
    let (destino, sinal) = (amostras.clone(), parar.clone());

    std::thread::Builder::new()
        .name("live-loopback".into())
        .spawn(move || {
            let abrir = || -> Result<(cpal::Stream, u32), String> {
                let dispositivo = cpal::default_host()
                    .default_output_device()
                    .ok_or("nenhum dispositivo de saída de áudio")?;
                let config = dispositivo.default_output_config().map_err(|e| format!("saída de áudio indisponível: {e}"))?;
                let canais = config.channels().max(1) as usize;
                let taxa = config.sample_rate().0;
                let formato = config.sample_format();
                let stream_config = config.config();
                let destino_f = destino.clone();
                let empurrar = move |quadros: &mut dyn Iterator<Item = (f32, f32)>| {
                    let mut buffer = travar(&destino_f);
                    for (l, r) in quadros {
                        buffer.push_back(l);
                        buffer.push_back(r);
                    }
                };
                let erro = |e| log::warn!("live: erro no loopback de áudio: {e}");
                // No WASAPI, abrir um stream de *entrada* no dispositivo de *saída* é o loopback.
                let stream = match formato {
                    SampleFormat::F32 => {
                        let empurrar = empurrar.clone();
                        dispositivo.build_input_stream(
                            &stream_config,
                            move |dados: &[f32], _| {
                                empurrar(&mut dados.chunks(canais).map(|q| (q[0], *q.get(1).unwrap_or(&q[0]))));
                            },
                            erro,
                            None,
                        )
                    }
                    SampleFormat::I16 => dispositivo.build_input_stream(
                        &stream_config,
                        move |dados: &[i16], _| {
                            let f = |s: i16| s as f32 / i16::MAX as f32;
                            empurrar(&mut dados.chunks(canais).map(|q| (f(q[0]), f(*q.get(1).unwrap_or(&q[0])))));
                        },
                        erro,
                        None,
                    ),
                    outro => return Err(format!("formato de áudio não suportado: {outro:?}")),
                }
                .map_err(|e| format!("não foi possível capturar o som do sistema: {e}"))?;
                stream.play().map_err(|e| format!("não foi possível iniciar o som do sistema: {e}"))?;
                Ok((stream, taxa))
            };
            match abrir() {
                Ok((stream, taxa)) => {
                    let _ = pronto_tx.send(Ok(taxa));
                    while !sinal.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    drop(stream);
                }
                Err(e) => {
                    let _ = pronto_tx.send(Err(e));
                }
            }
        })
        .map_err(|e| format!("falha ao criar a thread de áudio: {e}"))?;

    let taxa = pronto_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "o som do sistema não respondeu".to_string())??;
    Ok(Loopback { taxa, amostras, parar })
}

/// Escreve no FFmpeg o áudio no ritmo do relógio (estéreo `f32le`) até `parar` ou erro de escrita.
/// Sem `loopback`, escreve só silêncio.
pub async fn alimentar<W: tokio::io::AsyncWrite + Unpin>(mut destino: W, loopback: Option<Loopback>, parar: Arc<AtomicBool>) {
    let taxa = loopback.as_ref().map_or(TAXA_PADRAO, |l| l.taxa) as f64;
    let inicio = Instant::now();
    let mut escritos: u64 = 0;
    let mut tique = tokio::time::interval(Duration::from_millis(10));
    tique.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut bytes = Vec::<u8>::with_capacity(8192);
    while !parar.load(Ordering::SeqCst) {
        tique.tick().await;
        let devidos = (inicio.elapsed().as_secs_f64() * taxa) as u64;
        let faltam = devidos.saturating_sub(escritos) as usize;
        if faltam == 0 {
            continue;
        }
        bytes.clear();
        let mut entregues = 0usize;
        if let Some(lb) = &loopback {
            let mut buffer = travar(&lb.amostras);
            let excesso = buffer.len() / 2;
            let maximo = faltam + (FOLGA_MAXIMA * taxa) as usize;
            if excesso > maximo {
                buffer.drain(..(excesso - maximo) * 2);
            }
            while entregues < faltam && buffer.len() >= 2 {
                let (l, r) = (buffer.pop_front().unwrap_or(0.0), buffer.pop_front().unwrap_or(0.0));
                bytes.extend_from_slice(&l.to_le_bytes());
                bytes.extend_from_slice(&r.to_le_bytes());
                entregues += 1;
            }
        }
        for _ in entregues..faltam {
            bytes.extend_from_slice(&0f32.to_le_bytes());
            bytes.extend_from_slice(&0f32.to_le_bytes());
        }
        if destino.write_all(&bytes).await.is_err() {
            break;
        }
        escritos += faltam as u64;
    }
    let _ = destino.shutdown().await;
}

/// Loopback do processo (`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`).
mod processo {
    use std::mem::ManuallyDrop;
    use std::sync::mpsc;

    use windows::core::{implement, IUnknown, Interface, Ref, HRESULT, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, E_FAIL, HANDLE};
    use windows::Win32::Media::Audio::{
        ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
        IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient, AUDCLNT_BUFFERFLAGS_SILENT,
        AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
        AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, AUDIOCLIENT_ACTIVATION_PARAMS,
        AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
        PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX,
    };
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, BLOB, COINIT_MULTITHREADED};
    use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
    use windows::Win32::System::Variant::VT_BLOB;

    use super::*;

    const TAXA: u32 = 48_000;
    /// `WAVE_FORMAT_IEEE_FLOAT`.
    const FLOAT: u16 = 3;

    #[implement(IActivateAudioInterfaceCompletionHandler)]
    struct Ativado(mpsc::Sender<()>);

    impl IActivateAudioInterfaceCompletionHandler_Impl for Ativado_Impl {
        fn ActivateCompleted(&self, _op: Ref<IActivateAudioInterfaceAsyncOperation>) -> windows::core::Result<()> {
            let _ = self.0.send(());
            Ok(())
        }
    }

    struct Cliente {
        cliente: IAudioClient,
        captura: IAudioCaptureClient,
        evento: HANDLE,
    }

    unsafe fn abrir(pid: u32) -> windows::core::Result<Cliente> {
        unsafe {
            let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
                ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
                Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                    ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                        TargetProcessId: pid,
                        ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                    },
                },
            };
            // O blob aponta para `params` na pilha: nada de `PropVariantClear` nele.
            let mut prop = ManuallyDrop::new(PROPVARIANT::default());
            {
                let interno = &mut prop.Anonymous.Anonymous;
                interno.vt = VT_BLOB;
                interno.Anonymous.blob = BLOB {
                    cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
                    pBlobData: (&raw mut params).cast(),
                };
            }
            let (tx, rx) = mpsc::channel();
            let ao_ativar: IActivateAudioInterfaceCompletionHandler = Ativado(tx).into();
            let op = ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, &IAudioClient::IID, Some(&*prop), &ao_ativar)?;
            rx.recv_timeout(Duration::from_secs(5)).map_err(|_| windows::core::Error::from(E_FAIL))?;
            let mut resultado = HRESULT(0);
            let mut interface: Option<IUnknown> = None;
            op.GetActivateResult(&mut resultado, &mut interface)?;
            resultado.ok()?;
            let cliente: IAudioClient = interface.ok_or_else(|| windows::core::Error::from(E_FAIL))?.cast()?;
            let formato = WAVEFORMATEX {
                wFormatTag: FLOAT,
                nChannels: 2,
                nSamplesPerSec: TAXA,
                nAvgBytesPerSec: TAXA * 8,
                nBlockAlign: 8,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            cliente.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK
                    | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
                    | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                    | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                200_000, // 20 ms
                0,
                &formato,
                None,
            )?;
            let evento = CreateEventW(None, false, false, PCWSTR::null())?;
            cliente.SetEventHandle(evento)?;
            let captura: IAudioCaptureClient = cliente.GetService()?;
            cliente.Start()?;
            Ok(Cliente { cliente, captura, evento })
        }
    }

    /// Lê os pacotes até `parar` (o processo só entrega pacotes enquanto toca algo).
    unsafe fn ler(c: &Cliente, destino: &Mutex<VecDeque<f32>>, parar: &AtomicBool) -> windows::core::Result<()> {
        unsafe {
            while !parar.load(Ordering::SeqCst) {
                WaitForSingleObject(c.evento, 100);
                while c.captura.GetNextPacketSize()? > 0 {
                    let mut dados: *mut u8 = std::ptr::null_mut();
                    let (mut quadros, mut flags) = (0u32, 0u32);
                    c.captura.GetBuffer(&mut dados, &mut quadros, &mut flags, None, None)?;
                    {
                        let mut buffer = travar(destino);
                        if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 || dados.is_null() {
                            buffer.extend(std::iter::repeat_n(0.0, quadros as usize * 2));
                        } else {
                            buffer.extend(std::slice::from_raw_parts(dados.cast::<f32>(), quadros as usize * 2));
                        }
                    }
                    c.captura.ReleaseBuffer(quadros)?;
                }
            }
            Ok(())
        }
    }

    pub fn iniciar(pid: u32) -> Result<Loopback, String> {
        let amostras = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        let parar = Arc::new(AtomicBool::new(false));
        let (pronto_tx, pronto_rx) = mpsc::channel::<Result<(), String>>();
        let (destino, sinal) = (amostras.clone(), parar.clone());
        std::thread::Builder::new()
            .name("live-som-do-jogo".into())
            .spawn(move || unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                match abrir(pid) {
                    Ok(c) => {
                        let _ = pronto_tx.send(Ok(()));
                        if let Err(e) = ler(&c, &destino, &sinal) {
                            log::warn!("live: o som do jogo parou: {e}");
                        }
                        let _ = c.cliente.Stop();
                        let _ = CloseHandle(c.evento);
                    }
                    Err(e) => {
                        let _ = pronto_tx.send(Err(e.to_string()));
                    }
                }
                CoUninitialize();
            })
            .map_err(|e| format!("falha ao criar a thread de áudio: {e}"))?;
        pronto_rx.recv_timeout(Duration::from_secs(6)).map_err(|_| "o som do jogo não respondeu".to_string())??;
        Ok(Loopback { taxa: TAXA, amostras, parar })
    }
}

#[cfg(test)]
mod neste_pc {
    use super::*;

    /// Toca um tom neste processo e confere que o loopback do processo ouve:
    /// `cargo test -- --ignored som_do_processo --nocapture`.
    #[test]
    #[ignore = "depende de som neste PC"]
    fn som_do_processo() {
        let lb = processo::iniciar(std::process::id()).unwrap();
        let tom = std::thread::spawn(|| {
            use rodio::Source;
            let (_s, h) = rodio::OutputStream::try_default().unwrap();
            let sink = rodio::Sink::try_new(&h).unwrap();
            sink.append(rodio::source::SineWave::new(440.0).take_duration(Duration::from_secs(2)).amplify(0.3));
            sink.sleep_until_end();
        });
        std::thread::sleep(Duration::from_millis(1500));
        let b = travar(&lb.amostras);
        let pico = b.iter().fold(0f32, |m, s| m.max(s.abs()));
        println!("amostras {} pico {pico}", b.len());
        drop(b);
        tom.join().unwrap();
        assert!(pico > 0.1);
    }
}
