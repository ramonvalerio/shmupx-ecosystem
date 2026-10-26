use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

use super::types::{AnalysisProfile, MappingBoxInput, NormalizedRegion};

/// Chaves de região reconhecidas pelo domínio de análise (Spec 001). `continues` ainda não é
/// mapeado pela UI do canvas (TASKS.md 2.3b), mas já é aceito aqui para não travar o export
/// quando essa região existir no frontend.
const VALID_REGION_KEYS: &[&str] = &["score", "lives", "continues", "gameOver"];

fn normalize_variant(variant: &str) -> Result<String, String> {
    match variant.to_ascii_lowercase().as_str() {
        "lcd" => Ok("lcd".to_string()),
        "crt" => Ok("crt".to_string()),
        other => Err(format!(
            "variante inválida: \"{other}\" (esperado \"LCD\" ou \"CRT\")"
        )),
    }
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

/// Divide x/largura por `img_width` e y/altura por `img_height`, convertendo o bounding box
/// em pixels absolutos para uma região normalizada 0..1 (TASKS.md 2.6).
fn normalize_box(
    box_input: &MappingBoxInput,
    img_width: u32,
    img_height: u32,
) -> Result<(String, NormalizedRegion), String> {
    if !VALID_REGION_KEYS.contains(&box_input.key.as_str()) {
        return Err(format!(
            "região desconhecida \"{}\" (esperado uma de {VALID_REGION_KEYS:?})",
            box_input.key
        ));
    }

    let width = img_width as f64;
    let height = img_height as f64;

    Ok((
        box_input.key.clone(),
        NormalizedRegion {
            x: clamp01(box_input.x / width),
            y: clamp01(box_input.y / height),
            width: clamp01(box_input.width / width),
            height: clamp01(box_input.height / height),
        },
    ))
}

/// Lê a versão de um perfil já exportado no mesmo caminho (se existir) e incrementa o PATCH;
/// caso contrário, começa em `v0.1.0`. Mantém o versionamento semântico da Spec 001 sem exigir
/// que o frontend acompanhe estado de versão (ainda não há persistência local, TASKS.md 0.3).
fn next_version(existing_path: &Path) -> String {
    let fallback = "v0.1.0".to_string();

    let Ok(contents) = fs::read_to_string(existing_path) else {
        return fallback;
    };
    let Ok(existing) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return fallback;
    };
    let Some(version) = existing.get("version").and_then(|v| v.as_str()) else {
        return fallback;
    };
    let Some(stripped) = version.strip_prefix('v') else {
        return fallback;
    };

    let parts: Vec<&str> = stripped.split('.').collect();
    if parts.len() != 3 {
        return fallback;
    }

    let (Ok(major), Ok(minor), Ok(patch)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) else {
        return fallback;
    };

    format!("v{major}.{minor}.{}", patch + 1)
}

/// Exporta o mapeamento atual do Lab como `profile.json`: normaliza as bounding boxes em
/// pixels absolutos para coordenadas 0..1 e grava em
/// `app_data_dir()/exports/profile_<game_id>_<variant>.json`. Retorna o caminho absoluto salvo.
#[tauri::command]
pub async fn exportar_perfil_jogo(
    app: AppHandle,
    game_id: String,
    variant: String,
    boxes: Vec<MappingBoxInput>,
    img_width: u32,
    img_height: u32,
) -> Result<String, String> {
    if game_id.trim().is_empty() {
        return Err("game_id não pode ser vazio".into());
    }
    if img_width == 0 || img_height == 0 {
        return Err(
            "dimensões da imagem de referência inválidas (img_width/img_height devem ser > 0)"
                .into(),
        );
    }
    if boxes.is_empty() {
        return Err("nenhuma região mapeada para exportar".into());
    }

    let variant = normalize_variant(&variant)?;

    let mut regions = BTreeMap::new();
    for box_input in &boxes {
        let (key, region) = normalize_box(box_input, img_width, img_height)?;
        regions.insert(key, region);
    }

    let exports_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))?
        .join("exports");
    fs::create_dir_all(&exports_dir).map_err(|e| {
        format!(
            "falha ao criar a pasta de exportações ({}): {e}",
            exports_dir.display()
        )
    })?;

    let output_path = exports_dir.join(format!("profile_{game_id}_{variant}.json"));
    let version = next_version(&output_path);
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let profile = AnalysisProfile {
        game_id,
        version,
        variant,
        regions,
        created_at,
    };

    let json = serde_json::to_string_pretty(&profile)
        .map_err(|e| format!("falha ao serializar o perfil: {e}"))?;

    fs::write(&output_path, json)
        .map_err(|e| format!("falha ao salvar o perfil em {}: {e}", output_path.display()))?;

    Ok(output_path.display().to_string())
}
