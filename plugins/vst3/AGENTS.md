# Windows VST3 adapter

Read root AGENTS.md and crates/core/AGENTS.md. This is a tiny native control/status surface built with the pinned official Steinberg SDK. No plugin-side business logic, filesystem scanning, hashing, database, peer network or project path discovery.

- `src/plugin.cpp`: SDK factory, transparent audio processing, permanent project UUID in versioned component state, save/termination signals.
- `src/bridge.cpp`: bounded local Named Pipe worker; only this worker performs IPC or starts the bundled Rust agent. It pins the module until its own cleanup so terminate never waits for pipe I/O.
- `src/view.cpp`: native Windows view and Send/Receive commands. Receive directs to the DAW-closed desktop workflow; never install into an open DAW project.
- Existing PluginEvent/PluginStatus, protocol names, signing bytes and database values remain unchanged. State UUID changes must rebind before commands. Editor close is not session end.
- CMake disables SDK validator execution and installed-folder links. Do not add/run automated tests or benchmarks. Build/static checks and explicit manual native operation only.
- Phase 6 save settlement is owned by Rust core/snapshot; keep this shim's save observation independent of filesystem completion. Full session-aware lifetime remains Phase 7. Record limits and actual validation in docs/IMPLEMENTATION.md.
