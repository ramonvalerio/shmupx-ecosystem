# ADR 007 — Versões do Client e avisos antecipados

- **Status:** Aceito; Lab, backend e sininho do Client implementados. Atualização automática do Client pendente.
- **Data:** 2026-10-27
- **Complementa:** a [ADR 004](004-validadores-offline-por-jogo.md) (scripts por versão) e a [ADR 006](006-pacote-por-jogo-e-robotx-offline.md) (publicação de scripts).

## Contexto

Uma versão nova do ShmupX Client pode mudar o que o jogador vive: novidades e correções, regras de competição e, principalmente, os scripts de validação aprovados no Lab, que só chegam aos jogadores com o Client. Mudanças assim precisam ser anunciadas antes de valer, e algumas precisam ser obrigatórias para que todos joguem com as mesmas regras.

## Decisão

### O que é uma versão
`releases/{versão}` no Firestore (`1.2.3`): título, novidades, correções, mudanças de regra, scripts de validação incluídos (`jogo@versão`) e se a atualização é obrigatória. O Lab escreve pelo backend (`/api/admin/releases`, só admin).

### Ciclo
- **Rascunho:** editável no Lab (menu **Versões**).
- **Agendada:** ao agendar, o admin escolhe a antecedência: **Urgente** (libera na hora, como antes), **24 h**, **48 h** ou **7 dias**. O servidor fixa `releaseAt`, publica o aviso e dispara os e-mails. O texto não muda mais depois disso. A versão precisa ser maior que todas as já agendadas ou liberadas.
- **Liberada:** pelo relógio do servidor, quando chega `releaseAt`. Não há job: o status é calculado na leitura.
- **Cancelada:** só antes da liberação; o aviso some do Client.

### Avisos
- **No Client:** sininho no topo com o contador de não lidas, lista ao clicar e tela de detalhe ao clicar numa notificação (`notifications/{id}`; o lido fica em `players/{uid}.readNotifications`). Uma faixa no topo avisa versão disponível ou obrigatória.
- **Por e-mail (Amazon SES):** para os jogadores com e-mail verificado e aviso ligado (`players/{uid}.emailUpdates`, ligado por padrão, desligável no próprio aviso). Um e-mail por destinatário, sem expor os outros. Configuração só no servidor (`EMAIL_FROM` e a role da AWS em `AWS_SECRETS_ROLE_ARN`, sem chave guardada); sem ela, só o aviso no Client, e o Lab mostra o motivo.
- O jogador entra na lista de destinatários quando o Client consulta as notificações (o backend só conhece o e-mail pelo token de quem está logado).

### Obrigatoriedade
O Client manda a própria versão em toda chamada (`x-shmupx-client-version`). Depois que uma versão obrigatória é liberada, `POST /api/runs/start` recusa Clients mais antigos, antes de debitar a ficha. Versões não obrigatórias só avisam.

## Consequências

- (+) Mudança de regra ou de script nunca pega o jogador de surpresa, e a antecedência é escolha do admin a cada versão.
- (+) Uma versão obrigatória garante que todos jogam com os mesmos scripts de validação.
- (−) A atualização ainda é manual: o aviso diz que existe versão nova, mas o Client não se atualiza sozinho.
- Pendente: atualizador do Tauri (pacote assinado, chave privada fora do app e do servidor, como na ADR 006), liberado só depois de `releaseAt`; publicar junto os scripts incluídos na versão e tirar a lista fixa de validadores do Client (`offline.rs`).
