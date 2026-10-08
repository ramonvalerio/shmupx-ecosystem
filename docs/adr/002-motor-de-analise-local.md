# ADR 002 — Motor de análise local (ONNX), perfis versionados e variantes CRT/LCD

- **Status:** Aceito
- **Data:** 2026-10-08

## Contexto

Ao fim de uma run o Client precisa extrair high score, continues, game over e vidas perdidas do vídeo gravado. A análise não pode travar o PC do jogador nem depender de servidor (custo e latência de upload de vídeos grandes).

## Decisão

1. **Inferência local com ONNX Runtime**, carregada diretamente no Rust do Client (crate `ort`), com modelos leves (OCR de dígitos / classificadores de HUD). A análise roda em thread de baixa prioridade e processa frames amostrados (não o vídeo inteiro), guiados pelo perfil.
2. **Perfil de análise versionado** = JSON de coordenadas (regiões normalizadas 0..1) + referência a pesos ONNX. Versionamento semântico por jogo (`gameId@MAJOR.MINOR.PATCH`).
3. **Variantes por tipo de tela:** cada jogo pode ter `lcd` e `crt`. CRT e shaders de emulador distorcem geometria, brilho e scanlines; separar variantes evita um modelo único e frágil.
4. **Seleção dinâmica no Client:** a variante é escolhida pela configuração/detecção da tela do jogador, com override manual, e pode ser trocada em runtime.
5. **Distribuição:** o Lab publica no Supabase Storage; o Client sincroniza em segundo plano, valida o hash e só ativa o perfil após verificação.

## Alternativas consideradas

- Análise no servidor: custo de egress e privacidade do vídeo. Rejeitada.
- Python/OpenCV embarcado: peso de distribuição. Rejeitada.

## Consequências

- (+) Zero upload de vídeo; análise rápida.
- (+) Atualizações de perfil sem nova versão do app.
- (−) Dois conjuntos de perfis por jogo para manter e testar.
- (−) Necessário validar a integridade dos modelos baixados (ver spec 001).
