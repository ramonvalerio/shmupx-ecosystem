# Ambiente de build do ShmupX Client para Linux (.deb e AppImage). Ubuntu 22.04: glibc 2.35, o
# pacote roda no Ubuntu 22.04+ / Debian 12+ / Mint 21+ / Fedora 36+.
# Uso: scripts/bundle-linux.ps1 (monta o repositório em /src e grava os pacotes em /out).
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential curl wget file ca-certificates git rsync xz-utils pkg-config \
        libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev \
        libasound2-dev libudev-dev patchelf \
        python3 python3-venv python3-dev \
    && rm -rf /var/lib/apt/lists/*

# Node 22
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs && rm -rf /var/lib/apt/lists/*

# Rust (edition 2024)
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.90.0

# FFmpeg estático (GPL, com libx264 e x11grab) levado dentro do pacote.
RUN mkdir -p /opt/ffmpeg && curl -fsSL https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz \
        | tar -xJ -C /opt/ffmpeg --strip-components=1

# O linuxdeploy do AppImage é um AppImage: sem FUSE dentro do contêiner, ele se extrai e roda.
ENV APPIMAGE_EXTRACT_AND_RUN=1

WORKDIR /work
