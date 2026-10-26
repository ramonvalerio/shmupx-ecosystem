use std::io::Cursor;

use rodio::{Decoder, OutputStream, Sink};

/// Decodifica e reproduz um áudio (ex.: a resposta `audio/mpeg` do RobotX) no dispositivo de
/// saída padrão do sistema. Bloqueante até o áudio terminar — quem chama deve rodar isto numa
/// thread dedicada (`tokio::task::spawn_blocking`), nunca direto numa task assíncrona.
///
/// Falhas (sem dispositivo de saída, áudio corrompido/formato não suportado) viram `Err` em vez
/// de panic — mesma política defensiva do resto do módulo de áudio (`capture/audio.rs`).
pub fn reproduzir_audio(bytes: Vec<u8>) -> Result<(), String> {
    let (_stream, handle) = OutputStream::try_default()
        .map_err(|e| format!("falha ao abrir o dispositivo de áudio de saída: {e}"))?;

    let sink =
        Sink::try_new(&handle).map_err(|e| format!("falha ao criar a fila de reprodução: {e}"))?;

    let fonte = Decoder::new(Cursor::new(bytes))
        .map_err(|e| format!("falha ao decodificar o áudio de resposta: {e}"))?;

    sink.append(fonte);
    sink.sleep_until_end();

    Ok(())
}
