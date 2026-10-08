# TrackMesh

Private Windows transfer of Ableton Live and FL Studio project folders between two or three explicitly paired devices. Project files and samples stay on collaborator PCs. Immutable snapshots use BLAKE3, fixed 4 MiB chunks, signed manifests, and transactional SQLite history.

**Current implementation: Phases 1–8, runnable through the Rust CLI, Windows Tauri 2/React desktop, native C++ VST3/Named Pipe surface, and separate infrastructure services.** Manual snapshot/publication, receive/resume, pairing, bounded remote history, offline availability, divergence UX, event-triggered settled save/publication and session/work-based owner retention are implemented locally. The plugin was operated in the official SDK editor host; simulated save events, loopback publication, and disposable remote-history/divergence surfaces were validated. Actual DAW save/reopen/audio/hooks and real NAT/HTTPS deployment remain unverified.

For a first trial with a collaborator, follow the [two-PC setup entry point (Türkçe)](docs/setup/README.md): launch the executable, configure LAN/public IP and TCP/UDP forwarding, privately send an invitation, approve the automatic device request, and receive a snapshot. Choose the [sender/host guide](docs/setup/host/README.md) or the [receiving peer guide](docs/setup/peer/README.md). There is no installer yet; the app guides connection setup but does not configure the router or Windows Firewall automatically. [Pairing security](docs/setup/PAIRING_SECURITY.md) describes the bounded enrollment channel and explicit approval; legacy signed card files remain supported.

Build on Windows with Rust and the Visual Studio C++ build tools:

```powershell
cargo build -p trackmesh-core -p trackmesh-mailbox-service -p trackmesh-relay-service --locked
target\debug\trackmesh-agent.exe --help
```

See [Phase 5](docs/PHASE5.md) for official SDK VST3 build, UUID state and plugin/agent IPC; [Phase 4](docs/PHASE4.md) for desktop operation; [Phase 2](docs/PHASE2.md) for CLI pairing/transfer; and [Phase 3](docs/PHASE3.md) for encrypted publication/refresh, HTTPS and private infrastructure. [Implementation progress](docs/IMPLEMENTATION.md) records phase status, validation evidence and integration gaps.

## For contributors and agents

Start with [AGENTS.md](AGENTS.md) ([Claude entry point](CLAUDE.md)), then use the [code map](docs/CODEMAP.md) to read the files that own your task. Crate-local instructions cover transfer, storage, network, and core invariants. The [documentation index](docs/INDEX.md) routes architecture, build guidance, accepted MVP scope, and operation instructions.

Development uses `D:\CodeSpace\track-mesh`. The refactored crate entry points preserve public exports, while responsibility modules hold implementations. Reusable status/check/receive/recovery workflows live in the Rust core.

The default local data directory is `%LOCALAPPDATA%\TrackMesh`. `--data-dir` or `TRACKMESH_DATA_DIR` selects another directory. Identity and project keys are protected with current-user Windows DPAPI. Keep the data directory separate from DAW project folders.

There are no automated test suites, benchmarks, project folder watchers, or continuous synchronization.
