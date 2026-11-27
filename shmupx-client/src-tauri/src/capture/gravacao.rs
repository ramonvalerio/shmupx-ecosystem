//! Gravação da run nos jogos validados pelo vídeo. Os validados pela memória não gravam: o placar
//! vem do ShmupX Emulator, e gravar só gastaria CPU ao lado do jogo.
//!
//! No Windows grava só a janela do jogo: a área interna, sem barra de título nem bordas, encaixada
//! em 1280×720 a 60 quadros/s, com o som do processo do jogo (`live::audio`). A captura é da janela
//! (Windows Graphics Capture), não da tela: segue a janela se ela mudar de tamanho ou for para tela
//! cheia, e não grava o que estiver por cima dela. O recorte, a redução e a conversão de cor são
//! feitos na placa de vídeo (`janela_nv12`).
//!
//! O FFmpeg recebe o vídeo NV12 pelo stdin e o áudio por TCP local, codifica na placa de vídeo
//! quando dá (`ffmpeg::encoder_h264`) e roda abaixo do normal: se faltar CPU, quem perde é o
//! vídeo, nunca o jogo. No Linux (ainda sem captura de janela), a tela
//! inteira pelo `x11grab`, sem som.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::process::{Child, Command};

use super::ffmpeg;

/// Tempo que o FFmpeg precisa sobreviver para a gravação contar como iniciada.
const CONFIRMACAO_INICIO: Duration = Duration::from_millis(1500);

/// Gravação em andamento.
pub struct Gravacao {
    child: Child,
    /// Para os escritores de vídeo e áudio: o FFmpeg recebe o fim das entradas e fecha o arquivo.
    parar: Arc<AtomicBool>,
    /// Captura da janela e som do jogo: param ao serem descartados.
    _fontes: Vec<Box<dyn std::any::Any + Send>>,
    /// O FFmpeg lê comandos do stdin (`q` para parar) em vez de vídeo.
    para_com_q: bool,
    /// O vídeo tem o som do jogo.
    pub com_som: bool,
}

/// Log do FFmpeg ao lado do vídeo (`run.mp4` -> `run.ffmpeg.log`), para diagnosticar falhas.
pub fn caminho_do_log(video: &Path) -> PathBuf {
    video.with_extension("ffmpeg.log")
}

impl Gravacao {
    /// PID do FFmpeg.
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Encerra fechando o vídeo direito (o arquivo fica completo); se o FFmpeg não sair a tempo,
    /// força. O MP4 é fragmentado: mesmo forçado, o que foi gravado continua legível.
    pub async fn parar(mut self) -> io::Result<()> {
        if self.para_com_q {
            return ffmpeg::graceful_stop(&mut self.child).await;
        }
        self.parar.store(true, Ordering::SeqCst);
        self._fontes.clear();
        match tokio::time::timeout(Duration::from_secs(10), self.child.wait()).await {
            Ok(saida) => saida.map(|_| ()),
            Err(_) => self.child.kill().await,
        }
    }

    /// Corta na hora (limite de tempo da ficha): só o que foi gravado até aqui vale.
    pub async fn cortar(mut self) -> io::Result<()> {
        self.parar.store(true, Ordering::SeqCst);
        self.child.kill().await
    }
}

/// Confirma que a gravação começou: o FFmpeg aborta logo no início quando algo está errado
/// (codec, disco, permissão). Se ele já saiu, devolve a última linha do log como motivo.
async fn confirmar_inicio(child: &mut Child, log: &Path) -> Result<(), String> {
    tokio::time::sleep(CONFIRMACAO_INICIO).await;
    if child.try_wait().map_err(|e| e.to_string())?.is_none() {
        return Ok(());
    }
    let texto = std::fs::read_to_string(log).unwrap_or_default();
    Err(format!("a gravação não iniciou: {}", ffmpeg::ultima_linha(&texto)))
}

#[cfg(windows)]
pub use janela_do_jogo::{esperar_janela, iniciar};

/// Grava o jogo recém-aberto (`jogo_pid`; na Steam, também os processos da pasta dele): espera a
/// janela (até 1 min no ShmupX Emulator; até 3 min na Steam, que pode estar abrindo ou atualizando)
/// e grava só ela, com o som.
#[cfg(windows)]
pub async fn iniciar_do_jogo(video: &Path, jogo_pid: Option<u32>, steam_app_id: Option<u32>) -> Result<Gravacao, String> {
    ffmpeg::preparar_encoder_h264();
    // Na Steam, quem abriu pode ter sido um launcher: vale também qualquer processo da pasta do jogo.
    let pasta = match steam_app_id {
        Some(app_id) => tauri::async_runtime::spawn_blocking(move || crate::steam::biblioteca::pasta_do_jogo(app_id)).await.ok().flatten(),
        None => None,
    };
    let pids = move || {
        let mut pids: Vec<u32> = jogo_pid.into_iter().filter(|pid| crate::steam::detector::processo_vivo(*pid)).collect();
        if let Some(pasta) = &pasta {
            pids.extend(crate::steam::detector::processos_na_pasta(pasta));
        }
        pids
    };
    let prazo = std::time::Duration::from_secs(if steam_app_id.is_some() { 180 } else { 60 });
    let janela = esperar_janela(pids, prazo).await?;
    iniciar(video, janela).await
}

#[cfg(not(windows))]
pub async fn iniciar_do_jogo(video: &Path, _jogo_pid: Option<u32>, _steam_app_id: Option<u32>) -> Result<Gravacao, String> {
    iniciar_tela_inteira(video).await
}

#[cfg(windows)]
mod janela_do_jogo {
    use super::*;
    use crate::capture::janela_nv12;
    use crate::live::{audio, janela};

    /// Sem nenhum processo do jogo vivo por este tempo, ele fechou antes de a gravação começar (a
    /// folga cobre um launcher da Steam que fecha antes de o jogo abrir).
    const SEM_PROCESSO: Duration = Duration::from_secs(15);

    /// Espera a janela do jogo recém-aberto (`pids`: os processos vivos dele, consultados de novo a
    /// cada volta, porque um launcher pode trocar de processo). Ao achar, dá um instante para a
    /// janela assentar (o motor ainda pode estar indo para tela cheia) e pega a definitiva.
    pub async fn esperar_janela<F>(pids: F, prazo: Duration) -> Result<janela::Window, String>
    where
        F: Fn() -> Vec<u32> + Clone + Send + 'static,
    {
        let procurar = move || {
            let pids = pids.clone();
            async move { tauri::async_runtime::spawn_blocking(move || { let pids = pids(); (pids.is_empty(), janela::janela_dos_processos(&pids)) }).await.unwrap_or((false, None)) }
        };
        let inicio = tokio::time::Instant::now();
        let mut sem_processo_desde: Option<tokio::time::Instant> = None;
        while inicio.elapsed() < prazo {
            match procurar().await {
                (_, Some(achada)) => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    return Ok(procurar().await.1.unwrap_or(achada));
                }
                (true, None) => {
                    let desde = *sem_processo_desde.get_or_insert_with(tokio::time::Instant::now);
                    if desde.elapsed() >= SEM_PROCESSO {
                        return Err("o jogo fechou antes de a gravação começar".to_string());
                    }
                }
                (false, None) => sem_processo_desde = None,
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Err("a janela do jogo não apareceu a tempo de começar a gravação".to_string())
    }

    /// Começa a gravar a janela do jogo, com o som do processo dono dela, em `video`.
    pub async fn iniciar(video: &Path, janela_do_jogo: janela::Window) -> Result<Gravacao, String> {
        let log = caminho_do_log(video);
        let encoder = ffmpeg::encoder_h264_async().await;
        let tela = janela_nv12::tela_inicial();
        let captura = janela_nv12::iniciar(janela_do_jogo, tela.clone())?;
        let pid_do_jogo = captura.pid;
        let fonte: audio::Fonte = Arc::default();
        let com_som = match tauri::async_runtime::spawn_blocking(move || audio::iniciar(pid_do_jogo)).await.map_err(|e| e.to_string())? {
            Ok(loopback) => {
                *fonte.lock().unwrap_or_else(|p| p.into_inner()) = Some(loopback);
                true
            }
            Err(e) => {
                log::warn!("gravação: sem o som do jogo ({e}); o vídeo sai com silêncio");
                false
            }
        };

        let porta = crate::live::porta_livre()?;
        let mut child = spawn(video, porta, encoder, &log).map_err(|e| format!("falha ao iniciar o FFmpeg: {e}"))?;
        let stdin = child.stdin.take().ok_or("FFmpeg sem entrada de vídeo")?;
        let stdin = std::fs::File::from(stdin.into_owned_handle().map_err(|e| format!("FFmpeg sem entrada de vídeo: {e}"))?);
        let parar = Arc::new(AtomicBool::new(false));
        janela::alimentar(stdin, tela, parar.clone());
        // O FFmpeg abre a entrada de áudio depois de ler o vídeo: tenta conectar por até 10 s.
        let mut conexao = None;
        for _ in 0..100 {
            if let Ok(c) = tokio::net::TcpStream::connect(("127.0.0.1", porta)).await {
                conexao = Some(c);
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let Some(conexao) = conexao else {
            parar.store(true, Ordering::SeqCst);
            let _ = child.kill().await;
            let texto = std::fs::read_to_string(&log).unwrap_or_default();
            return Err(format!("a gravação não iniciou: {}", ffmpeg::ultima_linha(&texto)));
        };
        tokio::spawn(audio::alimentar(conexao, fonte.clone(), parar.clone()));
        if let Err(e) = confirmar_inicio(&mut child, &log).await {
            parar.store(true, Ordering::SeqCst);
            return Err(e);
        }
        Ok(Gravacao { child, parar, _fontes: vec![Box::new(captura), Box::new(fonte)], para_com_q: false, com_som })
    }

    /// FFmpeg da gravação: vídeo NV12 1280×720 a 60 quadros/s pelo stdin (entrada 0) e áudio
    /// `f32le` estéreo por TCP local (entrada 1, o FFmpeg escuta), em H.264 (`encoder`, de
    /// preferência na placa de vídeo) e AAC num MP4 fragmentado (legível mesmo se o processo for
    /// interrompido).
    fn spawn(video: &Path, porta_audio: u16, encoder: ffmpeg::EncoderH264, log: &Path) -> io::Result<Child> {
        let mut cmd = Command::new(ffmpeg::programa());
        cmd.args(["-hide_banner", "-nostats", "-loglevel", "warning", "-y"]);
        let tamanho = format!("{}x{}", janela::LARGURA, janela::ALTURA);
        let fps = janela::QUADROS_POR_SEGUNDO.to_string();
        // Formatos conhecidos: sem análise das entradas (ela segurava o FFmpeg alguns segundos e o
        // som chegava atrasado).
        let sem_analise = ["-probesize", "32", "-analyzeduration", "0"];
        cmd.args(sem_analise);
        cmd.args(["-thread_queue_size", "64", "-f", "rawvideo", "-pix_fmt", "nv12", "-s", &tamanho, "-framerate", &fps, "-i", "pipe:0"]);
        cmd.args(sem_analise);
        cmd.args(["-thread_queue_size", "1024", "-f", "f32le", "-ar", &audio::TAXA_PADRAO.to_string(), "-ac", "2", "-i", &format!("tcp://127.0.0.1:{porta_audio}?listen=1")]);
        // NV12 já é o formato dos encoders: sem conversão. Cor BT.601 16–235 (a da placa de vídeo).
        cmd.args(["-map", "0:v", "-map", "1:a", "-colorspace", "smpte170m", "-color_primaries", "smpte170m", "-color_trc", "smpte170m", "-color_range", "tv"]);
        cmd.args(encoder.gravacao());
        cmd.args(["-c:a", "aac", "-b:a", "160k"]);
        cmd.args(["-movflags", "+frag_keyframe+empty_moov"]);
        cmd.arg(video);
        cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::from(std::fs::File::create(log)?));
        cmd.kill_on_drop(true);
        // Sem janela de console (ela tiraria o foco do jogo) e abaixo do normal: o jogo vem antes.
        cmd.creation_flags(0x0800_0000 | 0x0000_4000); // CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS
        cmd.spawn()
    }
}

/// Linux: a tela inteira do `DISPLAY` atual pelo `x11grab`, reduzida a 720p, sem som.
#[cfg(not(windows))]
pub async fn iniciar_tela_inteira(video: &Path) -> Result<Gravacao, String> {
    let log = caminho_do_log(video);
    let tela = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
    let encoder = ffmpeg::encoder_h264_async().await;
    let mut cmd = Command::new(ffmpeg::programa());
    cmd.args(["-hide_banner", "-y", "-f", "x11grab", "-framerate", "60", "-i", &tela]);
    cmd.args(["-vf", "scale=-2:720:flags=area,format=nv12", "-fps_mode", "cfr", "-r", "60"]);
    cmd.args(encoder.gravacao()).args(["-movflags", "+frag_keyframe+empty_moov"]);
    cmd.arg(video);
    let arquivo_log = std::fs::File::create(&log).map_err(|e| format!("falha ao criar o log da gravação: {e}"))?;
    cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::from(arquivo_log));
    cmd.kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| format!("falha ao iniciar o FFmpeg: {e}"))?;
    confirmar_inicio(&mut child, &log).await?;
    Ok(Gravacao { child, parar: Arc::default(), _fontes: Vec::new(), para_com_q: true, com_som: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_fica_ao_lado_do_video() {
        assert_eq!(caminho_do_log(Path::new("C:/dados/shmupx_run_1.mp4")), PathBuf::from("C:/dados/shmupx_run_1.ffmpeg.log"));
    }
}

#[cfg(all(test, windows))]
mod neste_pc {
    use super::*;
    use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    /// Tempo de CPU (s) do processo.
    fn cpu(processo: HANDLE) -> f64 {
        let (mut a, mut b, mut kernel, mut usuario) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        unsafe { GetProcessTimes(processo, &mut a, &mut b, &mut kernel, &mut usuario) }.expect("GetProcessTimes");
        let s = |f: FILETIME| ((u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime)) as f64 / 1e7;
        s(kernel) + s(usuario)
    }

    /// Grava a janela do ShmupX Emulator já aberto (o benchmark abre o motor por fora) por
    /// `SHMUPX_TESTE_SEGUNDOS` em `SHMUPX_TESTE_VIDEO` e mostra a CPU da captura (este processo) e
    /// do FFmpeg: `cargo test --release -- --ignored grava_a_janela_do_motor --nocapture`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "depende do ShmupX Emulator aberto"]
    async fn grava_a_janela_do_motor() {
        let segundos: u64 = std::env::var("SHMUPX_TESTE_SEGUNDOS").ok().and_then(|s| s.parse().ok()).unwrap_or(10);
        let video = PathBuf::from(std::env::var("SHMUPX_TESTE_VIDEO").expect("SHMUPX_TESTE_VIDEO"));
        let pids = || crate::steam::detector::processos_por_nome("shmupx-engine.exe");
        let janela = esperar_janela(pids, Duration::from_secs(30)).await.expect("janela do motor");
        let gravacao = iniciar(&video, janela).await.expect("gravação");
        let ff = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, gravacao.pid().unwrap()) }.expect("OpenProcess");
        let (eu0, ff0, t0) = (cpu(unsafe { GetCurrentProcess() }), cpu(ff), std::time::Instant::now());
        tokio::time::sleep(Duration::from_secs(segundos)).await;
        let (eu1, ff1, dt) = (cpu(unsafe { GetCurrentProcess() }), cpu(ff), t0.elapsed().as_secs_f64());
        let com_som = gravacao.com_som;
        gravacao.parar().await.expect("parar");
        unsafe { CloseHandle(ff) }.ok();
        println!("captura_nucleos={:.2} ffmpeg_nucleos={:.2} com_som={com_som}", (eu1 - eu0) / dt, (ff1 - ff0) / dt);
    }
}
