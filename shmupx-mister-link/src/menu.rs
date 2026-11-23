//! Jogos do Lab no menu ShmupX do MiSTer. Ao sincronizar a biblioteca do MiSTer, os jogos do
//! catálogo marcados com MiSTer que o aparelho tem (o `.mra` em `_Arcade`) entram no
//! `shmupx/jogos.txt`, e o menu é refeito pelos scripts do próprio MiSTer (`modelos.sh` e `menu.sh`).
//! Só acrescenta: o que já está na lista (inclusive à mão) não é tocado.

use serde::Deserialize;

use crate::ssh::Ssh;

const PASTA: &str = "/media/fat/shmupx";

/// Um jogo do catálogo para o menu: título no menu, o `.mra` do MiSTer e os modos do Lab.
#[derive(Debug, Clone, Deserialize)]
pub struct JogoParaMenu {
    pub titulo: String,
    /// Nome do `.mra` em `/media/fat/_Arcade`, sem a extensão.
    pub mra: String,
    /// Modos do catálogo: `highscore`, `1cc`, `caravan` (vazio = todos).
    pub modos: Vec<String>,
}

/// Nome seguro para arquivo no cartão: sem barras, `|`, quebras de linha nem caracteres que o
/// Windows/FAT recusam; espaços repetidos viram um.
fn nome_de_arquivo(texto: &str) -> String {
    let limpo: String = texto
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | '|' | ':' | '*' | '?' | '"' | '<' | '>') || c.is_control() { ' ' } else { c })
        .collect();
    limpo.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Modos do catálogo no formato do `jogos.txt` (Caravan vira Sprint e Classic).
fn modos_do_menu(modos: &[String]) -> String {
    let mut saida: Vec<&str> = Vec::new();
    let todos = modos.is_empty();
    if todos || modos.iter().any(|m| m == "highscore") { saida.push("hiscore"); }
    if todos || modos.iter().any(|m| m == "1cc") { saida.push("1cc"); }
    if todos || modos.iter().any(|m| m == "caravan") { saida.extend(["sprint", "classic"]); }
    saida.join(" ")
}

/// Linhas a acrescentar ao `jogos.txt` atual: só jogos que ainda não estão nele, pelo nome no menu
/// ou pelo mesmo `.mra`.
fn linhas_novas(atual: &str, jogos: &[JogoParaMenu]) -> Vec<(String, String)> {
    let mut nomes: Vec<String> = Vec::new();
    let mut origens: Vec<String> = Vec::new();
    for linha in atual.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let mut partes = linha.split('|');
        let arquivo = partes.next().unwrap_or("").trim();
        let origem = partes.nth(1).unwrap_or("").trim();
        nomes.push(arquivo.rsplit_once('.').map_or(arquivo, |(n, _)| n).to_lowercase());
        origens.push(origem.to_lowercase());
    }
    let mut novas = Vec::new();
    for jogo in jogos {
        let titulo = nome_de_arquivo(&jogo.titulo);
        let mra = format!("{}.mra", jogo.mra.trim().trim_end_matches(".mra"));
        if titulo.is_empty() || mra.contains(['|', '\n', '/', '\\']) {
            continue;
        }
        if nomes.contains(&titulo.to_lowercase()) || origens.contains(&mra.to_lowercase()) {
            continue;
        }
        nomes.push(titulo.to_lowercase());
        origens.push(mra.to_lowercase());
        novas.push((titulo.clone(), format!("{titulo}.mra|{}|{mra}", modos_do_menu(&jogo.modos))));
    }
    novas
}

/// Acrescenta ao menu ShmupX do MiSTer os jogos que faltam e refaz o menu. Devolve os títulos
/// adicionados (vazio se nada mudou ou se o ShmupX ainda não está instalado no MiSTer).
pub async fn sincronizar(host: &str, usuario: &str, senha: &str, jogos: &[JogoParaMenu]) -> Result<Vec<String>, String> {
    if jogos.is_empty() {
        return Ok(Vec::new());
    }
    let ssh = Ssh::conectar(host, usuario, senha).await?;
    let (codigo, atual) = ssh.executar(&format!("cat {PASTA}/jogos.txt"), None).await?;
    if codigo != 0 {
        // ShmupX ainda não instalado: o "Conectar ao meu MiSTer" instala e já traz a lista.
        return Ok(Vec::new());
    }
    let novas = linhas_novas(&atual, jogos);
    if novas.is_empty() {
        return Ok(Vec::new());
    }
    let mut texto = String::new();
    if !atual.is_empty() && !atual.ends_with('\n') {
        texto.push('\n');
    }
    for (_, linha) in &novas {
        texto.push_str(linha);
        texto.push('\n');
    }
    ssh.exigir(&format!("cat >> {PASTA}/jogos.txt"), Some(texto.as_bytes()), "atualizar a lista de jogos do ShmupX").await?;
    ssh.exigir(
        &format!("cd {PASTA} && SHMUPX_SD=/media/fat ./modelos.sh >/dev/null 2>&1; SHMUPX_SD=/media/fat ./menu.sh >/dev/null 2>&1; true"),
        None,
        "refazer o menu ShmupX",
    )
    .await?;
    Ok(novas.into_iter().map(|(titulo, _)| titulo).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jogo(titulo: &str, mra: &str, modos: &[&str]) -> JogoParaMenu {
        JogoParaMenu { titulo: titulo.into(), mra: mra.into(), modos: modos.iter().map(|m| m.to_string()).collect() }
    }

    const ATUAL: &str = "# comentário\nEarthion.mgl|hiscore|@megadrive-ra;1;1;*earthion*\nDarius Gaiden.mra|hiscore 1cc sprint|Darius Gaiden.mra\n";

    #[test]
    fn acrescenta_so_o_que_falta() {
        let novas = linhas_novas(ATUAL, &[
            jogo("Darius Gaiden", "Darius Gaiden", &[]),
            jogo("DoDonPachi", "DoDonPachi (Japan) (1997-02-05)", &["highscore", "1cc"]),
        ]);
        assert_eq!(novas.len(), 1);
        assert_eq!(novas[0].1, "DoDonPachi.mra|hiscore 1cc|DoDonPachi (Japan) (1997-02-05).mra");
    }

    #[test]
    fn mesmo_mra_com_outro_nome_nao_duplica() {
        assert!(linhas_novas(ATUAL, &[jogo("Darius Gaiden (World)", "Darius Gaiden", &[])]).is_empty());
    }

    #[test]
    fn caravan_vira_sprint_e_classic_e_vazio_e_todos() {
        assert_eq!(modos_do_menu(&["caravan".into()]), "sprint classic");
        assert_eq!(modos_do_menu(&[]), "hiscore 1cc sprint classic");
    }

    #[test]
    fn titulo_vira_nome_de_arquivo_seguro() {
        assert_eq!(nome_de_arquivo("Mushihime-sama: Futari 1.5 / Ver | x"), "Mushihime-sama Futari 1.5 Ver x");
        assert!(linhas_novas("", &[jogo("A", "a/b", &[])]).is_empty());
    }
}
