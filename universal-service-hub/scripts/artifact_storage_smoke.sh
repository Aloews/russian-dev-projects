#!/usr/bin/env bash
set -Eeuo pipefail

: "${USH_ARTIFACT_SMOKE_DATABASE_URL:?USH_ARTIFACT_SMOKE_DATABASE_URL is required}"
: "${USH_ARTIFACT_SMOKE_API_TOKEN:?USH_ARTIFACT_SMOKE_API_TOKEN is required}"

image="${USH_ARTIFACT_SMOKE_IMAGE:-universal-service-hub:test}"
silo_image="${USH_ARTIFACT_SMOKE_SILO_IMAGE:-docker.io/pgsty/silo:RELEASE.2026-11-09T00-00-00Z}"
silo_name="ush-artifact-silo-smoke"
api_name="ush-artifact-api-smoke"
silo_endpoint="http://127.0.0.1:19000"
base_url="http://127.0.0.1:18001"
bucket="ush-artifacts"
access_key="ush-artifact-ci"
secret_key="ush-artifact-ci-secret-2026-11-18"
prefix="ush-artifacts-v1"
suffix="${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}-$$"
project_id="artifact-smoke-${suffix}"
other_project_id="artifact-smoke-other-${suffix}"
job_key="artifact-smoke-${suffix}"
tmp="$(mktemp -d)"
api_started=0
silo_started=0

redact_docker_logs() {
  local container="$1"
  docker logs --tail 200 "$container" 2>&1 |
    USH_ARTIFACT_SMOKE_DATABASE_URL="$USH_ARTIFACT_SMOKE_DATABASE_URL" \
    USH_ARTIFACT_SMOKE_API_TOKEN="$USH_ARTIFACT_SMOKE_API_TOKEN" \
    USH_ARTIFACT_ACCESS_KEY_ID="$access_key" \
    USH_ARTIFACT_SECRET_ACCESS_KEY="$secret_key" \
    python3 -c '
import os, sys
text = sys.stdin.read()
secrets = [
    os.environ.get("USH_ARTIFACT_SMOKE_DATABASE_URL", ""),
    os.environ.get("USH_ARTIFACT_SMOKE_API_TOKEN", ""),
    os.environ.get("USH_ARTIFACT_ACCESS_KEY_ID", ""),
    os.environ.get("USH_ARTIFACT_SECRET_ACCESS_KEY", ""),
]
for secret in sorted((value for value in secrets if value), key=len, reverse=True):
    text = text.replace(secret, "[REDACTED]")
sys.stdout.write(text[-30000:])
'
}

cleanup() {
  status=$?
  if [[ "$status" -ne 0 ]]; then
    echo "Artifact storage smoke failed; sanitized service logs follow." >&2
    if [[ "$api_started" == "1" ]]; then
      echo "Artifact API container state:" >&2
      docker inspect --format 'status={{.State.Status}} exit={{.State.ExitCode}}' "$api_name" >&2 || true
      echo "Artifact API startup logs (redacted):" >&2
      redact_docker_logs "$api_name" >&2 || true
    fi
    if [[ "$silo_started" == "1" ]]; then
      echo "S3-compatible test store logs (redacted):" >&2
      redact_docker_logs "$silo_name" >&2 || true
    fi
  fi
  docker rm -f "$api_name" "$silo_name" >/dev/null 2>&1 || true
  rm -rf "$tmp"
  exit "$status"
}
trap cleanup EXIT

docker run -d --rm --name "$silo_name" \
  -p 127.0.0.1:19000:9000 \
  -e MINIO_ROOT_USER="$access_key" \
  -e MINIO_ROOT_PASSWORD="$secret_key" \
  "$silo_image" server /data >/dev/null
silo_started=1

ready=0
for _ in $(seq 1 60); do
  if curl -fsS --max-time 2 "$silo_endpoint/minio/health/ready" >/dev/null; then
    ready=1
    break
  fi
  sleep 1
done
if [[ "$ready" != "1" ]]; then
  echo "S3-compatible test store did not become ready." >&2
  exit 1
fi

docker exec "$silo_name" mcli alias set local http://127.0.0.1:9000 "$access_key" "$secret_key" >/dev/null
docker exec "$silo_name" mcli mb "local/$bucket" >/dev/null

api_started=1
docker run -d --name "$api_name" --network host \
  -e USH_DATABASE_URL="$USH_ARTIFACT_SMOKE_DATABASE_URL" \
  -e USH_API_TOKEN="$USH_ARTIFACT_SMOKE_API_TOKEN" \
  -e USH_RATE_LIMIT_PER_MINUTE=200 \
  -e USH_ENVIRONMENT=development \
  -e USH_SECRET_BACKEND=env \
  -e USH_ARTIFACT_BUCKET="$bucket" \
  -e USH_ARTIFACT_REGION=us-east-1 \
  -e USH_ARTIFACT_ENDPOINT="$silo_endpoint" \
  -e USH_ARTIFACT_ACCESS_KEY_ID="$access_key" \
  -e USH_ARTIFACT_SECRET_ACCESS_KEY="$secret_key" \
  -e USH_ARTIFACT_PREFIX="$prefix" \
  -e USH_ARTIFACT_MAX_BYTES=20971520 \
  -e HOST=127.0.0.1 \
  -e PORT=18001 \
  "$image" >/dev/null
api_started=1

ready=0
for _ in $(seq 1 60); do
  if curl -fsS --max-time 2 "$base_url/readyz" >/dev/null; then
    ready=1
    break
  fi
  sleep 1
done
if [[ "$ready" != "1" ]]; then
  echo "Artifact API did not become ready; container is preserved for startup diagnostics." >&2
  exit 1
fi

admin_headers=(-H "Authorization: Bearer $USH_ARTIFACT_SMOKE_API_TOKEN" -H "Content-Type: application/json")
project_body="$(jq -n --arg id "$project_id" '{id:$id,name:"Artifact integration smoke",allowed_capabilities:["ai.chat"],allowed_provider_ids:[]}')"
other_project_body="$(jq -n --arg id "$other_project_id" '{id:$id,name:"Artifact tenant isolation smoke",allowed_capabilities:["ai.chat"],allowed_provider_ids:[]}')"
curl -fsS -X POST "$base_url/v1/projects" "${admin_headers[@]}" -d "$project_body" >/dev/null
curl -fsS -X POST "$base_url/v1/projects" "${admin_headers[@]}" -d "$other_project_body" >/dev/null

project_token_response="$(curl -fsS -X POST "$base_url/v1/projects/$project_id/tokens" "${admin_headers[@]}" -d '{"name":"artifact-smoke-operator","role":"operator"}')"
project_token="$(jq -er '.token' <<<"$project_token_response")"
viewer_token_response="$(curl -fsS -X POST "$base_url/v1/projects/$project_id/tokens" "${admin_headers[@]}" -d '{"name":"artifact-smoke-viewer","role":"viewer"}')"
viewer_token="$(jq -er '.token' <<<"$viewer_token_response")"
other_token_response="$(curl -fsS -X POST "$base_url/v1/projects/$other_project_id/tokens" "${admin_headers[@]}" -d '{"name":"artifact-smoke-other-operator","role":"operator"}')"
other_token="$(jq -er '.token' <<<"$other_token_response")"

job_response="$(curl -fsS -X POST "$base_url/v1/projects/$project_id/jobs" \
  -H "Authorization: Bearer $project_token" -H "Content-Type: application/json" \
  -d "$(jq -n --arg key "$job_key" '{idempotency_key:$key,job_type:"system_noop",agent_role:"integrator"}')")"
job_id="$(jq -er '.job_id' <<<"$job_response")"

payload="USH artifact API integration payload; exact bytes must round-trip."
printf '%s' "$payload" > "$tmp/payload"
expected_sha="$(sha256sum "$tmp/payload" | awk '{print $1}')"
upload_response="$(curl -fsS -X POST "$base_url/v1/projects/$project_id/jobs/$job_id/artifacts" \
  -H "Authorization: Bearer $project_token" \
  -H "Content-Type: application/octet-stream" \
  -H "X-Artifact-Type: test-result" \
  --data-binary "@$tmp/payload")"
artifact_id="$(jq -er '.artifact_id' <<<"$upload_response")"
actual_sha="$(jq -er '.sha256' <<<"$upload_response")"
actual_size="$(jq -er '.size_bytes' <<<"$upload_response")"
test "$actual_sha" = "$expected_sha"
test "$actual_size" = "$(wc -c < "$tmp/payload" | tr -d ' ')"
test "$(jq -er '.content_type' <<<"$upload_response")" = "application/octet-stream"

download_status="$(curl -sS -o "$tmp/download" -D "$tmp/headers" -w '%{http_code}' \
  -H "Authorization: Bearer $project_token" \
  "$base_url/v1/projects/$project_id/artifacts/$artifact_id")"
test "$download_status" = "200"
cmp -s "$tmp/payload" "$tmp/download"
grep -qi "^x-artifact-sha256: $expected_sha" "$tmp/headers"
grep -qi '^content-disposition: attachment' "$tmp/headers"
grep -qi '^x-content-type-options: nosniff' "$tmp/headers"
grep -qi '^cache-control: private, no-store' "$tmp/headers"
grep -qi '^content-type: application/octet-stream' "$tmp/headers"

unauthorized_status="$(curl -sS -o "$tmp/unauthorized" -w '%{http_code}' "$base_url/v1/projects/$project_id/artifacts/$artifact_id")"
test "$unauthorized_status" = "401"
viewer_status="$(curl -sS -o "$tmp/viewer" -w '%{http_code}' \
  -H "Authorization: Bearer $viewer_token" "$base_url/v1/projects/$project_id/artifacts/$artifact_id")"
test "$viewer_status" = "403"
cross_project_status="$(curl -sS -o "$tmp/cross-project" -w '%{http_code}' \
  -H "Authorization: Bearer $other_token" "$base_url/v1/projects/$project_id/artifacts/$artifact_id")"
test "$cross_project_status" = "403"

invalid_type_status="$(curl -sS -o "$tmp/invalid-type" -w '%{http_code}' -X POST \
  "$base_url/v1/projects/$project_id/jobs/$job_id/artifacts" \
  -H "Authorization: Bearer $project_token" -H "Content-Type: application/octet-stream" \
  -H "X-Artifact-Type: ../invalid" --data-binary "@$tmp/payload")"
test "$invalid_type_status" = "422"
invalid_content_status="$(curl -sS -o "$tmp/invalid-content" -w '%{http_code}' -X POST \
  "$base_url/v1/projects/$project_id/jobs/$job_id/artifacts" \
  -H "Authorization: Bearer $project_token" -H "Content-Type: application/" \
  -H "X-Artifact-Type: test-result" --data-binary "@$tmp/payload")"
test "$invalid_content_status" = "415"
task_mismatch_status="$(curl -sS -o "$tmp/task-mismatch" -w '%{http_code}' -X POST \
  "$base_url/v1/projects/$project_id/jobs/$job_id/artifacts" \
  -H "Authorization: Bearer $project_token" -H "Content-Type: application/octet-stream" \
  -H "X-Artifact-Type: test-result" -H "X-Task-ID: 00000000-0000-4000-8000-000000000001" \
  --data-binary "@$tmp/payload")"
test "$task_mismatch_status" = "409"

head -c 2097153 /dev/zero > "$tmp/oversize"
oversize_status="$(curl -sS -o "$tmp/oversize-response" -w '%{http_code}' -X POST \
  "$base_url/v1/projects/$project_id/jobs/$job_id/artifacts" \
  -H "Authorization: Bearer $project_token" -H "Content-Type: application/octet-stream" \
  -H "X-Artifact-Type: test-result" --data-binary "@$tmp/oversize")"
test "$oversize_status" = "413"

object_key="$prefix/$project_id/jobs/$job_id/$artifact_id"
printf 'corrupted artifact bytes' > "$tmp/corrupt"
docker cp "$tmp/corrupt" "$silo_name:/tmp/corrupt" >/dev/null
docker exec "$silo_name" mcli cp /tmp/corrupt "local/$bucket/$object_key" >/dev/null
integrity_status="$(curl -sS -o "$tmp/integrity" -w '%{http_code}' \
  -H "Authorization: Bearer $project_token" \
  "$base_url/v1/projects/$project_id/artifacts/$artifact_id")"
test "$integrity_status" = "500"
test "$(jq -er '.error' "$tmp/integrity")" = "artifact_integrity_check_failed"

echo "S3-compatible artifact API smoke passed: upload/download SHA-256, private headers, project isolation, validation, size bounds and integrity mismatch."
