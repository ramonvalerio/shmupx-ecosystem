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

/// O processo `pid` ainda existe?
pub fn processo_vivo(pid: u32) -> bool {
    let mut system = System::new();
    let alvo = sysinfo::Pid::from_u32(pid);
    system.refresh_processes(ProcessesToUpdate::Some(&[alvo]), true);
    system.process(alvo).is_some()
}

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

/// Processos com este nome de executável (ex.: `fcadefbneo.exe`, o emulador do Fightcade).
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
