//! Jogos abertos no MODO TREINO (sem run): o Client lembra o processo para o PARAR JOGO poder
//! fechá-lo e avisa a interface (`treino://encerrado`) quando o jogo fecha por conta própria.
//! Só fecha processos que ele mesmo abriu no treino. Com "Gravar gameplays" marcado
//! (Configurações › Armazenamento), grava a janela do jogo até ele fechar.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter};

fn treinos() -> &'static Mutex<HashSet<u32>> {
    static TREINOS: OnceLock<Mutex<HashSet<u32>>> = OnceLock::new();
    TREINOS.get_or_init(|| Mutex::new(HashSet::new()))
}

#[derive(Clone, Serialize)]
struct Encerrado {
    pid: u32,
}

fn vivo(pid: u32) -> bool {
    let mut sistema = System::new();
    sistema.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
    sistema.process(Pid::from_u32(pid)).is_some()
}

/// Grava o treino, se "Gravar gameplays" estiver marcado. Uma falha só vai para o log: o treino segue.
async fn gravar(app: &AppHandle, pid: u32, steam_app_id: Option<u32>) -> Option<crate::capture::gravacao::Gravacao> {
    if !crate::capture::destino::gravar_gameplays(app) || !crate::capture::ffmpeg::is_ffmpeg_available() {
        return None;
    }
    let pasta = crate::capture::destino::pasta_das_gravacoes(app).inspect_err(|e| log::warn!("treino: sem pasta para gravar: {e}")).ok()?;
    let carimbo = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or_default();
    let video = pasta.join(format!("shmupx_treino_{carimbo}.mp4"));
    match crate::capture::gravacao::iniciar_do_jogo(&video, Some(pid), steam_app_id).await {
        Ok(gravacao) => Some(gravacao),
        Err(e) => {
            log::warn!("treino: não deu para gravar ({e})");
            let _ = std::fs::remove_file(&video);
            None
        }
    }
}

/// Passa a acompanhar o jogo do treino: quando o processo some, avisa a interface (e fecha a
/// gravação, se houver).
pub fn acompanhar(app: AppHandle, pid: u32, steam_app_id: Option<u32>) {
    treinos().lock().unwrap_or_else(|e| e.into_inner()).insert(pid);
    let gravando = app.clone();
    let gravacao = tauri::async_runtime::spawn(async move { gravar(&gravando, pid, steam_app_id).await });
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let ainda = tauri::async_runtime::spawn_blocking(move || vivo(pid)).await.unwrap_or(false);
            if !ainda {
                treinos().lock().unwrap_or_else(|e| e.into_inner()).remove(&pid);
                let _ = app.emit("treino://encerrado", Encerrado { pid });
                if let Ok(Some(gravacao)) = gravacao.await {
                    if let Err(e) = gravacao.parar().await {
                        log::warn!("treino: falha ao fechar a gravação: {e}");
                    }
                }
                return;
            }
        }
    });
}

/// PARAR JOGO no treino: fecha o jogo (só um processo aberto pelo treino).
#[tauri::command]
pub fn treino_parar(pid: u32) -> Result<(), String> {
    if !treinos().lock().unwrap_or_else(|e| e.into_inner()).contains(&pid) {
        return Err("esse jogo não foi aberto no modo treino".into());
    }
    let mut sistema = System::new();
    sistema.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
    match sistema.process(Pid::from_u32(pid)) {
        Some(processo) => {
            processo.kill();
            Ok(())
        }
        None => Ok(()),
    }
}
