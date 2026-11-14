#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="${1:?usage: product_factory_verify.sh <project-root> <report-path>}"
REPORT="${2:?usage: product_factory_verify.sh <project-root> <report-path>}"
ROOT="$(cd "$ROOT" && pwd)"
mkdir -p "$(dirname "$REPORT")"
: > "$REPORT"
FAILURES=0
GATES=0

run_gate() {
  local label="$1"
  shift
  GATES=$((GATES + 1))
  {
    printf '\n## %s\n' "$label"
    printf '$'
    printf ' %q' "$@"
    printf '\n'
  } >> "$REPORT"
  set +e
  "$@" >> "$REPORT" 2>&1
  local rc=$?
  set -e
  printf '\nexit_code=%s\n' "$rc" >> "$REPORT"
  if [[ "$rc" -ne 0 ]]; then
    FAILURES=$((FAILURES + 1))
  fi
}

if [[ -f "$ROOT/Cargo.toml" ]]; then
  run_gate "Rust formatting" bash -c 'cd "$1" && cargo fmt --all -- --check' _ "$ROOT"
  run_gate "Rust Clippy" bash -c 'cd "$1" && cargo clippy --workspace --all-targets --locked -- -D warnings' _ "$ROOT"
  run_gate "Rust tests" bash -c 'cd "$1" && cargo test --workspace --locked' _ "$ROOT"
  run_gate "Rust release build" bash -c 'cd "$1" && cargo build --release --workspace --locked' _ "$ROOT"
fi

if [[ -f "$ROOT/package-lock.json" && -f "$ROOT/package.json" ]]; then
  run_gate "Node dependency install" bash -c 'cd "$1" && npm ci' _ "$ROOT"
  run_gate "Node dependency audit" bash -c 'cd "$1" && npm audit --audit-level=low' _ "$ROOT"
  if node -e 'process.exit(require(process.argv[1]).scripts?.lint ? 0 : 1)' "$ROOT/package.json"; then
    run_gate "Frontend lint" bash -c 'cd "$1" && npm run lint' _ "$ROOT"
  fi
  if node -e 'process.exit(require(process.argv[1]).scripts?.test ? 0 : 1)' "$ROOT/package.json"; then
    run_gate "Frontend tests" bash -c 'cd "$1" && npm run test --if-present' _ "$ROOT"
  fi
  if node -e 'process.exit(require(process.argv[1]).scripts?.build ? 0 : 1)' "$ROOT/package.json"; then
    run_gate "Frontend production build" bash -c 'cd "$1" && npm run build' _ "$ROOT"
  fi
  if [[ -f "$ROOT/playwright.config.ts" || -f "$ROOT/playwright.config.js" || -f "$ROOT/playwright.config.mts" || -f "$ROOT/playwright.config.mjs" ]]; then
    run_gate "Playwright Chromium installation" bash -c 'cd "$1" && npx playwright install --with-deps chromium' _ "$ROOT"
    if node -e 'process.exit(require(process.argv[1]).scripts?.["test:e2e"] ? 0 : 1)' "$ROOT/package.json"; then
      run_gate "Browser end-to-end tests" bash -c 'cd "$1" && npm run test:e2e' _ "$ROOT"
    else
      run_gate "Browser end-to-end tests" bash -c 'cd "$1" && npx playwright test' _ "$ROOT"
    fi
  else
    GATES=$((GATES + 1))
    FAILURES=$((FAILURES + 1))
    printf '\n## Browser end-to-end tests\nMissing Playwright configuration; critical user journeys are not verified.\nexit_code=2\n' >> "$REPORT"
  fi
fi

if [[ -f "$ROOT/services/platform-api/Dockerfile" ]]; then
  run_gate "Production API Docker build" docker build --tag product-factory-api:verification --file "$ROOT/services/platform-api/Dockerfile" "$ROOT"
fi

if [[ "$GATES" -eq 0 ]]; then
  echo "No supported Cargo workspace or npm lockfile found; refusing to report success." >> "$REPORT"
  exit 2
fi

{
  printf '\n## Verification summary\n'
  printf 'Gates: %s\n' "$GATES"
  printf 'Failed: %s\n' "$FAILURES"
} >> "$REPORT"

if [[ "$FAILURES" -ne 0 ]]; then
  exit 1
fi
