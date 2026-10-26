# ADR 005 — Firebase como base de dados da plataforma

- **Data:** 2026-10-26
- **Status:** Firestore Standard criado pelo usuário; regras e índices testados e publicados; integração e migração pendentes.

## Escopo

O usuário ampliou o escopo para usuários, jogos, eventos, gameplays, estado das análises, fichas e rankings por jogo/global. Firebase Authentication será responsável pela identidade; Cloud Firestore guardará os registros da plataforma. Vídeos continuam locais no primeiro estágio, mantendo o plano Spark e a análise offline.

Projeto `shmupx-project-id`, banco `(default)`, edição Standard, com modo de produção selecionado. Os metadados consultados pela CLI confirmam a região `southamerica-east1`, edição `STANDARD` e `freeTier: true`. O console enviado confirma o banco criado e vazio.

A configuração em `shmupx-infra/firebase/` contém protótipo de regras, índices e testes no emulador. A autenticação da CLI foi concluída. Após autorização explícita do usuário, regras e índices foram publicados com sucesso em 2026-10-26; a consulta posterior confirmou os cinco índices compostos e três exclusões de indexação. O backend PostgreSQL/Supabase ainda existe: a mudança de destino não migra registros, identidades ou sessões automaticamente.

## Responsabilidades

- O script offline interpreta a gameplay e produz evidências e métricas.
- O backend autentica o jogador, verifica a submissão e decide sua elegibilidade. Um resultado produzido na máquina do jogador não autoriza escrita direta no ranking.
- O publicador do backend atualiza os rankings no Firebase. Credenciais administrativas permanecem exclusivamente no servidor.
- Cliente desktop e portal consultam as classificações publicadas, sem permissão de alterar pontos, notas ou posições.

## Consistência a preservar na implementação

Publicar a mesma run mais de uma vez não deve duplicar contagens. Falha de comunicação com o Firebase deve permitir repetição da publicação sem consumir outra ficha ou gerar outra run. Revisões ou invalidações precisam atualizar tanto o ranking do jogo quanto o global.

Não executar gravações independentes em PostgreSQL e Firebase como se fossem uma transação única: a estratégia de publicação e recuperação precisa ser implementada explicitamente, com controle de versões e reprocessamento.

## Regra global existente

O código atual em `shmupx-backend/src/routes/rankings.rs` soma as melhores notas normalizadas dos cinco jogos mais jogados de cada jogador. Seleciona os jogos pelo número de runs válidas, com desempate por nota e ID do jogo. Essa regra deve ser preservada na migração até uma decisão explícita de produto.

A spec 002 ainda contém alternativas de cálculo e não coincide integralmente com a implementação. Migrar o armazenamento não deve alterar silenciosamente a fórmula ou somar placares brutos de jogos com escalas diferentes.

## Próximas etapas

1. Integrar o aplicativo ao projeto Firebase e habilitar o método de autenticação escolhido.
2. Integrar Firebase Auth e implementar persistência no backend seguindo o modelo proposto.
3. Implementar publicação idempotente e consultas paginadas dos dois rankings.
4. Testar regras de acesso, empates, repetição, falhas de publicação e invalidação de runs.
5. Configurar o projeto real e validar a integração antes de substituir as leituras atuais.

O banco foi criado manualmente pelo usuário. A ferramenta publicou regras e índices após autorização explícita; nenhum documento de usuário ou gameplay foi inserido.
