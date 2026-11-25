#!/bin/bash
# Refaz a pasta de menu /media/fat/_ShmupX a partir de:
#   - shmupx.ini: username = [ramonvalerio, dantrius, ...]  -> uma pasta por jogador
#   - jogos.txt: <arquivo>|<modos>|<origem>                 -> em que modos cada jogo aparece
#   - modelos/<arquivo> (.mgl gerado e .mra copiado pelo modelos.sh)
#
#   ShmupX > 1 RAMONVALERIO > Hi-Score > Earthion
#                           > Caravan > Sprint (2 min) > Darius Gaiden
#
# O menu do MiSTer ordena as pastas por ordem alfabética e não tem arquivo de ordem; o número
# na frente impõe a ordem: quem jogou por último é o 1, os outros seguem a ordem do ini.
#
# Cada item leva jogador e modo no nome do core, que o MiSTer grava em /tmp/CORENAME
# (cortado em 31 caracteres, por isso os códigos de modo são curtos: hs 1cc spr cls):
#   .mgl: <setname same_dir="1"><código>_<id>_<modo></setname>  (código = nome do jogo, só letras e
#         dígitos, até 10; o serviço vê o core em /tmp/RBFNAME)
#   .mra: <setname>dariusg_<id>_<modo></setname>
# O código de cada jogo (o setname do .mra, ou o do .mgl) vai para codigos.txt (<código>|<nome>):
# com ele o serviço sabe qual jogo abriu e avisa o ShmupX (jogo aberto na conta).
# Rodado pelo instalador, pelo serviço (quando alguém começa a jogar) e ao abrir
# Scripts > shmupx, então basta editar o shmupx.ini ou o jogos.txt.
set -eu

SD="${SHMUPX_SD:-/media/fat}"
PASTA="$SD/shmupx"
MENU="$SD/_ShmupX"
MODELOS="$PASTA/modelos"
JOGOS="$PASTA/jogos.txt"
# <modo no jogos.txt>|<código no nome do core>|<pasta no menu>
MODOS='hiscore|hs|_Hi-Score
1cc|1cc|_1CC
sprint|spr|_Caravan/_Sprint (2 min)
classic|cls|_Caravan/_Classic (5 min)'

usuarios=$("$PASTA/shmupx-mister" --pasta "$PASTA" usuarios 2>/dev/null || true)
[ -n "$usuarios" ] || usuarios="jogador JOGADOR"
jogos=$(grep -v '^[[:space:]]*#' "$JOGOS" 2>/dev/null | grep '|' || true)

# Código do jogo no nome do core: .mra usa o setname do arquivo; .mgl, o nome do jogo (letras e
# dígitos, até 10, para caber com jogador e modo nos 31 caracteres do CORENAME).
codigo_do_jogo() {
  case "$1" in
    *.mgl) printf '%s' "${1%.*}" | tr 'A-Z' 'a-z' | tr -cd 'a-z0-9' | cut -c1-10 ;;
    *.mra) sed -n 's|.*<setname>\([^<]*\)</setname>.*|\1|p' "$2" | head -n 1 ;;
  esac
}
echo "$jogos" | while IFS='|' read -r arquivo _modos _origem; do
  [ -n "$arquivo" ] && [ -f "$MODELOS/$arquivo" ] || continue
  jogo_codigo=$(codigo_do_jogo "$arquivo" "$MODELOS/$arquivo")
  [ -n "$jogo_codigo" ] && printf '%s|%s\n' "$jogo_codigo" "${arquivo%.*}"
done > "$PASTA/codigos.txt.novo" || true
mv "$PASTA/codigos.txt.novo" "$PASTA/codigos.txt"

# Monta ao lado e troca no fim: o menu nunca fica pela metade.
NOVO="$SD/_ShmupX.novo"
rm -rf "$NOVO"
mkdir -p "$NOVO"
# Um .mra fora de _Arcade só acha o core se houver uma pasta "cores" acima dele (sem "_",
# não aparece no menu).
ln -s "$SD/_Arcade/cores" "$NOVO/cores"

# Com 10 ou mais jogadores, 01..09 mantém a ordem (a comparação do MiSTer não é numérica).
total=$(echo "$usuarios" | wc -l)
posicao=0
echo "$usuarios" | while read -r id nome; do
  posicao=$((posicao + 1))
  if [ "$total" -ge 10 ]; then numero=$(printf '%02d' "$posicao"); else numero="$posicao"; fi
  jogador="$NOVO/_$numero $nome"
  echo "$MODOS" | while IFS='|' read -r modo codigo caminho; do
    mkdir -p "$jogador/$caminho"
    echo "$jogos" | while IFS='|' read -r arquivo modos _origem; do
      [ -n "$arquivo" ] || continue
      case " $modos " in *" $modo "*) ;; *) continue ;; esac
      modelo="$MODELOS/$arquivo"
      [ -f "$modelo" ] || continue
      destino="$jogador/$caminho/$arquivo"
      case "$arquivo" in
        *.mgl)
          jogo_codigo=$(codigo_do_jogo "$arquivo" "$modelo")
          sed "s|<rbf>|<setname same_dir=\"1\">${jogo_codigo:-sx}_${id}_${codigo}</setname>\n    <rbf>|" "$modelo" > "$destino"
          ;;
        *.mra)
          sed "s|<setname>\([^<]*\)</setname>|<setname>\1_${id}_${codigo}</setname>|" "$modelo" > "$destino"
          ;;
      esac
    done
  done
done
rm -rf "$MENU"
mv "$NOVO" "$MENU"
