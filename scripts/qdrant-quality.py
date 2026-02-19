#!/usr/bin/env python3
"""Standalone Qdrant collection quality checker.

Usage:
  python3 scripts/qdrant-quality.py health
  python3 scripts/qdrant-quality.py check
  python3 scripts/qdrant-quality.py check --collection cortex
  python3 scripts/qdrant-quality.py check-all
  python3 scripts/qdrant-quality.py delete-duplicates --collection cortex
  python3 scripts/qdrant-quality.py delete-excluded --collection cortex
"""

from __future__ import annotations

import argparse
import textwrap
import json
import os
import re
import socket
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from collections import defaultdict
from dataclasses import dataclass
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

COLORS_ENABLED = os.getenv("AXON_NO_COLOR") is None and os.getenv("CORTEX_NO_COLOR") is None


def _style(text: str, *, fg_256: int | None = None, bold: bool = False, dim: bool = False) -> str:
    if not COLORS_ENABLED:
        return text
    codes: list[str] = []
    if bold:
        codes.append("1")
    if dim:
        codes.append("2")
    if fg_256 is not None:
        codes.append(f"38;5;{fg_256}")
    if not codes:
        return text
    return f"\x1b[{';'.join(codes)}m{text}\x1b[0m"


def primary(text: str) -> str:
    # Match crates/core/ui.rs primary(): color256(211) + bold
    return _style(text, fg_256=211, bold=True)


def accent(text: str) -> str:
    # Match crates/core/ui.rs accent(): color256(153)
    return _style(text, fg_256=153)


def muted(text: str) -> str:
    return _style(text, dim=True)


def status_text(status: str) -> str:
    s = (status or "").lower()
    if s in {"completed", "green", "ok"}:
        return _style(status, fg_256=2)
    if s in {"failed", "error", "red"}:
        return _style(status, fg_256=1)
    if s in {"pending", "running", "processing", "scraping", "yellow"}:
        return _style(status, fg_256=3)
    return _style(status, fg_256=6)


def render_help_text() -> str:
    return "\n".join(
        [
            primary("Qdrant Quality"),
            primary("Usage"),
            f"  {accent('python3 scripts/qdrant-quality.py')} {muted('<command> [options]')}",
            "",
            primary("Commands"),
            f"  {accent('help')}                   {muted('Show this help')}",
            f"  {accent('health')}                 {muted('Show cluster and collection health stats')}",
            f"  {accent('aliases')}                {muted('Audit aliases and flag dangling targets')}",
            f"  {accent('payload-schema')}         {muted('Audit payload field presence and types')}",
            f"  {accent('domain-breakdown')}       {muted('Show top domains by points and duplicate rates')}",
            f"  {accent('stale-data')}             {muted('Check age of points using timestamps')}",
            f"  {accent('strict-exclude-sync')}    {muted('Compare script/Rust default exclude prefixes')}",
            f"  {accent('check')}                  {muted('Audit one collection (quality, duplicates, exclusions)')}",
            f"  {accent('check-all')}              {muted('Audit all collections')}",
            f"  {accent('delete-duplicates')}      {muted('Delete duplicate points in one collection')}",
            f"  {accent('delete-excluded')}        {muted('Delete points matching exclude path prefixes in one collection')}",
            f"  {accent('delete-duplicates-all')}  {muted('Delete duplicate points in all collections')}",
            f"  {accent('delete-excluded-all')}    {muted('Delete exclude-path matches in all collections')}",
            "",
            primary("Global Options"),
            f"  {accent('--url <url>')}                    {muted('Qdrant base URL (default: env/.env QDRANT_URL)')}",
            f'  {accent("--exclude-path-prefix <value>")}  {muted("Repeat or comma-separate; use \'none\' to disable defaults")}',
            f"  {accent('--dry-run')}                      {muted('Preview delete actions without deleting')}",
            f"  {accent('--json')}                         {muted('Emit machine-readable JSON output')}",
            f"  {accent('--sample <n>')}                   {muted('Limit scan to first N points for quick checks')}",
            f"  {accent('--yes')}                          {muted('Skip confirmation prompt for destructive commands')}",
            f"  {accent('--help')}                         {muted('Show this help')}",
            "",
            primary("Examples"),
            f"  {accent('python3 scripts/qdrant-quality.py health')}",
            f"  {accent('python3 scripts/qdrant-quality.py check --collection cortex')}",
            f"  {accent('python3 scripts/qdrant-quality.py check-all')}",
            f"  {accent('python3 scripts/qdrant-quality.py delete-duplicates --collection cortex')}",
            f"  {accent('python3 scripts/qdrant-quality.py delete-excluded --collection cortex')}",
            f"  {accent('python3 scripts/qdrant-quality.py check --exclude-path-prefix /fr,/de')}",
            f"  {accent('python3 scripts/qdrant-quality.py check --exclude-path-prefix none')}",
            f"  {accent('python3 scripts/qdrant-quality.py aliases --json')}",
            f"  {accent('python3 scripts/qdrant-quality.py delete-duplicates --collection cortex --dry-run')}",
            f"  {accent('python3 scripts/qdrant-quality.py domain-breakdown --collection firecrawl --top 20 --sample 50000')}",
            f"  {accent('python3 scripts/qdrant-quality.py stale-data --collection cortex --days 30 --json')}",
        ]
    )


def load_dotenv_file() -> dict[str, str]:
    """Parse repository .env file into key/value pairs."""
    env_path = Path(__file__).resolve().parents[1] / ".env"
    values: dict[str, str] = {}
    if not env_path.exists():
        return values

    for raw_line in env_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip()
        if not key:
            continue
        if value and ((value[0] == value[-1]) and value[0] in {"'", '"'}):
            value = value[1:-1]
        values[key] = value

    return values


DOTENV_VALUES = load_dotenv_file()
QDRANT_URL = os.getenv("QDRANT_URL", DOTENV_VALUES.get("QDRANT_URL", "http://localhost:53333")).rstrip("/")
DEFAULT_COLLECTION = os.getenv("QDRANT_COLLECTION", DOTENV_VALUES.get("QDRANT_COLLECTION", "cortex"))

# Must stay aligned with crates/core/config.rs::default_exclude_prefixes().
DEFAULT_EXCLUDE_PREFIXES = [
    "/fr",
    "/de",
    "/es",
    "/ja",
    "/zh",
    "/zh-cn",
    "/zh-tw",
    "/ko",
    "/pt",
    "/pt-br",
    "/it",
    "/nl",
    "/pl",
    "/ru",
    "/tr",
    "/ar",
    "/id",
    "/vi",
    "/th",
    "/cs",
    "/da",
    "/fi",
    "/no",
    "/sv",
    "/he",
    "/uk",
    "/ro",
    "/hu",
    "/el",
]


def running_in_container() -> bool:
    return os.path.exists("/.dockerenv")


def hostname_resolves(hostname: str) -> bool:
    try:
        socket.getaddrinfo(hostname, None)
        return True
    except socket.gaierror:
        return False


def endpoint_reachable(base_url: str) -> bool:
    try:
        req = urllib.request.Request(f"{base_url.rstrip('/')}/", method="GET")
        with urllib.request.urlopen(req, timeout=2):
            return True
    except Exception:
        return False


def resolve_runtime_qdrant_url(configured_url: str) -> str:
    if running_in_container():
        return configured_url

    parsed = urllib.parse.urlparse(configured_url)
    host = parsed.hostname
    if not host or host in {"localhost", "127.0.0.1"}:
        return configured_url

    if hostname_resolves(host):
        return configured_url

    candidates: list[str] = []
    if parsed.port == 6333:
        candidates.extend(["http://localhost:53333", "http://127.0.0.1:53333"])
    candidates.extend(["http://localhost:6333", "http://127.0.0.1:6333"])

    for candidate in candidates:
        if endpoint_reachable(candidate):
            return candidate

    return configured_url


@dataclass
class DuplicateGroup:
    url: str
    count: int
    ids: list[Any]


@dataclass
class DataQualityIssues:
    missing_url: int = 0
    missing_content: int = 0
    empty_content: int = 0
    missing_chunk_index: int = 0

    @property
    def total(self) -> int:
        return (
            self.missing_url
            + self.missing_content
            + self.empty_content
            + self.missing_chunk_index
        )


@dataclass
class ExcludeViolationStats:
    matched_points: int
    matched_urls: int
    matched_ids: list[Any]
    top_urls: list[tuple[str, int, str]]


@dataclass
class NormalizedExcludePrefixes:
    prefixes: list[str]
    disable_defaults: bool


def qdrant_request(path: str, method: str = "GET", body: dict[str, Any] | None = None, timeout: int = 30) -> dict[str, Any]:
    def should_retry_http(status: int) -> bool:
        return status == 429 or status >= 500

    url = f"{QDRANT_URL}{path}"
    payload = None
    headers = {"Content-Type": "application/json"}

    if body is not None:
        payload = json.dumps(body).encode("utf-8")

    req = urllib.request.Request(url=url, data=payload, headers=headers, method=method)
    retries = 3
    backoff_seconds = 0.25
    last_error: Exception | None = None

    for attempt in range(retries + 1):
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                raw = resp.read().decode("utf-8")
                return json.loads(raw) if raw else {}
        except urllib.error.HTTPError as exc:
            last_error = exc
            if attempt < retries and should_retry_http(exc.code):
                time.sleep(backoff_seconds * (attempt + 1))
                continue
            msg = exc.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"Qdrant request failed {exc.code} {exc.reason}: {msg}") from exc
        except urllib.error.URLError as exc:
            last_error = exc
            if attempt < retries:
                time.sleep(backoff_seconds * (attempt + 1))
                continue
            raise RuntimeError(f"Qdrant request failed: {exc.reason}") from exc

    raise RuntimeError(f"Qdrant request failed after retries: {last_error}")


def get_cluster_info() -> dict[str, Any]:
    data = qdrant_request("/", timeout=10)
    return {"version": data.get("version", "unknown"), "commit": data.get("commit")}


def list_collections() -> list[str]:
    data = qdrant_request("/collections", timeout=10)
    rows = data.get("result", {}).get("collections", [])
    return [row.get("name") for row in rows if isinstance(row, dict) and row.get("name")]


def list_aliases() -> list[dict[str, str]]:
    data = qdrant_request("/aliases", timeout=10)
    result = data.get("result")
    aliases_rows: list[Any]
    if isinstance(result, dict):
        aliases_rows = result.get("aliases", []) or []
    elif isinstance(result, list):
        aliases_rows = result
    else:
        aliases_rows = []

    aliases: list[dict[str, str]] = []
    for row in aliases_rows:
        if not isinstance(row, dict):
            continue
        alias_name = row.get("alias_name")
        collection_name = row.get("collection_name")
        if isinstance(alias_name, str) and isinstance(collection_name, str):
            aliases.append({"alias_name": alias_name, "collection_name": collection_name})
    return aliases


def canonicalize_url_for_dedupe(url: str) -> str:
    """Normalize URL for duplicate grouping, matching Rust crawler behavior."""
    parsed = urllib.parse.urlparse(url)

    # Fragments do not affect stored page content.
    fragmentless = parsed._replace(fragment="")

    # Normalize default ports.
    netloc = fragmentless.netloc
    hostname = fragmentless.hostname or ""
    port = fragmentless.port
    if (fragmentless.scheme == "http" and port == 80) or (
        fragmentless.scheme == "https" and port == 443
    ):
        userinfo = ""
        if "@" in netloc:
            userinfo = netloc.split("@", 1)[0] + "@"
        netloc = f"{userinfo}{hostname}"

    # Normalize trailing slash except for root.
    path = fragmentless.path or "/"
    if len(path) > 1:
        path = path.rstrip("/")
        if not path:
            path = "/"

    normalized = fragmentless._replace(netloc=netloc, path=path)
    return urllib.parse.urlunparse(normalized)


def extract_rust_default_excludes() -> list[str]:
    """Extract default exclude prefixes from crates/core/config.rs."""
    config_path = Path(__file__).resolve().parents[1] / "crates/core/config.rs"
    if not config_path.exists():
        return []

    text = config_path.read_text(encoding="utf-8")
    match = re.search(
        r"fn\s+default_exclude_prefixes\(\)\s*->\s*Vec<String>\s*\{\s*vec!\[(?P<body>.*?)\]\s*\.into_iter\(\)",
        text,
        re.DOTALL,
    )
    if not match:
        return []

    body = match.group("body")
    values = re.findall(r'"([^"]+)"', body)
    return sorted(set(values))


def get_collection_info(collection: str) -> dict[str, Any]:
    data = qdrant_request(f"/collections/{collection}", timeout=10)
    result = data.get("result")
    if not isinstance(result, dict):
        raise RuntimeError(f"No collection info returned for '{collection}'")
    return result


def fetch_all_points(
    collection: str,
    *,
    emit_output: bool = True,
    sample_limit: int | None = None,
) -> list[dict[str, Any]]:
    if emit_output:
        print(f"Fetching points from {QDRANT_URL}/collections/{collection}...", flush=True)
    points: list[dict[str, Any]] = []
    offset: Any | None = None

    while True:
        body: dict[str, Any] = {"limit": 100, "with_payload": True, "with_vector": False}
        if offset is not None:
            body["offset"] = offset

        data = qdrant_request(
            f"/collections/{collection}/points/scroll",
            method="POST",
            body=body,
            timeout=60,
        )

        result = data.get("result", {})
        batch = result.get("points") or []
        if not batch:
            break

        points.extend(batch)
        if sample_limit is not None and sample_limit > 0 and len(points) >= sample_limit:
            points = points[:sample_limit]
            if emit_output:
                sys.stderr.write(f"\rSample limit reached at {len(points)} points\n")
            break
        if emit_output:
            sys.stderr.write(f"\rFetched {len(points)} points...")
            sys.stderr.flush()

        next_offset = result.get("next_page_offset")
        if next_offset is None:
            break
        offset = next_offset

    if emit_output:
        sys.stderr.write(f"\rFetched {len(points)} points total\n")
    return points


def check_data_quality(points: list[dict[str, Any]]) -> DataQualityIssues:
    issues = DataQualityIssues()
    for point in points:
        payload = point.get("payload") or {}
        url = payload.get("url")
        chunk_text = payload.get("chunk_text")
        chunk_index = payload.get("chunk_index")

        if not url:
            issues.missing_url += 1

        if chunk_text is None:
            issues.missing_content += 1
        elif isinstance(chunk_text, str) and chunk_text.strip() == "":
            issues.empty_content += 1

        if chunk_index is None:
            issues.missing_chunk_index += 1
    return issues


def find_duplicates(points: list[dict[str, Any]]) -> list[DuplicateGroup]:
    grouped: dict[str, list[Any]] = defaultdict(list)

    for point in points:
        payload = point.get("payload") or {}
        url = payload.get("url")
        chunk_index = payload.get("chunk_index")
        point_id = point.get("id")
        if not url:
            continue
        canonical_url = canonicalize_url_for_dedupe(url)
        key = f"{canonical_url}:::{chunk_index if chunk_index is not None else 'none'}"
        grouped[key].append(point_id)

    out: list[DuplicateGroup] = []
    for key, ids in grouped.items():
        if len(ids) > 1:
            url = key.split(":::", 1)[0]
            out.append(DuplicateGroup(url=url, count=len(ids), ids=ids))

    out.sort(key=lambda x: x.count, reverse=True)
    return out


def parse_csv_values(raw: str) -> list[str]:
    return [part.strip() for part in raw.split(",")]


def normalize_exclude_prefixes(values: list[str]) -> NormalizedExcludePrefixes:
    disable_by_empty = len(values) == 1 and values[0].strip() in {"", "/"}
    disable_by_none = any(v.strip().lower() == "none" for v in values)
    if disable_by_none:
        return NormalizedExcludePrefixes(prefixes=[], disable_defaults=True)

    out: list[str] = []
    for raw in values:
        trimmed = raw.strip()
        if not trimmed or trimmed == "/":
            continue
        out.append(trimmed if trimmed.startswith("/") else f"/{trimmed}")

    out = sorted(set(out))
    return NormalizedExcludePrefixes(prefixes=out, disable_defaults=disable_by_empty)


def path_prefix_excluded(path: str, prefix: str) -> bool:
    normalized = prefix if prefix.startswith("/") else f"/{prefix}"
    boundary_prefix = normalized.rstrip("/")
    if not boundary_prefix:
        return False
    if path == boundary_prefix:
        return True
    if path.startswith(boundary_prefix):
        rest = path[len(boundary_prefix) :]
        return rest.startswith("/")
    return False


def is_excluded_url_path(url: str, prefixes: list[str]) -> tuple[bool, str | None]:
    if not prefixes:
        return (False, None)

    try:
        path = urllib.parse.urlparse(url).path or "/"
    except Exception:
        path = "/"

    for prefix in prefixes:
        if path_prefix_excluded(path, prefix):
            return (True, prefix)

    return (False, None)


def check_exclude_violations(points: list[dict[str, Any]], prefixes: list[str]) -> ExcludeViolationStats:
    if not prefixes:
        return ExcludeViolationStats(0, 0, [], [])

    matched_by_url: dict[str, tuple[int, str]] = {}
    matched_ids: list[Any] = []
    matched_points = 0

    for point in points:
        payload = point.get("payload") or {}
        url = payload.get("url")
        if not isinstance(url, str) or not url:
            continue

        matched, matched_prefix = is_excluded_url_path(url, prefixes)
        if not matched or not matched_prefix:
            continue

        matched_points += 1
        matched_ids.append(point.get("id"))

        prev = matched_by_url.get(url)
        if prev is None:
            matched_by_url[url] = (1, matched_prefix)
        else:
            matched_by_url[url] = (prev[0] + 1, prev[1])

    top_urls = sorted(
        [(u, c, p) for u, (c, p) in matched_by_url.items()],
        key=lambda row: row[1],
        reverse=True,
    )[:10]

    return ExcludeViolationStats(
        matched_points=matched_points,
        matched_urls=len(matched_by_url),
        matched_ids=matched_ids,
        top_urls=top_urls,
    )


def delete_points(collection: str, ids: list[Any], *, emit_output: bool = True) -> None:
    if not ids:
        return

    batch_size = 1000
    deleted = 0
    if emit_output:
        print(f"Deleting {len(ids)} points in batches of {batch_size}...")

    for idx in range(0, len(ids), batch_size):
        batch = ids[idx : idx + batch_size]
        qdrant_request(
            f"/collections/{collection}/points/delete?wait=true",
            method="POST",
            body={"points": batch},
            timeout=60,
        )
        deleted += len(batch)
        if emit_output:
            sys.stderr.write(f"\rDeleted {deleted}/{len(ids)} points...")
            sys.stderr.flush()

    if emit_output:
        sys.stderr.write("\n")
        print(f"Deleted {deleted} points")


def collect_health_info() -> dict[str, Any]:
    info = get_cluster_info()
    collections = list_collections()
    collection_rows: list[dict[str, Any]] = []
    total_points = 0
    total_vectors = 0

    for name in collections:
        c = get_collection_info(name)
        points_count = int(c.get("points_count") or 0)
        vectors_count = int(c.get("vectors_count") or 0)
        indexed_vectors_count = int(c.get("indexed_vectors_count") or 0)
        segments_count = int(c.get("segments_count") or 0)
        status = str(c.get("status", "unknown"))

        total_points += points_count
        total_vectors += vectors_count
        collection_rows.append(
            {
                "name": name,
                "points_count": points_count,
                "vectors_count": vectors_count,
                "indexed_vectors_count": indexed_vectors_count,
                "segments_count": segments_count,
                "status": status,
            }
        )

    return {
        "cluster": {"version": info.get("version", "unknown"), "commit": info.get("commit")},
        "collections_count": len(collection_rows),
        "collections": collection_rows,
        "totals": {"points": total_points, "vectors": total_vectors},
    }


def display_health_info(*, emit_output: bool = True) -> dict[str, Any]:
    result = collect_health_info()
    if not emit_output:
        return result

    print(f"\n{primary('Qdrant Health & Statistics')}")
    print(muted("=" * 60))
    print(f"\n{primary('Cluster Info')}:")
    print(f"  {accent('Version:')} {result['cluster']['version']}")
    if result["cluster"].get("commit"):
        print(f"  {accent('Commit:')} {str(result['cluster']['commit'])[:8]}")

    print(f"\n{primary('Collections:')} {result['collections_count']} total")
    if result["collections_count"] == 0:
        print("  (No collections found)")
        return result

    for row in result["collections"]:
        print(f"\n  - {accent(row['name'])}")
        print(f"    {accent('Points:')} {row['points_count']:,}")
        print(f"    {accent('Vectors:')} {row['vectors_count']:,}")
        print(f"    {accent('Indexed:')} {row['indexed_vectors_count']:,}")
        print(f"    {accent('Segments:')} {row['segments_count']}")
        print(f"    {accent('Status:')} {status_text(row['status'])}")

    print(f"\n{primary('Overall Stats')}:")
    print(f"  {accent('Total points:')} {result['totals']['points']:,}")
    print(f"  {accent('Total vectors:')} {result['totals']['vectors']:,}")
    return result


def display_aliases_info(*, emit_output: bool = True) -> dict[str, Any]:
    aliases = list_aliases()
    collections = set(list_collections())
    dangling = [a for a in aliases if a["collection_name"] not in collections]
    result = {
        "aliases_count": len(aliases),
        "dangling_count": len(dangling),
        "aliases": aliases,
        "dangling": dangling,
    }

    if not emit_output:
        return result

    print(f"\n{primary('Qdrant Aliases')}")
    print(muted("=" * 60))
    print(f"  {accent('Aliases:')} {len(aliases)}")
    print(f"  {accent('Dangling:')} {len(dangling)}")
    if not aliases:
        print(f"  {muted('No aliases found.')}")
        return result

    for row in aliases:
        marker = status_text("failed") if row in dangling else status_text("completed")
        print(f"  {marker} {accent(row['alias_name'])} {muted('->')} {row['collection_name']}")

    return result


def summarize_chunk_distribution(points: list[dict[str, Any]]) -> tuple[dict[str, int], list[int]]:
    url_counts: dict[str, int] = defaultdict(int)
    for point in points:
        payload = point.get("payload") or {}
        url = payload.get("url")
        if isinstance(url, str) and url:
            url_counts[url] += 1
    counts = sorted(url_counts.values())
    return dict(url_counts), counts


def analyze_payload_schema(points: list[dict[str, Any]]) -> dict[str, Any]:
    checks: dict[str, dict[str, int]] = {
        "url": {"present": 0, "missing": 0, "type_mismatch": 0},
        "chunk_text": {"present": 0, "missing": 0, "type_mismatch": 0},
        "chunk_index": {"present": 0, "missing": 0, "type_mismatch": 0},
        "title": {"present": 0, "missing": 0, "type_mismatch": 0},
        "scraped_at": {"present": 0, "missing": 0, "type_mismatch": 0},
    }

    for point in points:
        payload = point.get("payload") or {}

        def inspect(field: str, value: Any, expected: tuple[type, ...], aliases: list[str] | None = None) -> None:
            vals: list[Any] = [value]
            if aliases:
                vals.extend(payload.get(a) for a in aliases)
            actual = next((v for v in vals if v is not None), None)
            if actual is None:
                checks[field]["missing"] += 1
                return
            if isinstance(actual, expected):
                checks[field]["present"] += 1
                return
            checks[field]["type_mismatch"] += 1

        inspect("url", payload.get("url"), (str,))
        inspect("chunk_text", payload.get("chunk_text"), (str,))
        inspect("chunk_index", payload.get("chunk_index"), (int,))
        inspect("title", payload.get("title"), (str,))
        inspect("scraped_at", payload.get("scraped_at"), (str,), aliases=["scrapedAt", "file_modified_at", "fileModifiedAt"])

    return {
        "total_points": len(points),
        "fields": checks,
    }


def analyze_domain_breakdown(points: list[dict[str, Any]], top: int = 20) -> dict[str, Any]:
    domain_points: dict[str, int] = defaultdict(int)
    by_domain_for_duplicates: dict[str, list[dict[str, Any]]] = defaultdict(list)
    unique_urls_per_domain: dict[str, set[str]] = defaultdict(set)

    for point in points:
        payload = point.get("payload") or {}
        url = payload.get("url")
        if not isinstance(url, str) or not url:
            continue
        parsed = urllib.parse.urlparse(url)
        domain = (parsed.hostname or "").lower() or "(invalid-url)"
        domain_points[domain] += 1
        by_domain_for_duplicates[domain].append(point)
        unique_urls_per_domain[domain].add(url)

    rows: list[dict[str, Any]] = []
    for domain, count in domain_points.items():
        duplicates = find_duplicates(by_domain_for_duplicates[domain])
        duplicate_points = sum(max(0, d.count - 1) for d in duplicates)
        dup_rate = (duplicate_points / count * 100.0) if count > 0 else 0.0
        rows.append(
            {
                "domain": domain,
                "points": count,
                "unique_urls": len(unique_urls_per_domain[domain]),
                "duplicate_groups": len(duplicates),
                "duplicate_points": duplicate_points,
                "duplicate_rate_pct": round(dup_rate, 2),
            }
        )

    rows.sort(key=lambda r: r["points"], reverse=True)
    return {
        "total_domains": len(rows),
        "top_domains": rows[:top],
    }


def parse_payload_timestamp(payload: dict[str, Any]) -> datetime | None:
    candidates = [
        payload.get("scraped_at"),
        payload.get("scrapedAt"),
        payload.get("file_modified_at"),
        payload.get("fileModifiedAt"),
    ]
    for value in candidates:
        if not isinstance(value, str) or not value.strip():
            continue
        v = value.strip()
        if v.endswith("Z"):
            v = v[:-1] + "+00:00"
        try:
            dt = datetime.fromisoformat(v)
            if dt.tzinfo is None:
                dt = dt.replace(tzinfo=UTC)
            return dt.astimezone(UTC)
        except ValueError:
            continue
    return None


def analyze_stale_data(points: list[dict[str, Any]], days: int) -> dict[str, Any]:
    now = datetime.now(UTC)
    threshold = now - timedelta(days=days)
    with_timestamp = 0
    stale = 0
    newest: datetime | None = None
    oldest: datetime | None = None

    for point in points:
        payload = point.get("payload") or {}
        ts = parse_payload_timestamp(payload)
        if ts is None:
            continue
        with_timestamp += 1
        if ts < threshold:
            stale += 1
        if newest is None or ts > newest:
            newest = ts
        if oldest is None or ts < oldest:
            oldest = ts

    return {
        "threshold_days": days,
        "threshold_utc": threshold.isoformat(),
        "points_total": len(points),
        "points_with_timestamp": with_timestamp,
        "points_missing_timestamp": len(points) - with_timestamp,
        "stale_points": stale,
        "stale_rate_pct": round((stale / with_timestamp * 100.0), 2) if with_timestamp else 0.0,
        "newest_timestamp": newest.isoformat() if newest else None,
        "oldest_timestamp": oldest.isoformat() if oldest else None,
    }


def analyze_exclude_sync(effective_excludes: list[str]) -> dict[str, Any]:
    rust_defaults = extract_rust_default_excludes()
    script_defaults = sorted(set(DEFAULT_EXCLUDE_PREFIXES))
    rust_set = set(rust_defaults)
    script_set = set(script_defaults)

    missing_in_script = sorted(rust_set - script_set)
    extra_in_script = sorted(script_set - rust_set)
    in_sync = not missing_in_script and not extra_in_script

    return {
        "in_sync": in_sync,
        "rust_defaults_count": len(rust_defaults),
        "script_defaults_count": len(script_defaults),
        "effective_excludes_count": len(effective_excludes),
        "missing_in_script": missing_in_script,
        "extra_in_script": extra_in_script,
        "rust_defaults": rust_defaults,
        "script_defaults": script_defaults,
        "effective_excludes": effective_excludes,
    }


def confirm_destructive_action(command: str, *, yes: bool, dry_run: bool, target: str) -> None:
    if dry_run:
        return
    if yes:
        return
    if not sys.stdin.isatty():
        raise RuntimeError(
            f"Destructive command '{command}' on {target} requires --yes in non-interactive mode"
        )
    prompt = f"{command} will mutate {target}. Proceed? [y/N]: "
    answer = input(prompt).strip().lower()
    if answer not in {"y", "yes"}:
        raise RuntimeError("Aborted by user")


def check_collection(
    collection: str,
    *,
    delete_duplicates: bool = False,
    delete_excluded: bool = False,
    dry_run: bool = False,
    exclude_prefixes: list[str] | None = None,
    sample_limit: int | None = None,
    emit_output: bool = True,
) -> dict[str, Any]:
    if emit_output:
        print(f"\n{primary('Collection:')} {accent(collection)}")
        print(muted("=" * 60))

    points = fetch_all_points(collection, emit_output=emit_output, sample_limit=sample_limit)
    if not points:
        if emit_output:
            print("No points found in collection")
        return {
            "collection": collection,
            "total_points": 0,
            "unique_urls": 0,
            "data_quality": {"missing_url": 0, "missing_content": 0, "empty_content": 0, "missing_chunk_index": 0, "total_issues": 0},
            "exclude": {"matched_points": 0, "matched_urls": 0, "top_urls": [], "deleted_points": 0, "would_delete_points": 0},
            "duplicates": {"groups": 0, "total_duplicate_points": 0, "top_urls": [], "deleted_points": 0, "would_delete_points": 0},
            "chunk_distribution": None,
        }

    prefixes = exclude_prefixes if exclude_prefixes is not None else DEFAULT_EXCLUDE_PREFIXES

    if emit_output:
        print(f"\n{primary('Data Quality Check')}")
    issues = check_data_quality(points)
    if emit_output:
        if issues.total == 0:
            print("  OK: no data quality issues")
        else:
            print(f"  Issues found: {issues.total}")
            if issues.missing_url:
                print(f"  - Missing URL: {issues.missing_url}")
            if issues.missing_content:
                print(f"  - Missing content: {issues.missing_content}")
            if issues.empty_content:
                print(f"  - Empty content: {issues.empty_content}")
            if issues.missing_chunk_index:
                print(f"  - Missing chunk_index: {issues.missing_chunk_index}")

    if emit_output:
        print(f"\n{primary('Exclude Rules Check')}")
    excluded = check_exclude_violations(points, prefixes)
    excluded_unique_ids = [x for x in dict.fromkeys(excluded.matched_ids) if x is not None]
    excluded_deleted = 0
    if emit_output:
        if excluded.matched_points == 0:
            print("  OK: no points matched exclude rules")
        else:
            print(f"  Matched: {excluded.matched_points} points across {excluded.matched_urls} URLs")
            for url, count, pattern in excluded.top_urls:
                print(f"  - {count}x {url}")
                print(f"    pattern: {pattern}")

    if delete_excluded and excluded_unique_ids:
        if dry_run:
            if emit_output:
                print(f"  {muted('dry-run: would delete')} {len(excluded_unique_ids)} {muted('exclude-matched points')}")
        else:
            delete_points(collection, excluded_unique_ids, emit_output=emit_output)
            excluded_deleted = len(excluded_unique_ids)

    if emit_output:
        print(f"\n{primary('Duplicate Chunk Analysis')}")
    duplicates = find_duplicates(points)
    duplicate_ids_to_delete: list[Any] = []
    for dup in duplicates:
        duplicate_ids_to_delete.extend(dup.ids[1:])
    total_duplicate_points = len(duplicate_ids_to_delete)
    duplicate_deleted = 0

    if emit_output:
        if not duplicates:
            print("  OK: no duplicate chunks")
        else:
            print(f"  Duplicate groups: {len(duplicates)}")
            for dup in duplicates[:10]:
                print(f"  - {dup.count}x {dup.url}")
            if len(duplicates) > 10:
                print(f"  ... and {len(duplicates) - 10} more")
            print(f"  Total duplicate points: {total_duplicate_points}")

    if delete_duplicates and duplicate_ids_to_delete:
        if dry_run:
            if emit_output:
                print(f"  {muted('dry-run: would delete')} {len(duplicate_ids_to_delete)} {muted('duplicate points')}")
        else:
            delete_points(collection, duplicate_ids_to_delete, emit_output=emit_output)
            duplicate_deleted = len(duplicate_ids_to_delete)

    url_counts, counts = summarize_chunk_distribution(points)
    chunk_distribution: dict[str, Any] | None = None
    if counts:
        chunk_distribution = {
            "min": counts[0],
            "median": counts[len(counts) // 2],
            "avg": len(points) / len(url_counts),
            "max": counts[-1],
            "urls_over_50": len([c for c in counts if c > 50]),
        }
    if emit_output:
        print(f"\n{primary('Chunk Distribution')}")
        if chunk_distribution:
            print(f"  Min chunks per URL: {chunk_distribution['min']}")
            print(f"  Median chunks per URL: {chunk_distribution['median']}")
            print(f"  Average chunks per URL: {chunk_distribution['avg']:.1f}")
            print(f"  Max chunks per URL: {chunk_distribution['max']}")

    result = {
        "collection": collection,
        "total_points": len(points),
        "unique_urls": len(url_counts),
        "data_quality": {
            "missing_url": issues.missing_url,
            "missing_content": issues.missing_content,
            "empty_content": issues.empty_content,
            "missing_chunk_index": issues.missing_chunk_index,
            "total_issues": issues.total,
        },
        "exclude": {
            "matched_points": excluded.matched_points,
            "matched_urls": excluded.matched_urls,
            "top_urls": [{"url": u, "points": c, "prefix": p} for u, c, p in excluded.top_urls],
            "would_delete_points": len(excluded_unique_ids),
            "deleted_points": excluded_deleted,
        },
        "duplicates": {
            "groups": len(duplicates),
            "total_duplicate_points": total_duplicate_points,
            "top_urls": [{"url": d.url, "count": d.count} for d in duplicates[:10]],
            "would_delete_points": len(duplicate_ids_to_delete),
            "deleted_points": duplicate_deleted,
        },
        "chunk_distribution": chunk_distribution,
    }

    if emit_output:
        print(f"\n{primary('Summary')}")
        print(f"  {accent('Total points:')} {result['total_points']}")
        print(f"  {accent('Unique URLs:')} {result['unique_urls']}")
        print(f"  {accent('Duplicate groups:')} {result['duplicates']['groups']}")
        print(f"  {accent('Data quality issues:')} {result['data_quality']['total_issues']}")
        print(
            f"  {accent('Exclude rule matches:')} {result['exclude']['matched_points']} points ({result['exclude']['matched_urls']} URLs)"
        )

    return result


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="qdrant-quality.py",
        description="Qdrant collection quality toolkit for duplicate detection, exclusion audits, and cleanup.",
        formatter_class=argparse.RawTextHelpFormatter,
        epilog=textwrap.dedent(
            """\
            Examples:
              python3 scripts/qdrant-quality.py health
              python3 scripts/qdrant-quality.py check
              python3 scripts/qdrant-quality.py check --collection cortex
              python3 scripts/qdrant-quality.py check-all
              python3 scripts/qdrant-quality.py delete-duplicates --collection cortex
              python3 scripts/qdrant-quality.py delete-excluded --collection cortex
              python3 scripts/qdrant-quality.py check --exclude-path-prefix /fr,/de
              python3 scripts/qdrant-quality.py check --exclude-path-prefix none
            """
        ),
    )
    parser.add_argument(
        "--url",
        default=QDRANT_URL,
        help="Qdrant base URL. Defaults to env/.env QDRANT_URL, then runtime-aware local fallback.",
    )
    parser.add_argument(
        "--exclude-path-prefix",
        action="append",
        default=[],
        help=(
            "Path prefix exclusion override for audits.\n"
            "Repeat flag or use comma-separated values.\n"
            "Use 'none' to disable default language-prefix exclusions."
        ),
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Preview delete actions without deleting points.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit machine-readable JSON output.",
    )
    parser.add_argument(
        "--sample",
        type=int,
        default=0,
        help="Limit scan to first N points (0 = full collection scan).",
    )
    parser.add_argument(
        "--yes",
        action="store_true",
        help="Skip confirmation prompt for destructive commands.",
    )

    sub = parser.add_subparsers(
        dest="command",
        required=True,
        title="Commands",
        metavar="COMMAND",
        description="Run one of the following commands:",
    )

    sub.add_parser(
        "help",
        help="Show full help with command descriptions and examples",
        description="Print top-level help output.",
    )
    sub.add_parser(
        "health",
        help="Show cluster and collection health stats",
        description="Display Qdrant version plus per-collection point/vector/status metrics.",
    )
    sub.add_parser(
        "aliases",
        help="Audit aliases and flag dangling targets",
        description="List aliases and identify aliases pointing to missing collections.",
    )
    p_schema = sub.add_parser(
        "payload-schema",
        help="Audit payload field presence and types",
        description="Validate payload key presence/type rates for a collection.",
    )
    p_schema.add_argument("--collection", default=DEFAULT_COLLECTION)

    p_domains = sub.add_parser(
        "domain-breakdown",
        help="Show top domains by points and duplicate rates",
        description="Compute per-domain point counts and duplicate density.",
    )
    p_domains.add_argument("--collection", default=DEFAULT_COLLECTION)
    p_domains.add_argument("--top", type=int, default=20, help="Number of top domains to show.")

    p_stale = sub.add_parser(
        "stale-data",
        help="Check age of points using timestamps",
        description="Assess stale data rate based on scraped/file-modified timestamps.",
    )
    p_stale.add_argument("--collection", default=DEFAULT_COLLECTION)
    p_stale.add_argument("--days", type=int, default=90, help="Staleness threshold in days.")

    sub.add_parser(
        "strict-exclude-sync",
        help="Compare script/Rust default exclude prefixes",
        description="Detect drift between script defaults and Rust default_exclude_prefixes().",
    )

    p_check = sub.add_parser(
        "check",
        help="Audit one collection (quality, duplicates, exclusions, distribution)",
        description="Run full quality analysis for a single collection.",
    )
    p_check.add_argument("--collection", default=DEFAULT_COLLECTION)

    p_all = sub.add_parser(
        "check-all",
        help="Audit every collection",
        description="Run full quality analysis for all collections in Qdrant.",
    )

    p_dedup = sub.add_parser(
        "delete-duplicates",
        help="Delete duplicate points in one collection",
        description="Find duplicate points by (url, chunk_index) and delete extras.",
    )
    p_dedup.add_argument("--collection", default=DEFAULT_COLLECTION)

    p_ex = sub.add_parser(
        "delete-excluded",
        help="Delete points whose URL path matches exclude prefixes",
        description="Delete points that match effective exclude-path-prefix rules.",
    )
    p_ex.add_argument("--collection", default=DEFAULT_COLLECTION)

    p_all_dedup = sub.add_parser(
        "delete-duplicates-all",
        help="Delete duplicate points in all collections",
        description="Find and delete duplicate points across every collection.",
    )
    p_all_ex = sub.add_parser(
        "delete-excluded-all",
        help="Delete excluded-path matches in all collections",
        description="Delete points matching exclude-path-prefix rules in every collection.",
    )

    return parser


def main() -> int:
    raw_args = sys.argv[1:]
    if not raw_args or raw_args[0] == "help" or "-h" in raw_args or "--help" in raw_args:
        print(render_help_text())
        return 0

    parser = build_parser()
    forced_json = "--json" in raw_args
    forced_dry_run = "--dry-run" in raw_args
    forced_sample: int | None = None
    sample_parse_error: str | None = None
    normalized_args: list[str] = []
    i = 0
    while i < len(raw_args):
        token = raw_args[i]
        if token in {"--json", "--dry-run"}:
            i += 1
            continue
        if token == "--sample":
            if i + 1 < len(raw_args):
                try:
                    forced_sample = int(raw_args[i + 1])
                except ValueError:
                    sample_parse_error = f"invalid int value for --sample: {raw_args[i + 1]!r}"
                i += 2
                continue
            sample_parse_error = "--sample requires a value"
            i += 1
            continue
        if token.startswith("--sample="):
            try:
                forced_sample = int(token.split("=", 1)[1])
            except ValueError:
                sample_parse_error = f"invalid int value for --sample: {token.split('=', 1)[1]!r}"
            i += 1
            continue
        normalized_args.append(token)
        i += 1

    args = parser.parse_args(normalized_args)
    if sample_parse_error:
        parser.error(sample_parse_error)
    json_mode = bool(args.json or forced_json)
    dry_run_mode = bool(args.dry_run or forced_dry_run)

    global QDRANT_URL
    configured_url = str(args.url).rstrip("/")
    QDRANT_URL = resolve_runtime_qdrant_url(configured_url)

    emit_output = not json_mode
    if emit_output:
        print(f"\n{primary('Qdrant Quality Check')}")
        print(f"{accent('URL:')} {QDRANT_URL}")
        if QDRANT_URL != configured_url:
            print(
                f"{muted('Configured URL')} {configured_url} {muted('was not reachable from this runtime; using')} {QDRANT_URL}"
            )

    raw_prefixes: list[str] = []
    env_prefixes = os.getenv("AXON_EXCLUDE_PATH_PREFIX") or DOTENV_VALUES.get("AXON_EXCLUDE_PATH_PREFIX") or os.getenv("EXCLUDE_PATH_PREFIX") or DOTENV_VALUES.get("EXCLUDE_PATH_PREFIX")
    if env_prefixes:
        raw_prefixes.extend(parse_csv_values(env_prefixes))
    for item in args.exclude_path_prefix:
        raw_prefixes.extend(parse_csv_values(item))

    normalized = normalize_exclude_prefixes(raw_prefixes)
    effective_exclude_prefixes = normalized.prefixes.copy()
    if not effective_exclude_prefixes and not normalized.disable_defaults:
        effective_exclude_prefixes = list(DEFAULT_EXCLUDE_PREFIXES)

    if emit_output:
        print(f"{accent('Exclude path prefixes loaded:')} {len(effective_exclude_prefixes)}")

    command = args.command
    resolved_sample = forced_sample if forced_sample is not None else args.sample
    sample_limit = resolved_sample if resolved_sample and resolved_sample > 0 else None
    destructive_commands = {
        "delete-duplicates",
        "delete-excluded",
        "delete-duplicates-all",
        "delete-excluded-all",
    }
    if sample_limit is not None and command in destructive_commands and not dry_run_mode:
        parser.error("--sample is only allowed with destructive commands when --dry-run is set")

    if command == "health":
        result = display_health_info(emit_output=emit_output)
        if json_mode:
            print(json.dumps({"command": "health", **result}, indent=2))
        return 0
    
    if command == "aliases":
        result = display_aliases_info(emit_output=emit_output)
        if json_mode:
            print(json.dumps({"command": "aliases", **result}, indent=2))
        return 0

    if command == "strict-exclude-sync":
        result = analyze_exclude_sync(effective_exclude_prefixes)
        if emit_output:
            print(f"\n{primary('Strict Exclude Sync')}")
            print(muted("=" * 60))
            sync_status = status_text("completed" if result["in_sync"] else "failed")
            print(f"  {accent('Status:')} {sync_status}")
            print(f"  {accent('Rust defaults:')} {result['rust_defaults_count']}")
            print(f"  {accent('Script defaults:')} {result['script_defaults_count']}")
            print(f"  {accent('Effective excludes:')} {result['effective_excludes_count']}")
            if result["missing_in_script"]:
                print(f"  {accent('Missing in script:')} {', '.join(result['missing_in_script'])}")
            if result["extra_in_script"]:
                print(f"  {accent('Extra in script:')} {', '.join(result['extra_in_script'])}")
        if json_mode:
            print(json.dumps({"command": "strict-exclude-sync", **result}, indent=2))
        return 0

    if command == "payload-schema":
        points = fetch_all_points(args.collection, emit_output=emit_output, sample_limit=sample_limit)
        result = analyze_payload_schema(points)
        if emit_output:
            print(f"\n{primary('Payload Schema Audit')} {accent(args.collection)}")
            print(muted("=" * 60))
            print(f"  {accent('Points scanned:')} {result['total_points']}")
            for field, stat in result["fields"].items():
                print(
                    f"  {accent(field + ':')} present={stat['present']} missing={stat['missing']} type_mismatch={stat['type_mismatch']}"
                )
        if json_mode:
            print(json.dumps({"command": "payload-schema", "collection": args.collection, "sample_limit": sample_limit, "result": result}, indent=2))
        return 0

    if command == "domain-breakdown":
        points = fetch_all_points(args.collection, emit_output=emit_output, sample_limit=sample_limit)
        result = analyze_domain_breakdown(points, top=max(1, int(args.top)))
        if emit_output:
            print(f"\n{primary('Domain Breakdown')} {accent(args.collection)}")
            print(muted("=" * 60))
            print(f"  {accent('Domains:')} {result['total_domains']}")
            for row in result["top_domains"]:
                print(
                    f"  {accent(row['domain'])} {muted('points=')}{row['points']} {muted('urls=')}{row['unique_urls']} {muted('dup_rate=')}{row['duplicate_rate_pct']}%"
                )
        if json_mode:
            print(json.dumps({"command": "domain-breakdown", "collection": args.collection, "sample_limit": sample_limit, "top": args.top, "result": result}, indent=2))
        return 0

    if command == "stale-data":
        points = fetch_all_points(args.collection, emit_output=emit_output, sample_limit=sample_limit)
        result = analyze_stale_data(points, days=max(1, int(args.days)))
        if emit_output:
            print(f"\n{primary('Stale Data Audit')} {accent(args.collection)}")
            print(muted("=" * 60))
            print(f"  {accent('Threshold days:')} {result['threshold_days']}")
            print(f"  {accent('Points total:')} {result['points_total']}")
            print(f"  {accent('With timestamp:')} {result['points_with_timestamp']}")
            print(f"  {accent('Missing timestamp:')} {result['points_missing_timestamp']}")
            print(f"  {accent('Stale points:')} {result['stale_points']} ({result['stale_rate_pct']}%)")
            print(f"  {accent('Newest:')} {result['newest_timestamp']}")
            print(f"  {accent('Oldest:')} {result['oldest_timestamp']}")
        if json_mode:
            print(json.dumps({"command": "stale-data", "collection": args.collection, "sample_limit": sample_limit, "days": args.days, "result": result}, indent=2))
        return 0

    if command == "check":
        result = check_collection(
            args.collection,
            dry_run=dry_run_mode,
            exclude_prefixes=effective_exclude_prefixes,
            sample_limit=sample_limit,
            emit_output=emit_output,
        )
        if json_mode:
            print(json.dumps({"command": "check", "dry_run": dry_run_mode, "result": result}, indent=2))
        return 0

    if command == "check-all":
        health = display_health_info(emit_output=emit_output)
        collections = list_collections()
        if not collections:
            if emit_output:
                print("\nNo collections found")
            if json_mode:
                print(json.dumps({"command": "check-all", "dry_run": dry_run_mode, "health": health, "results": []}, indent=2))
            return 0
        all_results: list[dict[str, Any]] = []
        for name in collections:
            all_results.append(
                check_collection(
                    name,
                    dry_run=dry_run_mode,
                    exclude_prefixes=effective_exclude_prefixes,
                    sample_limit=sample_limit,
                    emit_output=emit_output,
                )
            )
        if json_mode:
            print(json.dumps({"command": "check-all", "dry_run": dry_run_mode, "health": health, "results": all_results}, indent=2))
        return 0

    if command == "delete-duplicates":
        confirm_destructive_action(
            command,
            yes=args.yes,
            dry_run=dry_run_mode,
            target=f"collection '{args.collection}'",
        )
        result = check_collection(
            args.collection,
            delete_duplicates=True,
            dry_run=dry_run_mode,
            exclude_prefixes=effective_exclude_prefixes,
            sample_limit=sample_limit,
            emit_output=emit_output,
        )
        if json_mode:
            print(json.dumps({"command": "delete-duplicates", "dry_run": dry_run_mode, "result": result}, indent=2))
        return 0

    if command == "delete-excluded":
        confirm_destructive_action(
            command,
            yes=args.yes,
            dry_run=dry_run_mode,
            target=f"collection '{args.collection}'",
        )
        result = check_collection(
            args.collection,
            delete_excluded=True,
            dry_run=dry_run_mode,
            exclude_prefixes=effective_exclude_prefixes,
            sample_limit=sample_limit,
            emit_output=emit_output,
        )
        if json_mode:
            print(json.dumps({"command": "delete-excluded", "dry_run": dry_run_mode, "result": result}, indent=2))
        return 0

    if command == "delete-duplicates-all":
        confirm_destructive_action(
            command,
            yes=args.yes,
            dry_run=dry_run_mode,
            target="all collections",
        )
        collections = list_collections()
        all_results: list[dict[str, Any]] = []
        for name in collections:
            all_results.append(
                check_collection(
                    name,
                    delete_duplicates=True,
                    dry_run=dry_run_mode,
                    exclude_prefixes=effective_exclude_prefixes,
                    sample_limit=sample_limit,
                    emit_output=emit_output,
                )
            )
        if json_mode:
            print(json.dumps({"command": "delete-duplicates-all", "dry_run": dry_run_mode, "results": all_results}, indent=2))
        return 0

    if command == "delete-excluded-all":
        confirm_destructive_action(
            command,
            yes=args.yes,
            dry_run=dry_run_mode,
            target="all collections",
        )
        collections = list_collections()
        all_results: list[dict[str, Any]] = []
        for name in collections:
            all_results.append(
                check_collection(
                    name,
                    delete_excluded=True,
                    dry_run=dry_run_mode,
                    exclude_prefixes=effective_exclude_prefixes,
                    sample_limit=sample_limit,
                    emit_output=emit_output,
                )
            )
        if json_mode:
            print(json.dumps({"command": "delete-excluded-all", "dry_run": dry_run_mode, "results": all_results}, indent=2))
        return 0

    parser.print_help()
    return 2


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        print("\nInterrupted", file=sys.stderr)
        raise SystemExit(130)
    except Exception as exc:  # noqa: BLE001
        print(f"Error: {exc}", file=sys.stderr)
        raise SystemExit(1)
