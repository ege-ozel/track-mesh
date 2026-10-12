# TrackMesh Supabase backend (M5d)

This directory contains the Supabase server side for signed endpoint metadata, its Rust client contract, and encrypted single-read invite drops. The client implementation lives in `crates/mailbox`, `crates/crypto`, `crates/core`, and the Windows Desktop adapter. The backend has not been deployed. Project UUIDs are opaque routes, not project content. Endpoint ciphertext and invite ciphertext are stored; plaintext addresses, project keys, and invite contents never go to Supabase.

## Before deployment

- Review the owner decisions and signing contract in [`docs/plans/phase1-supabase-backend.md`](../../docs/plans/phase1-supabase-backend.md), especially TOFU `project-claim`, member grants/revocations, and the exact tuple bytes.
- Use a dedicated Supabase project per environment and keep its owner account protected with MFA.
- The client receives only the Project URL and publishable key (or legacy anon key). Those values identify the project and do not grant membership.
- Do not copy the `service_role`/secret key into this repository, a TrackMesh client, a `.env` committed to Git, a support ticket, or logs. Edge Functions use Supabase's injected `SUPABASE_SERVICE_ROLE_KEY` environment variable only inside server code. The value must never be returned in a response or written to a log.
- The Functions disable gateway JWT verification because TrackMesh authenticates requests with its own Ed25519 signature. Every request must pass that verification and the SQL ACL/nonce checks. A public API key or function URL is not authorization.

## Supabase Dashboard setup

1. Create a Supabase project and choose its region and plan. Copy the Project URL and publishable key into the TrackMesh project configuration only after the M5d-client work is available.
2. In **SQL Editor**, apply [`supabase/migrations/20261012000100_trackmesh_m5d.sql`](supabase/migrations/20261012000100_trackmesh_m5d.sql) once. Check that the four tables have RLS enabled, no `anon`/`authenticated` policies or grants, and that the RPC is executable only by `service_role`.
3. Apply [`supabase/migrations/20261012000200_m6_invite_drop.sql`](supabase/migrations/20261012000200_m6_invite_drop.sql) once after the M5d migration. Check that `invite_drops`, `invite_request_nonces`, and `invite_rate_limits` have RLS enabled and no `anon`/`authenticated` policies or grants.
4. In **Edge Functions**, deploy `project-claim`, `member-grant`, `member-revoke`, `endpoint-publish`, `endpoint-list`, `invite-create`, and `invite-consume`. Use the function source bundle so `supabase/functions/_shared/http.ts` is included. Keep JWT verification disabled for all seven functions; the committed `supabase/config.toml` records that setting.
5. Confirm the Edge Function runtime provides `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` as server environment variables. Do not paste either value into function source. The latter is privileged and must remain inside the Edge Function runtime.
6. Keep the client URL/key separate from the Function secret. A deployment without the injected privileged key must fail closed; do not work around it by putting a secret key in the desktop application.

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
supabase functions deploy invite-create --no-verify-jwt
supabase functions deploy invite-consume --no-verify-jwt
```

`supabase db push` applies committed migrations to the linked project. Review the target ref before running it. The functions expect Supabase's injected `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY`; this package does not define or track a copy of either secret. Local `supabase start` / `supabase functions serve` can be used for development, but never paste local service keys into repository files or agent/chat logs. `supabase/config.toml` contains no project ref, URL, API key, or secret.

## API contract and limits

The five M5d routes accept POST and the `x-trackmesh-auth` header. `invite-create` uses the same signed header and scope; `invite-consume` is intentionally unsigned because a recipient has not joined the project yet. The body hash is SHA-256 over exact request bytes. The signed tuple uses `trackmesh-supabase-request-v1`; timestamps use UTC `YYYY-MM-DDTHH:mm:ss.SSSZ` and nonce is lowercase canonical UUIDv4. Authenticated routes check scope, Ed25519 signature, ±2 minute timestamp window, nonce, bounded body, and strict JSON fields.

M5d state changes happen in one `trackmesh_dispatch` Postgres function invocation. M6 uploads call `trackmesh_invite_create`, which verifies the active signer membership and consumes its nonce in one transaction. `invite-consume` calls `trackmesh_invite_consume`; its conditional `UPDATE ... RETURNING` consumes a drop atomically so concurrent recipients cannot both obtain it. The `invite_drops` table has no project route or invite plaintext. Both new tables and all RPCs are inaccessible to `anon`/`authenticated`; RLS is deny-all. Errors are generic. Functions do not log headers, bodies, ciphertext, public keys, signatures, API keys, or SQL errors.

Invite IDs are 256 random bits encoded base64url. The link fragment contains the separate 256-bit decryption key. Ciphertext stays under 64 KiB and expires within 15 minutes. `invite-create` requires a valid active project member signature. `invite-consume` accepts only an ID because the recipient is not yet a member; this allows a person who learns the locator to consume it and deny the intended recipient. To bound this denial-of-service path, create is globally limited to 60/hour, consume to 300/hour globally, and each ID to 10 consume attempts/hour. The unauthenticated global limits can themselves be exhausted to delay legitimate operations; no IP value is trusted or stored. Bounded cleanup runs in the RPC transaction on calls and removes at most 500 expired drops, 1,000 old nonces, and 1,000 old rate buckets.

Initial fixed-window limits are:

| Operation | Per device | Per project |
| --- | ---: | ---: |
| `project-claim` | 3/hour | 10/hour |
| `member-grant` / `member-revoke` | 10/hour | 30/hour |
| `endpoint-publish` | 30/minute | 120/minute |
| `endpoint-list` | 60/minute | 180/minute |
| `invite-create` | 60/hour global | 60/hour global |
| `invite-consume` | 300/hour global, 10/hour per ID | 300/hour global |

`project-claim` additionally has one global limit of 60/hour across all routes in this Supabase project. We do not use per-IP limiting: Supabase documents request header and log field names, but the documents inspected do not establish that the Edge Function receives a spoof-resistant client-IP header. The source remains `[V]`; global claim throttling limits random-route/key churn meanwhile.

The global limit is shared by every user of the Supabase project. An attacker can consume it and delay legitimate claims until the hourly window resets. This denial-of-service tradeoff is accepted for the expected small user base; per-IP limiting remains `[V]` until a trusted client-IP source is established.

The Rust client stores endpoint data as an XChaCha20-Poly1305 wrapper encrypted with the project key and AAD `trackmesh-endpoint-mailbox-v1`. Its JSON shape is `{version, project_route, writer_device_id, seq, expires_at, nonce, ciphertext}`. `seq` is a canonical decimal string; `expires_at` is UTC RFC3339 with exactly millisecond precision; `nonce` is 24 random bytes encoded base64url without padding; `ciphertext` is the encrypted serialized signed `EndpointRecord`, also base64url without padding. The client base64url-encodes the complete wrapper JSON into the `ciphertext` field sent to `endpoint-publish`. The outer `seq` and `expires_at` request fields must exactly match the wrapper and inner record. On receipt, the client checks all duplicated metadata before calling the shared trusted endpoint validation path.

These are abuse-control starting limits, not a Supabase platform quota. Failed/expired requests are generic. The cleanup batch removes at most 500 expired endpoint rows, 1,000 old nonces, and 1,000 old rate buckets per accepted operation. It runs inside the RPC transaction; repeated activity drains backlog. There is no `pg_cron` dependency.

## Current boundaries and remaining validation

- `project-claim` is trust-on-first-use. A first claimant of a project UUID becomes owner. If another device claims the route first, the real device gets the same generic failure; continue with direct peer/manual invite-file setup. There is no remote reset/recovery operation.
- Any active member may grant a paired device as a member after local pairing proof and explicit inviter approval. At most three project devices may be active. Any active member may revoke itself; only the owner can revoke another device. If the owner revokes itself, no owner remains and the route cannot be claimed again. Owner recovery/transfer is not implemented.
- M6 is implemented locally but has not been deployed. Live RLS/grants, Edge Function secrets/JWT settings, invite one-read concurrency, generic errors, rate limits, expiry cleanup, WebCrypto Ed25519 equivalence to Rust `verify_strict`, and endpoint paging still require staging validation. Real Supabase availability is not assumed; direct connections and manual invitation files remain available.

No tests or benchmarks are added or run by this package.
