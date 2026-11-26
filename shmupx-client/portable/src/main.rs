//! ShmupX Portable: o jogador dá dois cliques e o ShmupX Client abre, sem instalar nada.
//!
//! Na primeira vez (e a cada versão nova), o lançador descompacta o Client que vem anexado ao
//! próprio `.exe` (ver `lib.rs`) em `%LOCALAPPDATA%\ShmupX Portable\app\<versão>`, mostrando o
//! progresso; nas seguintes, abre direto. Se já houver uma versão igual ou mais nova descompactada
//! (atualizada pelo próprio Client), abre ela: um `.exe` antigo nunca volta o jogador de versão.
//!
//! ```text
//! %LOCALAPPDATA%\ShmupX Portable\
//!   app\<versão>\     o Client (exe, runtime, engine, validator, games, ffmpeg)
//!   roms\, recordings\  do jogador: ficam de uma versão para a outra (o Client cria)
//!   portable.log      detalhes técnicos de falhas do lançador
//! ```
//!
//! Também confere o WebView2 (a "janela" do Client): Windows 11 já tem; se faltar, instala pelo
//! instalador oficial da Microsoft que vem no pacote.
#![cfg_attr(not(test), windows_subsystem = "windows")]

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use shmupx_portable::{PRONTO, Pacote, Versao};

/// Falha mostrada ao jogador (`texto`, em linguagem simples) com o detalhe só no log.
struct Falha {
    texto: Option<String>,
    tecnico: String,
}

impl Falha {
    fn nova(texto: String, tecnico: impl std::fmt::Display) -> Self {
        Falha { texto: Some(texto), tecnico: tecnico.to_string() }
    }
    /// O jogador cancelou: sai sem mensagem.
    fn cancelada() -> Self {
        Falha { texto: None, tecnico: "cancelado pelo jogador".into() }
    }
    /// Erro de disco: espaço cheio ganha mensagem própria.
    fn disco(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::StorageFull {
            return Falha::nova(
                t(
                    "Não há espaço livre suficiente no disco.\n\nO ShmupX precisa de cerca de 1 GB livre no disco C:. Libere espaço e abra o ShmupX de novo.",
                    "There is not enough free disk space.\n\nShmupX needs about 1 GB free on drive C:. Free up some space and open ShmupX again.",
                ),
                e,
            );
        }
        Falha::nova(
            t(
                "Não foi possível preparar o ShmupX nesta pasta do Windows.\n\nReinicie o computador e abra o ShmupX de novo.",
                "ShmupX could not be prepared on this computer.\n\nRestart the computer and open ShmupX again.",
            ),
            e,
        )
    }
    fn pacote_corrompido(e: impl std::fmt::Display) -> Self {
        Falha::nova(
            t(
                "Este arquivo do ShmupX Portable está incompleto (o download pode ter sido interrompido).\n\nBaixe de novo em shmupx.com/download e abra o arquivo novo.",
                "This ShmupX Portable file is incomplete (the download may have been interrupted).\n\nDownload it again from shmupx.com/download and open the new file.",
            ),
            e,
        )
    }
}

impl From<io::Error> for Falha {
    fn from(e: io::Error) -> Self {
        Falha::disco(e)
    }
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if let Err(falha) = rodar(args) {
        registrar(&falha.tecnico);
        if let Some(texto) = falha.texto {
            janela::mensagem(&texto);
        }
    }
}

/// `%LOCALAPPDATA%\ShmupX Portable`.
fn base() -> Result<PathBuf, Falha> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or_else(|| Falha::disco(io::Error::other("LOCALAPPDATA não definido")))?;
    Ok(PathBuf::from(local).join("ShmupX Portable"))
}

fn registrar(linha: &str) {
    let Ok(base) = base() else { return };
    let _ = fs::create_dir_all(&base);
    if let Ok(mut log) = fs::OpenOptions::new().create(true).append(true).open(base.join("portable.log")) {
        let agora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(log, "[{agora}] {linha}");
    }
}

fn rodar(args: Vec<OsString>) -> Result<(), Falha> {
    let exe = std::env::current_exe()?;
    let pacote = Pacote::abrir(&exe).map_err(Falha::pacote_corrompido)?;
    let apps = base()?.join("app");
    fs::create_dir_all(&apps)?;
    // Dois cliques duplos seguidos: o segundo espera o primeiro terminar de descompactar.
    let _trava = janela::Trava::pegar();
    let pasta = match mais_nova_pronta(&apps) {
        Some((versao, pasta)) if versao >= pacote.versao => pasta,
        _ => extrair(&pacote, &apps)?,
    };
    limpar_antigas(&apps, &pasta);
    garantir_webview2(&pasta)?;
    Command::new(pasta.join("shmupx-client.exe")).args(args).current_dir(&pasta).spawn().map_err(|e| {
        Falha::nova(
            t(
                "O ShmupX não conseguiu abrir.\n\nReinicie o computador e tente de novo. Se continuar, baixe o ShmupX Portable de novo em shmupx.com/download.",
                "ShmupX could not start.\n\nRestart the computer and try again. If it keeps happening, download ShmupX Portable again from shmupx.com/download.",
            ),
            format!("abrir {}: {e}", pasta.display()),
        )
    })?;
    Ok(())
}

/// Versões já descompactadas e completas, da mais nova para a mais antiga.
fn prontas(apps: &Path) -> Vec<(Versao, PathBuf)> {
    let mut lista: Vec<(Versao, PathBuf)> = fs::read_dir(apps)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entrada| {
            let pasta = entrada.path();
            let versao = Versao::ler(&entrada.file_name().to_string_lossy())?;
            (pasta.join(PRONTO).is_file() && pasta.join("shmupx-client.exe").is_file()).then_some((versao, pasta))
        })
        .collect();
    lista.sort_by(|a, b| b.0.cmp(&a.0));
    lista
}

fn mais_nova_pronta(apps: &Path) -> Option<(Versao, PathBuf)> {
    prontas(apps).into_iter().next()
}

/// Apaga versões mais antigas que a que vai abrir e restos de extrações interrompidas. Renomeia
/// antes de apagar: o Windows recusa renomear a pasta de um Client antigo ainda aberto, e então
/// ela fica intacta para a próxima vez (apagar direto tiraria arquivos do app em uso).
fn limpar_antigas(apps: &Path, atual: &Path) {
    for entrada in fs::read_dir(apps).into_iter().flatten().flatten() {
        let pasta = entrada.path();
        if pasta == atual || !pasta.is_dir() {
            continue;
        }
        let nome = entrada.file_name().to_string_lossy().into_owned();
        if nome.ends_with(".extraindo") || nome.ends_with(".apagando") {
            let _ = fs::remove_dir_all(&pasta);
        } else if Versao::ler(&nome).is_some() {
            let lixo = apps.join(format!("{nome}.apagando"));
            if fs::rename(&pasta, &lixo).is_ok() {
                let _ = fs::remove_dir_all(&lixo);
            }
        }
    }
}

fn extrair(pacote: &Pacote, apps: &Path) -> Result<PathBuf, Falha> {
    let destino = apps.join(pacote.versao.to_string());
    let temporaria = apps.join(format!("{}.extraindo", pacote.versao));
    let _ = fs::remove_dir_all(&temporaria);
    if destino.exists() {
        // Extração antiga incompleta (sem `.pronto`).
        fs::remove_dir_all(&destino)?;
    }
    let progresso = janela::Progresso::abrir(
        &t("Preparando o ShmupX…", "Preparing ShmupX…"),
        &t("Só na primeira vez de cada versão: leva alguns segundos.", "Only the first time for each version: it takes a few seconds."),
        false,
    );
    let resultado = descompactar(pacote, &temporaria, |feito, total| {
        if let Some(p) = &progresso {
            p.avancar(feito, total);
            if p.cancelado() {
                return false;
            }
        }
        true
    });
    drop(progresso);
    if let Err(falha) = resultado {
        let _ = fs::remove_dir_all(&temporaria);
        return Err(falha);
    }
    File::create(temporaria.join(PRONTO))?.write_all(pacote.versao.to_string().as_bytes())?;
    fs::rename(&temporaria, &destino)?;
    registrar(&format!("versão {} descompactada em {}", pacote.versao, destino.display()));
    Ok(destino)
}

/// Descompacta o pacote em `destino`, chamando `continuar(feito, total)` a cada pedaço; `false` cancela.
fn descompactar(pacote: &Pacote, destino: &Path, mut continuar: impl FnMut(u64, u64) -> bool) -> Result<(), Falha> {
    let mut zip = zip::ZipArchive::new(pacote.zip()?).map_err(Falha::pacote_corrompido)?;
    let total: u64 = (0..zip.len()).filter_map(|i| zip.by_index_raw(i).ok().map(|f| f.size())).sum();
    let mut feito = 0u64;
    let mut buffer = vec![0u8; 1 << 20];
    for i in 0..zip.len() {
        let mut item = zip.by_index(i).map_err(Falha::pacote_corrompido)?;
        let Some(relativo) = item.enclosed_name() else { continue };
        let caminho = destino.join(relativo);
        if item.is_dir() {
            fs::create_dir_all(&caminho)?;
            continue;
        }
        if let Some(pai) = caminho.parent() {
            fs::create_dir_all(pai)?;
        }
        let mut saida = io::BufWriter::new(File::create(&caminho)?);
        loop {
            // Erro de leitura aqui é o zip (CRC/dados), não o disco.
            let n = item.read(&mut buffer).map_err(Falha::pacote_corrompido)?;
            if n == 0 {
                break;
            }
            saida.write_all(&buffer[..n])?;
            feito += n as u64;
            if !continuar(feito, total) {
                return Err(Falha::cancelada());
            }
        }
        saida.flush()?;
    }
    Ok(())
}

fn garantir_webview2(pasta: &Path) -> Result<(), Falha> {
    if janela::webview2_instalado() {
        return Ok(());
    }
    let instalador = pasta.join("webview2").join("MicrosoftEdgeWebview2Setup.exe");
    let progresso = janela::Progresso::abrir(
        &t("Instalando um componente do Windows (WebView2)…", "Installing a Windows component (WebView2)…"),
        &t("O ShmupX precisa dele para abrir. Só acontece uma vez e usa a internet.", "ShmupX needs it to open. It only happens once and uses the internet."),
        true,
    );
    let resultado = Command::new(&instalador).args(["/silent", "/install"]).status();
    drop(progresso);
    if janela::webview2_instalado() {
        registrar("WebView2 instalado");
        return Ok(());
    }
    Err(Falha::nova(
        t(
            "O ShmupX precisa do Microsoft Edge WebView2, que não pôde ser instalado agora.\n\nConfira se o computador está conectado à internet e abra o ShmupX de novo.",
            "ShmupX needs Microsoft Edge WebView2, which could not be installed now.\n\nCheck that the computer is connected to the internet and open ShmupX again.",
        ),
        format!("WebView2: {} -> {resultado:?}", instalador.display()),
    ))
}

/// Texto no idioma do Windows: português se o Windows estiver em português, senão inglês.
fn t(pt: &str, en: &str) -> String {
    if janela::em_portugues() { pt } else { en }.to_string()
}

#[cfg(windows)]
mod janela {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Globalization::GetUserDefaultUILanguage;
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    use windows::Win32::System::Threading::{CreateMutexW, INFINITE, ReleaseMutex, WaitForSingleObject};
    use windows::Win32::UI::Shell::{
        CLSID_ProgressDialog, IProgressDialog, PROGDLG_AUTOTIME, PROGDLG_MARQUEEPROGRESS, PROGDLG_NOCANCEL, PROGDLG_NOMINIMIZE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW};
    use windows::core::{HSTRING, w};

    pub fn em_portugues() -> bool {
        // LANG_PORTUGUESE (pt-BR e pt-PT).
        (unsafe { GetUserDefaultUILanguage() } & 0x3ff) == 0x16
    }

    pub fn mensagem(texto: &str) {
        unsafe {
            MessageBoxW(None, &HSTRING::from(texto), w!("ShmupX"), MB_OK | MB_ICONERROR | MB_SETFOREGROUND);
        }
    }

    /// Mutex nomeado da sessão: um lançador por vez.
    pub struct Trava(HANDLE);

    impl Trava {
        pub fn pegar() -> Option<Self> {
            unsafe {
                let mutex = CreateMutexW(None, false, w!("Local\\ShmupXPortable")).ok()?;
                WaitForSingleObject(mutex, INFINITE);
                Some(Trava(mutex))
            }
        }
    }

    impl Drop for Trava {
        fn drop(&mut self) {
            unsafe {
                let _ = ReleaseMutex(self.0);
                let _ = CloseHandle(self.0);
            }
        }
    }

    /// Janela de progresso padrão do Windows (a mesma de copiar arquivos no Explorer).
    pub struct Progresso(IProgressDialog);

    impl Progresso {
        /// `continuo`: barra animada sem porcentagem e sem Cancelar (instalação de terceiros).
        pub fn abrir(titulo: &str, detalhe: &str, continuo: bool) -> Option<Self> {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let dialogo: IProgressDialog = CoCreateInstance(&CLSID_ProgressDialog, None, CLSCTX_INPROC_SERVER).ok()?;
                dialogo.SetTitle(w!("ShmupX")).ok()?;
                dialogo.SetLine(1, &HSTRING::from(titulo), false, None).ok()?;
                dialogo.SetLine(2, &HSTRING::from(detalhe), false, None).ok()?;
                let modo = if continuo { PROGDLG_MARQUEEPROGRESS | PROGDLG_NOCANCEL } else { PROGDLG_AUTOTIME };
                dialogo.StartProgressDialog(None, None, modo | PROGDLG_NOMINIMIZE, None).ok()?;
                Some(Progresso(dialogo))
            }
        }

        pub fn avancar(&self, feito: u64, total: u64) {
            unsafe {
                let _ = self.0.SetProgress64(feito, total.max(1));
            }
        }

        pub fn cancelado(&self) -> bool {
            unsafe { self.0.HasUserCancelled().as_bool() }
        }
    }

    impl Drop for Progresso {
        fn drop(&mut self) {
            unsafe {
                let _ = self.0.StopProgressDialog();
            }
        }
    }

    /// WebView2 Runtime instalado (para todos ou só para o usuário), como a Microsoft documenta.
    pub fn webview2_instalado() -> bool {
        const CHAVES: [(windows::Win32::System::Registry::HKEY, &str); 2] = [
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"),
            (HKEY_CURRENT_USER, r"Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"),
        ];
        CHAVES.iter().any(|(raiz, chave)| {
            let mut buffer = [0u16; 64];
            let mut tamanho = (buffer.len() * 2) as u32;
            let ok = unsafe {
                RegGetValueW(*raiz, &HSTRING::from(*chave), w!("pv"), RRF_RT_REG_SZ, None, Some(buffer.as_mut_ptr().cast()), Some(&mut tamanho))
            };
            if ok.is_err() {
                return false;
            }
            let versao = String::from_utf16_lossy(&buffer[..(tamanho as usize / 2).saturating_sub(1)]);
            !versao.is_empty() && versao != "0.0.0.0"
        })
    }
}

/// Fora do Windows o lançador não tem uso; isto só deixa o workspace compilar no Linux.
#[cfg(not(windows))]
mod janela {
    pub fn em_portugues() -> bool {
        false
    }
    pub fn mensagem(texto: &str) {
        eprintln!("{texto}");
    }
    pub struct Trava;
    impl Trava {
        pub fn pegar() -> Option<Self> {
            Some(Trava)
        }
    }
    pub struct Progresso;
    impl Progresso {
        pub fn abrir(_: &str, _: &str, _: bool) -> Option<Self> {
            None
        }
        pub fn avancar(&self, _: u64, _: u64) {}
        pub fn cancelado(&self) -> bool {
            false
        }
    }
    pub fn webview2_instalado() -> bool {
        true
    }
}
