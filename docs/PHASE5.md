# Phase 5 Windows VST3 and IPC operation guide

TrackMesh's native C++ VST3 is a small status/control adapter. Rust core owns snapshots, publication, storage and networking. Current progress and actual validation evidence are in [IMPLEMENTATION.md](IMPLEMENTATION.md).

## Build

Use Windows x64, the x64 MSVC developer environment, Windows SDK, CMake 3.25+ and Ninja. Build the desktop's static frontend as described in [PHASE4.md](PHASE4.md). The reference SDK is official Steinberg `v3.8.0_build_66`, root commit `9fad9770f2ae8542ab1a548a68c1ad1ac690abe0`, with its pinned submodules. SDK sources and their licenses remain in ignored `vendor`; they are not copied into repository source.

```powershell
git clone --depth 1 --branch v3.8.0_build_66 --recurse-submodules --shallow-submodules https://github.com/steinbergmedia/vst3sdk.git vendor/vst3sdk
cargo build -p trackmesh-core -p trackmesh-desktop --locked --offline
cmake -S plugins/vst3 -B .build/vst3 -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build .build/vst3 --target TrackMesh --parallel 4
```

The output is `.build/vst3/VST3/Release/TrackMesh.vst3`: a **directory bundle** with `Contents/x86_64-win/TrackMesh.vst3`, bundled desktop/CLI executables and SDK-generated `Contents/Resources/moduleinfo.json`. Keep the entire bundle together. Desktop needs WebView2; C++ needs the MSVC runtime. CMake defaults to the built debug Rust executables; for a release bundle build Rust with `--release` and set `TRACKMESH_AGENT` and `TRACKMESH_DESKTOP` to their release paths. Every plugin-target build refreshes changed Rust executables even when C++ sources need no recompilation. Close applications using this bundle before rebuilding because Windows locks loaded binaries. `TRACKMESH_VST3_SDK` accepts another local copy of the pinned SDK.

CMake disables SDK validator execution and installed-directory links. No tests or benchmarks run. Optional `-DTRACKMESH_BUILD_EDITOR_HOST=ON` exposes the SDK editor host; build only `TrackMesh` and `editorhost`, then open the bundle in `bin/editorhost.exe` for manual inspection. This does not establish DAW compatibility or audio validation.

## Project and controls

The bundle retains the SDK's original license notice in `Contents/Resources/Steinberg-SDK-LICENSE.txt`.

1. Load TRACKMESH as a VST3 effect. It transparently passes mono/stereo 32-bit or 64-bit audio with zero latency. `process` performs no allocation, lock, IPC, filesystem or network work.
2. A random permanent UUID lives in versioned 24-byte component state: format marker/version and 16 UUID bytes. Saving/reopening the same DAW project restores that UUID. Invalid, nil or truncated state is rejected before changing the current ID. Class ID `AD81AE224F924B139E7A621C14B409D8` is permanent.
3. Without a plugin pipe, the worker starts the bundled desktop, which owns the Rust agent and single peer runtime. Select/copy the plugin UUID and use desktop **Link folder** with that exact UUID. There is no automatic `.als`/`.flp` path discovery. Pair/import private invitations with existing native desktop controls. Invitation project UUID and VST state must match; importing another project's invitation does not reassign VST state.
4. **Send Snapshot** queues the existing Rust snapshot/publication workflow. HTTP failure preserves the commit and displays `PENDING`. It publishes metadata, not unsolicited file pushes. Polling displays local/remote IDs, newest known remote author/time, that author's route and durable state. Last-known history is not verified equality.
5. **Receive...** instructs the user to close the DAW project and use desktop Receive; it never installs while this plugin is loaded. Collaborator/snapshot selection, divergence and recovery stay in desktop/core; complete VST receive UX belongs to Phase 8.

The SDK `SingleComponentEffect` combines the small controller/processor to keep one UUID/session owner independent of editor lifetime. Separate components were considered: they require extra UUID/lifetime messaging without moving business logic out of Rust. This non-distributable Windows surface uses native HWND `CPluginView`, with no VSTGUI/additional GUI framework. Editor close/reopen is not component session end.

## IPC and lifetime

Endpoint: `\\.\pipe\TrackMesh-v1-<Windows-user-SID>-<suffix>`, empty suffix by default. Protected current-user-only DACL, remote-client rejection and exclusive first-instance ownership. Each frame is an unsigned 32-bit **little-endian byte length**, then UTF-8 JSON, bounded to 64 KiB. Existing `PluginEvent` and `PluginStatus` serialization is unchanged.

Connect with `PLUGIN_CONNECTED` and a non-nil session/project pair. `PROJECT_ID` rebinds the same session after state restore; all other events must match. Duplicate live sessions, malformed/oversized frames and mismatches disconnect that client. Limits: 16 clients, eight simultaneous snapshot workers, one snapshot job per project, 64 remembered project messages, bounded frame/write timeouts and polling rate. Blocking summaries/snapshots run outside the listener and network actor.

`getState` coalesces `SAVE_OBSERVED`: serialization observation includes host probing and does not establish completed DAW disk writing. Phase 6 now processes accepted events through a project debounce and bounded inventory/writer settlement, then the existing guarded snapshot/commit/publication workflow. It displays waiting, committed/pending, or failure guidance. No folder watcher runs. Unchanged contents reuse their snapshot; a failure never invents a completed snapshot. Details and actual evidence are in [IMPLEMENTATION.md](IMPLEMENTATION.md). Termination queues `SESSION_ENDING` without awaiting pipe I/O. A worker-held DLL reference protects code/state until bounded I/O and C++ cleanup finish. Signals carry their UUID generation so old button actions cannot target a restored project.

After a successful connection, closing desktop does not cause repeated automatic reopening. Reopen desktop to reconnect. Desktop close immediately rejects new IPC work and awaits accepted save debounce/settlement and snapshot/publication workers. Desktop still has explicit window lifetime; DAW-aware lifetime, delivery of signals still queued inside a terminating plugin, and owner retention for pending publication remain Phase 7. Save settlement's two-second quiet period and Windows writer checks cannot prove completion of host writes delayed beyond that period; real DAW behavior remains unverified.

Explicit headless bridge:

```powershell
target\debug\trackmesh-agent.exe plugin-serve
```

Existing network/mailbox options plus `--ipc-suffix` are accepted. Exit follows 60 seconds without bound sessions/active snapshots, or Ctrl+C; started snapshots are awaited. It does not promise Phase 7 pending-publication retention. The headless bridge owns the data-directory runtime lease: close plugin sessions and let it exit before opening desktop against that profile. Ordinary CLI `serve` does not expose IPC.

`TRACKMESH_AGENT_PATH` selects an absolute `trackmesh-desktop.exe` or `trackmesh-agent.exe` path; no shell/PATH search or project-directory executable is selected automatically. `TRACKMESH_DATA_DIR` retains the data profile contract. Separate/disposable profiles need matching `TRACKMESH_IPC_SUFFIX` in DAW, desktop and headless bridge (up to 32 ASCII letters/digits/hyphens). Suffix is routing, not authentication. Never start another swarm against the same directory.

References: [official SDK](https://github.com/steinbergmedia/vst3sdk/tree/v3.8.0_build_66), [SingleComponentEffect](https://steinbergmedia.github.io/vst3_doc/vstsdk/classSteinberg_1_1Vst_1_1SingleComponentEffect.html), [Microsoft named-pipe security](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights).
