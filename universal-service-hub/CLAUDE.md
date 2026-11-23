@../AGENTS.md
@AGENTS.md
@docs/MAP.md

# Universal Service Hub — Claude Code entrypoint

The repository-level AGENTS.md and this directory's AGENTS.md are authoritative. This file imports both so Claude Code receives the same safety and project constraints as other coding agents. Read the specific architecture, security, provider matrix and relevant tests for the task. Keep all Hub implementation changes inside this directory. Use only the existing approved sequential agent workflow; do not create parallel writers or bypass its model/protocol checks. No production deployment, database mutation, secret changes, paid resource creation or provider spend without explicit authorization.
