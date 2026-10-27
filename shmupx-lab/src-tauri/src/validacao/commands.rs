use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{watch, Mutex};

use super::dados::{self, agora, sessao, Dataset, Estado, Gabarito, Origem, Repositorio, Resumo, Video};
use super::script::{self, ScriptInfo};
use super::ytdlp;

/// Evento emitido a cada mudança no banco de um jogo; a tela relê com `validacao_listar`.
const EVENTO: &str = "validacao://atualizado";

/// Serializa as gravações do banco e a fila de análise (um vídeo por vez: a análise é pesada).
#[derive(Default)]
pub struct ValidacaoState {
    gravacao: Mutex<()>,
    fila: Mutex<()>,
}

/// Vídeo com o veredito contra o gabarito já calculado (a regra fica só aqui, não na tela).
#[derive(Debug, Serialize)]
pub struct VideoView {
    #[serde(flatten)]
    pub video: Video,
    pub acertou: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct DatasetView {
    pub videos: Vec<VideoView>,
    pub resumo: Resumo,
}

#[derive(Debug, Serialize)]
pub struct Item {
    pub ok: bool,
    pub detalhe: String,
}

#[derive(Debug, Serialize)]
pub struct Ambiente {
    pub python: Item,
    /// OpenCV e NumPy no Python escolhido.
    pub dependencias: Item,
    pub ytdlp: Item,
    pub scripts: Vec<ScriptInfo>,
}

/// Pasta da validação dentro dos dados do Lab (bancos, vídeos baixados e o yt-dlp instalado).
fn pasta_validacao(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|pasta| pasta.join("validacao"))
        .map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))
}

fn repositorio(app: &AppHandle) -> Result<Repositorio, String> {
    pasta_validacao(app).map(Repositorio::new)
}

fn localizar_ytdlp(app: &AppHandle) -> Option<PathBuf> {
    ytdlp::localizar(&ytdlp::caminho_gerenciado(&pasta_validacao(app).ok()?))
}

fn conferir_jogo(jogo: &str) -> Result<(), String> {
    if dados::id_seguro(jogo) { Ok(()) } else { Err("id de jogo inválido".into()) }
}

fn novo_id() -> Result<String, String> {
    let mut bytes = [0u8; 8];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("falha ao gerar id: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Lê, altera e grava o banco do jogo sob a trava, e avisa a tela.
async fn alterar<R>(app: &AppHandle, jogo: &str, mudanca: impl FnOnce(&mut Dataset) -> Result<R, String>) -> Result<R, String> {
    let estado = app.state::<ValidacaoState>();
    let _trava = estado.gravacao.lock().await;
    let repo = repositorio(app)?;
    let mut dataset = repo.carregar(jogo)?;
    let resposta = mudanca(&mut dataset)?;
    repo.salvar(jogo, &dataset)?;
    let _ = app.emit(EVENTO, json!({ "game_id": jogo }));
    Ok(resposta)
}

/// Aplica uma mudança a um vídeo, se ele ainda existir (pode ter sido removido no meio do caminho).
async fn alterar_video(app: &AppHandle, jogo: &str, id: &str, mudanca: impl FnOnce(&mut Video)) -> Result<(), String> {
    alterar(app, jogo, |dataset| {
        if let Some(video) = dataset.videos.iter_mut().find(|v| v.id == id) {
            mudanca(video);
        }
        Ok(())
    })
    .await
}

/// O que a análise precisa e o que está faltando nesta máquina.
#[tauri::command]
pub async fn validacao_ambiente(app: AppHandle) -> Result<Ambiente, String> {
    let ytdlp = match localizar_ytdlp(&app) {
        Some(exe) => match ytdlp::versao(&exe).await {
            Ok(versao) => Item { ok: true, detalhe: format!("yt-dlp {versao}") },
            Err(e) => Item { ok: false, detalhe: e },
        },
        None => Item { ok: false, detalhe: "yt-dlp não encontrado (nem instalado pelo Lab, nem no PATH)".into() },
    };
    let raiz = match script::raiz_offline() {
        Ok(raiz) => raiz,
        Err(e) => {
            let falta = Item { ok: false, detalhe: e.clone() };
            return Ok(Ambiente { python: falta, dependencias: Item { ok: false, detalhe: e }, ytdlp, scripts: vec![] });
        }
    };
    let (python, scripts) = match script::listar_scripts(&raiz).await {
        Ok(scripts) => (Item { ok: true, detalhe: script::python(&raiz).to_string_lossy().into_owned() }, scripts),
        Err(e) => (Item { ok: false, detalhe: e }, vec![]),
    };
    let dependencias = match script::dependencias_ok(&raiz).await {
        Ok(()) => Item { ok: true, detalhe: "OpenCV e NumPy instalados".into() },
        Err(e) => Item { ok: false, detalhe: format!("faltam bibliotecas do script ({e})") },
    };
    Ok(Ambiente { python, dependencias, ytdlp, scripts })
}

#[tauri::command]
pub async fn validacao_listar(app: AppHandle, state: State<'_, ValidacaoState>, game_id: String) -> Result<DatasetView, String> {
    conferir_jogo(&game_id)?;
    let _trava = state.gravacao.lock().await;
    let dataset = repositorio(&app)?.carregar(&game_id)?;
    let resumo = dados::resumo(&dataset);
    let videos = dataset.videos.into_iter().map(|video| VideoView { acertou: video.acertou(), video }).collect();
    Ok(DatasetView { videos, resumo })
}

/// Cadastra o vídeo e baixa em segundo plano; a tela acompanha pelos eventos.
#[tauri::command]
pub async fn validacao_adicionar_youtube(app: AppHandle, game_id: String, url: String) -> Result<Video, String> {
    conferir_jogo(&game_id)?;
    let url = ytdlp::url_valida(&url).ok_or_else(|| "Isso não parece um link de vídeo (http/https).".to_string())?;
    let executavel = localizar_ytdlp(&app).ok_or_else(|| "yt-dlp não encontrado: instale pela tela de Validação".to_string())?;
    let video = Video {
        id: novo_id()?,
        titulo: url.clone(),
        origem: Origem::Youtube { url: url.clone() },
        caminho: None,
        adicionado_em: agora(),
        gabarito: Gabarito::default(),
        estado: Estado::Baixando { progresso: 0.0, sessao: sessao() },
        resultado: None,
    };
    let novo = video.clone();
    alterar(&app, &game_id, |dataset| {
        dataset.videos.push(novo);
        Ok(())
    })
    .await?;

    let pasta = repositorio(&app)?.pasta_videos(&game_id, &video.id);
    let (app_tarefa, jogo, id) = (app.clone(), game_id.clone(), video.id.clone());
    tauri::async_runtime::spawn(async move {
        let (tx, mut rx) = watch::channel(0.0f32);
        // Publica o progresso no banco no máximo uma vez por segundo.
        let (app_progresso, jogo_progresso, id_progresso) = (app_tarefa.clone(), jogo.clone(), id.clone());
        let progresso = tauri::async_runtime::spawn(async move {
            while rx.changed().await.is_ok() {
                let valor = *rx.borrow_and_update();
                let _ = alterar_video(&app_progresso, &jogo_progresso, &id_progresso, |v| {
                    if matches!(v.estado, Estado::Baixando { .. }) {
                        v.estado = Estado::Baixando { progresso: valor, sessao: sessao() };
                    }
                })
                .await;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        let resultado = ytdlp::baixar(&executavel, &url, &pasta, tx).await;
        let _ = progresso.await;
        let _ = alterar_video(&app_tarefa, &jogo, &id, |v| match resultado {
            Ok(arquivo) => {
                v.titulo = arquivo.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| v.titulo.clone());
                v.caminho = Some(arquivo.to_string_lossy().into_owned());
                v.estado = Estado::Pronto;
            }
            Err(erro) => v.estado = Estado::Falhou { erro },
        })
        .await;
    });
    Ok(video)
}

/// Cadastra vídeos locais por referência (sem copiar o arquivo).
#[tauri::command]
pub async fn validacao_adicionar_arquivos(app: AppHandle, game_id: String, caminhos: Vec<String>) -> Result<usize, String> {
    conferir_jogo(&game_id)?;
    let mut videos = Vec::new();
    for caminho in caminhos {
        let arquivo = PathBuf::from(&caminho)
            .canonicalize()
            .map_err(|e| format!("arquivo indisponível ({caminho}): {e}"))?;
        if !arquivo.is_file() {
            return Err(format!("não é um arquivo: {caminho}"));
        }
        videos.push(Video {
            id: novo_id()?,
            titulo: arquivo.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            origem: Origem::Arquivo,
            caminho: Some(arquivo.to_string_lossy().into_owned()),
            adicionado_em: agora(),
            gabarito: Gabarito::default(),
            estado: Estado::Pronto,
            resultado: None,
        });
    }
    let quantidade = videos.len();
    alterar(&app, &game_id, |dataset| {
        dataset.videos.extend(videos);
        Ok(quantidade)
    })
    .await
}

#[tauri::command]
pub async fn validacao_definir_gabarito(app: AppHandle, game_id: String, video_id: String, gabarito: Gabarito) -> Result<(), String> {
    conferir_jogo(&game_id)?;
    if gabarito.placar.is_some_and(|p| p < 0) || gabarito.vidas.is_some_and(|v| v < 0) || gabarito.continues.is_some_and(|c| c < 0) {
        return Err("os valores do gabarito não podem ser negativos".into());
    }
    alterar(&app, &game_id, |dataset| {
        let video = dataset.videos.iter_mut().find(|v| v.id == video_id).ok_or("vídeo não encontrado")?;
        video.gabarito = gabarito;
        Ok(())
    })
    .await
}

/// Remove o vídeo do banco. Um vídeo baixado pelo Lab tem a pasta apagada; um arquivo local
/// só deixa de ser referenciado (o original fica intacto).
#[tauri::command]
pub async fn validacao_remover(app: AppHandle, game_id: String, video_id: String) -> Result<(), String> {
    conferir_jogo(&game_id)?;
    let repo = repositorio(&app)?;
    let removido = alterar(&app, &game_id, |dataset| {
        let posicao = dataset.videos.iter().position(|v| v.id == video_id).ok_or("vídeo não encontrado")?;
        if matches!(dataset.videos[posicao].estado, Estado::Analisando { .. }) {
            return Err("aguarde a análise deste vídeo terminar".to_string());
        }
        Ok(dataset.videos.remove(posicao))
    })
    .await?;
    if matches!(removido.origem, Origem::Youtube { .. }) {
        let pasta = repo.pasta_videos(&game_id, &removido.id);
        if repo.e_pasta_propria(&pasta) {
            let _ = std::fs::remove_dir_all(pasta);
        }
    }
    Ok(())
}

/// Põe os vídeos na fila de análise e processa um por vez em segundo plano. Devolve quantos
/// entraram (vídeos sem arquivo, baixando ou já na fila são ignorados).
#[tauri::command]
pub async fn validacao_rodar(app: AppHandle, game_id: String, video_ids: Vec<String>) -> Result<usize, String> {
    conferir_jogo(&game_id)?;
    let raiz = script::raiz_offline()?;
    let enfileirados = alterar(&app, &game_id, |dataset| {
        let mut ids = Vec::new();
        for video in dataset.videos.iter_mut().filter(|v| video_ids.contains(&v.id)) {
            let pode = video.caminho.is_some()
                && matches!(video.estado, Estado::Pronto | Estado::Concluido);
            if pode {
                video.estado = Estado::NaFila { sessao: sessao() };
                ids.push(video.id.clone());
            }
        }
        Ok(ids)
    })
    .await?;

    let quantidade = enfileirados.len();
    let (app_tarefa, jogo) = (app.clone(), game_id.clone());
    tauri::async_runtime::spawn(async move {
        let estado = app_tarefa.state::<ValidacaoState>();
        for id in enfileirados {
            let _vez = estado.fila.lock().await;
            // Pode ter sido removido enquanto esperava na fila.
            let caminho = {
                let _trava = estado.gravacao.lock().await;
                repositorio(&app_tarefa)
                    .and_then(|repo| repo.carregar(&jogo))
                    .ok()
                    .and_then(|d| d.videos.into_iter().find(|v| v.id == id))
                    .filter(|v| matches!(v.estado, Estado::NaFila { .. }))
                    .and_then(|v| v.caminho)
            };
            let Some(caminho) = caminho else { continue };
            let _ = alterar_video(&app_tarefa, &jogo, &id, |v| v.estado = Estado::Analisando { sessao: sessao() }).await;
            let resultado = script::analisar(&raiz, &jogo, &PathBuf::from(caminho)).await;
            let _ = alterar_video(&app_tarefa, &jogo, &id, |v| {
                v.resultado = Some(resultado);
                v.estado = Estado::Concluido;
            })
            .await;
        }
    });
    Ok(quantidade)
}

/// Instala (ou atualiza) o yt-dlp oficial na pasta do Lab, conferindo o SHA-256 da release.
#[tauri::command]
pub async fn validacao_instalar_ytdlp(app: AppHandle) -> Result<Item, String> {
    let destino = ytdlp::caminho_gerenciado(&pasta_validacao(&app)?);
    let versao = ytdlp::instalar(&destino).await?;
    Ok(Item { ok: true, detalhe: format!("yt-dlp {versao}") })
}
