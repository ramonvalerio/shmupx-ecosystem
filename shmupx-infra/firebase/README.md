# Firebase do ShmupX

Projeto: `shmupx-project-id`. Banco: `(default)`, edição **Standard**, plano **Spark**. Banco recriado em 2026-10-27 na região `us-central1`, edição Standard (metadados confirmados pela CLI); a região `southamerica-east1` anterior foi descartada por custo e o Cloud Run do backend fica na mesma região. Após a recriação, regras e índices precisam ser republicados. Autenticação da ferramenta concluída. Nove testes de regras passaram no emulador Standard. Regras e índices publicados em 2026-10-26 após autorização explícita do usuário. A CLI confirmou a compilação e liberação das regras; uma consulta posterior confirmou os cinco índices compostos e as três exclusões de indexação. Nenhum documento de usuário ou gameplay foi criado nesta publicação.

## Ambientes

| Ambiente | Banco e login | Quem usa |
| --- | --- | --- |
| **Produção** | projeto `shmupx-project-id`, banco `(default)` | backend no Cloud Run, builds de distribuição do Lab e do Client |
| **Desenvolvimento** | emuladores do Firestore e do Authentication em Docker, projeto `demo-shmupx` | `start-dev.ps1` (padrão), `npm run dev` / `tauri dev` do Lab e do Client |

Projetos `demo-*` nunca alcançam recursos reais. O backend só aceita os tokens sem assinatura do emulador com projeto `demo-*`, Firestore também no emulador e fora do Cloud Run; senão, recusa subir. O Lab e o Client escolhem o ambiente pelo modo (dev em `npm run dev`, prd nos builds) ou por `VITE_SHMUPX_ENV=dev|prd`, e mostram a faixa **DESENVOLVIMENTO** ou **PRODUÇÃO**.

```powershell
# sobe os emuladores (dados persistem em emulator-data/, fora do Git) e o backend de dev
..\..\shmupx-backend\scripts\start-dev.ps1
# só os emuladores
docker compose up -d        # interface: http://127.0.0.1:4000
docker compose down         # para e grava os dados
# backend local contra PRODUÇÃO (dados e avisos reais), só quando necessário
..\..\shmupx-backend\scripts\start-dev.ps1 -Ambiente prd
```

O login Google funciona no emulador: o usuário é criado só nele. Os testes automatizados usam o emulador temporário do `npx ... emulators:exec` (abaixo), não o do Docker, para não misturar dados de teste com os de desenvolvimento.

## Responsabilidades

- Firebase Authentication guarda identidades e credenciais; seu UID identifica os documentos de jogador. Não guardar senhas no Firestore.
- Firestore guarda os registros da plataforma descritos abaixo.
- Vídeos e análise continuam locais neste estágio. Os documentos guardam metadados, não os bytes dos vídeos. Cloud Storage exige avaliação separada de faturamento.
- Backend valida resultados, fichas, transições de estado e publicação de rankings. Admin SDK não é limitado pelas Security Rules: IAM e validação no servidor são obrigatórios.

## Coleções propostas

Todos os campos de data abaixo são `Timestamp`, não strings.

| Caminho | Dados principais | Acesso pelo aplicativo |
| --- | --- | --- |
| `users/{uid}` | `displayName`, `locale`, `createdAt`, `updatedAt` | Próprio usuário lê/cria/edita perfil validado. Sem saldo, privilégios ou senha. |
| `publicProfiles/{uid}` | Nome e avatar públicos, sincronizados pelo servidor | Leitura autenticada, sem e-mail ou dados privados. |
| `games/{gameId}` | Título, `validatorId`, versão, regras, `published` | Leitura autenticada dos publicados. |
| `events/{eventId}` | Nome, datas, jogos, regulamento, `published` | Leitura autenticada dos publicados. |
| `seasons/{seasonId}` | Nome, datas e `published` | Leitura autenticada dos publicados. |
| `gameplays/{runId}` | `ownerId`, `gameId`, `seasonId`, `eventId` opcional, estado, datas, análise e resultado | Leitura apenas pelo dono. Escrita pelo backend. |
| `analysisJobs/{jobId}` | Trabalho interno, tentativas, versão do validador e diagnóstico | Exclusivo do backend. |
| `wallets/{uid}` | Saldo e versão do registro | Dono lê, backend escreve por transação. |
| `wallets/{uid}/entries/{entryId}` | Movimentações imutáveis, motivo e referência da run | Dono lê, backend escreve. |
| `seasons/{seasonId}/gameRankings/{gameId}/entries/{uid}` | Melhor placar, nota, run de origem, nome público e data | Leitura autenticada em temporada publicada. |
| `seasons/{seasonId}/globalRankings/{uid}` | Soma normalizada, jogos considerados e versão do cálculo | Leitura autenticada em temporada publicada. |

Um documento de gameplay tem campos `ownerId`, `gameId`, `seasonId`, `status`, `createdAt`, `updatedAt`, `capture`, `analysis`, `result` e `video`. Os mapas `analysis` e `video` não têm índices automáticos: dados detalhados não são usados para filtrar listas. A origem e a comprovação do vídeo ainda precisam de contrato no backend; um caminho local recebido do cliente não é prova de integridade.

Estados propostos: `recording`, `queued`, `analyzing`, `needs_review`, `approved`, `rejected`, `failed`, `expired`. O backend controla as transições. Completar a análise não aprova automaticamente uma gameplay. O resumo do trabalho pode ser espelhado no documento privado da gameplay; diagnósticos administrativos ficam em `analysisJobs`.

As coleções aparecem quando seu primeiro documento é escrito. Não é necessário criar documentos vazios no console. IDs de jogador são Firebase Auth UID; IDs de run devem ser gerados no backend e usados como chave idempotente.

## Consultas planejadas

- Jogos: `published == true`, página de até 50.
- Eventos: `published == true`, `startsAt ASC`, até 50.
- Histórico: `ownerId == uid`, `createdAt DESC`, até 50; pode adicionar `status == estado`.
- Ranking do jogo: caminho da temporada/jogo, `bestScore DESC`, `achievedAt ASC`, até 100; document ID como desempate final estável.
- Global: caminho da temporada, `totalRating DESC`, `playerId ASC`, até 100.
- Páginas posteriores usam cursor; evitar ler coleções inteiras ou observar todos os históricos em tempo real.

As Rules não são filtros: uma consulta de gameplays sem restringir `ownerId` é negada. A configuração atual exige autenticação também para catálogo/rankings; leitura anônima pode ser decidida para o portal posteriormente. Os testes do emulador não comprovam que índices de produção estejam prontos; esperar a conclusão da criação antes de homologar consultas reais.

## Protótipo de regras

As regras seguem negação por padrão, perfil privado por dono e proibição de escrita pelo cliente em fichas, análises e rankings. A criação e atualização do perfil passam pela mesma validação de campos, tamanhos e timestamps; `createdAt` não pode mudar.

I've set up prototype Security Rules to keep the data in Firestore safe. They are designed to be secure for private player data and server-controlled scores, balances, and gameplay approvals. However, you should review and verify them before broadly sharing your app. If you'd like, I can help you harden these rules.

## Teste e publicação

Use Node 24 e Java 21 ou superior para as ferramentas atuais. As dependências de teste estão no `package-lock.json`.

```powershell
npm ci
npx -y firebase-tools@latest emulators:exec --only firestore --project demo-shmupx "npm test"
```

O teste usa apenas o projeto demonstrativo no emulador local. Não deve escrever no banco real.

Após autorização explícita da publicação no projeto real:

```powershell
npx -y firebase-tools@latest login
npx -y firebase-tools@latest firestore:databases:get "(default)" --project shmupx-project-id
npx -y firebase-tools@latest deploy --only firestore:rules,firestore:indexes --project shmupx-project-id
```

Executar a publicação modifica as permissões e índices do banco real; não é parte do teste local.

## Migração pendente

O backend Rust ainda usa PostgreSQL/Supabase; o cliente tem autenticação e catálogo demonstrativos. Esta configuração não migra contas nem substitui rotas automaticamente. Não há publicação dupla implementada. Antes de aceitar dados reais, implementar validação Firebase Auth no backend, associação do catálogo ao script offline, transações de fichas, submissão idempotente, atualização dos rankings e revisão de falhas. Preservar a regra existente do ranking global até uma decisão explícita.

## Aplicativo registrado e login escolhido

O aplicativo Web **ShmupX Client** foi registrado no projeto em 2026-10-26. Sua configuração pública, obtida pela CLI, está em `apps/shmupx-client.json`. Essa configuração identifica o projeto; não contém senha de jogador, chave privada de conta de serviço ou segredo OAuth. A segurança depende de Authentication, Rules e validação no backend.

O método escolhido pelo usuário é **Google**, considerando que as gameplays serão publicadas no canal YouTube do próprio jogador. A habilitação do provedor foi orientada no console; ainda falta o teste de login real. E-mail/senha não é requisito do fluxo inicial.

O registro Web permite usar os SDKs JavaScript no React, mas não implementa por si só o login desktop. No Tauri, a autenticação Google deve abrir o navegador do sistema e retornar por um fluxo de autorização protegido; não usar formulário de senha nem OAuth dentro da WebView embutida. Nenhum segredo OAuth deve ser distribuído dentro do cliente.

Implementado em 2026-10-26: login Google no navegador do sistema (loopback + PKCE no Rust do cliente), sessão Firebase no cliente e criação transacional de `users/{uid}`. O token demonstrativo foi removido. O backend valida o ID token do Firebase e escreve `wallets` e `gameplays` com conta de serviço; ver `shmupx-backend/TASKS.md`.

A autorização para publicar no YouTube é separada do login no ShmupX. Permissões de upload devem ser pedidas na conexão do canal, com escolha e confirmação do canal de destino. Não solicitar acesso de upload ao registrar o perfil no Firestore.
