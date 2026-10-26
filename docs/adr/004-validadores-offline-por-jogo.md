# ADR 004 — Um validador offline por jogo

- **Status:** Aceito; adaptador do cliente implementado, fluxo real ainda não homologado.
- **Data:** 2026-10-26
- **Substitui:** a escolha de motor único ONNX da ADR 002. As decisões de captura e distribuição não são implementadas por esta mudança.

## Decisão

Cada jogo tem um módulo Python independente em `shmupx_offline/shmupx/validators/`, registrado explicitamente em `registry.py`. O Earthion passa a possuir o algoritmo antes localizado em `pipeline.py`, sem alteração de suas regras de leitura e contagem. O import antigo é mantido como compatibilidade.

`games/<id>.json` guarda calibração e parâmetros; `templates/<id>/` guarda imagens de referência. Leitura de vídeo e utilitários de HUD continuam compartilhados. Um novo jogo pode usar uma estratégia diferente; copiar apenas o JSON não o torna suportado.

O comando comum escolhe o script, recebe vídeo local e escreve o resultado. O padrão funciona offline. A conferência externa existente com Grok fica opcional e depende de autorização explícita por `--grok`; sua dependência está separada.

## Limites entre projetos

- **Client:** captura, gestão da sessão e execução assíncrona do validador, fora da interface React.
- **Offline:** interpretação do vídeo, regras por jogo e evidências de leitura.
- **Lab:** preparação e revisão de recursos; não substitui os testes do script.
- **Backend:** autenticação, regras competitivas e aceite da submissão. Não deve tratar JSON local como prova antifraude.

## Consequências e próximos passos

O algoritmo de cada jogo evolui separadamente, com versão no registro. Os testes e vídeos de referência devem comprovar cada mudança de regras. Python/OpenCV precisarão de empacotamento para distribuição desktop.

A integração Rust usa o adaptador Python, com argumentos separados (sem shell), timeout de 15 minutos, saída limitada e validação do envelope `client-v1`. Falhas não geram placares substitutos. Baixa confiança ou métricas heurísticas exigem revisão e não são enviadas automaticamente. O código ONNX anterior permanece isolado para referência.

Ainda faltam catálogo real UUID → script, homologação com vídeos reais, fluxo de revisão/reenvio e empacotamento do runtime Python. Downloads e execução de scripts arbitrários não fazem parte desta decisão. Consulte [o guia operacional](../specs/004-integracao-offline-client.md).
