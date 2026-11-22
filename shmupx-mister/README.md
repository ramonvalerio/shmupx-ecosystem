# ShmupX MiSTer

Agente do ShmupX que roda no ARM do MiSTer FPGA, para quem joga no hardware, em CRT. Ele não grava vídeo: lê o placar direto da RAM do jogo, pelo espelho que o core modificado escreve na DDR3. Decisão e limites em [ADR 008](../docs/adr/008-agente-mister.md).

**Estado:** funcionando num MiSTer real. Aparece no menu, lê o placar do Earthion e grava cada partida no cartão SD. O envio ao backend do ShmupX está pronto e testado contra o emulador; vale quando o backend novo estiver publicado (ver [Envio ao ShmupX](#envio-ao-shmupx)).

## Requisitos no MiSTer

- **Core MegaDrive com espelho de RAM**, do fork de RetroAchievements de odelot ([MegaDrive_MiSTer](https://github.com/odelot/MegaDrive_MiSTer), testado com a v1.1). O release se chama `MegaDrive.rbf`; renomeie para `MegaDriveRA_<data>.rbf` e coloque em `/media/fat/shmupx/cores/`. Assim ele fica fora dos menus e não se confunde com o core oficial (`_Console/MegaDrive_<data>.rbf`). O core oficial não expõe a RAM, e o `status` avisa quando ele está carregado.
- A ROM pode estar zipada: o instalador aponta o `.mgl` para o `.md` de dentro do zip.
- **Não use o Main_MiSTer de RetroAchievements com conquistas ligadas ao mesmo tempo.** Os dois publicam a lista de endereços no mesmo lugar e um apaga a do outro.
- Acesso root (o padrão do MiSTer) para abrir `/dev/mem`.

## Compilar

```powershell
rustup target add armv7-unknown-linux-musleabihf   # uma vez
cargo build --release --target armv7-unknown-linux-musleabihf
cargo test                                         # testes com FPGA simulado, no PC
```

O resultado é um binário estático, sem dependência da imagem do MiSTer, linkado pelo `rust-lld` (veja `.cargo/config.toml`): `target/armv7-unknown-linux-musleabihf/release/shmupx-mister`.

## Instalar no MiSTer

**Pelo ShmupX Client (o caminho do jogador):** Importar jogos › MiSTer FPGA › *Conectar ao meu MiSTer*. Com o IP e a senha do root (padrão `1`), o Client entra por SSH, baixa o pacote publicado (`downloads/mister/latest.json`, conferido pelo SHA-256), extrai em `/media/fat/shmupx`, baixa o core MegaDrive do odelot (v1.1) se faltar, grava o `shmupx.ini` (jogador, `api` e um token novo, com `shmupx.ini.bak`) e roda o `instalar.sh`. Código: `shmupx-client/src-tauri/src/steam/mister_ssh.rs`.

**Publicar o pacote que o Client instala:** `python scripts/pacote.py` compila e gera `target/pacote/shmupx-mister-<versão>-<commit>.tar` e `latest.json`; os dois vão para `s3://<bucket do site>/downloads/mister/` (o `latest.json` com `Cache-Control: no-cache`).

**Para desenvolver**, com o MiSTer ligado na rede, a partir do Windows:

```powershell
.\scripts\implantar.ps1 -Mister 192.168.0.50
# ou, se a ROM não estiver em /media/fat/games:
.\scripts\implantar.ps1 -Mister 192.168.0.50 -Rom /media/fat/games/Genesis/Earthion.md
```

O script compila, copia tudo para `/media/fat/shmupx` (pede a senha do root, que por padrão é `1`) e roda o [`instalar.sh`](mister/shmupx/instalar.sh) no MiSTer. O instalador:

| O que | Onde | Para quê |
| --- | --- | --- |
| Pasta de menu | `/media/fat/_ShmupX/` | **ShmupX** na tela principal do MiSTer |
| Uma pasta por jogador | `_ShmupX/_1 RAMONVALERIO`, `_2 DANTRIUS`... | Quem vai jogar; vem do `username` do ini. O MiSTer ordena pastas por ordem alfabética e não tem arquivo de ordem, então o número impõe a ordem: quem jogou por último é o 1 (gravado em `shmupx/ultimo_jogador` quando a partida começa), os outros seguem o ini |
| Uma pasta por modo | `_<JOGADOR>/_Hi-Score`, `_1CC`, `_Caravan/_Sprint (2 min)`, `_Caravan/_Classic (5 min)` | Escolha do modo |
| Um item por jogo | `.../_Hi-Score/Earthion.mgl`, `.../Ketsui.mra` | Abre o jogo com jogador e modo no `setname` (`SX_kwayd_hs` nos consoles, `ket_kwayd_cls` no arcade); o MiSTer grava em `/tmp/CORENAME`, **cortado em 31 caracteres** (por isso os códigos `hs 1cc spr cls`). O serviço vê o core em `/tmp/RBFNAME` |
| Lista de jogos | `shmupx/jogos.txt` (`nome\|modos\|origem`) | Em que modos cada jogo aparece (`hiscore 1cc sprint classic`) e de onde vem: `.mra` de `_Arcade`, ou core, delay, index e busca da ROM para `.mgl`. Hoje: Earthion, Darius Gaiden, Ketsui, Aero Fighters 2 |
| Modelos dos jogos | `shmupx/modelos/` | Gerados pelo `modelos.sh`; o `menu.sh` copia para cada jogador e modo. Os dois rodam ao abrir Scripts > shmupx |
| Link `cores` | `_ShmupX/cores -> _Arcade/cores` | Um `.mra` fora de `_Arcade` só acha o core com uma pasta `cores` acima dele (conferido no aparelho) |
| Telas no menu Scripts | `/media/fat/Scripts/shmupx*.sh` | `shmupx` (estado e últimas partidas), `shmupx_ranking_local` (top 10 deste MiSTer), `shmupx_ranking_online` (ainda indisponível) |
| Jogadores | `shmupx/shmupx.ini` (`username = [ramonvalerio, dantrius, kwayd, convidado]`) | Um nome ou uma lista; em maiúsculas, até 12 caracteres. O menu é refeito ao abrir Scripts > shmupx |
| Serviço | `linux/user-startup.sh` → `shmupx/iniciar.sh` | Liga no boot e registra cada partida em `shmupx/partidas/*.json` |

Pode rodar de novo à vontade: o instalador não duplica nada e não sobrescreve perfis que já estão no MiSTer.

Caravan: o serviço só conta o placar feito em 2 min (Sprint) ou 5 min (Classic), medidos em quadros do core a partir do primeiro ponto, porque ele não distingue a tela-título do início da partida. 1CC ainda não verifica continues: por enquanto registra como Hi-Score do modo. Jogos de arcade (Darius Gaiden, core Rayforce) abrem pelo menu e sobem o jogador ao topo, mas ainda não têm leitura de placar: o espelho de RAM só existe no core MegaDrive modificado.

Fluxo do jogador: **ShmupX > RAMONVALERIO > Hi-Score > Earthion**. O `setname` do `.mgl` faz o MiSTer gravar `MegaDrive_ramonvalerio` em `/tmp/CORENAME` (conferido no aparelho); assim o serviço sabe quem joga. O jogo abre e o serviço percebe sozinho, lê o placar durante a partida e grava o resultado quando ele volta ao menu.

Um `.mgl` só abre cores, não programas, e a janela do menu Scripts não repassa botões ao script: no código do Main_MiSTer, o script roda por `popen` sem entrada, o A/B/Y aciona o Cancel e o controle fica preso ao Main com o menu aberto. Por isso a navegação (modo > jogo) usa as pastas do menu principal. O ranking e o estado ficam em telas de leitura no menu Scripts, com no máximo ~28 colunas e ~14 linhas, porque a janela não rola.

Conferido num MiSTer real em 2026-11-11, com o core v1.1 do odelot e o Earthion v4.0.0 zipado:
- o `.mgl` (`delay=1`, `index=1`, caminho `../` e entrada dentro do zip) abre o jogo, inclusive por `echo "load_core /media/fat/_ShmupX/Earthion.mgl" > /dev/MiSTer_cmd`;
- `/tmp/CORENAME` fica `MegaDrive`;
- o espelho responde (core `0x0200`, protocolo `0x05`) e o `dump` leva 0,27 s.

## Envio ao ShmupX

As partidas entram no ranking do jogo junto com as de PC (mesmo jogo e versão). Decisões na [ADR 008](../docs/adr/008-agente-mister.md).

1. No **shmupx-client**: Configurações > Conexões > **MiSTer FPGA** > *Enviar token para meu e-mail*.
2. No `shmupx/shmupx.ini` do MiSTer, o endereço do backend e a linha que chegou por e-mail:
   ```ini
   api = https://<backend do ShmupX>
   token.ramonvalerio = sxm_...
   ```
3. `./shmupx-mister conexao` confere cada jogador **sem gastar ficha**.

O perfil do jogo diz o id no catálogo e a versão (`"jogo_id": "earthion"`, `"versao": "4.0.0"`); o jogo precisa estar publicado no catálogo com o modo jogado. Cada partida gasta 1 ficha, aberta no primeiro ponto. Sem `api`, sem token (CONVIDADO) ou sem ficha, a partida fica só no ranking local, com o motivo gravado no JSON (`envio`). Sem internet, vai para `pendentes/` e é reenviada a cada minuto com o MiSTer ocioso.

## Uso por linha de comando

```sh
cd /media/fat/shmupx
./shmupx-mister status                       # core carregado e estado do espelho
./shmupx-mister dump a.bin                   # 64 KB da RAM do 68000, em ~0,3 s
./shmupx-mister procurar a.bin=12300 b.bin=45600 c.bin=78900
./shmupx-mister placar earthion.json --continuo
```

### Descobrir o endereço do placar de um jogo novo

O Earthion já está mapeado em [`perfis/earthion.json`](perfis/earthion.json): 8 dígitos, um por palavra de 16 bits, a partir de `0xFF1C48`. Foi descoberto e conferido ao vivo em 2026-11-11 com 230100 na tela. Para outro jogo:

1. Carregue o jogo no core modificado e jogue até ter algum placar.
2. Pause, anote o placar da tela e rode `./shmupx-mister dump a.bin`.
3. Repita com placares diferentes (`b.bin`, `c.bin`...). Três ou quatro bastam.
4. Rode `procurar` com os pares `dump=placar`. Pode ser no MiSTer ou no PC, depois de copiar os `.bin`. Ele testa BCD e binário (de 2 a 4 bytes) e um dígito por palavra (8 dígitos), com multiplicadores 1, 10 e 100. Zeros à esquerda geram candidatos repetidos, deslocados em uma palavra; nesse caso, fique com o de multiplicador 1.
5. Coloque o candidato no perfil (`"endereco": "0xFF...."`, bytes, codificação e multiplicador) e confira com `placar --continuo` enquanto joga.

## Como funciona

- O ARM escreve, a partir de `0x3D040000`, uma lista ordenada de até 4096 endereços e um id de pedido.
- A cada VBlank, o FPGA devolve um byte por endereço em `0x3D048008` e assina a resposta com o id e o número do quadro.
- A leitura só é aceita se o FPGA não estava no meio de uma varredura e se o quadro não mudou durante a cópia.
- O placar só é aceito quando se repete em dois quadros seguidos.

Os endereços seguem a convenção do RetroAchievements (bytes trocados dentro de cada palavra). O código trabalha na ordem do 68000 e converte só na fronteira (`genesis.rs`).

O protocolo foi reimplementado a partir da descrição do fork, que é GPL-3. Nenhum código dele foi copiado.

## Próximos passos

- Validar no hardware: `status`, `dump` e um placar real do Earthion.
- Identificar o jogo carregado. Hoje o perfil é escolhido à mão; o `/tmp/CORENAME` só diz o core.
- Cronômetro de 45 min, login do jogador e envio das partidas ao backend.
- Uma tela própria na CRT, além do texto do menu Scripts.
- Outros cores com espelho (SNES, PC Engine e arcade, se o fork os suportar).
