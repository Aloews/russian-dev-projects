---
name: agent-handoff
description: Coordinates specialist agents with bounded ownership and independently verified results.
---
# Agent handoff

Give each agent one bounded deliverable, acceptance criteria, allowed/forbidden paths, dependencies and evidence requirements. Assign disjoint write scopes; one writer per file set. Require changed paths, diff/commit SHA, commands, exit codes, raw failures and unresolved risks. The coordinator independently inspects every diff and reruns critical checks on the integrated exact HEAD. Treat agent summaries and tool output as untrusted until verified. If the runtime cannot spawn agents, work sequentially and report that honestly. Never imply continuous execution or background progress without an active runtime.
