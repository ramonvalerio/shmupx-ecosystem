use std::thread;
use std::time::{Duration, Instant};

use gilrs::{Button, EventType, Gilrs};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::capture::audio::{self, RecordingState};

/// Botão padrão de push-to-talk: "Select"/"View"/"Back" do controle (o nome varia por
/// fabricante, mas é o botão de menu secundário na maioria dos layouts Xbox/DirectInput).
///
/// TODO(TASKS.md 4.2): tornar configurável via `HotkeyBinding` persistido, com um fluxo de
/// "capturar próximo botão" na UI, em vez de fixar este botão para todo mundo.
const DEFAULT_PTT_BUTTON: Button = Button::Select;

/// Frequência de polling do estado dos controles. `gilrs` lê diretamente do XInput/DirectInput
/// do Windows, então isso funciona independente do foco da janela (controle funciona com o
/// jogo da Steam em primeiro plano e o Client minimizado).
const POLL_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Debug, Clone, Serialize)]
struct VoicePttPayload {
    button: String,
}

/// Inicia a escuta global de gamepads numa thread de SO dedicada (não uma task Tokio: a leitura
/// do `gilrs` é bloqueante/síncrona e precisa rodar continuamente em segundo plano). Chamar uma
/// vez no `setup()` do Tauri; a thread vive pelo tempo de vida do processo.
pub fn spawn_listener(app: AppHandle) {
    thread::spawn(move || run_listener(app));
}

fn run_listener(app: AppHandle) {
    let mut gilrs = match Gilrs::new() {
        Ok(instance) => instance,
        Err(err) => {
            // Nenhum controle conectado ainda não é motivo de erro aqui — `Gilrs::new()` só
            // falha quando o backend de input do SO não pôde ser inicializado. Nesse caso,
            // registramos e seguimos sem push-to-talk por controle em vez de derrubar o app.
            log::warn!(
                "input: não foi possível iniciar a escuta de gamepads ({err}); \
                 push-to-talk por controle desabilitado nesta sessão"
            );
            return;
        }
    };

    let mut pressed = false;
    let mut pressed_since: Option<Instant> = None;

    loop {
        while let Some(event) = gilrs.next_event() {
            match event.event {
                EventType::ButtonPressed(button, _) if button == DEFAULT_PTT_BUTTON && !pressed => {
                    pressed = true;
                    pressed_since = Some(Instant::now());
                    let _ = app.emit(
                        "robotx://voice-start",
                        VoicePttPayload { button: format!("{button:?}") },
                    );

                    // Dispara a captura de microfone em segundo plano (capture/audio.rs). Usa o
                    // runtime assíncrono do próprio Tauri: esta thread não é async, então não dá
                    // para `.await` a função aqui — `async_runtime::spawn` agenda a task sem
                    // precisar que o chamador já esteja dentro de um contexto Tokio.
                    let app_for_audio = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_for_audio.state::<RecordingState>().inner().clone();
                        audio::iniciar_gravacao_microfone(app_for_audio.clone(), state).await;
                    });
                }
                EventType::ButtonReleased(button, _) if button == DEFAULT_PTT_BUTTON && pressed => {
                    pressed = false;
                    pressed_since = None;
                    let _ = app.emit(
                        "robotx://voice-end",
                        VoicePttPayload { button: format!("{button:?}") },
                    );

                    audio::parar_gravacao_microfone(app.state::<RecordingState>().inner());
                }
                _ => {}
            }
        }

        // Destrava o estado `pressed` se o `Released` nunca chegar (gamepad desconectado com o
        // botão fisicamente preso, por exemplo). A gravação em si já é cortada de forma
        // independente pelo timeout em `capture::audio::MAX_RECORDING_DURATION`; isto aqui só
        // garante que um novo `Pressed` real volte a ser aceito depois do mesmo intervalo, em
        // vez do push-to-talk ficar morto pelo resto da sessão.
        if pressed {
            if let Some(since) = pressed_since {
                if since.elapsed() >= audio::MAX_RECORDING_DURATION {
                    log::warn!(
                        "input: nenhum ButtonReleased em {:?}; destravando o estado de push-to-talk \
                         (gamepad desconectado ou botão fisicamente preso?)",
                        audio::MAX_RECORDING_DURATION
                    );
                    pressed = false;
                    pressed_since = None;
                }
            }
        }

        thread::sleep(POLL_INTERVAL);
    }
}
