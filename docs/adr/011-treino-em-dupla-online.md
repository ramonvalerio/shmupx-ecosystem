# ADR 011 — Treino em dupla online com rollback no ShmupX Engine

- **Status:** Aceito (2026-11-26)
- **Data:** 2026-11-26
- **Relaciona-se com:** o [ShmupX Engine](https://github.com/ramonvalerio/shmupx-engine) (`docs/NETPLAY.md`) e a [ADR 003](003-agente-robotx-e-salas.md) (salas).

## Contexto

Dois jogadores na mesma sala querem jogar o jogo em dupla (jogador 1 e jogador 2), cada um no seu PC. A dupla tem que ser robusta (sem desync), fluida e com o controle local com o mesmo atraso do jogo offline. O ShmupX não depende mais do Fightcade, então a rede fica no próprio motor.

## Decisão

1. **Convite pela lista de jogadores da sala:** clique direito num jogador › Convidar para jogar. Só no treino (não vale ranking nem gasta ficha), só em jogo certificado e só com o ROM no ShmupX Emulator.
2. **Sessão no Firestore** (`netplaySessions/{id}`, só os dois leem): `invited` → `accepted` (o convidado publica os endereços dele) → `ready` (o anfitrião publica os dele e abre o motor; o convidado abre o dele) → `ended`/`declined`/`canceled`/`failed` com `active: false`. Cada ponta só muda a sua parte (Security Rules). A chave da sessão vai para os motores.
3. **Conexão direta por UDP:** o Client abre uma porta, pergunta o endereço público a um STUN público e a fecha; o motor abre a mesma porta logo depois e manda pacotes para todos os endereços do parceiro (rede local, público e a própria máquina). O primeiro que responde vale (hole punching).
4. **Rollback no motor** (`-shmupx_netplay host|guest`): estado guardado a cada quadro, previsão da entrada do parceiro, volta de até 8 quadros sem vídeo nem som, sincronia de tempo sem pular quadro, CRC do estado a cada 30 quadros e ressincronia pelo estado do anfitrião. O começo da partida também usa o estado do anfitrião.
5. **Certificação por jogo:** um jogo só entra em `NETPLAY_ROMS` (`shmupx-client/src/data/netplay.ts`) depois do SyncTest e do teste longo entre dois motores (`shmupx-engine/tools/netplay/certificar.ps1`).

## Consequências

- (+) Controle local no tempo do offline em conexões boas; o parceiro chega com a correção do rollback.
- (+) Desync detectado e corrigido sozinho, sem derrubar a partida.
- (+) Sem servidor de jogo: o Firestore só faz o encontro.
- (−) Redes que não deixam conexão direta (NAT simétrico, CGNAT restrito) não conectam até existir o relay em VM.
- (−) Jogos com estado muito grande (CV1000) custam mais por quadro e podem não passar na certificação.
- (−) Os dois precisam do mesmo build do motor e do mesmo ROM (o motor confere no aperto de mão).
