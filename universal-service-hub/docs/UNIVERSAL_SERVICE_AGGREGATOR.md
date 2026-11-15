# Universal Service Aggregator — Product Blueprint

## Product promise

One workspace for the useful things people do on the internet. A person describes an outcome in natural language, discovers the right tool, connects it when supported, and completes the work in one project space. The Hub should feel like a unified operating layer over the internet's best services — not a pile of links and not a collection of shallow clones.

The strategic target is **80% coverage of common high-value digital jobs**. It is not a claim to replicate 80% of websites or to integrate 80% of all vendors.

## Two separate catalogs

Do not mix these concepts:

- **Product catalog**: customer-facing discovery of tools and capabilities by the job the user wants to do.
- **Infrastructure/provider registry**: technical integrations used by the Hub and projects (hosting, Git, databases, queues, storage, AI providers).

The current `/v1/integrations/catalog` is the latter. Its read-only verification status must not be presented as full product integration or provisioning.

## User experience

### Global shell
- Universal command/search bar: "What do you want to get done?"
- Home: pinned tools, recent projects, recent artifacts, notifications and suggested workflows.
- Catalog: browse by outcome/category, compare alternatives and see honest availability.
- My services: connected accounts, scopes, health, last verified time, disconnect/revoke.
- Projects: each project has its own overview, task graph, activity/discussion, agents, connected apps, files/artifacts, approvals and budget.

### In-project workflow
A user describes a goal. The coordinator decomposes it into a dependency graph, chooses native capabilities and connected services, estimates time/cost, and asks for approval only where required. New comments are interpreted asynchronously; status questions do not mutate plans; additions update only affected branches; unrelated work continues.

### Tool experience
A catalog entry can be:
1. **Native** — the action runs inside the Hub and has Hub-owned tests.
2. **Connected** — official API/OAuth connection with implemented and tested operations.
3. **Embedded / federated** — supported provider UI or deep link opens with project context; Hub does not claim to control it.
4. **Directory only** — discovery and outbound link only.

Show the mode, available actions, permissions, price verification date and limitations before a user connects or launches a tool. Never use a "Connect" action for a directory-only entry.

## Initial category map

| Category | Jobs users should be able to complete | Example service families |
|---|---|---|
| AI, search & research | Ask, compare sources, research a topic, summarize web/files, manage knowledge | AI assistants, search engines, research agents, knowledge bases |
| Documents & office | Write, edit, convert, sign and collaborate on documents, sheets and slides | Office suites, PDF editors, e-signatures |
| Communication | Email, calendar, meetings, chat, transcription and follow-ups | Email, calendars, video meetings, team chat |
| Projects & business | Plan work, track tasks, manage customers and support requests | Project trackers, CRM, help desks |
| Design & whiteboards | Create graphics, diagrams, prototypes, maps and collaborative boards | Design suites, diagramming, whiteboards |
| Image & video | Generate, edit, upscale, subtitle and publish visual media | Image/video editors and generators |
| Audio & music | Generate original music, edit audio, speech and podcasts | Audio workstations, TTS/STT, music tools |
| Developer workspace | Build, review, test, host and observe software | IDEs, code hosts, CI/CD, API testing, observability |
| Automation & agents | Connect apps, trigger workflows and delegate repeatable work | iPaaS, workflow engines, agent platforms |
| Cloud & backend | Deploy apps, run compute, manage databases, queues and storage | Hosting, BaaS, SQL, object storage, cache |
| Data & analytics | Query data, build dashboards, forecast and report | BI, notebooks, warehouses, analytics |
| Files & knowledge | Store, sync, search, share, convert and archive files | Cloud drives, DAM, knowledge management |
| Finance & accounting | Budget, invoice, reconcile, track expenses and manage books | Accounting, invoicing, expense tools |
| Commerce & payments | Sell, subscribe, collect payments and manage orders | Storefronts, payment processors, subscriptions |
| Marketing & growth | Publish, email customers, manage campaigns and measure growth | SEO, social scheduling, email marketing, analytics |
| Learning | Learn skills, practice languages and get tutoring | Courses, tutoring, flashcards, language apps |
| Travel & mobility | Plan trips, compare routes, book stays and navigate | Maps, travel search, transport and lodging |
| Local services | Find providers, compare offers and make reservations | Local search, marketplaces, appointment booking |
| Health & wellness | Organize wellness information and access approved services | Fitness, wellness and patient-facing tools; strict privacy boundaries |
| Security & identity | Manage identity, credentials, access and privacy | Password managers, SSO, MFA, secure sharing |
| Legal & administration | Prepare forms, manage contracts and handle company administration | Contract lifecycle, e-signatures, compliance tools |

This is the initial taxonomy, not a claim that every category is already implemented. Add subcategories based on measured user jobs and search demand.

## Catalog data contract

Every item needs stable, machine-readable metadata:

- `id`, `name`, `category_id`, `subcategories`
- `jobs[]`: canonical task IDs the service can support
- `delivery_mode`: `native | connected | embedded | directory`
- `provider_url`, `regions[]`, `languages[]`
- `pricing_model`, `pricing_verified_at` (nullable; unknown is explicit)
- `api_status`, `embedding_status`, `supported_actions[]`
- `auth_method`, `requested_scopes[]`, `data_handling`, `disconnect_supported`
- `alternatives[]`, `tags[]`, `quality_score`, `last_verified_at`
- `evidence[]`: source, check type, result, timestamp and verifier

Do not store secrets in catalog metadata. Provider account credentials belong in the secret vault, scoped to the user/project and revocable.

## Measuring the 80% target

Build a benchmark of at least 200 representative user jobs across the categories above. Each job has a weight derived from frequency, value/time saved, target-region relevance, reliability and privacy requirements.

A job counts as **execution-covered** only when its core outcome can be completed through:
- a native Hub capability with acceptance tests;
- a connected provider operation with a contract test and successful live/sandbox verification; or
- a supported embedded/federated handoff with a verified destination and clear boundary.

A directory-only link counts as **discovery-covered**, never execution-covered.

Publish these separate metrics:
- weighted discovery coverage;
- weighted execution coverage;
- number of native capabilities;
- number of connected providers and tested actions;
- category coverage and regional/language coverage;
- stale/unverified catalog share.

Do not report "80%" until the benchmark, weights and evidence are reproducible.

## Prioritization formula

For each candidate task/service, rank:
- user demand and task frequency;
- time or money saved;
- number of users/categories benefited;
- reliability and API maturity;
- integration cost and ongoing maintenance;
- price/free-tier accessibility;
- privacy, data residency and permission quality;
- vendor lock-in and data portability.

Prefer one reliable workflow that solves an entire user job over ten superficial integrations.

## Delivery plan

### P0 — foundation
- Freeze the canonical task taxonomy and 200-job benchmark.
- Separate product catalog from infrastructure registry.
- Implement the catalog metadata schema, search/filter API and truthful status labels.
- Build the responsive catalog and global command/search UI.
- Add favorites, recent tools and project launch context.

### P1 — useful everyday workflows
- Documents, research, communication, project management, files and data analysis.
- OAuth/connection vault with least privilege, test/refresh, disconnect and audit.
- In-project discussion, task graph, artifact history and async change handling.

### P2 — creative and technical workflows
- Design, image/video/audio, coding, cloud, automation and analytics.
- Multi-provider routing with cost/latency/reliability policy.
- Durable jobs, checkpoints, idempotency, evidence-based verification and budget guardrails.

### P3 — broaden coverage
- Finance, commerce, marketing, learning, travel, local services, health/wellness and legal/admin.
- Expand by benchmark gaps and real user demand, not by provider-count vanity metrics.

## Acceptance tests

1. Catalog schema rejects missing IDs, invalid delivery modes, duplicate IDs and invalid category references.
2. Directory-only entries never advertise connect, API actions or execution coverage.
3. Connected entries expose only actions backed by registered adapters and contract tests.
4. Every pricing/availability assertion has a verification timestamp or is explicitly unknown.
5. Search returns relevant results for natural-language tasks, synonyms and alternative-provider requests.
6. Project launch carries project context without leaking another project's credentials or files.
7. Revoking a connection blocks subsequent provider calls and invalidates cached credentials.
8. Provider outage is visible and does not falsely mark an action complete.
9. Handoff tests verify destination, parameters and return-to-project behavior.
10. Coverage report is computed from benchmark evidence, not hand-authored percentages.

## Current verified boundary

At the time this blueprint was added, the Hub has an infrastructure integration catalog and a Rust control-plane foundation. It is **not yet** a customer-facing universal service marketplace, and the full catalog UI, OAuth vault, cross-service workflows and benchmark are not complete. The current catalog's provider account checks are read-only; they do not provision external projects or deployments.
