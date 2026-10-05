# TrackMesh

Private Windows transfer of Ableton Live and FL Studio project folders between two or three explicitly paired devices. Project files and samples stay on collaborator PCs. Immutable snapshots use BLAKE3, fixed 4 MiB chunks, signed manifests, and transactional SQLite history.

**Current implementation: Phases 1–6, runnable through the Rust CLI, Windows Tauri 2/React desktop, native C++ VST3/Named Pipe surface, and separate infrastructure services.** Manual snapshot/publication, receive/resume, pairing, status and event-triggered settled save/publication are implemented locally. The plugin was operated in the official SDK editor host; simulated save events and loopback publication were validated. Actual DAW save/reopen/audio/hooks, real NAT/HTTPS deployment and the complete session workflow remain unverified or unfinished.

Build on Windows with Rust and the Visual Studio C++ build tools:

```powershell
cargo build -p trackmesh-core -p trackmesh-mailbox-service -p trackmesh-relay-service --locked
target\debug\trackmesh-agent.exe --help
```

See [Phase 5](docs/PHASE5.md) for official SDK VST3 build, UUID state and plugin/agent IPC; [Phase 4](docs/PHASE4.md) for desktop operation; [Phase 2](docs/PHASE2.md) for CLI pairing/transfer; and [Phase 3](docs/PHASE3.md) for encrypted publication/refresh, HTTPS and private infrastructure. [Implementation progress](docs/IMPLEMENTATION.md) records phase status, validation evidence and integration gaps.

## For contributors and agents

Start with [AGENTS.md](AGENTS.md), then use the [code map](docs/CODEMAP.md) to read the files that own your task. Crate-local instructions cover transfer, storage, network, and core invariants. The [documentation index](docs/INDEX.md) routes architecture, build guidance, accepted MVP scope, and operation instructions.

Development uses `D:\CodeSpace\track-mesh`. The refactored crate entry points preserve public exports, while responsibility modules hold implementations. Reusable status/check/receive/recovery workflows live in the Rust core.

The default local data directory is `%LOCALAPPDATA%\TrackMesh`. `--data-dir` or `TRACKMESH_DATA_DIR` selects another directory. Identity and project keys are protected with current-user Windows DPAPI. Keep the data directory separate from DAW project folders.

There are no automated test suites, benchmarks, project folder watchers, or continuous synchronization.
