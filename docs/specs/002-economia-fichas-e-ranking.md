# Spec 002 — Economia de Fichas, Antifraude e Ranking

- **Status:** Rascunho
- **Relacionado:** ADR 001, ADR 002

## 1. Fichas digitais

- Jogar valendo para o ranking consome **1 ficha**.
- O consumo ocorre no backend, em transação atômica, ao iniciar a run: o início valida saldo, debita e devolve `run_id`, `server_started_at` e `max_duration`.
- Run abandonada ou inválida não devolve a ficha, salvo falha comprovada de infraestrutura.

## 2. Timer seguro e cortes do FFmpeg

- A duração máxima da run é de **45 a 60 min** (definida por jogo no servidor).
- O timer vive em **Rust** (relógio monotônico, `Instant`), não em JS. O servidor guarda `server_started_at` como referência soberana.
- Ao estourar o limite: o Rust **encerra o FFmpeg imediatamente**, marca a run como `expired` e só o trecho gravado até então é elegível.
- Verificações antifraude:
  - Metadados do vídeo: duração gravada ≤ limite + tolerância; timestamps coerentes com `server_started_at`.
  - Se o FFmpeg morrer fora do esperado, a run é `invalid`.
  - A submissão exige `run_id` ativo, uma única vez, dentro da janela de tempo + tolerância de envio.
  - O resultado inclui `profileVersion`, `variant` e hash do vídeo.
- O streaming RTMP opcional (YouTube Live) compartilha o mesmo processo e ciclo de vida do FFmpeg.

## 3. Normalização do ranking

Cada shmup pontua em escala diferente. Dentro de uma **temporada** e de um **jogo** `g`:

- Usa-se o **melhor score válido** `s` de cada jogador.
- **Score Z:** `z = (s − μ_g) / σ_g`, com `μ_g` e `σ_g` calculados sobre os melhores scores válidos de `g` na temporada. Como scores de shmups têm cauda longa, aplica-se `ln(s)` antes do cálculo quando a assimetria excede um limiar (flag por jogo).
- **Percentil:** `p = rank_g(s) / N_g`, alternativa robusta quando `N_g` é pequeno ou σ ≈ 0.
- Mínimo de `N_min` jogadores para usar Z; abaixo disso usa-se percentil.
- **Índice global do jogador:** média (ou soma limitada aos top-K jogos) dos índices por jogo, com *shrinkage* para quem tem poucos jogos.
- O cálculo roda em job periódico no backend e grava a tabela materializada `season_rankings`, consumida pelo portal web.

## 4. Pontos em aberto

- Valores de `N_min` e K; janela exata (45 vs 60 min) por jogo.
- Política de revisão manual para runs `needs_review`.
