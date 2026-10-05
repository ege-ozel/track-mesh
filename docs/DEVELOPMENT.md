# Development workflow

## Windows prerequisites and location

Use `D:\CodeSpace\track-mesh`, Rust for Windows, and Visual Studio C++ build tools. SQLite is bundled. Run the build commands from an x64 Visual Studio developer shell so the MSVC compiler/linker and Windows SDK header/library paths are initialized. Private device/project material uses current-user Windows DPAPI; a sandbox account may be unable to decrypt another account's data. There is no plaintext-key fallback.

The last successful implementation build used Rust 1.98.1. The workspace currently declares `rust-version = "1.85"`; the pinned dependency/toolchain combination has not been verified on that minimum version. Use the known working toolchain unless the task is explicitly minimum-version compatibility work.

## Build and static checks

Run from repository root:

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked --offline
cargo clippy --workspace --lib --bins --locked --offline -- -D warnings
cargo build -p trackmesh-core -p trackmesh-mailbox-service -p trackmesh-relay-service --locked --offline
target\debug\trackmesh-agent.exe --help
target\debug\trackmesh-mailbox-server.exe --help
target\debug\trackmesh-relay-server.exe --help
```

`--offline` works when the locked dependencies are already cached. On a new machine, omit it for the initial dependency download while retaining `--locked`. For an optimized binary use `cargo build -p trackmesh-core --release --locked`; the release profile enables thin LTO and one codegen unit. No benchmark claim follows from those settings.

Do not add/run automated tests or benchmarks for this MVP. Formatting, compilation, static analysis, code inspection, and explicitly authorized manual operation provide the current checks. Report actual evidence and limitations. Historical loopback checks in `IMPLEMENTATION.md` do not establish real NAT traversal or validate a later refactor.

## Data and manual operation

- Default data: `%LOCALAPPDATA%\TrackMesh`; override with `--data-dir` or `TRACKMESH_DATA_DIR`.
- Local storage: `trackmesh.db`, `chunks`, protected `secrets`, and per-project lock files. Keep it separate from DAW project folders.
- Each simulated device needs a separate data directory and working folder. Fixtures/logs/build outputs belong in ignored scratch paths, never beside production samples.
- Invitations contain a shared project key. Exchange them privately; never print them into logs or commit them.
- Receive and recover require closing the DAW project. Preserve `.trackmesh-<project>-<transfer>` staging/backup data if recovery is pending.
- Logs go to stderr; CLI JSON goes to stdout. For transfer details use `$env:RUST_LOG='trackmesh_transfer=info,trackmesh_network=info'`.
- Follow [PHASE2.md](PHASE2.md) for pairing and commands. Do not install a permanent daemon or introduce file watchers while debugging.
- Follow [PHASE3.md](PHASE3.md) for mailbox ACLs, HTTPS, publication/refresh, and the separate private relay. Keep service and simulated-device data in separate directories.

## Local phase branches

Commit the current completed work before beginning a phase. For each new phase, create a local `codex/phase-N-description` branch from the integration branch, implement and validate it, then commit and merge it locally into `master` with `--no-ff`. Push the resulting `master` normally; no force push or remote branch/PR is required. This is the user's requested Git workflow, not automatic merging of divergent project snapshots.

## Before editing

1. Read root `AGENTS.md` and the owning crate's instructions.
2. Use `CODEMAP.md` to inspect the narrowest relevant path and its callers.
3. Identify affected compatibility contracts: signed bytes, wire frames, schema/state labels, paths, journal, API/CLI fields, or transaction ordering.
4. Preserve unrelated working changes. Update maps/docs with module/API moves and record the checks actually run.

When filesystem permissions prevent editing the canonical repository, request the correct workspace/writable root. Preparing a reviewable staging copy is possible, but do not claim the canonical repository was updated or bypass the permission boundary.
