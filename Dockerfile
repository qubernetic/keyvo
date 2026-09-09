# Development image for keyvo (ADR-0002, two-layer dev model).
# Everything that needs Rust or Node runs here; the host only runs the binary.
# The Rust version here and in rust-toolchain.toml are bumped together.
FROM rust:1.98.0-slim-trixie

ARG UID=1000
ARG GID=1000
ARG NODE_VERSION=24.21.0
ARG JUST_VERSION=1.53.0
ARG CARGO_DENY_VERSION=0.20.2
ARG CARGO_LLVM_COV_VERSION=0.9.1
ARG CARGO_NEXTEST_VERSION=0.9.143
ARG CARGO_INSTA_VERSION=1.48.0

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        libssl-dev \
        libudev-dev \
        pkg-config \
        xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Node LTS from the official tarball (no distro package, no nvm).
RUN set -eu; \
    case "$(uname -m)" in \
        x86_64) arch=x64 ;; \
        aarch64) arch=arm64 ;; \
        *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;; \
    esac; \
    curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${arch}.tar.xz" \
        | tar -xJ -C /usr/local --strip-components=1 --exclude='*/CHANGELOG.md' --exclude='*/README.md' --exclude='*/LICENSE'; \
    node --version; \
    npm --version

# Toolchain components the workspace expects (rust-toolchain.toml).
RUN rustup component add rustfmt clippy llvm-tools-preview

# Cargo tools as prebuilt binaries via cargo-binstall; versions are pinned.
RUN curl -fsSL --proto '=https' --tlsv1.2 \
        https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh \
        | bash \
    && cargo binstall --no-confirm --locked \
        "just@${JUST_VERSION}" \
        "cargo-deny@${CARGO_DENY_VERSION}" \
        "cargo-llvm-cov@${CARGO_LLVM_COV_VERSION}" \
        "cargo-nextest@${CARGO_NEXTEST_VERSION}" \
        "cargo-insta@${CARGO_INSTA_VERSION}" \
    && rm -rf /usr/local/cargo/registry/* /usr/local/cargo/git/*

# Non-root user matching the host UID/GID so bind-mounted files keep their owner.
RUN set -eu; \
    if ! getent group "${GID}" >/dev/null; then groupadd -g "${GID}" dev; fi; \
    useradd -m -u "${UID}" -g "${GID}" -s /bin/bash dev; \
    mkdir -p /usr/local/cargo/registry /usr/local/cargo/git /workspace; \
    chown -R "${UID}:${GID}" /usr/local/cargo /usr/local/rustup /workspace; \
    git config --system --add safe.directory /workspace

USER dev
WORKDIR /workspace
