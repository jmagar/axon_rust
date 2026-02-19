#!/bin/sh
# shellcheck shell=sh
set -eu

for service in crawl-worker batch-worker extract-worker embed-worker; do
  s6-svstat -u "/run/service/${service}" >/dev/null
  s6-svstat -u "/run/service/${service}/log" >/dev/null
done
