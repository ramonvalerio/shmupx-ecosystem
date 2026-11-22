//! Jogos abertos no MODO TREINO (sem run): o Client lembra o processo para o PARAR JOGO poder
//! fechá-lo e avisa a interface (`treino://encerrado`) quando o jogo fecha por conta própria.
//! Só fecha processos que ele mesmo abriu no treino.

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

/// Passa a acompanhar o jogo do treino: quando o processo some, avisa a interface.
pub fn acompanhar(app: AppHandle, pid: u32) {
    treinos().lock().unwrap_or_else(|e| e.into_inner()).insert(pid);
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let ainda = tauri::async_runtime::spawn_blocking(move || vivo(pid)).await.unwrap_or(false);
            if !ainda {
                treinos().lock().unwrap_or_else(|e| e.into_inner()).remove(&pid);
                let _ = app.emit("treino://encerrado", Encerrado { pid });
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
