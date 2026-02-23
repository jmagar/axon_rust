#!/usr/bin/env python3
"""
One-shot migration:
Rewrite legacy Qdrant payload.url values that point to local markdown files
(.cache/.../markdown/*.md) into canonical crawled page URLs from manifest.jsonl.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Stats:
    manifests_scanned: int = 0
    mappings_discovered: int = 0
    mappings_with_hits: int = 0
    points_matched: int = 0
    mappings_updated: int = 0
    update_errors: int = 0


def post_json(url: str, payload: dict) -> dict:
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=30) as resp:
        body = resp.read().decode("utf-8")
    return json.loads(body) if body else {}


def iter_manifest_paths(root: Path) -> list[Path]:
    if not root.exists():
        return []
    out: list[Path] = []
    for dirpath, _, filenames in os.walk(root):
        if "manifest.jsonl" in filenames:
            out.append(Path(dirpath) / "manifest.jsonl")
    out.sort()
    return out


def old_path_candidates(raw_path: str, cwd: Path) -> set[str]:
    candidates: set[str] = set()
    raw = raw_path.strip()
    if not raw:
        return candidates

    def add(value: str) -> None:
        v = value.strip()
        if not v:
            return
        candidates.add(v)
        normalized_slashes = v.replace("\\", "/")
        candidates.add(normalized_slashes)

        # Legacy runtime roots that should map to repo-local cache-style paths.
        for marker in ("/appdata/axon-worker-output/", "appdata/axon-worker-output/"):
            idx = normalized_slashes.find(marker)
            if idx >= 0:
                tail = normalized_slashes[idx + len(marker) :].lstrip("/")
                candidates.add(f".cache/axon-rust/output/{tail}")
        if normalized_slashes.startswith("/app/output/"):
            tail = normalized_slashes[len("/app/output/") :].lstrip("/")
            candidates.add(f".cache/axon-rust/output/{tail}")

    add(raw)
    add(os.path.normpath(raw))

    p = Path(raw)
    try:
        abs_path = p.resolve(strict=False)
        add(str(abs_path))
        try:
            rel = abs_path.relative_to(cwd)
            add(str(rel))
            add(f"./{rel}")
        except ValueError:
            pass
    except Exception:
        pass

    return candidates


def build_mapping(root: Path, cwd: Path) -> tuple[dict[str, str], int]:
    mapping: dict[str, str] = {}
    manifests = iter_manifest_paths(root)
    for manifest in manifests:
        with manifest.open("r", encoding="utf-8", errors="ignore") as handle:
            for line in handle:
                line = line.strip()
                if not line:
                    continue
                try:
                    item = json.loads(line)
                except json.JSONDecodeError:
                    continue
                url = str(item.get("url", "")).strip()
                file_path = str(item.get("file_path", "")).strip()
                if not url.startswith(("http://", "https://")) or not file_path:
                    continue
                for candidate in old_path_candidates(file_path, cwd):
                    mapping.setdefault(candidate, url)
    return mapping, len(manifests)


def qdrant_count(qdrant_url: str, collection: str, old_url: str) -> int:
    endpoint = f"{qdrant_url.rstrip('/')}/collections/{collection}/points/count"
    payload = {
        "exact": True,
        "filter": {"must": [{"key": "url", "match": {"value": old_url}}]},
    }
    try:
        result = post_json(endpoint, payload)
        return int(result.get("result", {}).get("count", 0))
    except Exception:
        return 0


def qdrant_url_counts(qdrant_url: str, collection: str, limit: int = 1_000_000) -> dict[str, int]:
    endpoint = f"{qdrant_url.rstrip('/')}/collections/{collection}/facet"
    payload = {"key": "url", "limit": limit}
    result = post_json(endpoint, payload)
    out: dict[str, int] = {}
    hits = result.get("result", {}).get("hits", [])
    if not isinstance(hits, list):
        return out
    for hit in hits:
        if not isinstance(hit, dict):
            continue
        value = str(hit.get("value", "")).strip()
        if not value:
            continue
        count = int(hit.get("count", 0) or 0)
        out[value] = count
    return out


def qdrant_update_url(
    qdrant_url: str,
    collection: str,
    old_url: str,
    new_url: str,
) -> None:
    endpoint = f"{qdrant_url.rstrip('/')}/collections/{collection}/points/payload?wait=true"
    parsed = urllib.parse.urlparse(new_url)
    payload_data = {"url": new_url}
    if parsed.hostname:
        payload_data["domain"] = parsed.hostname
    payload = {
        "payload": payload_data,
        "filter": {"must": [{"key": "url", "match": {"value": old_url}}]},
    }
    post_json(endpoint, payload)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Migrate legacy local markdown source paths in Qdrant payload.url to canonical URLs."
    )
    parser.add_argument(
        "--root",
        default=".cache/axon-rust/output",
        help="Root directory to scan recursively for manifest.jsonl files.",
    )
    parser.add_argument(
        "--qdrant-url",
        default=os.getenv("QDRANT_URL", "http://127.0.0.1:53333"),
        help="Qdrant base URL.",
    )
    parser.add_argument(
        "--collection",
        default=os.getenv("AXON_COLLECTION", os.getenv("COLLECTION", "cortex")),
        help="Qdrant collection name.",
    )
    parser.add_argument(
        "--max-mappings",
        type=int,
        default=0,
        help="Optional cap on number of mappings to process (0 = unlimited).",
    )
    parser.add_argument(
        "--contains",
        action="append",
        default=[],
        help="Only process legacy source paths containing this substring. Repeatable.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Report what would be changed without writing updates.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    cwd = Path.cwd()
    root = Path(args.root)
    stats = Stats()

    mapping, stats.manifests_scanned = build_mapping(root, cwd)
    stats.mappings_discovered = len(mapping)

    if not mapping:
        print("No manifest mappings discovered. Nothing to migrate.")
        return 0

    try:
        existing_counts = qdrant_url_counts(args.qdrant_url, args.collection)
    except Exception as err:
        print(f"Failed to query Qdrant URL facets: {err}", file=sys.stderr)
        return 1

    processed = 0
    contains_filters = [value for value in args.contains if value]
    for old_url, count in sorted(existing_counts.items()):
        if not old_url.startswith(".cache/axon-rust/output/jobs/"):
            continue
        if contains_filters and not any(f in old_url for f in contains_filters):
            continue
        new_url = mapping.get(old_url)
        if not new_url:
            continue
        if old_url == new_url:
            continue
        if not new_url.startswith(("http://", "https://")):
            continue
        if args.max_mappings > 0 and processed >= args.max_mappings:
            break
        processed += 1

        if count <= 0:
            continue

        stats.mappings_with_hits += 1
        stats.points_matched += count

        if args.dry_run:
            print(f"DRY-RUN match={count} old={old_url} -> new={new_url}")
            continue

        try:
            qdrant_update_url(args.qdrant_url, args.collection, old_url, new_url)
            stats.mappings_updated += 1
            print(f"UPDATED match={count} old={old_url} -> new={new_url}")
        except urllib.error.HTTPError as err:
            stats.update_errors += 1
            body = err.read().decode("utf-8", errors="ignore")
            print(
                f"ERROR status={err.code} old={old_url} new={new_url} body={body}",
                file=sys.stderr,
            )
        except Exception as err:  # pragma: no cover - defensive
            stats.update_errors += 1
            print(f"ERROR old={old_url} new={new_url} err={err}", file=sys.stderr)

    print()
    print("Migration summary")
    print(f"  manifests_scanned: {stats.manifests_scanned}")
    print(f"  mappings_discovered: {stats.mappings_discovered}")
    print(f"  mappings_with_hits: {stats.mappings_with_hits}")
    print(f"  points_matched: {stats.points_matched}")
    print(f"  mappings_updated: {stats.mappings_updated}")
    print(f"  update_errors: {stats.update_errors}")
    print(f"  dry_run: {args.dry_run}")
    print(f"  collection: {args.collection}")
    print(f"  qdrant_url: {args.qdrant_url}")

    return 0 if stats.update_errors == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
