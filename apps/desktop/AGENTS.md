# Windows desktop adapter

Read root AGENTS.md and crates/core/AGENTS.md. This adapter owns presentation, bounded job tracking, native dialogs, and its explicit window session. Core owns project decisions.

- src-tauri/src/lib.rs is the adapter API/ownership map; private modules implement commands, documents, and runtime.
- Call Engine workflows. One PeerRuntime owns the network; reload its handle after setup/trust changes. Never run a swarm per button.
- Blocking startup, DPAPI, SQLite, documents, snapshot, and recovery belong in blocking workers. Await installation during shutdown; never abort apply.
- Native dialog-selected invitation/card documents stay in Rust. Do not expose invitation payloads or secrets to React, events, logs, or clipboard.
- Keep the existing data-directory contract, signed bytes, schema/state values, journal and installation order, and CLI fields.
- Custom commands require explicit main-window permissions, a bundled-only CSP, and no remote navigation or generic JS filesystem/shell/HTTP access.
- Use static frontend builds and native executable checks. Do not run watchers, automated tests, or benchmarks.
- This is Phase 4; VST/Named Pipe, save signals, DAW session lifetime and complete history UX remain later phases.
