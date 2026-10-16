# TrackMesh implementation status

The current state in one page. The full dated log up to 2026-10-17 is in [history/IMPLEMENTATION-to-2026-10-17.md](history/IMPLEMENTATION-to-2026-10-17.md) (about 50k tokens): search it for a task (`#44`, `M5`, a date), do not read it whole.

Terminology: **Foundation phases 1–8** are the 2026-09/10 build in `PHASE*.md`; **Aşama 1 M0–M10** is the [direct-P2P plan](plans/phase1-direct-p2p.md); **#NN** names review/UX tasks. Validation is compilation, static checks and manual operation; no automated tests or benchmarks exist.

## What exists

- Windows Rust CLI, Tauri 2/React desktop, C++ VST3 plugin over a Named Pipe, NSIS installer (app, VST3, firewall rule).
- Desktop flow: Add project → Invite (one-read, 15-minute v2 link) → paste link → signed device request → explicit Approve → automatic first version into an empty folder → Share current version / Get collaborator's version. One 15-minute approval window starts at Invite; accept and Approve must both happen inside it.
- Get moves the files of the previous version to a `.trackmesh-…` backup and installs the collaborator's version; only missing chunks travel. "Open my previous files" opens the backup.
- Optional per-user Supabase service: encrypted invite drops, endpoint records, head hints, membership grant/revoke (creator only).
- Presence warning, default-off DAW auto-share, tray lifetime, storage retention, divergence choice (use theirs / keep mine).
- Mailbox and relay code is kept unbuilt in `future/`.

## Verified live (user-run, one host + Windows Sandbox, live Supabase)

- 2026-10-13: invite link, consume, automatic pairing, approval, first receive (host → Sandbox).
- 2026-10-17, build `252bd91`: re-pair from scratch and a ~100 MB send succeeded. Host ran the unbundled exe; Sandbox used the installer.

## Not verified

- A real DAW (project UUID persistence, save hooks, audio), scaled DPI.
- Separate physical networks, CGNAT, IP change while paired.
- Link expiry and second-open rejection, clipboard clearing, revocation, the three-device limit.
- Sandbox → host direction on the current build; the installed host build.
- Self-hosted HTTPS deployment; deployment of the newest Supabase functions.

## Open issues

- A "Project folder not found" notice flashes on the receiver during every Get; `root_available` is a plain folder check and the cause is unknown. Reproduce with a screen recording and logging before changing anything.

## Log

Add new checks below as short dated entries: what changed, checks run, what was not checked. Move entries to `history/` when this file passes ~200 lines.

### Sandbox send check of 252bd91 (2026-10-17)

Release build of `252bd91`; host unbundled exe, Sandbox installer, live Supabase. The user re-paired from scratch and sent a project of about 100 MB; reported successful. Not recorded: transfer time, the reverse direction, a DAW.
