# TrackMesh repository instructions for Claude

Read [AGENTS.md](AGENTS.md) first. It is the canonical repository policy; follow its task routing, crate-local instructions, compatibility/security rules and branch/check/handoff workflow. Use [docs/CODEMAP.md](docs/CODEMAP.md) for narrow file selection and [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for build/static checks. Do not add or run automated tests or benchmarks.

For two-PC setup/documentation work, start with [docs/setup/README.md](docs/setup/README.md). Sender instructions live in [host/README.md](docs/setup/host/README.md); receiver instructions live in [peer/README.md](docs/setup/peer/README.md). These roles describe the first transfer direction, not a central server architecture.

The standalone peer-package agent template is [docs/setup/peer/CLAUDE.md](docs/setup/peer/CLAUDE.md). Copy that template to the package root as `CLAUDE.md`; do not distribute this repository-development entry point in its place. The package template must work without a repo checkout. Keep invitation payloads, keys, secrets, databases and user projects out of agent context, logs and distribution packages; native import accepts only the private invitation file path.

Keep setup instructions in their role folders and shared troubleshooting in the setup entry point. Update links when moving documents. Record actual checks and remaining integration work only in [docs/IMPLEMENTATION.md](docs/IMPLEMENTATION.md); instructions are not validation evidence.
