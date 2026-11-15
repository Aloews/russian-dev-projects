# Service integration catalog

The Rust control plane exposes `GET /v1/integrations/catalog`. It returns the service choices available to a project-creation UI, their integration role, required credential style, and whether the Hub currently has a read-only account verifier.

The catalog intentionally separates three different capabilities:

- **Catalog entry** — the provider can be selected as a target in project planning.
- **Account verification** — the Hub can validate the supplied credential against the provider's account API without creating or changing resources.
- **Project provisioning** — the Hub can create repositories, projects, databases, deployments, or other resources in the connected account.

A provider being listed does not imply that provisioning is implemented. At this revision, `project_provisioning` is false for every catalog entry. Do not describe a connected account as a created project or deployed application.

## Providers

### Frontend hosting — Vercel + five alternatives

| Provider | Typical role | Account verification |
|---|---|---|
| Vercel | Frontend and framework deployment | Supported |
| Netlify | Frontend hosting and edge functions | Supported |
| Cloudflare Pages and Workers | Static hosting and edge runtime | Supported through Cloudflare account token |
| Render | Web services and static sites | Supported |
| Fly.io | Regional application runtime | Supported |
| AWS Amplify | Frontend hosting in AWS | Catalog only |

### Application platform — Railway + five alternatives

| Provider | Typical role | Account verification |
|---|---|---|
| Railway | App services, workers and managed dependencies | Supported |
| Render | Web services, workers and managed databases | Supported |
| Fly.io | Machines and regional app runtime | Supported |
| Northflank | Services, jobs and managed environments | Supported |
| DigitalOcean App Platform | Managed application deployment | Supported |
| Coolify | Self-hosted deployment control plane | Catalog only |

### Backend — Supabase + five alternatives

| Provider | Typical role | Account verification |
|---|---|---|
| Supabase | PostgreSQL, Auth, Storage and Realtime | Supported |
| Firebase | Google-managed app backend | Catalog only |
| Appwrite | Self-hostable backend services | Catalog only |
| PocketBase | Lightweight SQLite backend | Catalog only |
| Nhost | PostgreSQL and GraphQL backend | Catalog only |
| Convex | Reactive TypeScript backend | Catalog only |

### Git hosting — GitHub + five alternatives

| Provider | Typical role | Account verification |
|---|---|---|
| GitHub | Git repositories, pull requests and Actions | Supported |
| GitLab | Git repositories and integrated CI/CD | Supported |
| Bitbucket | Git repositories and Pipelines | Supported with OAuth access token |
| Gitea | Self-hosted Git service | Supported |
| Forgejo | Community-governed self-hosted Git service | Supported |
| Codeberg | Hosted Forgejo for open-source projects | Supported |

GitHub has real alternatives. GitLab and Bitbucket are hosted platforms; Gitea and Forgejo are self-hostable; Codeberg is a hosted Forgejo instance rather than a separate Git server implementation.

## Configure a verified account

1. Store the credential in Infisical under `/projects/shared/providers` using a secret name such as `GITHUB_TOKEN`, `VERCEL_TOKEN`, or `GITLAB_TOKEN`.
2. Register the provider with `PUT /v1/providers/{provider_id}`. The `api_key_env` field is a secret reference name only; never send a credential value in the provider JSON.
3. For Gitea or Forgejo, set `base_url` to the HTTPS origin of the self-hosted instance. Do not include a path, query, fragment, or embedded credentials.
4. Call `POST /v1/integrations/{provider_id}/verify`. A successful response means the Hub authenticated and read the account identity; it does not mean it created a repository or deployment.
5. Add the provider ID to the project's `allowed_provider_ids` when creating/updating the project.

Example GitHub provider record:

```json
{
  "name": "GitHub",
  "capabilities": ["git.repository", "git.pull_request", "git.ci"],
  "priority": 100,
  "enabled": true,
  "adapter_type": "github",
  "api_key_env": "GITHUB_TOKEN",
  "timeout_seconds": 15
}
```

Example self-hosted Forgejo provider record:

```json
{
  "name": "Forgejo",
  "capabilities": ["git.repository", "git.pull_request"],
  "priority": 80,
  "enabled": true,
  "adapter_type": "forgejo",
  "base_url": "https://git.example.org",
  "api_key_env": "FORGEJO_TOKEN",
  "timeout_seconds": 15
}
```

The API never returns credentials. Verification is read-only and does not follow redirects. Production credentials must be stored in Infisical; environment-variable fallback is development-only.

## Remaining work before full project creation

The following must be implemented and tested before claiming end-to-end project provisioning:

- OAuth installation/callback flows and encrypted credential lifecycle (the current setup uses pre-provisioned Infisical secret references).
- Per-project integration ownership and credential isolation (current provider credentials resolve from the shared integration path).
- Repository creation/import, branch protection, webhook registration and CI workflow setup.
- Vercel/Railway/Render deployment creation and environment-variable linking.
- Supabase/Firebase/Appwrite/Nhost/Convex project/database provisioning.
- A project-creation web UI with provider selection, connection state, least-privilege scopes, verification results and disconnect/revoke controls.
- Contract tests against provider sandbox accounts, plus idempotency and rollback tests for resource creation.

Until these exist, the feature is an integration catalog plus read-only account verification for the providers marked as supported, not a complete one-click project factory.
