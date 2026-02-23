#!/usr/bin/env bash
# Healthcheck: verify at least one axon worker process is running.
# Uses /proc directly — no procps (ps/pgrep) required.
set -euo pipefail

workers=0
for cmdline_file in /proc/[0-9]*/cmdline; do
    cmd=$(tr '\0' ' ' < "$cmdline_file" 2>/dev/null || true)
    case "$cmd" in
        *axon*worker*) workers=$((workers + 1)) ;;
    esac
done

if [ "$workers" -lt 1 ]; then
    echo "healthcheck FAIL: no axon worker processes running" >&2
    exit 1
fi

echo "healthcheck OK: ${workers} axon worker process(es) running"
exit 0
