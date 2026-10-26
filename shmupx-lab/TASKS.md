# ShmupX Lab — Tarefas de implementação

Ordem sequencial. Contexto: `docs/adr/002` e `docs/specs/001`.

## 0. Fundação
- [x] 0.1a Layout inicial do backoffice em `src/components/layout` (`GameSidebar.tsx`): menu lateral com jogos cadastrados (mockados em `src/data/mockGames.ts`) e painel central (`App.tsx`). Sem store/roteamento dedicados ainda — estado local em `useState`.
- [ ] 0.1b Escolher store de estado (ex.: Zustand) e roteamento quando o app crescer além de uma tela.
- [ ] 0.2 Camada `src/lib/tauri` com wrappers tipados de `invoke`.
- [ ] 0.3 Banco local (SQLite via `rusqlite`/plugin SQL) para jogos, datasets, perfis e execuções de treino (hoje `MOCK_GAMES` é estático).

## 1. Gerenciador de datasets (`features/datasets`)
- [ ] 1.1 Cadastro de jogo (nome, `steam_app_id`, notas).
- [ ] 1.2 Importar pastas de vídeos/screenshots por referência (sem copiar gigabytes), com indexação no SQLite.
- [ ] 1.3 Geração de miniaturas e extração de frames de referência via FFmpeg (comando Rust, em background).
- [ ] 1.4 Grid virtualizado de mídia, filtros por jogo/tipo/tela (LCD/CRT) e marcação de frames-chave.
- [ ] 1.5 Rotular frames (placar, vidas, game over) para alimentar o treino.

## 2. Canvas de mapeamento (`features/mapping`, `components/canvas`)
- [x] 2.1a Upload de screenshot de referência (`MappingCanvas.tsx`) com leitura das dimensões naturais (`naturalWidth`/`naturalHeight`) e exibição escalada (sem distorcer as coordenadas reais).
- [ ] 2.1b Zoom/pan sobre o frame de referência (hoje só escala para caber em `MAX_DISPLAY_WIDTH`).
- [x] 2.2a Desenho/edição de retângulos: mover (arrastar) e redimensionar (alça no canto) via mouse, implementado sem dependência externa (`window.mousemove`/`mouseup`), com coordenadas armazenadas em **pixels exatos** da imagem (`MappingBox` em `types/mapping.ts`) — a conversão para 0..1 (`Region`) fica para a exportação do perfil.
- [ ] 2.2b Persistir os retângulos por jogo/variante (hoje só vive em memória via `App.tsx`, perdido ao recarregar).
- [x] 2.3a Três regiões obrigatórias mapeadas por padrão ao criar um mapeamento: `score` ("High Score"), `lives` ("Vidas Restantes"), `gameOver` ("Tela de Game Over") — ver `createDefaultMappingBoxes()`.
- [ ] 2.3b Tipo de região `continues` (hoje só as três obrigatórias do Spec 001 estão cobertas).
- [ ] 2.4 Pré-visualização do recorte e overlay sobre vários frames para checar consistência.
- [ ] 2.5 Undo/redo e atalhos de teclado.
- [x] 2.6 Comando Tauri `exportar_perfil_jogo` (`src-tauri/src/profiles/commands.rs`): recebe `game_id`, `variant` ("LCD"/"CRT"), as `MappingBox` em pixels e `img_width`/`img_height`; normaliza cada região dividindo x/largura por `img_width` e y/altura por `img_height` (clamp 0..1), monta o `AnalysisProfile` (versão com auto-incremento de PATCH a partir do arquivo existente, ou `v0.1.0` na primeira exportação) e salva em `app_data_dir()/exports/profile_<game_id>_<variant>.json`. Botão "Exportar Perfil" em `GameWorkspace.tsx` chama o comando via `invoke` e mostra um banner de sucesso/erro inline.
  - **Pendências:** `sha256`/`modelRef` ficam de fora (só entram na publicação real, TASKS 5.2); não há persistência das `MappingBox` entre sessões (perdidas ao recarregar, ver 2.2b); `continues` ainda não é desenhável no canvas (2.3b), só aceito pelo Rust se vier do frontend no futuro.

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
