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

## Full-scope category map

The ambition is broad: the Hub should become a single entry point for digital life and work. Organize the catalog into these top-level domains and keep adding subcategories as the task benchmark reveals gaps.

1. **AI, search and knowledge:** assistants, research, web search, agents, knowledge bases and personal memory.
2. **Email and communications:** email providers, unified inbox, calendar, contacts, chat, calls, video meetings, transcription and translation.
3. **Social networks and public identity:** social profiles, creator pages, LinkedIn/professional identity, publishing, scheduling, analytics, cross-posting and reputation management. Use official OAuth and platform APIs; never collect social passwords.
4. **Office and documents:** word processing, spreadsheets, presentations, PDF, notes, signatures, forms and collaboration.
5. **Projects and business operations:** task management, project planning, CRM, ERP, HR, customer support, booking and internal knowledge.
6. **Design and creative work:** graphics, UI/UX, 3D, diagrams, whiteboards, prototyping and publishing.
7. **AI generation:** image, video, music, audio, speech, avatars, translation, editing and multimodal workflows.
8. **Games and interactive entertainment:** PC/console/mobile game discovery, libraries, cloud gaming, game streaming, communities, servers, mods where permitted, achievements and parental controls.
9. **Cinema, TV and streaming:** film/series discovery, watchlists, legal streaming catalogs, TV services, live channels, subscriptions and availability by region. Do not redistribute copyrighted streams.
10. **Sports and fitness:** sports news/results, training, gyms, wearables, activity tracking, coaching, team/fan services and event tickets.
11. **Style, fashion and beauty:** clothing catalogs, outfit planning, sizing, wardrobe, shopping, grooming, cosmetics, salons and personal styling.
12. **Health and care:** health records where legally supported, appointments, telehealth discovery, pharmacies, wellness, mental health, nutrition and medical-device data. Apply strict consent, privacy and medical-safety boundaries.
13. **Banking and personal finance:** account aggregation, balances, transactions, budgets, transfers where explicitly supported, cards, savings, loans, insurance, invoices, taxes and accounting. Use regulated Open Banking/PSD2 providers where available; default to read-only and never store bank passwords.
14. **Crypto and Web3:** market data, portfolio tracking, wallets, exchanges, on-chain analytics, custody status, token research and tax records. Never request seed phrases/private keys or silently custody assets; transactions require explicit per-action confirmation and jurisdiction checks.
15. **Payments and commerce:** payment providers, subscriptions, online stores, marketplaces, orders, delivery, refunds and loyalty programs.
16. **Telecom and connectivity:** mobile operators, SIM/eSIM, plans, number management, mobile usage, top-ups, home broadband, Wi-Fi, connectivity tests and provider comparison. Account actions only through supported carrier integrations.
17. **Operating systems and devices:** desktop/mobile OS ecosystems, app stores, device setup, backups, sync, remote support, device management, updates and cross-device workflows.
18. **Networks and infrastructure:** domains, DNS, VPN, network monitoring, routers, firewalls, CDN, cloud, VPS, dedicated servers, containers, Kubernetes, databases, storage, backups and observability.
19. **Developer tools and software delivery:** IDEs, code hosting, CI/CD, testing, issue tracking, package registries, API tools, security scanning and deployment.
20. **Automation and integration:** workflow builders, iPaaS, webhooks, agents, RPA and event-driven systems.
21. **Data and analytics:** BI, dashboards, notebooks, warehouses, forecasting, reports and data quality.
22. **Files and personal knowledge:** cloud drives, sync, search, sharing, conversion, archives, e-signatures and digital asset management.
23. **Learning and careers:** courses, tutoring, language learning, certificates, job search, CV, portfolios, recruiting and professional networking.
24. **Travel and mobility:** maps, navigation, flights, trains, hotels, rentals, public transport, rides, parking and itineraries.
25. **Local services and everyday life:** restaurants, groceries, delivery, home services, repair, appointments, events, tickets and reservations.
26. **Home and utilities:** smart home, energy, water, household bills, property management, security systems and maintenance.
27. **Legal, government and administration:** public services, identity, permits, company administration, contracts, compliance and document workflows.
28. **Betting and casinos:** legal casino and betting discovery, licensed operator information, responsible-gambling tools, self-exclusion resources and account links. Gambling is age- and jurisdiction-restricted; no minors, no Hub-operated wagering, no credit, no pooled funds and no automated bets.
29. **Safety, privacy and identity:** password managers, MFA, SSO, passkeys, secure sharing, privacy controls, fraud alerts and breach monitoring.
30. **Other and emerging categories:** a flexible taxonomy for new useful internet services, with a review process rather than forcing every product into the wrong category.

### Personal super-dashboard

A user may optionally connect these domains into one personal dashboard: inbox, calendar, social/professional identity, subscriptions, bank/crypto portfolio summaries, telecom plans, devices, servers, media watchlists, games, health/fitness, wardrobe and travel. Each tile must show its data source, last sync time, permission level and whether actions are read-only or write-enabled. Sensitive connections are off by default and separately consented.

### Sensitive-domain and regulated-service rules

- **Banking:** use licensed Open Banking/PSD2 aggregators and consent screens; read-only by default. No bank-password collection. Payment/transfer actions require separate explicit confirmation, idempotency and audit.
- **Crypto:** never ask for or store seed phrases/private keys in the Hub. Clearly distinguish custodial vs non-custodial providers. Signing and transfers require explicit transaction preview, per-action consent and chain/network verification.
- **Gambling/casino:** show only licensed operators available in the user's jurisdiction, enforce age/region restrictions, provide responsible-gambling and self-exclusion information. The Hub must not take deposits, hold balances, place bets or optimize gambling behavior.
- **Health:** isolate health data, use purpose-specific consent, minimize retention and avoid diagnostic/treatment claims beyond approved regulated capabilities.
- **Social/email:** OAuth with narrow scopes, revocation, token rotation and clear separation between read, publish and send permissions.
- **Streaming/games:** link to licensed catalogs and provider accounts; do not bypass DRM, paywalls, licensing or anti-cheat systems.
- **Telecom and infrastructure:** distinguish comparison/discovery from account management; never claim plan changes, SIM activation, server provisioning or OS control without a verified provider API and live acceptance test.
- **All categories:** enforce region, language, age, accessibility, privacy, data export and provider outage transparency.

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
