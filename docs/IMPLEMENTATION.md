# TrackMesh MVP implementation

**Phases 1–3 are implemented locally through the Windows Rust CLI and separate infrastructure services.** Development takes place in `D:\CodeSpace\track-mesh`. Real HTTPS deployment, two-PC NAT/relay/DCUtR validation, and Phases 4–8 remain unfinished.

The repository follows the eight requested phases. Compilation, static checks, and manual operation are the validation tools. No automated tests, benchmarks, project watchers, cloud file storage, or continuous synchronization have been added.

## Phase status

| Phase | Status | Deliverable |
| --- | --- | --- |
| 1 | Implemented | Rust workspace, DPAPI device/project keys, signed immutable manifests, fixed 4 MiB BLAKE3 chunks, local chunk store, and transactional SQLite history. |
| 2 | Implemented | Explicit mutual pairing, direct QUIC, TCP with Noise/Yamux fallback, signed ancestry comparison, missing-chunk transfer, durable resume, verified installation, and journal recovery. Private rendezvous, relay, and DCUtR client behaviors are integrated. |
| 3 | Implemented locally; deployment/NAT unverified | Encrypted signed metadata publication/refresh, durable ACK/retry/cursor flow, Axum/SQLite opaque mailbox behind HTTPS, separate allow-listed relay/rendezvous service. |
| 4 | Pending | Tauri 2 / React desktop controls and status. |
| 5 | Pending | Official Steinberg SDK C++ VST3 surface and Rust Windows Named Pipe bridge. |
| 6 | Pending | Settled save processing and publication retry. |
| 7 | Pending | Session-aware agent lifetime after DAW shutdown. |
| 8 | Pending | Desktop/VST remote-history, offline-availability, and send/receive workflow. CLI lineage and connection reporting are already available for Phase 2. |

## Phase 2 implementation

- A signed device card binds the application Ed25519 key to its libp2p PeerId. Connections accept explicitly paired devices and pinned infrastructure; each data request also requires project authorization.
- QUIC is attempted first, followed by authenticated direct TCP and then configured relay circuits. No public DHT or multicast discovery is used. Rendezvous namespaces derive from the shared project key, and results are filtered against trusted peers.
- Eight concurrent binary chunk streams use bounded messages. Cached chunks are rehashed before reuse. Corrupted or missing chunks are downloaded, checked with BLAKE3, flushed, atomically installed in the local chunk store, and recorded durably.
- Received files are assembled on the project volume, checked against full-file hashes, and flushed before atomic installation. The working folder must match its committed base; unsnapshotted changes and divergent or incomplete history block receive.
- An explicit `BACKUP → INSTALL → COMMIT → DONE` journal supports interruption recovery. Originals remain in a sibling backup folder. SQLite advertises the received snapshot as current only after the complete folder matches its signed manifest.
- Windows project locks serialize local snapshot/receive/recovery operations. Snapshot files, chunk references/catalog, and the local head commit in one SQLite transaction. No filesystem watcher or permanent daemon is installed.

## Build and manual validation

The manual-operation records below describe the Phase 2 implementation before the modularization. They are historical evidence, not a repeated runtime validation of the refactor.

Manual checks used separate device identities, databases, and project folders on one Windows machine, connected over loopback. The fixture contained dummy DAW project files, empty files, large sample data, and many small files. These checks do not establish DAW-host compatibility or behavior across Internet NATs.

| Manual operation | Observed result |
| --- | --- |
| Mutual pairing and direct QUIC receive | Completed; independently compared file hashes matched. Both peers acknowledged the same signed snapshot. |
| Incremental sample change | Only the affected 4 MiB chunk required transfer; unchanged chunks were reused. |
| Source interrupted during download | Transfer became `PAUSED`; restarting reused completed chunks in the same transfer session. |
| Cached received chunk deliberately corrupted | Resume rejected the corrupted chunk and downloaded it again. Final file hashes matched. |
| QUIC endpoint unavailable, TCP reachable | Connected as `DIRECT` over TCP with Noise/Yamux; online comparison reported verified equality. |
| Unpaired third device | Project access rejected; no snapshot was delivered. |
| Receiver stopped during `INSTALL` | `recovery_required` stayed set; explicit recovery completed locally and cleared it. Installed hashes matched. |
| Unsnapshotted local edits | Receive refused and preserved the working folder. |
| Both devices authored children of the same parent | Comparison reported `DIVERGED`; receive refused to overwrite local history. |

Build commands, run without automated test or benchmark targets:

```powershell
cargo check --workspace --locked --offline
cargo clippy --workspace --lib --bins --locked --offline -- -D warnings
cargo fmt --all -- --check
cargo build -p trackmesh-core --locked --offline
```

See [the Phase 2 operation guide](PHASE2.md) for the CLI workflow.

## Modularization and agent documentation

The structural refactor retains the seven workspace crates and their public exports. Transfer code is split into receiver, verification/serving, download, assembly, journal, durable apply orchestration, installation, and recovery. Storage keeps one connection owner with domain modules for projects, trust, snapshots, transfers, and mailbox persistence. Network separates transport construction, command/event ownership, discovery/retries, identity, framing, and status.

`TransferState` replaces string-based session states. SQLite schema v2 and the six supported persisted/JSON labels are unchanged. Journal phases and signed/wire models remain unchanged. Core exposes reusable local status, online comparison, receive, and recovery workflows; the CLI delegates those decisions.

Agent entry points, local crate instructions, a task-to-file code map, architecture invariants, the accepted MVP contract, and development guidance provide focused reading routes. `docs/INDEX.md` links the documentation; this file remains the single phase/progress record.

Applied to the canonical `D:\CodeSpace\track-mesh` repository on 2026-10-05. Before writing, `apply_trackmesh_refactor.py --check` validated all 41 packaged files without conflicts and confirmed the seven unchanged compatibility contracts. The post-application check reported zero pending files.

Canonical checks completed successfully with Rust/Cargo 1.98.1:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked --offline` | Passed |
| `cargo clippy --workspace --lib --bins --locked --offline -- -D warnings` | Passed |
| `cargo build -p trackmesh-core --locked --offline` | Passed; Windows agent linked |

Initial build attempts lacked the MSVC linker and C++ header environment. The successful build initialized process-local paths for installed MSVC 14.44.35207 and Windows SDK 10.0.26100.0 (`PATH`, `INCLUDE`, `LIB`, and toolchain environment). Workspace check and strict Clippy also passed in that environment. No compiler installation, dependency, or repository build configuration was changed; the development guide now states the developer-shell prerequisite.

Final inspection confirmed all 37 public storage method names, 23 local documentation links, and byte-for-byte preservation of `Cargo.toml`, `Cargo.lock`, the SQLite schema, shared protocol/signing models, crypto, scanner, and network codec. `init.txt` and files outside the packaged source/documentation set were preserved.

No runtime regression checks or automated tests/benchmarks were run for this refactor. At that handoff, Phase 3 had not started. The separate Phase 3 work and its own validation evidence are recorded below.

## Phase 3 implementation and validation

Implemented on the local `codex/phase-3-mailbox-relay` branch after committing and pushing the Phase 1–2/refactor baseline (`9aa0336`). Phase work now uses a separate local branch, followed by validated commit and local merge into `master`; [DEVELOPMENT.md](DEVELOPMENT.md) records that workflow.

- `crates/mailbox` provides bounded HTTPS transport with certificate/hostname validation, no redirects, signed request authentication, endpoint/project binding, bounded ACK/pages, and an explicit literal-loopback HTTP development option.
- Core owns own-author publication, persists a single envelope before POST, reuses it on failure, and accepts `PUBLISHED` only after a matching ACK against the durable envelope. CLI adds `publish` and `refresh`; `serve` retries metadata every 30 seconds during its explicit session.
- Refresh checks ordered bounded records, project, trusted author/key, signature, authenticated decryption, and immutable metadata consistency. Local storage commits a whole page and expected cursor transactionally, rechecking author authorization. Metadata does not advance the local head or create online equality/file-availability claims.
- `services/mailbox` supplies the Axum API and independent SQLite opaque record store, signed-card project/device ACL, persistent nonce replay protection, freshness/rate/quota/concurrency/body limits, immutable idempotent ACKs, and loopback backend binding. Production HTTPS is configured through the documented reverse proxy example.
- `services/relay` supplies separate QUIC/TCP Noise/Yamux relay/rendezvous, persistent DPAPI infrastructure identity, signed-card allow-list with fail-closed reload outside its swarm, connection/registration/circuit limits, and explicit external addresses. Each instance belongs to a private group of at most three devices.

Existing protocol version/names, manifest/envelope/card signing bytes, envelope AAD, chunk framing, local SQLite schema v2/values, project locks, snapshot/apply transactions, and journal/recovery ordering remain unchanged. HTTP authentication adds a separate signed model/domain; no project-file cloud storage, watcher, permanent agent daemon, automated tests, or benchmarks were added. Workspace now includes the mailbox client and two service packages in addition to the original seven crates.

Final static/build checks ran successfully with Rust/Cargo 1.98.1 and the process-local MSVC/Windows SDK environment described above:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked --offline` | Passed |
| `cargo clippy --workspace --lib --bins --locked --offline -- -D warnings` | Passed |
| `cargo build -p trackmesh-core -p trackmesh-mailbox-service -p trackmesh-relay-service --locked --offline` | Passed; all three Windows executables linked |
| CLI help for all three executables | Passed; publication/refresh and service options present |
| Git whitespace check and local documentation link inspection | Passed |

Manual operations used only separate disposable device/project/service directories under ignored `.build`, on one Windows PC. Normal-user execution was needed for current-user DPAPI; sandbox execution could not protect the fixture identity. No production project folder, database, chunk store, or recovery backup was modified. These operations are not an automated test suite.

| Phase 3 manual operation | Observed result |
| --- | --- |
| Authored snapshot, encrypted POST, matching ACK | Published once; local label `PUBLISHED`, pending count zero. |
| Refresh while source peer service was not running | B saw A's signed decrypted metadata as `REMOTE`; local head and last verification stayed null. No project files were downloaded. |
| Loopback HTTP without development switch | Request refused before HTTP. |
| Signed request from a device absent from service ACL | HTTP 403; no metadata delivered. |
| Loopback proxy dropped an already committed POST response | Local envelope stayed `PENDING` and byte-equivalent to the sent envelope. Retry ACKed the same server sequence; service record count stayed two. |
| Replay identical signed HTTP request | HTTP 401. After stopping/restarting the service with the same database, a fresh captured request (about 16 seconds old) still produced 401; its nonce was retained. |
| Deliberately changed fixture ciphertext | Signature verification failed; B's cursor/history remained at their prior values. Restoring the opaque row allowed normal refresh. |
| Start explicit `serve` with new pending authored snapshot | Metadata worker published it without a separate `publish` call. |
| Two paired devices with empty direct-address cards and configured private infrastructure | Online comparison reached the source through discovery and ended on direct TCP, reporting `REMOTE_AHEAD` and no equality ACK. This does not prove forced circuit transfer or Internet hole punching. |
| Final binaries: publish then refresh | New snapshot ACKed; B advanced metadata cursor to five while head/last verification stayed null. |
| Invalid mailbox ACL during final-binary refresh | HTTP 503; access failed closed. |

The Phase 3 operation/deployment guide is [PHASE3.md](PHASE3.md). A real HTTPS proxy/certificate deployment, forced circuit data transfer, relay revocation behavior under active traffic, and real two-PC firewall/NAT/DCUtR remain unverified. No throughput benchmark or complete Phase 2 runtime regression was run for this change.

## Remaining integration work

Deploy the Phase 3 services with real DNS, valid HTTPS certificates, private ACLs, pinned infrastructure PeerId, and reachable advertised addresses; then validate on two real Windows PCs. Real firewall behavior, Internet NAT traversal, forced circuit relay, and DCUtR remain unverified. Direct transfers still do not imply a mailbox acknowledgement.

Snapshot chunks are retained locally without automatic pruning. This keeps earlier snapshots retrievable when the working folder changes. The mailbox receives small encrypted announcements only; file manifests and chunks travel through authenticated peer connections. Next implementation phase: create a new local `codex/phase-4-desktop` branch for Tauri 2/React controls and status; Phases 5–8 remain after that.
