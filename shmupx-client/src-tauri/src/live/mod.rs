//! Live no YouTube do jogador, só no Ranked (spec 033/036 do Client antigo, revista).
//!
//! Fluxo: o backend cria a live (não listada) e devolve o endereço RTMP → o FFmpeg começa a
//! enviar a tela de espera da ShmupX, com silêncio → quando o YouTube acusa o vídeo chegando, a
//! live vai ao ar → **só então** o jogo abre, no modo ranked (ShmupX Engine no estado inicial de 1
//! crédito, sem continue; ou pela Steam), e a interface começa a run (ficha e gravação) como num
//! Play Ranked comum → a janela do jogo (`janela`: quadro fixo 1280×720, o jogo encaixado sem
//! distorcer) entra no lugar da tela de espera, e o som do jogo no lugar do silêncio → termina no
//! END LIVE, se o jogo fechar, se o FFmpeg cair ou em 45 min. Se a run ranked não começar, a live é
//! encerrada: não existe live fora do Ranked.
//!
//! Várias runs numa live: no ShmupX Emulator, outro jogo abre na mesma janela (`sessao_motor`) e a
//! live segue no ar, sem cortar para quem assiste; a run ranked do jogo novo entra na mesma live (o
//! servidor troca o título para o jogo e o modo dela). Depois do fim de uma run, a live espera o
//! próximo jogo ranked por até `INTERVALO`; sem ele, termina. Um treino com a live no ar a encerra.
//!
//! A live não grava nada neste PC: o vídeo fica no canal do jogador. Se a run entrar no top 3 do
//! ranking, o `destaque` baixa o vídeo da live do YouTube depois que ela termina, corta a tela de
//! espera e envia ao canal oficial da ShmupX.
//! O endereço RTMP leva a chave da transmissão: nunca vai para log (FFmpeg em `-loglevel error`).

pub(crate) mod audio;
pub mod commands;
pub mod destaque;
pub mod ytdlp;
#[cfg(windows)]
pub mod janela;
#[cfg(not(windows))]
#[path = "janela_outros.rs"]
pub mod janela;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::process::{Child, Command};
use tokio::sync::Notify;

use crate::capture::ffmpeg;

/// Limite da live: a mesma sessão de 45 min da gravação (spec 033 FR-03316).
pub const LIMITE: Duration = Duration::from_secs(45 * 60);
/// Tempo para o YouTube acusar o vídeo chegando (spec 033 FR-03306).
const ATIVACAO: Duration = Duration::from_secs(90);
/// Depois que a janela do jogo aparece, o tempo para a run ranked começar (ficha e gravação).
const INICIO_DA_RUN: Duration = Duration::from_secs(60);
/// Depois do fim de uma run, quanto a live espera o próximo jogo ranked (na mesma janela do
/// ShmupX Emulator) antes de terminar.
pub const INTERVALO: Duration = Duration::from_secs(120);
pub const EVENTO: &str = "live://estado";

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Estado {
    /// `idle`, `preparing`, `connecting`, `live`, `ending` ou `failed`.
    pub status: &'static str,
    pub game_id: Option<String>,
    pub watch_url: Option<String>,
    pub public: bool,
    /// Fim automático (ms desde 1970), enquanto no ar.
    pub ends_at_ms: Option<u64>,
    pub error: Option<String>,
    /// A última live terminou pelo limite de 45 min.
    pub ended_by_time_limit: bool,
    /// Som do jogo indisponível: a live está com trilha de silêncio.
    pub silent: bool,
    /// No ar com o jogo na tela. Antes disso (`live` sem jogo), a live mostra a tela de espera
    /// enquanto o jogo abre.
    pub game_open: bool,
    /// O jogo da live é do ShmupX Emulator: outro jogo ranked dele abre na mesma janela e entra na
    /// live (Play Ranked em outra sala com a live no ar).
    pub emulator: bool,
}

/// Como abrir o jogo depois que a live entra no ar (o mesmo que o Play Ranked faria).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "platform", rename_all = "lowercase")]
pub enum Abertura {
    Steam {
        #[serde(rename = "steamAppId")]
        steam_app_id: u32,
        /// Jogo sem a proteção da Steam (No Steam Protection): abre direto, sem a Steam.
        #[serde(default, rename = "semSteam")]
        sem_steam: bool,
    },
    Emulator {
        roms: Vec<String>,
        #[serde(default)]
        pasta: Option<String>,
        #[serde(default, rename = "motorVideo")]
        motor_video: Option<crate::steam::emulador::VideoDoMotor>,
        #[serde(default, rename = "motorControles")]
        motor_controles: Option<Vec<crate::steam::controles::ControleMapeado>>,
        /// Live Ranked em Caravan: o tempo do Caravan do jogo (contador e fim no motor).
        #[serde(default, rename = "caravanSegundos")]
        caravan_segundos: Option<u32>,
        #[serde(default, rename = "cartaoJogador")]
        cartao_jogador: Option<bool>,
        #[serde(default)]
        idioma: Option<String>,
        /// Jogo de console: o sistema do MAME que abre o cartucho.
        #[serde(default)]
        sistema: Option<String>,
    },
}

impl Abertura {
    fn steam_app_id(&self) -> Option<u32> {
        match self {
            Abertura::Steam { steam_app_id, .. } => Some(*steam_app_id).filter(|id| *id > 0),
            Abertura::Emulator { .. } => None,
        }
    }
}

struct Corrente {
    broadcast_id: Option<String>,
    child: Option<Child>,
    parar_audio: Arc<AtomicBool>,
    /// Captura da janela do jogo (para ao ser descartada).
    captura: Option<janela::CapturaJanela>,
}

/// A run ranked transmitida agora por esta live (e as que já passaram por ela).
struct Vinculo {
    jogo_id: String,
    broadcast_id: String,
    /// Quando a live entrou no ar (o começo do vídeo dela no YouTube).
    no_ar_desde: Instant,
    /// Segundo do vídeo da live em que o jogo apareceu (o destaque corta a tela de espera).
    jogo_aos_s: Option<f64>,
    run_id: Option<String>,
    /// O jogo é do ShmupX Emulator: outro jogo pode abrir na mesma janela e entrar nesta live.
    emulador: bool,
    /// A run terminou e a live espera o próximo jogo ranked (desde quando).
    entre_runs: Option<Instant>,
    /// Segundo do vídeo em que o próximo jogo apareceu (trocado na janela), antes da run dele.
    proximo_aos_s: Option<f64>,
    /// Runs que já passaram por esta live: (run, começo, fim) em segundos do vídeo.
    anteriores: Vec<(String, f64, f64)>,
}

impl Vinculo {
    fn novo(jogo_id: &str, broadcast_id: &str, emulador: bool) -> Self {
        Vinculo {
            jogo_id: jogo_id.to_string(),
            broadcast_id: broadcast_id.to_string(),
            no_ar_desde: Instant::now(),
            jogo_aos_s: None,
            run_id: None,
            emulador,
            entre_runs: None,
            proximo_aos_s: None,
            anteriores: Vec::new(),
        }
    }

    fn agora_s(&self) -> f64 {
        self.no_ar_desde.elapsed().as_secs_f64()
    }

    /// A live aceita a run de `jogo_id`: a do jogo dela, ou outro jogo aberto na mesma janela do
    /// ShmupX Emulator depois que a run anterior terminou.
    fn aceita(&self, jogo_id: &str) -> bool {
        self.run_id.is_none() && (self.jogo_id == jogo_id || (self.emulador && self.entre_runs.is_some()))
    }
}

/// A live de uma run, guardada quando a live termina, até o resultado da run decidir se ela vai
/// para o canal oficial (top 3).
#[derive(Debug, Clone, PartialEq)]
pub struct LiveDaRun {
    /// Id do vídeo da live no YouTube (o do broadcast).
    pub video_id: String,
    /// Segundos do começo a pular (a tela de espera antes do jogo abrir, ou as runs anteriores).
    pub corte_s: f64,
    /// Onde a run acaba no vídeo, quando outra run veio depois dela na mesma live.
    pub fim_s: Option<f64>,
}

#[derive(Clone)]
pub struct LiveState {
    estado: Arc<Mutex<Estado>>,
    corrente: Arc<tokio::sync::Mutex<Option<Corrente>>>,
    cancelar: Arc<Notify>,
    vinculo: Arc<Mutex<Option<Vinculo>>>,
    lives: Arc<Mutex<HashMap<String, LiveDaRun>>>,
}

impl Default for LiveState {
    fn default() -> Self {
        Self {
            estado: Arc::new(Mutex::new(Estado { status: "idle", ..Default::default() })),
            corrente: Arc::default(),
            cancelar: Arc::default(),
            vinculo: Arc::default(),
            lives: Arc::default(),
        }
    }
}

fn travar<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl LiveState {
    pub fn snapshot(&self) -> Estado {
        travar(&self.estado).clone()
    }

    /// Live em andamento (de "preparando" até "encerrando").
    pub fn ativa(&self) -> bool {
        !matches!(self.snapshot().status, "idle" | "failed")
    }

    fn definir(&self, app: &AppHandle, novo: Estado) {
        *travar(&self.estado) = novo.clone();
        let _ = app.emit(EVENTO, novo);
    }

    fn alterar(&self, app: &AppHandle, f: impl FnOnce(&mut Estado)) {
        let mut atual = self.snapshot();
        f(&mut atual);
        self.definir(app, atual);
    }

    /// Antes de debitar a ficha de uma run de `jogo_id`: o id da live que vai transmiti-la (o
    /// backend liga a gameplay à live), ou nenhum sem live. `Err`: há uma live que não é desta run.
    pub fn broadcast_para_run(&self, jogo_id: &str) -> Result<Option<String>, String> {
        if !self.ativa() {
            return Ok(None);
        }
        let no_ar = self.snapshot().status == "live";
        match travar(&self.vinculo).as_ref() {
            Some(v) if no_ar && v.aceita(jogo_id) => Ok(Some(v.broadcast_id.clone())),
            _ => Err("Há uma live em andamento: encerre-a antes de iniciar outra run.".into()),
        }
    }

    /// A run ranked desta live começou (ficha debitada, gravação confirmada). Um jogo novo na mesma
    /// janela passa a ser o da live.
    pub fn vincular_run(&self, app: &AppHandle, jogo_id: &str, run_id: &str) {
        let mut trocou = false;
        if let Some(v) = travar(&self.vinculo).as_mut().filter(|v| v.aceita(jogo_id)) {
            trocou = v.jogo_id != jogo_id;
            if v.entre_runs.take().is_some() {
                v.jogo_aos_s = Some(v.proximo_aos_s.take().unwrap_or_else(|| v.agora_s()));
            }
            v.jogo_id = jogo_id.to_string();
            v.run_id = Some(run_id.to_string());
        }
        if trocou {
            self.alterar(app, |e| e.game_id = Some(jogo_id.to_string()));
        }
    }

    /// A run terminou (STOP, jogo fechado ou fim da ficha): a live espera o próximo jogo ranked na
    /// mesma janela (`INTERVALO`); sem ele, termina.
    pub fn run_terminou(&self, run_id: &str) {
        if let Some(v) = travar(&self.vinculo).as_mut().filter(|v| v.run_id.as_deref() == Some(run_id)) {
            let fim = v.agora_s();
            v.anteriores.push((run_id.to_string(), v.jogo_aos_s.unwrap_or(0.0), fim));
            v.run_id = None;
            v.jogo_aos_s = None;
            v.entre_runs = Some(Instant::now());
        }
    }

    /// Outro jogo abriu na janela do ShmupX Emulator da live (antes da run dele começar): a espera
    /// pela run dele recomeça.
    pub fn jogo_trocado(&self) {
        if let Some(v) = travar(&self.vinculo).as_mut() {
            v.proximo_aos_s = Some(v.agora_s());
            if v.entre_runs.is_some() {
                v.entre_runs = Some(Instant::now());
            }
        }
    }

    /// END LIVE (também cancela um início em andamento).
    pub fn encerrar(&self) {
        self.cancelar.notify_waiters();
    }

    /// Um jogo ranked do ShmupX Emulator pode entrar nesta live agora (no ar, com o jogo na tela).
    pub fn aceita_outro_jogo(&self) -> bool {
        let e = self.snapshot();
        e.status == "live" && e.game_open && travar(&self.vinculo).as_ref().is_some_and(|v| v.emulador)
    }

    /// (run atual da live, a live esperou o próximo jogo além do `INTERVALO`).
    fn run_da_live(&self) -> (Option<String>, bool) {
        travar(&self.vinculo).as_ref().map_or((None, false), |v| (v.run_id.clone(), v.entre_runs.is_some_and(|d| d.elapsed() >= INTERVALO)))
    }

    /// A live da run `run_id` (só depois que a live terminou).
    pub fn tomar_live(&self, run_id: &str) -> Option<LiveDaRun> {
        travar(&self.lives).remove(run_id)
    }

    /// Fim da live: guarda o vídeo dela para cada run decidir (sem run, não há o que guardar).
    fn guardar_live(&self) {
        let Some(v) = travar(&self.vinculo).take() else { return };
        let mut lives = travar(&self.lives);
        for (run_id, corte_s, fim_s) in v.anteriores {
            lives.insert(run_id, LiveDaRun { video_id: v.broadcast_id.clone(), corte_s, fim_s: Some(fim_s) });
        }
        if let Some(run_id) = v.run_id {
            lives.insert(run_id, LiveDaRun { video_id: v.broadcast_id, corte_s: v.jogo_aos_s.unwrap_or(0.0), fim_s: None });
        }
    }
}

fn agora_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Moeda da ShmupX no centro da tela de espera.
const MOEDA: &[u8] = include_bytes!("../../../public/img/shmupx-token-front.png");
/// Fundo da tela de espera (BGRA).
const FUNDO: [u8; 4] = [0x16, 0x0e, 0x0b, 0xff];

/// Tela de espera (BGRA 1280×720): fundo escuro com a moeda da ShmupX, no ar até o jogo abrir.
fn tela_de_espera() -> Vec<u8> {
    let (largura, altura) = (janela::LARGURA, janela::ALTURA);
    let mut quadro = FUNDO.repeat(largura * altura);
    let Ok(imagem) = image::load_from_memory_with_format(MOEDA, image::ImageFormat::Png) else { return quadro };
    let lado = 320u32;
    let moeda = image::imageops::resize(&imagem.to_rgba8(), lado, lado, image::imageops::FilterType::Triangle);
    let (ox, oy) = ((largura - lado as usize) / 2, (altura - lado as usize) / 2);
    for (x, y, pixel) in moeda.enumerate_pixels() {
        let a = u32::from(pixel[3]);
        let i = ((oy + y as usize) * largura + ox + x as usize) * 4;
        for (canal, valor) in [(0, pixel[2]), (1, pixel[1]), (2, pixel[0])] {
            quadro[i + canal] = ((u32::from(valor) * a + u32::from(quadro[i + canal]) * (255 - a)) / 255) as u8;
        }
    }
    quadro
}

/// FFmpeg da live: vídeo BGRA 1280×720 a 60 quadros/s pelo stdin (entrada 0) e o áudio `f32le`
/// por uma conexão TCP local (entrada 1, o FFmpeg escuta), direto para o YouTube (nada no PC).
fn spawn_ffmpeg_janela(ingest: &str, porta_audio: u16, encoder: ffmpeg::EncoderH264, log: &PathBuf) -> std::io::Result<Child> {
    let mut cmd = Command::new(ffmpeg::programa());
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"]);
    let tamanho = format!("{}x{}", janela::LARGURA, janela::ALTURA);
    let fps = janela::QUADROS_POR_SEGUNDO.to_string();
    // Formatos conhecidos: sem análise das entradas. Analisar segurava o FFmpeg ~4 s antes de
    // começar, o som acumulava nesse tempo e a live saía com o áudio atrasado.
    let sem_analise = ["-probesize", "32", "-analyzeduration", "0"];
    cmd.args(sem_analise);
    cmd.args(["-f", "rawvideo", "-pix_fmt", "bgra", "-s", &tamanho, "-framerate", &fps, "-i", "pipe:0"]);
    cmd.args(sem_analise);
    cmd.args(["-f", "f32le", "-ar", &audio::TAXA_PADRAO.to_string(), "-ac", "2", "-i", &format!("tcp://127.0.0.1:{porta_audio}?listen=1")]);
    cmd.args(["-map", "0:v", "-map", "1:a", "-vf", "format=nv12"]);
    cmd.args(encoder.live()).args(ffmpeg::TAXA_LIVE);
    cmd.args(["-c:a", "aac", "-b:a", "160k", "-ar", "48000"]);
    cmd.args(["-f", "flv", "-flvflags", "no_duration_filesize", ingest]);
    cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::from(std::fs::File::create(log)?));
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd.spawn()
}

/// Porta livre em 127.0.0.1 (para o FFmpeg escutar o áudio).
pub(crate) fn porta_livre() -> Result<u16, String> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("sem porta local para o áudio: {e}"))
}

/// Liga o sinal de parada da live (`parar_audio`) ao de cada escritor.
fn ligar_parada(da_live: Arc<AtomicBool>, do_escritor: Arc<AtomicBool>) {
    tokio::spawn(async move {
        while !da_live.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        do_escritor.store(true, Ordering::SeqCst);
    });
}

/// Envio da live: FFmpeg, escritor de vídeo (da `tela`, que começa com a tela de espera) e de
/// áudio (da `fonte`, que começa em silêncio).
async fn abrir_envio(ingest: &str, log: &PathBuf, parar_audio: Arc<AtomicBool>, tela: janela::Tela, fonte: audio::Fonte) -> Result<Child, String> {
    let porta = porta_livre()?;
    let encoder = ffmpeg::encoder_h264_async().await;
    let mut child = spawn_ffmpeg_janela(ingest, porta, encoder, log).map_err(|e| format!("FFmpeg indisponível: {e}"))?;
    let stdin = child.stdin.take().ok_or("FFmpeg sem entrada de vídeo")?;
    #[cfg(windows)]
    let stdin = std::fs::File::from(stdin.into_owned_handle().map_err(|e| format!("FFmpeg sem entrada de vídeo: {e}"))?);
    #[cfg(unix)]
    let stdin = std::fs::File::from(stdin.into_owned_fd().map_err(|e| format!("FFmpeg sem entrada de vídeo: {e}"))?);
    let parar = Arc::new(AtomicBool::new(false));
    janela::alimentar(stdin, tela, parar.clone());
    // O FFmpeg abre a entrada de áudio depois de ler o vídeo: tenta conectar por até 10 s.
    let conexao = async {
        for _ in 0..100 {
            if let Ok(conexao) = tokio::net::TcpStream::connect(("127.0.0.1", porta)).await {
                return Some(conexao);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        None
    }
    .await;
    let Some(conexao) = conexao else {
        parar.store(true, Ordering::SeqCst);
        let _ = child.kill().await;
        return Err(format!("o envio do vídeo não iniciou: {}", motivo_do_log(log)));
    };
    tokio::spawn(audio::alimentar(conexao, fonte, parar.clone()));
    tokio::time::sleep(Duration::from_millis(1500)).await;
    if child.try_wait().ok().flatten().is_some() {
        parar.store(true, Ordering::SeqCst);
        return Err(format!("o envio do vídeo não iniciou: {}", motivo_do_log(log)));
    }
    ligar_parada(parar_audio, parar);
    Ok(child)
}

/// Primeira linha útil do log do FFmpeg (motivo de uma falha), sem nada que lembre a chave.
fn motivo_do_log(log: &PathBuf) -> String {
    let texto = std::fs::read_to_string(log).unwrap_or_default();
    let linha = ffmpeg::ultima_linha(&texto);
    if linha.contains("rtmp") || linha.contains("live2") {
        "o envio do vídeo ao YouTube falhou".to_string()
    } else if linha.is_empty() {
        "o FFmpeg encerrou sem motivo registrado".to_string()
    } else {
        linha
    }
}

/// Apaga cópias de lives que versões antigas do Client deixavam na pasta das gravações.
fn limpar_copias_antigas(pasta: &Path) {
    let Ok(itens) = std::fs::read_dir(pasta) else { return };
    let limite = Duration::from_secs(2 * 24 * 60 * 60);
    for item in itens.flatten() {
        let nome = item.file_name().to_string_lossy().to_string();
        let velho = item.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|idade| idade > limite);
        if nome.starts_with("shmupx_live_") && nome.ends_with(".mp4") && velho {
            let _ = std::fs::remove_file(item.path());
        }
    }
}

/// Leva a live do início ao ar e abre o jogo. Erros viram `failed` com a mensagem; cancelar
/// volta a `idle`.
#[allow(clippy::too_many_arguments)]
pub async fn conduzir(app: AppHandle, live: LiveState, token: String, jogo_id: String, titulo: String, modo: Option<String>, publica: bool, abertura: Abertura, log: PathBuf, pasta: PathBuf) {
    limpar_copias_antigas(&pasta);
    let resultado = tokio::select! {
        r = colocar_no_ar(&app, &live, &token, &jogo_id, &titulo, modo.as_deref(), publica, &abertura, &log) => r,
        _ = live.cancelar.notified() => Err(String::new()),
    };
    match resultado {
        Ok(()) => vigiar(app, live, token, log).await,
        Err(e) => {
            encerrar_recursos(&live, &token).await;
            live.definir(&app, if e.is_empty() {
                Estado { status: "idle", ..Default::default() }
            } else {
                Estado { status: "failed", game_id: Some(jogo_id), error: Some(e), ..Default::default() }
            });
        }
    }
}

/// PIDs do jogo da sala: pela pasta de instalação da Steam ou, sem Steam, o ShmupX Emulator.
fn pids_do_jogo(steam_app_id: Option<u32>) -> Vec<u32> {
    match steam_app_id.filter(|id| *id > 0) {
        Some(app_id) => crate::steam::biblioteca::pasta_do_jogo(app_id).map(|p| crate::steam::detector::processos_na_pasta(&p)).unwrap_or_default(),
        None => crate::steam::detector::processos_por_nome("shmupx-engine.exe"),
    }
}

/// Play Ranked com a live no ar: o jogo novo abre na mesma janela do ShmupX Emulator e a run dele
/// entra nesta live (a interface começa a run pelo aviso de jogo aberto, como no Play Ranked).
pub async fn trocar_jogo(app: &AppHandle, jogo_id: &str, abertura: &Abertura) -> Result<(), String> {
    abrir_jogo(app, jogo_id, abertura).await
}

/// Abre o jogo no modo ranked, como o Play Ranked: a interface recebe o aviso de jogo aberto
/// (`steam://game-launched` ou `emulator://game-launched`) e começa a run (ficha e gravação).
async fn abrir_jogo(app: &AppHandle, jogo_id: &str, abertura: &Abertura) -> Result<(), String> {
    let aberto = match abertura.clone() {
        Abertura::Steam { steam_app_id, sem_steam } => crate::steam::commands::inicializar_jogo_steam(app.clone(), steam_app_id, Some(false), Some(sem_steam)).await,
        Abertura::Emulator { roms, pasta, motor_video, motor_controles, caravan_segundos, cartao_jogador, idioma, sistema } => {
            crate::steam::commands::inicializar_jogo_emulador(app.clone(), jogo_id.to_string(), roms, pasta, Some(false), Some(false), motor_video, motor_controles, caravan_segundos, cartao_jogador, idioma, None, sistema).await
        }
    };
    aberto.map_err(|e| {
        log::warn!("live: o jogo não abriu: {e}");
        "o jogo não abriu; confira se ele está instalado (ou o ROM na pasta de ROMs) e tente de novo".to_string()
    })
}

/// Espera a janela do jogo recém-aberto: até 3 min na Steam (ela pode estar abrindo ou
/// atualizando) e 1 min no ShmupX Emulator, como o `WaitingForGame` do Client antigo.
async fn esperar_janela(steam_app_id: Option<u32>) -> Result<janela::Window, String> {
    let procurar = move || janela::janela_dos_processos(&pids_do_jogo(steam_app_id));
    let espera = if steam_app_id.is_some() { Duration::from_secs(180) } else { Duration::from_secs(60) };
    let inicio = tokio::time::Instant::now();
    while inicio.elapsed() < espera {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if let Some(janela) = tauri::async_runtime::spawn_blocking(procurar).await.ok().flatten() {
            // Dá um instante para o jogo terminar de montar a janela.
            tokio::time::sleep(Duration::from_secs(2)).await;
            return Ok(tauri::async_runtime::spawn_blocking(procurar).await.ok().flatten().unwrap_or(janela));
        }
        // Emulador que nem chegou a abrir (ROM ruim, emulador fechado): não espera o prazo todo.
        if steam_app_id.is_none() && inicio.elapsed() > Duration::from_secs(15) {
            let vivo = tauri::async_runtime::spawn_blocking(move || !pids_do_jogo(None).is_empty()).await.unwrap_or(false);
            if !vivo {
                return Err("o ShmupX Emulator não abriu o jogo; confira o ROM na sua pasta de ROMs e tente de novo".to_string());
            }
        }
    }
    Err("a janela do jogo não apareceu; tente de novo".to_string())
}

#[allow(clippy::too_many_arguments)]
async fn colocar_no_ar(app: &AppHandle, live: &LiveState, token: &str, jogo_id: &str, titulo: &str, modo: Option<&str>, publica: bool, abertura: &Abertura, log: &PathBuf) -> Result<(), String> {
    let base = Estado { game_id: Some(jogo_id.to_string()), public: publica, emulator: abertura.steam_app_id().is_none(), ..Default::default() };
    // Fora do Windows ainda não há captura da janela do jogo.
    if cfg!(not(windows)) {
        return Err("a live ainda não está disponível no Linux".to_string());
    }
    // 1. A live no YouTube, com o jogo ainda fechado. Com o jogo e o modo, o servidor monta o
    // título (`Ketsui · HI-SCORE — jogador | ShmupX`); `title` vale para um servidor antigo.
    live.definir(app, Estado { status: "preparing", ..base.clone() });
    ffmpeg::preparar_encoder_h264();
    let criada = crate::net::backend::chamar_json(
        token,
        reqwest::Method::POST,
        "/api/live/start",
        Some(serde_json::json!({ "title": titulo, "public": publica, "game_id": jogo_id, "mode": modo })),
    )
    .await?;
    let texto = |c: &str| criada[c].as_str().map(str::to_string).ok_or_else(|| "resposta inesperada do servidor".to_string());
    let (broadcast_id, stream_id, ingest, watch) = (texto("broadcast_id")?, texto("stream_id")?, texto("ingest_url")?, texto("watch_url")?);
    let parar_audio = Arc::new(AtomicBool::new(false));
    *live.corrente.lock().await = Some(Corrente { broadcast_id: Some(broadcast_id.clone()), child: None, parar_audio: parar_audio.clone(), captura: None });
    *travar(&live.vinculo) = Some(Vinculo::novo(jogo_id, &broadcast_id, abertura.steam_app_id().is_none()));

    // 2. A tela de espera indo para o YouTube (e para a cópia), até a live entrar no ar.
    live.definir(app, Estado { status: "connecting", watch_url: Some(watch.clone()), ..base.clone() });
    let tela: janela::Tela = Arc::new(Mutex::new(tela_de_espera()));
    let fonte: audio::Fonte = Arc::default();
    let child = abrir_envio(&ingest, log, parar_audio, tela.clone(), fonte.clone()).await?;
    if let Some(c) = live.corrente.lock().await.as_mut() {
        c.child = Some(child);
    }
    let prazo = tokio::time::Instant::now() + ATIVACAO;
    loop {
        if let Some(c) = live.corrente.lock().await.as_mut() {
            if let Some(child) = c.child.as_mut() {
                if child.try_wait().ok().flatten().is_some() {
                    return Err(format!("o envio do vídeo parou: {}", motivo_do_log(log)));
                }
            }
        }
        let situacao = crate::net::backend::chamar_json(token, reqwest::Method::GET, &format!("/api/live/streams/{stream_id}/status"), None).await?;
        if situacao["status"].as_str() == Some("active") {
            break;
        }
        if tokio::time::Instant::now() >= prazo {
            return Err("o YouTube não recebeu o vídeo a tempo; verifique sua conexão e tente de novo".to_string());
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    crate::net::backend::chamar_json(
        token,
        reqwest::Method::POST,
        &format!("/api/live/broadcasts/{broadcast_id}/transition"),
        Some(serde_json::json!({ "status": "live" })),
    )
    .await?;
    // O vídeo da live no YouTube começa aqui: o corte da tela de espera conta a partir deste instante.
    if let Some(v) = travar(&live.vinculo).as_mut() {
        v.no_ar_desde = Instant::now();
    }
    live.definir(app, Estado {
        status: "live",
        watch_url: Some(watch),
        ends_at_ms: Some(agora_ms() + LIMITE.as_millis() as u64),
        ..base
    });

    // 3. Live no ar: só agora o jogo abre, no modo ranked.
    abrir_jogo(app, jogo_id, abertura).await?;
    let janela_do_jogo = esperar_janela(abertura.steam_app_id()).await?;
    let captura = janela::iniciar(janela_do_jogo, tela)?;
    let pid = captura.pid;
    let loopback = tauri::async_runtime::spawn_blocking(move || audio::iniciar(pid)).await.map_err(|e| e.to_string())?;
    let silenciosa = match loopback {
        Ok(l) => {
            *travar(&fonte) = Some(l);
            false
        }
        Err(e) => {
            log::warn!("live: sem som do jogo ({e}); a live segue com silêncio");
            true
        }
    };
    if let Some(c) = live.corrente.lock().await.as_mut() {
        c.captura = Some(captura);
    }
    if let Some(v) = travar(&live.vinculo).as_mut() {
        v.jogo_aos_s = Some(v.no_ar_desde.elapsed().as_secs_f64());
    }
    live.alterar(app, |e| {
        e.game_open = true;
        e.silent = silenciosa;
    });

    // 4. A run ranked tem que começar (ficha e gravação): sem ela, não há live.
    let prazo = tokio::time::Instant::now() + INICIO_DA_RUN;
    while live.run_da_live().0.is_none() {
        if tokio::time::Instant::now() >= prazo {
            return Err("a partida ranked não começou (ficha ou gravação); a live só acontece no Ranked".to_string());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Ok(())
}

/// No ar: termina no END LIVE, se o jogo fechar, se o FFmpeg cair, no limite de 45 min ou sem um
/// jogo ranked novo depois da run (`INTERVALO`).
async fn vigiar(app: AppHandle, live: LiveState, token: String, log: PathBuf) {
    // `true`: o jogo fechou ou a run acabou (fim normal); `false`: o FFmpeg caiu.
    let caiu = async {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if live.run_da_live().1 {
                break true;
            }
            let mut guarda = live.corrente.lock().await;
            let vivo = guarda
                .as_mut()
                .and_then(|c| c.child.as_mut())
                .is_some_and(|child| child.try_wait().ok().flatten().is_none());
            if !vivo {
                break false;
            }
            if guarda.as_ref().and_then(|c| c.captura.as_ref()).is_some_and(|c| c.fechada.load(Ordering::SeqCst)) {
                break true;
            }
        }
    };
    let restante = live.snapshot().ends_at_ms.map_or(LIMITE, |fim| Duration::from_millis(fim.saturating_sub(agora_ms())));
    let (motivo_erro, pelo_tempo) = tokio::select! {
        _ = live.cancelar.notified() => (None, false),
        _ = tokio::time::sleep(restante) => (None, true),
        fim_normal = caiu => if fim_normal { (None, false) } else { (Some(format!("a transmissão caiu: {}", motivo_do_log(&log))), false) },
    };
    live.alterar(&app, |e| e.status = "ending");
    let jogo = live.snapshot().game_id;
    encerrar_recursos(&live, &token).await;
    live.definir(&app, match motivo_erro {
        Some(e) => Estado { status: "failed", game_id: jogo, error: Some(e), ..Default::default() },
        None => Estado { status: "idle", ended_by_time_limit: pelo_tempo, ..Default::default() },
    });
}

/// Para o áudio e o FFmpeg, encerra a live no YouTube (melhor esforço: sem rede, os recursos
/// locais são liberados mesmo assim) e guarda o vídeo da live para o resultado da run.
async fn encerrar_recursos(live: &LiveState, token: &str) {
    let corrente = live.corrente.lock().await.take();
    if let Some(mut corrente) = corrente {
        corrente.parar_audio.store(true, Ordering::SeqCst);
        drop(corrente.captura.take());
        if let Some(mut child) = corrente.child.take() {
            // Sem vídeo e sem áudio chegando, o FFmpeg fecha as saídas e sai sozinho.
            if tokio::time::timeout(Duration::from_secs(6), child.wait()).await.is_err() {
                let _ = child.kill().await;
            }
        }
        if let Some(id) = corrente.broadcast_id.take() {
            let _ = crate::net::backend::chamar_json(
                token,
                reqwest::Method::POST,
                &format!("/api/live/broadcasts/{id}/transition"),
                Some(serde_json::json!({ "status": "complete" })),
            )
            .await;
        }
    }
    live.guardar_live();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tela_de_espera_tem_o_tamanho_da_live_e_a_moeda_no_centro() {
        let tela = tela_de_espera();
        assert_eq!(tela.len(), janela::LARGURA * janela::ALTURA * 4);
        let pixel = |x: usize, y: usize| tela[(y * janela::LARGURA + x) * 4..(y * janela::LARGURA + x) * 4 + 4].to_vec();
        assert_eq!(pixel(10, 10), FUNDO, "fundo escuro");
        assert_ne!(pixel(janela::LARGURA / 2, janela::ALTURA / 2), FUNDO, "moeda no centro");
    }

    #[test]
    fn so_a_run_da_live_recebe_o_broadcast() {
        let live = LiveState::default();
        assert_eq!(live.broadcast_para_run("aero"), Ok(None), "sem live, run comum");
        *travar(&live.estado) = Estado { status: "live", game_id: Some("aero".into()), ..Default::default() };
        let mut vinculo = Vinculo::novo("aero", "b1", false);
        vinculo.jogo_aos_s = Some(12.5);
        *travar(&live.vinculo) = Some(vinculo);
        assert!(live.broadcast_para_run("outro").is_err(), "live de outro jogo barra a run");
        assert_eq!(live.broadcast_para_run("aero"), Ok(Some("b1".into())));
        travar(&live.vinculo).as_mut().unwrap().run_id = Some("r1".into());
        assert!(live.broadcast_para_run("aero").is_err(), "uma run de cada vez");
        assert_eq!(live.run_da_live(), (Some("r1".into()), false));
        live.run_terminou("r1");
        assert_eq!(live.run_da_live(), (None, false), "a live espera o próximo jogo");
        assert!(live.broadcast_para_run("outro").is_err(), "jogo da Steam: outro jogo não entra");
        // No fim da live, o vídeo dela fica guardado para a run (uma vez), até o fim dela.
        live.guardar_live();
        let guardada = live.tomar_live("r1").unwrap();
        assert_eq!((guardada.video_id.as_str(), guardada.corte_s), ("b1", 12.5));
        assert!(guardada.fim_s.is_some());
        assert!(live.tomar_live("r1").is_none());
    }

    #[test]
    fn outro_jogo_do_emulador_entra_na_mesma_live() {
        let live = LiveState::default();
        *travar(&live.estado) = Estado { status: "live", game_id: Some("aero".into()), game_open: true, ..Default::default() };
        let mut vinculo = Vinculo::novo("aero", "b1", true);
        vinculo.jogo_aos_s = Some(10.0);
        vinculo.run_id = Some("r1".into());
        *travar(&live.vinculo) = Some(vinculo);
        assert!(live.aceita_outro_jogo());
        assert!(live.broadcast_para_run("ketsui").is_err(), "com a run em andamento, não");
        live.run_terminou("r1");
        assert_eq!(live.broadcast_para_run("ketsui"), Ok(Some("b1".into())), "depois do fim da run, o jogo novo entra");
        live.jogo_trocado();
        assert!(travar(&live.vinculo).as_ref().unwrap().aceita("ketsui"));
        // Sem AppHandle no teste: o vínculo da run nova como o vincular_run faz.
        {
            let mut v = travar(&live.vinculo);
            let v = v.as_mut().unwrap();
            v.entre_runs = None;
            v.jogo_aos_s = v.proximo_aos_s.take();
            v.jogo_id = "ketsui".into();
            v.run_id = Some("r2".into());
        }
        live.guardar_live();
        let r1 = live.tomar_live("r1").unwrap();
        let r2 = live.tomar_live("r2").unwrap();
        assert_eq!(r1.corte_s, 10.0);
        assert!(r1.fim_s.is_some(), "a primeira run acaba onde a segunda começa");
        assert!(r2.fim_s.is_none());
        assert_eq!(r1.video_id, r2.video_id);
    }
}

#[cfg(test)]
mod neste_pc {
    use super::*;

    /// Envio completo para um arquivo local (no lugar do YouTube), com um tom tocando:
    /// `cargo test -- --ignored envio_local --nocapture`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "depende de som e FFmpeg neste PC"]
    async fn envio_local() {
        let pasta = std::env::temp_dir();
        let saida = pasta.join("shmupx_live_teste.flv");
        let log = pasta.join("shmupx_live_teste.log");
        let _ = std::fs::remove_file(&saida);
        let tela: janela::Tela = Arc::new(Mutex::new(tela_de_espera()));
        let fonte: audio::Fonte = Arc::default();
        let tom = std::thread::spawn(|| {
            use rodio::Source;
            let (_s, h) = rodio::OutputStream::try_default().unwrap();
            let sink = rodio::Sink::try_new(&h).unwrap();
            sink.append(rodio::source::SineWave::new(440.0).take_duration(Duration::from_secs(5)).amplify(0.3));
            sink.sleep_until_end();
        });
        let parar = Arc::new(AtomicBool::new(false));
        let mut child = abrir_envio(saida.to_str().unwrap(), &log, parar.clone(), tela, fonte.clone()).await.unwrap();
        // O "jogo" (este processo) entra depois, como na live do Ranked.
        tokio::time::sleep(Duration::from_secs(2)).await;
        *travar(&fonte) = audio::iniciar(std::process::id()).ok();
        let t0 = std::time::Instant::now();
        tokio::time::sleep(Duration::from_secs(6)).await;
        println!("no ar por {:?}", t0.elapsed());
        parar.store(true, Ordering::SeqCst);
        let _ = tokio::time::timeout(Duration::from_secs(6), child.wait()).await;
        tom.join().unwrap();
        println!("log: {}", std::fs::read_to_string(&log).unwrap_or_default());
        println!("saida: {} bytes", std::fs::metadata(&saida).map(|m| m.len()).unwrap_or(0));
    }
}
