//! Telemetria do ShmupX Engine: o plugin `shmupx` do motor lê a memória do jogo e escreve, na
//! saída padrão, uma linha `SHMUPX {json}` por evento (início, run, campo mudou, game over). O
//! Client guarda o que chegou de cada processo do motor e, no fim da run, usa o placar da memória
//! em vez de analisar o vídeo, nos jogos de `jogos-pela-memoria.json` ([`rom_pela_memoria`]).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;

/// Jogo do catálogo cuja run ranked é validada pela memória, e o ROM do mapa dele no ShmupX Engine
/// (`plugins/shmupx/maps/<rom>.lua`).
#[derive(Debug, Deserialize)]
struct JogoPelaMemoria {
    game: String,
    rom: String,
}

/// Jogos validados pela memória (placar exato, sem script de vídeo): `jogos-pela-memoria.json`, o
/// mesmo arquivo que a interface lê para liberar o Ranked. Os outros jogos continuam pelo vídeo.
fn jogos_pela_memoria() -> &'static [JogoPelaMemoria] {
    static LISTA: OnceLock<Vec<JogoPelaMemoria>> = OnceLock::new();
    LISTA.get_or_init(|| serde_json::from_str(include_str!("../../jogos-pela-memoria.json")).expect("jogos-pela-memoria.json inválido"))
}

/// A run deste ROM é validada pela memória.
pub fn rom_pela_memoria(rom: &str) -> bool {
    jogos_pela_memoria().iter().any(|j| j.rom == rom)
}

/// ROMs mapeados no ShmupX Engine para este jogo do catálogo (vazio: o jogo não vale pela memória).
pub fn roms_pela_memoria(jogo_id: &str) -> Vec<String> {
    jogos_pela_memoria().iter().filter(|j| j.game == jogo_id).map(|j| j.rom.clone()).collect()
}

/// Velocidade média aceita na run ranked (100 = a da placa original).
const VELOCIDADE_MIN: f64 = 98.5;
const VELOCIDADE_MAX: f64 = 101.5;

/// Como a primeira run do processo terminou.
#[derive(Debug, Clone, PartialEq)]
pub enum Fim {
    /// O jogo deu game over (o motor fecha sozinho em seguida).
    GameOver { velocidade: f64 },
    /// A partida acabou sem game over (o jogador fechou o motor).
    Abandono { velocidade: f64 },
}

/// O que chegou do motor de um processo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Telemetria {
    pub rom: Option<String>,
    pub mapa: bool,
    pub ranked: bool,
    pub estado_inicial: bool,
    pub run_comecou: bool,
    pub placar: Option<i64>,
    pub vidas_no_inicio: Option<i64>,
    pub vidas: Option<i64>,
    pub nave: Option<String>,
    pub fim: Option<Fim>,
    /// O jogador zerou o jogo (evento `cleared` do motor, pelo `cleared` do mapa do jogo).
    pub zerou: bool,
    /// O motor fechou (fim da saída padrão).
    pub encerrado: bool,
    /// O motor pausou o jogo e mostrou o resultado (game over ou fim do tempo do Caravan): a partida
    /// já foi colhida, e a janela fica aberta para o jogador fechar.
    pub resultado_mostrado: bool,
}

#[derive(Deserialize)]
struct Evento {
    ev: String,
    #[serde(default)]
    rom: Option<String>,
    #[serde(default)]
    map: Option<String>,
    #[serde(default)]
    ranked: Option<bool>,
    #[serde(default)]
    start_state: Option<bool>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    value: Option<i64>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    speed: Option<f64>,
    #[serde(default)]
    cleared: Option<bool>,
}

impl Telemetria {
    /// Aplica uma linha da saída do motor. Só a primeira run do processo conta: o que vem depois
    /// do fim dela (reset, outra partida) não muda o resultado.
    pub fn aplicar(&mut self, linha: &str) {
        let Some(json) = linha.trim().strip_prefix("SHMUPX ") else { return };
        let Ok(evento) = serde_json::from_str::<Evento>(json) else { return };
        if evento.ev == "result" {
            self.resultado_mostrado = true;
            return;
        }
        if self.fim.is_some() {
            return;
        }
        match evento.ev.as_str() {
            "start" if !self.run_comecou => {
                self.rom = evento.rom;
                self.mapa = evento.map.as_deref() == Some("ok");
                self.ranked = evento.ranked.unwrap_or(false);
                self.estado_inicial = evento.start_state.unwrap_or(false);
            }
            "run_start" => self.run_comecou = true,
            "field" if self.run_comecou => match (evento.name.as_deref(), evento.value) {
                (Some("score"), Some(v)) => self.placar = Some(v),
                (Some("lives"), Some(v)) => {
                    self.vidas_no_inicio.get_or_insert(v);
                    self.vidas = Some(v);
                }
                (Some("ship"), _) => self.nave = evento.label.or(evento.value.map(|v| v.to_string())),
                _ => {}
            },
            "cleared" if self.run_comecou => self.zerou = true,
            "gameover" if self.run_comecou => {
                self.zerou |= evento.cleared.unwrap_or(false);
                self.fim = Some(Fim::GameOver { velocidade: evento.speed.unwrap_or(0.0) });
            }
            "abort" if self.run_comecou => self.fim = Some(Fim::Abandono { velocidade: evento.speed.unwrap_or(0.0) }),
            _ => {}
        }
    }
}

/// Resultado da run lido da memória.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoMemoria {
    pub placar: i64,
    pub vidas_perdidas: i32,
    pub game_over: bool,
    pub nave: Option<String>,
    /// Zerou o jogo: com 0 continues (o Ranked não tem continue), a run entra também na tabela 1CC.
    pub zerou: bool,
}

/// Decide a run pela telemetria. `Ok(None)`: este jogo/processo não vale pela memória (outro jogo,
/// emulador sem telemetria) e a run segue pelo vídeo. `Err`: a telemetria mostra que a
/// partida não vale (fora do modo ranking, sem o estado inicial, velocidade alterada).
pub fn resultado(t: &Telemetria) -> Result<Option<ResultadoMemoria>, String> {
    let Some(rom) = t.rom.as_deref() else { return Ok(None) };
    if !rom_pela_memoria(rom) || !t.mapa {
        return Ok(None);
    }
    if !t.ranked {
        return Err("a partida não abriu no modo ranking do ShmupX Emulator".into());
    }
    if !t.estado_inicial {
        return Err("a partida não começou do estado inicial do ShmupX (tela de start com 1 crédito)".into());
    }
    if !t.run_comecou {
        return Err("a partida não chegou a começar".into());
    }
    let (game_over, velocidade) = match t.fim {
        Some(Fim::GameOver { velocidade }) => (true, velocidade),
        Some(Fim::Abandono { velocidade }) => (false, velocidade),
        None => return Err("o ShmupX Emulator fechou sem informar o fim da partida".into()),
    };
    if !(VELOCIDADE_MIN..=VELOCIDADE_MAX).contains(&velocidade) {
        return Err(format!("a velocidade da partida ficou fora do normal ({velocidade:.1}%)"));
    }
    let placar = t.placar.ok_or("o placar não foi lido da memória")?;
    let vidas_perdidas = match (t.vidas_no_inicio, t.vidas) {
        (Some(inicio), Some(fim)) => (inicio - fim).max(0) as i32,
        _ => 0,
    };
    Ok(Some(ResultadoMemoria { placar, vidas_perdidas, game_over, nave: t.nave.clone(), zerou: t.zerou }))
}

fn registro() -> &'static Mutex<HashMap<u32, Telemetria>> {
    static REGISTRO: OnceLock<Mutex<HashMap<u32, Telemetria>>> = OnceLock::new();
    REGISTRO.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Lê a saída do motor (processo `pid`) numa thread, guardando a telemetria até a run pedir.
pub fn acompanhar(pid: u32, saida: impl Read + Send + 'static) {
    if let Ok(mut mapa) = registro().lock() {
        // Um pid antigo pode ter sido reaproveitado pelo Windows: começa do zero.
        mapa.insert(pid, Telemetria::default());
    }
    std::thread::spawn(move || {
        for linha in BufReader::new(saida).lines().map_while(Result::ok) {
            if !linha.starts_with("SHMUPX ") {
                continue;
            }
            log::info!("motor {pid}: {linha}");
            if let Ok(mut mapa) = registro().lock() {
                mapa.entry(pid).or_default().aplicar(&linha);
            }
        }
        if let Ok(mut mapa) = registro().lock() {
            mapa.entry(pid).or_default().encerrado = true;
        }
    });
}

/// O motor do processo `pid` mostrou o resultado e pausou o jogo.
pub fn resultado_mostrado(pid: u32) -> bool {
    registro().lock().ok().and_then(|mapa| mapa.get(&pid).map(|t| t.resultado_mostrado)).unwrap_or(false)
}

/// Telemetria do processo `pid` (e a retira do registro). Espera até `espera` o motor terminar de
/// escrever (a run acaba quando o processo fecha; a última linha pode chegar logo depois).
pub async fn tomar(pid: u32, espera: Duration) -> Option<Telemetria> {
    let limite = Instant::now() + espera;
    loop {
        let pronto = registro().lock().ok()?.get(&pid).map(|t| t.encerrado || t.fim.is_some());
        match pronto {
            None => return None,
            Some(true) => break,
            Some(false) if Instant::now() >= limite => break,
            Some(false) => tokio::time::sleep(Duration::from_millis(200)).await,
        }
    }
    registro().lock().ok()?.remove(&pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(linhas: &[&str]) -> Telemetria {
        let mut t = Telemetria::default();
        for l in linhas {
            t.aplicar(l);
        }
        t
    }

    const INICIO: &str = r#"SHMUPX {"ev":"start","rom":"sonicwi2","map":"ok","ranked":true,"start_state":true,"frame":0}"#;

    #[test]
    fn run_com_game_over_vale_pela_memoria() {
        let t = run(&[
            INICIO,
            r#"SHMUPX {"ev":"run_start","frame":1328}"#,
            r#"SHMUPX {"ev":"field","name":"score","value":0,"frame":1328}"#,
            r#"SHMUPX {"ev":"field","name":"lives","value":2,"frame":1328}"#,
            r#"SHMUPX {"ev":"field","name":"ship","value":6,"label":"Rafale","frame":1328}"#,
            r#"SHMUPX {"ev":"field","name":"score","value":123400,"frame":9000}"#,
            r#"SHMUPX {"ev":"field","name":"lives","value":-1,"frame":9100}"#,
            r#"SHMUPX {"ev":"gameover","frame":9101,"speed":100.002}"#,
            // depois do fim, nada muda o resultado
            r#"SHMUPX {"ev":"field","name":"score","value":999999,"frame":9200}"#,
        ]);
        assert_eq!(
            resultado(&t),
            Ok(Some(ResultadoMemoria { placar: 123400, vidas_perdidas: 3, game_over: true, nave: Some("Rafale".into()), zerou: false }))
        );
    }

    #[test]
    fn run_zerada_marca_zerou() {
        let t = run(&[
            INICIO,
            r#"SHMUPX {"ev":"run_start","frame":1328}"#,
            r#"SHMUPX {"ev":"field","name":"score","value":0,"frame":1328}"#,
            r#"SHMUPX {"ev":"field","name":"score","value":4500000,"frame":88000}"#,
            r#"SHMUPX {"ev":"cleared","frame":88000}"#,
            r#"SHMUPX {"ev":"gameover","frame":90000,"speed":100.0,"cleared":true}"#,
        ]);
        let r = resultado(&t).unwrap().unwrap();
        assert!(r.zerou);
        assert_eq!(r.placar, 4500000);
        // Sem o evento cleared, o gameover sozinho com "cleared": true também marca.
        let t = run(&[INICIO, r#"SHMUPX {"ev":"run_start","frame":1}"#, r#"SHMUPX {"ev":"field","name":"score","value":10,"frame":2}"#,
            r#"SHMUPX {"ev":"gameover","frame":3,"speed":100.0,"cleared":true}"#]);
        assert!(resultado(&t).unwrap().unwrap().zerou);
    }

    #[test]
    fn outro_jogo_ou_sem_telemetria_segue_pelo_video() {
        assert_eq!(resultado(&Telemetria::default()), Ok(None));
        let t = run(&[r#"SHMUPX {"ev":"start","rom":"ddp3","map":"ok","ranked":true,"start_state":true,"frame":0}"#]);
        assert_eq!(resultado(&t), Ok(None));
    }

    #[test]
    fn partida_que_nao_vale_e_recusada() {
        let sem_ranking = run(&[r#"SHMUPX {"ev":"start","rom":"sonicwi2","map":"ok","ranked":false,"start_state":false,"frame":0}"#]);
        assert!(resultado(&sem_ranking).is_err());
        let sem_estado = run(&[r#"SHMUPX {"ev":"start","rom":"sonicwi2","map":"ok","ranked":true,"start_state":false,"frame":0}"#]);
        assert!(resultado(&sem_estado).is_err());
        let rapida = run(&[
            INICIO,
            r#"SHMUPX {"ev":"run_start","frame":10}"#,
            r#"SHMUPX {"ev":"field","name":"score","value":5000,"frame":10}"#,
            r#"SHMUPX {"ev":"gameover","frame":90,"speed":180.0}"#,
        ]);
        assert!(resultado(&rapida).unwrap_err().contains("velocidade"));
        let sem_fim = run(&[INICIO, r#"SHMUPX {"ev":"run_start","frame":10}"#]);
        assert!(resultado(&sem_fim).is_err());
    }

    #[test]
    fn lista_dos_jogos_pela_memoria() {
        for rom in ["sonicwi2", "dariusg", "zedblade", "ket", "grindstm", "bgaregga", "cawing", "ddonpach", "espgal2", "raidendx"] {
            assert!(rom_pela_memoria(rom), "{rom}");
        }
        assert!(!rom_pela_memoria("ddp3"));
        assert_eq!(roms_pela_memoria("darius-gaiden"), ["dariusg"]);
        assert_eq!(roms_pela_memoria("dodonpachi"), ["ddonpach"]);
        assert!(roms_pela_memoria("dodonpachi-daioujou").is_empty());
    }

    #[test]
    fn outro_jogo_mapeado_vale_pela_memoria() {
        let t = run(&[
            r#"SHMUPX {"ev":"start","rom":"bgaregga","map":"ok","ranked":true,"start_state":true,"frame":0}"#,
            r#"SHMUPX {"ev":"run_start","frame":129}"#,
            r#"SHMUPX {"ev":"field","name":"lives","value":2,"frame":138}"#,
            r#"SHMUPX {"ev":"field","name":"score","value":7740,"frame":2423}"#,
            r#"SHMUPX {"ev":"field","name":"lives","value":0,"frame":2500}"#,
            r#"SHMUPX {"ev":"gameover","frame":2618,"speed":100.0}"#,
        ]);
        assert_eq!(resultado(&t).unwrap().map(|r| (r.placar, r.game_over)), Some((7740, true)));
    }

    #[test]
    fn fechar_antes_do_game_over_vale_o_placar_feito() {
        let t = run(&[
            INICIO,
            r#"SHMUPX {"ev":"run_start","frame":10}"#,
            r#"SHMUPX {"ev":"field","name":"score","value":45600,"frame":10}"#,
            r#"SHMUPX {"ev":"field","name":"lives","value":2,"frame":10}"#,
            r#"SHMUPX {"ev":"abort","frame":5000,"speed":99.99}"#,
        ]);
        assert_eq!(resultado(&t).unwrap().map(|r| (r.placar, r.game_over)), Some((45600, false)));
    }
}
