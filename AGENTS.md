# TrackMesh agent entry point

## Read this first

- Canonical repository: `D:\CodeSpace\track-mesh`. Work in this repository; do not edit another checkout by assumption.
- Current product: Windows Rust CLI, Tauri 2/React desktop, and separate infrastructure services, Phases 1–4 implemented locally. Phases 5–8 and real NAT/HTTPS deployment validation remain pending. The complete DAW workflow is unfinished.
- Priorities: integrity/security → practical direct-transfer speed → reliability → simple workflow → development speed.
- Preserve architecture, protocol names, signed serialization, database values, and recovery ordering unless the task explicitly changes them.
- No automated tests, benchmarks, filesystem watchers, continuous sync, cloud project files, public sharing/discovery, automatic project conflict merging, or non-Windows desktop work in this MVP.

## Find the task owner

Use [docs/CODEMAP.md](docs/CODEMAP.md) to select files. Read the local `AGENTS.md` in the owning crate, then the relevant implementation. Avoid reading every crate or the entire documentation tree.

| Task | Start here |
| --- | --- |
| Receive/resume/installation/recovery | `crates/transfer/AGENTS.md` |
| SQLite/history/trust/publication state | `crates/storage/AGENTS.md` |
| Connectivity/discovery/transport | `crates/network/AGENTS.md` |
| Shared application workflow or CLI | `crates/core/AGENTS.md` |
| Desktop window, native documents, controls/status | `apps/desktop/AGENTS.md` |
| Mailbox HTTP transport or infrastructure | `crates/mailbox/AGENTS.md`, `services/mailbox/AGENTS.md` |
| Separate private relay/rendezvous service | `services/relay/AGENTS.md` |
| Identity/signatures/encryption | `crates/crypto/src/lib.rs` and `shared/protocol/src/lib.rs` |
| Scan/hash/path safety/local chunks | `crates/snapshot/src/lib.rs` |
| Next MVP phase or scope question | [docs/IMPLEMENTATION.md](docs/IMPLEMENTATION.md), then the relevant section of [docs/MVP.md](docs/MVP.md) |

## Working rules

- Keep `lib.rs` files as API/ownership maps. Public APIs are re-exported at crate roots; private modules hold implementation.
- Put reusable project decisions in `core`; CLI, desktop, and future IPC are adapters. The VST3 bridge is a tiny control/status surface.
- One network actor owns the swarm. Blocking disk work stays outside it and outside any audio callback.
- Snapshot creation and transfer installation are different operations. Preserve their locks, transactional commits, and failure states.
- `TransferState` owns session labels; `ApplyPhase` owns journal phases. Do not duplicate either with string literals.
- Add dependencies or abstractions only when the assigned behavior requires them.
- Never print device secrets, decrypted project keys, or invitation payloads. The invitation is a private capability-bearing document.
- Do not change or delete user project folders, local databases, chunk stores, or recovery backups as cleanup.
- Update the code map when moving modules. Update phase status only when the deliverable works; mark unverified integrations explicitly.
- Use a separate local `codex/phase-N-description` branch for each phase. After validation, commit and merge locally into `master` with `--no-ff`, then push `master` normally. Keep phase branches local; do not force push.

## Checks and handoff

Build/static-check commands and Windows runtime requirements are in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md). Do not add or run test/benchmark suites. Report which checks actually ran; historical manual checks are not evidence for a later change.

For a handoff, state the task, affected modules, invariants preserved, checks run, remaining work, and next concrete action. Keep `docs/IMPLEMENTATION.md` as the authoritative progress record; do not create competing status documents.
