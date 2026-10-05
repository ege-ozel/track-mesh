# TrackMesh

Private Windows transfer of Ableton Live and FL Studio project folders between two or three explicitly paired devices. Project files and samples stay on collaborator PCs. Immutable snapshots use BLAKE3, fixed 4 MiB chunks, signed manifests, and transactional SQLite history.

**Current implementation: Phases 1–4, runnable through the Rust CLI, Windows Tauri 2/React desktop, and separate infrastructure services.** Manual desktop snapshot/publication, receive/resume, pairing, and status are implemented locally. Real NAT/HTTPS deployment validation, VST3 integration, and the complete save/session workflow remain unfinished.

Build on Windows with Rust and the Visual Studio C++ build tools:

```powershell
cargo build -p trackmesh-core -p trackmesh-mailbox-service -p trackmesh-relay-service --locked
target\debug\trackmesh-agent.exe --help
```

See [Phase 4](docs/PHASE4.md) for desktop build and operation, [Phase 2](docs/PHASE2.md) for CLI pairing/transfer, and [Phase 3](docs/PHASE3.md) for encrypted publication/refresh, HTTPS, and private infrastructure. [Implementation progress](docs/IMPLEMENTATION.md) records completed phases, validation evidence, and integration gaps.

## For contributors and agents

Start with [AGENTS.md](AGENTS.md), then use the [code map](docs/CODEMAP.md) to read the files that own your task. Crate-local instructions cover transfer, storage, network, and core invariants. The [documentation index](docs/INDEX.md) routes architecture, build guidance, accepted MVP scope, and operation instructions.

Development uses `D:\CodeSpace\track-mesh`. The refactored crate entry points preserve public exports, while responsibility modules hold implementations. Reusable status/check/receive/recovery workflows live in the Rust core.

The default local data directory is `%LOCALAPPDATA%\TrackMesh`. `--data-dir` or `TRACKMESH_DATA_DIR` selects another directory. Identity and project keys are protected with current-user Windows DPAPI. Keep the data directory separate from DAW project folders.

There are no automated test suites, benchmarks, project folder watchers, or continuous synchronization.
