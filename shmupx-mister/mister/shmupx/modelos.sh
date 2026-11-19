#!/bin/bash
# Gera shmupx/modelos/ a partir do jogos.txt: um .mgl por jogo de console (acha a ROM em
# games/) e uma cópia do .mra de cada jogo de arcade. Depois o menu.sh espalha os modelos
# pelas pastas de jogador e modo. Rodado pelo instalador e ao abrir Scripts > shmupx.
set -euo pipefail

SD="${SHMUPX_SD:-/media/fat}"
PASTA="$SD/shmupx"
mkdir -p "$PASTA/modelos"

# --- Core: o MegaDrive com espelho de RAM (fork de RetroAchievements) ---
# Fica em shmupx/cores/MegaDriveRA_<data>.rbf, fora dos menus, para não se misturar com o
# core oficial (_Console/MegaDrive_<data>.rbf). Nos jogos, a origem @megadrive-ra aponta para ele.
if ls "$PASTA"/cores/MegaDriveRA_*.rbf >/dev/null 2>&1; then
  RBF_MD_RA="shmupx/cores/MegaDriveRA"
else
  RBF_MD_RA="_Console/MegaDrive"
  echo "AVISO: não achei $PASTA/cores/MegaDriveRA_*.rbf (core com espelho de RAM)."
  echo "       Os jogos de Mega Drive abrem no core oficial, mas o placar não será lido."
fi

xml() {
  printf '%s' "$1" | sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g' -e 's/"/\&quot;/g'
}

# .mgl: acha a ROM em games/ e grava o modelo. O caminho do .mgl é relativo à pasta de jogos
# do core; subir um nível e descer pela pasta real funciona seja qual for o nome dela.
gerar_mgl() {
  local arquivo="$1" origem="$2" rbf delay index busca rom relativo interno
  IFS=';' read -r rbf delay index busca <<< "$origem"
  [ "$rbf" = "@megadrive-ra" ] && rbf="$RBF_MD_RA"
  rom=$(find "$SD/games" -maxdepth 4 -type f -iname "$busca" ! -name '._*' 2>/dev/null | sort | head -n 1 || true)
  [ -n "$rom" ] || rom=$(find "$SD/games" -maxdepth 4 -type f -iname "${busca%.*}.zip" ! -name '._*' 2>/dev/null | sort | head -n 1 || true)
  if [ -z "$rom" ]; then
    echo "AVISO: ROM de ${arquivo%.mgl} ($busca) não encontrada em $SD/games; o jogo fica fora do menu."
    rm -f "$PASTA/modelos/$arquivo"
    return 0
  fi
  relativo="../${rom#"$SD"/games/}"
  # ROM zipada: o MiSTer abre o arquivo de dentro do zip (.../jogo.zip/jogo.md).
  case "$rom" in
    *.zip|*.ZIP)
      interno=$(unzip -l "$rom" | sed -n 's/^ *[0-9][0-9]* *[0-9-][0-9-]* *[0-9:][0-9:]* *//p' | grep -v '/$' | grep -v '^[0-9]* files*$' | head -n 1 || true)
      if [ -z "$interno" ]; then
        echo "AVISO: zip vazio: $rom"; return 0
      fi
      relativo="$relativo/$interno"
      ;;
  esac
  cat > "$PASTA/modelos/$arquivo" <<MGL
<mistergamedescription>
    <rbf>$(xml "$rbf")</rbf>
    <file delay="$delay" type="f" index="$index" path="$(xml "$relativo")"/>
</mistergamedescription>
MGL
  echo "jogo: ${arquivo%.mgl} ($rbf: ${rom#"$SD"/games/})"
}

# .mra: cópia do arquivo de _Arcade (o menu.sh põe jogador e modo no setname).
copiar_mra() {
  local arquivo="$1" origem="$2"
  if [ -f "$SD/_Arcade/$origem" ]; then
    cp "$SD/_Arcade/$origem" "$PASTA/modelos/$arquivo"
    echo "jogo: ${arquivo%.mra} (arcade: $origem)"
  else
    echo "AVISO: $SD/_Arcade/$origem não encontrado; o jogo fica fora do menu."
    rm -f "$PASTA/modelos/$arquivo"
  fi
}

# --- Jogos: jogos.txt ---
rm -f "$PASTA"/modelos/*.mgl "$PASTA"/modelos/*.mra
grep -v '^[[:space:]]*#' "$PASTA/jogos.txt" | grep '|' | while IFS='|' read -r arquivo modos origem; do
  case "$arquivo" in
    *.mgl) gerar_mgl "$arquivo" "$origem" ;;
    *.mra) copiar_mra "$arquivo" "$origem" ;;
  esac
done
