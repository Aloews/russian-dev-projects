#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
docker compose -f compose.yml down
echo "Universal Service Hub stopped. PostgreSQL volume was retained."
