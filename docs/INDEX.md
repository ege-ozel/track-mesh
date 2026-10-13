# Documentation index

Choose the page for the current task; these pages are not a required reading sequence.

| Page | Purpose |
| --- | --- |
| [Root README](../README.md) | Product status and quick build entry point |
| [Two-PC setup entry point (Türkçe)](setup/README.md) | Invitation-based discovery, panel status, IP/hotspot changes, bidirectional connections, database/relay roles and ordered troubleshooting |
| [Sender / host setup (Türkçe)](setup/host/README.md) | Source build/launch, project link, LAN/WAN addresses, TP-Link rules, Windows Firewall, invitation and snapshot |
| [Receiving peer setup (Türkçe)](setup/peer/README.md) | Package launch, Desktop or CLI choice, invitation import, automatic device request and check/receive/check |
| [Automatic pairing security (Türkçe)](setup/PAIRING_SECURITY.md) | Enrollment challenge, explicit approval, transport/authorization separation, bounds and sniffing limits |
| [Peer package CLAUDE.md (Türkçe)](setup/peer/CLAUDE.md) | Standalone distribution-package agent instructions; invitation payload stays outside agent context |
| [Agent entry point](../AGENTS.md) | Repository rules and task routing |
| [Code map](CODEMAP.md) | Exact file ownership, public entry points, and targeted searches |
| [Architecture](ARCHITECTURE.md) | Dependency boundaries, durable flows, and security/recovery invariants |
| [Development](DEVELOPMENT.md) | Build/static checks, Windows prerequisites, local data, and change discipline |
| [MVP contract](MVP.md) | Accepted scope and requirements for unfinished phases |
| [Phase 1 direct P2P plan](plans/phase1-direct-p2p.md) | Planned UPnP/PCP mapping, reachability state, endpoint updates; plan only, not validation |
| [Phase 1 M5/M6 endpoint and invitation design](plans/phase1-m5-endpoint-design.md) | Design only: signed endpoint updates, mailbox channel, schema migration and one-time invite-drop option |
| [Deferred until after 1.0.0](plans/post-1.0.md) | Plan only: relay, content-defined chunking, mailbox close lifetime |
| [Implementation progress](IMPLEMENTATION.md) | Current phase status, historical manual evidence, and integration gaps |
| [Phase 2 operation guide](PHASE2.md) | Current request-pairing workflow, explicit legacy card recipe, serve/check/receive/resume/recovery |
| [Phase 3 infrastructure guide](PHASE3.md) | Encrypted publication/refresh, service ACLs, HTTPS, and private relay/rendezvous |
| [Phase 4 desktop guide](PHASE4.md) | Native Windows build, pairing/documents, controls, status, and safe close |
| [Phase 5 VST3/IPC guide](PHASE5.md) | Official SDK build, UUID state, native controls, private pipe and owner lifetime |

Crate-local `AGENTS.md` files add focused guidance for core, network, storage, and transfer changes. Source and schema define exact current behavior; progress claims belong in `IMPLEMENTATION.md`.
