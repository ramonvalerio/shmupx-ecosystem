#!/bin/sh
# Liga o serviço do ShmupX se ele ainda não estiver rodando. Chamado no boot pelo
# /media/fat/linux/user-startup.sh e pelo instalador.
PASTA="$(cd "$(dirname "$0")" && pwd)"

if pidof shmupx-mister >/dev/null 2>&1; then
  exit 0
fi
# Log em /tmp (RAM): não gasta o cartão SD.
nohup "$PASTA/shmupx-mister" servico "$PASTA" >/tmp/shmupx.log 2>&1 &
echo "serviço ShmupX ligado"
