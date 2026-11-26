//! Ranking local: toda run válida que o servidor aceitou fica neste PC, por conta
//! (`ranking-local.json`, nos dados do app). O servidor guarda só o recorde de cada jogador (o ranking
//! online); o histórico das runs fica aqui. Esta tabela não entra em nenhuma comparação: o recorde
//! online é sempre a run atual contra o que está no banco, então apagar ou mexer neste arquivo não
//! muda o ranking online.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

/// A tabela mudou (a tela recarrega).
pub const EVENTO: &str = "ranking-local://mudou";
/// Runs guardadas por conta (as mais antigas saem primeiro).
const MAXIMO_POR_CONTA: usize = 5000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntradaLocal {
    pub run_id: String,
    pub jogo_id: String,
    /// Conta do Firebase dona da run: outra conta no mesmo PC não vê.
    pub uid: Option<String>,
    pub score: i64,
    pub vidas_perdidas: i32,
    pub continues: i32,
    /// O jogador zerou o jogo.
    #[serde(default)]
    pub zerou: bool,
    /// Rota percorrida (letras maiúsculas seguidas) e bombas usadas, nos jogos em que o motor lê.
    #[serde(default)]
    pub rota: Option<String>,
    #[serde(default)]
    pub bombas: Option<i32>,
    /// Nave/personagem escolhido e, no Caravan, o tempo vivido em ms.
    #[serde(default)]
    pub nave: Option<String>,
    #[serde(default)]
    pub sobrevivencia_ms: Option<i32>,
    /// Fase mais longe alcançada.
    #[serde(default)]
    pub estagio: Option<i32>,
    /// `highscore`, `caravan-sprint`...; `None` nas runs sem modo escolhido.
    #[serde(default)]
    pub modo: Option<String>,
    /// Quando a run foi aceita (ms desde a época Unix).
    pub quando: u64,
    /// A run ficou no ranking online (era o recorde do jogador quando foi enviada).
    #[serde(default)]
    pub online: bool,
}

static ARQUIVO: Mutex<()> = Mutex::new(());

fn caminho(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|pasta| pasta.join("ranking-local.json"))
}

fn ler(app: &AppHandle) -> Vec<EntradaLocal> {
    caminho(app)
        .and_then(|arquivo| std::fs::read(arquivo).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Acrescenta (ou atualiza, pelo `run_id`) a run e mantém no máximo [`MAXIMO_POR_CONTA`] por conta.
fn acrescentar(lista: &mut Vec<EntradaLocal>, entrada: EntradaLocal) {
    lista.retain(|e| e.run_id != entrada.run_id);
    let uid = entrada.uid.clone();
    lista.push(entrada);
    let mut da_conta: Vec<(u64, String)> = lista.iter().filter(|e| e.uid == uid).map(|e| (e.quando, e.run_id.clone())).collect();
    if da_conta.len() > MAXIMO_POR_CONTA {
        da_conta.sort();
        let sobra = da_conta.len() - MAXIMO_POR_CONTA;
        let sair: std::collections::HashSet<String> = da_conta.into_iter().take(sobra).map(|(_, id)| id).collect();
        lista.retain(|e| !sair.contains(&e.run_id));
    }
}

/// Guarda uma run aceita pelo servidor no ranking local e avisa a tela.
pub fn registrar(app: &AppHandle, entrada: EntradaLocal) {
    let _trava = ARQUIVO.lock().unwrap_or_else(|p| p.into_inner());
    let Some(arquivo) = caminho(app) else { return };
    let mut lista = ler(app);
    let jogo = entrada.jogo_id.clone();
    acrescentar(&mut lista, entrada);
    if let Some(pasta) = arquivo.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    match serde_json::to_vec(&lista) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(&arquivo, bytes) {
                log::warn!("ranking local: não foi possível salvar: {e}");
                return;
            }
            let _ = app.emit(EVENTO, jogo);
        }
        Err(e) => log::warn!("ranking local: {e}"),
    }
}

/// Ranking local de um jogo, da conta `uid`: do maior placar para o menor (no empate, a mais antiga).
#[tauri::command]
pub fn ranking_local(app: AppHandle, uid: Option<String>, jogo_id: String) -> Vec<EntradaLocal> {
    let _trava = ARQUIVO.lock().unwrap_or_else(|p| p.into_inner());
    let mut lista: Vec<EntradaLocal> = ler(&app).into_iter().filter(|e| e.jogo_id == jogo_id && (uid.is_none() || e.uid == uid)).collect();
    lista.sort_by(|a, b| b.score.cmp(&a.score).then(a.quando.cmp(&b.quando)));
    lista
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(run: &str, uid: &str, quando: u64) -> EntradaLocal {
        EntradaLocal { run_id: run.into(), jogo_id: "aero".into(), uid: Some(uid.into()), score: 10, vidas_perdidas: 0, continues: 0, zerou: false, rota: None, bombas: None, nave: None, sobrevivencia_ms: None, estagio: None, modo: None, quando, online: false }
    }

    #[test]
    fn mesma_run_nao_repete() {
        let mut lista = vec![e("r1", "ana", 1)];
        let mut de_novo = e("r1", "ana", 2);
        de_novo.online = true;
        acrescentar(&mut lista, de_novo);
        assert_eq!(lista.len(), 1);
        assert!(lista[0].online);
    }

    #[test]
    fn json_antigo_sem_campos_novos_ainda_le() {
        let json = r#"[{"run_id":"r","jogo_id":"aero","uid":"ana","score":5,"vidas_perdidas":1,"continues":0,"quando":3}]"#;
        let lista: Vec<EntradaLocal> = serde_json::from_str(json).unwrap();
        assert_eq!(lista[0].modo, None);
        assert!(!lista[0].online);
    }
}
