#!/usr/bin/env bash
set -euo pipefail

echo "[ask-quality] Running regression fixtures and policy tests..."

cargo test -q ask_quality_regression_fixtures_five_queries
cargo test -q normalize_ask_answer_dedupes_sources_by_url
cargo test -q normalize_ask_answer_formats_insufficient_evidence_when_uncited
cargo test -q normalize_ask_answer_formats_insufficient_evidence_when_flagged_in_body
cargo test -q procedural_query_requires_official_docs_citation
cargo test -q config_schema_query_requires_exact_page_citation
cargo test -q non_trivial_answer_requires_minimum_citation_count
cargo test -q authoritative_allowlist_matches_exact_and_suffix_hosts

echo "[ask-quality] All regression checks passed."
