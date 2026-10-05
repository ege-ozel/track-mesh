# Network changes

Read root `AGENTS.md`. The actor is the only swarm owner. Application callers use `Network`, not libp2p internals.

- `src/behaviour.rs`: transports/behaviors, protocol strings, limits, listening, advertised addresses.
- `src/actor.rs`: serialized commands/events, authorization, preferred connections, shutdown/error status.
- `src/discovery.rs`: allow-list refresh, private rendezvous, reservations, direct retry ordering, learned-address bounds.
- `src/handle.rs`: bounded command/RPC interface; `src/codec.rs`: bounded JSON and raw chunk frames.
- `src/identity.rs`: signed application key ↔ PeerId; `src/status.rs`: online/route/transport reporting; `src/config.rs`: startup settings.

Preserve QUIC preference, TCP + Noise/Yamux fallback, relay-client/DCUtR support, and six-second retry/refresh behavior. Direct connections replace relayed ones; QUIC replaces direct TCP. Keep existing timeout/stream/window/connection limits unless explicitly tuning them.

Paired peers and pinned infrastructure may connect; project request authorization is still required. Validate signed cards and key-derived PeerId. Filter rendezvous results through trust. Infrastructure identity alone grants no project-data permission. No public DHT, mDNS, swarms, or file upload to the mailbox.

The actor must not hash/scan large files or perform blocking disk work. Responders in `core/src/peer.rs` delegate those operations to blocking workers. Fatal actor exits resolve pending requests and report offline/error status.

Keep codec tags, size bounds, protocol strings, and signed types stable when moving code. Client support exists; real two-PC NAT/relay/DCUtR validation and the service deployment remain pending. No automated tests/benchmarks.
