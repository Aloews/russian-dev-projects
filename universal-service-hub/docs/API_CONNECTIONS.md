# Connecting provider APIs

The Rust service supports authenticated API verification probes. Configure providers with `USH_PROVIDERS_JSON`; set each provider's `api_base_url`, `verify_path`, `verify_method`, optional `verify_body`, and `api_token_env`. The last field is the *name* of an environment variable, not its value. The token is loaded at runtime, never serialized in API responses, and must be injected by the hosting platform's secret store.

Example (illustrative endpoints; validate API permissions and current vendor documentation before enabling):

```json
[
  {
    "id": "cloudflare",
    "name": "Cloudflare",
    "capabilities": ["edge", "object-storage"],
    "priority": 100,
    "enabled": true,
    "api_base_url": "https://api.cloudflare.com/client/v4",
    "verify_path": "/user/tokens/verify",
    "verify_method": "GET",
    "api_token_env": "CLOUDFLARE_API_TOKEN"
  },
  {
    "id": "supabase-management",
    "name": "Supabase Management API",
    "capabilities": ["database-management"],
    "priority": 100,
    "enabled": true,
    "api_base_url": "https://api.supabase.com/v1",
    "verify_path": "/projects",
    "verify_method": "GET",
    "api_token_env": "SUPABASE_ACCESS_TOKEN"
  },
  {
    "id": "railway",
    "name": "Railway GraphQL API",
    "capabilities": ["app-hosting", "backend"],
    "priority": 100,
    "enabled": true,
    "api_base_url": "https://backboard.railway.com",
    "verify_path": "/graphql/v2",
    "verify_method": "POST",
    "verify_body": {"query": "{ me { id name } }"},
    "api_token_env": "RAILWAY_API_TOKEN"
  }
]
```

Then call `POST /v1/providers/health-check` with `{}` to probe all configured entries. A provider with a missing token or failed verification is marked unhealthy. Use the per-provider form `{"provider_id":"cloudflare"}` to check one provider.

## Credential names to provision

Create these secret variables in the Hub runtime (never commit their values):

- `NORTHFLANK_API_TOKEN`
- `CLOUDFLARE_API_TOKEN`
- `NETLIFY_AUTH_TOKEN`
- `RENDER_API_KEY`
- `RAILWAY_API_TOKEN`
- `FLY_API_TOKEN`
- `DIGITALOCEAN_ACCESS_TOKEN`
- `SUPABASE_ACCESS_TOKEN`
- `NEON_API_KEY`
- `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` (S3-compatible)
- `B2_APPLICATION_KEY_ID` and `B2_APPLICATION_KEY` (Backblaze B2)

The current probe implementation supports bearer-token HTTP verification. S3-compatible signing (AWS SigV4), Backblaze's native authorize flow, vendor-specific resource operations and write/deploy actions require dedicated adapters and are not claimed as connected by this generic probe.

## Safety

Use least-privilege tokens. Start with read-only account/project verification. Do not grant deploy, delete, billing, DNS mutation, database write, or production migration scopes until a separate action explicitly requires them. Do not paste credentials into chat or GitHub issues. OAuth connection to GitHub does not grant access to any of these provider accounts.
