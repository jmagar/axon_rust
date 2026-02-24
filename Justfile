set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

check:
    cargo check -q

test:
    cargo test -q

test-fast:
    cargo test -q --lib

test-all:
    cargo test --all-targets --all-features

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets -- -D warnings

build:
    cargo build --release

install:
    cargo build --release
    ln -sf "$(pwd)/target/release/axon" ~/.local/bin/axon

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
    cargo clippy --fix --all-targets --allow-dirty --allow-staged

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
    cargo watch -x 'check -q' -x 'test -q --lib'

rebuild:
    just check
    just test
    just docker-build
