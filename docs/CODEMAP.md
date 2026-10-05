# Code map

Paths are relative to repository root. A crate's `src/lib.rs` lists its public API and private modules. Start with the task owner and follow only the calls needed for the change.

## Task to file

| Task | Primary owner | Related boundary |
| --- | --- | --- |
| Link folder, make snapshot, pair device, import/export invitation | `crates/core/src/lib.rs` | Snapshot, crypto, and storage APIs |
| Local status, compare with online peer, receive from peer, recover | `crates/core/src/workflows.rs` | `peer.rs`, transfer API |
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
| Announcements, envelopes, ACK state, cursor | `crates/storage/src/mailbox.rs` | Future mailbox client; no HTTP client exists yet |
| QUIC/TCP/Noise/Yamux and behavior configuration | `crates/network/src/behaviour.rs` | Config, codec, discovery |
| Swarm ownership, request authorization, preferred connections | `crates/network/src/actor.rs` | Discovery and status |
| Private rendezvous, relay reservation, QUIC/TCP/circuit retry | `crates/network/src/discovery.rs` | Pinned infrastructure and trusted cards |
| Bounded command/RPC interface | `crates/network/src/handle.rs` | Actor |
| PeerId/card key binding | `crates/network/src/identity.rs` | Crypto signatures |
| Online/route/transport reporting | `crates/network/src/status.rs` | Actor; equality is a core decision |
| Binary chunk and bounded JSON frames | `crates/network/src/codec.rs` | Peer message types |
| Device/project keys, signatures, mailbox encryption, DPAPI | `crates/crypto/src/lib.rs` | Protocol signing bytes/AAD |
| Scanner, chunk store, safe paths, Windows file/project locks | `crates/snapshot/src/lib.rs` | Manifest model and transfer disk operations |
| Manifest/path validation, signing serialization, peer/IPC/mailbox types | `shared/protocol/src/lib.rs` | All crates |

## Public call paths

```text
snapshot: Engine::snapshot → ProjectLock → snapshot::scan → Identity::sign_snapshot → Store::commit_local
check:    Engine::check_peer → fetch_snapshot_chain → validate_snapshot → lineage → peer Verified ACK
receive:  Engine::receive_from_peer → fetch_snapshot_chain → Receiver::receive → download → apply
recover:  Engine::recover → recover_project → trusted target validation → apply
serve:    PeerRuntime → transfer::serve → project authorization → head/manifest/indexed chunk lookup
```

`PeerRuntime` owns the network actor/responder lifetime. `Network` is a handle, not another swarm. SQLite ownership remains in `Store`; each domain module adds methods to that same type.

## Targeted search examples

```powershell
rg -n 'commit_local|begin_apply|finish_apply' crates/storage/src crates/transfer/src
rg -n 'TransferState|requires_recovery' crates/storage/src crates/transfer/src
rg -n 'PROTOCOL_VERSION|signing_bytes|aad' shared/protocol/src crates/crypto/src
rg -n 'send_request|authorized|P2pCircuit' crates/network/src
```

Exclude `target`, ignored scratch/data folders, and `.git` from broad searches. If parent ignore rules hide source, use `rg --no-ignore` against the explicit `crates`, `shared`, or `docs` path rather than the whole drive.
