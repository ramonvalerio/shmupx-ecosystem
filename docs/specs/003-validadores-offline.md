# Contrato de execução dos validadores offline

## Entrada e descoberta

Executar `python analyze.py VIDEO --game ID --player NOME`. `python -m shmupx` oferece a mesma interface a partir de `shmupx_offline/`. Caminhos de recursos são relativos ao módulo; vídeo, saída e depuração são relativos ao diretório do chamador quando não absolutos.

`--list-games` retorna uma lista JSON com `id`, `name` e `version`, sem carregar OpenCV. IDs não cadastrados falham antes da análise. O registro é código local revisado; não aceita caminhos ou módulos fornecidos pelo usuário.

## Saída compatível

Em sucesso, stdout contém somente o objeto JSON histórico `{ "<game-id>": { ... } }`. Progresso e diagnósticos vão para stderr. `--out` grava o mesmo objeto em arquivo. Código zero indica execução concluída, não aprovação competitiva. Erros encerram com código diferente de zero; um arquivo de resultado antigo não deve ser reutilizado pelo chamador após falha.

Campos preservados: `player`, `url`, `game-mode`, `duration` (HH:MM:SS), `fps-analysed` (quantidade de quadros lidos, não taxa), `hi-score` (inteiro ou null), `continues`, `lives-lost`, `game-over`, `attempts`, `observation`, `total_tokens` (null no modo offline).

O formato atual informa ressalvas em `observation`; a confiança também é registrada no diagnóstico. Não inferir aprovação de `1cc` nem converter `hi-score: null` em zero. `--format client-v1` disponibiliza um envelope com `schema_version: 1`, `game_id`, `validator_version`, `confidence`, `needs_review` e `gameplay` (os mesmos campos do objeto legado). Baixa confiança, placar ausente, continues ou game over exigem revisão; os dois últimos ainda dependem de heurísticas sem corpus negativo validado. O formato legado continua padrão.

## Interface de um módulo de jogo

O módulo fornece `create_analyzer(root, video_path, *, debug_dir=None, quiet=False)`. Retorna um analisador com `run()`, `v` (vídeo, duração, contagem e captura liberável), e o resultado interno com `hi`, `confidence`, `continues`, `lives_lost`, `game_over`, `attempts`, `observation`, `deaths`.

A auditoria externa legada utiliza adicionalmente `final_crop_time`, `hud.score_crop()`, `v.frame_at()` e `results`. Validadores futuros precisam implementar esse contrato antes de oferecer `--grok`, ou rejeitar essa opção explicitamente. Não assumir que todo jogo usa o mesmo HUD.

## Verificação

Executar `python -m unittest discover -s tests -v`. Os testes cobrem seleção segura, recursos do Earthion, compatibilidade de imports, isolamento dos logs e liberação do vídeo em sucesso/falha. A precisão de placares, continues e mortes precisa de vídeos de referência reais por jogo, incluindo casos negativos; testes com analisador simulado não comprovam precisão visual.
