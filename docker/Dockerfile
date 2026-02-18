FROM rust:1.93-bookworm AS builder

WORKDIR /src

COPY . .

RUN cargo build --release --bin cortex

FROM debian:12.9-slim

ARG S6_OVERLAY_VERSION=3.2.1.0

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
        curl \
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

COPY --from=builder /src/target/release/cortex /usr/local/bin/cortex

COPY docker/s6/cont-init.d/ /etc/cont-init.d/
COPY docker/s6/services.d/ /etc/services.d/
COPY docker/scripts/healthcheck-workers.sh /usr/local/bin/healthcheck-workers.sh

RUN groupadd -r axon && useradd -r -g axon -u 1001 axon

RUN chmod +x /etc/cont-init.d/* \
    && find /etc/services.d -type f -name run -exec chmod +x {} + \
    && chmod +x /usr/local/bin/healthcheck-workers.sh \
    && mkdir -p \
        /var/log/axon/crawl-worker \
        /var/log/axon/batch-worker \
        /var/log/axon/embed-worker \
        /var/log/axon/extract-worker \
    && chown -R axon:axon /var/log/axon /app

# NOTE: s6-overlay v3 requires root for /init (PID 1) to manage process
# supervision. Workers drop privileges via s6-setuidgid in their run scripts.
# Do NOT add `USER axon` here — it would break s6-overlay's /init entrypoint.

ENV AXON_ENV_FILE=/app/.env

HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD /usr/local/bin/healthcheck-workers.sh || exit 1

ENTRYPOINT ["/init"]
