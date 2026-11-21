#!/usr/bin/env bash
set -Eeuo pipefail

REPOSITORY_PATH="${1:?usage: checkout_candidate_base.sh <repository-path> <commit-sha>}"
BASE_SHA="${2:?usage: checkout_candidate_base.sh <repository-path> <commit-sha>}"

if [[ ! "$BASE_SHA" =~ ^[0-9a-fA-F]{40}$ ]]; then
  echo "Invalid candidate base commit SHA." >&2
  exit 2
fi

git -C "$REPOSITORY_PATH" remote get-url origin >/dev/null
ORIGIN_URL="$(git -C "$REPOSITORY_PATH" remote get-url origin)"
if [[ "$ORIGIN_URL" =~ ^https://github\.com/[^/]+/[^/]+(\.git)?$ ]]; then
  TOKEN="${PRODUCT_FACTORY_GITHUB_TOKEN:-}"
  if [[ -z "$TOKEN" ]]; then
    echo "A GitHub token is required to fetch the candidate base commit." >&2
    exit 2
  fi
  AUTH="$(printf 'x-access-token:%s' "$TOKEN" | base64 -w0)"
  git -C "$REPOSITORY_PATH" \
    -c http.https://github.com/.extraheader="AUTHORIZATION: basic $AUTH" \
    fetch --no-tags --depth=1 origin "$BASE_SHA"
elif [[ "$ORIGIN_URL" =~ ^file:// ]]; then
  git -C "$REPOSITORY_PATH" fetch --no-tags --depth=1 origin "$BASE_SHA"
else
  echo "Unsupported target repository origin host." >&2
  exit 2
fi

git -C "$REPOSITORY_PATH" cat-file -e "$BASE_SHA^{commit}"
git -C "$REPOSITORY_PATH" cat-file -e "$BASE_SHA^{tree}"
git -C "$REPOSITORY_PATH" checkout --detach "$BASE_SHA"
[[ "$(git -C "$REPOSITORY_PATH" rev-parse HEAD)" == "$BASE_SHA" ]]