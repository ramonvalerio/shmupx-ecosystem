# ADR 010 — Ranking local no Client, ranking online só com recordes

- **Status:** Aceito (2026-11-26)
- **Data:** 2026-11-26
- **Relaciona-se com:** a [ADR 005](005-rankings-no-firebase.md) (rankings no Firebase) e a [spec 002](../specs/002-economia-fichas-e-ranking.md) (fichas e ranking).

## Contexto

Até aqui o banco guardava uma gameplay por run ranked, com o placar, e o ranking de cada jogo era montado lendo todas elas e pegando o maior placar de cada jogador. Com centenas ou milhares de jogadores, isso enche o banco de runs abaixo do recorde de cada um e faz cada leitura do ranking crescer com o número de partidas.

## Decisão

1. **Toda run válida fica no ranking local**, no Client do jogador (`ranking-local.json` nos dados do app, por conta): placar, vidas, continues, se zerou, modo, data e se ficou online. A aba Local do ranking da sala mostra essa tabela, do maior placar para o menor.
2. **O ranking online guarda só o recorde.** Depois do envio de uma run, o backend compara as gameplays com resultado do jogador naquele jogo e deixa só a de maior placar de cada tabela (modo × versão do jogo × evento; uma run de Hi-Score zerada com 1 crédito vale também na tabela 1CC). As outras saem do banco (`services/recordes.rs`). Empate: fica a mais antiga.
3. **A comparação é sempre da run atual com o banco.** O ranking local nunca é enviado nem entra na comparação: mexer nele ou apagá-lo não muda o ranking online, e perdê-lo não afeta nada que vale.
4. **A resposta do envio diz se a run ficou online** (`online`); o Client avisa "novo recorde" ou "salva no ranking local".
5. **Totais do jogador por jogo num documento só** (`playerStats/{uid}_{gameId}`): fichas gastas, tempo de jogo e a última run, mantidos pelo backend no começo e no fim de cada run. A sala lê esse documento (sem ele, ainda conta as gameplays antigas). As Security Rules só liberam a leitura para o dono.

## Consequências

- (+) O banco fica com uma gameplay por jogador e tabela, e as leituras do ranking não crescem com as partidas jogadas.
- (+) O histórico completo de cada jogador continua disponível para ele, no PC.
- (−) O histórico de runs abaixo do recorde não fica no servidor: em outro PC, o ranking local começa vazio.
- (−) O envio de uma run abaixo do recorde não é mais idempotente: reenviada depois de apagada, responde "run não encontrada" (nada dela precisava ficar online).
- (−) O lançamento da ficha na carteira (`wallets/{uid}/entries`) continua um por run: é o registro da economia, não do ranking.
