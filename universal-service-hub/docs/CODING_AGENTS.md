# Coding agents for Universal Service Hub

## Roles

The Hub's own engineering workflow uses two coding CLIs through the existing Starimg account:

1. Codex CLI through Starimg's OpenAI Responses API is the primary implementation writer.
2. Claude Code CLI through Starimg's Anthropic Messages API is the sequential independent reviewer and repair writer. It inspects the actual Codex diff, fixes blocking defects and reruns relevant checks.
3. GitHub Actions verifies the exact PR head. Agents do not receive the GitHub write token; only the final publication step can push a candidate branch and open a PR.

This workflow is specific to developing Universal Service Hub itself. The existing Product Factory workflow for target products remains separate until its own migration is reviewed and tested.

## Provider configuration

The workflow reads this repository Actions configuration:

- Secret: STARIMG_API_KEY.
- Variable: STARIMG_BASE_URL; defaults to https://ai.starimg.ru.
- Optional variable: STARIMG_CODEX_MODEL; exact model ID from the live model catalogue.
- Optional variable: STARIMG_CLAUDE_MODEL; exact model ID from the live model catalogue.

Never put the API key in a repository file, workflow input, issue, PR, artifact or log. The workflow never prints it.

Model IDs are discovered from the live GET /v1/models response. If an explicit model variable is configured, it must exactly match the catalogue. Otherwise, the workflow prefers a model ID containing codex (or a listed gpt- model) for Codex and a listed Claude Sonnet model for Claude Code, falling back to a listed Claude Opus or other Claude model. It does not invent model IDs.

## Protocol separation

The same Starimg key is used with two different API protocols:

| Client | Protocol | Base URL | Required endpoint |
|---|---|---|---|
| Codex CLI | OpenAI Responses | https://ai.starimg.ru/v1 | POST /v1/responses |
| Claude Code CLI | Anthropic Messages | https://ai.starimg.ru | POST /v1/messages |

Before either CLI is started, the workflow sends a minimal forced tool-call request through each protocol. A models-list response alone is not considered proof of coding-agent compatibility. If either tool-call probe fails, the run stops before editing code and retains the provider error category without exposing the key.

Codex uses a named Starimg provider, wire_api = responses, and the STARIMG_API_KEY environment key. Claude Code receives ANTHROPIC_BASE_URL, ANTHROPIC_API_KEY and the discovered model ID. The base URLs deliberately differ: Claude Code appends its Messages path; Codex uses the OpenAI Responses path.

## Execution and safety

- Start the workflow manually from GitHub Actions: Universal Hub — Codex + Claude Code. After the workflow is merged to the default branch, the repository owner can also comment exactly /hub run on a PR to start the default objective.
- Choose a current base branch and a bounded engineering objective.
- Codex writes first. Claude Code then inspects the real diff and can repair it.
- The two agents run sequentially on one ephemeral GitHub-hosted runner to avoid concurrent edits.
- Both agents may modify only the universal-service-hub/ subtree. They cannot publish or deploy.
- The workflow rejects unexpected commits, out-of-scope file changes, secret-like additions and whitespace errors.
- A separate publication step creates an isolated branch and PR. GitHub CI must validate the exact PR head before merge.
- No automatic merge, production deployment, paid infrastructure provisioning, production data mutation, DNS change or credential rotation is permitted.

The Starimg key is scoped to the model-probe and CLI steps. Codex is configured to exclude STARIMG_API_KEY from shell-command environments; Claude Code is restricted to Read, Write, Edit, Glob and Grep, with no Bash or network tools. The GitHub write token is not exposed to either coding CLI and is introduced only in the publication step. GitHub-hosted runners are ephemeral; do not place production credentials in the runner environment.

## Current limitations

- The workflow has not yet been executed against the live Starimg account. The first run must prove both Responses function calling and Anthropic Messages tool use with the actual key and selected models.
- Codex and Claude Code model IDs, quotas and billing multipliers are account-dependent. The workflow discovers IDs but does not assume the account's remaining balance or spending limit.
- A green provider probe proves only the minimal protocol/tool-call contract. It does not prove long-context reliability, full coding quality, provider uptime or successful completion of a product task.
- GitHub Actions CI and independent review remain mandatory. No workflow run or generated patch is considered a finished Hub release by itself.
