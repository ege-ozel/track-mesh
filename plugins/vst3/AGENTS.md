# Windows VST3 adapter

Read root AGENTS.md and crates/core/AGENTS.md. This is a tiny native control/status surface built with the pinned official Steinberg SDK. No plugin-side business logic, filesystem scanning, hashing, database, peer network or project path discovery.

- `src/plugin.cpp`: SDK factory, transparent audio processing, permanent project UUID in versioned component state, save/termination signals.
- `src/bridge.cpp`: bounded local Named Pipe worker; only this worker performs IPC or starts the bundled Rust agent. It pins the module until its own cleanup so terminate never waits for pipe I/O.
- `src/view.cpp`: native Windows view and Send/Receive commands. Receive directs to the DAW-closed desktop workflow; never install into an open DAW project.
- Existing PluginEvent/PluginStatus, protocol names, signing bytes and database values remain unchanged. State UUID changes must rebind before commands. Editor close is not session end.
- CMake disables SDK validator execution and installed-folder links. Do not add/run automated tests or benchmarks. Build/static checks and explicit manual native operation only.
- Save settlement and work-based session lifetime are owned by Rust. Drain queued current-generation save/send signals before session ending, with bounded worker retry; host termination never joins IPC. Forced host-process exit can prevent undelivered events. Launch desktop with `--plugin-owner`; keep save observation independent of filesystem completion. Record limits and actual validation in docs/IMPLEMENTATION.md.
