//! Depois do envio de uma run: a posição dela no ranking do jogo, o pódio (top 3) da sala e o
//! destaque no canal oficial da ShmupX.
//!
//! - Pódio: os 3 primeiros do Hi-Score do jogo, lidos na hora (sem o cache de 60 s da rota pública).
//!   A sala compara com o pódio que mostra e reordena se mudou.
//! - Destaque: só a run Ranked **com live** que entra no top 3 (e é o melhor placar do jogador) vai
//!   para o canal oficial. O backend abre o envio do vídeo no canal (conexão do YouTube da conta
//!   em `SHMUPX_YOUTUBE_CANAL_UID`) com o nome do jogador, o placar e a posição, e o Client manda a
//!   cópia da live gravada no PC. O endereço do envio fica em `highlights/{runId}`, coleção sem
//!   regra de leitura (só o backend a vê).
//!
//! Nada aqui derruba o envio da run: uma falha vira só log e a resposta sai sem o destaque.

use chrono::Utc;
use serde::Serialize;

use crate::error::AppError;
use crate::firestore::valor::{data, inteiro, ler_inteiro, ler_mapa, ler_texto, texto};
use crate::firestore::{Campos, Firestore};
use crate::routes::ranking_publico::{jogos_publicados, ler_partidas, nome_ou_padrao, nomes_dos_jogadores};
use crate::services::placar;
use crate::services::youtube::{self, YouTube};

/// Só as 3 primeiras posições vão para o canal oficial.
pub const POSICOES_EM_DESTAQUE: usize = 3;
const COLECAO: &str = "highlights";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LugarNoPodio {
    pub posicao: usize,
    pub jogador_id: String,
    pub nome: String,
    pub score: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Destaque {
    pub posicao: usize,
    pub score: i64,
    /// Endereço do envio retomável no canal oficial: o Client manda o vídeo direto ao YouTube.
    pub upload_url: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct DepoisDoEnvio {
    /// Posição do jogador na tabela do modo da run, se este placar é o melhor dele.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posicao: Option<usize>,
    /// Top 3 do Hi-Score do jogo (o pódio da sala), já com esta run.
    pub podio: Vec<LugarNoPodio>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destaque: Option<Destaque>,
}

/// Conta do ShmupX cujo YouTube conectado é o canal oficial (`youtubeConnections/{uid}`).
fn canal_oficial() -> Option<String> {
    std::env::var("SHMUPX_YOUTUBE_CANAL_UID").ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Tabela do modo da run: `1cc` e Caravan têm a própria; o resto é Hi-Score.
fn tabela_do_modo(modo: Option<&str>) -> &'static str {
    match modo {
        Some("1cc") => "1cc",
        Some(m) if m.starts_with("caravan") => "caravan",
        _ => "highscore",
    }
}

/// Posição do jogador se o melhor placar dele na tabela é o desta run.
fn posicao_da_run(entradas: &[placar::EntradaJogo], uid: &str, score: i64) -> Option<usize> {
    entradas.iter().find(|e| e.jogador_id == uid).filter(|e| e.score == score).map(|e| e.posicao)
}

fn titulo_do_video(nome: &str, jogo: &str, posicao: usize, score: i64) -> String {
    format!("{nome} · {jogo} · {posicao}º lugar · {} pontos | ShmupX", separar_milhares(score))
}

fn separar_milhares(n: i64) -> String {
    let digitos = n.abs().to_string();
    let mut saida = String::new();
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i) % 3 == 0 {
            saida.push('.');
        }
        saida.push(c);
    }
    if n < 0 { format!("-{saida}") } else { saida }
}

fn descricao_do_video(nome: &str, jogo: &str, posicao: usize, score: i64, live: &str) -> String {
    format!(
        "{nome} ficou em {posicao}º lugar no ranking de {jogo} no ShmupX, com {} pontos.\n\n\
         Partida Ranked de 1 ficha, sem continue, transmitida ao vivo pelo ShmupX Client.\n\
         Live original: {live}\n\n\
         Só as partidas que entram no top 3 do ranking são publicadas no canal oficial da ShmupX.",
        separar_milhares(score)
    )
}

/// Posição, pódio e, se for o caso, o destaque da run `run_id` recém-enviada.
pub async fn depois_do_envio(fs: &Firestore, youtube: Option<&YouTube>, uid: &str, run_id: &str, game_id: &str) -> DepoisDoEnvio {
    match calcular(fs, youtube, uid, run_id, game_id).await {
        Ok(saida) => saida,
        Err(e) => {
            eprintln!("destaques: run {run_id}: {e:?}");
            DepoisDoEnvio::default()
        }
    }
}

async fn calcular(fs: &Firestore, youtube: Option<&YouTube>, uid: &str, run_id: &str, game_id: &str) -> Result<DepoisDoEnvio, AppError> {
    let gameplay = fs.ler_documento(&format!("gameplays/{run_id}")).await?.unwrap_or_default();
    let score = ler_mapa(&gameplay, "result").and_then(|r| ler_inteiro(&r, "score"));
    let jogos = jogos_publicados(fs).await?;
    let Some(titulo_jogo) = jogos.get(game_id).cloned() else { return Ok(DepoisDoEnvio::default()) };
    let (partidas, _) = ler_partidas(fs, Some(game_id), &jogos).await?;

    let hiscore = placar::ranking_do_jogo(&partidas, game_id, Some("highscore"), None);
    let tabela = tabela_do_modo(ler_texto(&gameplay, "mode").as_deref());
    let do_modo = if tabela == "highscore" { hiscore.clone() } else { placar::ranking_do_jogo(&partidas, game_id, Some(tabela), None) };
    let posicao = score.and_then(|s| posicao_da_run(&do_modo, uid, s));

    let topo: Vec<&placar::EntradaJogo> = hiscore.iter().take_while(|e| e.posicao <= POSICOES_EM_DESTAQUE).collect();
    let nomes = nomes_dos_jogadores(fs, topo.iter().map(|e| e.jogador_id.as_str()).chain(std::iter::once(uid))).await?;
    let podio = topo
        .iter()
        .map(|e| LugarNoPodio { posicao: e.posicao, jogador_id: e.jogador_id.clone(), nome: nome_ou_padrao(&nomes, &e.jogador_id), score: e.score })
        .collect();

    let live = ler_mapa(&gameplay, "live").and_then(|l| ler_texto(&l, "watchUrl"));
    let destaque = match (posicao, score, live, youtube, canal_oficial()) {
        (Some(posicao), Some(score), Some(live), Some(youtube), Some(canal)) if posicao <= POSICOES_EM_DESTAQUE => {
            let nome = nome_ou_padrao(&nomes, uid);
            Some(abrir_destaque(fs, youtube, &canal, uid, run_id, game_id, &titulo_jogo, &nome, posicao, score, &live).await?)
        }
        _ => None,
    };
    Ok(DepoisDoEnvio { posicao, podio, destaque })
}

#[allow(clippy::too_many_arguments)]
async fn abrir_destaque(
    fs: &Firestore,
    youtube: &YouTube,
    canal: &str,
    uid: &str,
    run_id: &str,
    game_id: &str,
    jogo: &str,
    nome: &str,
    posicao: usize,
    score: i64,
    live: &str,
) -> Result<Destaque, AppError> {
    let caminho = format!("{COLECAO}/{run_id}");
    // Reenvio do resultado (a resposta anterior se perdeu): o mesmo envio, sem abrir outro.
    if let Some(existente) = fs.ler_documento(&caminho).await? {
        if let Some(upload_url) = ler_texto(&existente, "uploadUrl").filter(|u| !u.is_empty()) {
            return Ok(Destaque { posicao, score, upload_url });
        }
        return Err(AppError::conflict("o destaque desta run já foi enviado"));
    }
    let titulo = titulo_do_video(nome, jogo, posicao, score);
    let descricao = descricao_do_video(nome, jogo, posicao, score, live);
    let upload_url = youtube.abrir_envio_de_video(fs, canal, &titulo, &descricao, &["ShmupX", jogo, "shmup", "1cc"]).await?;
    let mut campos = Campos::new();
    campos.insert("ownerId".into(), texto(uid));
    campos.insert("gameId".into(), texto(game_id));
    campos.insert("playerName".into(), texto(nome));
    campos.insert("position".into(), inteiro(posicao as i64));
    campos.insert("score".into(), inteiro(score));
    campos.insert("liveUrl".into(), texto(live));
    campos.insert("title".into(), texto(&titulo));
    campos.insert("uploadUrl".into(), texto(&upload_url));
    campos.insert("status".into(), texto("awaiting_upload"));
    campos.insert("createdAt".into(), data(Utc::now()));
    fs.gravar_novo(&caminho, campos).await?;
    Ok(Destaque { posicao, score, upload_url })
}

/// O Client terminou de mandar o vídeo: guarda o id dele no canal e esquece o endereço do envio.
pub async fn registrar_envio(fs: &Firestore, uid: &str, run_id: &str, video_id: &str) -> Result<(), AppError> {
    if !crate::services::gameplays::id_valido(run_id) {
        return Err(AppError::bad_request("run_id inválido"));
    }
    if !youtube::id_valido(video_id) {
        return Err(AppError::bad_request("id do vídeo inválido"));
    }
    let caminho = format!("{COLECAO}/{run_id}");
    let destaque = fs
        .ler_documento(&caminho)
        .await?
        .filter(|d| ler_texto(d, "ownerId").as_deref() == Some(uid))
        .ok_or_else(|| AppError::not_found("destaque não encontrado"))?;
    if ler_texto(&destaque, "status").as_deref() == Some("posted") {
        return Ok(());
    }
    let mut campos = Campos::new();
    campos.insert("status".into(), texto("posted"));
    campos.insert("videoId".into(), texto(video_id));
    campos.insert("videoUrl".into(), texto(&format!("https://www.youtube.com/watch?v={video_id}")));
    campos.insert("uploadUrl".into(), texto(""));
    campos.insert("postedAt".into(), data(Utc::now()));
    fs.gravar(&caminho, campos, &["status", "videoId", "videoUrl", "uploadUrl", "postedAt"]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrada(posicao: usize, jogador: &str, score: i64) -> placar::EntradaJogo {
        placar::EntradaJogo {
            posicao,
            jogador_id: jogador.into(),
            nome: String::new(),
            score,
            nota: 0.0,
            continues: 0,
            vidas_perdidas: None,
            runs: 1,
            modo: None,
            versao: None,
            origem: None,
            verificada: false,
            conquistado_em: None,
            run_id: String::new(),
            youtube_url: None,
        }
    }

    #[test]
    fn posicao_so_quando_a_run_e_o_melhor_placar() {
        let tabela = [entrada(1, "ana", 900), entrada(2, "bia", 800), entrada(3, "caio", 700)];
        assert_eq!(posicao_da_run(&tabela, "bia", 800), Some(2));
        // A run de 500 da Bia não é a que está no ranking (o melhor dela é 800).
        assert_eq!(posicao_da_run(&tabela, "bia", 500), None);
        assert_eq!(posicao_da_run(&tabela, "davi", 100), None);
    }

    #[test]
    fn modo_da_run_escolhe_a_tabela() {
        assert_eq!(tabela_do_modo(None), "highscore");
        assert_eq!(tabela_do_modo(Some("highscore")), "highscore");
        assert_eq!(tabela_do_modo(Some("1cc")), "1cc");
        assert_eq!(tabela_do_modo(Some("caravan-sprint")), "caravan");
    }

    #[test]
    fn titulo_com_nome_posicao_e_placar() {
        assert_eq!(titulo_do_video("RAMON", "Aero Fighters 2", 1, 1234567), "RAMON · Aero Fighters 2 · 1º lugar · 1.234.567 pontos | ShmupX");
        assert_eq!(separar_milhares(999), "999");
        assert_eq!(separar_milhares(1000), "1.000");
    }
}
