use std::path::Path;
use std::time::Duration;

/// Edge Function `robotx-voice` servida localmente pelo `supabase start` (Supabase CLI expõe
/// Edge Functions em `:54321/functions/v1/<nome>` por padrão). Sem variável de ambiente ainda.
const ROBOTX_VOICE_URL: &str = "http://localhost:54321/functions/v1/robotx-voice";

/// Mais generoso que o timeout do backend: o pipeline do lado de lá é Whisper + embedding +
/// busca vetorial + chat completion + TTS, em série — tudo isso pode legitimamente passar de
/// alguns segundos mesmo sem nada estar "travado".
const TIMEOUT: Duration = Duration::from_secs(30);

/// Envia o `.wav` gravado (push-to-talk) para a Edge Function do RobotX e devolve os bytes do
/// áudio de resposta (`audio/mpeg`) prontos para reprodução.
///
/// `sala_id` vem de `session::SessionState` (definido pelo frontend via `definir_sala_ativa`);
/// sem ele a Edge Function não tem como resolver o `jogo_id` no servidor e rejeita a chamada
/// (ADR 003 — isolamento de RAG por sala).
pub async fn enviar_audio_ptt(token: &str, sala_id: &str, wav_path: &Path) -> Result<Vec<u8>, String> {
    let bytes = tokio::fs::read(wav_path)
        .await
        .map_err(|e| format!("falha ao ler o áudio gravado ({}): {e}", wav_path.display()))?;

    let nome_arquivo = wav_path
        .file_name()
        .and_then(|nome| nome.to_str())
        .unwrap_or("audio.wav")
        .to_string();

    let parte_audio = reqwest::multipart::Part::bytes(bytes)
        .file_name(nome_arquivo)
        .mime_str("audio/wav")
        .map_err(|e| format!("falha ao montar a parte multipart do áudio: {e}"))?;

    let form = reqwest::multipart::Form::new()
        .part("audio", parte_audio)
        .text("sala_id", sala_id.to_string());

    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("falha ao construir cliente HTTP: {e}"))?;

    let response = client
        .post(ROBOTX_VOICE_URL)
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("falha de rede ao enviar o áudio ao RobotX: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let corpo = response.text().await.unwrap_or_default();
        return Err(format!("RobotX recusou o áudio (HTTP {status}): {corpo}"));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|e| format!("falha ao ler o áudio de resposta do RobotX: {e}"))
}
