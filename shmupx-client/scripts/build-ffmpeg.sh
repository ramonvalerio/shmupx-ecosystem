#!/usr/bin/env bash
set -euo pipefail

cache=${1:?uso: build-ffmpeg.sh <pasta de cache>}
mkdir -p "$cache"
cache=$(cd "$cache" && pwd)
receita=r2
jobs=${JOBS:-$(nproc)}

case "$(uname -s)" in
    MINGW*) exe=ffmpeg.exe; sistema=win64 ;;
    Linux) exe=ffmpeg; sistema=linux64 ;;
    *) echo "erro: sistema sem suporte: $(uname -s)" >&2; exit 1 ;;
esac

ultimo_compilado() {
    local d
    for d in $(ls -d "$cache"/ffmpeg-"$sistema"-* 2>/dev/null | sort -V -r); do
        if [ -x "$d/bin/$exe" ]; then echo "$d"; return 0; fi
    done
    return 1
}

for ferramenta in git make nasm pkg-config cc; do
    if ! command -v "$ferramenta" >/dev/null; then
        if [ "$sistema" = win64 ]; then
            echo "erro: falta $ferramenta no MSYS2; instale com: pacman -S --needed git make nasm diffutils mingw-w64-x86_64-gcc mingw-w64-x86_64-pkgconf mingw-w64-x86_64-ffnvcodec-headers mingw-w64-x86_64-amf-headers mingw-w64-x86_64-libvpl" >&2
        else
            echo "erro: falta $ferramenta (build-essential, git, nasm, pkg-config)" >&2
        fi
        exit 1
    fi
done

if [ -z "${FFMPEG_TAG:-}" ] || [ -z "${X264_REV:-}" ]; then
    FFMPEG_TAG=$(git ls-remote --tags --refs https://git.ffmpeg.org/ffmpeg.git 'n*' 2>/dev/null \
        | awk -F/ '{print $3}' | grep -E '^n[0-9]+\.[0-9]+(\.[0-9]+)?$' | sort -V | tail -n 1 || true)
    X264_REV=$(git ls-remote https://code.videolan.org/videolan/x264.git refs/heads/stable 2>/dev/null | cut -f1 || true)
    if [ -z "$FFMPEG_TAG" ] || [ -z "$X264_REV" ]; then
        if anterior=$(ultimo_compilado); then
            echo "aviso: sem acesso aos repositórios oficiais; usando $anterior" >&2
            echo "$anterior"
            exit 0
        fi
        echo "erro: sem acesso aos repositórios oficiais e sem FFmpeg compilado em $cache" >&2
        exit 1
    fi
fi

destino=$cache/ffmpeg-$sistema-$FFMPEG_TAG-x264-${X264_REV:0:12}-$receita
if [ -x "$destino/bin/$exe" ]; then
    echo "FFmpeg $FFMPEG_TAG já compilado: $destino" >&2
    echo "$destino"
    exit 0
fi

echo "Compilando FFmpeg $FFMPEG_TAG (git.ffmpeg.org) com x264 ${X264_REV:0:12} (code.videolan.org)" >&2
temp=$(mktemp -d "$cache/build.XXXXXX")
trap 'rm -rf "$temp"' EXIT
prefixo=$temp/prefixo

git clone -q --depth 1 --branch stable https://code.videolan.org/videolan/x264.git "$temp/x264"
if [ "$(git -C "$temp/x264" rev-parse HEAD)" != "$X264_REV" ]; then
    echo "erro: o stable do x264 mudou durante o build; rode de novo" >&2
    exit 1
fi
(
    cd "$temp/x264"
    ./configure --prefix="$prefixo" --enable-static --disable-cli --enable-pic
    make -j"$jobs"
    make install
) >&2

git clone -q --depth 1 --branch "$FFMPEG_TAG" https://git.ffmpeg.org/ffmpeg.git "$temp/ffmpeg"
opcoes=(
    --prefix="$prefixo" --pkg-config-flags=--static
    --enable-gpl --enable-libx264
    --disable-debug --disable-doc --disable-ffplay --disable-ffprobe
)
if [ "$sistema" = win64 ]; then
    opcoes+=(--extra-ldflags=-static --enable-ffnvcodec --enable-nvenc --enable-amf --enable-libvpl)
else
    opcoes+=(--disable-autodetect --enable-libxcb --enable-libxcb-shm --enable-libxcb-xfixes --enable-libxcb-shape)
fi
(
    cd "$temp/ffmpeg"
    PKG_CONFIG_PATH="$prefixo/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}" ./configure "${opcoes[@]}"
    make -j"$jobs"
    make install
) >&2

codificadores=$("$prefixo/bin/$exe" -hide_banner -encoders)
esperados=(libx264)
[ "$sistema" = win64 ] && esperados+=(h264_nvenc h264_amf h264_qsv)
for c in "${esperados[@]}"; do
    grep -q " $c " <<< "$codificadores" || { echo "erro: FFmpeg sem $c" >&2; exit 1; }
done
if [ "$sistema" = linux64 ]; then
    "$prefixo/bin/$exe" -hide_banner -devices | grep -q x11grab || { echo "erro: FFmpeg sem x11grab" >&2; exit 1; }
fi

novo=$temp/pronto
mkdir -p "$novo/bin"
cp "$prefixo/bin/$exe" "$novo/bin/"
cp "$temp/ffmpeg/COPYING.GPLv2" "$novo/LICENSE.txt"
echo "FFmpeg $FFMPEG_TAG (git.ffmpeg.org), x264 $X264_REV (code.videolan.org)" > "$novo/VERSION.txt"
mv "$novo" "$destino"
for d in "$cache"/ffmpeg-"$sistema"-*; do
    [ "$d" = "$destino" ] || rm -rf "$d"
done
echo "$destino"
