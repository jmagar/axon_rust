#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

expected_sha="$(git rev-parse HEAD)"
export AXON_GIT_SHA="$expected_sha"
services=("axon-workers" "axon-web")

echo "Expected SHA: $expected_sha"
echo

status=0

for service in "${services[@]}"; do
  cid="$(docker compose ps -q "$service")"
  if [[ -z "$cid" ]]; then
    echo "[$service] not running"
    status=1
    continue
  fi

  revision="$(docker inspect --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}' "$cid" 2>/dev/null || true)"
  axon_sha="$(docker inspect --format '{{ index .Config.Labels "io.axon.git_sha" }}' "$cid" 2>/dev/null || true)"
  label_sha="${revision:-$axon_sha}"

  if [[ -z "$label_sha" || "$label_sha" == "<no value>" ]]; then
    echo "[$service] missing image revision label"
    status=1
    continue
  fi

  if [[ "$label_sha" != "$expected_sha" ]]; then
    echo "[$service] SHA mismatch"
    echo "  running:  $label_sha"
    echo "  expected: $expected_sha"
    status=1
    continue
  fi

  echo "[$service] OK ($label_sha)"
done

if [[ "$status" -ne 0 ]]; then
  echo
  echo "Container revision check FAILED."
  echo "Run: ./scripts/rebuild-fresh.sh"
  exit 1
fi

echo
echo "Container revision check passed."
