# Universal Service Hub development environment

## Purpose

The repository includes a reproducible Linux development container for GitHub Codespaces and VS Code Dev Containers. It provides a persistent editor/terminal session with the Rust toolchain, GitHub CLI, Docker Engine and Docker Compose v2. It is a developer environment, not the production execution sandbox for untrusted customer agents.

## Start with GitHub Codespaces

1. Open the `Aloews/russian-dev-projects` repository on GitHub.
2. Select the branch `feat/ush-codespaces-development-environment` after this change is merged, or the corresponding PR branch while reviewing it.
3. Open **Code → Codespaces → Create codespace on ...**.
4. Wait for the devcontainer post-create step to finish.
5. Open a terminal and run:

```bash
cd universal-service-hub
bash scripts/dev-up.sh
```

GitHub creates and bills the Codespace under the account's available Codespaces quota. Check the current quota and billing settings before increasing machine size or leaving a Codespace running. Stop the Codespace when it is not in use.

## Start with VS Code

1. Install Docker Desktop or another Docker Engine with Compose v2.
2. Install the VS Code Dev Containers extension.
3. Clone the repository and open its root directory.
4. Run **Dev Containers: Reopen in Container**.
5. In the integrated terminal, run the same `scripts/dev-up.sh` command above.

## Included toolchain

- Linux Debian Bookworm development image.
- Rust toolchain, rustfmt and Clippy.
- Git and GitHub CLI.
- Docker Engine with Docker Compose v2 through the Dev Containers Docker-in-Docker feature.
- Rust Analyzer, TOML, Docker, GitHub Actions and Python editor extensions.
- Port forwarding for the local Hub API on port 8000.

The environment runs `cargo fetch` once during creation. It does not automatically compile, test, deploy, or mutate any remote database.

## Local application lifecycle

The existing `scripts/dev-up.sh` command:
- generates random local-only API and PostgreSQL credentials when needed;
- stores them in `.env.local` with restrictive file permissions;
- starts the Hub and PostgreSQL using `compose.yml`;
- waits for readiness;
- verifies that an unauthenticated API request is rejected and an authenticated read request succeeds.

The API is available at `http://localhost:8000`. The root token remains in `universal-service-hub/.env.local`; do not paste it into chat, commit it, or add it to logs.

Use `docker compose --env-file .env.local -f compose.yml logs --no-color hub postgres` from the Hub directory to inspect local logs. Use the repository's documented shutdown command or `docker compose --env-file .env.local -f compose.yml down` to stop the local stack. Do not add `-v` unless you intentionally want to delete the local database volume.

## Git worktrees

Use a separate Git worktree and branch for each independent task. Never allow two agents to write to the same worktree or branch. A worktree is a source-control isolation boundary only; it is not a security sandbox. Untrusted agent execution still requires a separate container/VM, resource limits, restricted credentials and network egress controls.

Example:

```bash
git worktree add ../ush-task-123 -b agent/task-123
```

Remove it only after preserving required commits and artifacts:

```bash
git worktree remove ../ush-task-123
```

## Security and scope limits

- This container is for trusted development by the repository owner and authorized contributors.
- Do not mount production credentials, SSH agent sockets or personal directories into untrusted agent workspaces.
- Do not run untrusted repository hooks or scripts with broad host privileges.
- Docker-in-Docker gives the workspace powerful container-management capability. Do not treat it as hostile-code isolation.
- No customer tenant can access this Codespace. Tenant-aware workspace provisioning, per-tenant VM/container isolation, network policy, ephemeral credentials, quotas, artifact isolation and audit are separate USH product milestones.
- No production deployment, paid compute provisioning, database migration or secret rotation is performed by this configuration.

## Verification

The dedicated GitHub Actions workflow validates the devcontainer JSON, bootstrap shell syntax and Cargo manifest metadata. A successful CI run proves repository configuration validity only. A live Codespace launch must still be checked in GitHub by opening the created environment and running the local startup smoke test.
