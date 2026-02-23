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

verify:
    just fmt-check
    just clippy
    just check
    just test

ci:
    just verify

precommit:
    python3 scripts/enforce_no_legacy_symbols.py
    python3 scripts/enforce_monoliths.py --staged
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
