# TrackMesh MVP implementation

**Phases 1–8 are implemented locally through the Windows Rust CLI, Tauri 2/React desktop, official-SDK C++ VST3/Named Pipe surface, event-triggered settled save/publication, bounded remote/offline/divergence UX, and separate infrastructure services.** Development takes place in `D:\CodeSpace\track-mesh`. Actual DAW save/reopen/audio/hooks validation, real HTTPS deployment and two-PC NAT/relay/DCUtR validation remain unfinished.

The repository follows the eight requested phases. Compilation, static checks, and manual operation are the validation tools. No automated tests, benchmarks, project watchers, cloud file storage, or continuous synchronization have been added.

## Phase status

| Phase | Status | Deliverable |
| --- | --- | --- |
| 1 | Implemented | Rust workspace, DPAPI device/project keys, signed immutable manifests, fixed 4 MiB BLAKE3 chunks, local chunk store, and transactional SQLite history. |
| 2 | Implemented | Explicit mutual pairing, direct QUIC, TCP with Noise/Yamux fallback, signed ancestry comparison, missing-chunk transfer, durable resume, verified installation, and journal recovery. Private rendezvous, relay, and DCUtR client behaviors are integrated. |
| 3 | Implemented locally; deployment/NAT unverified | Encrypted signed metadata publication/refresh, durable ACK/retry/cursor flow, Axum/SQLite opaque mailbox behind HTTPS, separate allow-listed relay/rendezvous service. |
| 4 | Implemented locally; Windows native validation on one PC | Tauri 2 / React controls, pairing, bounded status, manual snapshot/publication, receive/resume/recovery, and safe window-session shutdown. |
| 5 | Implemented locally; SDK editor host operated; DAW/audio/state restore unverified | Official Steinberg SDK C++ VST3 UUID/control/status surface, private Windows Named Pipe bridge and shared desktop/agent owner. |
| 6 | Implemented locally; simulated saves and loopback ACK validated; real DAW hooks unverified | Per-project save debounce/coalescing, bounded writer/inventory settlement, guarded snapshot commit, durable publication retry and accepted-work drain. |
| 7 | Implemented locally; simulated IPC/window lifetime validated; real DAW termination unverified | Session/work/pending-publication retention, queued plugin-ending delivery and 60-second idle grace. |
| 8 | Implemented locally; disposable desktop/IPC validation | Desktop/VST bounded remote-history, offline-availability, divergence, and send/receive handoff. Real DAW and two-PC Internet operation remain unverified. |

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

## Phase 4 implementation and validation

Implemented on the separate local `codex/phase-4-desktop` branch from Phase 3 integration commit `d4483c5`. The native Windows executable bundles React/TypeScript/Vite assets; it needs WebView2. This phase provides manual desktop controls, not the complete VST/save/session workflow. Operation instructions are in [PHASE4.md](PHASE4.md).

- `apps/desktop` owns the window, four explicitly permitted main-window commands, native document/folder dialogs, bounded tracked jobs, and an explicit session. It uses one `PeerRuntime` plus the existing session-scoped metadata worker. Disk work runs in blocking workers. Closing stops new work, pauses unfinished network downloads, awaits started snapshot/install/recovery work, and drains the runtime.
- React cards display local snapshot/author/time, selected collaborator's last-known metadata, direct/relay/offline connection status, pending publication, durable transfer progress, lineage and recovery. Controls include link/import invitation, device cards, local revocation, Send Snapshot, Check, Refresh, Retry, Receive/Resume and Recover. Receiving/recovering requires a DAW-closed confirmation; core still enforces all integrity guards.
- Core owns reusable bounded project summaries and the explicit snapshot-then-publication workflow. A successful local commit remains visible when HTTP fails. Publish/refresh share a per-project coordinator with the worker, preserving durable envelope, ACK and cursor semantics. Storage adds narrow summary queries without loading complete history/manifests during polling; existing CLI JSON remains intact.
- A Windows file lease protects data-directory peer-runtime ownership across CLI and desktop, including actor/responder drain. Tauri's single-instance plugin focuses an existing desktop window. Default desktop listening port is 43101; the existing data-directory and DPAPI contracts remain in effect.
- Invitations/cards use bounded native file access; invitation payloads and project keys never enter React. Export creates a new file rather than overwriting one. CSP and navigation rules restrict the window to bundled content, with no generic JavaScript filesystem/shell/HTTP capability.
- Fresh verified equality lasts at most 30 seconds and is invalidated by compared head/known-metadata changes or disconnection. Known divergence persists while those comparison inputs remain unchanged; it does not vanish when the equality freshness interval expires. Restart requires a new check.

Existing protocol names/version, signing bytes, envelope AAD, crypto/scanner/network/transfer implementations, SQLite schema v2 and persisted state labels, project locks, snapshot/apply transactions, and `BACKUP → INSTALL → COMMIT → DONE` ordering remain unchanged. The Cargo lock adds desktop dependencies while retaining every pre-existing locked package/version. No tests, benchmarks, watchers, permanent daemon, installer/updater, cloud project files, VST/IPC, or automatic conflict merge were added.

Final checks completed on 2026-10-05 with Rust/Cargo 1.98.1, Node 24.19.0, installed MSVC 14.44.35207, Windows SDK 10.0.26100.0 and WebView2 154.0.4258.53:

| Check | Result |
| --- | --- |
| `npm ci --offline`, `npm run typecheck`, `npm run build` | Passed using the locked dependencies and local npm cache; bundled static frontend produced. |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked --offline` | Passed |
| `cargo clippy --workspace --lib --bins --locked --offline -- -D warnings` | Passed |
| `cargo build -p trackmesh-core -p trackmesh-mailbox-service -p trackmesh-relay-service -p trackmesh-desktop --locked --offline` | Passed; all four Windows executables linked. |
| `--help` for all four executables | Passed |
| Staged Git whitespace check with Windows CR-at-EOL handling; local Markdown links | Passed; 30 relative links resolved. |

The desktop declares Rust 1.90; neither that minimum nor the other crates' declared 1.85 minimum was validated. Dependency fetching required network access during setup; final builds/checks used cached locked dependencies. The developer guide describes the required C++ developer environment and supported Node runtime.

Manual operation used the actual native desktop/WebView2, native file dialogs and separately identified CLI/mailbox processes, with disposable device/project/service data only under `.build/phase4-manual-414e2123`. These were individually performed manual operations, not an automated test suite. Normal-user execution was needed for DPAPI. Fixtures used dummy `.als`/`.flp` files, empty data, random sample data and small files; no real DAW host was exercised. All fixture processes were stopped after validation, while databases, chunks, projects and recovery backups were retained.

| Phase 4 manual operation | Observed result |
| --- | --- |
| Native invitation import, device-card export/import, mutual pairing | Linked the local folder and paired the devices; private invitation stayed in the native backend. |
| Metadata refresh with source stopped | Remote announcement visible; local head/last verification remained null and file receive unavailable. |
| Desktop Check and Receive over loopback QUIC | `DIRECT`, completed the 32 MiB sample fixture; all four received file hashes independently matched. Signed equality check returned `EQUAL` and `verified_equal=true`. |
| CLI attempted a second peer runtime using desktop's data directory | Refused by the exclusive Windows runtime lease. |
| Generic WebView filesystem request for a dummy file | Refused; no filesystem plugin/capability available. |
| Receive after unsnapshotted local edit | Refused; local edit and committed head preserved. |
| Mailbox unavailable during Send Snapshot | Local snapshot committed, metadata remained `PENDING`, publication failure displayed. A simultaneous second send was rejected. |
| Mailbox restored, Retry publication | Matching ACK changed metadata to `PUBLISHED`, pending count zero. |
| Independent children of the same parent | Check reported `DIVERGED`; backend receive refused overwrite. Known divergence remained visible with `fresh=false` beyond 30 seconds. |
| Safe close during a 256 MiB receive, then restart and Resume receive | 8 MiB completed progress remained `PAUSED`; same transfer UUID resumed to `COMPLETE`. Independent SHA-256 of the entire file matched. |
| Fixture desktop stopped during journal `INSTALL`, source stopped, desktop restarted | Recovery marker remained set, local head remained at the base. **Recover locally** completed without the source; marker cleared, target head committed, all 1,601 file hashes independently matched. |
| Final binary equality check, then source stopped | Fresh signed equality was visible as `UP TO DATE`; later offline status retained the last verification with `fresh=false`. |
| Final safe close | Desktop exited and its owned fixture services stopped; durable data/backups retained. |

Native appearance was inspected after the final equality/divergence checks. Installer/signing, real DAW compatibility, real HTTPS deployment, two-PC NAT/firewall, forced circuit transfer and DCUtR remain unverified. The Phase 2 TCP/corruption/unpaired-device checks and Phase 3 service adversarial checks were not rerun as a full regression matrix for this phase. Native invitation export, revocation and pagination were implemented and statically inspected; they were not separately exercised as manual acceptance operations.

## Phase 5 implementation and validation

Implemented on the separate local `codex/phase-5-vst3-ipc` branch from Phase 4 integration commit `de930a6`. The C++ adapter uses official Steinberg SDK tag `v3.8.0_build_66`, root commit `9fad9770f2ae8542ab1a548a68c1ad1ac690abe0`, and its pinned submodules in ignored `vendor/vst3sdk`. Operation/build instructions are in [PHASE5.md](PHASE5.md).

- `plugins/vst3` provides a non-distributable SDK `SingleComponentEffect`, permanent class ID, versioned 24-byte project-UUID component state and native HWND control/status view. The processor passes mono/stereo 32/64-bit audio without audio-thread locks, allocations, pipe calls, disk or network work. Invalid/truncated/nil UUID state is rejected before replacing the current project. These audio/state paths were compiled and inspected; they were not exercised by a real DAW.
- A dedicated worker sends the existing plugin events through small length-prefixed JSON. Signals carry their UUID generation, so a queued action cannot target a newly restored project. State serialization only emits a save observation; no claim is made that DAW writes have settled. Termination signals the worker and returns without joining pipe I/O. A retained DLL reference protects worker code/state through bounded I/O and cleanup.
- Without an existing pipe, the plugin starts the bundled desktop, giving users access to UUID mapping and Receive controls while sharing its Rust Engine and single peer runtime. After a successful connection, manual desktop close does not cause repeated auto-reopening. Explicit CLI `plugin-serve` supports headless operation with a 60-second idle exit. Full session-aware/pending-save/publication lifetime remains Phase 7.
- Core owns the current-user-only pipe DACL, remote-client rejection, first-instance ownership, 64 KiB frames, non-nil session/project bindings and explicit rebind. Clients, snapshot jobs, remembered project messages, polling and read/write waits are bounded. Status reads and snapshot disk work run outside the pipe listener/network actor. A manual send calls the existing Engine workflow, preserving commit-before-publication and durable `PENDING` behavior.
- Desktop hosts PluginRuntime using its existing Engine/network and stops accepting plugin work immediately on close, including initialization races. Started snapshot/publication workers are awaited. Receive from a loaded plugin only directs to the DAW-closed desktop flow; it does not install into an open DAW project. Compact status reports last-known history and the displayed remote author's route, without claiming verified equality.
- Snapshot notices render the current pending-publication count from storage on each status read, so a later mailbox ACK clears the earlier `PENDING` message without another Send operation.

Existing PluginEvent/PluginStatus serialization, protocol names/version, signed bytes/AAD, crypto/scanner/transfer algorithms, SQLite schema v2 and persisted labels, project locks, snapshot/apply transactions and recovery ordering remain unchanged. Cargo adds only a core dependency edge/features to already-locked `windows-sys 0.61.2`; existing package versions are unchanged. SDK sources are not committed. No SDK validator, automated tests/benchmarks, watcher, installer/updater, permanent service or automatic conflict merge was added/run.

Checks performed on 2026-10-05 with the existing Rust 1.98.1, MSVC 14.44.35207, Windows SDK 10.0.26100.0, supported Node 24.19.0 and installed CMake/Ninja environment:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked --offline` | Passed |
| `cargo clippy --workspace --lib --bins --locked --offline -- -D warnings` | Passed after final Rust changes |
| `cargo build -p trackmesh-core -p trackmesh-mailbox-service -p trackmesh-relay-service -p trackmesh-desktop --locked --offline` | Passed after rebuilding static frontend |
| Frontend `npm ci --offline`, typecheck and build | Passed using supported Node and repository `.build/npm-cache`; the first default-cache offline attempt failed before this successful run |
| CMake/Ninja `TrackMesh` and optional SDK `editorhost` targets | Passed; plugin sources built with `/W4 /WX`; SDK-generated moduleinfo and bundled Rust executables produced |
| CLI `plugin-serve --help` and desktop `--help` | Passed; IPC suffix and explicit bridge command present |
| Packaged Rust owner hashes | Both bundled executables match the final Rust build |
| Documentation links and staged whitespace | 49 relative links resolved; Git whitespace check passed with Windows CRLF handling |

Visual Studio generator attempts encountered inherited duplicate `Path`/`PATH` entries in MSBuild. Single-config Ninja succeeded after preserving the SDK bundle layout in the plugin CMake file. No global toolchain/environment setting or compiler installation was changed. SDK dependency builds emitted their own `/EHs`/`/EHa` override warnings; the plugin's strict compilation succeeded. Declared minimum Rust versions were not validated.

Individual manual operations used the official SDK editor host, actual native plugin controls, actual Tauri/WebView2 desktop and only disposable data under ignored `.build/phase5-manual-a31714d5`. Normal-user execution was needed for current-user DPAPI/native UI. Dummy `.als`/`.flp` content was used; no real DAW or production project/database was operated. An initial headless-owner run preceded the final default-desktop-owner run.

| Manual operation | Observed result |
| --- | --- |
| Initial SDK host load and headless owner startup | Native UUID visible; unlinked controls disabled; the bundled agent started and answered compact IPC status. |
| Native Send after fixture UUID mapping | Local snapshot committed; no mailbox configuration produced durable `PENDING`, visible through plugin status. |
| Different Windows user opened the owner's pipe | Access denied. |
| Initial SDK host normal close | Headless bridge exited after the idle period; fixture data retained. |
| Final bundle loaded in SDK host | Plugin automatically started its bundled desktop; desktop reported ready against the same isolated profile. |
| Desktop mapped final plugin UUID | Mapping to a separate fixture folder succeeded and native controls enabled. Mapping an already-bound fixture folder to another UUID was refused by the unchanged storage uniqueness guard. |
| Final native Send Snapshot | Plugin reported committed snapshot `e3c54867-009e-47ae-89e3-f6f247cda187` with `PENDING`; desktop reported the same head and pending count one in its shared Engine/history. |
| Final native Receive | Persistent instruction to close the DAW project and use desktop Receive; no installation or head change. |
| IPC `PROJECT_ID` rebind | Existing session returned the newly selected fixture project's history. This does not validate real host state restoration. |
| Send with a different project UUID without rebind | Connection closed; fixture heads unchanged. |
| Oversized 65,537-byte frame header | Server closed that pipe connection before reading a payload; the existing snapshot remained current. |
| Simulated IPC save observation | Returned explicit manual-send guidance. This is not evidence of a real DAW save hook or automatic publication. |
| Native view capture and owned desktop/SDK-host close | Appearance inspected; desktop used Close safely and SDK host used normal window close; project/database/chunk data retained. |
| Final Rust owner and loopback mailbox ACK | Before the service started, IPC showed one pending announcement. After the desktop's existing retry published to the allow-listed fixture service, CLI publish reported zero pending; the same IPC owner and desktop both reported zero pending, and the earlier snapshot notice changed to no pending authored metadata. Desktop was safely closed and the mailbox stopped with Ctrl+C; fixture data retained. |

Real Ableton/FL Studio compatibility, UUID persistence through actual DAW save/reopen, 32/64-bit audio operation, host-driven save/session signals, multi-instance host behavior and shutdown under active plugin snapshot work remain unverified. The last C++ adjustment tightened I/O deadlines/audio-buffer validation and was compiled; earlier native operations do not establish audio behavior. Full settled save-to-publication, DAW-ending lifetime and VST receive/history UX remain Phases 6–8. No full Phase 2–4 runtime regression matrix, HTTPS deployment or two-PC NAT/relay/DCUtR validation was performed for this phase.

## Phase 6 implementation and validation

Implemented on local `codex/phase-6-save-publication` from Phase 5 integration commit `c2be909`. Core owns save decisions; the VST3 event/protocol and audio callback are unchanged.

- `crates/core/src/ipc/jobs.rs` owns at most eight project jobs, one per project. Save observations coalesce by revision after two seconds without another observation, with a 60-second debounce limit. Observations during scan/publication request a settled follow-up pass; job completion and scheduling share a lock to avoid losing an observation during job removal. Accepted work survives IPC disconnect, `SESSION_ENDING` and project rebind. Safe owner close rejects new work and awaits accepted debounce/scan/commit/publication.
- `crates/snapshot/src/settlement.rs` samples the existing bounded path/size/mtime inventory every 500 ms. Two seconds of quiet plus Windows handles that refuse concurrent writers precede hashing. A 60-second settlement deadline bounds waiting; the existing guarded scanner may attempt up to three scans when files change. Stable large-project hashing may take longer than the wait limit. Inventory/scan failures cannot commit a partial manifest. There is no watcher or continuous scan.
- Existing snapshot ownership moved from core `lib.rs` to `snapshots.rs`. Manual and settled snapshots share the same project lock, recovery guard, unchanged-content reuse, signing and expected-head SQLite transaction. `sending.rs` exposes settled send through the same commit-before-publication flow. The existing 30-second owner-scoped `MailboxRuntime` retries persisted authored envelopes; only a matching ACK changes `PENDING` to `PUBLISHED`. IPC notices use current storage pending counts.

Protocol/signing/AAD, SQLite schema and values, transfer/recovery ordering, scanner/chunk/hash algorithms, manual CLI behavior and single-swarm ownership remain unchanged. No dependency, automated test/benchmark suite, filesystem watcher, permanent service or automatic conflict merge was added. Frontend and C++ sources are unchanged; their owner binaries were repackaged.

Checks performed on 2026-10-05 using the same Windows toolchain as Phase 5:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked --offline` | Passed |
| `cargo clippy --workspace --lib --bins --locked --offline -- -D warnings` | Passed after final Rust changes |
| `cargo build -p trackmesh-core -p trackmesh-desktop -p trackmesh-mailbox-service -p trackmesh-relay-service --locked --offline` | Passed; all four Windows executables linked against the current code |
| CMake/Ninja `TrackMesh` target | Passed; strict C++ compilation and Rust-owner repackaging succeeded |
| Packaged owner SHA-256 comparison | Bundled CLI and desktop match the final Rust build |
| Documentation links, preserved boundaries and staged whitespace | 50 relative links resolved; protocol/crypto/storage/network/transfer and UI/C++ source boundaries unchanged; Git whitespace check passed with Windows CRLF handling |

Individual manual operations used real Named Pipe framing, the actual Tauri/WebView2 owner, normal-user DPAPI and an allow-listed loopback mailbox. Data is confined to ignored `.build/phase6-manual-2f1ed37b`; `.flp` content is a dummy file, not a DAW project. These were separate manual operations, not an automated suite.

| Manual operation | Observed result |
| --- | --- |
| Two save observations followed by IPC session ending while one dummy writer held an exclusive file handle | Both replies showed settlement waiting. Desktop/IPC had no head and no pending announcement while the writer was active. After its final write/release, one 31-byte snapshot `1565e3ee-565d-4f67-a113-e468d74c63ec` committed; repeated observations did not create duplicate history. |
| Mailbox unavailable during that automatic save | Local snapshot remained current with durable `PENDING`, visible in IPC and history. |
| Allow-listed loopback service started without a new Send/Publish command | Existing owner retry changed history to `PUBLISHED`, pending count to zero, and the IPC notice to no pending authored metadata. |
| Another save observation with unchanged file contents | History count remained one; current snapshot was reused. |
| Changed dummy file, accepted save observation, then immediate desktop Close safely | Desktop exited only after a 59-byte child snapshot `d4b58d28-8277-4cfa-b0a6-9ae65612af45` committed and received ACK. History retained exactly two snapshots with the expected parent relationship. |
| Fixture shutdown | Owned desktop exited safely; mailbox stopped with Ctrl+C. Project/database/chunks retained. |

Real DAW serialization timing, delayed host writes beyond the quiet period, UUID save/reopen, audio, and delivery of plugin signals still queued during termination remain unverified. Quiet metadata/writer checks cannot prove the host has finished all future writes. Debounce/settlement timeout boundaries and active-hash follow-up races were compiled/inspected, not manually exercised. Full owner retention for DAW sessions/pending publication remains Phase 7; complete remote/offline/VST Receive UX remains Phase 8. No full transfer/recovery regression matrix, real HTTPS or two-PC NAT validation was performed for this phase.

## Phase 7 implementation and validation

Implemented on local `codex/phase-7-session-lifetime` from Phase 6 integration commit `0c28d07`.

- Core `lifetime.rs` owns a shared 60-second idle grace. Bound plugin sessions, accepted plugin work, adapter jobs and authored-local pending metadata retain the owner. Plugin and peer responder activity epochs reset grace for short requests between polls; desktop job/window epochs do the same. A narrow authored-local SQLite EXISTS query runs in blocking workers. Idle IPC shutdown claims the same gate as session/job acceptance, so a newly accepted save cannot enter after the final idle decision.
- Desktop window close now hides the window and releases its explicit window session while its same Engine, metadata worker and peer runtime continue. Downloads continue while their source remains available; local installation/recovery keep their existing locks and order. Explicit desktop launch restores the same window/session. Plugin launch uses `--plugin-owner`; repeat plugin launches do not manufacture a persistent window session. After all sessions/work/publication finish, grace precedes local-worker, metadata and peer drain. Explicit headless Ctrl+C still drains accepted local work and preserves committed pending metadata.
- The pinned C++ worker gives queued current-generation save/send signals priority over session ending. Failed transport restores the coalesced signal without replacing a newer generation; reconnect/ending retry has a 30-second budget checked between bounded requests. Host termination remains a prompt signal with no I/O join. The budget does not bound OS scheduling/cancellation drain, and forced host-process exit can kill the worker before handoff; agent-received events are the durable-work boundary.

Pending publication can retain an owner indefinitely when the mailbox is unavailable or unconfigured. This follows the accepted pending-work lifetime; no permanent service/autostart is installed. Reopen desktop to inspect/configure pending work; explicit interruption leaves it durable for restart. Paused/failed transfers and pending journal recovery are retained on disk rather than treated as actively running work. Incoming peer RPCs count as active responses and reset grace; a stalled remote receiver with no requests for the full grace interval does not reserve a permanent source session.

Protocol/version/signing/AAD, schema v2 and persisted labels, project locks, snapshot/apply transactions, transfer/recovery ordering, hashes/chunk algorithms and the one-swarm lease remain unchanged. No dependency, automated test/benchmark suite, watcher or conflict merge was added. A new storage read does not mutate publication state; only the existing matching ACK clears PENDING.

Individual Windows manual operations used current-user DPAPI, real Named Pipe framing, actual native Tauri/WebView2 and an allow-listed loopback mailbox, with dummy `.flp` data under ignored `.build/phase7-manual-29e46684`. The separate headless identity lives in that fixture's `headless-data`. These were manually initiated operations, not an automated suite.

| Manual operation | Observed result |
| --- | --- |
| Save observation followed by session ending, then desktop Close safely with mailbox stopped | Hidden owner completed a 39-byte snapshot `2848d127-adae-4b03-b062-91ee2f051204`, retained PENDING, and stayed available until a later service start. Its existing retry obtained ACK without another Send/Publish. |
| Explicit desktop relaunch while its owner was hidden | Same owned PID/window/profile returned ready, with the same head and zero pending metadata; closing released the window session again. |
| Hidden owner with zero pending publication and a 70-second polling IPC session | Every poll succeeded; session ending then allowed clean automatic process exit after grace. |
| Rebuilt desktop, changed dummy file, accepted save then immediate window close | A 53-byte child `7b854966-c5d4-47cc-a0ad-b48d3d82b7c7` committed after close. At 67 seconds after close the same PID/pipe still reported PENDING, exceeding idle grace. |
| Mailbox restarted after that pending-retention check | Existing retry ACK published the child; history retained two snapshots with the expected parent and sizes. Desktop exited automatically after work/grace; no Close/Publish command was needed. |
| Separate headless owner with an unlinked UUID and a 70-second bound polling session | Session remained connected despite unavailable project mapping; ending released it for idle exit. Owner command completed with exit code zero. |
| Fixture shutdown | Both owners exited automatically; mailbox stopped with Ctrl+C. All project/database/chunk/identity data retained. |

Checks performed on 2026-10-05 with Rust 1.98.1, supported Node 24.19, MSVC 14.44.35207 and Windows SDK 10.0.26100.0:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked --offline` | Passed after final Rust adjustment |
| `cargo clippy --workspace --lib --bins --locked --offline -- -D warnings` | Passed after final Rust adjustment |
| Frontend `npm run typecheck`, `npm run build` | Passed; current locked installed dependencies and bundled static assets used |
| `cargo build -p trackmesh-core -p trackmesh-desktop -p trackmesh-mailbox-service -p trackmesh-relay-service --locked --offline` | Passed; all four Windows executables linked |
| CMake/Ninja `TrackMesh` target | Passed with strict C++ compilation; final Rust owners repackaged |
| CLI `plugin-serve --help` and desktop `--help` | Passed; plugin-owner and existing IPC/network options present |
| Bundle owner hashes, local documentation links and staged whitespace | Passed; both owners match final build, 50 relative links resolved, Windows CR-at-EOL whitespace check passed; protocol/crypto/network/transfer algorithms and schema files unchanged |

Real DAW/audio/UUID restore/save/termination and native queued-event delivery remain unverified. The final narrow adjustments preserve idle handling for an already-finished failed plugin task and recheck queued signals after observing termination; both were compiled/inspected. Manual flows did not exercise task panics or the native termination race. Active receive/installation and long blocked responder lifetime were inspected, not rerun as a Phase 7 transfer/recovery matrix. Ending reconnect-budget boundaries, forced DAW process exit, real HTTPS and two-PC NAT/relay/DCUtR were not exercised.

## Phase 8 implementation and validation

Implemented on local `codex/phase-8-remote-workflow` from the Phase 7 integration commit `1824ac4`.

- Core adapter summaries now expose at most eight newest authored snapshots per paired device and a conservative last-known lineage. Storage resolves at most 64 ancestry steps with one bounded recursive query; incomplete, missing or deeper ancestry reports `UNKNOWN` rather than manufacturing equality or divergence. Complete CLI history and online signed-chain validation are unchanged.
- Desktop cards show each selected collaborator's bounded recent history, source availability and last-known lineage. Persisted verification time is attributed only when its stored PeerId and snapshot still match the selected peer and current local head. Only a fresh connected-session equality ACK renders `UP TO DATE`. Known divergence disables Receive; the existing core receiver independently preserves ancestry, clean-folder, lock, journal and recovery enforcement.
- The VST bridge consumes the existing `peer_online`, remote snapshot and state fields without changing `PluginStatus` serialization. Its Receive control requires an online known source and is disabled for known divergence. Named Pipe Receive requests explicitly refuse missing remote history, offline sources and divergence; accepted requests remain a DAW-closed desktop handoff and never install inside the loaded plugin.
- Existing signed bytes/AAD, protocol names and fields, schema v2, persisted labels, project locks, snapshot/apply transactions, recovery ordering, one-swarm ownership, save settlement and session lifetime are unchanged. No dependency, automated test/benchmark, watcher, cloud file storage or automatic merge was added.

Checks performed on 2026-10-05 with Rust 1.98.1, supported Node 24.19.0, MSVC 14.44.35207, Windows SDK 10.0.26100.0 and the existing pinned SDK/Ninja build:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked --offline` | Passed |
| `cargo clippy --workspace --lib --bins --locked --offline -- -D warnings` | Passed |
| Frontend TypeScript check and Vite build | Passed by invoking both entry points directly with supported Node 24.19.0; an earlier npm-script build completed under unsupported Node 22.11 with Vite's version warning and was not used as final evidence |
| `cargo build -p trackmesh-core -p trackmesh-desktop -p trackmesh-mailbox-service -p trackmesh-relay-service --locked --offline` | Passed; all four Windows executables linked |
| CMake/Ninja `TrackMesh` target | Passed with strict C++ compilation and current Rust owner repackaging after initializing the documented process-local MSVC/SDK environment; an initial shell without that environment could not find standard C++ headers |
| Packaged owner SHA-256 comparison | Bundled agent and desktop executables both match the final Rust build |
| Real Named Pipe status/Receive handoff | Passed against a copied disposable two-device profile: known sibling children rendered `DIVERGED` and refused Receive; equal last-known heads remained `LAST_KNOWN`, never `UP TO DATE`, and an offline source refused Receive |
| Actual Tauri/WebView2 desktop | Passed against the same disposable copy: two recent remote snapshots appeared per card, the divergent card showed its last-known warning, and Receive remained disabled; visual inspection found no clipping at the captured window size |

The disposable profile was copied under ignored `.build/phase8-manual-remote-workflow`; original historical fixture data and project folders were not changed. The source peer and mailbox were intentionally offline, so Phase 8 did not rerun a live receive, mailbox refresh, direct/relayed connection, recovery or fresh equality ACK. The current VST3 target was compiled, but its native editor view was not reopened for this phase. Real Ableton/FL Studio UUID restore, audio, save/session events, forced host termination, HTTPS deployment and two-PC NAT/relay/DCUtR remain unverified.

### Desktop connection setup follow-up (2026-10-07)

Implemented on local `codex/phase-8-connection-setup`. Collaboration details now show a Connection setup panel with live network listen addresses, local IPv4/port destinations, the current listen port (default 43101), public IPv4 and external router port entry, TCP/QUIC multiaddr previews, Windows Firewall/port-forward guidance and CGNAT/private-address warnings. Offline Check peer help states the existing 30-second connection window; the running countdown remains intact. Mailbox help describes encrypted metadata only, and relay help describes the fallback peer path with an online source.

The desktop export action accepts optional public IPv4/external port fields. Core's `public_ipv4_addresses` validates and constructs the two endpoints; native export prepends them before the existing bounded signed device-card address list, keeping both explicit public endpoints within the 16-address limit. Existing export without these fields still uses current network/startup external addresses. Invitation payloads/project keys stay in Rust and native file dialogs; only public connection information enters React. No signed serialization, protocol/schema/state labels, journal/recovery order, CLI flags or single-actor ownership changed. No automated tests, benchmarks, watchers or dependencies were added.

Public entry is per export and is not persisted or applied to the running swarm. The external router port may differ from the fixed local listen port; guidance maps both TCP and UDP to that local port. Changing the local listener still requires a new startup configuration after the existing owner exits. The UI does not detect public IP, configure router/firewall rules, verify reachability or diagnose CGNAT automatically.

Checks: supported Node 24.19 `npm ci`, `npm run typecheck` and `npm run build` passed; `cargo fmt --all -- --check`, `cargo check -p trackmesh-core -p trackmesh-desktop --locked --offline` and `cargo clippy -p trackmesh-core -p trackmesh-desktop --lib --bins --locked --offline -- -D warnings` passed. Initial sandbox/offline npm installation lacked the Vite cache; installation succeeded with the locked dependencies through normal registry access. `cargo build -p trackmesh-core -p trackmesh-desktop --locked --offline` and `target/debug/trackmesh-desktop.exe --help` also passed; the final desktop build includes the final static frontend. No live WebView/native invitation export or two-PC NAT/firewall/relay operation was performed for this follow-up. Next concrete validation: configure two real Windows PCs, export through the public-address button, privately exchange invitations/cards and run Check peer/direct Receive over separate networks.

### Manual two-PC trial documentation (2026-10-07)

Added `MANUAL_TRIAL.md` (Turkish) and entry links in README, the documentation index and the desktop operation guide. The guide separates current executable/source-build operation from future installer work, covers LAN and separate-Internet trials, Windows Firewall TCP/UDP rules, router mapping, CGNAT/double-NAT/disabled forwarding, public-address invitation export, mutual pairing and direct snapshot/Receive. It explicitly records per-export public fields, the device-card export limitation, optional metadata-only mailbox/private relay, 30-second Check peer, pending-publication retention and safe recovery.

Documentation was checked against current desktop/core code and official Microsoft firewall/RFC 6598 references; local Markdown links and Git whitespace were checked. No application code, secrets, protocols, schema/state labels or recovery order changed. No builds, automated tests, benchmarks or live two-PC operations were run for this documentation change. Installer packaging and real Windows/NAT/firewall/relay trial results remain unverified; next action is to follow the guide with two collaborators and record actual outcomes here.

### Trial quick checklist follow-up (2026-10-07)

Added a short seven-step A/B checklist at the beginning of `MANUAL_TRIAL.md` and an ordered symptom/action table for missing firewall permission, managed rules, disabled/locked modem forwarding, external-port conflicts, timeout, CGNAT, double NAT, changed addresses and Receive prerequisites. Public/external and local ports remain distinct; entered fields do not configure router/firewall rules. The guide treats timeout as an observation, not a diagnosis, and requires prepared infrastructure for a relay fallback.

Checks: documentation reviewed against the existing guide and the manufacturer's port-forward troubleshooting reference; local Markdown links and `git diff --check` passed. Documentation only: no application code/contracts changed, and no builds, automated tests, benchmarks or manual network operations ran. Next action remains an actual two-PC trial, recording the failing step and observed message here.

### Peer trial executable package (2026-10-07)

Prepared a private manual-trial ZIP under ignored `.build/peer-packages/TrackMesh-PeerTrial-20261007-162413.zip` (14,539,736 bytes). It contains only release x64 `trackmesh-desktop.exe`, `trackmesh-agent.exe`, app-local `vcruntime140.dll` from Visual Studio's redistributable directory, `CLAUDE.md`, public build metadata and SHA-256 sums. Application code is from `32941db`; no application source changed for packaging. The ZIP does not include invitations, project files, data profiles, databases or secrets, and is not an installer or a public release upload.

The source template `PEER_TRIAL_CLAUDE.md` is linked from the documentation index and manual trial guide. It instructs a collaborator's Claude Code to import a private invitation by path without reading its payload, export a device card, wait for mutual pairing/source snapshot, obtain UUIDs from safe local status, then check/receive/check. A separate persistent trial data profile avoids existing profiles; CLI network jobs and Desktop must not compete for its owner lease. The source remains online in Desktop. Mailbox/relay and Windows canonical path limitations are described; no automatic firewall/router/UAC/service setup was added.

Checks performed for this package: supported Node 24.19 `npm run typecheck` and `npm run build` passed; `cargo build -p trackmesh-core -p trackmesh-desktop --release --locked --offline` passed with Rust 1.98.1. Release CLI top-level/join/card/check/receive help and Desktop help passed. `dumpbin /DEPENDENTS` inspected both executable imports and the app-local CRT; packaged executables passed help checks as well. Copied executables match build hashes; ZIP entry allowlist and every archived file hash match the source package. Local Markdown links and Git whitespace checks passed. ZIP SHA-256: `149b61f085e9b94551f1f359515bab8fe46959877bb81b4b227eca4ab5b532dc`.

No automated tests/benchmarks, clean second-PC launch, real invitation import, DAW operation or Internet transfer ran for this package. Desktop still requires WebView2 on the recipient PC; CLI does not. Next action: privately send the ZIP plus a separately exported public-address invitation, import the collaborator's returned device card in the sender's Desktop, create a snapshot, and record actual Check/Receive outcomes on two PCs here.

### Guided sender setup and two-PC checklist (2026-10-07)

During the user-guided trial, the sender's Desktop Connection setup showed local listener 43101, two interface addresses and a manually entered router WAN IPv4 outside the private/shared ranges. A read-only `Get-NetConnectionProfile`/`Get-NetIPAddress` inspection confirmed that the router-subnet PC address belonged to the active Wi-Fi interface with Windows category Public. The user reported saving TP-Link TCP/UDP port-forward rules and completing Windows Firewall rules after guidance to use normal Allow and the active profile; router/firewall rule contents were not independently read back. No full public IPv4, credentials or invitation payload are recorded here.

Added reusable `TWO_PC_CHECKLIST.md` with sender/receiver responsibilities, the observed TP-Link form fields, Windows rule/profile choices, private ZIP plus separate invitation handoff, Claude/card steps, and the continuation when the collaborator's card arrives. Links were added to the index and manual trial guide. This is an operational checklist; this implementation record remains the authoritative validation history.

The user is waiting on the collaborator. Invitation delivery/import, returned card, mutual pairing, fresh check, direct/relayed receive and DAW reopening have not been confirmed. Next action: receive the collaborator's card, import it into the existing sender project, create/confirm the current snapshot, keep the source online, and run the collaborator's check/receive/check sequence.

Documentation-only follow-up: local Markdown links and Git whitespace checks passed; no application code/contracts changed and no builds, automated tests, benchmarks or network-transfer operations ran for this edit. The earlier executable ZIP remains unchanged.

## Remaining integration work

Deploy the Phase 3 services with real DNS, valid HTTPS certificates, private ACLs, pinned infrastructure PeerId, and reachable advertised addresses; then validate on two real Windows PCs. Real firewall behavior, Internet NAT traversal, forced circuit relay, and DCUtR remain unverified. Direct transfers still do not imply a mailbox acknowledgement.

Snapshot chunks are retained locally without automatic pruning. This keeps earlier snapshots retrievable when the working folder changes. The mailbox receives small encrypted announcements only; file manifests and chunks travel through authenticated peer connections. Validate the native VST3 in actual Windows DAWs before claiming host/audio/state/save compatibility. Phase 8 completes the local MVP implementation; remaining work is real integration/deployment validation rather than another numbered implementation phase.
