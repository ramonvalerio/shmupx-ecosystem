# ADR 006 — Pacote por jogo e RobotX offline

- **Status:** Aceito
- **Data:** 2026-10-26
- **Adia:** a [ADR 003](003-agente-robotx-e-salas.md) (voz durante o jogo, RobotX na nuvem).
- **Complementa:** a [ADR 004](004-validadores-offline-por-jogo.md) (validador por jogo) e a [ADR 005](005-rankings-no-firebase.md) (Firebase).

## Contexto

Cada jogo suportado precisa de uma sala, de um assistente especializado e de um validador próprio. A ADR 003 previa o RobotX na nuvem, acionado por voz durante a gameplay, o que gera custo de tokens por pergunta. Ainda não existem planos pagos definidos.

## Decisão

### Um pacote por jogo
- 1 jogo = 1 sala = 1 RobotX = 1 validador.
- O `gameId` é um identificador curto e legível (ex.: `earthion`), o mesmo em todos os lugares: `games/{gameId}` no Firestore, sala, conhecimento do RobotX e registro do validador em `shmupx_offline`. Isso substitui o "catálogo UUID → script" pendente na ADR 004.
- A sala é o próprio jogo: não há entidade de sala separada, e a lista de salas é a lista de jogos publicados.
- A gameplay deve registrar a versão do validador que produziu o resultado (pendente de implementação).

### RobotX gratuito: offline e fora da gameplay
- Roda na máquina do jogador, sem custo de tokens para a plataforma.
- Entrada por texto, no chat da sala. Sem voz nesta versão.
- O conhecimento vem do pacote do jogo da sala atual. O app só carrega o pacote desse jogo, então o isolamento entre jogos é feito por construção.
- Indisponível enquanto houver run em andamento: o app bloqueia o uso e libera a memória do modelo ao iniciar a captura.
- A primeira versão pode ser um modelo local pequeno (llama.cpp) alimentado com o guia do jogo, ou busca em perguntas e respostas curadas. A escolha fica para a implementação.

### Voz durante a gameplay: futuro plano pago
- Voz por botão do controle durante a run depende de processamento na nuvem (um modelo local disputaria GPU/CPU com o jogo).
- Fica adiada como candidata ao plano pago, cuja receita cobriria o custo de tokens. O desenho da ADR 003 é a referência para esse recurso.

### Lab gerencia o catálogo; o servidor decide
- O Lab cadastra e publica jogos em `games/{gameId}` pelo backend (`GET /api/admin/games`, `PUT /api/admin/games/{id}`). O Lab não carrega credencial do Firestore.
- Admin = e-mail verificado do token Firebase presente em `ADMIN_EMAILS` no servidor. O servidor decide a cada chamada; nenhuma permissão vive no app.
- O Client lê os jogos `published` direto do Firestore (Security Rules), em tempo real. Despublicar um jogo também bloqueia novas runs.
- O Lab também deve testar o validador de cada jogo (casos de teste com resultado esperado) e publicar o script (fase seguinte, abaixo).

### O Client não é confiável
Qualquer app no PC do jogador pode ser lido, adulterado ou imitado; ofuscar só atrasa. O ranking, portanto, nunca depende da honestidade do Client:
- O servidor é a única autoridade: identidade do token, ficha, prazo da run, estado e aprovação. Todo resultado entra como `needs_review`.
- **Pendente, pré-requisito para publicar ranking:** o servidor recebe a evidência da run (vídeo ou quadros-chave) e reanalisa por amostragem e em placares altos; limites de frequência; detecção de resultados fora do padrão; versão do validador registrada na gameplay.
- **Publicação de scripts (fase seguinte):** o app executa código baixado, então só aceita pacote com SHA-256 e assinatura válidos. A chave privada de assinatura fica com o admin, fora do app e do servidor; o app embute só a chave pública.

## Consequências

- (+) Custo zero de IA no plano gratuito.
- (+) Nenhum impacto de desempenho durante a gameplay.
- (+) Um único identificador liga Firestore, sala, RobotX e validador.
- (−) Download maior do app (modelo local), possivelmente opcional e sob demanda.
- (−) Respostas de modelo local são mais limitadas que as de modelos na nuvem; o conhecimento de cada jogo precisa ser bem curado.
- Pendente: definição dos planos, formato do pacote de conhecimento e distribuição dos pacotes.
