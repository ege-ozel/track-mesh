# TrackMesh

Private Windows transfer of Ableton Live and FL Studio project folders between two or three explicitly paired devices. Project files and samples stay on collaborator PCs. Immutable snapshots use BLAKE3, fixed 4 MiB chunks, signed manifests, and transactional SQLite history.

**Foundation phases 1–8 are implemented locally through the Rust CLI, Windows Tauri 2/React desktop, native C++ VST3/Named Pipe surface, and separate infrastructure services.** The newer [Aşama 1 M0–M10 plan](docs/plans/phase1-direct-p2p.md) covers the direct-P2P workflow; #NN numbers identify review/UX tasks. Manual snapshots, receive/resume, pairing, remote history, offline availability, explicit divergence resolution, settled save/publication and session/work retention are implemented. Official SDK editor-host, simulated save/loopback and disposable UI/IPC checks are recorded. Supabase invitation, pairing and automatic first receive were checked with a host plus Windows Sandbox. Actual DAW save/reopen/audio/hooks, transfer across separate networks, self-hosted HTTPS deployment and newer Supabase changes remain unverified. [Implementation progress](docs/IMPLEMENTATION.md) records the scope of each check.

For a first trial, follow the [two-PC setup entry point (Türkçe)](docs/setup/README.md). The Desktop flow is **Share a project folder → Add project → Invite collaborator → open or Paste invite link → Accept → Approve**. The receiver's first version downloads automatically into an empty, never-received folder. Later versions use **Share current version** and **Get collaborator's version**, with the DAW closed before receiving (TrackMesh asks once before it changes files). The interface is in English or Turkish (Settings → Language). Choose the [sender/host guide](docs/setup/host/README.md) or [receiving peer guide](docs/setup/peer/README.md); these roles describe the first transfer direction. The first Invite or Accept asks for UPnP consent. Manual port forwarding is under Advanced → This PC; shared connection troubleshooting is in the setup entry point. The installer adds one inbound Windows Firewall allow rule for the TrackMesh app.

The optional invite service is your own Supabase project. It holds small encrypted invite drops, endpoint records and snapshot head hints; project files and samples transfer between online PCs. A v2 link expires after 15 minutes and can be read once. Approval automatically queues a membership grant. Revoke on the project creator queues a membership revoke; it does not rotate the project key, so the removed device keeps previously received files and keys. [Pairing security](docs/setup/PAIRING_SECURITY.md) explains explicit approval and capability handling.

Invitation files, manual device cards and CLI `join` / `request-pairing` remain Advanced or legacy paths. Opening Desktop does not notify an uninvited peer. Endpoint updates can help existing collaborators reconnect; an established connection carries traffic both ways. A public directory, browser deployment, offline pairing queue, permanent background service and automatic merge are outside the current product. Relay service code exists but is outside the supported setup and [deferred to post-1.0](docs/plans/post-1.0.md#relay).

No current trial package contains this complete link flow. Build a new private trial package from the reviewed `master` when needed. Keep private invitation contents and personal Supabase configuration out of distributed public builds.

Build on Windows in an x64 Visual Studio developer shell. The workspace declares Rust 1.88; Desktop declares 1.90; the recorded successful toolchain is Rust 1.98.1. Use Node 22.12 or newer for Vite 8. Recent checks on Node 22.11.0 completed with a version warning and do not establish support for that version. See [Development](docs/DEVELOPMENT.md) for prerequisites and release-build cleanup.

```powershell
cargo build -p trackmesh-core -p trackmesh-mailbox-service -p trackmesh-relay-service --locked
target\debug\trackmesh-agent.exe --help

Set-Location apps\desktop
npm ci
npm run typecheck
npm run build
npx tauri build --no-bundle
```

The service build includes the legacy relay code; building it does not enable a supported fallback. A public release must not embed a personal Supabase project. `TRACKMESH_DEFAULT_SUPABASE_*` build-time values are for private builds only.

Foundation [phase 5](docs/PHASE5.md) covers official SDK VST3 build, UUID state and plugin/agent IPC; [phase 4](docs/PHASE4.md) covers Desktop implementation; [phase 2](docs/PHASE2.md) covers CLI pairing/transfer; and [phase 3](docs/PHASE3.md) covers the separate self-hosted infrastructure. Use the setup entry point for the current Desktop workflow.

## For contributors and agents

Start with [AGENTS.md](AGENTS.md) ([Claude entry point](CLAUDE.md)), then use the [code map](docs/CODEMAP.md) to read the files that own your task. Crate-local instructions cover transfer, storage, network, and core invariants. The [documentation index](docs/INDEX.md) routes architecture, build guidance, accepted MVP scope, and operation instructions.

Development uses `D:\CodeSpace\track-mesh`. The refactored crate entry points preserve public exports, while responsibility modules hold implementations. Reusable status/check/receive/recovery workflows live in the Rust core.

The default local data directory is `%LOCALAPPDATA%\TrackMesh`. `--data-dir` or `TRACKMESH_DATA_DIR` selects another directory. Identity and project keys are protected with current-user Windows DPAPI. Keep the data directory separate from DAW project folders.

There are no automated test suites, benchmarks, project folder watchers, or continuous synchronization.
