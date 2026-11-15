# Security baseline and target controls

## Product security promise

Universal Service Hub is designed to minimize the amount of sensitive data it handles. It is a security-first federation and orchestration layer, not a password vault for arbitrary provider credentials and not a central warehouse for every user's private life.

The current implementation is a development-stage control plane, not a certified or production-ready customer platform. See MAP.md and SECURITY_FIRST_ROADMAP.md for verified state and release gates.

## Security invariants

- **Zero trust:** authenticate each request and authorize each resource/action. Never trust a client-supplied tenant or project identifier.
- **Least privilege:** narrow scopes, short-lived credentials, read-only by default and separate confirmation for sensitive writes.
- **Data minimization:** federate where possible; retain only necessary metadata and user-approved artifacts.
- **Tenant isolation:** enforce ownership in application queries and at PostgreSQL RLS boundaries where applicable. Never rely only on UI filtering.
- **Fail closed:** deny on missing identity, policy, key service, authorization or required audit failure.
- **No secrets in agents:** coding agents, logs, prompts, telemetry and support exports never receive raw production secrets.
- **User control:** connection inspection, permission review, revoke/disconnect, export and deletion must be available and tested.
- **Honest capability labels:** native, connected-and-tested, embedded/federated and directory-only are distinct states.

## Data classification

| Class | Examples | Default handling |
|---|---|---|
| Public | Public catalog metadata, public provider documentation | Public cache permitted with source and freshness |
| Internal | Project plans, non-sensitive task metadata | Tenant-scoped access; encrypted transport and storage |
| Confidential | Email metadata, social account identifiers, private files, generated drafts | Explicit opt-in, minimal retention, encrypted storage, strict scopes |
| Restricted financial | Bank balances/transactions, payment metadata, tax records | Regulated aggregator; read-only by default; separate consent and audit |
| Restricted health | Health records, appointments, wearable/medical data | Separate consent and storage boundary; purpose limitation and deletion |
| Critical credentials | OAuth refresh tokens, API keys, signing credentials | Managed secret/KMS vault; envelope encryption; never log or expose |
| Never collect | Bank passwords, crypto seed phrases/private keys, unrelated provider passwords | Reject at product/API boundary; educate user to revoke if submitted |

## Current implemented controls (not a production certification)

- /v1/* routes require bearer authentication; the root token is configured outside source and compared using constant-time comparison.
- Project runtime tokens are stored as SHA-256 hashes, scoped to one project, role checked, revocable and returned once at issuance.
- Project capability/provider allowlists are enforced on current execution routes.
- Request body limit is 2 MiB.
- Provider credentials can resolve through Infisical references; production mode requires the Infisical backend.
- External provider requests use HTTPS, bounded timeouts and disabled redirects; private/local/link-local destinations are checked before outbound connections.
- AI audit records are designed to exclude prompts, responses and credentials.
- These controls do not yet provide per-user identity, MFA/passkeys, browser sessions, full RLS tenant isolation, managed KMS envelope encryption, rate limiting, production egress firewall, tamper-evident audit, complete security scanning or restore drills.

## Required controls by domain

### Identity and sessions
- OIDC/passkey-capable identity provider, MFA/passkeys, verified account recovery and device/session management.
- Secure, HttpOnly, SameSite cookies; session rotation, revocation and expiry; CSRF protection for cookie-authenticated mutations.
- OAuth authorization code + PKCE, state/nonce validation, exact redirect URI allowlists and token audience/issuer validation.
- Browser users never receive the root administrator token. Break-glass admin is separate, MFA-protected, tightly scoped, time-limited and audited.

### Authorization and tenant isolation
- RBAC plus resource/action scopes; default deny.
- Server derives tenant identity from the authenticated principal, not request body/path alone.
- PostgreSQL RLS with FORCE ROW LEVEL SECURITY for customer-owned tables, including background jobs and audit views where appropriate.
- Composite tenant foreign keys and object-storage namespaces; negative cross-tenant tests for list/read/write/delete/export/download/background processing.
- Separate health, finance, identity and credential data domains.

### Secrets and cryptography
- OAuth/API credentials encrypted with envelope encryption backed by managed KMS; key hierarchy separated by environment and tenant.
- Key rotation, revocation, access audit and tested recovery.
- TLS for every external hop; secure cookies; HSTS, CSP, X-Content-Type-Options, Referrer-Policy and restrictive Permissions-Policy.
- Restrictive CORS allowlist; never wildcard credentialed origins.
- No plaintext secrets in source, database, logs, test fixtures, agent context or generated reports.

### Data lifecycle and privacy
- Explicit purpose and consent per data category; data minimization and retention schedules.
- Export, deletion, disconnect and consent withdrawal; deletion propagation to derived indexes and object storage.
- No prompt/body/transaction/health payload logging. Telemetry uses redacted identifiers and bounded metadata.
- Region-aware processing and vendor subprocessors disclosed before connection.
- Sensitive integrations are disabled by default and read-only unless a separate action is authorized.

### Integrations, SSRF and agent safety
- Official provider APIs only; narrow scopes and explicit provider capability contracts.
- Egress gateway blocks loopback, private, link-local, multicast, reserved and cloud metadata ranges; resolve/pin DNS safely and revalidate every redirect (prefer no redirects).
- Tool allowlists, typed schemas, bounded inputs/outputs, per-tool authorization and sandboxing.
- Treat provider content, emails, documents, web pages and media metadata as untrusted input; prompt injection must not grant tools or privileges.
- Explicit confirmation and transaction preview for bank transfers, publishing, sending messages, crypto signing or other high-impact writes.
- Idempotency and durable checkpoints; never blindly retry a billable, financial or irreversible action.

### Operations and supply chain
- Structured redacted logs, tamper-evident append-only security audit, alerting and incident response.
- Secret scanning, SAST, dependency audit, container scanning, SBOM, provenance and protected CI environments.
- Encrypted backups, restore drills, disaster recovery objectives, deletion verification and incident/breach playbooks.
- Rate limits, quotas, anomaly detection, abuse prevention, kill switches and circuit breakers.

## Sensitive category policies

- **Banking:** licensed Open Banking/PSD2 provider; never collect bank login credentials. Read-only by default. Transfers/payments require explicit per-action consent, preview, idempotency and audit.
- **Crypto:** never collect or store seed phrases/private keys. Clearly disclose custodial status. Signing/transfer requires a user-controlled wallet confirmation and chain/network verification.
- **Health:** separate restricted-data boundary, explicit purpose-specific consent, minimum retention and no unsupported diagnostic/treatment claims.
- **Social/email:** official OAuth only; separate read, publish and send scopes; clear review before sending or publishing.
- **Games/media:** do not bypass DRM, licensing, platform security or anti-cheat protections.
- **Casino/betting:** legal operator discovery only, jurisdiction/age checks and responsible-gambling support. Hub does not hold funds, accept deposits, place bets, offer credit or automate wagering.

## Mandatory security test matrix

- Authentication: missing, malformed, expired, revoked and wrong-audience credentials are rejected.
- Authorization: role and action matrix, privilege escalation, IDOR and tenant spoofing.
- Tenant isolation: cross-tenant attempts across APIs, SQL/RLS, files, exports, background jobs and audit.
- OAuth: state/nonce/PKCE, redirect allowlist, scope minimization, token encryption, refresh rotation and revoke.
- Network: SSRF, DNS rebinding, IPv4/IPv6 special ranges, redirects, metadata endpoints and proxy bypass.
- Web: CSRF, XSS, CSP, CORS, cookie flags, clickjacking and request-size limits.
- Agent/tool: prompt injection, untrusted document instructions, tool escalation, schema abuse, data exfiltration and budget bypass.
- Secrets/privacy: canary secret scans, log/trace/audit redaction, retention and deletion.
- Resilience: provider outage, key-service outage, audit failure, rate-limit abuse, backup restore and key rotation.
- Supply chain: lockfile audit, SAST, container scan, SBOM and artifact provenance.

## Release gate

No production release until S0–S7 in SECURITY_FIRST_ROADMAP.md are complete for the enabled feature set, exact-SHA CI is green, independent security review is recorded, staging E2E uses synthetic data, backup restore and rollback are tested, and an owner-authorized release decision exists. A checklist or scan alone is not a security certification.


## API token rate limiting — implementation in progress

The Rust runtime applies a PostgreSQL-backed fixed-window limit per successfully authenticated bearer token. The default is 120 requests per token per UTC minute; operators may set `USH_RATE_LIMIT_PER_MINUTE` to an integer from 1 to 10000. The database stores only the SHA-256 token fingerprint, window start, request count and update timestamp. Fingerprint rows idle for 24 hours are pruned at startup and hourly. A rejected request returns HTTP 429 with `Retry-After: 60`.

This control limits abuse of valid credentials and is shared across runtime instances through PostgreSQL. It does **not** replace edge/IP-based throttling for unauthenticated traffic, sign-in/recovery abuse controls, or a distributed abuse-detection system. Operational dashboards remain outstanding. Do not describe S1 as complete.