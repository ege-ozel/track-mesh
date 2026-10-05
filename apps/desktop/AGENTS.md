# Windows desktop adapter

Read root AGENTS.md and crates/core/AGENTS.md. This adapter owns presentation, bounded job tracking, native dialogs, and its explicit window session. Core owns project decisions.

- src-tauri/src/lib.rs is the adapter API/ownership map; private modules implement commands, documents, and runtime.
- Call Engine workflows. One PeerRuntime owns the network; reload its handle after setup/trust changes. Never run a swarm per button.
- Blocking startup, DPAPI, SQLite, documents, snapshot, and recovery belong in blocking workers. Await installation during shutdown; never abort apply.
- Desktop owns PluginRuntime on the same Engine/network handle. Window close hides the window while sessions/work/pending publication retain the process; explicit reopen restores it. Claim final idle shutdown under the desktop/IPC acceptance gates, including initialization races, and await accepted work before stopping metadata/peer. Do not create a second plugin swarm.
- Native dialog-selected invitation/card documents stay in Rust. Do not expose invitation payloads or secrets to React, events, logs, or clipboard.
- Keep the existing data-directory contract, signed bytes, schema/state values, journal and installation order, and CLI fields.
- Custom commands require explicit main-window permissions, a bundled-only CSP, and no remote navigation or generic JS filesystem/shell/HTTP access.
- Use static frontend builds and native executable checks. Do not run watchers, automated tests, or benchmarks.
- Phases 5–8 provide VST/Named Pipe, settled save/publication, session/work retention with 60-second grace, and bounded remote/offline/divergence UX. Plugin-launched owners use `--plugin-owner`; repeat plugin launches must not retain an explicit window session. Real DAW hooks remain unverified.
