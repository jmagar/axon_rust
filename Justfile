set shell := ["bash", "-euo", "pipefail", "-c"]
rust_dev_env := "if command -v sccache >/dev/null 2>&1; then export RUSTC_WRAPPER=sccache; fi; if command -v mold >/dev/null 2>&1; then export RUSTFLAGS=\"${RUSTFLAGS:-} -C link-arg=-fuse-ld=mold\"; fi"

default:
    @just --list

check:
    {{rust_dev_env}}; cargo check -q --locked

check-tests:
    {{rust_dev_env}}; cargo check -q --tests --locked

test:
    if cargo nextest --version >/dev/null 2>&1; then {{rust_dev_env}}; cargo nextest run --locked --workspace -E 'not test(/worker_e2e/)'; else echo "cargo-nextest not installed; falling back to cargo test"; {{rust_dev_env}}; cargo test -q --locked -- --skip worker_e2e; fi

test-fast:
    if cargo nextest --version >/dev/null 2>&1; then {{rust_dev_env}}; cargo nextest run --locked --lib -E 'not test(/worker_e2e/)'; else {{rust_dev_env}}; cargo test -q --lib --locked -- --skip worker_e2e; fi

test-infra:
    {{rust_dev_env}}; cargo test --locked worker_e2e -- --ignored --nocapture

mcp-smoke:
    ./scripts/test-mcp-tools-mcporter.sh

test-all:
    {{rust_dev_env}}; cargo test --all-targets --all-features --locked

nextest-install:
    {{rust_dev_env}}; cargo install cargo-nextest --locked

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    {{rust_dev_env}}; cargo clippy --all-targets --locked -- -D warnings

build:
    {{rust_dev_env}}; cargo build --release --locked

install:
    {{rust_dev_env}}; cargo build --release --locked
    mkdir -p ~/.local/bin
    ln -sf "$(pwd)/target/release/axon" ~/.local/bin/axon

lint-all:
    just fmt-check
    just clippy
    cd apps/web && pnpm lint

verify:
    ./scripts/check_dockerignore_guards.sh
    just fmt-check
    just clippy
    just check
    just test

ci:
    just verify

precommit:
    python3 scripts/enforce_no_legacy_symbols.py
    ./scripts/check_dockerignore_guards.sh
    if [ -f "$HOME/.claude/hooks/enforce_monoliths.py" ]; then python3 "$HOME/.claude/hooks/enforce_monoliths.py" --staged; elif [ -f "scripts/enforce_monoliths.py" ]; then python3 scripts/enforce_monoliths.py --staged; else echo "ERROR: enforce_monoliths.py not found" && exit 1; fi
    just fmt-check
    just clippy
    just check
    just test

fix:
    cargo fmt --all
    {{rust_dev_env}}; cargo clippy --fix --all-targets --locked --allow-dirty --allow-staged

fix-all:
    just fix
    cd apps/web && pnpm format

llvm-cov-install:
    {{rust_dev_env}}; cargo install cargo-llvm-cov --locked

coverage-branch:
    if cargo llvm-cov --version >/dev/null 2>&1; then {{rust_dev_env}}; cargo llvm-cov --locked --workspace --all-features --lcov --output-path .cache/coverage/lcov.info; else echo "cargo-llvm-cov not installed. Run: just llvm-cov-install"; exit 1; fi

# ── Codegen ───────────────────────────────────────────────────

gen-mcp-schema *ARGS:
    python3 scripts/generate_mcp_schema_doc.py {{ARGS}}

clean:
    cargo clean

docker-build tag="axon:local":
    docker build -f docker/Dockerfile -t {{tag}} .

up:
    ./scripts/rebuild-fresh.sh

down:
    docker compose down

docker-up:
    ./scripts/rebuild-fresh.sh

docker-down:
    docker compose down

rebuild-fresh:
    ./scripts/rebuild-fresh.sh

cache-status:
    ./scripts/cache-guard.sh status

cache-prune:
    ./scripts/cache-guard.sh prune

docker-context-probe:
    ./scripts/check_docker_context_size.sh

check-container-revisions:
    ./scripts/check-container-revisions.sh

watch-check:
    cargo watch -x 'check -q --locked' -x 'check -q --tests --locked' -x 'test -q --lib --locked -- --skip worker_e2e'

rebuild:
    just check
    just test
    just docker-build

# ── Web UI (axum built-in server) ─────────────────────────────────

serve port="3939":
    {{rust_dev_env}}; cargo run --locked --bin axon -- serve --port {{port}}

serve-release port="3939":
    {{rust_dev_env}}; cargo run --release --locked --bin axon -- serve --port {{port}}

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
    {{rust_dev_env}}; cargo run --locked --bin axon -- serve --port 3939 &
    cd apps/web && pnpm dev &
    wait
