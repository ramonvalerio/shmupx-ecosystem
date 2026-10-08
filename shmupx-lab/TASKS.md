# ShmupX Lab — Tarefas de implementação

Ordem sequencial. Contexto: `docs/adr/002` e `docs/specs/001`.

## 0. Fundação
- [ ] 0.1 Escolher store de estado (ex.: Zustand) e roteamento; montar o layout em `src/components/layout`.
- [ ] 0.2 Camada `src/lib/tauri` com wrappers tipados de `invoke`.
- [ ] 0.3 Banco local (SQLite via `rusqlite`/plugin SQL) para jogos, datasets, perfis e execuções de treino.

## 1. Gerenciador de datasets (`features/datasets`)
- [ ] 1.1 Cadastro de jogo (nome, `steam_app_id`, notas).
- [ ] 1.2 Importar pastas de vídeos/screenshots por referência (sem copiar gigabytes), com indexação no SQLite.
- [ ] 1.3 Geração de miniaturas e extração de frames de referência via FFmpeg (comando Rust, em background).
- [ ] 1.4 Grid virtualizado de mídia, filtros por jogo/tipo/tela (LCD/CRT) e marcação de frames-chave.
- [ ] 1.5 Rotular frames (placar, vidas, game over) para alimentar o treino.

## 2. Canvas de mapeamento (`features/mapping`, `components/canvas`)
- [ ] 2.1 Canvas com zoom/pan sobre o frame de referência.
- [ ] 2.2 Desenho/edição de retângulos (mover, redimensionar) com coordenadas normalizadas 0..1 (`types/profile.ts`).
- [ ] 2.3 Tipos de região: `score`, `lives`, `continues`, `gameOver`.
- [ ] 2.4 Pré-visualização do recorte e overlay sobre vários frames para checar consistência.
- [ ] 2.5 Undo/redo e atalhos de teclado.

## 3. Treinamento local (`features/training`)
- [ ] 3.1 Comando Rust para gerar dataset de recortes a partir das regiões + rótulos.
- [ ] 3.2 Definir runtime de treino (processo Python sidecar ou crate Rust) e contrato de execução/progresso.
- [ ] 3.3 Treinar modelo leve (OCR de dígitos / classificador de HUD) e exportar ONNX.
- [ ] 3.4 Avaliação contra conjunto de verificação (acurácia, matriz de confusão) exibida na UI.
- [ ] 3.5 Teste de inferência com `ort` no próprio Lab usando o mesmo código do Client.

## 4. Perfis e variantes CRT/LCD (`features/profiles`)
- [ ] 4.1 Modelo de perfil versionado (semver) com `sha256` e persistência local.
- [ ] 4.2 Criar variante `lcd`/`crt` a partir de um perfil base, com ajuste independente de regiões e pesos.
- [ ] 4.3 Comparação lado a lado de resultados LCD vs CRT no mesmo dataset.
- [ ] 4.4 Diff entre versões de perfil (regiões e métricas).

## 5. Publicação (Supabase)
- [ ] 5.1 Autenticação de admin e configuração do projeto Supabase.
- [ ] 5.2 Empacotar `profile.json` + `model.onnx`, calcular SHA-256 e enviar ao Storage (`profiles/{gameId}/{version}/{variant}/`).
- [ ] 5.3 Registrar release em `profile_releases` (canal `draft` → `stable`); releases imutáveis.
- [ ] 5.4 Fluxo de promoção/rollback de release.
