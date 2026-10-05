# Code map

Paths are relative to repository root. A crate's `src/lib.rs` lists its public API and private modules. Start with the task owner and follow only the calls needed for the change.

## Task to file

| Task | Primary owner | Related boundary |
| --- | --- | --- |
| Link folder, pair device, import/export invitation | `crates/core/src/lib.rs` | Snapshot, crypto, and storage APIs |
| Manual/settled snapshot creation, unchanged-content reuse and commit | `crates/core/src/snapshots.rs` | Project lock, snapshot scanner and transactional storage |
| Local status, compare with online peer, receive from peer, recover | `crates/core/src/workflows.rs` | `peer.rs`, transfer API |
| Bounded project/peer metadata cards | `crates/core/src/status.rs` | Narrow storage queries; existing CLI status unchanged |
| Manual snapshot commit followed by publication | `crates/core/src/sending.rs` | Snapshot workflow and durable mailbox ACK |
| Windows plugin pipe, binding, status and send queue | `crates/core/src/ipc/mod.rs`, `ipc/security.rs`, `ipc/status.rs` | Existing plugin protocol; shared Engine/network |
| Project save debounce, coalescing and accepted-work drain | `crates/core/src/ipc/jobs.rs` | Engine settled send; bounded per-project revisions |
| VST3 factory, audio pass-through, UUID state/signals | `plugins/vst3/src/plugin.cpp`, `plugin.h` | Official Steinberg SDK; Rust owns decisions |
| Native VST3 view, IPC worker and bundled-owner launch | `plugins/vst3/src/view.cpp`, `bridge.cpp` | No audio-thread IPC or file/network work |
| Windows desktop entry point, capability inventory, navigation | `apps/desktop/src-tauri/src/lib.rs`, `build.rs`, `capabilities/main.json` | Tauri adapter; local bundled React |
| Desktop jobs, status freshness, explicit session shutdown | `apps/desktop/src-tauri/src/runtime.rs` | Core workflows and one leased `PeerRuntime` |
| Native invitation/card files and folder dialogs | `apps/desktop/src-tauri/src/documents.rs` | Bounded private documents; payloads stay in Rust |
| Main-window command boundary and typed frontend | `apps/desktop/src-tauri/src/commands.rs`, `apps/desktop/src/api.ts` | Narrow invoke API |
| Desktop project controls and styling | `apps/desktop/src/App.tsx`, `src/styles.css` | Core-derived state, no filesystem watcher |
| Start/stop peer runtime, answer disk requests, fetch signed ancestry | `crates/core/src/peer.rs` | Network actor and transfer serving |
| CLI arguments, JSON output, Ctrl+C | `crates/core/src/main.rs` | Core workflows; keep decisions out of this adapter |
| Receive decisions, local lock, failure state | `crates/transfer/src/receiver.rs` | Verification, download, apply |
| Chunk reuse, concurrency, retries, durable progress | `crates/transfer/src/download.rs` | Chunk store, network handle, storage transfers |
| Author/signature/project binding, preflight folder comparison | `crates/transfer/src/verification.rs` | Crypto and snapshot APIs |
| Serve head/manifest/chunk/verification requests | `crates/transfer/src/serving.rs` | Storage authorization and chunk index |
| Assemble/fsync files outside working folder | `crates/transfer/src/assemble.rs` | Chunk store |
| Durable phase orchestration | `crates/transfer/src/apply.rs` | Journal, installation, storage transactions |
| Backup originals, install files, verify final folder/commit head | `crates/transfer/src/installation.rs` | Snapshot path/file helpers |
| Journal format and atomic persistence | `crates/transfer/src/journal.rs` | Snapshot atomic replacement |
| Explicit interrupted-apply recovery | `crates/transfer/src/recovery.rs` | Apply and trusted target validation |
| SQLite open/schema migration/mutex | `crates/storage/src/lib.rs`, `src/schema.sql` | Domain storage modules |
| Persisted structs and row decoding | `crates/storage/src/models.rs` | `transfer_state.rs` |
| Six transfer labels and SQLite/JSON conversion | `crates/storage/src/transfer_state.rs` | Transfers and receiver/recovery |
| Project links and errors | `crates/storage/src/projects.rs` | Core |
| Device identity bindings and per-project authorization | `crates/storage/src/trust.rs` | Network/transfer requests |
| Immutable snapshot commit, indexed chunk authorization, lineage | `crates/storage/src/snapshots.rs` | Snapshot/apply |
| Session creation, progress, transactional apply markers | `crates/storage/src/transfers.rs` | Receiver/apply |
| Publication/retry and verified page ingestion | `crates/core/src/mailbox.rs` | `mailbox` transport and transactional storage |
| Announcements, immutable envelopes, ACK state, transactional page/cursor | `crates/storage/src/mailbox.rs` | Core publication/refresh workflows |
| Bounded HTTPS requests and response validation | `crates/mailbox/src/http.rs` | Request auth; no project decisions |
| Mailbox listener, authenticated project ACL, opaque SQLite records | `services/mailbox/src/main.rs`, `src/api.rs`, `src/store.rs` | Separate service database; HTTPS proxy |
| Private relay/rendezvous actor, allow-list, resource bounds | `services/relay/src/main.rs`, `src/actor.rs`, `src/behaviour.rs` | Existing pinned client protocols |
| QUIC/TCP/Noise/Yamux and behavior configuration | `crates/network/src/behaviour.rs` | Config, codec, discovery |
| Swarm ownership, request authorization, preferred connections | `crates/network/src/actor.rs` | Discovery and status |
| Private rendezvous, relay reservation, QUIC/TCP/circuit retry | `crates/network/src/discovery.rs` | Pinned infrastructure and trusted cards |
| Bounded command/RPC interface | `crates/network/src/handle.rs` | Actor |
| PeerId/card key binding | `crates/network/src/identity.rs` | Crypto signatures |
| Online/route/transport reporting | `crates/network/src/status.rs` | Actor; equality is a core decision |
| Binary chunk and bounded JSON frames | `crates/network/src/codec.rs` | Peer message types |
| Device/project keys, signatures, mailbox encryption, DPAPI | `crates/crypto/src/lib.rs` | Protocol signing bytes/AAD |
| Scanner, chunk store, safe paths, Windows file/project locks | `crates/snapshot/src/lib.rs` | Manifest model and transfer disk operations |
| Event-triggered inventory quiet period and writer-lock settlement | `crates/snapshot/src/settlement.rs` | Existing guarded scanner; no watcher |
| Manifest/path validation, signing serialization, peer/IPC/mailbox types | `shared/protocol/src/lib.rs` | All crates |
| Signed HTTP request scope, timestamp, nonce and body binding | `shared/protocol/src/mailbox.rs`, `crates/crypto/src/mailbox.rs` | Existing envelope signing/AAD unchanged |

## Public call paths

```text
snapshot: Engine::snapshot → ProjectLock → snapshot::scan → Identity::sign_snapshot → Store::commit_local
check:    Engine::check_peer → fetch_snapshot_chain → validate_snapshot → lineage → peer Verified ACK
receive:  Engine::receive_from_peer → fetch_snapshot_chain → Receiver::receive → download → apply
recover:  Engine::recover → recover_project → trusted target validation → apply
serve:    PeerRuntime → transfer::serve → project authorization → head/manifest/indexed chunk lookup
publish:  Engine::publish_metadata → persist envelope → MailboxClient POST → matching ACK → PUBLISHED
refresh:  Engine::refresh_metadata → bounded page → trust/signature/AEAD → page + cursor transaction
desktop:  typed Tauri command → bounded tracked job → Engine workflow → compact status page
send:     Engine::send_snapshot → blocking snapshot commit → publish_metadata → ACK or durable PENDING
plugin:   VST3 worker → bounded per-user Named Pipe → PluginRuntime → Engine summary/send_snapshot
save:     SAVE_OBSERVED → project debounce → snapshot_after_save → scan_after_save → commit → publish/ACK
```

`PeerRuntime` owns the network actor/responder lifetime. `MailboxRuntime` owns session-scoped metadata retry. `Network` is a handle, not another swarm. SQLite ownership remains in `Store`; each domain module adds methods to that same type. Infrastructure services own independent stores/swarms in their separate processes.

`PeerRuntime` holds an exclusive Windows data-directory lease shared with its actor/responder tasks. Desktop owns its window/job lifetime and calls `stop` after awaiting local work. Status summaries use bounded storage queries in `projects.rs`, `snapshots.rs`, and `transfers.rs`; `local_status` retains its complete CLI output.

## Targeted search examples

```powershell
rg -n 'commit_local|begin_apply|finish_apply' crates/storage/src crates/transfer/src
rg -n 'TransferState|requires_recovery' crates/storage/src crates/transfer/src
rg -n 'PROTOCOL_VERSION|signing_bytes|aad' shared/protocol/src crates/crypto/src
rg -n 'send_request|authorized|P2pCircuit' crates/network/src
```

Exclude `target`, ignored scratch/data folders, and `.git` from broad searches. If parent ignore rules hide source, use `rg --no-ignore` against the explicit `crates`, `shared`, or `docs` path rather than the whole drive.
