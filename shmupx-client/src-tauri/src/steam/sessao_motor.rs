//! O ShmupX Emulator aberto: um processo só, e cada jogo aberto nele é uma partida.
//!
//! Abrir outro jogo com o motor já aberto não abre outra janela: o Client manda o comando `trocar`
//! pela entrada padrão do motor (plugin `shmupx`), o jogo atual fecha e o novo abre na mesma janela
//! (patch 0013 do ShmupX Engine). Quem assiste à live continua vendo a mesma janela.
//!
//! Partida: o primeiro jogo do processo tem o id = PID do motor; cada jogo trocado ganha um id novo
//! (fora da faixa de PIDs). Tudo que recebe o "PID do jogo" (vigia da run, gravação, treino,
//! telemetria) recebe o id da partida: a partida anterior "fecha" (`processo_vivo` falso) quando o
//! motor avisa o fim dela, e a janela e o som continuam os do processo (`pid_real`).

use std::collections::HashMap;
use std::io::Write;
use std::process::ChildStdin;
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use super::emulador::Lancamento;

/// Ids das partidas trocadas: acima de qualquer PID do Windows.
const PRIMEIRO_ID: u32 = 0x4000_0000;
/// Tempo para o motor responder ao `trocar` (um motor antigo não responde: abre outro processo).
const RESPOSTA: Duration = Duration::from_secs(4);
/// Tempo para o jogo anterior fechar depois do `trocar` aceito.
const FIM_DO_ANTERIOR: Duration = Duration::from_secs(10);
/// Para fechar o motor anterior quando a troca não dá (como o PARAR JOGO).
const PRAZO_PARA_FECHAR: Duration = Duration::from_secs(5);

struct Aberto {
    pid: u32,
    entrada: Option<ChildStdin>,
    dupla: bool,
    /// Partida que está na tela.
    atual: u32,
    /// Partida pedida no `trocar`, até o motor fechar a atual.
    pendente: Option<u32>,
    /// Resposta do motor ao `trocar` (evento `switch`).
    resposta: Option<Result<(), String>>,
}

#[derive(Default)]
struct Estado {
    aberto: Option<Aberto>,
    /// Partida -> PID do motor dela (também as que já fecharam).
    pids: HashMap<u32, u32>,
    proximo: u32,
}

fn estado() -> &'static (Mutex<Estado>, Condvar) {
    static ESTADO: OnceLock<(Mutex<Estado>, Condvar)> = OnceLock::new();
    ESTADO.get_or_init(Default::default)
}

fn travar() -> MutexGuard<'static, Estado> {
    estado().0.lock().unwrap_or_else(|p| p.into_inner())
}

fn avisar() {
    estado().1.notify_all();
}

/// O motor `pid` acabou de abrir (com a entrada padrão dele, se houver).
pub fn registrar(pid: u32, entrada: Option<ChildStdin>, dupla: bool) {
    let mut e = travar();
    e.pids.insert(pid, pid);
    e.aberto = Some(Aberto { pid, entrada, dupla, atual: pid, pendente: None, resposta: None });
}

/// PID do motor da partida `id` (`None`: não é uma partida do motor).
pub fn pid_real(id: u32) -> Option<u32> {
    travar().pids.get(&id).copied()
}

/// A partida `id` ainda está na tela? `None`: não é uma partida do motor (vale o processo).
pub fn partida_atual(id: u32) -> Option<bool> {
    let e = travar();
    let pid = *e.pids.get(&id)?;
    Some(e.aberto.as_ref().is_some_and(|a| a.pid == pid && a.atual == id))
}

/// Partida que recebe agora a telemetria do motor `pid`.
pub fn partida_do_motor(pid: u32) -> u32 {
    travar().aberto.as_ref().filter(|a| a.pid == pid).map_or(pid, |a| a.atual)
}

/// O motor respondeu ao `trocar` (evento `switch`).
pub fn resposta(pid: u32, resultado: Result<(), String>) {
    let mut e = travar();
    if let Some(a) = e.aberto.as_mut().filter(|a| a.pid == pid && a.pendente.is_some()) {
        a.resposta = Some(resultado);
    }
    drop(e);
    avisar();
}

/// O jogo atual do motor `pid` fechou (evento `stop`). Numa troca, a partida pedida passa a ser a
/// atual: devolve (anterior, nova).
pub fn fim_do_jogo(pid: u32) -> Option<(u32, u32)> {
    let mut e = travar();
    let a = e.aberto.as_mut().filter(|a| a.pid == pid && matches!(a.resposta, Some(Ok(()))))?;
    let nova = a.pendente.take()?;
    let anterior = std::mem::replace(&mut a.atual, nova);
    a.resposta = None;
    drop(e);
    avisar();
    Some((anterior, nova))
}

/// O jogo do motor `pid` fechou sem troca pendente (a janela fechou, PARAR JOGO, fim do ranked):
/// fecha a entrada padrão dele. A leitura de comandos do plugin fica parada nela, com a trava do
/// arquivo, e a saída do processo espera essa trava: com a entrada aberta, o motor fechava a janela
/// mas o processo continuava vivo, e o Client seguia mostrando a partida em andamento.
pub fn jogo_fechou(pid: u32) {
    let mut e = travar();
    if let Some(a) = e.aberto.as_mut().filter(|a| a.pid == pid && a.pendente.is_none()) {
        a.entrada = None;
    }
}

/// Pede ao motor `pid` para fechar o jogo pela entrada padrão (comando `sair` do plugin), como o X
/// da janela: o motor sai normalmente e o fim da partida é registrado. É o que fecha o jogo quando
/// não há janela no PC (TV pelo MiSTer). `false`: sem motor aberto com a entrada (um motor antigo
/// ignora o comando; quem chama segue com a janela e, por fim, encerra o processo).
pub fn pedir_saida(pid: u32) -> bool {
    let mut e = travar();
    let Some(a) = e.aberto.as_mut().filter(|a| a.pid == pid) else { return false };
    let Some(entrada) = a.entrada.as_mut() else { return false };
    entrada.write_all(b"sair
").and_then(|_| entrada.flush()).is_ok()
}

/// O motor `pid` fechou (fim da saída padrão).
pub fn encerrado(pid: u32) {
    let mut e = travar();
    if e.aberto.as_ref().is_some_and(|a| a.pid == pid) {
        e.aberto = None;
    }
    drop(e);
    avisar();
}

/// Troca o jogo no motor aberto (mesma janela). `Ok(Some((nova, anterior)))`: trocou, o jogo
/// anterior já fechou; `Ok(None)`: não há motor aberto que aceite (abre um processo novo).
pub fn trocar(lancamento: &Lancamento) -> Result<Option<(u32, u32)>, String> {
    let Some(linha) = lancamento.comando_trocar() else { return Ok(None) };
    let aviso = &estado().1;
    let mut e = travar();
    let Some(pid) = e.aberto.as_ref().filter(|a| !a.dupla && a.entrada.is_some() && a.pendente.is_none()).map(|a| a.pid) else { return Ok(None) };
    if !super::detector::processo_vivo_real(pid) {
        e.aberto = None;
        return Ok(None);
    }
    e.proximo = e.proximo.max(PRIMEIRO_ID) + 1;
    let nova = e.proximo;
    let Some(a) = e.aberto.as_mut() else { return Ok(None) };
    let enviado = a.entrada.as_mut().map(|entrada| entrada.write_all(linha.as_bytes()).and_then(|_| entrada.flush()));
    if !matches!(enviado, Some(Ok(()))) {
        a.entrada = None;
        return Ok(None);
    }
    a.pendente = Some(nova);
    a.resposta = None;
    e.pids.insert(nova, pid);

    // A resposta do motor (um motor antigo não lê a entrada e não responde).
    let limite = Instant::now() + RESPOSTA;
    while e.aberto.as_ref().is_some_and(|a| a.pid == pid && a.pendente == Some(nova) && a.resposta.is_none()) && Instant::now() < limite {
        e = aviso.wait_timeout(e, limite.saturating_duration_since(Instant::now())).unwrap_or_else(|p| p.into_inner()).0;
    }
    let resposta = e.aberto.as_ref().filter(|a| a.pid == pid).and_then(|a| a.resposta.clone());
    match resposta {
        Some(Ok(())) => {}
        outra => {
            if let Some(a) = e.aberto.as_mut().filter(|a| a.pid == pid) {
                a.pendente = None;
                a.resposta = None;
                // Sem resposta: o motor não sabe trocar; não adianta tentar de novo.
                if outra.is_none() {
                    a.entrada = None;
                }
            }
            e.pids.remove(&nova);
            return match outra {
                Some(Err(motivo)) => Err(format!("o ShmupX Emulator recusou a troca ({motivo})")),
                _ => Ok(None),
            };
        }
    }

    // O jogo anterior fecha e o novo abre: espera a partida nova ser a atual.
    let anterior = e.aberto.as_ref().map_or(pid, |a| a.atual);
    let limite = Instant::now() + FIM_DO_ANTERIOR;
    while e.aberto.as_ref().is_some_and(|a| a.pid == pid && a.atual != nova) && Instant::now() < limite {
        e = aviso.wait_timeout(e, limite.saturating_duration_since(Instant::now())).unwrap_or_else(|p| p.into_inner()).0;
    }
    match e.aberto.as_ref().filter(|a| a.pid == pid) {
        Some(a) if a.atual == nova => Ok(Some((nova, anterior))),
        Some(_) => Err("o jogo anterior não fechou a tempo".to_string()),
        None => Err("o ShmupX Emulator fechou durante a troca de jogo".to_string()),
    }
}

/// Fecha o motor aberto (antes de abrir outro processo): um ShmupX Emulator por vez.
pub fn fechar_aberto() {
    let pid = travar().aberto.as_ref().map(|a| a.pid);
    if let Some(pid) = pid.filter(|pid| super::detector::processo_vivo_real(*pid)) {
        log::info!("emulador: fechando o ShmupX Emulator anterior ({pid}) para abrir outro");
        super::detector::fechar_jogo(pid, PRAZO_PARA_FECHAR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O estado é global: um teste só percorre todo o ciclo.
    #[test]
    fn partidas_do_mesmo_motor() {
        registrar(4242, None, false);
        assert_eq!(partida_do_motor(4242), 4242);
        assert_eq!(partida_atual(4242), Some(true));
        assert_eq!(partida_atual(77), None, "outro processo não é partida do motor");
        // Troca aceita: a partida nova passa a ser a atual quando o jogo anterior fecha.
        {
            let mut e = travar();
            let a = e.aberto.as_mut().unwrap();
            a.pendente = Some(PRIMEIRO_ID + 1);
            e.pids.insert(PRIMEIRO_ID + 1, 4242);
        }
        assert_eq!(fim_do_jogo(4242), None, "sem a resposta do motor, o fim não troca a partida");
        resposta(4242, Ok(()));
        assert_eq!(fim_do_jogo(4242), Some((4242, PRIMEIRO_ID + 1)));
        assert_eq!(partida_atual(4242), Some(false), "a partida anterior fechou");
        assert_eq!(partida_atual(PRIMEIRO_ID + 1), Some(true));
        assert_eq!(pid_real(PRIMEIRO_ID + 1), Some(4242));
        assert_eq!(partida_do_motor(4242), PRIMEIRO_ID + 1);
        encerrado(4242);
        assert_eq!(partida_atual(PRIMEIRO_ID + 1), Some(false), "motor fechado");
    }
}

#[cfg(all(test, windows))]
mod neste_pc {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use crate::steam::detector::{fechar_jogo, processo_vivo};
    use crate::steam::emulador::{abrir, Motor, VideoDoMotor};

    fn janela(id: u32) -> Option<(usize, String)> {
        let w = crate::live::janela::janela_dos_processos(&[id])?;
        Some((w.as_raw_hwnd() as usize, w.title().unwrap_or_default()))
    }

    fn esperar_janela(id: u32) -> (usize, String) {
        let limite = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(j) = janela(id) {
                return j;
            }
            assert!(Instant::now() < limite, "a janela do motor não apareceu");
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// Troca de jogo no ShmupX Emulator aberto, com o motor e os ROMs deste PC:
    /// `SHMUPX_TESTE_MOTOR=<pasta do shmupx-engine.exe> SHMUPX_TESTE_ROMS=<pasta de ROMs>
    /// cargo test --lib -- --ignored troca_de_jogo_na_mesma_janela --nocapture`.
    #[test]
    #[ignore = "abre o ShmupX Emulator deste PC"]
    fn troca_de_jogo_na_mesma_janela() {
        let pasta = PathBuf::from(std::env::var("SHMUPX_TESTE_MOTOR").expect("SHMUPX_TESTE_MOTOR"));
        let roms = std::env::var("SHMUPX_TESTE_ROMS").expect("SHMUPX_TESTE_ROMS");
        let video = VideoDoMotor { scanlines: false, ..VideoDoMotor::default() };
        let motor = Motor {
            exe: pasta.join("shmupx-engine.exe"),
            dados: std::env::temp_dir().join("shmupx-teste-troca"),
            ranking: false,
            video,
            controles: vec![],
            caravan_segundos: None,
            cartao_jogador: None,
            posicao_arquivo: None,
            idioma: None,
            dupla: None,
            sistema: None,
        };
        let primeiro = abrir(Some(&roms), None, None, &["sonicwi2".into()], true, Some(&motor)).expect("primeiro jogo");
        let (hwnd1, titulo1) = esperar_janela(primeiro.id);
        println!("1º: partida {} rom {} janela {hwnd1:#x} {titulo1:?}", primeiro.id, primeiro.rom);
        std::thread::sleep(Duration::from_secs(4));

        let segundo = abrir(Some(&roms), None, None, &["ket".into()], true, Some(&motor)).expect("segundo jogo");
        println!("2º: partida {} rom {} trocou de {:?}", segundo.id, segundo.rom, segundo.trocou_de);
        assert_eq!(segundo.trocou_de, Some(primeiro.id), "trocou no motor aberto");
        assert!(!processo_vivo(primeiro.id), "a partida anterior fechou");
        assert!(processo_vivo(segundo.id), "a partida nova está na tela");
        let (hwnd2, titulo2) = esperar_janela(segundo.id);
        println!("2º: janela {hwnd2:#x} {titulo2:?}");
        std::thread::sleep(Duration::from_secs(4));
        let (hwnd3, titulo3) = esperar_janela(segundo.id);
        println!("2º, depois: janela {hwnd3:#x} {titulo3:?}");
        assert_eq!(hwnd1, hwnd3, "a mesma janela");
        assert_ne!(titulo1, titulo3, "o título é o do jogo novo");

        // Ranked na mesma janela (com o estado inicial do jogo no pacote do motor).
        let ranked = Motor { ranking: true, ..motor };
        match abrir(Some(&roms), None, None, &["sonicwi2".into()], true, Some(&ranked)) {
            Ok(terceiro) => {
                println!("3º (ranked): partida {} trocou de {:?}", terceiro.id, terceiro.trocou_de);
                assert_eq!(terceiro.trocou_de, Some(segundo.id));
                std::thread::sleep(Duration::from_secs(4));
                assert_eq!(esperar_janela(terceiro.id).0, hwnd1, "a mesma janela no ranked");
                fechar_jogo(terceiro.id, Duration::from_secs(5));
            }
            Err(e) => {
                println!("3º (ranked): {e}");
                fechar_jogo(segundo.id, Duration::from_secs(5));
            }
        }
    }
}
