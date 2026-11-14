#!/usr/bin/env bash
set -Eeuo pipefail

: "\${RAILWAY_API_TOKEN:?RAILWAY_API_TOKEN is required}"
: "\${RAILWAY_PROJECT_ID:?RAILWAY_PROJECT_ID is required}"
: "\${RAILWAY_ENVIRONMENT_ID:?RAILWAY_ENVIRONMENT_ID is required}"
: "\${TARGET_REPO:?TARGET_REPO is required}"
: "\${TARGET_BRANCH:?TARGET_BRANCH is required}"
: "\${BASE_BRANCH:?BASE_BRANCH is required}"
: "\${OBJECTIVE:?OBJECTIVE is required}"
: "\${GITHUB_RUN_ID:?GITHUB_RUN_ID is required}"

if [[ ! "$TARGET_REPO" =~ ^Aloews/[A-Za-z0-9._-]+$ ]]; then
  echo "TARGET_REPO must be an owned Aloews repository" >&2
  exit 2
fi
for value in "$TARGET_BRANCH" "$BASE_BRANCH"; do
  if [[ ! "$value" =~ ^[A-Za-z0-9._/-]+$ ]]; then
    echo "Invalid branch name" >&2
    exit 2
  fi
done

API="https://backboard.railway.com/graphql/v2"
REPORT_DIR="\${REPORT_DIR:-artifacts/product-factory}"
mkdir -p "$REPORT_DIR"

gql() {
  local payload="$1"
  curl --silent --show-error --fail-with-body \\
    --request POST "$API" \\
    --header "Authorization: Bearer $RAILWAY_API_TOKEN" \\
    --header "Content-Type: application/json" \\
    --data "$payload"
}

dispatch_and_wait() {
  local role="$1"
  local model="$2"
  local effort="$3"
  local prompt="$4"
  local task_key="\${GITHUB_RUN_ID}-\${role}"
  local conversation_key="product-factory:\${TARGET_REPO}:\${role}"
  local agent_name="pf-\${role}"
  local mutation='mutation($input: CloudAgentTaskDispatchInput!) { cloudAgentTaskDispatch(input: $input) { taskId cloudAgentId sessionId status } }'
  local input response task_id cloud_agent_id session_id
  input=$(jq -n \\
    --arg project "$RAILWAY_PROJECT_ID" \\
    --arg environment "$RAILWAY_ENVIRONMENT_ID" \\
    --arg prompt "$prompt" \\
    --arg model "$model" \\
    --arg effort "$effort" \\
    --arg repo "$TARGET_REPO" \\
    --arg branch "$TARGET_BRANCH" \\
    --arg conversation "$conversation_key" \\
    --arg agent "$agent_name" \\
    --arg key "$task_key" \\
    --arg external "product-factory:$TARGET_REPO:$GITHUB_RUN_ID:$role" \\
    '{
      projectId:$project,
      environmentId:$environment,
      prompt:$prompt,
      model:$model,
      reasoningEffort:$effort,
      source:{repo:$repo,branch:$branch},
      conversationKey:$conversation,
      agentName:$agent,
      allowCreate:true,
      interactive:false,
      idempotencyKey:$key,
      externalRef:$external,
      metadata:{repository:$repo,branch:$branch,role:$agent}
    }')
  response=$(gql "$(jq -n --arg query "$mutation" --argjson input "$input" '{query:$query,variables:{input:$input}}')")
  if jq -e '.errors and (.errors|length>0)' >/dev/null <<<"$response"; then
    jq -c '.errors' <<<"$response" >&2
    return 1
  fi
  task_id=$(jq -er '.data.cloudAgentTaskDispatch.taskId' <<<"$response")
  cloud_agent_id=$(jq -er '.data.cloudAgentTaskDispatch.cloudAgentId' <<<"$response")
  session_id=$(jq -er '.data.cloudAgentTaskDispatch.sessionId' <<<"$response")
  echo "Dispatched role=$role task=$task_id model=$model" >&2

  local query='query($cloudAgentId:String!,$sessionId:String!,$taskId:String){cloudAgentTask(cloudAgentId:$cloudAgentId,sessionId:$sessionId,taskId:$taskId){taskId status text error model reasoningEffort completedAt}}'
  local vars poll task status text_value error_value
  vars=$(jq -n --arg agent "$cloud_agent_id" --arg session "$session_id" --arg task "$task_id" '{cloudAgentId:$agent,sessionId:$session,taskId:$task}')
  for attempt in $(seq 1 540); do
    poll=$(gql "$(jq -n --arg query "$query" --argjson variables "$vars" '{query:$query,variables:$variables}')")
    if jq -e '.errors and (.errors|length>0)' >/dev/null <<<"$poll"; then
      jq -c '.errors' <<<"$poll" >&2
      return 1
    fi
    task=$(jq -c '.data.cloudAgentTask' <<<"$poll")
    status=$(jq -r '.status // "UNKNOWN" | ascii_upcase' <<<"$task")
    case "$status" in
      COMPLETED)
        text_value=$(jq -r '.text // ""' <<<"$task")
        printf '%s\n' "$text_value" > "$REPORT_DIR/$role.md"
        printf '%s\n' "$task" > "$REPORT_DIR/$role.json"
        LAST_RESULT="$text_value"
        echo "Completed role=$role task=$task_id" >&2
        return 0
        ;;
      FAILED|CANCELLED)
        error_value=$(jq -r '.error // "Agent task failed without an error message"' <<<"$task")
        echo "Agent role=$role failed: $error_value" >&2
        printf '%s\n' "$task" > "$REPORT_DIR/$role.json"
        return 1
        ;;
      AWAITING_INPUT)
        echo "Agent role=$role requested input; interactive runs are disabled" >&2
        printf '%s\n' "$task" > "$REPORT_DIR/$role.json"
        return 1
        ;;
      PENDING|RUNNING) ;;
      *)
        echo "Unexpected Railway task status: $status" >&2
        return 1
        ;;
    esac
    sleep 10
  done
  echo "Timed out waiting for role=$role task=$task_id" >&2
  return 1
}

repo_context="Repository: $TARGET_REPO
Working branch: $TARGET_BRANCH
Base branch: $BASE_BRANCH
Objective: $OBJECTIVE

Always fetch the latest remote state before acting. Read AGENTS.md, the project map, open PRs, issues, CI and deployment configuration. Do not invent successful tests, data, integrations or production state. Keep all implementation changes on the named working branch. Never merge or deploy production."

dispatch_and_wait "architect" "claude-fable-5-1" "max" "$repo_context

ROLE: Chief architect. Inspect only; do not edit files, create commits or open a PR. Break the objective into a dependency-aware plan with explicit acceptance criteria, file scope, risks, data/security constraints and validation commands. Identify what is already implemented and avoid duplicating it. Return a concise evidence-backed plan."
ARCHITECT_RESULT="$LAST_RESULT"

dispatch_and_wait "architecture-review" "claude-opus-5-5" "high" "$repo_context

ROLE: Independent architecture reviewer. Read the repository and the proposed plan below. Do not edit files. Challenge assumptions, identify at least three concrete failure modes, review data boundaries and propose a safer/simpler design. Cite exact files or official sources. If the plan is sound, state why and list residual risks.

PROPOSED PLAN:
$ARCHITECT_RESULT"
ARCH_REVIEW="$LAST_RESULT"

dispatch_and_wait "security-review" "claude-sonnet-5-5" "high" "$repo_context

ROLE: Independent security/privacy reviewer. Do not edit files. Inspect the proposed work and plan below. Check authentication, authorization, tenant isolation, PII, secrets, supply-chain, SSRF, abuse, data retention and any legal/regulatory boundaries. Return blocking findings, severity, evidence and required tests.

PROPOSED PLAN:
$ARCHITECT_RESULT"
SECURITY_REVIEW="$LAST_RESULT"

dispatch_and_wait "research" "claude-haiku-5-5" "medium" "$repo_context

ROLE: Research analyst. Do not edit files. Verify material vendor/API/framework/security claims against official primary documentation. Return source URLs, access dates, exact version/endpoint facts, confidence and unknowns. Do not treat a configured credential as proof of a live connection."
RESEARCH_RESULT="$LAST_RESULT"

dispatch_and_wait "implementation" "claude-fable-5-1" "max" "$repo_context

ROLE: Lead implementation engineer. Reconcile the plan and all independent reviews below. Implement the objective in small, testable increments. Only you may edit files in this run. Add regression tests, run relevant checks, inspect the final diff and update the project map. Push the working branch and create/update a PR against the base branch. Do not merge or deploy. If a required credential or external decision is missing, continue independent work and record the precise blocker.

PLAN:
$ARCHITECT_RESULT

ARCHITECTURE REVIEW:
$ARCH_REVIEW

SECURITY REVIEW:
$SECURITY_REVIEW

RESEARCH:
$RESEARCH_RESULT

Closing report must state exact branch, commit SHA, PR URL, commands run and their raw outcomes, checks not run, known blockers and remaining work."
IMPLEMENTATION_RESULT="$LAST_RESULT"

for cycle in 1 2; do
  dispatch_and_wait "code-review-$cycle" "claude-opus-5-5" "high" "$repo_context

ROLE: Independent code reviewer. Do not edit files. Fetch the latest remote branch and inspect its exact HEAD commit and full diff against $BASE_BRANCH. Review correctness, architecture, data integrity, concurrency, failure paths, tests and regressions. Return findings only, each with severity, file/line, evidence and required fix. If no blocking findings, state that explicitly and list residual risks."
  CODE_REVIEW="$LAST_RESULT"

  dispatch_and_wait "qa-$cycle" "claude-sonnet-5-5" "high" "$repo_context

ROLE: Independent QA/release engineer. Do not edit files. Inspect the exact latest commit on $TARGET_BRANCH. Run the repository's full documented test, lint, typecheck, security and build commands that are safe in this environment. Inspect CI runs attached to that exact SHA. Do not count missing, stale or unrelated checks as passes. Return PASS/FAIL per gate, evidence, logs and regression gaps."
  QA_RESULT="$LAST_RESULT"

  if grep -Eiq '(^|[^A-Z])(BLOCKER|CRITICAL|HIGH|FAIL|FAILED)([^A-Z]|$)' <<<"$CODE_REVIEW"$'\n'"$QA_RESULT"; then
    if [[ "$cycle" -eq 2 ]]; then
      echo "Two review/QA cycles exhausted; leaving PR open and reporting blockers." >&2
      break
    fi
    dispatch_and_wait "repair" "claude-fable-5-1" "max" "$repo_context

ROLE: Lead repair engineer. Resolve every blocking finding below, add regression tests, run all affected checks, inspect the exact final diff, push the same working branch and update the PR. Do not merge or deploy. If a finding is incorrect, prove that with repository/test evidence and explain why; do not ignore it.

INDEPENDENT CODE REVIEW:
$CODE_REVIEW

QA REPORT:
$QA_RESULT

Previous implementation report:
$IMPLEMENTATION_RESULT"
    IMPLEMENTATION_RESULT="$LAST_RESULT"
  else
    echo "Independent code review and QA report contain no blocking findings." >&2
    break
  fi
done

printf '%s\n' "Product Factory run $GITHUB_RUN_ID" > "$REPORT_DIR/summary.md"
printf '%s\n' "Repository: $TARGET_REPO" >> "$REPORT_DIR/summary.md"
printf '%s\n' "Branch: $TARGET_BRANCH" >> "$REPORT_DIR/summary.md"
printf '%s\n' "Objective: $OBJECTIVE" >> "$REPORT_DIR/summary.md"
printf '%s\n' "Implementation result: $IMPLEMENTATION_RESULT" >> "$REPORT_DIR/summary.md"
printf '%s\n' "Final review: $CODE_REVIEW" >> "$REPORT_DIR/summary.md"
printf '%s\n' "Final QA: $QA_RESULT" >> "$REPORT_DIR/summary.md"
