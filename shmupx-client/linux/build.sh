#!/usr/bin/env bash
# Roda dentro do contêiner (linux/Dockerfile): copia o repositório de /src (sem node_modules nem
# builds do Windows), monta o que o pacote leva (motor de validação, scripts aceitos, FFmpeg) e gera
# o .deb e o AppImage assinados em /out. Equivalente Linux do scripts/prepare-bundle.ps1 + bundle.
set -euo pipefail
# Embutida na interface pelo Vite: sem ela o botão "Entrar com Google" não funciona.
if [ -z "${VITE_GOOGLE_DESKTOP_CLIENT_ID:-}" ]; then
    echo "erro: VITE_GOOGLE_DESKTOP_CLIENT_ID não definida (shmupx/client no OpenBao)" >&2
    exit 1
fi
mkdir -p /work/shmupx-client /work/shmupx-lab/validators /work/shmupx-mister-link

rsync -a --delete \
    --exclude node_modules --exclude target --exclude dist --exclude .venv --exclude .venv-build \
    --exclude build --exclude 'src-tauri/bundle' --exclude .git \
    /src/shmupx-client/ /work/shmupx-client/
rsync -a --delete --exclude .venv --exclude .venv-build --exclude build --exclude dist \
    /src/shmupx-lab/validators/ /work/shmupx-lab/validators/
# Código do MiSTer (FTP/SSH), compartilhado com o ShmupX Lite: dependência por caminho.
rsync -a --delete --exclude target /src/shmupx-mister-link/ /work/shmupx-mister-link/

# Motor de validação (PyInstaller). No Linux, o OpenCV "headless" (mesma API, sem depender de libGL).
cd /work/shmupx-lab/validators
if [ ! -x /cache/venv/bin/python ]; then python3 -m venv /cache/venv; fi
/cache/venv/bin/pip install --disable-pip-version-check -q \
    $(grep -v '^#' requirements-build.txt | sed 's/^opencv-python==/opencv-python-headless==/')
/cache/venv/bin/python -m PyInstaller analyze.py --noconfirm --clean --onedir --console \
    --name shmupx-validator --distpath dist --workpath build \
    --hidden-import engine.analyzers.hud_score \
    --exclude-module engine.grok --exclude-module tkinter >/dev/null

saida=/work/shmupx-client/src-tauri/bundle
rm -rf "$saida" && mkdir -p "$saida/games" "$saida/ffmpeg"
cp -r dist/shmupx-validator "$saida/validator"
python3 - "$saida" <<'PY'
import json, shutil, sys, pathlib
saida = pathlib.Path(sys.argv[1])
raiz = pathlib.Path("/work/shmupx-lab/validators/games")
for s in json.load(open("/work/shmupx-client/src-tauri/scripts-aceitos.json", encoding="utf-8")):
    origem = raiz / s["game"] / s["version"]
    if json.load(open(origem / "script.json", encoding="utf-8")).get("status") != "approved":
        sys.exit(f'{s["game"]} {s["version"]} não está aprovado no Lab')
    destino = saida / "games" / s["game"] / s["version"]
    shutil.copytree(origem, destino, ignore=shutil.ignore_patterns("raw_digits"))
PY
cp /opt/ffmpeg/ffmpeg "$saida/ffmpeg/ffmpeg"
cp /opt/ffmpeg/GPLv3.txt "$saida/ffmpeg/LICENSE.txt" 2>/dev/null || true
"$saida/validator/shmupx-validator" --list-games --games-dir "$saida/games" | head -c 300; echo

# App
cd /work/shmupx-client
npm ci --no-audit --no-fund --loglevel=error
export CARGO_TARGET_DIR=/cache/target
extra=()
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
    # Sem a chave do updater: pacote para download, sem os artefatos de atualização automática.
    echo "aviso: sem TAURI_SIGNING_PRIVATE_KEY; gerando sem assinatura do updater"
    extra=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
fi
# O linuxdeploy (AppImage) resolve as dependências de todo ELF do pacote, inclusive das bibliotecas
# do motor (PyInstaller), que se acham entre si pelo `$ORIGIN`. Ele não segue `$ORIGIN`: mostra
# essas pastas a ele.
LD_LIBRARY_PATH="$(find "$saida/validator" -name '*.so*' -printf '%h\n' | sort -u | paste -sd:)${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LD_LIBRARY_PATH
# A interface é montada aqui (o tauri.bundle.conf.json não roda o beforeBuildCommand), como no
# `npm run bundle` do Windows.
npm run build
npx tauri build --verbose --bundles deb,appimage --config src-tauri/tauri.bundle.conf.json "${extra[@]}"

mkdir -p /out
# O target vem do cache: leva só os pacotes desta versão, não os de builds anteriores.
versao=$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')
cp /cache/target/release/bundle/deb/*_"$versao"_*.deb /out/
cp /cache/target/release/bundle/deb/*_"$versao"_*.deb.sig /out/ 2>/dev/null || true
cp /cache/target/release/bundle/appimage/*_"$versao"_*.AppImage /out/
cp /cache/target/release/bundle/appimage/*_"$versao"_*.AppImage.sig /out/ 2>/dev/null || true
ls -la /out
