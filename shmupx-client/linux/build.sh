#!/usr/bin/env bash
# Roda dentro do contêiner (linux/Dockerfile): copia o repositório de /src (sem node_modules nem
# builds do Windows), monta o que o pacote leva (motor de validação, scripts aceitos, FFmpeg e o
# ShmupX Emulator, que no Linux roda pelo Wine) e gera o AppImage assinado em /out
# (o .deb deixou de ser gerado em 2026-11-27: o AppImage basta e é o que se atualiza sozinho).
# Equivalente Linux do scripts/prepare-bundle.ps1 + bundle.
set -euo pipefail
# Embutida na interface pelo Vite: sem ela o botão "Entrar com Google" não funciona.
if [ -z "${VITE_GOOGLE_DESKTOP_CLIENT_ID:-}" ]; then
    echo "erro: VITE_GOOGLE_DESKTOP_CLIENT_ID não definida (shmupx/client no OpenBao)" >&2
    exit 1
fi
mkdir -p /work/shmupx-client /work/shmupx-lab/validators /work/shmupx-lab/src-tauri /work/shmupx-mobile/src-tauri /work/shmupx-mister-link

rsync -a --delete \
    --exclude node_modules --exclude target --exclude dist --exclude .venv --exclude .venv-build \
    --exclude build --exclude 'src-tauri/bundle' --exclude .git \
    /src/shmupx-client/ /work/shmupx-client/
rsync -a --delete --exclude .venv --exclude .venv-build --exclude build --exclude dist \
    /src/shmupx-lab/validators/ /work/shmupx-lab/validators/
# Código do MiSTer (FTP/SSH), compartilhado com o ShmupX Lite: dependência por caminho.
for d in shmupx-mister-link shmupx-lab/src-tauri shmupx-mobile/src-tauri; do
    rsync -a --delete --exclude target --exclude target-dist --exclude gen /src/$d/ /work/$d/
done
cp /src/Cargo.toml /src/Cargo.lock /src/rust-toolchain.toml /work/
cp -r /src/.cargo /work/
export RUSTUP_HOME=/cache/rustup
(cd /work && rustup toolchain install)

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

# ShmupX Emulator (ShmupX Engine, executável do Windows; o Client o abre pelo Wine): o último build
# do main que passou, da release pública windows-latest, com o SHA-256 conferido.
python3 - "$saida/engine" <<'PY'
import hashlib, io, sys, urllib.request, zipfile
url = "https://github.com/ramonvalerio/shmupx-engine/releases/download/windows-latest/shmupx-engine-windows.zip"
dados = urllib.request.urlopen(url, timeout=300).read()
esperado = urllib.request.urlopen(url + ".sha256", timeout=60).read().decode().split()[0].lower()
obtido = hashlib.sha256(dados).hexdigest()
if obtido != esperado:
    sys.exit(f"SHA-256 do ShmupX Engine não confere ({obtido}, esperado {esperado})")
zipfile.ZipFile(io.BytesIO(dados)).extractall(sys.argv[1])
PY
for item in shmupx-engine.exe plugins/shmupx/init.lua COPYING VERSION.txt; do
    [ -e "$saida/engine/$item" ] || { echo "erro: pacote do ShmupX Engine incompleto, falta $item" >&2; exit 1; }
done
head -1 "$saida/engine/VERSION.txt"

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
npx tauri build --verbose --bundles appimage --config src-tauri/tauri.bundle.conf.json "${extra[@]}"

mkdir -p /out
# O target vem do cache: leva só os pacotes desta versão, não os de builds anteriores.
versao=$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')
cp /cache/target/release/bundle/appimage/*_"$versao"_*.AppImage /out/
cp /cache/target/release/bundle/appimage/*_"$versao"_*.AppImage.sig /out/ 2>/dev/null || true
ls -la /out
