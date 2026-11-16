#!/usr/bin/env bash
set -Eeuo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HUB_ROOT="$REPO_ROOT/universal-service-hub"

command -v rustup >/dev/null 2>&1 || {
  echo "rustup is required by the configured Rust devcontainer image." >&2
  exit 1
}
command -v cargo >/dev/null 2>&1 || {
  echo "cargo is required by the configured Rust devcontainer image." >&2
  exit 1
}

rustup component add rustfmt clippy
cargo metadata --manifest-path "$HUB_ROOT/Cargo.toml" --no-deps --format-version 1 >/dev/null
cargo fetch --manifest-path "$HUB_ROOT/Cargo.toml"

git config --global fetch.prune true

cat <<'EOF'
Universal Service Hub development environment initialized.

Next:
  cd universal-service-hub
  bash scripts/dev-up.sh

The local startup script generates development-only credentials in
universal-service-hub/.env.local and starts the Hub plus PostgreSQL.
It does not deploy or connect to production.
EOF
