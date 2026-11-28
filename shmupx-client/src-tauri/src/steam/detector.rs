use std::collections::HashSet;

use sysinfo::{ProcessesToUpdate, System};

/// Jogo Steam detectado em execução.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedGame {
    pub app_id: u32,
    pub name: String,
    pub pid: u32,
}

/// Porta do domínio: quem sabe dizer qual jogo está ativo.
/// A implementação concreta (varredura de processos casada com manifestos `.acf`) vem em
/// TASKS.md 1.2–1.4; hoje só a varredura bruta de processos existe (abaixo).
pub trait GameDetector: Send + Sync {
    fn current_game(&self) -> Option<DetectedGame>;
}

const STEAM_PROCESS_NAMES: &[&str] = &["steam.exe", "steam"];

/// Verifica se o cliente oficial da Steam está em execução no sistema do jogador.
pub fn is_steam_running() -> bool {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);

    system.processes().values().any(|process| {
        let name = process.name().to_string_lossy();
        STEAM_PROCESS_NAMES
            .iter()
            .any(|candidate| name.eq_ignore_ascii_case(candidate))
    })
}

/// O jogo `pid` ainda está aberto? No ShmupX Emulator, `pid` é a partida (`sessao_motor`): a
/// partida de um jogo trocado no mesmo processo fecha quando o jogo seguinte abre.
pub fn processo_vivo(pid: u32) -> bool {
    match super::sessao_motor::partida_atual(pid) {
        Some(false) => false,
        Some(true) => super::sessao_motor::pid_real(pid).is_some_and(processo_vivo_real),
        None => processo_vivo_real(pid),
    }
}

/// O processo `pid` ainda existe?
pub fn processo_vivo_real(pid: u32) -> bool {
    let mut system = System::new();
    let alvo = sysinfo::Pid::from_u32(pid);
    system.refresh_processes(ProcessesToUpdate::Some(&[alvo]), true);
    system.process(alvo).is_some()
}

/// Fecha o jogo como o X da janela: pede para as janelas do processo fecharem (`WM_CLOSE`; o
/// ShmupX Engine sai normalmente e a telemetria registra o fim da partida) e, se ele não fechar em
/// `prazo`, encerra o processo. Bloqueia até o jogo sair (use fora da thread assíncrona).
pub fn fechar_jogo(pid: u32, prazo: std::time::Duration) {
    // Partida do ShmupX Emulator: só se ainda for o jogo na tela (fecha o motor dela).
    if super::sessao_motor::partida_atual(pid) == Some(false) {
        return;
    }
    let pid = super::sessao_motor::pid_real(pid).unwrap_or(pid);
    // ShmupX Emulator: o comando `sair` pela entrada padrão (funciona sem janela, como na TV pelo
    // MiSTer); a janela recebe o pedido também, para o motor antigo que não conhece o comando.
    super::sessao_motor::pedir_saida(pid);
    pedir_para_fechar(pid);
    let limite = std::time::Instant::now() + prazo;
    while std::time::Instant::now() < limite {
        if !processo_vivo_real(pid) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let mut system = System::new();
    let alvo = sysinfo::Pid::from_u32(pid);
    system.refresh_processes(ProcessesToUpdate::Some(&[alvo]), true);
    if let Some(processo) = system.process(alvo) {
        log::warn!("o jogo {pid} não fechou em {}s: encerrando o processo", prazo.as_secs());
        processo.kill();
    }
}

#[cfg(windows)]
fn pedir_para_fechar(pid: u32) {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE};

    unsafe extern "system" fn cada_janela(janela: HWND, alvo: LPARAM) -> BOOL {
        let mut dono = 0u32;
        unsafe { GetWindowThreadProcessId(janela, Some(&mut dono)) };
        if dono == alvo.0 as u32 {
            let _ = unsafe { PostMessageW(Some(janela), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
        BOOL(1)
    }
    let _ = unsafe { EnumWindows(Some(cada_janela), LPARAM(pid as isize)) };
}

#[cfg(not(windows))]
fn pedir_para_fechar(_pid: u32) {}

/// Algum processo rodando de dentro da pasta do jogo (o jogo de verdade, quando quem abriu foi
/// um launcher que já fechou).
pub fn processo_na_pasta(pasta: &std::path::Path) -> Option<u32> {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let pasta = pasta.to_string_lossy().to_lowercase();
    system
        .processes()
        .iter()
        .find(|(_, p)| p.exe().is_some_and(|exe| exe.to_string_lossy().to_lowercase().starts_with(pasta.as_str())))
        .map(|(pid, _)| pid.as_u32())
}

/// Todos os processos rodando de dentro da pasta do jogo.
pub fn processos_na_pasta(pasta: &std::path::Path) -> Vec<u32> {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let pasta = pasta.to_string_lossy().to_lowercase();
    system
        .processes()
        .iter()
        .filter(|(_, p)| p.exe().is_some_and(|exe| exe.to_string_lossy().to_lowercase().starts_with(pasta.as_str())))
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

/// Processos com este nome de executável (ex.: `shmupx-engine.exe`, o ShmupX Emulator).
pub fn processos_por_nome(nome: &str) -> Vec<u32> {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
        .processes()
        .iter()
        .filter(|(_, p)| p.name().to_string_lossy().eq_ignore_ascii_case(nome))
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

/// Snapshot dos PIDs ativos no sistema, usado como baseline para detectar o surgimento
/// do processo do jogo após o disparo via `steam://run/<app_id>`.
pub fn snapshot_pids() -> HashSet<u32> {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system.processes().keys().map(|pid| pid.as_u32()).collect()
}

/// Heurística inicial (stub): procura por qualquer processo que não existia no `baseline`.
///
/// TODO(TASKS.md 1.2–1.4): trocar por casamento exato via `appmanifest_*.acf` (installdir →
/// app_id) para não confundir o jogo com qualquer outro processo iniciado no intervalo.
pub fn find_new_process(baseline: &HashSet<u32>, pasta_do_jogo: Option<&std::path::Path>) -> Option<(u32, String)> {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let pasta = pasta_do_jogo.map(|p| p.to_string_lossy().to_lowercase());
    system
        .processes()
        .iter()
        .filter(|(pid, _)| !baseline.contains(&pid.as_u32()))
        .find(|(_, process)| match &pasta {
            // Com a pasta do jogo (biblioteca da Steam): só um executável de dentro dela.
            Some(pasta) => process.exe().is_some_and(|exe| exe.to_string_lossy().to_lowercase().starts_with(pasta.as_str())),
            // Sem ela: qualquer processo novo que não seja da própria Steam (que pode estar abrindo).
            None => {
                let nome = process.name().to_string_lossy().to_lowercase();
                !nome.starts_with("steam") && !nome.starts_with("gameoverlay") && !nome.contains("crashhandler")
            }
        })
        .map(|(pid, process)| (pid.as_u32(), process.name().to_string_lossy().into_owned()))
}
