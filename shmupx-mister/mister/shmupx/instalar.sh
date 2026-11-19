#!/bin/bash
# Instala o ShmupX no MiSTer. Roda no próprio MiSTer, a partir de /media/fat/shmupx.
#
#   ./instalar.sh
#
# - Cria a pasta de menu _ShmupX (aparece como "ShmupX" na tela principal): uma pasta por
#   jogador do shmupx.ini e, dentro, uma por modo (Hi-Score, 1CC, Caravan) com os jogos.
# - Coloca no menu Scripts: shmupx (estado), shmupx_ranking_local e shmupx_ranking_online.
# - Os jogadores ficam em shmupx/shmupx.ini (username = [ramonvalerio, convidado]).
# - Liga o serviço que registra as partidas, agora e a cada boot.
#
# Os jogos e seus modos vêm do jogos.txt. Variável opcional: SHMUPX_SD (padrão /media/fat).
set -euo pipefail

SD="${SHMUPX_SD:-/media/fat}"
PASTA="$SD/shmupx"
MENU="$SD/_ShmupX"

echo "== ShmupX para MiSTer =="

chmod +x "$PASTA/shmupx-mister" "$PASTA/iniciar.sh" "$PASTA/menu.sh" "$PASTA/modelos.sh"
mkdir -p "$PASTA/perfis" "$PASTA/partidas" "$SD/Scripts"
# Modelos dos jogos (um arquivo por jogo); o jogos.txt diz em que modos cada um aparece e o
# menu.sh copia para a pasta de cada jogador. A versão anterior separava por modo.
rm -rf "$PASTA/modelos/_Hi-Score" "$PASTA/modelos/_1CC" "$PASTA/modelos/_Caravan"
mkdir -p "$PASTA/modelos"
# Jogadores: edite o username (um nome ou uma lista entre colchetes).
[ -f "$PASTA/shmupx.ini" ] || printf 'username = [jogador, convidado]\n' > "$PASTA/shmupx.ini"
# Restos de versões anteriores (troca de nome pelo menu, que foi retirada).
rm -f "$SD/Scripts/shmupx_nome.sh" "$SD/linux/shmupx-nome"

# --- Jogos: jogos.txt -> modelos/ ---
SHMUPX_SD="$SD" "$PASTA/modelos.sh"

# --- Menu principal: ShmupX > <jogador> > <modo> > <jogo> ---
SHMUPX_SD="$SD" "$PASTA/menu.sh"
echo "menu: ShmupX > $(ls "$MENU" | grep '^_' | sed 's/^_//' | xargs)"

# --- Menu Scripts: estado e rankings ---
for script in "$PASTA"/Scripts/shmupx*.sh; do
  cp "$script" "$SD/Scripts/"
  chmod +x "$SD/Scripts/$(basename "$script")"
  echo "menu Scripts: $(basename "$script" .sh)"
done

# --- Serviço no boot ---
STARTUP="$SD/linux/user-startup.sh"
if [ ! -f "$STARTUP" ]; then
  printf '#!/bin/sh\n' > "$STARTUP"
fi
if ! grep -q 'shmupx/iniciar.sh' "$STARTUP"; then
  printf '\n# ShmupX\n[ -x %s/iniciar.sh ] && %s/iniciar.sh\n' "$PASTA" "$PASTA" >> "$STARTUP"
fi
chmod +x "$STARTUP"
"$PASTA/iniciar.sh"

echo "Pronto. Volte ao menu principal do MiSTer: a pasta ShmupX já está lá."
