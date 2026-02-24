# syntax=docker/dockerfile:1
#
# Three-stage build using cargo-chef for dependency layer caching.
#
# Stage 1 (planner): generates a recipe.json from Cargo.toml + lock files only.
# Stage 2 (builder): compiles deps from recipe first (cached layer), then source.
# Stage 3 (runtime): debian-slim + s6-overlay; only the axon binary is copied in.
#
# Build context is the axon_rust directory.
# spider_agent and its transitive path deps are injected via the "spider-agent"
# BuildKit additional_context (declared in docker-compose.yaml) so the path dep
# ../spider/spider_agent resolves correctly without widening the build context
# to the whole workspace.

# ── Stage 1: planner ──────────────────────────────────────────────────────────
FROM rust:1.93-bookworm AS planner

WORKDIR /src

RUN cargo install cargo-chef --locked

COPY . ./axon_rust/
COPY --from=spider-agent . ./spider/spider_agent/
COPY --from=spider-agent-types . ./spider/spider_agent_types/
COPY --from=spider-agent-html . ./spider/spider_agent_html/

WORKDIR /src/axon_rust

RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 2: builder ──────────────────────────────────────────────────────────
FROM rust:1.93-bookworm AS builder

WORKDIR /src

RUN cargo install cargo-chef --locked

# Cook dependencies first — this layer is cached as long as recipe.json is
# unchanged (i.e., Cargo.toml / Cargo.lock have not changed).
COPY --from=planner /src/axon_rust/recipe.json recipe.json
COPY --from=spider-agent . ./spider/spider_agent/
COPY --from=spider-agent-types . ./spider/spider_agent_types/
COPY --from=spider-agent-html . ./spider/spider_agent_html/

WORKDIR /src/axon_rust

RUN cargo chef cook --release --recipe-path /src/recipe.json

# Now copy the actual source. Only this layer re-runs on source changes.
WORKDIR /src

COPY . ./axon_rust/

WORKDIR /src/axon_rust

RUN cargo build --release --bin axon

# ── Stage 3: runtime ──────────────────────────────────────────────────────────
FROM debian:12.9-slim

ARG S6_OVERLAY_VERSION=3.2.1.0

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
        curl \
        git \
        libssl3 \
        xz-utils \
    && rm -rf /var/lib/apt/lists/*

RUN set -eux; \
    arch="$(dpkg --print-architecture)"; \
    case "$arch" in \
      amd64) s6_arch="x86_64" ;; \
      arm64) s6_arch="aarch64" ;; \
      *) echo "Unsupported arch: $arch"; exit 1 ;; \
    esac; \
    curl -fsSL -o /tmp/s6-overlay-noarch.tar.xz \
      "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-noarch.tar.xz"; \
    curl -fsSL -o /tmp/s6-overlay-noarch.tar.xz.sha256 \
      "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-noarch.tar.xz.sha256"; \
    curl -fsSL -o "/tmp/s6-overlay-${s6_arch}.tar.xz" \
      "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-${s6_arch}.tar.xz"; \
    curl -fsSL -o "/tmp/s6-overlay-${s6_arch}.tar.xz.sha256" \
      "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-${s6_arch}.tar.xz.sha256"; \
    cd /tmp && sha256sum -c s6-overlay-noarch.tar.xz.sha256; \
    cd /tmp && sha256sum -c "s6-overlay-${s6_arch}.tar.xz.sha256"; \
    tar -C / -Jxpf /tmp/s6-overlay-noarch.tar.xz; \
    tar -C / -Jxpf "/tmp/s6-overlay-${s6_arch}.tar.xz"; \
    rm -f "/tmp/s6-overlay-noarch.tar.xz" "/tmp/s6-overlay-noarch.tar.xz.sha256" \
          "/tmp/s6-overlay-${s6_arch}.tar.xz" "/tmp/s6-overlay-${s6_arch}.tar.xz.sha256"

WORKDIR /app

# Install yt-dlp standalone binary (no Python required) for YouTube transcript ingestion
ARG YT_DLP_VERSION=2024.12.23
RUN set -eux; \
    arch="$(dpkg --print-architecture)"; \
    case "$arch" in \
      amd64) ytdlp_bin="yt-dlp_linux" ;; \
      arm64) ytdlp_bin="yt-dlp_linux_aarch64" ;; \
      *) echo "Unsupported arch: $arch"; exit 1 ;; \
    esac; \
    curl -fsSL -o /usr/local/bin/yt-dlp \
      "https://github.com/yt-dlp/yt-dlp/releases/download/${YT_DLP_VERSION}/${ytdlp_bin}"; \
    chmod +x /usr/local/bin/yt-dlp

COPY --from=builder /src/axon_rust/target/release/axon /usr/local/bin/axon

COPY docker/s6/cont-init.d/ /etc/cont-init.d/
COPY docker/s6/s6-rc.d/ /etc/s6-overlay/s6-rc.d/
COPY docker/scripts/healthcheck-workers.sh /usr/local/bin/healthcheck-workers.sh

RUN groupadd -r axon && useradd -r -g axon -u 1001 -d /home/axon -m axon

RUN chmod +x /etc/cont-init.d/* \
    && find /etc/s6-overlay/s6-rc.d -type f -name run -exec chmod +x {} + \
    && find /etc/s6-overlay/s6-rc.d -type f -name finish -exec chmod +x {} + \
    && chmod +x /usr/local/bin/healthcheck-workers.sh \
    && mkdir -p \
        /var/log/axon/crawl-worker \
        /var/log/axon/batch-worker \
        /var/log/axon/embed-worker \
        /var/log/axon/extract-worker \
        /var/log/axon/ingest-worker \
        /app/logs \
    && chown -R axon:axon /var/log/axon /app

# NOTE: s6-overlay v3 requires root for /init (PID 1) to manage process
# supervision. Workers drop privileges via s6-setuidgid in their run scripts.
# Do NOT add `USER axon` here — it would break s6-overlay's /init entrypoint.

ENV AXON_ENV_FILE=/app/.env

HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD /usr/local/bin/healthcheck-workers.sh || exit 1

ENTRYPOINT ["/init"]
