# Product Factory run request — 2026-11-14

Owner request: deliver AUREL Elite Estate as a working staging MVP, without Fable; use Starimg `claude-opus-5` with local Qwen fallback.

Use the workflow's configured primary/fallback routing and acceptance gates. Target `Aloews/full`, working branch `factory/elite-estate-rc4`, base `foundation/luxury-estate-platform`.

Required user journey: verified email OTP sign-up/sign-in, agency workspace, property submission with source/evidence, independent review, verified-only public search/detail, and consent-gated enquiry persistence. Use the provisioned Neon staging database and existing Vercel Preview/Development environment. Do not fabricate listings, partners, identity, legal approvals or test results. Do not merge or deploy production.

## Execution retry — 2026-11-19

Trigger this request only by pushing a meaningful update under `.github/scripts/**` to `fix/factory-starimg-primary`, so the workflow definition on that branch runs. The default-branch workflow currently has a different provider route (OpenRouter primary, local Qwen pre-pass, Starimg disabled); an issue-comment trigger would run that older definition and is not an acceptable substitute.

Expected route on the feature branch:
1. Probe Starimg Anthropic Messages tool-use compatibility with the configured `claude-opus-5` model.
2. If Starimg is unavailable, returns HTTP 402, fails the tool-use probe, or the implementation action fails, continue to local `qwen2.5-coder:3b`; do not require an unverified secondary Starimg key.
3. Do not use Fable or silently substitute OpenRouter.
4. Verify each candidate in isolated jobs. Publish only a verified patch as a Draft/open PR to `Aloews/full` from `factory/elite-estate-rc4` against `foundation/luxury-estate-platform`.
5. Preserve the base-branch SHA guard. No PR merge, production deployment, production migration, secret/permission change or paid resource provisioning.

Acceptance evidence: workflow run and job/step logs; redacted Starimg diagnostic (including HTTP status/key slot only); Qwen model selection and inference result if fallback is used; candidate patch and exact base SHA; full verifier report; target-product PR and exact head SHA; Vercel Preview URL tied to that SHA; browser/API evidence for OTP, tenant isolation, property/evidence submission, independent approval, verified-only listing and enquiry persistence. CI success alone is not a staging MVP.
