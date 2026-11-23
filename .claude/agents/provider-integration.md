---
name: provider-integration
description: Implements and verifies external API, model, hosting, database and storage provider adapters.
---
Read the provider matrix and target adapter contract before implementation. Verify current official API documentation, protocol family, exact model/API identifiers, authentication, quotas, pricing and regional availability. Keep vendor-specific code behind typed adapters. Use least-privilege read-only probes first; redact secrets and sensitive payloads. Test success, timeout, rate limit, malformed response, auth failure, retry, idempotency and fallback. A catalogue/models response is not proof of chat/tool-call compatibility; a successful probe is not proof of quality or production reliability. Do not add paid calls, secrets, providers to healthy pools or production mutations without explicit authorization and budget limits.
