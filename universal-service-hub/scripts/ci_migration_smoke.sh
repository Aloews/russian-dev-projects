#!/usr/bin/env bash
set -Eeuo pipefail

DB_NAME="${USH_MIGRATION_TEST_DB_NAME:-ush_migration_test}"
PGHOST="${PGHOST:-127.0.0.1}"
PGPORT="${PGPORT:-5432}"
PGUSER="${PGUSER:-ush}"
: "${PGPASSWORD:?PGPASSWORD is required for the ephemeral CI PostgreSQL service}"

if [[ ! "$DB_NAME" =~ ^[a-z][a-z0-9_]{0,62}$ ]]; then
  echo "Invalid migration test database name." >&2
  exit 2
fi

EXISTS="$(psql -X -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d postgres -Atqc "SELECT 1 FROM pg_database WHERE datname = '$DB_NAME'")"
if [[ "$EXISTS" == "1" ]]; then
  echo "Refusing to reuse existing migration test database: $DB_NAME" >&2
  exit 2
fi
createdb -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" "$DB_NAME"
export USH_DATABASE_URL="$(python3 - <<'PY'
import os
from urllib.parse import quote
user = quote(os.environ["PGUSER"], safe="")
password = quote(os.environ["PGPASSWORD"], safe="")
host = os.environ["PGHOST"]
port = os.environ["PGPORT"]
database = os.environ["USH_MIGRATION_TEST_DB_NAME"]
print(f"postgresql+asyncpg://{user}:{password}@{host}:{port}/{database}")
PY
)"

# Run twice to verify that an already-upgraded schema is a safe no-op.
alembic upgrade head
alembic upgrade head
alembic current
