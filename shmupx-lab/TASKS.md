# ShmupX Lab — Tarefas de implementação

Ordem sequencial. Contexto: `docs/adr/002` e `docs/specs/001`.

## 0. Fundação
- [x] 0.1a Layout inicial do backoffice em `src/components/layout` (`GameSidebar.tsx`): menu lateral com jogos cadastrados (mockados em `src/data/mockGames.ts`) e painel central (`App.tsx`). Sem store/roteamento dedicados ainda — estado local em `useState`.
- [ ] 0.1b Escolher store de estado (ex.: Zustand) e roteamento quando o app crescer além de uma tela.
- [ ] 0.2 Camada `src/lib/tauri` com wrappers tipados de `invoke`.
- [ ] 0.3 Banco local (SQLite via `rusqlite`/plugin SQL) para jogos, datasets, perfis e execuções de treino (hoje `MOCK_GAMES` é estático).

## 1. Gerenciador de datasets (`features/datasets`)
- [x] 1.1 Cadastro de jogo: tela **Catálogo** (`features/catalog`) com título, ID curto, `steam_app_id`, duração máxima da run e publicar/despublicar, gravando em `games/{gameId}` pelo backend (`/api/admin/games`). Login Google igual ao do Client; admin definido no servidor por `ADMIN_EMAILS`. Pendente: notas.
  - **Preenchimento por IA** (migrado do Lab antigo, `src-tauri/src/ai`): botão "Preencher com IA" e "Buscar na internet" (pesquisa na web como reserva). O Rust chama a Messages API da Anthropic com a chave de `ANTHROPIC_API_KEY` do PC do admin (nunca vai à tela, ao backend ou ao Firestore). Modelo padrão `claude-sonnet-5-5`, trocável por `SHMUPX_LAB_AI_MODEL`. Resposta em JSON estruturado, normalizada e só uma proposta: o admin revisa e salva. O `steam_app_id` sugerido é um palpite e a tela pede conferência na loja. Cache local `ai_cache_v2.json` (200 entradas; a versão no nome invalida respostas antigas) evita pagar duas vezes pelo mesmo título. Campos opcionais gravados em `games/{id}`: developer, publisher, genre, year, platform, modes. **Consulta à Steam** (opção ligada por padrão, `ai/steam.rs`): antes da IA, o Rust busca o título na API pública da loja (sem chave, sem custo) e entrega app_id, desenvolvedoras, publicadora e data como dados confiáveis. Depois da IA, a Steam confere o resultado e garante a versão da Steam na lista (todo jogo da loja com o mesmo nome do título entra no topo, mesmo que a IA não o devolva): um `steam_app_id` que ela não confirmou é descartado, e na versão da Steam os dados da loja prevalecem sobre os da IA. Se a Steam falhar, a IA segue sozinha com um aviso. Teste ao vivo: `cargo test -- --ignored loja_real`. Fora do escopo desta migração: geração de logo/imagem, busca de capa, análise de perfil por IA.
- [ ] 1.2 Importar pastas de vídeos/screenshots por referência (sem copiar gigabytes), com indexação no SQLite.
- [ ] 1.3 Geração de miniaturas e extração de frames de referência via FFmpeg (comando Rust, em background).
- [ ] 1.4 Grid virtualizado de mídia, filtros por jogo/tipo/tela (LCD/CRT) e marcação de frames-chave.
- [ ] 1.5 Rotular frames (placar, vidas, game over) para alimentar o treino.

## 2. Canvas de mapeamento (removido)
> Substituído pela tela **Validação** (seção 6). O desenho de regiões e o comando `exportar_perfil_jogo` pertenciam ao motor único da ADR 002; com um script por jogo (ADR 004) o perfil exportado não era usado. O código fica no histórico do git.

<details><summary>Histórico do canvas</summary>

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

</details>

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

## 5. Publicação (Firebase, ADR 005 e 006)
- Publicação do **catálogo** já feita (1.1). Abaixo, a publicação de **scripts e perfis**: o app executará código baixado, então o pacote precisa ser **assinado** (chave privada só com o admin, fora do app) e o Client só aceita pacote com hash e assinatura válidos. As tarefas 5.1 a 5.4 abaixo, escritas para o Supabase, devem ser reescritas para Firebase Storage + `games/{gameId}` antes de implementar.
- [ ] 5.0 Testar o validador no Lab: rodar o script do jogo num vídeo local, comparar com o resultado esperado e guardar casos de teste de regressão.
- [ ] 5.1 Autenticação de admin e configuração do projeto Supabase.
- [ ] 5.2 Empacotar `profile.json` + `model.onnx`, calcular SHA-256 e enviar ao Storage (`profiles/{gameId}/{version}/{variant}/`).
- [ ] 5.3 Registrar release em `profile_releases` (canal `draft` → `stable`); releases imutáveis.
- [ ] 5.4 Fluxo de promoção/rollback de release.

## 6. Validação dos scripts (`features/validation`, `src-tauri/src/validacao`)
- [x] 6.1 Tela **Validação** por jogo do catálogo, com o status do script (versões via `analyze.py --list-games`).
- [x] 6.0 Scripts independentes do `shmupx_offline`, em `shmupx-lab/validators` (veja o README de lá): motor genérico em `engine/` e um script por versão em `games/<jogo>/<versão>/` (`script.json`, `config.json`, `templates/`). A tela escolhe a versão, roda com `--version` e confere que o relatório veio dela; **Gerar script** cria a 0.1.0 de um jogo novo e **Nova versão** copia a escolhida para a próxima (rascunho). `SHMUPX_VALIDATORS_ROOT` aponta a pasta fora do modo de desenvolvimento.
- [x] 6.0.2 Menu **Versões** (ADR 007): rascunho da versão do Client (novidades, correções, regras, scripts aprovados incluídos, obrigatória), agendamento com antecedência (Urgente, 24 h, 48 h, 7 dias) com prévia do aviso, cancelamento antes da liberação e situação do envio de e-mails.
- [ ] 6.0.1 Aprovar uma versão pelo Lab (hoje: `status` no `script.json`) e publicar o pacote assinado para o Client (ADR 006). Até lá o Client continua com o `shmupx_offline`, que pode ser removido depois disso.
- [x] 6.2 Vídeos do YouTube (download pelo **yt-dlp**, com progresso; `SHMUPX_YTDLP_PATH` opcional). Contra o bloqueio "confirme que não é um robô", a tela escolhe o navegador dos cookies do YouTube (`config.json` da pasta de validação; `SHMUPX_YTDLP_COOKIES_FROM_BROWSER` vale quando nada foi escolhido), e a falha aparece explicada na tabela e arquivos locais (por referência, sem cópia).
- [x] 6.3 Gabarito por vídeo: placar real e, opcionais, vidas e continues (edição na linha e no painel).
- [x] 6.4 Fila de análise (um vídeo por vez) rodando o script com o contrato `client-v1`; relatório completo guardado.
- [x] 6.5 Métricas: **acerto** (placar lido = gabarito, entre os analisados com gabarito) e **aceitação automática** (o Client aceitaria sem revisão, mesma regra de `analysis/offline.rs`).
- [x] 6.6 Verificação do ambiente (Python, OpenCV/NumPy, yt-dlp) com o comando de instalação de cada item. O yt-dlp tem instalação automática: o Lab baixa o executável oficial da última release no GitHub, confere o SHA-256 contra o `SHA2-256SUMS` da mesma release e guarda em `app_data_dir/validacao/tools` (procurado antes do PATH). Teste ao vivo: `cargo test -- --ignored instala_o_yt_dlp_real`.
- Dados locais em `app_data_dir/validacao` (banco por jogo em `datasets/<id>.json`, vídeos baixados em `videos/<id>/`); nada vai ao Firestore.
- [x] 6.7 Andamento e controle: título e duração lidos antes do download (`yt-dlp --print`), barra de progresso no download e na análise (o script publica `SHMUPX_PROGRESS n` no stderr com `--progress`, opcional e sem efeito no Client), tempo restante estimado na tela, **Cancelar** (tira da fila ou encerra o yt-dlp/Python), **Baixar de novo** e link do YouTube que copia ou abre no navegador.
- [ ] 6.8 Separar calibração/teste e travar uma versão do script contra os casos antes de publicar (fase 3, publicação assinada).
