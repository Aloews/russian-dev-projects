#!/usr/bin/env bash
set -Eeuo pipefail

: "${RAILWAY_API_TOKEN:?RAILWAY_API_TOKEN is required}"
: "${RAILWAY_PROJECT_ID:?RAILWAY_PROJECT_ID is required}"
: "${RAILWAY_ENVIRONMENT_ID:?RAILWAY_ENVIRONMENT_ID is required}"
: "${TARGET_REPO:?TARGET_REPO is required}"
: "${TARGET_BRANCH:?TARGET_BRANCH is required}"
: "${BASE_BRANCH:?BASE_BRANCH is required}"
: "${OBJECTIVE:?OBJECTIVE is required}"
: "${GITHUB_RUN_ID:?GITHUB_RUN_ID is required}"

[[ "$TARGET_REPO" =~ ^Aloews/[A-Za-z0-9._-]+$ ]] || { echo "TARGET_REPO must be an owned Aloews repository" >&2; exit 2; }
for value in "$TARGET_BRANCH" "$BASE_BRANCH"; do
  [[ "$value" =~ ^[A-Za-z0-9._/-]+$ ]] || { echo "Invalid branch name" >&2; exit 2; }
done

API="https://backboard.railway.com/graphql/v2"
REPORT_DIR="${REPORT_DIR:-artifacts/product-factory}"
mkdir -p "$REPORT_DIR"

gql() {
  curl --silent --show-error --fail-with-body --request POST "$API" \
    --header "Authorization: Bearer $RAILWAY_API_TOKEN" \
    --header "Content-Type: application/json" --data "$1"
}

dispatch_and_wait() {
  local role="$1" model="$2" effort="$3" prompt="$4"
  local identity="$role"
  case "$role" in
    architect|implementation|repair) identity="fable-lead" ;;
    architecture-review|code-review-*) identity="opus-review" ;;
    security-review|qa-*) identity="sonnet-review" ;;
    research) identity="haiku-research" ;;
  esac
  local key="$GITHUB_RUN_ID-$role"
  local conversation="product-factory:$TARGET_REPO:$identity"
  local mutation='mutation($input: CloudAgentTaskDispatchInput!) { cloudAgentTaskDispatch(input: $input) { taskId cloudAgentId sessionId status } }'
  local input response task_id agent_id session_id
  input=$(jq -n --arg project "$RAILWAY_PROJECT_ID" --arg environment "$RAILWAY_ENVIRONMENT_ID" \
    --arg prompt "$prompt" --arg model "$model" --arg effort "$effort" \
    --arg repo "$TARGET_REPO" --arg branch "$TARGET_BRANCH" --arg conversation "$conversation" \
    --arg agent "pf-$identity" --arg key "$key" \
    '{projectId:$project,environmentId:$environment,prompt:$prompt,model:$model,reasoningEffort:$effort,source:{repo:$repo,branch:$branch},conversationKey:$conversation,agentName:$agent,allowCreate:true,interactive:false,idempotencyKey:$key,externalRef:$key,metadata:{repository:$repo,branch:$branch}}')
  response=$(gql "$(jq -n --arg query "$mutation" --argjson input "$input" '{query:$query,variables:{input:$input}}')")
  jq -e '.errors and (.errors|length>0)' >/dev/null <<<"$response" && { jq -c '.errors' <<<"$response" >&2; return 1; }
  task_id=$(jq -er '.data.cloudAgentTaskDispatch.taskId' <<<"$response")
  agent_id=$(jq -er '.data.cloudAgentTaskDispatch.cloudAgentId' <<<"$response")
  session_id=$(jq -er '.data.cloudAgentTaskDispatch.sessionId' <<<"$response")
  local query='query($cloudAgentId:String!,$sessionId:String!,$taskId:String){cloudAgentTask(cloudAgentId:$cloudAgentId,sessionId:$sessionId,taskId:$taskId){taskId status text error model reasoningEffort completedAt}}'
  local vars poll task status
  vars=$(jq -n --arg agent "$agent_id" --arg session "$session_id" --arg task "$task_id" '{cloudAgentId:$agent,sessionId:$session,taskId:$task}')
  for attempt in $(seq 1 210); do
    poll=$(gql "$(jq -n --arg query "$query" --argjson variables "$vars" '{query:$query,variables:$variables}')")
    jq -e '.errors and (.errors|length>0)' >/dev/null <<<"$poll" && { jq -c '.errors' <<<"$poll" >&2; return 1; }
    task=$(jq -c '.data.cloudAgentTask' <<<"$poll")
    status=$(jq -r '.status // "UNKNOWN" | ascii_upcase' <<<"$task")
    case "$status" in
      COMPLETED)
        LAST_RESULT=$(jq -r '.text // ""' <<<"$task")
        printf '%s\n' "$LAST_RESULT" > "$REPORT_DIR/$role.md"
        printf '%s\n' "$task" > "$REPORT_DIR/$role.json"
        return 0 ;;
      FAILED|CANCELLED|AWAITING_INPUT)
        printf '%s\n' "$task" > "$REPORT_DIR/$role.json"
        echo "Agent task $role ended with status $status" >&2
        return 1 ;;
      PENDING|RUNNING) sleep 10 ;;
      *) echo "Unexpected Railway task status: $status" >&2; return 1 ;;
    esac
  done
  echo "Timed out waiting for $role task $task_id" >&2
  return 1
}

repo_context="Repository: $TARGET_REPO
Working branch: $TARGET_BRANCH
Base branch: $BASE_BRANCH
Objective: $OBJECTIVE
Fetch latest remote state first. Read AGENTS.md, project map, open PRs, issues, CI and deployment config. Never fabricate checks or data. Keep changes on the target branch. Never merge or deploy production."

dispatch_and_wait architect claude-fable-5-1 max "$repo_context

Inspect only. Produce a dependency-aware plan, acceptance criteria, file scope, risks and validation commands. Do not edit files."
ARCHITECT_RESULT="$LAST_RESULT"

dispatch_and_wait architecture-review claude-opus-5-5 high "$repo_context

Independently challenge this plan; inspect repository evidence; do not edit.
$ARCHITECT_RESULT"
ARCH_REVIEW="$LAST_RESULT"

dispatch_and_wait security-review claude-sonnet-5-5 high "$repo_context

Independently inspect auth, tenant isolation, PII, secrets, supply chain, SSRF, abuse and legal boundaries. Do not edit.
$ARCHITECT_RESULT"
SECURITY_REVIEW="$LAST_RESULT"

dispatch_and_wait research claude-haiku-5-5 medium "$repo_context

Verify material vendor/API/framework claims against official primary sources. Return URLs, exact facts, confidence and unknowns. Do not edit."
RESEARCH_RESULT="$LAST_RESULT"

dispatch_and_wait implementation claude-fable-5-1 max "$repo_context

Implement the objective in small testable increments. Reconcile plan and reviews. Only you may edit. Add regression tests, run checks, inspect diff, update project map, push target branch and open/update PR against base. Never merge/deploy. Report exact SHA, PR, commands, outcomes and blockers.

PLAN:
$ARCHITECT_RESULT

ARCHITECTURE REVIEW:
$ARCH_REVIEW

SECURITY REVIEW:
$SECURITY_REVIEW

RESEARCH:
$RESEARCH_RESULT"
IMPLEMENTATION_RESULT="$LAST_RESULT"

FINAL_ACCEPTED=0
for cycle in 1 2; do
  dispatch_and_wait "code-review-$cycle" claude-opus-5-5 high "$repo_context

Read-only independent review of exact latest branch HEAD and full diff against $BASE_BRANCH. Return severity, file/line, evidence and required fixes. First line exactly BLOCKING_FINDINGS: N. Zero only if no blocking/high findings."
  CODE_REVIEW="$LAST_RESULT"

  dispatch_and_wait "qa-$cycle" claude-sonnet-5-5 high "$repo_context

Read-only QA of exact latest commit. Run full safe documented checks and inspect CI on exact SHA. Missing/stale checks are not passes. First line exactly OVERALL: PASS or OVERALL: FAIL. Give evidence per gate."
  QA_RESULT="$LAST_RESULT"

  if grep -Eiq '^BLOCKING_FINDINGS:[[:space:]]*0([[:space:]]|$)' <<<"$CODE_REVIEW" && grep -Eiq '^OVERALL:[[:space:]]*PASS([[:space:]]|$)' <<<"$QA_RESULT"; then
    FINAL_ACCEPTED=1
    break
  fi
  if [[ "$cycle" -eq 2 ]]; then
    break
  fi
  dispatch_and_wait repair claude-fable-5-1 max "$repo_context

Repair all findings below, add regression tests, run affected checks, inspect diff and push same branch/update PR. No merge/deploy.
CODE REVIEW:
$CODE_REVIEW
QA:
$QA_RESULT
Previous implementation:
$IMPLEMENTATION_RESULT"
  IMPLEMENTATION_RESULT="$LAST_RESULT"
done

{
  printf '%s\n' "Product Factory run $GITHUB_RUN_ID" "Repository: $TARGET_REPO" "Branch: $TARGET_BRANCH" \
    "Objective: $OBJECTIVE" "Accepted: $FINAL_ACCEPTED" "" "Implementation:" "$IMPLEMENTATION_RESULT" "" \
    "Review:" "${CODE_REVIEW:-not run}" "" "QA:" "${QA_RESULT:-not run}"
} > "$REPORT_DIR/summary.md"

if [[ "$FINAL_ACCEPTED" -ne 1 ]]; then
  echo "Independent review and QA gates not passed; PR remains unmerged." >&2
  exit 1
fi
