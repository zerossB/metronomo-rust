FROM docker.io/library/ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential ca-certificates debhelper dpkg-dev pkg-config python3 git \
    desktop-file-utils appstream librsvg2-bin libasound2-dev libfontconfig1-dev libfreetype-dev \
    libxkbcommon-dev libxkbcommon-x11-dev libx11-xcb-dev libwayland-dev \
    && rm -rf /var/lib/apt/lists/*

# Mount the checkout and the pinned host Rust toolchain/cache at runtime.
# See docs/release.md for the complete, offline-Cargo build command.
