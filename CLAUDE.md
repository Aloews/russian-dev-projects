@AGENTS.md

# Claude Code entrypoint

The repository-wide agent contract above is authoritative. For work inside `universal-service-hub/`, also read `universal-service-hub/AGENTS.md`, `universal-service-hub/docs/MAP.md` and the relevant security/architecture standards before editing. The Hub is a Rust production container with a legacy FastAPI reference implementation; inspect the actual target before assuming a stack or entrypoint.

Use the specialist agents and skills under `.claude/` only as additive role guidance. They never override repository instructions, user permissions, provider protocol constraints, security controls or budget limits. Do not run billable provider calls or mutate production as part of routine verification.
