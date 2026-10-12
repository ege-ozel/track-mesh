# TrackMesh Supabase backend (M5d-server)

This directory contains the Supabase server side for signed endpoint metadata. It does not contain the Rust client, invite-drop endpoints, or a deployed project. Project UUIDs are opaque routes, not project content. Endpoint ciphertext is the only endpoint payload stored; plaintext addresses, project keys, and invite contents never go to Supabase.

## Before deployment

- Review the owner decisions and signing contract in [`docs/plans/phase1-supabase-backend.md`](../../docs/plans/phase1-supabase-backend.md), especially TOFU `project-claim`, member grants/revocations, and the exact tuple bytes.
- Use a dedicated Supabase project per environment and keep its owner account protected with MFA.
- The client receives only the Project URL and publishable key (or legacy anon key). Those values identify the project and do not grant membership.
- Do not copy the `service_role`/secret key into this repository, a TrackMesh client, a `.env` committed to Git, a support ticket, or logs. Edge Functions use Supabase's injected `SUPABASE_SERVICE_ROLE_KEY` environment variable only inside server code. The value must never be returned in a response or written to a log.
- The Functions disable gateway JWT verification because TrackMesh authenticates requests with its own Ed25519 signature. Every request must pass that verification and the SQL ACL/nonce checks. A public API key or function URL is not authorization.

## Supabase Dashboard setup

1. Create a Supabase project and choose its region and plan. Copy the Project URL and publishable key into the TrackMesh project configuration only after the M5d-client work is available.
2. In **SQL Editor**, apply [`supabase/migrations/20261012000100_trackmesh_m5d.sql`](supabase/migrations/20261012000100_trackmesh_m5d.sql) once. Check that the four tables have RLS enabled, no `anon`/`authenticated` policies or grants, and that the RPC is executable only by `service_role`.
3. In **Edge Functions**, deploy `project-claim`, `member-grant`, `member-revoke`, `endpoint-publish`, and `endpoint-list`. Use the function source bundle so `supabase/functions/_shared/http.ts` is included. Keep JWT verification disabled for these five functions; the committed `supabase/config.toml` records that setting.
4. Confirm the Edge Function runtime provides `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` as server environment variables. Do not paste either value into function source. The latter is privileged and must remain inside the Edge Function runtime.
5. Keep the client URL/key separate from the Function secret. A deployment without the injected privileged key must fail closed; do not work around it by putting a secret key in the desktop application.

The Dashboard editor is useful for an initial project and inspection. The repository remains the source of truth for migration and Function source; Dashboard edits do not update this checkout. Prefer the CLI path below for repeatable deployments.

## Supabase CLI setup

Run from this directory with the official Supabase CLI installed and authenticated. The project ref and database password are deployment inputs; do not store a database password in a tracked file.

```powershell
supabase login
supabase link --project-ref <project-ref>
supabase db push
supabase functions deploy project-claim --no-verify-jwt
supabase functions deploy member-grant --no-verify-jwt
supabase functions deploy member-revoke --no-verify-jwt
supabase functions deploy endpoint-publish --no-verify-jwt
supabase functions deploy endpoint-list --no-verify-jwt
```

`supabase db push` applies committed migrations to the linked project. Review the target ref before running it. The functions expect Supabase's injected `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY`; this package does not define or track a copy of either secret. Local `supabase start` / `supabase functions serve` can be used for development, but never paste local service keys into repository files or agent/chat logs. `supabase/config.toml` contains no project ref, URL, API key, or secret.

## API contract and limits

All five routes accept POST and the `x-trackmesh-auth` header. The body hash is SHA-256 over the exact request body bytes. The signed tuple is the documented `serde_json` tuple with domain `trackmesh-supabase-request-v1`; timestamps use UTC RFC3339 with exactly millisecond precision. Each route checks the expected operation scope, Ed25519 signature, ±2 minute timestamp window, nonce format, bounded body, and strict JSON fields. `endpoint-publish` has a 64 KiB request cap; endpoint pages contain at most 8 rows and use the endpoint-only monotonic cursor.

Database state changes happen in one `trackmesh_dispatch` Postgres function invocation. That transaction serializes per-project operations, validates active membership and ACL rules, consumes the request nonce, updates the per-device and per-project fixed-window rate counters, then writes the claim/grant/revocation/endpoint row. `project_members.last_endpoint_seq` retains each device's endpoint replay watermark after short-lived ciphertext rows are deleted. Errors return a generic response without the database error text. Functions do not log headers, bodies, ciphertext, public keys, signatures, API keys, or SQL errors.

Initial fixed-window limits are:

| Operation | Per device | Per project |
| --- | ---: | ---: |
| `project-claim` | 3/hour | 10/hour |
| `member-grant` / `member-revoke` | 10/hour | 30/hour |
| `endpoint-publish` | 30/minute | 120/minute |
| `endpoint-list` | 60/minute | 180/minute |

These are abuse-control starting limits, not a Supabase platform quota. Failed/expired requests are generic. The cleanup batch removes at most 500 expired endpoint rows, 1,000 old nonces, and 1,000 old rate buckets per accepted operation. It runs inside the RPC transaction; repeated activity drains backlog. There is no `pg_cron` dependency.

## Current boundaries and remaining validation

- `project-claim` is trust-on-first-use. A first claimant of a project UUID becomes owner. If another device claims the route first, the real device gets the same generic failure; continue with direct peer/manual invite-file setup. There is no remote reset/recovery operation.
- Any active member may grant a paired device as a member after local pairing proof and explicit inviter approval. At most three project devices may be active. Any active member may revoke itself; only the owner can revoke another device. If the owner revokes itself, no owner remains and the route cannot be claimed again. Owner recovery/transfer is not implemented.
- M6 invite drops and their IP/ID limits are deliberately absent from this migration and Functions. They are a separate milestone.
- This work has not been deployed. Live RLS/grants, Edge Function secrets/JWT settings, simultaneous claim and revoke races, WebCrypto Ed25519 behavior in Supabase's runtime, rate limits, expiry cleanup, and endpoint pagination still require staging validation. Real Supabase availability is not assumed; the client must keep direct connections and the manual invitation file workflow available.

No tests or benchmarks are added or run by this package.
