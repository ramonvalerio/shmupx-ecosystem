# ADR 008 — Agente para MiSTer FPGA

- **Status:** Aceito; agente, menu e leitura de placar do Earthion funcionando num MiSTer real (2026-11-11). Envio ao backend implementado e testado contra o emulador; falta publicar o backend.
- **Data:** 2026-11-11
- **Relaciona-se com:** a [ADR 004](004-validadores-offline-por-jogo.md) (validação por vídeo, que continua sendo o caminho do PC).

## Contexto

Parte do público de shmups joga no hardware, em CRT, e o MiSTer FPGA é o caminho mais comum para isso. O ShmupX Client não roda nele:

- a captura usa APIs do Windows (`ddagrab`, `gdigrab`, `dshow`);
- os binários empacotados são Windows x64;
- o MiSTer é um Cortex-A9 de 32 bits, sem área de trabalho;
- a imagem vem do FPGA direto para a TV, sem passar pelo ARM, e o ARM não teria força para codificar vídeo a 60 fps.

Por outro lado, o fork de RetroAchievements do MiSTer (odelot: Main_MiSTer e cores modificados) faz o core expor a RAM do jogo ao ARM pela DDR3, em `0x3D000000`.

## Decisão

Um programa separado, `shmupx-mister`, em Rust, compilado estático para `armv7-unknown-linux-musleabihf`. Ele lê o placar **da RAM do jogo, não da imagem**.

- **Começa pelo Mega Drive:** o Earthion, primeiro jogo do validador, também existe em cartucho de Mega Drive. No core modificado, o ARM pede até 4096 endereços por quadro e o FPGA devolve os bytes a cada VBlank.
- **Um perfil por jogo** (`perfis/<jogo>.json`) diz onde está o placar e como decodificá-lo. O endereço é descoberto com `dump` e `procurar`, comparando cópias da RAM com placares conhecidos.
- O protocolo foi reimplementado a partir da descrição; não há código GPL do fork no projeto.

## Consequências

- (+) Joga-se em CRT, sem PC nem placa de captura, e sem atraso de vídeo.
- (+) Ler a RAM é exato e não depende de resolução, filtro ou escala, ao contrário de ler o placar pela imagem.
- (−) Sem vídeo, a partida não tem prova visual. O anti-fraude precisa de outro desenho, por exemplo amostrar o placar e o estado do jogo ao longo da partida, em vez de olhar só o resultado final.
- (−) Depende de um core de terceiros, experimental, e só funciona nos cores que ele modifica. A versão do protocolo precisa ser conferida (`status` mostra a versão).
- (−) Não convive com o Main_MiSTer de RetroAchievements com conquistas ligadas: os dois escrevem a mesma lista de endereços.
- Pendente: leitura de placar nos cores sem espelho (arcade, NeoGeo), início do Caravan pelo começo da fase (hoje conta do primeiro ponto) e verificação de continues no 1CC.

## Decisões de 2026-11-12: partidas do MiSTer no ShmupX

- **Mesmo ranking.** Partidas do MiSTer entram no ranking do jogo junto com as de PC, emulador, Steam ou placa original, desde que sejam do mesmo jogo e da mesma versão. A gameplay grava `source` (`pc` ou `mister`, decidido pela credencial, nunca pelo corpo), `mode` e `gameVersion`.
- **Fichas.** Mesma regra do PC: abrir a partida (`/api/runs/start`) debita 1 ficha. No 1CC a partida usa uma ficha só; não existe continue com outra ficha.
- **Vidas e continues** viram opcionais em `/api/runs`: o MiSTer só lê o placar.
- **Credencial do MiSTer.** O MiSTer não faz login com Google. O jogador logado no Client pede um token (`POST /api/mister/tokens`); o backend gera `sxm_...`, guarda só o hash em `misterTokens/{sha256}` e manda o token **por e-mail** para o endereço verificado da conta (Amazon SES). O token não aparece na tela nem volta pela API. Nas rotas de partida, `Bearer sxm_...` vale como o dono do token; revogar = `revoked: true`.
- **Envio.** O agente abre a partida no primeiro ponto, encerra e envia o placar ao voltar ao menu. Sem rede, guarda em `pendentes/` e reenvia depois. O HTTPS sai pelo `curl` do MiSTer com `/etc/ssl/certs/cacert.pem`, com verificação do servidor.

