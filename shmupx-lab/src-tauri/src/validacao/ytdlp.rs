//! Download de gameplays com o yt-dlp (programa externo, como no Lab antigo). Argumentos
//! separados, sem shell; uma pasta por vídeo, então o arquivo produzido é o único nela.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::watch;

pub const VARIAVEL_CAMINHO: &str = "SHMUPX_YTDLP_PATH";
/// Opcional: navegador de onde o yt-dlp lê os cookies do YouTube (ex.: `chrome`), o contorno que o
/// próprio yt-dlp recomenda para o bloqueio "Sign in to confirm you're not a bot".
pub const VARIAVEL_COOKIES: &str = "SHMUPX_YTDLP_COOKIES_FROM_BROWSER";
const TIMEOUT: Duration = Duration::from_secs(3600);
const EXTENSOES_VIDEO: [&str; 5] = ["mp4", "mkv", "webm", "mov", "avi"];

/// Release oficial do yt-dlp: o executável e a lista de SHA-256 publicada junto.
const RELEASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const TIMEOUT_INSTALACAO: Duration = Duration::from_secs(300);

/// Nome do executável standalone de cada sistema na release do yt-dlp.
fn arquivo_da_release() -> &'static str {
    if cfg!(windows) {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else {
        "yt-dlp_linux"
    }
}

/// Onde o Lab guarda o yt-dlp que ele mesmo instala (dentro da pasta `validacao`).
pub fn caminho_gerenciado(raiz_validacao: &Path) -> PathBuf {
    raiz_validacao.join("tools").join(if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" })
}

/// Caminho do yt-dlp: a variável de ambiente, o instalado pelo Lab ou o primeiro no PATH.
pub fn localizar(gerenciado: &Path) -> Option<PathBuf> {
    if let Some(caminho) = std::env::var_os(VARIAVEL_CAMINHO).map(PathBuf::from) {
        return caminho.is_file().then_some(caminho);
    }
    if gerenciado.is_file() {
        return Some(gerenciado.to_path_buf());
    }
    let nome = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|pasta| pasta.join(nome))
        .find(|caminho| caminho.is_file())
}

/// Hash esperado de `arquivo` na lista `SHA2-256SUMS` da release (`<hash>  <arquivo>` por linha).
fn hash_esperado(lista: &str, arquivo: &str) -> Option<String> {
    lista.lines().find_map(|linha| {
        let mut partes = linha.split_whitespace();
        let hash = partes.next()?;
        let nome = partes.next()?.trim_start_matches('*');
        (nome == arquivo && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Baixa o yt-dlp oficial da última release para `destino`, conferindo o SHA-256 contra a lista
/// publicada na mesma release. Devolve a versão instalada.
pub async fn instalar(destino: &Path) -> Result<String, String> {
    let http = reqwest::Client::builder()
        .timeout(TIMEOUT_INSTALACAO)
        .user_agent("shmupx-lab")
        .build()
        .map_err(|e| format!("falha ao construir cliente HTTP: {e}"))?;
    let baixar = |url: String| {
        let http = http.clone();
        async move {
            http.get(&url)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| format!("falha ao baixar {url}: {e}"))?
                .bytes()
                .await
                .map_err(|e| format!("falha ao baixar {url}: {e}"))
        }
    };

    let arquivo = arquivo_da_release();
    let lista = baixar(format!("{RELEASE}/SHA2-256SUMS")).await?;
    let esperado = hash_esperado(&String::from_utf8_lossy(&lista), arquivo)
        .ok_or_else(|| format!("a release do yt-dlp não lista o hash de {arquivo}"))?;
    let executavel = baixar(format!("{RELEASE}/{arquivo}")).await?;
    if sha256_hex(&executavel) != esperado {
        return Err("o arquivo baixado não confere com o SHA-256 publicado; tente de novo".into());
    }

    if let Some(pasta) = destino.parent() {
        std::fs::create_dir_all(pasta).map_err(|e| format!("falha ao criar a pasta do yt-dlp: {e}"))?;
    }
    let temporario = destino.with_extension("download");
    std::fs::write(&temporario, &executavel).map_err(|e| format!("falha ao gravar o yt-dlp: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporario, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("falha ao tornar o yt-dlp executável: {e}"))?;
    }
    std::fs::rename(&temporario, destino).map_err(|e| format!("falha ao instalar o yt-dlp: {e}"))?;
    versao(destino).await
}

pub async fn versao(executavel: &Path) -> Result<String, String> {
    let mut comando = Command::new(executavel);
    comando.arg("--version").stdin(Stdio::null()).kill_on_drop(true);
    sem_janela(&mut comando);
    let saida = tokio::time::timeout(Duration::from_secs(15), comando.output())
        .await
        .map_err(|_| "yt-dlp não respondeu".to_string())?
        .map_err(|e| format!("falha ao executar o yt-dlp: {e}"))?;
    Ok(String::from_utf8_lossy(&saida.stdout).trim().to_string())
}

/// Só http(s): o yt-dlp aceita muitos esquemas, e nenhum outro faz sentido aqui.
pub fn url_valida(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

/// `[download]  42.3% of ...` -> 42.3
pub fn progresso_da_linha(linha: &str) -> Option<f32> {
    let resto = linha.trim_start().strip_prefix("[download]")?;
    let token = resto.split_whitespace().find(|t| t.ends_with('%'))?;
    token.trim_end_matches('%').parse::<f32>().ok().filter(|p| (0.0..=100.0).contains(p))
}

fn sem_janela(comando: &mut Command) {
    #[cfg(windows)]
    comando.creation_flags(0x08000000); // CREATE_NO_WINDOW
    #[cfg(not(windows))]
    let _ = comando;
}

/// Baixa o vídeo para `pasta` (criada se preciso) e devolve o arquivo produzido. O progresso
/// (0 a 100) é publicado em `progresso`.
pub async fn baixar(executavel: &Path, url: &str, pasta: &Path, progresso: watch::Sender<f32>) -> Result<PathBuf, String> {
    let url = url_valida(url).ok_or_else(|| "Isso não parece um link de vídeo (http/https).".to_string())?;
    std::fs::create_dir_all(pasta).map_err(|e| format!("falha ao criar a pasta do vídeo: {e}"))?;

    let mut argumentos: Vec<OsString> = [
        "--no-playlist",
        "--newline",
        "--no-warnings",
        "--no-part",
        // Um MP4 único primeiro (toca mesmo sem FFmpeg para juntar); depois o par vídeo+áudio.
        "-f",
        "b[ext=mp4]/bv*[ext=mp4]+ba[ext=m4a]/b",
        "--merge-output-format",
        "mp4",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    if let Some(navegador) = std::env::var_os(VARIAVEL_COOKIES).filter(|v| !v.is_empty()) {
        argumentos.push("--cookies-from-browser".into());
        argumentos.push(navegador);
    }
    argumentos.push("-o".into());
    argumentos.push(pasta.join("%(title)s.%(ext)s").into_os_string());
    argumentos.push("--".into());
    argumentos.push(url.into());

    let mut comando = Command::new(executavel);
    comando
        .args(&argumentos)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    sem_janela(&mut comando);
    let mut filho = comando.spawn().map_err(|e| format!("falha ao iniciar o yt-dlp: {e}"))?;
    let stdout = filho.stdout.take().ok_or("saída do yt-dlp indisponível")?;
    let mut stderr = filho.stderr.take().ok_or("saída de erro do yt-dlp indisponível")?;

    let ler_progresso = async {
        let mut linhas = BufReader::new(stdout).lines();
        while let Ok(Some(linha)) = linhas.next_line().await {
            if let Some(p) = progresso_da_linha(&linha) {
                let _ = progresso.send(p);
            }
        }
    };
    let ler_erro = async {
        let mut bytes = Vec::new();
        let _ = (&mut stderr).take(64 * 1024).read_to_end(&mut bytes).await;
        bytes
    };
    let processo = async {
        let ((), erro, status) = tokio::join!(ler_progresso, ler_erro, filho.wait());
        (erro, status)
    };
    let (erro, status) = tokio::time::timeout(TIMEOUT, processo)
        .await
        .map_err(|_| "o download passou de 1 hora e foi cancelado".to_string())?;
    let status = status.map_err(|e| format!("falha ao aguardar o yt-dlp: {e}"))?;
    if !status.success() {
        let texto = String::from_utf8_lossy(&erro);
        let ultima = texto.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("sem detalhe");
        return Err(format!("o yt-dlp falhou: {}", ultima.trim()));
    }
    arquivo_baixado(pasta).ok_or_else(|| "o yt-dlp terminou, mas nenhum vídeo apareceu na pasta".to_string())
}

/// O maior arquivo de vídeo da pasta (é a única coisa nela, salvo restos de formatos parciais).
fn arquivo_baixado(pasta: &Path) -> Option<PathBuf> {
    std::fs::read_dir(pasta)
        .ok()?
        .filter_map(Result::ok)
        .map(|entrada| entrada.path())
        .filter(|caminho| {
            caminho
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| EXTENSOES_VIDEO.contains(&e.to_ascii_lowercase().as_str()))
        })
        .max_by_key(|caminho| std::fs::metadata(caminho).map(|m| m.len()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_progresso_das_linhas_do_yt_dlp() {
        assert_eq!(progresso_da_linha("[download]  42.3% of   12.00MiB at  1.00MiB/s ETA 00:07"), Some(42.3));
        assert_eq!(progresso_da_linha("[download] 100% of 12.00MiB in 00:10"), Some(100.0));
        assert_eq!(progresso_da_linha("[download] Destination: x.mp4"), None);
        assert_eq!(progresso_da_linha("[youtube] abc: Downloading webpage"), None);
        assert_eq!(progresso_da_linha("[download] 140% of x"), None);
    }

    #[test]
    fn le_o_hash_do_arquivo_na_lista_da_release() {
        let hash = "a".repeat(64);
        let lista = format!("{}  yt-dlp\n{hash}  yt-dlp.exe\n{}  yt-dlp_macos\n", "b".repeat(64), "c".repeat(64));
        assert_eq!(hash_esperado(&lista, "yt-dlp.exe"), Some(hash));
        assert_eq!(hash_esperado(&lista, "yt-dlp_x86.exe"), None);
        assert_eq!(hash_esperado("curto  yt-dlp.exe", "yt-dlp.exe"), None, "hash inválido é ignorado");
        assert_eq!(hash_esperado(&format!("{}  *yt-dlp.exe", "D".repeat(64)), "yt-dlp.exe"), Some("d".repeat(64)));
    }

    /// Contra a release real (rede, ~17 MB, sem custo): baixa, confere o hash e executa.
    /// `cargo test -- --ignored instala_o_yt_dlp_real`
    #[tokio::test]
    #[ignore = "exige acesso à rede"]
    async fn instala_o_yt_dlp_real() {
        let pasta = std::env::temp_dir().join(format!("shmupx-lab-ytdlp-instalacao-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        let destino = caminho_gerenciado(&pasta);
        let versao = instalar(&destino).await.unwrap();
        assert!(destino.is_file());
        assert!(versao.starts_with("20"), "versão no formato AAAA.MM.DD: {versao}");
        assert_eq!(localizar(&destino).as_deref(), Some(destino.as_path()));
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn sha256_em_hexadecimal() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn so_aceita_links_http() {
        assert!(url_valida("https://www.youtube.com/watch?v=abc").is_some());
        assert!(url_valida("  https://youtu.be/abc ").is_some());
        assert!(url_valida("file:///C:/x.mp4").is_none());
        assert!(url_valida("--exec calc").is_none());
        assert!(url_valida("").is_none());
    }

    #[test]
    fn escolhe_o_maior_video_da_pasta() {
        let pasta = std::env::temp_dir().join(format!("shmupx-lab-ytdlp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("a.mp4"), vec![0u8; 10]).unwrap();
        std::fs::write(pasta.join("b.MKV"), vec![0u8; 50]).unwrap();
        std::fs::write(pasta.join("c.txt"), vec![0u8; 999]).unwrap();
        assert_eq!(arquivo_baixado(&pasta).unwrap().file_name().unwrap(), "b.MKV");
    }
}
