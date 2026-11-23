# Security gate

Review changed code and affected runtime for authentication/authorization, sessions, secret handling, SSRF, DNS rebinding, injection, path traversal, unsafe deserialization, CSRF/CORS/CSP, rate limits, RLS/tenant isolation, sensitive logging, dependency and CI supply-chain risk. For every finding provide severity, affected path, reachable exploit scenario, impact, minimal fix and regression test. Run available scanners without printing secrets. Do not weaken controls or modify production. State what was not inspected.
