//! Antes da janela abrir (Windows): o que impediria o Client de aparecer vira uma mensagem simples
//! para o jogador, em vez de o programa fechar sem mostrar nada.
//!
//! - Sem o Microsoft Edge WebView2 (a interface do Client roda nele): o instalador do Client baixa o
//!   WebView2 durante a instalação, mas sem internet, com proxy ou antivírus esse passo falha em
//!   silêncio e o Client não abria. Agora ele avisa e instala na hora com o instalador oficial da
//!   Microsoft que vai junto (`webview2/MicrosoftEdgeWebview2Setup.exe`); se não der, mostra o passo
//!   a passo e abre a página oficial.
//! - Falha ao abrir (panic): grava o motivo em `logs/ShmupX-falha.txt` e avisa o jogador. Depois que
//!   o Client abriu, um panic só vai para esse arquivo (o log normal continua com o resto).

use std::sync::atomic::{AtomicBool, Ordering};

const TITULO: &str = "ShmupX Client";

/// O Client ainda está abrindo (falha aqui = nenhuma janela na tela).
static ABRINDO: AtomicBool = AtomicBool::new(true);

/// Pasta dos logs do Client (a mesma do log normal), antes de o Tauri existir.
fn pasta_dos_logs() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let pasta = std::path::Path::new(&base).join("com.shmupx.client").join("logs");
    std::fs::create_dir_all(&pasta).ok()?;
    Some(pasta)
}

fn gravar_falha(texto: &str) -> Option<std::path::PathBuf> {
    use std::io::Write;
    let arquivo = pasta_dos_logs()?.join("ShmupX-falha.txt");
    let quando = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&arquivo).ok()?;
    writeln!(f, "[{quando}] versão {}: {texto}", env!("CARGO_PKG_VERSION")).ok()?;
    Some(arquivo)
}

#[cfg(windows)]
mod janela {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MESSAGEBOX_RESULT, MESSAGEBOX_STYLE};
    pub use windows::Win32::UI::WindowsAndMessaging::{IDOK, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_OKCANCEL, MB_SETFOREGROUND, MB_TOPMOST};

    pub fn mensagem(texto: &str, estilo: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
        unsafe { MessageBoxW(None, &HSTRING::from(texto), &HSTRING::from(super::TITULO), estilo | MB_SETFOREGROUND | MB_TOPMOST) }
    }
}

/// Falha que impede o Client de abrir: grava o motivo e avisa o jogador com o que fazer.
pub fn falha_ao_abrir(motivo: &str) {
    let arquivo = gravar_falha(motivo);
    #[cfg(windows)]
    {
        let onde = arquivo.map(|a| a.display().to_string()).unwrap_or_else(|| "(não foi possível gravar o arquivo)".into());
        janela::mensagem(
            &format!(
                "O ShmupX não conseguiu abrir.\n\n1. Reinicie o computador e abra o ShmupX de novo.\n2. Se continuar, reinstale o ShmupX pelo site shmupx.com.\n3. Se ainda assim não abrir, mande para o suporte o arquivo:\n{onde}"
            ),
            janela::MB_OK | janela::MB_ICONERROR,
        );
    }
    #[cfg(not(windows))]
    let _ = arquivo;
}

/// Um panic enquanto o Client abre vira a mensagem de [`falha_ao_abrir`]; depois, só o arquivo.
pub fn vigiar_panics() {
    let padrao = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let onde = info.location().map(|l| format!(" em {}:{}", l.file(), l.line())).unwrap_or_default();
        let texto = info.payload().downcast_ref::<&str>().map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        let motivo = format!("{texto}{onde}");
        if ABRINDO.load(Ordering::SeqCst) {
            falha_ao_abrir(&motivo);
        } else {
            gravar_falha(&motivo);
        }
        padrao(info);
    }));
}

/// A janela abriu: daqui em diante um panic não interrompe o jogador com mensagem.
pub fn abriu() {
    ABRINDO.store(false, Ordering::SeqCst);
}

/// Garante o WebView2 antes de abrir a janela. `false`: o jogador desistiu ou não deu para instalar
/// (o Client fecha, com a mensagem já mostrada).
#[cfg(windows)]
pub fn garantir_webview2() -> bool {
    if tauri::webview_version().is_ok() {
        return true;
    }
    gravar_falha("Microsoft Edge WebView2 não encontrado");
    let instalar = janela::mensagem(
        "Para abrir, o ShmupX precisa de um componente do Windows que não está neste computador: o Microsoft Edge WebView2.\n\nClique em OK para instalar agora. Precisa de internet e leva cerca de um minuto; depois o ShmupX abre sozinho.",
        janela::MB_OKCANCEL | janela::MB_ICONINFORMATION,
    ) == janela::IDOK;
    if !instalar {
        return false;
    }
    let instalador = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("webview2").join("MicrosoftEdgeWebview2Setup.exe")))
        .filter(|p| p.is_file());
    match instalador {
        Some(instalador) => match std::process::Command::new(&instalador).args(["/silent", "/install"]).status() {
            Ok(status) => {
                gravar_falha(&format!("instalador do WebView2 terminou: {status}"));
            }
            Err(e) => {
                gravar_falha(&format!("não deu para rodar o instalador do WebView2: {e}"));
            }
        },
        None => {
            gravar_falha("instalador do WebView2 não está na pasta do Client");
        }
    }
    if tauri::webview_version().is_ok() {
        return true;
    }
    janela::mensagem(
        "Não foi possível instalar o componente agora.\n\n1. Confira se o computador está conectado à internet.\n2. Na página da Microsoft que vai abrir, baixe e instale o \"Evergreen Bootstrapper\" do WebView2.\n3. Abra o ShmupX de novo.",
        janela::MB_OK | janela::MB_ICONWARNING,
    );
    let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", "https://developer.microsoft.com/microsoft-edge/webview2/"]).spawn();
    false
}

#[cfg(not(windows))]
pub fn garantir_webview2() -> bool {
    true
}
