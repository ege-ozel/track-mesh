# Deferred until after 1.0.0

Plan only, not validation. These items are intentionally left out of 1.0.0 and are candidates for the next major version.

## Relay

Decided 2026-10-12: a relay for peers that cannot reach each other directly (CGNAT, blocked forwarding) is deferred until after 1.0.0. Supabase cannot host a libp2p circuit relay, so this needs separate infrastructure. Until then, 1.0.0 relies on direct connections plus the optional private relay described in the setup guide.

## Content-defined chunking

Today snapshots split each file into fixed 4 MiB chunks (`DEFAULT_CHUNK_SIZE`), hashed with BLAKE3.
- Unchanged files and in-place overwrites already transfer only the changed chunks.
- An insertion or deletion in the middle of a file shifts every later chunk boundary, so the rest of that file transfers again.

Content-defined chunking (for example FastCDC) chooses boundaries from the content, so an insert only changes the chunks around it. This changes the manifest/chunk format, so it needs a protocol version bump and a compatibility plan for existing chunk stores and peers on the old format. Typical audio projects either leave files unchanged or rewrite them completely (render/bounce), so the expected gain should be measured on real projects before this is scheduled.
