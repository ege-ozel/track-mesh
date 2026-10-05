# Core and CLI changes

Read root `AGENTS.md`. Core owns reusable application decisions; adapters own presentation and their session lifetime.

- `src/lib.rs`: engine/data ownership, project linking, snapshot creation, pairing/invitations, private namespaces.
- `src/workflows.rs`: typed local status, online peer check, receive setup, explicit recovery. Future UI/IPC should call these methods.
- `src/peer.rs`: network/responder lifetime, blocking request workers, signed ancestry retrieval.
- `src/mailbox.rs`: durable own-author publication, ACK validation, verified page/cursor commit, session-scoped retry. No head advance or online-equality claim from metadata.
- `src/main.rs`: CLI parsing, bounded input files, output, network startup/wait/stop, Ctrl+C. Keep lineage and snapshot-install decisions in core/transfer.

Maintain separate project/data directories, persistent DPAPI identity, immutable manifests, and project locks. Snapshot creation rejects pending recovery. Every ancestor is checked for project/requested ID, trusted author, signature/hash, cycle, and bounded history depth.

Online equality needs the contacted peer's acknowledgement. Local status reports last-known verification; it must not imply an offline source is downloadable or currently equal. Network shutdown must drain responders; blocking file apply must retain its lock and finish safely after Ctrl+C.

Core snapshot/recovery methods perform disk work. Future desktop/IPC callers must schedule them away from UI and DAW audio threads. Do not introduce permanent-daemon behavior before implementing the agreed session-aware lifetime phase.

Preserve CLI flags and JSON fields during structural refactors. Current product is the CLI, not the completed desktop/VST3 MVP. No automated tests/benchmarks.
