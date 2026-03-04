#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

sha="$(git rev-parse HEAD)"
echo "Rebuilding axon-web + axon-workers at git SHA: $sha"

AXON_GIT_SHA="$sha" docker compose up -d --build --force-recreate axon-workers axon-web

echo
echo "Verifying running container revisions..."
"$repo_root/scripts/check-container-revisions.sh"
