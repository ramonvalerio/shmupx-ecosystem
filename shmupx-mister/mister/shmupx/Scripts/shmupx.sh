#!/bin/bash
# Menu Scripts do MiSTer > shmupx: estado do servico e ultimas partidas.
#
# Pelo menu Scripts o MiSTer nao da terminal ao script: mostra a saida numa janela do
# proprio menu, com cerca de 28 colunas e o botao Cancel. Por isso o texto e curto, sem
# acentos e sem codigos de cor, e o script fica esperando ate o Cancel. Num terminal de
# verdade (F9 ou SSH) a mesma tela aparece numa janela do dialog.
PASTA=/media/fat/shmupx
LARGURA=28

# 1101200 -> 1.101.200
milhar() {
  local n="$1" r=""
  while [ ${#n} -gt 3 ]; do
    r=".${n: -3}$r"
    n="${n:0:${#n}-3}"
  done
  echo "$n$r"
}

# Mudancas no shmupx.ini (jogadores) e no jogos.txt entram no menu ao abrir esta tela.
"$PASTA/modelos.sh" >/dev/null 2>&1 || true
"$PASTA/menu.sh" >/dev/null 2>&1 || true

if pidof shmupx-mister >/dev/null 2>&1; then
  servico="ligado"
else
  "$PASTA/iniciar.sh" >/dev/null 2>&1
  servico="religado agora"
fi
estado=$(cat /tmp/shmupx-estado.txt 2>/dev/null || echo "sem informacao ainda")

tela() {
  echo "========= ShmupX ========="
  echo "Servico: $servico"
  echo "$estado" | fold -s -w "$LARGURA"
  echo
  echo "ULTIMAS PARTIDAS"
  local partidas arquivo jogo pontos inicio quando
  partidas=$(ls -t "$PASTA"/partidas/*.json 2>/dev/null | head -n 4)
  if [ -z "$partidas" ]; then
    echo " nenhuma ainda"
  fi
  for arquivo in $partidas; do
    jogo=$(sed -n 's/.*"jogo": "\(.*\)".*/\1/p' "$arquivo")
    pontos=$(sed -n 's/.*"placar_maximo": \([0-9]*\).*/\1/p' "$arquivo")
    inicio=$(sed -n 's/.*"inicio": \([0-9]*\).*/\1/p' "$arquivo")
    quando=$(date -d "@$inicio" '+%d/%m %H:%M' 2>/dev/null || echo "$inicio")
    printf ' %s %.14s\n' "$quando" "$jogo"
    printf ' %26s\n' "$(milhar "$pontos") pts"
  done
  echo
  echo "Jogar: menu > ShmupX"
}

TTY=/dev/tty
if [ -t 1 ] || { command -v dialog >/dev/null 2>&1 && { : > "$TTY"; } 2>/dev/null; }; then
  # Terminal de verdade: janela do dialog, fecha no botao Voltar.
  texto=$(tela | sed '1d')
  stty sane < "$TTY" 2>/dev/null || true
  dialog --title " ShmupX " --ok-label "Voltar" --msgbox "$texto" 20 34 < "$TTY" > "$TTY" 2>&1
  clear > "$TTY" 2>/dev/null
else
  # Janela do menu Scripts: mostra e espera o Cancel.
  tela
  echo "Sair: Cancel"
  while :; do sleep 60; done
fi
