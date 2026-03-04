#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
middleware="${root}/apps/web/middleware.ts"

if [[ -f "${middleware}" ]]; then
  echo "ERROR: deprecated Next.js middleware file detected:"
  echo "  - apps/web/middleware.ts"
  echo
  echo "Use apps/web/proxy.ts instead. middleware.ts must not exist."
  exit 1
fi

echo "OK: no deprecated apps/web/middleware.ts file"
