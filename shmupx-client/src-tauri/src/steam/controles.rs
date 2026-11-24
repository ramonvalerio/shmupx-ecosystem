//! Controles do ShmupX Engine (Configurações › Controle): o jogador clica num botão do gabinete e
//! aperta o botão, tecla ou direção que quer usar.
//!
//! A leitura é feita pelo próprio motor, escondido e sem jogo (`___empty`, plugin `shmupx` com
//! `SHMUPX_CAPTURA=1`, ver `plugins/shmupx/captura.lua` no `shmupx-engine`): assim o código gravado
//! é exatamente o que o MAME entende (`JOYCODE_1_BUTTON3`, `KEYCODE_Z`), sem tradução. Cada aperto
//! vira o evento `engine://capture`.
//!
//! Ao abrir um jogo, o mapeamento vira o arquivo de controles do MAME (`ctrlr/shmupx.cfg`, opção
//! `-ctrlr`), que vale para todos os jogos.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use super::emulador::Motor;

/// Controle do painel do gabinete mapeado pelo jogador. `control`: um de [`CONTROLES`]; `code`:
/// código do MAME (`JOYCODE_1_BUTTON3`); `device`: id do aparelho (só controles), para o arquivo
/// prender o código a ele mesmo que a ordem dos controles mude.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControleMapeado {
    pub control: String,
    pub code: String,
    #[serde(default)]
    pub device: Option<String>,
}

/// Controles do painel: id do Client, tipo de porta do MAME e a sequência padrão do MAME (a do
/// GroovyMAME 0.289 para o jogador 1, sem os códigos de mouse e pistola).
const CONTROLES: [(&str, &str, &[&str]); 10] = [
    ("up", "P1_JOYSTICK_UP", &["KEYCODE_UP", "JOYCODE_1_YAXIS_UP_SWITCH", "JOYCODE_1_HAT1UP"]),
    ("down", "P1_JOYSTICK_DOWN", &["KEYCODE_DOWN", "JOYCODE_1_YAXIS_DOWN_SWITCH", "JOYCODE_1_HAT1DOWN"]),
    ("left", "P1_JOYSTICK_LEFT", &["KEYCODE_LEFT", "JOYCODE_1_XAXIS_LEFT_SWITCH", "JOYCODE_1_HAT1LEFT"]),
    ("right", "P1_JOYSTICK_RIGHT", &["KEYCODE_RIGHT", "JOYCODE_1_XAXIS_RIGHT_SWITCH", "JOYCODE_1_HAT1RIGHT"]),
    ("button1", "P1_BUTTON1", &["KEYCODE_LCONTROL", "JOYCODE_1_BUTTON1"]),
    ("button2", "P1_BUTTON2", &["KEYCODE_LALT", "JOYCODE_1_BUTTON2"]),
    ("button3", "P1_BUTTON3", &["KEYCODE_SPACE", "JOYCODE_1_BUTTON3"]),
    ("button4", "P1_BUTTON4", &["KEYCODE_LSHIFT", "JOYCODE_1_BUTTON4"]),
    ("start", "START1", &["KEYCODE_1", "JOYCODE_1_START"]),
    ("coin", "COIN1", &["KEYCODE_5", "JOYCODE_1_SELECT"]),
];

/// Código do MAME aceito no arquivo: letras maiúsculas, dígitos e `_` (nada que quebre o XML).
fn codigo_valido(code: &str) -> bool {
    !code.is_empty() && code.len() <= 48 && code.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// Índice do controle em `JOYCODE_<n>_...`.
fn indice_do_controle(code: &str) -> Option<u32> {
    code.strip_prefix("JOYCODE_")?.split('_').next()?.parse().ok()
}

/// Parte estável do id do aparelho para o `mapdevice` (o MAME compara por trecho, sem diferenciar
/// maiúsculas): `product_<guid>` do DirectInput quando houver; senão o id inteiro.
fn id_do_aparelho(device: &str) -> String {
    device
        .split_whitespace()
        .find(|parte| parte.starts_with("product_"))
        .map(str::to_string)
        .unwrap_or_else(|| device.to_string())
}

fn xml(texto: &str) -> String {
    texto.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Arquivo de controles do MAME para o mapeamento. Controle mapeado: só o código escolhido. Não
/// mapeado: o padrão do MAME, sem os códigos que o jogador deu a outro controle (senão um aperto
/// acionaria os dois). Sem mapeamento, `None`: valem os padrões.
pub fn arquivo_ctrlr(mapeamento: &[ControleMapeado]) -> Option<String> {
    let validos: Vec<&ControleMapeado> = mapeamento
        .iter()
        .filter(|m| codigo_valido(&m.code) && CONTROLES.iter().any(|(id, ..)| *id == m.control))
        .collect();
    if validos.is_empty() {
        return None;
    }
    let usados: BTreeSet<&str> = validos.iter().map(|m| m.code.as_str()).collect();
    let mut aparelhos: BTreeMap<u32, String> = BTreeMap::new();
    for m in &validos {
        if let (Some(n), Some(device)) = (indice_do_controle(&m.code), m.device.as_deref()) {
            aparelhos.entry(n).or_insert_with(|| id_do_aparelho(device));
        }
    }
    let mut saida = String::from("<?xml version=\"1.0\"?>\n<!-- ShmupX Client: Configurações › Controle -->\n<mameconfig version=\"10\">\n    <system name=\"default\">\n        <input>\n");
    for (n, device) in &aparelhos {
        saida.push_str(&format!("            <mapdevice device=\"{}\" controller=\"JOYCODE_{n}\" />\n", xml(device)));
    }
    for (id, porta, padrao) in CONTROLES {
        let sequencia: Vec<&str> = match validos.iter().rev().find(|m| m.control == id) {
            Some(m) => vec![m.code.as_str()],
            None => {
                let livre: Vec<&str> = padrao.iter().copied().filter(|c| !usados.contains(c)).collect();
                if livre.len() == padrao.len() {
                    continue; // o padrão não conflita: fica o do MAME
                }
                livre
            }
        };
        let sequencia = if sequencia.is_empty() { "NONE".to_string() } else { sequencia.join(" OR ") };
        saida.push_str(&format!(
            "            <port type=\"{porta}\">\n                <newseq type=\"standard\">{sequencia}</newseq>\n            </port>\n"
        ));
    }
    saida.push_str("        </input>\n    </system>\n</mameconfig>\n");
    Some(saida)
}

/// Grava (ou apaga) `ctrlr/shmupx.cfg` nos dados do motor e devolve os argumentos para usá-lo.
pub fn preparar_ctrlr(dados: &Path, mapeamento: &[ControleMapeado]) -> Result<Vec<String>, String> {
    let pasta = dados.join("ctrlr");
    let arquivo = pasta.join("shmupx.cfg");
    let Some(conteudo) = arquivo_ctrlr(mapeamento) else {
        let _ = std::fs::remove_file(&arquivo);
        return Ok(vec![]);
    };
    std::fs::create_dir_all(&pasta).map_err(|e| format!("não foi possível preparar os controles do ShmupX Engine: {e}"))?;
    std::fs::write(&arquivo, conteudo).map_err(|e| format!("não foi possível gravar os controles do ShmupX Engine: {e}"))?;
    Ok(vec!["-ctrlrpath".into(), pasta.to_string_lossy().into_owned(), "-ctrlr".into(), "shmupx".into()])
}

/// Motor aberto em modo captura (um por vez).
#[derive(Default)]
pub struct CapturaState(Mutex<Option<Child>>);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EventoCaptura {
    /// `ready` (pronto para ler), `input` (aperto), `release` (soltou) ou `ended` (o motor fechou).
    kind: String,
    code: Option<String>,
    label: Option<String>,
    device: Option<String>,
    device_name: Option<String>,
}

fn campo(valor: &serde_json::Value, nome: &str) -> Option<String> {
    valor.get(nome).and_then(|v| v.as_str()).map(str::to_string)
}

fn argumentos_da_captura(dados: &Path) -> Vec<String> {
    let cfg: PathBuf = dados.join("captura");
    [
        "___empty", "-video", "none", "-sound", "none", "-noswitchres",
        // Lê os controles com o Client em foco; o teclado pelo DirectInput, que também lê em segundo plano.
        "-background_input", "-keyboardprovider", "dinput",
        "-skip_gameinfo", "-plugin", "shmupx",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain(["-cfg_directory".to_string(), cfg.to_string_lossy().into_owned()])
    .collect()
}

/// Abre o motor em modo captura, se ainda não estiver aberto. Os apertos e as solturas chegam por `engine://capture`.
#[tauri::command]
pub fn motor_captura_iniciar(app: AppHandle, estado: State<'_, CapturaState>) -> Result<(), String> {
    let mut atual = estado.0.lock().map_err(|e| e.to_string())?;
    if let Some(filho) = atual.as_mut() {
        if matches!(filho.try_wait(), Ok(None)) {
            return Ok(());
        }
    }
    let Motor { exe, dados, .. } = super::emulador::motor(&app).ok_or("o ShmupX Emulator não foi encontrado neste computador")?;
    std::fs::create_dir_all(dados.join("captura")).map_err(|e| format!("não foi possível preparar o ShmupX Engine: {e}"))?;
    // No Linux, pelo Wine, como o jogo.
    let mut comando = super::emulador::comando_exe(&exe);
    comando.args(argumentos_da_captura(&dados)).env("SHMUPX_CAPTURA", "1").stdout(Stdio::piped()).stderr(Stdio::null());
    if let Some(pasta) = exe.parent() {
        comando.current_dir(pasta);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        comando.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: o motor fica escondido
    }
    let mut filho = comando.spawn().map_err(super::emulador::erro_ao_abrir)?;
    let saida = filho.stdout.take();
    *atual = Some(filho);
    drop(atual);
    let app = app.clone();
    std::thread::spawn(move || {
        if let Some(saida) = saida {
            for linha in BufReader::new(saida).lines().map_while(Result::ok) {
                let Some(json) = linha.strip_prefix("SHMUPX ") else { continue };
                let Ok(valor) = serde_json::from_str::<serde_json::Value>(json) else { continue };
                let kind = match valor.get("ev").and_then(|v| v.as_str()) {
                    Some("capture_ready") => "ready",
                    Some("input") => "input",
                    Some("release") => "release",
                    _ => continue,
                };
                let _ = app.emit(
                    "engine://capture",
                    EventoCaptura {
                        kind: kind.into(),
                        code: campo(&valor, "code"),
                        label: campo(&valor, "label"),
                        device: campo(&valor, "device"),
                        device_name: campo(&valor, "device_name"),
                    },
                );
            }
        }
        let _ = app.emit("engine://capture", EventoCaptura { kind: "ended".into(), code: None, label: None, device: None, device_name: None });
    });
    Ok(())
}

/// Fecha o motor do modo captura (a tela de controles fechou).
#[tauri::command]
pub fn motor_captura_parar(estado: State<'_, CapturaState>) {
    if let Ok(mut atual) = estado.0.lock() {
        if let Some(mut filho) = atual.take() {
            let _ = filho.kill();
            let _ = filho.wait();
        }
    }
}

/// Fecha a captura ao sair do app.
pub fn encerrar(app: &AppHandle) {
    if let Some(estado) = app.try_state::<CapturaState>() {
        motor_captura_parar(estado);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(control: &str, code: &str, device: Option<&str>) -> ControleMapeado {
        ControleMapeado { control: control.into(), code: code.into(), device: device.map(str::to_string) }
    }

    #[test]
    fn sem_mapeamento_valem_os_padroes() {
        assert_eq!(arquivo_ctrlr(&[]), None);
        assert_eq!(arquivo_ctrlr(&[m("button9", "KEYCODE_Z", None), m("button1", "KEYCODE_<x>", None)]), None);
    }

    #[test]
    fn botao_mapeado_e_padrao_sem_conflito() {
        let pad = "REAL ARCADE Pro.V3 product_00220f0d-0000-0000-0000-504944564944 instance_c0497310";
        let xml = arquivo_ctrlr(&[m("button1", "JOYCODE_2_BUTTON3", Some(pad)), m("coin", "KEYCODE_Z", None)]).unwrap();
        assert!(xml.contains(r#"<mapdevice device="product_00220f0d-0000-0000-0000-504944564944" controller="JOYCODE_2" />"#));
        assert!(xml.contains("<port type=\"P1_BUTTON1\">\n                <newseq type=\"standard\">JOYCODE_2_BUTTON3</newseq>"));
        assert!(xml.contains("<port type=\"COIN1\">\n                <newseq type=\"standard\">KEYCODE_Z</newseq>"));
        // O padrão do botão 3 não usa JOYCODE_2: fica o do MAME (nem aparece no arquivo).
        assert!(!xml.contains("P1_BUTTON3"));
    }

    #[test]
    fn codigo_tirado_do_padrao_de_outro_controle() {
        let xml = arquivo_ctrlr(&[m("button1", "JOYCODE_1_BUTTON3", Some("Pad product_x"))]).unwrap();
        // O botão 3 perde o JOYCODE_1_BUTTON3 (agora é o botão 1) e fica com o teclado.
        assert!(xml.contains("<port type=\"P1_BUTTON3\">\n                <newseq type=\"standard\">KEYCODE_SPACE</newseq>"));
        let xml = arquivo_ctrlr(&[m("start", "KEYCODE_5", None)]).unwrap();
        assert!(xml.contains("<port type=\"COIN1\">\n                <newseq type=\"standard\">JOYCODE_1_SELECT</newseq>"));
    }

    #[test]
    fn aparelho_sem_product_usa_o_id_inteiro_escapado() {
        assert_eq!(id_do_aparelho("XInput Player 1"), "XInput Player 1");
        let xml = arquivo_ctrlr(&[m("up", "JOYCODE_1_HAT1UP", Some("Pad \"A\" & B"))]).unwrap();
        assert!(xml.contains(r#"device="Pad &quot;A&quot; &amp; B""#));
    }
}
