use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::analysis;
use crate::net::backend;
use crate::session::SessionState;

use super::ffmpeg;
use super::pendentes::{self, RunPendente};
use super::state::{AnalysisQueue, RunHandle, RunState};

#[derive(Debug, Clone, Serialize)]
struct RunExpiredPayload {
    jogo_id: String,
    pid: u32,
}

/// Inicia uma corrida. Grava a janela do jogo só quando precisa:
/// - jogo validado pelo vídeo: grava sempre, para a validação; sem "Gravar gameplays", o vídeo é
///   apagado depois que a run é enviada;
/// - "Gravar gameplays" marcado (Configurações › Armazenamento): grava toda run e o vídeo fica;
/// - live: o vídeo fica no YouTube, nunca na pasta (só o temporário da validação, se precisar);
/// - jogo do ShmupX Emulator validado pela memória (`rom` em `jogos-pela-memoria.json`), sem a
///   opção: não grava; a telemetria do motor decide a run.
///
/// Com a gravação confirmada (se houver), debita a ficha no backend (`POST /api/runs/start`) e usa a
/// `max_duration_segundos` devolvida para o timer seguro. Uma gravação que não inicia não custa ficha.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn iniciar_corrida_safe(
    app: AppHandle,
    state: State<'_, RunState>,
    session: State<'_, SessionState>,
    live: State<'_, crate::live::LiveState>,
    jogo_id: String,
    // Processo do jogo (evento `steam://game-launched`) e app_id: ao fechar o jogo, a gravação para.
    jogo_pid: Option<u32>,
    steam_app_id: Option<u32>,
    // Modo escolhido no Play Ranked (`highscore`, `1cc`, `caravan`): vai para a gameplay.
    modo: Option<String>,
    // Evento em que a partida vale (o jogador se inscreveu): o backend confere e grava o `eventId`.
    evento: Option<String>,
    // ROM aberto no ShmupX Emulator (`emulator://game-launched`): decide se a run vale pela memória.
    rom: Option<String>,
) -> Result<u32, String> {
    // Uma live no ar só transmite a run ranked do jogo dela (Play Ranked com live): a run sai
    // ligada à live, para o destaque do top 3. Live de outro jogo barra a run.
    let live_broadcast = live.broadcast_para_run(&jogo_id)?;
    let pela_memoria = steam_app_id.is_none()
        && jogo_pid.is_some()
        && rom.as_deref().is_some_and(crate::steam::telemetria::rom_pela_memoria);
    let manter_video = super::destino::gravar_gameplays(&app) && live_broadcast.is_none();
    let gravar = !pela_memoria || manter_video;
    if gravar && !ffmpeg::is_ffmpeg_available() {
        return Err("FFmpeg não encontrado: reinstale o ShmupX Client".into());
    }

    let mut guard = state
        .0
        .try_lock()
        .map_err(|_| "aguarde o processamento da corrida anterior".to_string())?;
    if guard.is_some() {
        return Err("já existe uma corrida em andamento".into());
    }

    let token = session.snapshot().token.ok_or_else(|| {
        "sem sessão: entre com sua conta Google antes de iniciar uma run".to_string()
    })?;

    let (gravacao, video_path, pid) = match jogo_pid.filter(|_| !gravar) {
        Some(jogo_pid) => {
            log::info!("run de {jogo_id} pela memória ({}): sem gravação", rom.as_deref().unwrap_or_default());
            (None, None, jogo_pid)
        }
        None => {
            // Pasta escolhida em Configurações › Armazenamento (padrão: dados do app). Cada run
            // preserva seu vídeo mesmo se outra começar durante a análise. O nome é local (a
            // gravação começa antes de o backend devolver o id da run).
            let data_dir = super::destino::pasta_das_gravacoes(&app)?;
            let carimbo = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default();
            let output_path = data_dir.join(format!("shmupx_run_{carimbo}.mp4"));
            let gravacao = match super::gravacao::iniciar_do_jogo(&output_path, jogo_pid, steam_app_id).await {
                Ok(g) => g,
                Err(erro) => {
                    let _ = std::fs::remove_file(&output_path);
                    return Err(erro);
                }
            };
            let Some(pid) = gravacao.pid() else {
                let _ = gravacao.cortar().await;
                let _ = std::fs::remove_file(&output_path);
                return Err("a gravação parou logo depois de começar".into());
            };
            (Some(gravacao), Some(output_path), pid)
        }
    };

    // Débito de ficha + abertura da "ficha ativa" no backend com a gravação já confirmada. Se o
    // backend recusar (sem saldo, jogo não publicado, token inválido), a gravação é descartada.
    let inicio = match backend::iniciar_run(&token, &jogo_id, modo.as_deref(), live_broadcast.as_deref(), evento.as_deref()).await {
        Ok(inicio) => inicio,
        Err(e) => {
            if let Some(gravacao) = gravacao {
                let _ = gravacao.cortar().await;
            }
            if let Some(video) = &video_path {
                let _ = std::fs::remove_file(video);
                let _ = std::fs::remove_file(super::gravacao::caminho_do_log(video));
            }
            return Err(format!("backend recusou iniciar a run: {e}"));
        }
    };
    let run_id = inicio.run_id.clone();
    if live_broadcast.is_some() {
        live.vincular_run(&jogo_id, &run_id);
    }

    *guard = Some(RunHandle {
        gravacao,
        pid,
        jogo_id: jogo_id.clone(),
        run_id: inicio.run_id,
        token: token.clone(),
        video_path,
        manter_video,
        jogo_pid,
        steam_app_id,
        modo: modo.clone(),
    });
    drop(guard);

    // `max_duration_segundos` negativo é impossível vindo de um backend correto, mas `as u64`
    // em um `i32` negativo faria um número gigante — `.max(0)` evita essa armadilha.
    let limite = std::time::Duration::from_secs(inicio.max_duration_segundos.max(0) as u64);
    if let Some(jogo_pid) = jogo_pid {
        spawn_game_watcher(app.clone(), state.0.clone(), jogo_id.clone(), run_id.clone(), jogo_pid, steam_app_id);
    }
    // Caravan na Steam: o jogo fecha no tempo do Caravan, como o PARAR JOGO, e o placar feito até
    // ali vale. No ShmupX Emulator o tempo é do motor (contador, pausa e resultado), que avisa o
    // Client; aqui não há timer.
    if let Some(segundos) = inicio.caravan_segundos.filter(|s| *s > 0).filter(|_| steam_app_id.is_some()) {
        log::info!("run {run_id}: Caravan de {segundos}s");
        spawn_caravan_timer(app.clone(), state.0.clone(), run_id.clone(), std::time::Duration::from_secs(segundos as u64));
    }
    spawn_safe_timer(app, state.0.clone(), jogo_id, run_id, limite);

    Ok(pid)
}

/// Fecha a gravação da run (se houver) com o vídeo completo.
async fn parar_gravacao(app: &AppHandle, handle: &mut RunHandle) -> Result<(), String> {
    let Some(gravacao) = handle.gravacao.take() else { return Ok(()) };
    gravacao.parar().await.map_err(|error| {
        emit_analysis(app, &handle.jogo_id, handle.pid, "failed", Some(format!("falha ao encerrar a gravação: {error}")));
        error.to_string()
    })
}

/// Fecha o jogo da run como o X da janela (o ShmupX Engine sai normalmente e a telemetria registra
/// o fim da partida); se não fechar em `PRAZO_PARA_FECHAR`, encerra o processo. Aberto por um
/// launcher que já fechou: fecha o processo que roda da pasta do jogo.
async fn fechar_o_jogo(jogo_pid: Option<u32>, steam_app_id: Option<u32>) {
    let Some(jogo_pid) = jogo_pid else { return };
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let alvo = if crate::steam::detector::processo_vivo(jogo_pid) {
            Some(jogo_pid)
        } else {
            steam_app_id
                .and_then(crate::steam::biblioteca::pasta_do_jogo)
                .and_then(|pasta| crate::steam::detector::processo_na_pasta(&pasta))
        };
        if let Some(pid) = alvo {
            log::info!("fechando o jogo {pid}");
            crate::steam::detector::fechar_jogo(pid, PRAZO_PARA_FECHAR);
        }
    })
    .await;
}

/// Tempo do Caravan: quando acaba, fecha o jogo e encerra a run como o PARAR JOGO (a análise tira o
/// placar feito até ali). A run que já acabou por outro caminho (PARAR JOGO, o jogo fechado, o
/// limite da ficha) não é tocada.
fn spawn_caravan_timer(app: AppHandle, run_state: std::sync::Arc<tokio::sync::Mutex<Option<RunHandle>>>, run_id: String, tempo: std::time::Duration) {
    tokio::spawn(async move {
        tokio::time::sleep(tempo).await;
        let mut guard = run_state.lock().await;
        if guard.as_ref().is_none_or(|handle| handle.run_id != run_id) {
            return;
        }
        let Some(mut handle) = guard.take() else { return };
        log::info!("Caravan: o tempo da run {run_id} acabou, fechando o jogo");
        fechar_o_jogo(handle.jogo_pid, handle.steam_app_id).await;
        if parar_gravacao(&app, &mut handle).await.is_err() {
            return;
        }
        drop(guard);
        let _ = app.emit("run://caravan-fim", GameClosedPayload { jogo_id: handle.jogo_id.clone(), pid: handle.pid });
        encerrar_e_enfileirar(&app, handle).await;
    });
}

/// O motor pausou o jogo e mostrou o resultado (game over ou fim do tempo do Caravan): a partida já
/// foi colhida, então a run se encerra (a análise e o envio seguem) e a janela do jogo continua
/// aberta com o resultado, para o jogador fechar à mão. Devolve `false` se a run ainda não existe.
pub async fn encerrar_run_pelo_resultado(app: &AppHandle, jogo_pid: u32) -> bool {
    let state = app.state::<RunState>();
    let mut guard = state.0.lock().await;
    if guard.as_ref().is_none_or(|handle| handle.jogo_pid != Some(jogo_pid)) {
        return false;
    }
    let Some(mut handle) = guard.take() else { return false };
    drop(guard);
    log::info!("o motor mostrou o resultado: encerrando a run {} sem fechar a janela", handle.run_id);
    let _ = parar_gravacao(app, &mut handle).await;
    encerrar_e_enfileirar(app, handle).await;
    true
}

/// Quanto o jogo tem para fechar sozinho no PARAR JOGO antes de o processo ser encerrado.
const PRAZO_PARA_FECHAR: std::time::Duration = std::time::Duration::from_secs(5);

/// PARAR JOGO de uma run: fecha o jogo (como o X da janela, para o ShmupX Engine registrar o fim
/// da partida), para a gravação, avisa o backend do fim da gameplay e põe a análise na fila.
/// Retorna logo depois: o jogador pode iniciar outra run enquanto esta é analisada.
#[tauri::command]
pub async fn finalizar_corrida_manual(
    app: AppHandle,
    state: State<'_, RunState>,
) -> Result<(), String> {
    let mut guard = state
        .0
        .try_lock()
        .map_err(|_| "aguarde a parada da corrida anterior".to_string())?;
    // Tirar a run do estado antes de fechar o jogo: o vigia do processo vê que ela acabou e não
    // a encerra de novo.
    let mut handle = guard
        .take()
        .ok_or("nenhuma corrida em andamento".to_string())?;
    fechar_o_jogo(handle.jogo_pid, handle.steam_app_id).await;
    parar_gravacao(&app, &mut handle).await?;
    // Com a gravação fechada, a próxima run já pode começar.
    drop(guard);
    encerrar_e_enfileirar(&app, handle).await;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct RunAnalysisPayload {
    jogo_id: String,
    pid: u32,
    /// Id da run no servidor, quando já existe (análise e envio; "Analisar de novo").
    run_id: Option<String>,
    status: String,
    message: Option<String>,
    /// Placar enviado (`submitted`).
    score: Option<i64>,
    /// Posição da run no ranking do jogo (`submitted`, quando é o melhor placar do jogador).
    position: Option<usize>,
    /// Pódio (top 3 do Hi-Score) do jogo já com esta run (`submitted`): a sala reordena se mudou.
    podium: Option<Vec<backend::LugarNoPodio>>,
    /// `submitted`: a run ficou no ranking online (é o recorde do jogador); senão, só no local.
    online: Option<bool>,
}

fn emit_analysis(app: &AppHandle, jogo_id: &str, pid: u32, status: &str, message: Option<String>) {
    emit_run(app, jogo_id, pid, None, status, message, None);
}

fn emit_run(app: &AppHandle, jogo_id: &str, pid: u32, run_id: Option<&str>, status: &str, message: Option<String>, score: Option<i64>) {
    let _ = app.emit(
        "run://analysis",
        RunAnalysisPayload {
            jogo_id: jogo_id.to_string(),
            pid,
            run_id: run_id.map(str::to_string),
            status: status.into(),
            message,
            score,
            position: None,
            podium: None,
            online: None,
        },
    );
}

/// Placar de uma run aceita (o que vai para o ranking local).
struct Placar {
    score: i64,
    vidas_perdidas: i32,
    continues: i32,
    zerou: bool,
    rota: Option<String>,
    bombas_usadas: Option<i32>,
    nave: Option<String>,
    sobrevivencia_ms: Option<i32>,
    estagio: Option<i32>,
}

/// Resultado aceito: guarda a run no ranking local, avisa a interface (placar, posição, pódio e se
/// ficou no ranking online) e decide a live da run: no top 3, vai para o canal oficial da ShmupX.
fn run_enviada(app: &AppHandle, job: &AnaliseJob, placar: Placar, resposta: backend::SubmeterRunResponse) {
    pendentes::remover(app, &job.run_id);
    let agora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    super::ranking_local::registrar(app, super::ranking_local::EntradaLocal {
        run_id: job.run_id.clone(),
        jogo_id: job.jogo_id.clone(),
        uid: pendentes::uid_do_token(&job.token),
        score: placar.score,
        vidas_perdidas: placar.vidas_perdidas,
        continues: placar.continues,
        zerou: placar.zerou,
        rota: placar.rota.clone(),
        bombas: placar.bombas_usadas,
        nave: placar.nave.clone(),
        sobrevivencia_ms: placar.sobrevivencia_ms,
        estagio: placar.estagio,
        modo: job.modo.clone(),
        quando: agora,
        online: resposta.online,
    });
    // Vídeo só para a validação ("Gravar gameplays" desmarcado ou live): sai depois do envio.
    if !job.manter_video {
        if let Some(video) = &job.video_path {
            apagar_video(video);
        }
    }
    let _ = app.emit(
        "run://analysis",
        RunAnalysisPayload {
            jogo_id: job.jogo_id.clone(),
            pid: job.pid,
            run_id: Some(job.run_id.clone()),
            status: "submitted".into(),
            message: None,
            score: Some(placar.score),
            position: resposta.posicao,
            podium: Some(resposta.podio),
            online: Some(resposta.online),
        },
    );
    match resposta.destaque {
        Some(destaque) => crate::live::destaque::enviar(app, &job.jogo_id, &job.run_id, destaque),
        None => crate::live::destaque::descartar(app, &job.run_id),
    }
}

/// O que a análise precisa depois que a captura terminou (sem o processo do FFmpeg).
struct AnaliseJob {
    pid: u32,
    jogo_id: String,
    run_id: String,
    token: String,
    /// `None`: run pela memória, sem vídeo.
    video_path: Option<PathBuf>,
    /// O vídeo fica na pasta ("Gravar gameplays" marcado, fora da live); senão, sai depois do envio.
    manter_video: bool,
    jogo_pid: Option<u32>,
    /// Modo escolhido no Play Ranked (vai para o ranking local).
    modo: Option<String>,
}

/// O ID token do Firebase vale ~1 h: uma análise longa na fila envia com o token atual da sessão.
fn token_atual(app: &AppHandle, guardado: &str) -> String {
    app.state::<SessionState>()
        .snapshot()
        .token
        .unwrap_or_else(|| guardado.to_string())
}

/// Registra no servidor o fim da gameplay e agenda a análise. Se o servidor disser que a
/// gameplay passou do tempo da ficha, não há o que analisar; uma falha de rede não impede a
/// análise (o envio ainda cabe na janela da gameplay).
async fn encerrar_e_enfileirar(app: &AppHandle, handle: RunHandle) {
    // Ranked com live: a live termina junto com a run.
    app.state::<crate::live::LiveState>().run_terminou(&handle.run_id);
    let job = AnaliseJob {
        pid: handle.pid,
        jogo_id: handle.jogo_id,
        run_id: handle.run_id,
        token: handle.token,
        video_path: handle.video_path,
        manter_video: handle.manter_video,
        jogo_pid: handle.jogo_pid,
        modo: handle.modo,
    };
    match backend::encerrar_run(&token_atual(app, &job.token), &job.run_id).await {
        Ok(()) => {}
        Err(backend::FalhaEncerramento::Recusado(erro)) => {
            log::warn!(
                "capture: run {} recusada no encerramento: {erro}",
                job.run_id
            );
            emit_analysis(
                app,
                &job.jogo_id,
                job.pid,
                "failed",
                Some(format!(
                    "vídeo preservado; a run foi encerrada pelo servidor: {erro}"
                )),
            );
            return;
        }
        Err(backend::FalhaEncerramento::Outro(erro)) => {
            log::warn!(
                "capture: aviso de encerramento da run {} falhou: {erro}",
                job.run_id
            );
        }
    }
    emit_run(app, &job.jogo_id, job.pid, Some(&job.run_id), "analyzing", None, None);
    let app = app.clone();
    tauri::async_runtime::spawn(async move { analisar_e_enviar(&app, job).await });
}

/// Espera a vez na fila, analisa o vídeo e envia o resultado. Sem prazo para a análise: o limite
/// de tempo é da gameplay, já registrado no encerramento.
async fn analisar_e_enviar(app: &AppHandle, job: AnaliseJob) {
    // Jogo validado pela memória (ShmupX Engine, ex.: Aero Fighters 2): o placar vem da telemetria
    // do motor, exato, sem analisar o vídeo. Partida que a telemetria mostra não valer (fora do
    // modo ranking, velocidade alterada) não é enviada nem pode ser analisada de novo pelo vídeo.
    let memoria = match job.jogo_pid {
        Some(pid) => crate::steam::telemetria::tomar(pid, std::time::Duration::from_secs(5)).await,
        None => None,
    };
    let pela_memoria = match memoria.as_ref().map(crate::steam::telemetria::resultado) {
        Some(Ok(Some(resultado))) => Some(resultado),
        Some(Err(motivo)) => {
            log::warn!("capture: run {} recusada pela telemetria: {motivo}", job.run_id);
            emit_run(app, &job.jogo_id, job.pid, Some(&job.run_id), "failed", Some(format!("a run não vale para o ranking: {motivo}")), None);
            return;
        }
        _ => None,
    };
    if let Some(resultado) = pela_memoria {
        log::info!(
            "capture: run {} pela memória: placar {}, vidas perdidas {}, nave {:?}, {}{}",
            job.run_id,
            resultado.placar,
            resultado.vidas_perdidas,
            resultado.nave,
            if resultado.game_over { "game over" } else { "encerrada antes do game over" },
            if resultado.zerou { ", zerou" } else { "" }
        );
        match backend::submeter_run(&token_atual(app, &job.token), &job.run_id, &job.jogo_id, resultado.placar, resultado.vidas_perdidas, 0, Some(resultado.zerou), resultado.rota.as_deref(), resultado.bombas_usadas, resultado.nave.as_deref(), resultado.sobrevivencia_ms, resultado.estagio).await {
            Ok(resposta) => run_enviada(app, &job, Placar { score: resultado.placar, vidas_perdidas: resultado.vidas_perdidas, continues: 0, zerou: resultado.zerou, rota: resultado.rota.clone(), bombas_usadas: resultado.bombas_usadas, nave: resultado.nave.clone(), sobrevivencia_ms: resultado.sobrevivencia_ms, estagio: resultado.estagio }, resposta),
            Err(error) => {
                log::warn!("capture: run {} (memória) não enviada: {error}", job.run_id);
                emit_run(app, &job.jogo_id, job.pid, Some(&job.run_id), "failed", Some(format!("falha ao enviar resultado: {error}")), None);
            }
        }
        return;
    }

    // Run pela memória (sem vídeo) cuja telemetria não chegou: não há o que analisar.
    let Some(video_path) = job.video_path.clone() else {
        log::warn!("capture: run {} sem telemetria do motor e sem vídeo", job.run_id);
        emit_run(app, &job.jogo_id, job.pid, Some(&job.run_id), "failed", Some("não foi possível ler o resultado da partida no ShmupX Emulator".into()), None);
        return;
    };
    let fila = app.state::<AnalysisQueue>().0.clone();
    let _vez = fila.lock().await;
    let result = async {
        let result = analysis::analisar_run(app, &video_path, &job.jogo_id).await?;
        let resposta = backend::submeter_run(
            &token_atual(app, &job.token),
            &job.run_id,
            &job.jogo_id,
            result.score_bruto,
            result.vidas_perdidas,
            result.continues_usados,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .map_err(|error| format!("vídeo preservado; falha ao enviar resultado: {error}"))?;
        let placar = Placar { score: result.score_bruto, vidas_perdidas: result.vidas_perdidas, continues: result.continues_usados, zerou: false, rota: None, bombas_usadas: None, nave: None, sobrevivencia_ms: None, estagio: None };
        Ok::<(Placar, backend::SubmeterRunResponse), String>((placar, resposta))
    }
    .await;
    match result {
        Ok((placar, resposta)) => run_enviada(app, &job, placar, resposta),
        Err(error) => {
            log::warn!("capture: run {} não enviada: {error}", job.run_id);
            // O vídeo fica: o jogador pode analisar de novo (Ranking da sala › Analisar de novo).
            pendentes::guardar(app, RunPendente {
                run_id: job.run_id.clone(),
                jogo_id: job.jogo_id.clone(),
                uid: pendentes::uid_do_token(&job.token),
                iniciada_em: pendentes::inicio_pelo_video(&video_path),
                encerrada_em: pendentes::fim_pelo_video(&video_path),
                video_path,
                temporario: !job.manter_video,
                erro: error.clone(),
            });
            emit_run(app, &job.jogo_id, job.pid, Some(&job.run_id), "failed", Some(error), None);
        }
    }
}

/// Runs "Não enviada" da conta (`uid`, ou a da sessão) com o vídeo ainda no disco.
#[tauri::command]
pub fn runs_pendentes(app: AppHandle, uid: Option<String>) -> Vec<RunPendente> {
    let uid = uid.or_else(|| app.state::<SessionState>().snapshot().token.as_deref().and_then(pendentes::uid_do_token));
    pendentes::listar(&app, uid.as_deref())
}

/// Analisa de novo o vídeo de uma run "Não enviada" e envia o resultado. Entra na mesma fila
/// das análises; o andamento chega pelo evento `run://analysis` com o `run_id`.
#[tauri::command]
pub fn reanalisar_run(app: AppHandle, run_id: String) -> Result<(), String> {
    let run = pendentes::buscar(&app, &run_id).ok_or("run não encontrada entre as não enviadas")?;
    if !run.video_path.is_file() {
        return Err("o vídeo desta run não está mais no disco".into());
    }
    let token = app
        .state::<SessionState>()
        .snapshot()
        .token
        .ok_or("sem sessão: entre com sua conta Google")?;
    let job = AnaliseJob { pid: 0, jogo_id: run.jogo_id, run_id: run.run_id, token, video_path: Some(run.video_path), manter_video: !run.temporario, jogo_pid: None, modo: None };
    emit_run(&app, &job.jogo_id, job.pid, Some(&job.run_id), "analyzing", None, None);
    tauri::async_runtime::spawn(async move { analisar_e_enviar(&app, job).await });
    Ok(())
}

/// Apaga o vídeo de uma run e o log do FFmpeg ao lado dele.
fn apagar_video(video: &std::path::Path) {
    let _ = std::fs::remove_file(video);
    let _ = std::fs::remove_file(super::gravacao::caminho_do_log(video));
}

/// Tira a run da lista de não enviadas. O vídeo continua na pasta das gravações, a não ser que
/// fosse só para a validação ("Gravar gameplays" desmarcado).
#[tauri::command]
pub fn descartar_run_pendente(app: AppHandle, run_id: String) {
    if let Some(run) = pendentes::buscar(&app, &run_id) {
        pendentes::remover(&app, &run_id);
        if run.temporario {
            apagar_video(&run.video_path);
        }
        emit_run(&app, &run.jogo_id, 0, Some(&run_id), "discarded", None, None);
    }
}

/// Intervalo entre as conferências do processo do jogo.
const VIGIA_JOGO: std::time::Duration = std::time::Duration::from_secs(2);
/// Conferências seguidas sem o jogo antes de parar (evita parar num instante de troca de processo).
const AUSENCIAS_PARA_PARAR: u32 = 2;

#[derive(Debug, Clone, Serialize)]
struct GameClosedPayload {
    jogo_id: String,
    pid: u32,
}

/// Fechou a janela do jogo: para a gravação como o STOP (fecha o vídeo, avisa o backend e põe a
/// análise na fila). Se quem abriu foi um launcher que já fechou, passa a vigiar o processo que
/// estiver rodando da pasta do jogo.
fn spawn_game_watcher(
    app: AppHandle,
    run_state: std::sync::Arc<tokio::sync::Mutex<Option<RunHandle>>>,
    jogo_id: String,
    run_id: String,
    jogo_pid: u32,
    steam_app_id: Option<u32>,
) {
    tokio::spawn(async move {
        let pasta = match steam_app_id {
            Some(app_id) => tauri::async_runtime::spawn_blocking(move || crate::steam::biblioteca::pasta_do_jogo(app_id)).await.ok().flatten(),
            None => None,
        };
        let mut vigiado = jogo_pid;
        let mut ausencias = 0;
        loop {
            tokio::time::sleep(VIGIA_JOGO).await;
            // A run acabou por outro caminho (STOP, limite da ficha): nada a fazer.
            if run_state.lock().await.as_ref().is_none_or(|handle| handle.run_id != run_id) {
                return;
            }
            let (pid, pasta_atual) = (vigiado, pasta.clone());
            let encontrado = tauri::async_runtime::spawn_blocking(move || {
                if crate::steam::detector::processo_vivo(pid) {
                    Some(pid)
                } else {
                    pasta_atual.as_deref().and_then(crate::steam::detector::processo_na_pasta)
                }
            })
            .await
            .ok()
            .flatten();
            match encontrado {
                Some(pid) => {
                    vigiado = pid;
                    ausencias = 0;
                }
                None => {
                    ausencias += 1;
                    if ausencias >= AUSENCIAS_PARA_PARAR {
                        break;
                    }
                }
            }
        }

        let mut guard = run_state.lock().await;
        if guard.as_ref().is_none_or(|handle| handle.run_id != run_id) {
            return;
        }
        let Some(mut handle) = guard.take() else { return };
        drop(guard);
        log::info!("o jogo fechou: encerrando a run {run_id}");
        let _ = app.emit("run://game-closed", GameClosedPayload { jogo_id, pid: handle.pid });
        if parar_gravacao(&app, &mut handle).await.is_err() {
            return;
        }
        encerrar_e_enfileirar(&app, handle).await;
    });
}

/// O timer pertence a uma run específica, não apenas ao jogo (pode haver outra run depois).
fn spawn_safe_timer(
    app: AppHandle,
    run_state: std::sync::Arc<tokio::sync::Mutex<Option<RunHandle>>>,
    jogo_id: String,
    run_id: String,
    limit: std::time::Duration,
) {
    tokio::spawn(async move {
        tokio::time::sleep(limit).await;
        let mut guard = run_state.lock().await;
        if guard.as_ref().is_none_or(|handle| handle.run_id != run_id) {
            return;
        }
        if let Some(mut handle) = guard.take() {
            drop(guard);
            if let Some(gravacao) = handle.gravacao.take() {
                if let Err(error) = gravacao.cortar().await {
                    emit_analysis(
                        &app,
                        &handle.jogo_id,
                        handle.pid,
                        "failed",
                        Some(format!("falha ao cortar a gravação: {error}")),
                    );
                    return;
                }
            }
            let _ = app.emit(
                "run://expired",
                RunExpiredPayload {
                    jogo_id,
                    pid: handle.pid,
                },
            );
            encerrar_e_enfileirar(&app, handle).await;
        }
    });
}
