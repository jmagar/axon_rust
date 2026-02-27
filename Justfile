set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

check:
    cargo check -q --locked

test:
    cargo test -q --locked

test-fast:
    cargo test -q --lib --locked

test-all:
    cargo test --all-targets --all-features --locked

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets --locked -- -D warnings

build:
    cargo build --release --locked

install:
    cargo build --release --locked
    mkdir -p ~/.local/bin
    ln -sf "$(pwd)/target/release/axon" ~/.local/bin/axon

lint-all:
    just fmt-check
    just clippy
    cd apps/web && pnpm lint

verify:
    just fmt-check
    just clippy
    just check
    just test

ci:
    just verify

precommit:
    python3 scripts/enforce_no_legacy_symbols.py
    if [ -f "$HOME/.claude/hooks/enforce_monoliths.py" ]; then python3 "$HOME/.claude/hooks/enforce_monoliths.py" --staged; elif [ -f "scripts/enforce_monoliths.py" ]; then python3 scripts/enforce_monoliths.py --staged; else echo "ERROR: enforce_monoliths.py not found" && exit 1; fi
    just fmt-check
    just clippy
    just check
    just test

fix:
    cargo fmt --all
    cargo clippy --fix --all-targets --locked --allow-dirty --allow-staged

fix-all:
    just fix
    cd apps/web && pnpm format

clean:
    cargo clean

docker-build tag="axon:local":
    docker build -f docker/Dockerfile -t {{tag}} .

up:
    docker compose up -d --build

down:
    docker compose down

docker-up:
    docker compose up -d --build

docker-down:
    docker compose down

watch-check:
    cargo watch -x 'check -q --locked' -x 'test -q --lib --locked'

rebuild:
    just check
    just test
    just docker-build

# ── Web UI (axum built-in server) ─────────────────────────────────

serve port="3939":
    cargo run --bin axon -- serve --port {{port}}

serve-release port="3939":
    cargo run --release --bin axon -- serve --port {{port}}

# ── Web UI (Next.js dashboard) ────────────────────────────────────

web-dev:
    cd apps/web && pnpm dev

web-build:
    cd apps/web && pnpm build

web-lint:
    cd apps/web && pnpm lint

web-format:
    cd apps/web && pnpm format

# ── Full stack ────────────────────────────────────────────────────

# Kill any running axon serve or Next.js dev processes
stop:
    -pkill -f 'axon.*serve' 2>/dev/null || true
    -pkill -f 'next dev' 2>/dev/null || true
    @echo "Stopped running servers"

# Start infra, axum server, and Next.js dev server (all foreground)
dev:
    just stop
    docker compose up -d
    cargo run --bin axon -- serve --port 3939 &
    cd apps/web && pnpm dev &
    wait
