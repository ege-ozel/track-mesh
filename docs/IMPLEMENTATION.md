# TrackMesh MVP implementation

**Phases 1–2 are implemented and runnable through the Windows Rust CLI.** Development now takes place in `D:\CodeSpace\track-mesh`.

The repository follows the eight requested phases. Compilation, static checks, and manual operation are the validation tools. No automated tests, benchmarks, project watchers, cloud file storage, or continuous synchronization have been added.

## Phase status

| Phase | Status | Deliverable |
| --- | --- | --- |
| 1 | Implemented | Rust workspace, DPAPI device/project keys, signed immutable manifests, fixed 4 MiB BLAKE3 chunks, local chunk store, and transactional SQLite history. |
| 2 | Implemented | Explicit mutual pairing, direct QUIC, TCP with Noise/Yamux fallback, signed ancestry comparison, missing-chunk transfer, durable resume, verified installation, and journal recovery. Private rendezvous, relay, and DCUtR client behaviors are integrated. |
| 3 | Pending | Encrypted signed metadata mailbox and separate relay/rendezvous service. |
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

No runtime regression checks or automated tests/benchmarks were run for this refactor. Historical Phase 2 manual checks above remain historical; real two-PC NAT/relay/DCUtR validation and Phases 3–8 remain pending. This refactor/documentation task is complete; Phase 3 has not started.

## Remaining integration work

Real two-PC firewall behavior, Internet NAT traversal, circuit relay, and DCUtR have not been manually exercised. They require the Phase 3 infrastructure deployment and real network environments. Mailbox publication is still pending implementation: authored snapshots retain `PENDING`, and direct transfers do not imply a server acknowledgement.

Snapshot chunks are retained locally without automatic pruning. This keeps earlier snapshots retrievable when the working folder changes. The future mailbox receives small encrypted announcements only; file manifests and chunks travel through authenticated peer connections.
