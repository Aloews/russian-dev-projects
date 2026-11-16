#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ ! -f .env.local ]]; then
  echo "No local Hub environment exists; nothing to stop."
  exit 0
fi
if [[ -L .env.local ]]; then
  echo "Refusing to read a symlinked .env.local file." >&2
  exit 1
fi

USH_API_TOKEN="$(sed -n 's/^USH_API_TOKEN=//p' .env.local | tail -n 1)"
USH_DB_PASSWORD="$(sed -n 's/^USH_DB_PASSWORD=//p' .env.local | tail -n 1)"
if [[ ! "$USH_API_TOKEN" =~ ^[A-Za-z0-9_-]{32,}$ || ! "$USH_DB_PASSWORD" =~ ^[A-Za-z0-9_-]{32,}$ ]]; then
  echo "Local .env.local credentials are invalid; refusing to run Compose." >&2
  exit 1
fi
export USH_API_TOKEN USH_DB_PASSWORD

docker compose --env-file .env.local -f compose.yml down
echo "Universal Service Hub stopped. PostgreSQL volume was retained."
