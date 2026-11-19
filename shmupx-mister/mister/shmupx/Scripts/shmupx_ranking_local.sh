#!/bin/bash
# Menu Scripts do MiSTer > shmupx_ranking_local: top 10 das partidas deste MiSTer.
# A janela do menu Scripts nao aceita botoes; o script fica aberto ate o Cancel.
PASTA=/media/fat/shmupx

for perfil in "$PASTA"/perfis/*.json; do
  jogo=$(basename "$perfil" .json)
  "$PASTA/shmupx-mister" --pasta "$PASTA" ranking "$jogo"
  # A janela mostra so as primeiras ~14 linhas: por enquanto, um jogo por tela.
  break
done
echo
echo "Sair: Cancel"
[ -t 1 ] && exit 0
while :; do sleep 60; done
