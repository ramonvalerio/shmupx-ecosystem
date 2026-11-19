use std::path::Path;

use image::imageops::FilterType;
use image::{GenericImageView, GrayImage};

use super::profile::Region;

/// Dimensões de entrada esperadas pelo modelo (ver analysis/model.rs).
pub const MODEL_INPUT_WIDTH: u32 = 128;
pub const MODEL_INPUT_HEIGHT: u32 = 32;

/// Recorta `region` (coordenadas normalizadas 0..1, relativas ao frame de referência usado no
/// Lab) de `frame_path`, converte para escala de cinza e redimensiona para o tamanho fixo que o
/// modelo espera. `region` é confiada ciegamente quanto à proporção, mas as coordenadas
/// absolutas resultantes são sempre limitadas (`clamp`) aos limites reais da imagem — um
/// perfil mapeado para uma resolução diferente da gravação real não deve conseguir estourar o
/// recorte para fora do frame.
pub fn recortar_regiao(frame_path: &Path, region: &Region) -> Result<GrayImage, String> {
    let imagem = image::open(frame_path).map_err(|e| {
        format!(
            "falha ao abrir o frame {} para recorte: {e}",
            frame_path.display()
        )
    })?;

    let (largura, altura) = imagem.dimensions();
    if largura == 0 || altura == 0 {
        return Err(format!(
            "frame {} tem dimensões inválidas",
            frame_path.display()
        ));
    }

    let largura_f = largura as f64;
    let altura_f = altura as f64;

    let x = (region.x * largura_f).round().clamp(0.0, largura_f - 1.0) as u32;
    let y = (region.y * altura_f).round().clamp(0.0, altura_f - 1.0) as u32;
    let w = (region.width * largura_f)
        .round()
        .clamp(1.0, (largura - x) as f64) as u32;
    let h = (region.height * altura_f)
        .round()
        .clamp(1.0, (altura - y) as f64) as u32;

    let recorte = imagem.crop_imm(x, y, w, h).to_luma8();
    let redimensionado = image::imageops::resize(
        &recorte,
        MODEL_INPUT_WIDTH,
        MODEL_INPUT_HEIGHT,
        FilterType::Triangle,
    );

    Ok(redimensionado)
}
