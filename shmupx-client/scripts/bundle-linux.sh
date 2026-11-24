#!/usr/bin/env bash
set -euo pipefail

client=$(cd "$(dirname "$0")/.." && pwd)
repo=$(dirname "$client")
saida=$client/src-tauri/target/linux
cache=$client/src-tauri/target/linux-cache
img=${DOCKER_DATA_IMG:-$(dirname "$repo")/docker-data.img}
tamanho=${DOCKER_DATA_SIZE:-20G}
data_root=/mnt/docker-data
config=$(printf '{\n  "data-root": "%s"\n}' "$data_root")

root() { if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi; }

esperar_docker() {
    for _ in $(seq 30); do root docker info >/dev/null 2>&1 && return; sleep 1; done
    echo "erro: o Docker não respondeu (veja /var/log/docker.log)" >&2
    exit 1
}

iniciar_docker() {
    if command -v systemctl >/dev/null && systemctl list-unit-files docker.service >/dev/null 2>&1; then
        root systemctl restart docker
    else
        root pkill -x dockerd || true
        while pgrep -x dockerd >/dev/null; do sleep 1; done
        root /etc/rc.d/rc.docker
    fi
    esperar_docker
}

reiniciar=0
if ! mountpoint -q "$data_root"; then
    if [ ! -f "$img" ]; then
        echo "Docker: criando $img ($tamanho, ext4)"
        root truncate -s "$tamanho" "$img"
        root mkfs.ext4 -q -F "$img"
    fi
    root mkdir -p "$data_root"
    root mount -o loop "$img" "$data_root"
    reiniciar=1
fi
if [ "$(cat /etc/docker/daemon.json 2>/dev/null)" != "$config" ]; then
    if [ -f /etc/docker/daemon.json ] && ! python3 -c 'import json, sys; sys.exit(set(json.load(open(sys.argv[1]))) != {"data-root"})' /etc/docker/daemon.json; then
        echo "erro: /etc/docker/daemon.json tem outras opções; coloque \"data-root\": \"$data_root\" nele e rode de novo" >&2
        exit 1
    fi
    root mkdir -p /etc/docker
    printf '%s\n' "$config" | root tee /etc/docker/daemon.json >/dev/null
    reiniciar=1
fi
if [ $reiniciar = 1 ] || ! root docker info >/dev/null 2>&1; then iniciar_docker; fi

if docker info >/dev/null 2>&1; then docker=(docker); else docker=(sudo docker); fi

if [ -z "${VITE_GOOGLE_DESKTOP_CLIENT_ID:-}" ] && [ -f "$client/.env.local" ]; then
    VITE_GOOGLE_DESKTOP_CLIENT_ID=$(sed -n 's/^VITE_GOOGLE_DESKTOP_CLIENT_ID=//p' "$client/.env.local")
fi
if [ -z "${VITE_GOOGLE_DESKTOP_CLIENT_ID:-}" ]; then
    read -rp "VITE_GOOGLE_DESKTOP_CLIENT_ID (shmupx/client no OpenBao): " VITE_GOOGLE_DESKTOP_CLIENT_ID
fi
if [ -z "$VITE_GOOGLE_DESKTOP_CLIENT_ID" ]; then
    echo "erro: VITE_GOOGLE_DESKTOP_CLIENT_ID vazia" >&2
    exit 1
fi
export VITE_GOOGLE_DESKTOP_CLIENT_ID
export SHMUPX_BACKEND_URL=${SHMUPX_BACKEND_URL:-https://shmupx-backend-230979440400.us-central1.run.app}

mkdir -p "$saida" "$cache/cache" "$cache/work"
"${docker[@]}" build -t shmupx-client-linux "$client/linux"

envs=$(mktemp)
trap 'rm -f "$envs"' EXIT
for nome in TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD SHMUPX_BACKEND_URL VITE_GOOGLE_DESKTOP_CLIENT_ID; do
    if [ -n "${!nome:-}" ]; then printf '%s=%s\n' "$nome" "${!nome}" >> "$envs"; fi
done
status=0
"${docker[@]}" run --rm --env-file "$envs" \
    -v "$repo:/src:ro" -v "$saida:/out" -v "$cache/cache:/cache" -v "$cache/work:/work" \
    shmupx-client-linux bash /src/shmupx-client/linux/build.sh || status=$?

"${docker[@]}" run --rm -v "$saida:/out" -v "$cache:/c" shmupx-client-linux chown -R "$(id -u):$(id -g)" /out /c
[ $status = 0 ] || { echo "erro: o build Linux falhou" >&2; exit $status; }
ls -lh "$saida"
