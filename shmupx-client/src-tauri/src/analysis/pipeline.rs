use std::path::Path;

use tauri::AppHandle;

use super::{crop, frames, model, profile};
use profile::Region;

const CLASSES: usize = 10;
const POSICOES: usize = 8;

/// Resultado de uma análise — nomes de campo já no formato que
/// `net::backend::submeter_run` espera, para injetar direto na submissão.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub score_bruto: i64,
    pub vidas_perdidas: i32,
    pub continues_usados: i32,
    /// Confiança média (0.0–1.0) do frame escolhido como melhor leitura. Ver nota de
    /// confiabilidade em `analysis/model.rs` — com o modelo placeholder, isto mede o quão
    /// "confiante" o modelo (não treinado) ficou da própria resposta aleatória, não a
    /// confiabilidade real da leitura.
    pub confidence: f64,
}

/// Ponto de entrada do pipeline: extrai frames do final de `video_path`, recorta as regiões do
/// perfil de `jogo_id`, roda a inferência e devolve a melhor leitura entre os frames
/// amostrados (maior confiança média). Qualquer etapa pode falhar (perfil ausente/corrompido,
/// vídeo corrompido, inferência com erro) — tudo vira `Err` com uma mensagem explicativa, nunca
/// panic; quem chama decide como reagir (ver `capture::commands`, que cai para valores mock em
/// caso de falha, para a submissão da run não travar nisso).
pub async fn analisar_run(
    app: &AppHandle,
    video_path: &Path,
    jogo_id: &str,
) -> Result<AnalysisResult, String> {
    let caminho_perfil = profile::caminho_local(app, jogo_id)?;
    let perfil = profile::carregar(&caminho_perfil)?;

    let regiao_score = perfil
        .regions
        .get("score")
        .copied()
        .ok_or_else(|| "perfil de análise não tem a região \"score\" mapeada".to_string())?;
    let regiao_vidas = perfil.regions.get("lives").copied();
    let regiao_continues = perfil.regions.get("continues").copied();

    let dir_frames = video_path.with_extension("frames");
    let frames_extraidos = frames::extrair_frames_finais(video_path, &dir_frames).await?;

    let mut melhor: Option<AnalysisResult> = None;
    let mut melhor_confianca = -1.0_f64;

    for frame_path in &frames_extraidos {
        let candidato = analisar_frame(
            frame_path,
            &regiao_score,
            regiao_vidas.as_ref(),
            regiao_continues.as_ref(),
        )?;

        if candidato.confidence > melhor_confianca {
            melhor_confianca = candidato.confidence;
            melhor = Some(candidato);
        }
    }

    // Cache temporário desta análise — não precisa sobreviver além dela.
    let _ = tokio::fs::remove_dir_all(&dir_frames).await;

    melhor.ok_or_else(|| "nenhum frame pôde ser analisado".to_string())
}

fn analisar_frame(
    frame_path: &Path,
    regiao_score: &Region,
    regiao_vidas: Option<&Region>,
    regiao_continues: Option<&Region>,
) -> Result<AnalysisResult, String> {
    let (score, confianca_score) = ler_sequencia_digitos(frame_path, regiao_score)?;

    let (vidas, confianca_vidas) = match regiao_vidas {
        Some(regiao) => ler_primeiro_digito(frame_path, regiao)?,
        // Sem a região "vidas" mapeada no perfil: 0 é um valor padrão, não uma medição —
        // por isso a confiança associada é 0.0 em vez de inflar a média do frame.
        None => (0, 0.0),
    };

    let (continues, confianca_continues) = match regiao_continues {
        Some(regiao) => ler_primeiro_digito(frame_path, regiao)?,
        None => (0, 0.0),
    };

    let regioes_medidas = 1 + regiao_vidas.is_some() as usize + regiao_continues.is_some() as usize;
    let confianca_total = confianca_score + confianca_vidas + confianca_continues;
    let confianca_media = confianca_total / regioes_medidas as f64;

    Ok(AnalysisResult {
        score_bruto: score as i64,
        vidas_perdidas: vidas as i32,
        continues_usados: continues as i32,
        confidence: confianca_media,
    })
}

/// Lê as `POSICOES` posições do recorte como um número decimal (zeros à esquerda ignorados).
fn ler_sequencia_digitos(frame_path: &Path, regiao: &Region) -> Result<(u64, f64), String> {
    let logits = inferir_regiao(frame_path, regiao)?;

    let mut digitos = String::with_capacity(POSICOES);
    let mut confiancas = Vec::with_capacity(POSICOES);

    for posicao in 0..POSICOES {
        let inicio = posicao * CLASSES;
        let (classe, probabilidade) = softmax_argmax(&logits[inicio..inicio + CLASSES]);
        digitos.push(char::from_digit(classe as u32, 10).unwrap_or('0'));
        confiancas.push(probabilidade);
    }

    let sem_zeros_a_esquerda = digitos.trim_start_matches('0');
    let valor = if sem_zeros_a_esquerda.is_empty() {
        0
    } else {
        sem_zeros_a_esquerda.parse::<u64>().unwrap_or(0)
    };

    let confianca_media = confiancas.iter().sum::<f64>() / confiancas.len() as f64;
    Ok((valor, confianca_media))
}

/// Lê só a primeira posição do recorte como um único dígito (0–9) — usado para contadores de
/// um dígito como vidas/continues, onde não faz sentido interpretar as outras 7 posições.
fn ler_primeiro_digito(frame_path: &Path, regiao: &Region) -> Result<(u8, f64), String> {
    let logits = inferir_regiao(frame_path, regiao)?;
    let (classe, probabilidade) = softmax_argmax(&logits[0..CLASSES]);
    Ok((classe as u8, probabilidade))
}

fn inferir_regiao(frame_path: &Path, regiao: &Region) -> Result<Vec<f32>, String> {
    let imagem = crop::recortar_regiao(frame_path, regiao)?;
    let entrada: Vec<f32> = imagem
        .pixels()
        .map(|pixel| pixel.0[0] as f32 / 255.0)
        .collect();
    model::inferir_digitos(&entrada)
}

fn softmax_argmax(logits: &[f32]) -> (usize, f64) {
    let maximo = logits.iter().cloned().fold(f32::MIN, f32::max);
    let exponenciais: Vec<f32> = logits.iter().map(|&x| (x - maximo).exp()).collect();
    let soma: f32 = exponenciais.iter().sum();

    let mut melhor_indice = 0;
    let mut melhor_prob = 0.0_f32;
    for (indice, &exp) in exponenciais.iter().enumerate() {
        let prob = exp / soma;
        if prob > melhor_prob {
            melhor_prob = prob;
            melhor_indice = indice;
        }
    }

    (melhor_indice, melhor_prob as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn softmax_argmax_identifica_a_classe_de_maior_logit() {
        let logits = [0.1, 0.2, 5.0, 0.3, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0];
        let (classe, probabilidade) = softmax_argmax(&logits);
        assert_eq!(classe, 2);
        assert!(
            probabilidade > 0.9,
            "classe dominante deveria ter alta confiança, teve {probabilidade}"
        );
    }

    #[test]
    fn softmax_argmax_com_logits_iguais_divide_a_confianca() {
        let logits = [1.0; CLASSES];
        let (_classe, probabilidade) = softmax_argmax(&logits);
        // todas as classes empatadas: cada uma deveria ficar com ~1/CLASSES de probabilidade.
        assert!((probabilidade - 1.0 / CLASSES as f64).abs() < 1e-6);
    }

    #[test]
    fn softmax_argmax_e_estavel_com_logits_grandes_sem_overflow() {
        // Sem a subtração do máximo antes do exp(), isto geraria NaN/inf.
        let logits = [1000.0, 1001.0, 999.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let (classe, probabilidade) = softmax_argmax(&logits);
        assert_eq!(classe, 1);
        assert!(probabilidade.is_finite());
    }
}
