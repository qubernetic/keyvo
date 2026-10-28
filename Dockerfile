# Development image for keyvo (ADR-0002, two-layer dev model).
# Everything that needs Rust or Node runs here; the host only runs the binary.
# The Rust version here and in rust-toolchain.toml are bumped together.
# The base image is pinned by its multi-arch index digest; a tag bump also
# moves the digest (CONTRIBUTING.md, "Bumping a pinned version").
FROM rust:1.98.0-slim-trixie@sha256:17d1ba895198f9934c6314ec5346a0d5115372f3243390c3d731e242f35c2f27

ARG UID=1000
ARG GID=1000
# Every downloaded artifact is checked against a sha256 committed here; a
# version bump updates the version and both per-architecture checksums.
ARG NODE_VERSION=24.21.0
ARG NODE_SHA256_X64=fd8e59d5a511510f6a298afb548f18c7d2b1be404d8b4a27d94fbe49f56cb2d6
ARG NODE_SHA256_ARM64=6ad1325edbdb5649c379b75a237147a666c95d4f9ae8d340fef2d1575d289ad2
ARG CARGO_BINSTALL_VERSION=1.25.3
ARG CARGO_BINSTALL_SHA256_X86_64=1f72931713e9091478538c6c78c3f11f205b4f6f394f35e61835036b23929d2a
ARG CARGO_BINSTALL_SHA256_AARCH64=19ef7a058397f83d6b2cc83727fb8d38babd8ad657210c921b7771a9e8b3a6fb
ARG JUST_VERSION=1.53.0
ARG CARGO_DENY_VERSION=0.20.2
ARG CARGO_LLVM_COV_VERSION=0.9.1
ARG CARGO_NEXTEST_VERSION=0.9.143
ARG CARGO_INSTA_VERSION=1.48.0

ENV DEBIAN_FRONTEND=noninteractive

# Debian packages come unpinned from the digest-pinned base image's trixie
# repository. webkit2gtk-4.1 and the rest of the list from build-essential
# on are the Tauri 2 build prerequisites for the desktop app (app/src-tauri);
# python3 with aiohttp, tomlkit and PyYAML runs the Flatpak source generators
# (tools/flatpak/generate.py, `just flatpak-sources`).
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        libssl-dev \
        libudev-dev \
        pkg-config \
        xz-utils \
        build-essential \
        file \
        libayatana-appindicator3-dev \
        librsvg2-dev \
        libwebkit2gtk-4.1-dev \
        libxdo-dev \
        python3 \
        python3-aiohttp \
        python3-tomlkit \
        python3-yaml \
    && rm -rf /var/lib/apt/lists/*

# Node LTS from the official tarball (no distro package, no nvm), verified
# before it is extracted.
RUN set -eu; \
    case "$(uname -m)" in \
        x86_64) arch=x64; sha="${NODE_SHA256_X64}" ;; \
        aarch64) arch=arm64; sha="${NODE_SHA256_ARM64}" ;; \
        *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;; \
    esac; \
    tarball="node-v${NODE_VERSION}-linux-${arch}.tar.xz"; \
    curl -fsSL --proto '=https' --tlsv1.2 -o "/tmp/${tarball}" \
        "https://nodejs.org/dist/v${NODE_VERSION}/${tarball}"; \
    echo "${sha}  /tmp/${tarball}" | sha256sum -c -; \
    tar -xJf "/tmp/${tarball}" -C /usr/local --strip-components=1 \
        --exclude='*/CHANGELOG.md' --exclude='*/README.md' --exclude='*/LICENSE'; \
    rm -f "/tmp/${tarball}"; \
    node --version; \
    npm --version

# Toolchain components the workspace expects (rust-toolchain.toml).
RUN rustup component add rustfmt clippy llvm-tools-preview

# cargo-binstall from its pinned release archive, verified before it is
# extracted. No install script, nothing from a branch ref.
RUN set -eu; \
    case "$(uname -m)" in \
        x86_64) sha="${CARGO_BINSTALL_SHA256_X86_64}" ;; \
        aarch64) sha="${CARGO_BINSTALL_SHA256_AARCH64}" ;; \
        *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;; \
    esac; \
    archive="cargo-binstall-$(uname -m)-unknown-linux-musl.tgz"; \
    curl -fsSL --proto '=https' --tlsv1.2 -o "/tmp/${archive}" \
        "https://github.com/cargo-bins/cargo-binstall/releases/download/v${CARGO_BINSTALL_VERSION}/${archive}"; \
    echo "${sha}  /tmp/${archive}" | sha256sum -c -; \
    tar -xzf "/tmp/${archive}" -C /usr/local/cargo/bin cargo-binstall; \
    rm -f "/tmp/${archive}"; \
    cargo binstall -V

# Cargo tools as prebuilt binaries from their own release assets; versions are
# pinned. The third-party quick-install mirror and source builds are disabled.
RUN cargo binstall --no-confirm --locked \
        --disable-strategies quick-install,compile \
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
