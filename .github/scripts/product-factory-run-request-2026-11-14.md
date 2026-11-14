# Product Factory run request — 2026-11-14

Owner request: deliver AUREL Elite Estate as a working staging MVP, without Fable; use Starimg `claude-opus-5` with local Qwen fallback.

Use the workflow's configured primary/fallback routing and acceptance gates. Target `Aloews/full`, working branch `factory/elite-estate-rc4`, base `foundation/luxury-estate-platform`.

Required user journey: verified email OTP sign-up/sign-in, agency workspace, property submission with source/evidence, independent review, verified-only public search/detail, and consent-gated enquiry persistence. Use the provisioned Neon staging database and existing Vercel Preview/Development environment. Do not fabricate listings, partners, identity, legal approvals or test results. Do not merge or deploy production.

This file exists to create a push event because GitHub issue-comment and workflow-dispatch triggers are not available until the workflow is present on the repository's default branch. The workflow is intentionally still on its feature branch and must not be merged merely to dispatch a run.
