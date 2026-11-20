#!/usr/bin/env bash
set -euo pipefail
compose() { docker compose --env-file .env.local -f compose.yml "$@"; }
set -a
. ./.env.local
set +a
if [[ -z "$USH_API_TOKEN" || -z "$USH_WORKER_SMOKE_BASE_URL" ]]; then echo "required smoke env missing" >&2; exit 1; fi
worker_password=ush-worker-ci-test-password
if printenv USH_WORKER_TEST_DB_PASSWORD >/dev/null 2>&1; then worker_password="$USH_WORKER_TEST_DB_PASSWORD"; fi
if [[ ! "$worker_password" =~ ^[A-Za-z0-9._-]{20,100}$ ]]; then
  echo "worker smoke password must be 20-100 safe ASCII characters" >&2
  exit 1
fi
suffix=local
if printenv GITHUB_RUN_ID >/dev/null 2>&1; then suffix="$GITHUB_RUN_ID"; fi
project_id="worker-smoke-$suffix"
idempotency_key="worker-smoke-$suffix"
crash_container=""
restart_container=""
cleanup() {
  docker rm -f "$crash_container" "$restart_container" >/dev/null 2>&1 || true
  compose exec -T postgres psql -U ush -d ush -v ON_ERROR_STOP=1 -c "DROP OWNED BY ush_worker; DROP ROLE IF EXISTS ush_worker;" >/dev/null 2>&1 || true
}
trap cleanup EXIT

compose exec -T postgres psql -U ush -d ush -v ON_ERROR_STOP=1 -c "DROP OWNED BY ush_worker; DROP ROLE IF EXISTS ush_worker;" >/dev/null 2>&1 || true
compose exec -T postgres psql -U ush -d ush -v ON_ERROR_STOP=1 -c "CREATE ROLE ush_worker LOGIN PASSWORD '$worker_password';"
compose exec -T postgres psql -U ush -d ush -v ON_ERROR_STOP=1 <<'SQL'
GRANT CONNECT ON DATABASE ush TO ush_worker;
GRANT USAGE ON SCHEMA public TO ush_worker;
GRANT SELECT (id, project_id, task_id, job_type, status, priority, attempt_count, max_attempts, available_at, created_at, lease_token, lease_expires_at) ON execution_jobs TO ush_worker;
GRANT UPDATE (status, lease_owner, lease_token, lease_expires_at, heartbeat_at, attempt_count, available_at, last_error_code, finished_at, updated_at) ON execution_jobs TO ush_worker;
GRANT SELECT (project_id, job_id, attempt_number, lease_token, status, started_at) ON execution_attempts TO ush_worker;
GRANT INSERT ON execution_attempts TO ush_worker;
GRANT UPDATE (status, finished_at, duration_ms, error_code, result_metadata) ON execution_attempts TO ush_worker;
SQL

if compose exec -T postgres env PGPASSWORD="$worker_password" psql -h postgres -U ush_worker -d ush -c "SELECT payload FROM execution_jobs LIMIT 1" >/dev/null 2>&1; then
  echo "worker role unexpectedly has access to job payloads" >&2
  exit 1
fi
if compose exec -T postgres env PGPASSWORD="$worker_password" psql -h postgres -U ush_worker -d ush -c "SELECT id FROM projects LIMIT 1" >/dev/null 2>&1; then
  echo "worker role unexpectedly has access to the projects table" >&2
  exit 1
fi

project_payload=$(jq -cn --arg id "$project_id" --arg name "Worker runtime smoke $suffix" '{id:$id,name:$name,allowed_capabilities:["ai.chat"],allowed_provider_ids:[]}')
project_response=$(curl -sS -w '\n%{http_code}' -X POST "$USH_WORKER_SMOKE_BASE_URL/v1/projects" -H "Authorization: Bearer $USH_API_TOKEN" -H "Content-Type: application/json" --data-binary "$project_payload")
project_status="${project_response##*$'\n'}"
project_body="${project_response%$'\n'*}"
if [[ ! "$project_status" =~ ^2 ]]; then echo "project creation failed: HTTP $project_status: $project_body" >&2; exit 1; fi
curl -fsS -X POST "$USH_WORKER_SMOKE_BASE_URL/v1/projects/$project_id/jobs" -H "Authorization: Bearer $USH_API_TOKEN" -H "Content-Type: application/json" -d "{\"idempotency_key\":\"$idempotency_key\",\"job_type\":\"system_noop\",\"agent_role\":\"integrator\"}" >/dev/null

job_id=$(compose exec -T postgres psql -U ush -d ush -tA -v ON_ERROR_STOP=1 -c "SELECT id FROM execution_jobs WHERE project_id='$project_id' AND idempotency_key='$idempotency_key'")
artifact_unauthorized_status=$(curl -sS -o /dev/null -w "%{http_code}" -X POST "$USH_WORKER_SMOKE_BASE_URL/v1/projects/$project_id/jobs/$job_id/artifacts" -H "Content-Type: text/plain" -H "X-Artifact-Type: test" --data-binary "unauthorized")
if [[ "$artifact_unauthorized_status" != "401" && "$artifact_unauthorized_status" != "403" ]]; then
  echo "artifact upload must require project authorization, got HTTP $artifact_unauthorized_status" >&2
  exit 1
fi
artifact_disabled_status=$(curl -sS -o /dev/null -w "%{http_code}" -X POST "$USH_WORKER_SMOKE_BASE_URL/v1/projects/$project_id/jobs/$job_id/artifacts" -H "Authorization: Bearer $USH_API_TOKEN" -H "Content-Type: text/plain" -H "X-Artifact-Type: test" --data-binary "artifact-storage-disabled")
if [[ "$artifact_disabled_status" != "503" ]]; then
  echo "artifact API must fail closed without configured storage, got HTTP $artifact_disabled_status" >&2
  exit 1
fi


# Worker artifact endpoint uses the lease token as a one-job capability and
# never asks the worker for the project/admin bearer token or S3 credentials.
artifact_job_key="worker-artifact-smoke-$suffix"
artifact_job_payload=$(jq -cn --arg key "$artifact_job_key" '{idempotency_key:$key,job_type:"system_noop",agent_role:"integrator",priority:1000}')
curl -fsS -X POST "$USH_WORKER_SMOKE_BASE_URL/v1/projects/$project_id/jobs" \
  -H "Authorization: Bearer $USH_API_TOKEN" -H "Content-Type: application/json" \
  --data-binary "$artifact_job_payload" >/dev/null
artifact_job_id=$(compose exec -T postgres psql -U ush -d ush -tA -v ON_ERROR_STOP=1 -c "SELECT id FROM execution_jobs WHERE project_id='$project_id' AND idempotency_key='$artifact_job_key'")
artifact_claim=$(curl -fsS -X POST "$USH_WORKER_SMOKE_BASE_URL/v1/projects/$project_id/workers/claim" \
  -H "Authorization: Bearer $USH_API_TOKEN" -H "Content-Type: application/json" \
  -d '{"worker_id":"artifact-smoke-worker","lease_seconds":30}')
artifact_lease_token=$(printf '%s' "$artifact_claim" | jq -r '.job.lease_token')
test -n "$artifact_lease_token" && test "$artifact_lease_token" != "null"
wrong_lease_status=$(curl -sS -o /dev/null -w "%{http_code}" -X POST \
  "$USH_WORKER_SMOKE_BASE_URL/v1/worker/projects/$project_id/jobs/$artifact_job_id/artifacts" \
  -H "X-USH-Worker-ID: artifact-smoke-worker" -H "X-USH-Lease-Token: 00000000-0000-4000-8000-000000000000" \
  -H "X-Artifact-Idempotency-Key: artifact-smoke-$suffix" -H "X-Artifact-Type: test" \
  -H "Content-Type: text/plain" --data-binary "artifact")
if [[ "$wrong_lease_status" != "409" ]]; then echo "invalid worker lease must be rejected, got HTTP $wrong_lease_status" >&2; exit 1; fi
valid_lease_no_store_status=$(curl -sS -o /dev/null -w "%{http_code}" -X POST \
  "$USH_WORKER_SMOKE_BASE_URL/v1/worker/projects/$project_id/jobs/$artifact_job_id/artifacts" \
  -H "X-USH-Worker-ID: artifact-smoke-worker" -H "X-USH-Lease-Token: $artifact_lease_token" \
  -H "X-Artifact-Idempotency-Key: artifact-smoke-$suffix" -H "X-Artifact-Type: test" \
  -H "Content-Type: text/plain" --data-binary "artifact")
if [[ "$valid_lease_no_store_status" != "503" ]]; then echo "valid lease without storage must fail closed with 503, got HTTP $valid_lease_no_store_status" >&2; exit 1; fi
fail_payload=$(jq -cn --arg worker "artifact-smoke-worker" --arg token "$artifact_lease_token" '{worker_id:$worker,lease_token:$token,error_code:"smoke_cleanup",retryable:false}')
curl -fsS -X POST "$USH_WORKER_SMOKE_BASE_URL/v1/projects/$project_id/jobs/$artifact_job_id/fail" \
  -H "Authorization: Bearer $USH_API_TOKEN" -H "Content-Type: application/json" \
  --data-binary "$fail_payload" >/dev/null

crash_container=$(compose run -d --no-deps -e "USH_WORKER_DATABASE_URL=postgres://ush_worker:$worker_password@postgres:5432/ush" -e "USH_WORKER_ID=ci-crash-worker" -e "USH_WORKER_CONCURRENCY=1" -e "USH_WORKER_POLL_MS=200" -e "USH_WORKER_LEASE_SECONDS=10" -e "USH_WORKER_TEST_PAUSE_AFTER_CLAIM_MS=30000" -e "USH_WORKER_JOB_TYPES=system_noop" worker)

job_status=""
for _ in $(seq 1 30); do
  job_status=$(compose exec -T postgres psql -U ush -d ush -tA -v ON_ERROR_STOP=1 -c "SELECT status FROM execution_jobs WHERE project_id='$project_id' AND idempotency_key='$idempotency_key'")
  if [[ "$job_status" == "leased" ]]; then break; fi
  sleep 1
done
if [[ "$job_status" != "leased" ]]; then docker logs "$crash_container" >&2 || true; echo "worker did not claim the smoke job" >&2; exit 1; fi

docker update --restart=no "$crash_container" >/dev/null
docker kill --signal=SIGKILL "$crash_container" >/dev/null
crash_exit=$(docker wait "$crash_container" || true)
if [[ "$crash_exit" != "137" ]]; then echo "expected SIGKILL exit 137, got $crash_exit" >&2; exit 1; fi
sleep 11

restart_container=$(compose run -d --no-deps -e "USH_WORKER_DATABASE_URL=postgres://ush_worker:$worker_password@postgres:5432/ush" -e "USH_WORKER_ID=ci-restarted-worker" -e "USH_WORKER_CONCURRENCY=1" -e "USH_WORKER_POLL_MS=200" -e "USH_WORKER_LEASE_SECONDS=10" -e "USH_WORKER_JOB_TYPES=system_noop" worker)

verified=false
for _ in $(seq 1 30); do
  verified=$(compose exec -T postgres psql -U ush -d ush -tA -v ON_ERROR_STOP=1 -c "SELECT (j.status='succeeded' AND (SELECT count(*) FROM execution_attempts a WHERE a.project_id=j.project_id AND a.job_id=j.id)=2 AND (SELECT count(*) FROM execution_attempts a WHERE a.project_id=j.project_id AND a.job_id=j.id AND a.status='lease_expired')=1 AND (SELECT count(*) FROM execution_attempts a WHERE a.project_id=j.project_id AND a.job_id=j.id AND a.status='succeeded')=1)::text FROM execution_jobs j WHERE j.project_id='$project_id' AND j.idempotency_key='$idempotency_key'")
  if [[ "$verified" == "true" ]]; then break; fi
  sleep 1
done
if [[ "$verified" != "true" ]]; then
  docker logs "$restart_container" >&2 || true
  compose exec -T postgres psql -U ush -d ush -c "SELECT status,attempt_count,last_error_code FROM execution_jobs WHERE project_id='$project_id' AND idempotency_key='$idempotency_key'" >&2 || true
  compose exec -T postgres psql -U ush -d ush -c "SELECT attempt_number,status,error_code FROM execution_attempts WHERE project_id='$project_id' ORDER BY attempt_number" >&2 || true
  echo "worker restart did not reconcile and complete the leased job" >&2
  exit 1
fi

docker kill --signal=SIGTERM "$restart_container" >/dev/null
docker wait "$restart_container" >/dev/null
docker logs "$restart_container" | grep -q "worker supervisor shutdown complete"
echo "worker crash/restart smoke passed: expired attempt recovered and job completed once"
