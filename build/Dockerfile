FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive \
    LANG=C.UTF-8

RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential \
      curl \
      wget \
      file \
      git \
      pkg-config \
      ca-certificates \
      libssl-dev \
      libwebkit2gtk-4.1-dev \
      libayatana-appindicator3-dev \
      librsvg2-dev \
      libxdo-dev \
      libasound2-dev \
      libpulse-dev \
      libx11-dev \
      libxi-dev \
      libxtst-dev \
      libxrandr-dev \
      libgtk-3-dev \
      cmake \
      clang \
      libclang-dev \
      patchelf \
      xdg-utils \
      dpkg-dev \
      fakeroot \
      rpm \
      squashfs-tools \
      xvfb \
      x11-utils \
      xdotool \
      imagemagick \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --no-modify-path --profile minimal --default-toolchain stable \
    && chmod -R a+w /usr/local/rustup /usr/local/cargo \
    && rustc --version \
    && cargo --version

RUN cargo install tauri-cli --version "^2" --locked

WORKDIR /src
