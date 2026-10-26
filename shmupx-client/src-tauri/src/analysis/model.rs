use std::sync::{Mutex, OnceLock};

use ort::session::Session;

/// Modelo embutido no binário em tempo de compilação. **Estado honesto deste modelo**: a
/// arquitetura é real (crop normalizado → escala de cinza 32×128 → conv → softmax por posição
/// de dígito), mas os pesos são aleatórios — não existe ainda nenhum modelo treinado publicado
/// pelo Lab (o treinamento em si é um item pendente, ver shmupx-lab/TASKS.md seção 3). Os
/// "dígitos reconhecidos" por este modelo não têm significado nenhum; o que está testado e
/// funcionando de verdade é o pipeline mecânico (extração de frame → recorte → tensor →
/// inferência → decodificação), não a inteligência dentro do modelo.
///
/// `include_bytes!` evita resolver caminho em runtime; quando a sincronização de perfis/modelos
/// existir (Spec 001, passo 6), isto deve carregar do caminho sincronizado em `app_data_dir()`
/// em vez de ficar embutido no binário.
const MODELO_PLACEHOLDER_BYTES: &[u8] =
    include_bytes!("../../models/digit_strip_placeholder.onnx");

const POSICOES: usize = 8;
const CLASSES: usize = 10;

// `Session::run` exige `&mut self` (uso interno de buffers/IoBinding) — um `Mutex` em vez de
// uma referência compartilhada direta, já que a sessão é um singleton estático reusado por
// todas as inferências do processo.
static SESSAO: OnceLock<Result<Mutex<Session>, String>> = OnceLock::new();

fn sessao() -> Result<&'static Mutex<Session>, String> {
    let resultado = SESSAO.get_or_init(|| {
        let sessao = Session::builder()
            .map_err(|e| format!("falha ao preparar o ambiente ONNX Runtime: {e}"))?
            .commit_from_memory(MODELO_PLACEHOLDER_BYTES)
            .map_err(|e| format!("falha ao carregar o modelo ONNX embutido: {e}"))?;
        Ok(Mutex::new(sessao))
    });

    resultado.as_ref().map_err(|e| e.clone())
}

/// Roda a imagem (já recortada/redimensionada para 32×128 em escala de cinza, pixels
/// normalizados 0.0–1.0) pelo modelo. Devolve os logits brutos: `POSICOES` posições × `CLASSES`
/// dígitos (0–9) cada, na ordem posição-maior-primeiro (`logits[posicao * CLASSES + classe]`).
pub fn inferir_digitos(pixels_normalizados: &[f32]) -> Result<Vec<f32>, String> {
    let esperado = crate::analysis::crop::MODEL_INPUT_WIDTH as usize
        * crate::analysis::crop::MODEL_INPUT_HEIGHT as usize;
    if pixels_normalizados.len() != esperado {
        return Err(format!(
            "tensor de entrada com tamanho errado: esperado {esperado} pixels, recebido {}",
            pixels_normalizados.len()
        ));
    }

    let mut sessao = sessao()?
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let shape = [1_i64, 1, crop_height(), crop_width()];
    let tensor = ort::value::Tensor::from_array((shape, pixels_normalizados.to_vec()))
        .map_err(|e| format!("falha ao montar o tensor de entrada: {e}"))?;

    let outputs = sessao
        .run(ort::inputs![tensor])
        .map_err(|e| format!("falha na inferência ONNX: {e}"))?;

    let (_shape, dados) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("formato de saída do modelo inesperado: {e}"))?;

    if dados.len() != POSICOES * CLASSES {
        return Err(format!(
            "saída do modelo com tamanho inesperado: esperado {}, recebido {}",
            POSICOES * CLASSES,
            dados.len()
        ));
    }

    Ok(dados.to_vec())
}

fn crop_height() -> i64 {
    crate::analysis::crop::MODEL_INPUT_HEIGHT as i64
}

fn crop_width() -> i64 {
    crate::analysis::crop::MODEL_INPUT_WIDTH as i64
}
