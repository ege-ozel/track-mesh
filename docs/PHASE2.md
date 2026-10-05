# Phase 2: trusted connectivity and resumable transfer

The CLI implements mutual device pairing, authenticated direct QUIC, TCP with Noise/Yamux fallback, signed snapshot ancestry, missing-chunk transfer, durable resume state, verified file installation, and explicit recovery. Circuit relay, private rendezvous, and DCUtR client behaviors are connected to the network actor; the separately deployed service is Phase 3 work.

## Pair two devices

Use an existing private channel to exchange the invitation and device card. Check the PeerId with your collaborator. The invitation contains the shared project key, so keep it private.

The examples use project UUID `550e8400-e29b-41d4-a716-446655440000`, a project folder on each PC, and listening port `43101`. Substitute the actual project UUID and PC A's reachable LAN address for `192.168.1.10`.

On PC A:

```powershell
trackmesh-agent.exe --device-name "Studio A" link --project-id 550e8400-e29b-41d4-a716-446655440000 --name "Track 07" --folder "D:\Projects\Track 07"
trackmesh-agent.exe snapshot 550e8400-e29b-41d4-a716-446655440000 --output snapshot.json
trackmesh-agent.exe invite 550e8400-e29b-41d4-a716-446655440000 --address /ip4/192.168.1.10/udp/43101/quic-v1 --address /ip4/192.168.1.10/tcp/43101 --output invite.json
```

On PC B, link an empty destination folder by importing the invitation:

```powershell
trackmesh-agent.exe --device-name "Studio B" join --invite-file invite.json --folder "D:\Projects\Track 07"
trackmesh-agent.exe card --output b-card.json
```

Send `b-card.json` back to PC A. PC A must explicitly authorize PC B:

```powershell
trackmesh-agent.exe pair 550e8400-e29b-41d4-a716-446655440000 --card-file b-card.json
```

The signed card contains the device UUID, name, public key, PeerId, and optional addresses. The public key must derive the advertised PeerId. The local project accepts at most two other devices. Adding a third collaborator requires each participating PC to pair that collaborator's card.

## Serve, receive, and compare

On PC A:

```powershell
trackmesh-agent.exe serve --port 43101
```

Close the DAW project on PC B before applying a received snapshot. Use PC A's `device_id` from `invite.json`:

```powershell
trackmesh-agent.exe receive 550e8400-e29b-41d4-a716-446655440000 --device-id <A-device-UUID>
trackmesh-agent.exe check 550e8400-e29b-41d4-a716-446655440000 --device-id <A-device-UUID>
trackmesh-agent.exe status
```

Both receiving and serving processes answer authenticated peer requests. `check` contacts the peer and reports lineage, verified equality, `DIRECT`/`RELAYED`, and transport. `status` reports local history and the last verification; it makes no assertion that an offline peer is currently up to date. Source unavailability produces an error after the connection timeout (`--connect-timeout`, default 30 seconds).

QUIC addresses get a separate first attempt. Direct TCP is attempted after the QUIC handshake timeout, and relay circuits follow failed direct attempts. Local inbound firewall access must permit the chosen UDP/TCP port. There is no public DHT, multicast discovery, or swarm.

To configure the private infrastructure later, link the project with a pinned relay address such as `/dns4/relay.example.com/tcp/4001/p2p/<relay-PeerId>`. `--external-address` on `serve`, `receive`, or `check` advertises an explicitly reachable address. Rendezvous namespaces are derived from the project key; discovered peers are filtered against explicit trust before dialing. The relay identity can transport and discover peers, but cannot request project data unless separately paired for that project.

## Resume and recovery

An interrupted network transfer becomes `PAUSED`. Repeat `receive` with the same `--snapshot-id` shown in `status`. Only missing or corrupted chunks are requested; retained chunks are rehashed before reuse. With `RUST_LOG=trackmesh_transfer=info,trackmesh_network=info`, logs show `reused_bytes`, `missing_bytes`, and `missing_chunks`. The transfer's `total_bytes` and `completed_bytes` count distinct chunk bytes, including reused chunks; the snapshot's `total_size` counts every file byte.

Eight concurrent streams carry binary chunks. Each chunk is checked with BLAKE3, written to a temporary local chunk file, flushed, atomically installed, and then recorded in SQLite. Files are assembled outside the working folder on the same volume, checked against their full-file hash, and flushed before installation.

Applying a snapshot holds a Windows project lock shared by all local snapshot/receive operations. The live folder is checked against the committed base snapshot before applying. Uncommitted local changes, unknown ancestry, and divergent histories block receive. Original files are verified again while being moved to backups. Files excluded from the new snapshot remain in those backups. No recursive deletion or automatic conflict merging is used.

An apply journal moves through `BACKUP`, `INSTALL`, `COMMIT`, and `DONE`. The working folder is marked as requiring recovery before any original files move. An interrupted install continues from the journal instead of recreating files already installed. The local snapshot becomes current only after the whole installed folder matches the signed manifest.

If the process was stopped during file installation, close the DAW and run:

```powershell
trackmesh-agent.exe recover 550e8400-e29b-41d4-a716-446655440000
```

Recovery data remains beside the project folder at `.trackmesh-<project-UUID>-<transfer-UUID>`. Retain the `backup` folder until satisfied with the received project. A damaged journal or unexpected files stop recovery with an explicit error; backups are retained for inspection.

`Ctrl+C` stops networking. If local file installation has started, the agent waits for it to finish safely. `revoke <project-UUID> --device-id <device-UUID>` removes a peer's authorization for that project; running network actors refresh their allow list every six seconds and reject unauthorized project requests immediately.

## Publication state

Phase 2 does not publish mailbox announcements. Authored snapshots remain `PENDING`. Received copies carry `REMOTE`, indicating that their author owns publication; it is not a mailbox acknowledgement. A direct file transfer never manufactures `PUBLISHED` status.

## Validation

Build and manual validation results are recorded in [implementation progress](IMPLEMENTATION.md). Cross-PC firewall behavior and Internet NAT traversal require two real PCs and the Phase 3 relay/rendezvous deployment.
