use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::net::{playback, robotx};
use crate::session::SessionState;

/// Tempo máximo de uma gravação de push-to-talk antes do corte forçado pelo Rust. Mitiga o
/// botão travado / gamepad desconectado durante o `Pressed` sem nunca receber o `Released`
/// correspondente (bug latente documentado em `input/gamepad.rs`, TASKS.md 4.6).
///
/// `pub(crate)` porque `input/gamepad.rs` reusa o mesmo valor para destravar seu próprio
/// estado `pressed` depois do mesmo intervalo — uma única fonte de verdade para os "15s" em
/// vez de duas constantes que podem divergir com o tempo.
pub(crate) const MAX_RECORDING_DURATION: Duration = Duration::from_secs(15);

/// Estado global da gravação: canal para sinalizar "pare" à thread ativa, se houver.
/// `None` = ocioso. Protegido por `Mutex` padrão (não `tokio::sync::Mutex`): só é travado por
/// operações síncronas e rápidas (checar/trocar um `Option`), nunca mantido através de um
/// `.await`.
#[derive(Clone, Default)]
pub struct RecordingState(Arc<Mutex<Option<mpsc::Sender<()>>>>);

#[derive(Debug, Clone, Serialize)]
struct VoiceReadyPayload {
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct VoiceErrorPayload {
    message: String,
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Abre o microfone padrão do sistema e começa a gravar em segundo plano. Chamada pelo
/// listener do gamepad (`input/gamepad.rs`) assim que o botão de push-to-talk é pressionado.
///
/// Se já houver uma gravação em andamento (ex.: sinal de `Pressed` duplicado), não abre uma
/// segunda stream — apenas ignora, silenciosamente.
pub async fn iniciar_gravacao_microfone(app: AppHandle, state: RecordingState) {
    {
        let mut guard = lock_or_recover(&state.0);
        if guard.is_some() {
            return;
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *guard = Some(stop_tx);
        drop(guard);

        // `cpal` + o canal de controle são bloqueantes por natureza; roda numa thread da pool
        // de blocking do Tokio em vez de travar uma worker thread assíncrona.
        tokio::task::spawn_blocking(move || record_until_stopped(app, state, stop_rx));
    }
}

/// Sinaliza o fim da gravação (`robotx://voice-end` do gamepad, ou qualquer outro encerramento
/// manual). Encerramento gracioso: a própria thread de gravação finaliza a stream, escreve o
/// `.wav` e emite o evento de "pronto para envio" — esta função só manda o sinal de parada.
pub fn parar_gravacao_microfone(state: &RecordingState) {
    if let Some(stop_tx) = lock_or_recover(&state.0).take() {
        let _ = stop_tx.send(());
    }
}

fn record_until_stopped(app: AppHandle, state: RecordingState, stop_rx: mpsc::Receiver<()>) {
    let samples: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));

    let (stream, sample_rate, channels) = match build_input_stream(samples.clone()) {
        Ok(built) => built,
        Err(message) => {
            log::warn!(
                "input: falha ao abrir o microfone ({message}); push-to-talk sem áudio nesta tentativa"
            );
            let _ = app.emit("robotx://voice-error", VoiceErrorPayload { message });
            *lock_or_recover(&state.0) = None;
            return;
        }
    };

    if let Err(err) = stream.play() {
        let message = format!("falha ao iniciar a stream de áudio: {err}");
        log::warn!("input: {message}");
        let _ = app.emit("robotx://voice-error", VoiceErrorPayload { message });
        *lock_or_recover(&state.0) = None;
        return;
    }

    // Aguarda o sinal de stop (voice-end) OU o timeout de segurança de 15s — o que vier
    // primeiro. `recv_timeout` retornando `Err` (timeout ou canal fechado) ainda cai aqui:
    // em ambos os casos, é hora de encerrar a captura.
    let _ = stop_rx.recv_timeout(MAX_RECORDING_DURATION);

    drop(stream); // para a captura (o callback do cpal deixa de ser chamado a partir daqui)
    *lock_or_recover(&state.0) = None;

    let captured = lock_or_recover(&samples).clone();
    if captured.is_empty() {
        log::warn!("input: gravação de voz terminou sem nenhuma amostra capturada");
        return;
    }

    let path = match save_wav(&app, &captured, sample_rate, channels) {
        Ok(path) => path,
        Err(message) => {
            log::warn!("input: falha ao salvar o áudio capturado ({message})");
            let _ = app.emit("robotx://voice-error", VoiceErrorPayload { message });
            return;
        }
    };

    let _ = app.emit(
        "robotx://voice-ready",
        VoiceReadyPayload {
            path: path.display().to_string(),
        },
    );

    enviar_e_reproduzir_resposta(app, path);
}

/// Encaminha o `.wav` para a Edge Function `robotx-voice` e reproduz a resposta falada. Roda
/// dentro da mesma thread bloqueante de `record_until_stopped` (via `Handle::current().block_on`,
/// seguro aqui porque esta thread já é dedicada — não é uma worker thread assíncrona do Tokio —
/// e foi criada justamente por `spawn_blocking`) em vez de disparar uma task separada: mantém o
/// fluxo linear (gravar → salvar → enviar → reproduzir) fácil de seguir.
fn enviar_e_reproduzir_resposta(app: AppHandle, wav_path: PathBuf) {
    let sessao = app.state::<SessionState>().snapshot();
    let (token, sala_id) = match (sessao.token, sessao.sala_id) {
        (Some(token), Some(sala_id)) => (token, sala_id),
        _ => {
            log::warn!(
                "input: sessão sem token/sala ativa; não é possível enviar o áudio ao RobotX"
            );
            let _ = app.emit(
                "robotx://voice-error",
                VoiceErrorPayload {
                    message: "sessão não configurada (token ou sala ativa ausentes)".to_string(),
                },
            );
            return;
        }
    };

    let resultado = tokio::runtime::Handle::current()
        .block_on(async { robotx::enviar_audio_ptt(&token, &sala_id, &wav_path).await });

    let audio_resposta = match resultado {
        Ok(bytes) => bytes,
        Err(message) => {
            log::warn!("input: falha ao enviar o áudio ao RobotX ({message})");
            let _ = app.emit("robotx://voice-error", VoiceErrorPayload { message });
            return;
        }
    };

    let _ = app.emit("robotx://voice-response-ready", ());

    match playback::reproduzir_audio(audio_resposta) {
        Ok(()) => {
            let _ = app.emit("robotx://voice-response-played", ());
        }
        Err(message) => {
            log::warn!("input: falha ao reproduzir a resposta do RobotX ({message})");
            let _ = app.emit("robotx://voice-error", VoiceErrorPayload { message });
        }
    }
}

/// Resolve o microfone padrão e abre uma stream de entrada, convertendo as amostras para PCM
/// 16-bit no buffer compartilhado (`samples`). Qualquer falha (sem microfone, sem permissão,
/// formato não suportado) vira `Err` em vez de panic — quem chama decide como reagir.
fn build_input_stream(samples: Arc<Mutex<Vec<i16>>>) -> Result<(cpal::Stream, u32, u16), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "nenhum microfone padrão encontrado no sistema".to_string())?;

    let supported_config = device
        .default_input_config()
        .map_err(|e| format!("não foi possível ler a configuração padrão do microfone: {e}"))?;

    let sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels();
    let stream_config = supported_config.config();

    let err_fn = |err| log::warn!("input: erro na stream de áudio: {err}");

    let stream = match supported_config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _| {
                let mut buffer = lock_or_recover(&samples);
                buffer.extend(
                    data.iter()
                        .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16),
                );
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _| {
                lock_or_recover(&samples).extend_from_slice(data);
            },
            err_fn,
            None,
        ),
        other => {
            return Err(format!(
                "formato de amostra de áudio não suportado: {other:?}"
            ));
        }
    }
    .map_err(|e| format!("falha ao abrir a stream de entrada de áudio: {e}"))?;

    Ok((stream, sample_rate, channels))
}

/// Grava o PCM capturado como `.wav` em `app_data_dir()/robotx-voice/`.
fn save_wav(
    app: &AppHandle,
    samples: &[i16],
    sample_rate: u32,
    channels: u16,
) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))?
        .join("robotx-voice");
    std::fs::create_dir_all(&data_dir).map_err(|e| {
        format!(
            "falha ao criar a pasta de áudio ({}): {e}",
            data_dir.display()
        )
    })?;

    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let output_path = data_dir.join(format!("ptt_{timestamp_ms}.wav"));

    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(&output_path, spec)
        .map_err(|e| format!("falha ao criar o arquivo wav: {e}"))?;

    for &sample in samples {
        writer
            .write_sample(sample)
            .map_err(|e| format!("falha ao escrever amostra no wav: {e}"))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("falha ao finalizar o wav: {e}"))?;

    Ok(output_path)
}
