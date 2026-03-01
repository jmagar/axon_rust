#!/usr/bin/env bash
set -euo pipefail

echo "[ask-quality] Running regression fixtures and policy tests..."

cargo test -q --locked ask_quality_regression_fixtures_five_queries
cargo test -q --locked normalize_ask_answer_dedupes_sources_by_url
cargo test -q --locked normalize_ask_answer_formats_insufficient_evidence_when_uncited
cargo test -q --locked normalize_ask_answer_formats_insufficient_evidence_when_flagged_in_body
cargo test -q --locked procedural_query_requires_official_docs_citation
cargo test -q --locked config_schema_query_requires_exact_page_citation
cargo test -q --locked non_trivial_answer_requires_minimum_citation_count
cargo test -q --locked authoritative_allowlist_matches_exact_and_suffix_hosts

echo "[ask-quality] All regression checks passed."
