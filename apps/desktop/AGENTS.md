# Windows desktop adapter

Read root AGENTS.md and crates/core/AGENTS.md. This adapter owns presentation, bounded job tracking, native dialogs, and its explicit window session. Core owns project decisions.

- src-tauri/src/lib.rs is the adapter API/ownership map; private modules implement commands, documents, and runtime.
- Call Engine workflows. One PeerRuntime owns the network; reload its handle after setup/trust changes. Never run a swarm per button.
- Blocking startup, DPAPI, SQLite, documents, snapshot, and recovery belong in blocking workers. Await installation during shutdown; never abort apply.
- Desktop also owns Phase 5 PluginRuntime on the same Engine/network handle. Reject IPC work at close, including initialization races, and await started plugin snapshot/publication workers. Do not create a second plugin swarm.
- Native dialog-selected invitation/card documents stay in Rust. Do not expose invitation payloads or secrets to React, events, logs, or clipboard.
- Keep the existing data-directory contract, signed bytes, schema/state values, journal and installation order, and CLI fields.
- Custom commands require explicit main-window permissions, a bundled-only CSP, and no remote navigation or generic JS filesystem/shell/HTTP access.
- Use static frontend builds and native executable checks. Do not run watchers, automated tests, or benchmarks.
- Phase 5 VST/Named Pipe controls are integrated locally. Settled save processing, full DAW session lifetime and complete history/VST receive UX remain Phases 6–8.
