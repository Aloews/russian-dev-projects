#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v docker >/dev/null 2>&1 || { echo "Docker is required." >&2; exit 1; }
docker compose version >/dev/null 2>&1 || { echo "Docker Compose v2 is required." >&2; exit 1; }
command -v openssl >/dev/null 2>&1 || { echo "OpenSSL is required to generate local credentials." >&2; exit 1; }
command -v curl >/dev/null 2>&1 || { echo "curl is required for the startup check." >&2; exit 1; }

if [[ -L .env ]]; then
  echo "Refusing to read a symlinked .env file." >&2
  exit 1
fi
if [[ ! -f .env ]]; then
  touch .env
fi
USH_API_TOKEN="$(sed -n 's/^USH_API_TOKEN=//p' .env | tail -n 1)"
USH_DB_PASSWORD="$(sed -n 's/^USH_DB_PASSWORD=//p' .env | tail -n 1)"
if [[ -z "$USH_API_TOKEN" || -z "$USH_DB_PASSWORD" ]]; then
  umask 077
  api_token="$(openssl rand -hex 32)"
  db_password="$(openssl rand -hex 32)"
  rate_limit="$(sed -n 's/^USH_RATE_LIMIT_PER_MINUTE=//p' .env | tail -n 1)"
  port_from_file="$(sed -n 's/^PORT=//p' .env | tail -n 1)"
  cat > .env <<EOF
USH_API_TOKEN=$api_token
USH_DB_PASSWORD=$db_password
USH_RATE_LIMIT_PER_MINUTE=${rate_limit:-120}
PORT=${port_from_file:-8000}
EOF
fi
chmod 600 .env

USH_API_TOKEN="$(sed -n 's/^USH_API_TOKEN=//p' .env | tail -n 1)"
USH_DB_PASSWORD="$(sed -n 's/^USH_DB_PASSWORD=//p' .env | tail -n 1)"
PORT="${PORT:-$(sed -n 's/^PORT=//p' .env | tail -n 1)}"
PORT="${PORT:-8000}"
if [[ ! "$PORT" =~ ^[0-9]{1,5}$ ]]; then
  echo "PORT must be an integer from 1 to 65535." >&2
  exit 1
fi
PORT="$((10#$PORT))"
if (( PORT < 1 || PORT > 65535 )); then
  echo "PORT must be an integer from 1 to 65535." >&2
  exit 1
fi
export PORT
if [[ ! "$USH_API_TOKEN" =~ ^[A-Za-z0-9_-]{32,}$ || ! "$USH_DB_PASSWORD" =~ ^[A-Za-z0-9_-]{32,}$ ]]; then
  echo "Local .env must contain safe USH_API_TOKEN and USH_DB_PASSWORD values of at least 32 characters." >&2
  echo "Remove the local .env and rerun this script to generate new values." >&2
  exit 1
fi
export USH_API_TOKEN USH_DB_PASSWORD

docker compose -f compose.yml up --build -d

base_url="http://127.0.0.1:${PORT:-8000}"
ready=0
for ((attempt = 1; attempt <= 60; attempt++)); do
  if curl -fsS "$base_url/readyz" >/dev/null; then
    ready=1
    break
  fi
  sleep 2
done

if [[ "$ready" -ne 1 ]]; then
  docker compose -f compose.yml logs --no-color hub postgres
  echo "Universal Service Hub did not become ready." >&2
  exit 1
fi

unauthorized_status="$(curl -sS -o /dev/null -w '%{http_code}' "$base_url/v1/providers")"
[[ "$unauthorized_status" == "401" ]] || {
  echo "Expected unauthenticated API request to return 401; received $unauthorized_status." >&2
  exit 1
}

curl -fsS -H "Authorization: Bearer $USH_API_TOKEN" "$base_url/v1/providers" >/dev/null

echo "Universal Service Hub is running at $base_url"
echo "Health: $base_url/healthz"
echo "Readiness: $base_url/readyz"
echo "Root API token is stored only in $ROOT/.env (mode 0600); it was not printed."
