# Rust provider integration status

The Rust runtime exposes a read-only connection verification API. It never provisions, mutates, or deletes provider resources.

| Provider | API | Verification operation | Status in this increment |
|---|---|---|---|
| Northflank | REST | List projects | Implemented; live credentials required |
| Railway | GraphQL | Read authenticated account | Implemented; live credentials required |
| Fly.io | REST | List Machines apps | Implemented; live credentials required |
| Render | REST | List services | Implemented; live credentials required |
| Netlify | REST | List sites | Implemented; live credentials required |
| DigitalOcean | REST | Read account | Implemented; live credentials required |
| Cloudflare R2 | REST | List account buckets | Implemented; account ID and token required |
| Supabase | Management REST | List projects | Implemented; PAT required |
| Neon | REST | List projects | Implemented; API key required |
| Backblaze B2 | REST | Authorize account | Implemented; key ID and application key required |
| PostgreSQL | Wire protocol | SELECT 1 | Catalogued; connector pending |
| Redis | RESP | PING | Catalogued; connector pending |
| S3-compatible storage | S3 API | Signed bucket operation | Catalogued; SigV4 connector pending |

## API contract

- `GET /healthz`
- `GET /v1/providers`
- `POST /v1/connections/{provider_id}/test`

All routes except `/healthz` require `Authorization: Bearer $USH_API_TOKEN`. Provider keys are read from runtime environment variables. The API never accepts provider secrets in JSON bodies and never returns upstream response bodies.

## Acceptance gates

A successful HTTP response proves only that the specific account-level check succeeded. It does not prove deploy permissions, database access, storage read/write, queue semantics, billing headroom, or production failover. Each operation must receive its own least-privilege integration test before being eligible for automatic workload routing.

Official API references:
- https://northflank.com/docs/v1/api/introduction
- https://docs.railway.com/integrations/api
- https://fly.io/docs/machines/api/
- https://render.com/docs/api
- https://docs.netlify.com/api-and-cli-guides/api-guides/
- https://docs.digitalocean.com/reference/api/
- https://developers.cloudflare.com/r2/api/
- https://supabase.com/docs/reference/api/introduction
- https://neon.tech/docs/reference/api-reference
- https://www.backblaze.com/apidocs/b2-authorize-account
