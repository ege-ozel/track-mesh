TASK: UX-flow2 #35, STATUS: DONE

Implemented TASK #35 sections A–J on `codex/phase-1-ux-flow2` in two commits. J adds an explicit project-root update for moved or renamed folders, missing-folder status with Locate folder, and Change folder under Advanced. Root changes preserve the project UUID and stored trust, keys, backend configuration, snapshots, and history. Core rejects unfinished durable transfers and recovery, checks the data-directory and linked-root conflicts, scans without persisting chunks, requires confirmation when the new folder differs from the latest local snapshot, and updates only the stored root without reloading the network.

Checks run: `cargo fmt --all -- --check`; locked/offline `cargo check` and strict Clippy (`-D warnings`) for `trackmesh-snapshot`, `trackmesh-storage`, `trackmesh-core`, and `trackmesh-desktop`; locked/offline Windows-target Desktop `cargo check`; Desktop `npm run typecheck`; Desktop `npm run build`; and `git diff --check`. No automated tests or benchmarks were run. The build emitted the existing Vite warning that Node 22.11.0 is below the stated 22.12+ minimum.

**live two-instance check not done**. The next concrete action is to validate moving a linked project folder on Windows with two live instances, covering both matching and differing snapshot contents.
