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

### Sandbox notes, UI round (2026-10-17)

UX notes 37–39, 42, 43, 45–51: red/amber boxes close with × and no longer clear on a timer (a retry is a new job and replaces the old one); a cancelled file dialog shows nothing; the empty-clipboard error is translated with a next step; a chosen device name is one line with Edit that opens with an ease; Advanced labels the folder as this PC's and lists collaborators under their own heading; project tabs lose their frame (selected = light green surface), the idle dot is a ring; paths that open Explorer end in a small teal "open outside" mark; the Get question says the folder will match and current files go to a backup; the approval window closes once its last pending device is approved (an approved device still gets `Approved` without a window); collaborator names in card sentences sit on the Connected badge's background; confirmation dialogs use one warning icon. Not done: 40/41 (remove project, remove collaborator or leave from the card) need a storage command; 44 covers only the clipboard and file-dialog texts. Checks run: `npx tsc --noEmit`, `cargo fmt --all --check`, `cargo check -p trackmesh-core -p trackmesh-desktop`, the window's source in headless Chrome for ten card states. **Not checked: the Tauri window, the dialog icon, the approval window closing live.**

### Remove project, remove collaborator, Turkish errors (2026-10-17)

UX notes 40, 41, 44. A card can be removed from TrackMesh (the folder and its files are never touched): every collaborator's access is removed (queued on the invite service when this PC created the project; a member leaves locally); a checkbox also deletes this PC's versions, transfers and collaborator links now and the project key at the next start once no access removal is queued. Without it the project is hidden (`projects.removed`, local database v4) and Settings, Removed projects, brings it back without collaborators. The creator can remove the collaborator from the card footer; a member sees Leave project. Removed projects stay in the invite-service loop until their removals are sent. Backend errors a person can reach have Turkish text with a next step; the raw text stays under Details; a cancelled folder change is silent. A database opened by this build can no longer be opened by an older build. Checks run: `npx tsc --noEmit`, `cargo fmt --all --check`, `cargo check -p trackmesh-core -p trackmesh-desktop`, `git diff --check`, the window's source in headless Chrome (ten card states and the remove panel). **Not checked: removing, restoring or leaving in the real app, key deletion at start, the queued service removal.**
