//! Som do jogo para a live: loopback WASAPI do dispositivo de saída padrão (o que sai nas caixas
//! ou no fone), sem microfone nem driver extra.
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
use tokio::process::ChildStdin;

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

/// Liga o loopback numa thread própria (o `cpal::Stream` não pode mudar de thread). Devolve a
/// taxa do dispositivo; o áudio sai sempre em estéreo `f32`.
pub fn iniciar() -> Result<Loopback, String> {
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
pub async fn alimentar(mut destino: ChildStdin, loopback: Option<Loopback>, parar: Arc<AtomicBool>) {
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
