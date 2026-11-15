# Universal Service Hub — Security-First Development Roadmap

**Product rule:** protect the user's identity, data, money and digital accounts before maximizing service coverage. Security is a product capability and a release gate, not a post-launch hardening task.

**Status vocabulary:** NOT STARTED / IN PROGRESS / BLOCKED / VERIFIED. A milestone can be VERIFIED only with an exact commit SHA, CI run, reproducible test commands/results and reviewer evidence. Documentation alone does not verify implementation.

## 1. Product vision and security promise

Universal Service Hub is a single, secure workspace across digital life: email, social profiles and LinkedIn, banking, crypto, games, cinema/TV, mobile connectivity and broadband, operating systems and devices, networks and servers, AI generation, health, style, sports, commerce, work and other useful services.

The Hub should federate access where possible instead of copying all provider data into a central database. Users control what is connected, what data is read, what actions are permitted, how long data is retained and when access is revoked.

### Security-first design principles

1. Zero trust and default deny.
2. Least privilege and short-lived credentials.
3. Data minimization and privacy by default.
4. Strong tenant isolation at API, database, job and object-storage layers.
5. Encryption in transit and at rest, with managed key lifecycle.
6. User consent and explicit confirmation for high-impact actions.
7. Auditable operations without collecting sensitive payloads.
8. Secure-by-default agent/tool execution and outbound network policy.
9. User export, disconnect, revoke and deletion controls.
10. Verifiable release evidence and honest capability labels.

## 2. Data classification and trust boundaries

| Classification | Examples | Handling |
|---|---|---|
| Public | Public catalog metadata | Cache with source/freshness |
| Internal | Project/task metadata | Tenant-scoped and encrypted |
| Confidential | Private files, email metadata, drafts, social IDs | Explicit opt-in, narrow scopes, short retention |
| Restricted financial | Bank balances/transactions, tax/payment data | Licensed aggregator, read-only default, separate consent |
| Restricted health | Health records, appointments, wearable data | Isolated data domain, purpose-specific consent |
| Critical secrets | OAuth refresh tokens, API keys | Envelope encryption with KMS; never in logs/agent context |
| Prohibited collection | Bank passwords, seed phrases/private keys | Reject; never request or store |

Trust boundaries: browser/device; identity provider; Hub API; tenant database; secret/KMS service; agent execution sandbox; provider adapters/egress gateway; external providers; object storage; observability and CI/CD. Each boundary requires authentication, authorization, data minimization and failure behavior.

## 3. Threat model

Track and test at least these threats:
- Account takeover, session fixation/theft, weak recovery and MFA bypass.
- OAuth phishing, malicious consent grants, token leakage and redirect manipulation.
- Broken object-level authorization, IDOR, cross-tenant SQL/object/job access.
- SSRF, DNS rebinding, IPv6 bypass, redirects and cloud metadata theft.
- XSS, CSRF, clickjacking, unsafe CORS and insecure browser storage.
- Prompt injection in email, documents, websites, media metadata and provider responses.
- Agent/tool escalation, unauthorized publishing/sending/transfers, budget abuse and duplicate billable actions.
- Insider misuse, audit tampering, sensitive telemetry leakage and over-retention.
- Compromised dependency, build runner, container, artifact or provider.
- Ransomware, data exfiltration, provider outage, key-service outage and destructive deletion.

For every threat record: assets, attacker capability, entry point, trust boundary, impact, likelihood, mitigation, residual risk, owner and regression test.

## 4. Milestone map

### S0 — Security foundation and threat model
**Goal:** establish what is protected and how security is proven.

- Inventory routes, data stores, credentials, integrations, jobs, logs, users and trust boundaries.
- Classify every data field and define purpose, retention, residency and deletion behavior.
- Maintain threat model, data-flow diagrams and abuse-case catalogue.
- Define security invariants, risk register and test matrix.
- Add security status dashboard and exact-evidence format to project map.

**Acceptance:** all existing routes and persisted entities are inventoried; each sensitive data flow has an owner, classification and mitigation; threat model reviewed.

### S1 — Identity, sessions and authorization
**Goal:** remove shared-token dependence for human customers.

- Adopt maintained OIDC identity with passkeys/WebAuthn and MFA.
- Implement account recovery, email verification, session/device listing, rotation and revoke.
- Use secure HttpOnly SameSite cookies, CSRF defenses and restrictive origin policy.
- Implement RBAC plus resource/action scopes and service-to-service identities.
- Keep root break-glass admin outside browser sessions; MFA, short-lived access and immutable audit.
- Add rate limits and anomaly controls to sign-in, recovery, token issue and sensitive endpoints.

**Acceptance:** no browser uses root token; missing/expired/revoked/wrong-audience sessions fail; privilege-escalation and session-revocation tests pass.

### S2 — Tenant isolation and data lifecycle
**Goal:** prevent one user, project or agent from accessing another's data.

- Derive tenant identity only from authenticated principal.
- Add tenant ownership to all customer tables and composite foreign keys.
- Enable and FORCE PostgreSQL RLS for tenant-owned rows.
- Enforce isolation in repository queries, background jobs, caches, search indexes, exports and audit views.
- Separate restricted finance, health, identity and secret domains.
- Isolate object-storage keys by tenant/project; short-lived signed URLs; scan/quarantine uploads.
- Implement export, retention, deletion and deletion verification.

**Acceptance:** automated cross-tenant negative tests cover every list/read/write/delete/export/download/job path; RLS tests run using non-owner application role.

### S3 — Cryptography and credential vault
**Goal:** protect provider access even if the application database is exposed.

- Implement OAuth connection vault with state, nonce, PKCE, exact redirect allowlist and minimal scopes.
- Encrypt refresh/access tokens with envelope encryption backed by managed KMS.
- Separate keys by environment and tenant; rotation, revocation, key versioning and audited unwrap.
- Store only secret references and encrypted payloads; no plaintext secrets in agent or observability context.
- TLS enforcement, HSTS, CSP, X-Content-Type-Options, Referrer-Policy and restrictive Permissions-Policy.
- Add canary secret tests for source, build artifacts, API responses, logs and traces.

**Acceptance:** database-only dump does not expose usable credentials; rotation/revoke works; OAuth callback and token lifecycle tests pass.

### S4 — Secure integrations, network and agents
**Goal:** safely operate across external services and untrusted content.

- Official APIs and documented SDKs only; capability/action registry and contract tests.
- Egress gateway blocks private, loopback, link-local, reserved, multicast and metadata IPs; DNS pinning/revalidation; redirects disabled or revalidated.
- Per-provider host allowlists and bounded timeouts/body sizes.
- Agent sandbox with tool allowlists, typed schemas, per-action authorization, untrusted-input boundaries and no ambient credentials.
- Explicit preview/confirmation for sending email, publishing posts, payments/transfers, crypto signing and destructive actions.
- Idempotency, durable checkpoints, budgets, quotas and kill switches.
- Fail closed on policy/key/audit failures; no silent provider fallback for sensitive actions.

**Acceptance:** SSRF/DNS-rebinding/IPv6/redirect tests pass; prompt injection cannot grant tools; duplicate and ambiguous submissions cannot trigger a second financial/billable action.

### S5 — Security operations and resilience
**Goal:** detect, contain and recover from incidents.

- Redacted structured logs, tamper-evident audit, alerts and incident response runbooks.
- Secret scanning, SAST, dependency audit, container scanning, SBOM, provenance and protected CI.
- Encrypted backups, tested restore, disaster recovery objectives and deletion verification.
- Rate limits, quotas, abuse detection, anomaly monitoring and provider circuit breakers.
- Key compromise, account takeover, provider breach and data exposure response exercises.

**Acceptance:** security CI gates run on exact SHA; restore and incident exercises have recorded outcomes; logs contain no canary secrets or sensitive payloads.

### S6 — Universal service catalog and personal dashboard
**Goal:** expand coverage without weakening the security model.

Domains include:
- Email, calendar, contacts and communication.
- Social profiles, creator accounts and LinkedIn.
- Banking, Open Banking, payments, finance and insurance.
- Crypto portfolio and wallet connections (never custody seed phrases/private keys).
- Games, gaming communities and cloud gaming.
- Cinema, TV, music and licensed streaming discovery.
- Mobile operators, SIM/eSIM, broadband and connectivity.
- Operating systems, devices, applications and backups.
- Domains, networks, VPN, cloud, VPS, servers, databases and storage.
- AI assistants and image/video/music/voice generation.
- Health, wellness, fitness and wearables.
- Style, wardrobe, fashion and beauty.
- Sports, training, events and teams.
- Work, learning, commerce, travel, home and local services.
- Legal, government, identity and business administration.
- Casino/betting discovery only where lawful, with age/jurisdiction restrictions and responsible-gambling tools; no deposits, custody, credit, bets or automated wagering by Hub.

Every catalog entry includes data classification, region, scopes, read/write actions, retention, API/embedding mode, last verification, pricing freshness, privacy notes, disconnect/delete path and known limitations. Sensitive connections are opt-in and read-only by default.

**Acceptance:** no fake Connect controls; all connected status backed by a live authorized check; each integration has tested revoke/disconnect; catalog search and benchmark do not count link-only listings as executable coverage.

### S7 — Independent security verification and controlled release
**Goal:** release only the feature set whose risks are tested and accepted.

- Independent threat-model and code review.
- SAST, dependency, container, secret and SBOM scans.
- DAST, API authorization fuzzing, SSRF, OAuth and tenant-isolation tests.
- Staging E2E with synthetic data only.
- Backup restore, rollback, key rotation and incident drill.
- Document residual risks, data processors, regions, retention and support process.
- Production release requires explicit owner authorization and rollback plan.

**Acceptance:** all mandatory gates pass on exact release SHA; independent reviewer signs off; no critical/high unresolved security finding; staging user journey, restore and rollback are verified.

## 5. Global security requirements for every feature

- Data classification and purpose documented before implementation.
- Tenant and action authorization tested before UI is exposed.
- No secrets, prompts, private content or restricted payloads in logs, analytics, traces, agent context or support artifacts.
- Consent is granular, revocable and visible; read-only default.
- Retention and deletion are defined before collecting data.
- External services are marked Native / Connected and tested / Embedded / Directory-only truthfully.
- High-impact actions require preview and explicit confirmation.
- Security controls must be usable: explain permissions, show connected sessions, revoke access and export/delete data.
- No claim of secure, private, encrypted, compliant or production-ready without specific evidence.

## 6. Current baseline and known gaps

Verified source baseline before this roadmap:
- Rust Axum/Tokio/SQLx service; /v1/* bearer authentication.
- Shared root admin token plus project-scoped SHA-256-hashed tokens with owner/operator/viewer roles.
- Project capability/provider allowlists.
- Infisical secret references supported; production mode requires Infisical configuration.
- 2 MiB request body limit.
- HTTPS-only provider clients with redirects disabled, bounded timeouts and destination checks.
- AI audit design excludes prompts/responses.
- No verified production deployment.

Known gaps:
- No per-user customer identity, passkeys/MFA or browser session system.
- No full RLS tenant enforcement for all product data.
- No customer OAuth vault with KMS envelope encryption.
- No complete rate limiting, production egress firewall, tamper-evident audit, user data export/deletion lifecycle, full security scan pipeline or restore drill.
- No independent security assessment or certification.

These gaps prevent a claim that the Hub is currently production-ready or security-certified.

## 7. Execution and evidence protocol

For each PR:
1. Link milestone IDs (S0–S7) and update this roadmap and docs/MAP.md.
2. Name exact files, threat cases and acceptance tests.
3. One implementation writer; independent reviewer/QA on immutable PR head.
4. Run format, lint, unit, integration, migration, container, security and relevant E2E checks.
5. Attach exact commit SHA, run URL, commands, test counts, scan reports and unresolved risks.
6. Fix failed checks; no bypass, no false-green and no agent-summary-as-proof.
7. Merge only after required CI and review pass. Production remains separately gated.

## 8. Definition of done

The platform is ready for a production release only when S0–S7 applicable gates are VERIFIED, user identity and tenant isolation are enforced, secrets are protected by managed key lifecycle, all enabled integrations have least-privilege scopes and revoke paths, sensitive actions require consent, security operations and recovery are tested, independent review is recorded, and an owner-authorized release has a tested rollback.

**Current overall status: NOT STARTED as a complete security programme.** Existing controls are partial foundations, not evidence of completed S0–S7.
