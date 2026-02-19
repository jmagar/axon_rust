#!/usr/bin/env python3
"""Fail when removed legacy symbols are reintroduced outside approved tests."""

from __future__ import annotations

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]

BANNED_SYMBOLS = (
    "ops_legacy",
    "crawl_jobs_legacy",
    "AXON_VECTOR_IMPL",
    "AXON_CRAWL_JOBS_IMPL",
)

# Guard tests are allowed to contain banned symbol strings as assertions.
ALLOWLIST_PATHS = {
    "tests/vector_v2_no_legacy_calls.rs",
    "tests/crawl_jobs_v2_migration.rs",
    "scripts/enforce_no_legacy_symbols.py",
}

SKIP_DIRS = {
    ".git",
    "target",
    "node_modules",
    ".cache",
}

SCANNED_ROOTS = (
    "crates",
    "tests",
    "scripts",
    ".github",
)

SCANNED_SUFFIXES = (
    ".rs",
    ".py",
    ".sh",
    ".toml",
    ".yml",
    ".yaml",
)


def iter_files() -> list[Path]:
    files: list[Path] = []
    for root in SCANNED_ROOTS:
        base = REPO_ROOT / root
        if not base.exists():
            continue
        for path in base.rglob("*"):
            if not path.is_file():
                continue
            rel = path.relative_to(REPO_ROOT)
            if any(part in SKIP_DIRS for part in rel.parts):
                continue
            if path.suffix.lower() not in SCANNED_SUFFIXES:
                continue
            files.append(path)
    return files


def main() -> int:
    violations: list[str] = []
    for path in iter_files():
        rel = path.relative_to(REPO_ROOT).as_posix()
        if rel in ALLOWLIST_PATHS:
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        for symbol in BANNED_SYMBOLS:
            if symbol in text:
                violations.append(f"{rel}: contains banned symbol '{symbol}'")

    if violations:
        print("Legacy symbol deny-check failed:")
        for item in sorted(violations):
            print(f"  - {item}")
        return 1

    print("Legacy symbol deny-check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
