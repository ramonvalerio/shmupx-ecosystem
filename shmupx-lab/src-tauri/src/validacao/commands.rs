use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{watch, Mutex};

use super::dados::{self, agora, sessao, Dataset, Estado, Gabarito, Origem, Repositorio, Resumo, Video};
use super::fonte::{self, ArquivoFonte};
use super::gerador::{self, ScriptGerado};
use super::script::{self, ScriptInfo};
use super::ytdlp;

/// Evento emitido a cada mudança no banco de um jogo; a tela relê com `validacao_listar`.
const EVENTO: &str = "validacao://atualizado";

/// Serializa as gravações do banco e a fila de análise (um vídeo por vez: a análise é pesada).
#[derive(Default)]
pub struct ValidacaoState {
    gravacao: Mutex<()>,
    fila: Mutex<()>,
    /// Vídeos com a duração sendo lida agora (evita repetir a leitura a cada atualização da lista).
    medindo: Mutex<HashSet<String>>,
    /// Sinal de cancelamento de cada download ou análise em andamento, por id do vídeo.
    cancelamentos: Mutex<HashMap<String, watch::Sender<bool>>>,
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
    /// Deno, o runtime JavaScript que o yt-dlp usa para o YouTube.
    pub deno: Item,
    pub scripts: Vec<ScriptInfo>,
    /// Navegador escolhido para os cookies do YouTube (`None` = sem cookies).
    pub cookies_navegador: Option<String>,
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

/// Cria o sinal de cancelamento do vídeo e devolve o lado que a tarefa escuta.
async fn registrar_cancelamento(app: &AppHandle, id: &str) -> watch::Receiver<bool> {
    let (tx, rx) = watch::channel(false);
    app.state::<ValidacaoState>().cancelamentos.lock().await.insert(id.to_string(), tx);
    rx
}

async fn liberar_cancelamento(app: &AppHandle, id: &str) {
    app.state::<ValidacaoState>().cancelamentos.lock().await.remove(id);
}

/// Publica no banco o progresso que a tarefa manda por `rx`, no máximo uma vez por segundo.
/// Termina quando a tarefa larga o canal.
fn acompanhar(
    app: &AppHandle,
    jogo: &str,
    id: &str,
    mut rx: watch::Receiver<f32>,
    aplicar: fn(&mut Video, f32),
) -> tauri::async_runtime::JoinHandle<()> {
    let (app, jogo, id) = (app.clone(), jogo.to_string(), id.to_string());
    tauri::async_runtime::spawn(async move {
        while rx.changed().await.is_ok() {
            let valor = *rx.borrow_and_update();
            let _ = alterar_video(&app, &jogo, &id, |v| aplicar(v, valor)).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}

fn progresso_do_download(video: &mut Video, valor: f32) {
    if matches!(video.estado, Estado::Baixando { .. }) {
        video.estado = Estado::Baixando { progresso: valor, sessao: sessao() };
    }
}

fn progresso_da_analise(video: &mut Video, valor: f32) {
    if matches!(video.estado, Estado::Analisando { .. }) {
        video.estado = Estado::Analisando { progresso: valor, sessao: sessao() };
    }
}

/// Estado de um vídeo que sai da fila ou da análise sem resultado novo.
fn estado_de_repouso(video: &Video) -> Estado {
    if video.resultado.is_some() { Estado::Concluido } else { Estado::Pronto }
}

/// Baixa o vídeo em segundo plano (metadados, progresso, cancelamento); a tela acompanha pelos
/// eventos. O vídeo já deve estar no banco em `Baixando`.
fn iniciar_download(app: &AppHandle, jogo: &str, id: &str, url: String, executavel: PathBuf) -> Result<(), String> {
    let repo = repositorio(app)?;
    let pasta = repo.pasta_videos(jogo, id);
    let cookies = ytdlp::navegador_cookies(&pasta_validacao(app)?);
    let (app, jogo, id) = (app.clone(), jogo.to_string(), id.to_string());
    tauri::async_runtime::spawn(async move {
        let cancelar = registrar_cancelamento(&app, &id).await;
        // Título e duração primeiro (segundos), para a lista não mostrar só o link durante o download.
        if let Ok((titulo, duracao)) = ytdlp::metadados(&executavel, &url, cookies.as_ref()).await {
            let _ = alterar_video(&app, &jogo, &id, |v| {
                if let Some(titulo) = titulo {
                    v.titulo = titulo;
                }
                v.duracao_segundos = duracao.or(v.duracao_segundos);
            })
            .await;
        }
        let (tx, rx) = watch::channel(0.0f32);
        let progresso = acompanhar(&app, &jogo, &id, rx, progresso_do_download);
        let resultado = ytdlp::baixar(&executavel, &url, &pasta, cookies.as_ref(), tx, cancelar).await;
        let _ = progresso.await;
        liberar_cancelamento(&app, &id).await;
        if resultado.is_err() && repo.e_pasta_propria(&pasta) {
            // Restos de um download que não terminou não servem para nada.
            let _ = std::fs::remove_dir_all(&pasta);
        }
        let _ = alterar_video(&app, &jogo, &id, |v| match resultado {
            Ok(arquivo) => {
                // Sem metadados, o nome do arquivo (que o yt-dlp gera a partir do título) vira o título.
                if matches!(&v.origem, Origem::Youtube { url } if *url == v.titulo) {
                    v.titulo = arquivo.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| v.titulo.clone());
                }
                v.caminho = Some(arquivo.to_string_lossy().into_owned());
                v.estado = Estado::Pronto;
            }
            Err(erro) => v.estado = Estado::Falhou { erro },
        })
        .await;
    });
    Ok(())
}

/// O que a análise precisa e o que está faltando nesta máquina.
#[tauri::command]
pub async fn validacao_ambiente(app: AppHandle) -> Result<Ambiente, String> {
    let cookies_navegador = pasta_validacao(&app).ok().and_then(|pasta| ytdlp::navegador_salvo(&pasta));
    let deno = match ytdlp::versao_deno().await {
        Ok(versao) => Item { ok: true, detalhe: versao },
        Err(e) => Item { ok: false, detalhe: e },
    };
    let ytdlp = match localizar_ytdlp(&app) {
        Some(exe) => match ytdlp::versao(&exe).await {
            Ok(versao) => Item { ok: true, detalhe: format!("yt-dlp {versao}") },
            Err(e) => Item { ok: false, detalhe: e },
        },
        None => Item { ok: false, detalhe: "yt-dlp não encontrado (nem instalado pelo Lab, nem no PATH)".into() },
    };
    let raiz = match script::raiz_validadores() {
        Ok(raiz) => raiz,
        Err(e) => {
            let falta = Item { ok: false, detalhe: e.clone() };
            return Ok(Ambiente { python: falta, dependencias: Item { ok: false, detalhe: e }, ytdlp, deno, scripts: vec![], cookies_navegador });
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
    Ok(Ambiente { python, dependencias, ytdlp, deno, scripts, cookies_navegador })
}

#[tauri::command]
pub async fn validacao_listar(app: AppHandle, state: State<'_, ValidacaoState>, game_id: String) -> Result<DatasetView, String> {
    conferir_jogo(&game_id)?;
    let _trava = state.gravacao.lock().await;
    let dataset = repositorio(&app)?.carregar(&game_id)?;
    let resumo = dados::resumo(&dataset);
    let sem_duracao: Vec<(String, String)> = dataset
        .videos
        .iter()
        .filter(|v| v.duracao_segundos.is_none() && !matches!(v.estado, Estado::Baixando { .. }))
        .filter_map(|v| Some((v.id.clone(), v.caminho.clone()?)))
        .collect();
    let videos = dataset.videos.into_iter().map(|video| VideoView { acertou: video.acertou(), video }).collect();
    medir_duracoes(&app, &game_id, sem_duracao).await;
    Ok(DatasetView { videos, resumo })
}

/// Lê em segundo plano a duração de vídeos que ainda não a têm (ex.: cadastrados antes de a
/// duração existir). Cada vídeo é medido uma vez por execução do Lab.
async fn medir_duracoes(app: &AppHandle, jogo: &str, videos: Vec<(String, String)>) {
    let Ok(raiz) = script::raiz_validadores() else { return };
    let pendentes: Vec<(String, String)> = {
        let estado = app.state::<ValidacaoState>();
        let mut medindo = estado.medindo.lock().await;
        videos.into_iter().filter(|(id, _)| medindo.insert(id.clone())).collect()
    };
    if pendentes.is_empty() {
        return;
    }
    let (app_tarefa, jogo) = (app.clone(), jogo.to_string());
    tauri::async_runtime::spawn(async move {
        for (id, caminho) in pendentes {
            if let Some(duracao) = script::duracao_video(&raiz, &PathBuf::from(caminho)).await {
                let _ = alterar_video(&app_tarefa, &jogo, &id, |v| v.duracao_segundos = Some(duracao)).await;
            }
        }
    });
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
        duracao_segundos: None,
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
    iniciar_download(&app, &game_id, &video.id, url, executavel)?;
    Ok(video)
}

/// Tenta de novo o download de um vídeo do YouTube que falhou ou foi cancelado.
#[tauri::command]
pub async fn validacao_baixar_de_novo(app: AppHandle, game_id: String, video_id: String) -> Result<(), String> {
    conferir_jogo(&game_id)?;
    let executavel = localizar_ytdlp(&app).ok_or_else(|| "yt-dlp não encontrado: instale pela tela de Validação".to_string())?;
    let url = alterar(&app, &game_id, |dataset| {
        let video = dataset.videos.iter_mut().find(|v| v.id == video_id).ok_or("vídeo não encontrado")?;
        let Origem::Youtube { url } = &video.origem else {
            return Err("só vídeos do YouTube podem ser baixados de novo".to_string());
        };
        if !matches!(video.estado, Estado::Falhou { .. }) {
            return Err("este vídeo não está com falha no download".to_string());
        }
        let url = url.clone();
        video.estado = Estado::Baixando { progresso: 0.0, sessao: sessao() };
        Ok(url)
    })
    .await?;
    iniciar_download(&app, &game_id, &video_id, url, executavel)
}

/// Cancela o que estiver em andamento no vídeo: tira da fila, ou interrompe o download ou a
/// análise (o processo externo é encerrado).
#[tauri::command]
pub async fn validacao_cancelar(app: AppHandle, game_id: String, video_id: String) -> Result<(), String> {
    conferir_jogo(&game_id)?;
    let sinalizar = alterar(&app, &game_id, |dataset| {
        let video = dataset.videos.iter_mut().find(|v| v.id == video_id).ok_or("vídeo não encontrado")?;
        match video.estado {
            Estado::NaFila { .. } => {
                video.estado = estado_de_repouso(video);
                Ok(false)
            }
            Estado::Baixando { .. } | Estado::Analisando { .. } => Ok(true),
            _ => Err("nada em andamento neste vídeo".to_string()),
        }
    })
    .await?;
    if sinalizar {
        if let Some(tx) = app.state::<ValidacaoState>().cancelamentos.lock().await.get(&video_id) {
            let _ = tx.send(true);
        }
    }
    Ok(())
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
            duracao_segundos: None,
            gabarito: Gabarito::default(),
            estado: Estado::Pronto,
            resultado: None,
        });
    }
    let quantidade = videos.len();
    alterar(&app, &game_id, |dataset| {
        dataset.videos.extend(videos);
        Ok(())
    })
    .await?;
    // A duração dos novos é lida em segundo plano quando a lista é relida (`medir_duracoes`).
    Ok(quantidade)
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

/// Remove o vídeo do banco. Um download em andamento é cancelado; um vídeo baixado pelo Lab tem
/// a pasta apagada; um arquivo local só deixa de ser referenciado (o original fica intacto).
#[tauri::command]
pub async fn validacao_remover(app: AppHandle, game_id: String, video_id: String) -> Result<(), String> {
    conferir_jogo(&game_id)?;
    let repo = repositorio(&app)?;
    let removido = alterar(&app, &game_id, |dataset| {
        let posicao = dataset.videos.iter().position(|v| v.id == video_id).ok_or("vídeo não encontrado")?;
        if matches!(dataset.videos[posicao].estado, Estado::Analisando { .. }) {
            return Err("cancele a análise deste vídeo antes de removê-lo".to_string());
        }
        Ok(dataset.videos.remove(posicao))
    })
    .await?;
    if matches!(removido.estado, Estado::Baixando { .. }) {
        // A tarefa de download apaga a própria pasta ao ser cancelada.
        if let Some(tx) = app.state::<ValidacaoState>().cancelamentos.lock().await.get(&removido.id) {
            let _ = tx.send(true);
        }
        return Ok(());
    }
    if matches!(removido.origem, Origem::Youtube { .. }) {
        let pasta = repo.pasta_videos(&game_id, &removido.id);
        if repo.e_pasta_propria(&pasta) {
            let _ = std::fs::remove_dir_all(pasta);
        }
    }
    Ok(())
}

/// Próximo número de ordem da fila: crescente durante toda a execução do Lab.
fn proxima_ordem() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static ORDEM: AtomicU64 = AtomicU64::new(1);
    ORDEM.fetch_add(1, Ordering::Relaxed)
}

/// Põe os vídeos na fila de análise e processa um por vez em segundo plano, sempre o de menor
/// ordem (a posição que a tela mostra). Devolve quantos entraram (vídeos sem arquivo, baixando ou
/// já na fila são ignorados).
#[tauri::command]
pub async fn validacao_rodar(app: AppHandle, game_id: String, versao: String, video_ids: Vec<String>) -> Result<usize, String> {
    conferir_jogo(&game_id)?;
    if !gerador::versao_valida(&versao) {
        return Err("versão de script inválida".into());
    }
    let raiz = script::raiz_validadores()?;
    let enfileirados = alterar(&app, &game_id, |dataset| {
        let mut ids = Vec::new();
        for video in dataset.videos.iter_mut().filter(|v| video_ids.contains(&v.id)) {
            let pode = video.caminho.is_some() && matches!(video.estado, Estado::Pronto | Estado::Concluido);
            if pode {
                video.estado = Estado::NaFila { sessao: sessao(), ordem: proxima_ordem() };
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
        // Uma vez por vídeo enfileirado aqui; cada vez roda o primeiro da fila (pode ser de outro
        // clique em Rodar). Quando a fila esvazia, as tarefas que sobraram não acham nada e saem.
        for _ in 0..enfileirados.len() {
            let _vez = estado.fila.lock().await;
            // Lido agora: vídeos podem ter sido removidos ou tirados da fila enquanto esperavam.
            let proximo = {
                let _trava = estado.gravacao.lock().await;
                repositorio(&app_tarefa).and_then(|repo| repo.carregar(&jogo)).ok().and_then(|d| d.proximo_da_fila())
            };
            let Some((id, caminho)) = proximo else { break };
            let cancelar = registrar_cancelamento(&app_tarefa, &id).await;
            let _ = alterar_video(&app_tarefa, &jogo, &id, |v| v.estado = Estado::Analisando { progresso: 0.0, sessao: sessao() }).await;
            let (tx, rx) = watch::channel(0.0f32);
            let progresso = acompanhar(&app_tarefa, &jogo, &id, rx, progresso_da_analise);
            let resultado = script::analisar(&raiz, &jogo, &versao, &PathBuf::from(caminho), tx, cancelar).await;
            let _ = progresso.await;
            liberar_cancelamento(&app_tarefa, &id).await;
            let _ = alterar_video(&app_tarefa, &jogo, &id, |v| match resultado {
                Some(resultado) => {
                    v.resultado = Some(resultado);
                    v.estado = Estado::Concluido;
                }
                // Cancelada: volta ao que era, mantendo o resultado anterior, se houver.
                None => v.estado = estado_de_repouso(v),
            })
            .await;
        }
    });
    Ok(quantidade)
}

/// Arquivos de uma versão do script do jogo, só para leitura na tela.
#[tauri::command]
pub async fn validacao_script_fonte(game_id: String, versao: String) -> Result<Vec<ArquivoFonte>, String> {
    conferir_jogo(&game_id)?;
    fonte::ler(&script::raiz_validadores()?, &game_id, &versao)
}

/// Cria o primeiro script do jogo (0.1.0, rascunho) a partir da calibração do Earthion.
/// Não sobrescreve nada que já exista.
#[tauri::command]
pub async fn validacao_gerar_script(game_id: String, titulo: String) -> Result<ScriptGerado, String> {
    conferir_jogo(&game_id)?;
    gerador::gerar(&script::raiz_validadores()?, &game_id, &titulo)
}

/// Cria a próxima versão do script (rascunho) copiando calibração e imagens de `de_versao`.
#[tauri::command]
pub async fn validacao_nova_versao(game_id: String, de_versao: String) -> Result<ScriptGerado, String> {
    conferir_jogo(&game_id)?;
    gerador::nova_versao(&script::raiz_validadores()?, &game_id, &de_versao)
}

/// Escolhe o navegador de onde o yt-dlp lê os cookies do YouTube (`None` desliga).
#[tauri::command]
pub async fn validacao_definir_cookies(app: AppHandle, navegador: Option<String>) -> Result<(), String> {
    ytdlp::salvar_navegador(&pasta_validacao(&app)?, navegador.as_deref())
}

/// Instala (ou atualiza) o yt-dlp oficial na pasta do Lab, conferindo o SHA-256 da release.
#[tauri::command]
pub async fn validacao_instalar_ytdlp(app: AppHandle) -> Result<Item, String> {
    let destino = ytdlp::caminho_gerenciado(&pasta_validacao(&app)?);
    let versao = ytdlp::instalar(&destino).await?;
    Ok(Item { ok: true, detalhe: format!("yt-dlp {versao}") })
}
