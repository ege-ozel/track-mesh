# Core and CLI changes

Read root `AGENTS.md`. Core owns reusable application decisions; adapters own presentation and their session lifetime.

- `src/lib.rs`: engine/data ownership, project linking, snapshot creation, pairing/invitations, private namespaces.
- `src/workflows.rs`: typed local status, online peer check, receive setup, explicit recovery. Future UI/IPC should call these methods.
- `src/status.rs`: compact bounded project/peer/transfer summaries for adapters; full CLI status remains unchanged.
- `src/sending.rs`: explicit manual snapshot plus publication result; a mailbox failure must preserve and report the successful local commit.
- `src/peer.rs`: network/responder lifetime, blocking request workers, signed ancestry retrieval.
- Peer runtime startup acquires an exclusive Windows data-directory lease held through actor/responder drain, including when an adapter handle is dropped. Adapters must explicitly stop their session.
- `src/mailbox.rs`: durable own-author publication, ACK validation, verified page/cursor commit, session-scoped retry. No head advance or online-equality claim from metadata.
- Metadata methods share a per-project coordinator within an Engine so foreground commands and the session worker cannot race page cursors; SQLite expected-cursor guards remain authoritative across processes.
- `src/main.rs`: CLI parsing, bounded input files, output, network startup/wait/stop, Ctrl+C. Keep lineage and snapshot-install decisions in core/transfer.
- `src/ipc`: current-user-only Windows pipe, bound plugin sessions, compact status and bounded manual send workers. Reuse the owner's Engine/network; receive is a DAW-closed desktop handoff. `PluginRuntime::begin_close` rejects commands immediately; `stop` awaits started work. Save settlement/session-aware lifetime remain Phases 6–7.

Maintain separate project/data directories, persistent DPAPI identity, immutable manifests, and project locks. Snapshot creation rejects pending recovery. Every ancestor is checked for project/requested ID, trusted author, signature/hash, cycle, and bounded history depth.

Online equality needs the contacted peer's acknowledgement. Local status reports last-known verification; it must not imply an offline source is downloadable or currently equal. Network shutdown must drain responders; blocking file apply must retain its lock and finish safely after Ctrl+C.

Core snapshot/recovery methods perform disk work. Future desktop/IPC callers must schedule them away from UI and DAW audio threads. Do not introduce permanent-daemon behavior before implementing the agreed session-aware lifetime phase.

Preserve CLI flags and JSON fields during structural refactors. Current product includes Windows CLI/desktop and a local VST3/IPC surface; the complete save/session/remote workflow remains unfinished. No automated tests/benchmarks.
