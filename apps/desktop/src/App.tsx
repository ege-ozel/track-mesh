import { useCallback, useEffect, useRef, useState } from 'react';
import { chooseFolder, closeApp, getStatus, setUpnpMapping, submitAction } from './api';
import type { Action, CheckView, DesktopStatus, ProjectCard, SnapshotInfo } from './api';

const short = (id: string | null | undefined) => id ? id.slice(0, 8) : '—';
const time = (value: string | null | undefined) => value ? new Date(value).toLocaleString() : 'Never';
const bytes = (n: number) => n >= 1024 ** 3 ? `${(n / 1024 ** 3).toFixed(1)} GB` : n >= 1024 ** 2 ? `${(n / 1024 ** 2).toFixed(1)} MB` : n >= 1024 ? `${(n / 1024).toFixed(1)} KB` : `${n} B`;
const CHECK_TIMEOUT_SECONDS = 30;

function localIpv4Addresses(status: DesktopStatus) {
  return [...new Set((status.network?.addresses ?? []).flatMap(address => {
    const match = address.match(/^\/ip4\/([^/]+)\/(tcp|udp)\/(\d+)(?:\/|$)/);
    return match && match[1] !== '0.0.0.0' && !match[1].startsWith('127.') && Number(match[3]) === status.listen_port ? [match[1]] : [];
  }))];
}

function publicIpv4(value: string) {
  const parts = value.split('.');
  if (parts.length !== 4 || parts.some(p => !/^(0|[1-9]\d{0,2})$/.test(p) || Number(p) > 255)) return false;
  const [a, b, c] = parts.map(Number);
  return a > 0 && a < 224 && a !== 10 && a !== 127
    && !(a === 100 && b >= 64 && b <= 127) && !(a === 169 && b === 254)
    && !(a === 172 && b >= 16 && b <= 31) && !(a === 192 && b === 168)
    && !(a === 192 && b === 0 && c === 2) && !(a === 198 && b === 51 && c === 100)
    && !(a === 203 && b === 0 && c === 113) && !(a === 198 && (b === 18 || b === 19));
}

function ConnectionSetup({ status, projectId, busy, act }: { status: DesktopStatus; projectId: string; busy: boolean; act: (action: Action) => Promise<void> }) {
  const [ip, setIp] = useState('');
  const [port, setPort] = useState(String(status.listen_port));
  const [mappingSaving, setMappingSaving] = useState(false);
  const [mappingError, setMappingError] = useState<string | null>(null);
  async function changeMappingConsent(enabled: boolean) {
    setMappingSaving(true);
    setMappingError(null);
    try { await setUpnpMapping(enabled); }
    catch (error) { setMappingError(String(error)); }
    finally { setMappingSaving(false); }
  }
  const publicIp = ip.trim();
  const validPort = /^\d{1,5}$/.test(port) && Number(port) > 0 && Number(port) <= 65535;
  const valid = publicIpv4(publicIp) && validPort;
  const addresses = status.network?.addresses ?? [];
  const localIps = localIpv4Addresses(status);
  const observation = status.network_observation;
  const gateways = observation.default_gateways;
  return <details className="connection-setup"><summary>Connection setup</summary>
    <p><strong>Observation (read only)</strong></p>
    <p>Observed local IPv4 interfaces: {observation.interfaces.length ? observation.interfaces.map(iface => <span key={iface.interface_index}>{iface.name || `Interface ${iface.interface_index}`} [{iface.ipv4.join(', ') || 'UNKNOWN'}]; </span>) : 'UNKNOWN'}</p>
    <p>Observed default gateway(s): {gateways.length ? gateways.map(gateway => <span key={`${gateway.interface_index}-${gateway.address}`}>{gateway.address} (interface {gateway.interface_index}; {gateway.source}); </span>) : 'UNKNOWN'}</p>
    <p>WAN IP = visible IP? {observation.wan_visible_ip.state} (source: {observation.wan_visible_ip.source})</p>
    <p>Port mapping observation: {observation.mappings.map(mapping => `${mapping.method} ${mapping.transport}: ${mapping.state}`).join('; ')}</p>
    <p><strong>UPnP IGD (observation)</strong>: {status.network?.port_mapping.state ?? 'NOT_ATTEMPTED'}; TCP {status.network?.port_mapping.tcp ?? 'NOT_ATTEMPTED'}; UDP {status.network?.port_mapping.udp ?? 'NOT_ATTEMPTED'}; router external IPv4 {status.network?.port_mapping.external_ip_state ?? 'UNKNOWN'}</p>
    <label><input type="checkbox" checked={status.network?.port_mapping.consent_enabled ?? false} disabled={busy || mappingSaving || !status.ready || status.closing}
      onChange={event => { void changeMappingConsent(event.currentTarget.checked); }} />
      Request time-limited UPnP port mappings on my router (explicit consent). Inbound reachability remains unverified.</label>
    {mappingError && <p className="error">Could not save UPnP preference: {mappingError}</p>}
    {status.network?.port_mapping.detail && <p>{status.network.port_mapping.detail}</p>}
    <p>Local listen port: <strong>{status.listen_port}</strong> (default 43101). This session keeps its listen port; a different local port requires reopening with that startup port after the current owner exits.</p>
    <p>Local IPv4 / destination port: {localIps.length ? localIps.map(localIp => <code key={localIp}>{localIp}:{status.listen_port} </code>) : 'Waiting for a local IPv4 listen address. Do not use 0.0.0.0 as the router destination.'}</p>
    <p>Select the IP on the network connected to your router. Reserve that PC address in the router DHCP settings.</p>
    <div className="connection-inputs"><label>Public IPv4 (router WAN / ISP)<input value={ip} maxLength={15} disabled={busy} onChange={e => setIp(e.target.value)} placeholder="Enter public IPv4" /></label>
      <label>External router port<input type="number" min={1} max={65535} value={port} disabled={busy} onChange={e => setPort(e.target.value)} /></label></div>
    {publicIp && !valid && <p className="error">Enter a public IPv4 and a port from 1 to 65535.</p>}
    <p>CGNAT / private ranges: 100.64.0.0/10, 10/8, 172.16/12 and 192.168/16 are not public IPs. If your router WAN is in these ranges, ask your ISP for a reachable public IPv4 or use a configured private relay. Entering an address does not verify reachability.</p>
    {valid && <div className="connection-addresses"><code>/ip4/{publicIp}/tcp/{port}</code><code>/ip4/{publicIp}/udp/{port}/quic-v1</code></div>}
    <p>Router port forwarding: TCP {validPort ? port : 'PORT'} → this PC, local port {status.listen_port}; UDP {validPort ? port : 'PORT'} → this PC, local port {status.listen_port}. In Windows Firewall, allow TrackMesh inbound TCP {status.listen_port} and UDP {status.listen_port} on the active network profile. No firewall or router rules are changed automatically.</p>
    <p>Keep TrackMesh open, configure forwarding and firewall, then export. The generated public endpoints are included in this invitation only; these fields are not saved or advertised by the running swarm. Re-export and privately exchange a new invitation if the public IP changes.</p>
    <button disabled={busy || !valid} onClick={() => act({ kind: 'export_invite', project_id: projectId, public_ip: publicIp, external_port: Number(port) })}>Export invitation with public addresses</button>
    <details><summary>Current advertised / listen addresses</summary>{addresses.map(address => <code className="connection-address" key={address}>{address}</code>)}
      {status.configured_external_addresses.map(address => <code className="connection-address" key={`external-${address}`}>{address} (startup external address)</code>)}</details>
  </details>;
}

function Snapshot({ value, empty }: { value: SnapshotInfo | null; empty: string }) {
  return value ? <><div className="snapshot-id">{short(value.snapshot_id)} <span>{value.metadata_status}</span></div>
    <p>{value.author_name} · {time(value.created_at)}</p>
    <p>{value.file_count} files · {bytes(value.total_size)} · {bytes(value.changed_bytes)} changed</p></>
    : <p className="muted">{empty}</p>;
}

function jobStatus(job: DesktopStatus['jobs'][number], startedAt: number | undefined) {
  if (!job.running) {
    return job.error ? `${job.label}: ${job.error}`
      : job.outcome?.publication_error ? `Snapshot committed locally. Publication pending: ${String(job.outcome.publication_error)}`
      : job.outcome?.message ? String(job.outcome.message) : `${job.label} completed`;
  }
  if (job.label !== 'Check peer') return `${job.label}…`;
  const elapsed = startedAt ? Math.floor((Date.now() - startedAt) / 1000) : 0;
  const remaining = Math.max(0, CHECK_TIMEOUT_SECONDS - elapsed);
  return `Check peer… trying to reach collaborator for up to ${CHECK_TIMEOUT_SECONDS}s (${remaining}s left)`;
}

function JobNotice({ job, startedAt }: { job: DesktopStatus['jobs'][number]; startedAt: number | undefined }) {
  const warning = job.error || job.outcome?.pairing_state === 'retry';
  return <div className={`job ${warning ? 'danger' : ''}`} role="status">
    {jobStatus(job, startedAt)}
    {typeof job.outcome?.path === 'string' && <p>Saved file: <code>{job.outcome.path}</code></p>}
  </div>;
}

function ConnectionAssistant({ card, status, peer, check, busy, act }: {
  card: ProjectCard; status: DesktopStatus; peer: ProjectCard['peers'][number] | undefined;
  check: CheckView | undefined; busy: boolean; act: (action: Action) => Promise<void>;
}) {
  const projectId = card.project.project_id;
  const candidates = status.pairing.project_id === projectId ? status.pairing.requests : [];
  const connection = peer ? status.network?.peers[peer.device.peer_id] : undefined;
  const checkJob = status.jobs.find(job => job.project_id === projectId && job.label === 'Check peer');
  const pairingRetry = status.jobs.some(job => job.project_id === projectId && job.outcome?.pairing_state === 'retry');
  const localIps = localIpv4Addresses(status);
  const runtimeError = status.error ?? status.network?.error;
  const [expanded, setExpanded] = useState(!connection?.online || !!runtimeError);
  let next = 'Export a current-address invitation on the sender PC, then accept it on the receiver PC.';
  if (runtimeError) next = 'The local network session has an error. Reopen TrackMesh after current work finishes.';
  else if (candidates.length) next = 'Compare the pending Device and Identity with the other PC, then approve the expected device in Pair collaborator.';
  else if (!peer && status.pairing.project_id === projectId) next = 'The pairing window is open. Keep TrackMesh running while the other PC accepts the private invitation.';
  else if (peer && checkJob?.running) next = 'Trying the selected collaborator now. This can take up to 30 seconds.';
  else if (peer && checkJob?.error) next = connection?.online
    ? 'The peer connection exists, but the project check failed. Read the error below and confirm the sender approved this device for this project.'
    : 'The connection check failed. Keep both PCs open, compare the sender’s current Wi-Fi IP with the invitation, then check guest isolation and inbound Windows Firewall on the sender. A timeout cannot identify which one blocked traffic.';
  else if (peer && !connection?.online) next = 'Run the connection check. If the invitation was imported but approval is still pending, open the sender’s pairing window and retry automatic pairing here.';
  else if (peer && !check?.fresh) next = 'The peer is connected. Run the connection check to verify project access and compare current snapshots.';
  else if (card.project.recovery_required) next = 'Local installation needs recovery. Close the DAW project and use Recover locally before another Receive.';
  else if (check?.result.lineage === 'DIVERGED') next = 'Both folders changed independently. Resolve the difference manually before receiving; TrackMesh does not merge projects.';
  else if (check?.result.verified_equal) next = 'Both snapshot heads are equal on this live connection.';
  else if (check?.result.lineage === 'LOCAL_AHEAD') next = 'Your local snapshot is ahead. The other PC should Check peer and Receive from this PC.';
  else if (check?.result.lineage === 'UNKNOWN') next = 'Snapshot ancestry is unknown. Review the current project warning before receiving.';
  else if (check?.result.remote_snapshot) next = 'A source snapshot is available. Close the DAW project, then use Receive when the button is enabled.';
  else if (card.local_snapshot) next = 'Your local snapshot is ready. The other PC should run Check peer and Receive.';
  else if (peer) next = 'No source snapshot is available yet. Create one with Send Snapshot on the source PC.';

  return <details className="connection-assistant" open={expanded} onToggle={event => setExpanded(event.currentTarget.open)}>
    <summary>Connection check · one-click peer check and next step</summary>
    <div className="diagnostic-grid">
      <p><strong>Local app:</strong> {runtimeError ? `Network error: ${runtimeError}` : status.ready ? 'Network session running' : 'Starting'}</p>
      <p><strong>Local address:</strong> {localIps.length ? localIps.map(ip => `${ip}:${status.listen_port}`).join(', ') : `No Wi-Fi IPv4 address shown yet · listen port ${status.listen_port}`}</p>
      <p><strong>Device:</strong> {candidates.length ? `${candidates.length} request awaiting your approval` : peer ? `${peer.device.name} recorded locally; remote approval is verified by Check peer` : status.pairing.project_id === projectId ? 'Pairing window open; waiting for the other PC' : 'No collaborator recorded yet'}</p>
      <p><strong>Connection:</strong> {connection?.online ? `${connection.route} · ${connection.transport}` : checkJob?.running ? 'Checking now' : checkJob?.error ? 'Last check failed' : 'No active peer connection'}</p>
      <p><strong>Snapshot:</strong> {check?.fresh ? check.result.verified_equal ? 'Fresh equality verified' : check.result.remote_snapshot ? `Source snapshot ${short(check.result.remote_snapshot)} available` : 'No remote snapshot on the fresh check' : 'Not freshly verified'}</p>
    </div>
    <p className="diagnostic-next"><strong>Next:</strong> {next}</p>
    {peer && <div className="actions"><button disabled={busy} onClick={() => act({ kind: 'check', project_id: projectId, device_id: peer.device.device_id })}>{checkJob?.running ? 'Checking…' : 'Run connection check'}</button>
      {pairingRetry && <button disabled={busy} onClick={() => act({ kind: 'request_pairing', project_id: projectId, device_id: peer.device.device_id })}>Retry automatic pairing</button>}</div>}
    <p className="muted">This checks the app’s own session and selected peer. Windows Firewall, guest Wi-Fi isolation and the other PC’s current IP cannot be identified separately from a connection timeout.</p>
  </details>;
}

function Card({ card, status, jobStartedAt, act }: { card: ProjectCard; status: DesktopStatus; jobStartedAt: Record<string, number>; act: (action: Action) => Promise<void> }) {
  const [selected, setSelected] = useState('');
  const [dawClosed, setDawClosed] = useState(false);
  const project = card.project;
  const peer = card.peers.find(p => p.device.device_id === selected) ?? card.peers[0];
  const peerId = peer?.device.device_id;
  const connection = peer ? status.network?.peers[peer.device.peer_id] : undefined;
  const online = connection?.online ?? false;
  const job = status.jobs.find(j => j.project_id === project.project_id);
  const busy = !status.ready || status.closing || status.jobs.some(j => j.running && (!j.project_id || j.project_id === project.project_id));
  const check = status.checks.find(c => c.result.project_id === project.project_id && c.device_id === peerId);
  const fresh = !busy && (check?.fresh ?? false);
  const checkedLineage = check?.current_known ? check.result.lineage : null;
  const lineage = checkedLineage ?? peer?.known_lineage ?? null;
  const divergence = lineage === 'DIVERGED';
  const verifiedForPeer = project.verified_peer_id === peer?.device.peer_id
    && project.verified_snapshot_id === project.latest_local_snapshot;
  const transfer = card.transfer;
  const resumable = transfer?.state === 'PAUSED' && peerId === transfer.source_device_id;
  const receiving = transfer?.state === 'RECEIVING' || transfer?.state === 'APPLYING';
  const disabledReceive = busy || !peer || !online || !dawClosed || project.recovery_required || divergence;
  const request = (kind: 'send' | 'publish' | 'refresh' | 'pair' | 'export_invite') => act({ kind, project_id: project.project_id });
  return <article className="project-card">
    <header className="card-title"><div><h2>{project.name}</h2><p title={project.root}>{project.root}</p></div>
      <span className={`badge ${online ? 'online' : ''}`}>{online ? connection?.route : 'OFFLINE'}</span></header>
    <div className="card-meta"><span>PROJECT {short(project.project_id)}</span><span>{card.pending_publications ? `${card.pending_publications} publication pending` : 'No publication pending'}</span></div>
    <div className="snapshots"><section><h3>LOCAL SNAPSHOT</h3><Snapshot value={card.local_snapshot} empty="Create your first snapshot with Send Snapshot." /></section>
      <section><h3>KNOWN REMOTE METADATA</h3><Snapshot value={peer?.announcement ?? null} empty="Pair a collaborator, then refresh or check." />
        {!online && peer?.announcement && <p className="offline-note">Source offline. Project files stay on collaborator PCs.</p>}
        {peer && peer.history.length > 1 && <details className="remote-history"><summary>{peer.history.length} recent snapshots</summary>
          <ol>{peer.history.map(snapshot => <li key={snapshot.snapshot_id}><code>{short(snapshot.snapshot_id)}</code><span>{time(snapshot.created_at)} · {bytes(snapshot.changed_bytes)} changed</span></li>)}</ol>
        </details>}</section></div>
    <div className="peer-line"><label>Collaborator <select value={peerId ?? ''} disabled={busy || !card.peers.length} onChange={e => { setSelected(e.target.value); setDawClosed(false); }}>
      {!card.peers.length && <option value="">No paired devices</option>}{card.peers.map(p => <option key={p.device.device_id} value={p.device.device_id}>{p.device.name}</option>)}</select></label>
      <span className="muted">{online ? connection?.transport.toUpperCase() : 'Last-known status · Check peer may take up to 30s'}</span>
      <button className="text-button" disabled={busy || !peer} onClick={() => peer && act({ kind: 'check', project_id: project.project_id, device_id: peer.device.device_id })}>Check peer</button></div>
    <div className={`verification ${divergence ? 'danger' : ''}`}>
      {fresh && check?.result.verified_equal ? `UP TO DATE · verified ${check.age_seconds}s ago`
        : check?.current_known && check.result.lineage && check.result.lineage !== 'EQUAL' ? `${check.result.lineage.replaceAll('_', ' ')} · ${fresh ? 'checked' : 'last checked'} ${check.age_seconds}s ago`
        : lineage && lineage !== 'EQUAL' ? `${lineage.replaceAll('_', ' ')} · last-known metadata, Check peer before receiving`
        : `${verifiedForPeer ? `Last verified: ${time(project.last_verified_at)}` : 'No current verification for this collaborator'} · Check peer for a fresh comparison`}
    </div>
    <ConnectionAssistant card={card} status={status} peer={peer} check={check} busy={busy} act={act} />
    {project.recovery_required && <div className="notice danger">Interrupted installation requires local recovery. Backups are retained.</div>}
    {transfer && <div className="transfer"><div><span>Last transfer · {short(transfer.snapshot_id)} · {transfer.state.replaceAll('_', ' ')}</span><span>{bytes(transfer.completed_bytes)} / {bytes(transfer.total_bytes)}</span></div>
      <progress max={Math.max(transfer.total_bytes, 1)} value={transfer.completed_bytes} />
      {transfer.error && <p className="error">{transfer.error}</p>}{receiving && <p className="muted">Verified chunks are saved durably. Installation finishes safely before exit.</p>}</div>}
    <label className="confirmation"><input type="checkbox" checked={dawClosed} disabled={busy} onChange={e => setDawClosed(e.target.checked)} /> I closed this project in the DAW before Receive or Recovery.</label>
    <div className="actions"><button className="primary" disabled={busy || project.recovery_required} onClick={() => request('send')}>↑ Send Snapshot</button>
      <button disabled={disabledReceive} onClick={() => peer && act({ kind: 'receive', project_id: project.project_id, device_id: peer.device.device_id, snapshot_id: resumable ? transfer.snapshot_id : null, daw_closed: dawClosed })}>{resumable ? 'Resume receive' : '↓ Receive'}</button>
      {project.recovery_required && <button disabled={busy || !dawClosed} onClick={() => act({ kind: 'recover', project_id: project.project_id, daw_closed: dawClosed })}>Recover locally</button>}
      <button disabled={busy || !project.mailbox_url} onClick={() => request('refresh')}>Refresh metadata</button>
      <button disabled={busy || !project.mailbox_url || !card.pending_publications} onClick={() => request('publish')}>Retry publication</button></div>
    {job && <JobNotice job={job} startedAt={jobStartedAt[job.job_id]} />}
    <details className="setup" open={status.pairing.project_id === project.project_id && status.pairing.requests.length > 0}><summary>Collaboration & connection details</summary>
      <details className="pairing-setup" open={status.pairing.project_id === project.project_id}><summary>Pair collaborator</summary>
        <ol>
          <li><strong>Send an invitation.</strong> Use current addresses on the same LAN, or public addresses in Connection setup below. Export opens a 10-minute pairing window; keep TrackMesh online.</li>
          <li><strong>Your collaborator accepts it.</strong> Their signed device identity and invitation proof arrive automatically over an encrypted connection. They do not send a card file back.</li>
          <li><strong>Verify and approve their device here.</strong> Confirm the expected device identity with your collaborator. Approval grants project access. Then Send Snapshot and keep TrackMesh open while they Check peer and Receive.</li>
        </ol>
        <div className="actions"><button disabled={busy} onClick={() => request('export_invite')}>Export invitation with current addresses</button>
          <button disabled={busy || status.pairing.project_id === project.project_id} onClick={() => act({ kind: 'open_pairing', project_id: project.project_id })}>Open pairing window</button>
          <button disabled={busy || !peer} onClick={() => peer && act({ kind: 'request_pairing', project_id: project.project_id, device_id: peer.device.device_id })}>Retry automatic pairing</button></div>
        {status.pairing.project_id === project.project_id && <div role="status">
          <p>Waiting for pairing requests · {status.pairing.remaining_seconds}s left. One project window is open at a time; closing, expiry or restart removes unapproved requests.</p>
          <button disabled={busy} onClick={() => act({ kind: 'close_pairing', project_id: project.project_id })}>Close pairing window</button>
          {status.pairing.requests.map(candidate => <div className="notice" key={candidate.request_id}>
            <p><strong>{candidate.name}</strong> requests access to {project.name}.</p>
            <p>Device <code>{candidate.device_id}</code></p><p>Identity <code>{candidate.peer_id}</code></p>
            <p>Invitation possession verified. A device name alone is not identity verification; approve only the expected collaborator.</p>
            <div className="actions"><button disabled={busy} onClick={() => window.confirm(`Authorize device ${candidate.name} (${candidate.device_id}) for ${project.name}? Confirm its identity with your collaborator first.`) && act({ kind: 'approve_pairing', project_id: project.project_id, request_id: candidate.request_id })}>Approve device</button>
              <button disabled={busy} onClick={() => act({ kind: 'reject_pairing', project_id: project.project_id, request_id: candidate.request_id })}>Reject</button></div>
          </div>)}
        </div>}
        <p>Encryption protects message contents, not IP addresses or traffic timing. Invitation keys never leave the native backend. Unapproved devices cannot fetch project files.</p>
        <p>Your device <code>{status.device?.device_id}</code> · Your identity <code>{status.device?.peer_id}</code>. Compare these public IDs with your collaborator before approval. The receiver waits up to 60s for approval after delivering the request; a later approval can be checked with Retry automatic pairing.</p>
        <details><summary>Legacy file pairing</summary><p>For an older build, explicitly exchange signed device cards. This grants access only after the inviter imports the card for this project.</p>
          <div className="actions"><button disabled={busy} onClick={() => request('pair')}>Open pairing reply</button>
            <button disabled={busy} onClick={() => act({ kind: 'export_card' })}>Save pairing reply</button></div></details>
      </details>
      <div className="actions">
        <button className="text-button danger" disabled={busy || !peer} onClick={() => peer && window.confirm(`Remove ${peer.device.name}'s local access to this project? Infrastructure ACLs also need an operator update.`) && act({ kind: 'revoke', project_id: project.project_id, device_id: peer.device.device_id })}>Revoke selected peer</button></div>
      <p>UUID <code>{project.project_id}</code></p><p>Mailbox <code>{project.mailbox_url ?? 'Not configured'}</code></p><p>Relay <code>{project.relay_address ?? 'Not configured'}</code></p>
      <p>Mailbox stores encrypted metadata / snapshot announcements only. It does not carry project files. A configured private relay provides a peer connection path when direct connection fails; file transfer still requires the collaborator online.</p>
      <ConnectionSetup status={status} projectId={project.project_id} busy={busy} act={act} />
    </details>
  </article>;
}

export default function App() {
  const [status, setStatus] = useState<DesktopStatus | null>(null);
  const [error, setError] = useState('');
  const [offset, setOffset] = useState(0);
  const [setup, setSetup] = useState<'link' | 'join' | null>(null);
  const [folder, setFolder] = useState('');
  const [name, setName] = useState('');
  const [uuid, setUuid] = useState('');
  const [mailbox, setMailbox] = useState('');
  const [relay, setRelay] = useState('');
  const [jobStartedAt, setJobStartedAt] = useState<Record<string, number>>({});
  const polling = useRef(false);
  const mounted = useRef(true);
  const generation = useRef(0);
  const submitted = useRef(false);
  const poll = useCallback(async () => {
    if (polling.current) return;
    polling.current = true;
    const request = generation.current;
    try { const value = await getStatus(offset); if (mounted.current && generation.current === request) { setStatus(value); setError(''); } }
    catch (err) { if (mounted.current) setError(String(err)); }
    finally { polling.current = false; }
  }, [offset]);
  useEffect(() => {
    mounted.current = true;
    generation.current += 1;
    void poll();
    const interval = window.setInterval(() => void poll(), 1500);
    return () => { mounted.current = false; generation.current += 1; window.clearInterval(interval); };
  }, [poll]);
  useEffect(() => {
    if (!status) return;
    setJobStartedAt(current => {
      const next: Record<string, number> = {};
      const now = Date.now();
      for (const job of status.jobs) {
        next[job.job_id] = current[job.job_id] ?? now;
      }
      return next;
    });
  }, [status?.jobs]);
  const act = async (action: Action) => {
    if (submitted.current) return;
    submitted.current = true;
    setError('');
    try { await submitAction(action); await poll(); }
    catch (err) { setError(String(err)); }
    finally { submitted.current = false; }
  };
  const busy = !status?.ready || status.closing || status.jobs.some(j => j.running);
  const browse = async () => { try { const value = await chooseFolder(); if (value) setFolder(value); } catch (err) { setError(String(err)); } };
  const saveSetup = async () => {
    if (setup === 'join') await act({ kind: 'join', folder });
    else await act({ kind: 'link', project_id: uuid.trim(), name: name.trim(), folder, mailbox_url: mailbox.trim() || null, relay_address: relay.trim() || null });
    setSetup(null);
  };
  return <div className="app-shell"><aside className="rail"><div className="brand-mark">TM</div><div className="rail-label">PROJECTS</div><div className="rail-bottom">PRIVATE<br />BY DESIGN</div></aside>
    <main><header className="topbar"><div><div className="eyebrow">YOUR STUDIO, CONNECTED</div><h1>TRACKMESH</h1><p>Immutable snapshots. Trusted collaborators.</p></div>
      <div className="device"><span className={`status-dot ${status?.ready ? 'active' : ''}`} />{status?.device?.name ?? 'Starting device…'}<small>{short(status?.device?.device_id)}</small></div></header>
      {(error || status?.error || status?.network?.error) && <div className="notice danger" role="alert">{error || status?.error || status?.network?.error}</div>}
      {status?.closing && <div className="notice">Work finished. TRACKMESH is shutting down safely.</div>}
      <div className="section-heading"><div><h2>Projects</h2><span className="muted">Project files travel between collaborator PCs.</span></div><div className="actions">
        <button disabled={busy} onClick={() => { setSetup('join'); setFolder(''); }}>Accept invitation</button>
        <button className="primary" disabled={busy} onClick={() => { setSetup('link'); setFolder(''); setUuid(''); setName(''); }}>+ Link folder</button></div></div>
      {setup && <section className="setup-panel"><h2>{setup === 'join' ? 'Pair with an inviter' : 'Link a local project'}</h2>
        {setup === 'link' && <><label>Project name<input value={name} maxLength={320} onChange={e => setName(e.target.value)} placeholder="Track 07" /></label>
          <label>Permanent project UUID<div className="input-row"><input value={uuid} onChange={e => setUuid(e.target.value)} placeholder="Existing UUID, or create one" /><button onClick={() => setUuid(crypto.randomUUID())}>New UUID</button></div></label></>}
        <label>Local project folder<div className="input-row"><input value={folder} onChange={e => setFolder(e.target.value)} placeholder="D:\Projects\Track 07" /><button onClick={() => void browse()}>Browse…</button></div></label>
        {setup === 'link' && <><label>HTTPS mailbox (optional)<input value={mailbox} onChange={e => setMailbox(e.target.value)} placeholder="https://mailbox.example.com" /></label><label>Pinned relay address (optional)<input value={relay} onChange={e => setRelay(e.target.value)} placeholder="/dns4/relay.example.com/udp/4001/quic-v1/p2p/…" /></label></>}
        {setup === 'join' && <p className="muted">Choose your local folder, then open the private invitation. TrackMesh sends your pairing request automatically; keep the inviter online with their pairing window open. Wait for their approval before Check peer and Receive. The invitation key stays in the native backend.</p>}
        <div className="actions"><button className="primary" disabled={busy || !folder || (setup === 'link' && (!name || !uuid))} onClick={() => void saveSetup()}>{setup === 'join' ? 'Accept invitation & connect…' : 'Link project'}</button><button onClick={() => setSetup(null)}>Cancel</button></div>
      </section>}
      <div className="project-list">{status?.page?.projects.map(card => <Card key={card.project.project_id} card={card} status={status} jobStartedAt={jobStartedAt} act={act} />)}</div>
      {status?.ready && !status.page?.projects.length && !setup && <div className="empty"><div className="empty-icon">↗</div><h2>Your first shared project</h2><p>Link a project folder or import a collaborator’s invitation.<br />Snapshots stay immutable; files stay on your PCs.</p></div>}
      <div className="pagination"><button disabled={!offset || busy} onClick={() => setOffset(Math.max(0, offset - 32))}>Previous</button><span className="muted">Page {offset / 32 + 1}</span><button disabled={!status?.page?.has_more || busy} onClick={() => setOffset(offset + 32)}>Next</button></div>
      {status?.jobs.filter(j => !j.project_id).map(job => <JobNotice key={job.job_id} job={job} startedAt={jobStartedAt[job.job_id]} />)}
      <footer><div><span className="muted">Data directory</span><code>{status?.data_directory ?? 'Opening protected local storage…'}</code></div><div className="actions"><button title="Close the window. Active DAW sessions, transfers and pending publication keep TRACKMESH running; it exits 60 seconds after work finishes. Reopen desktop to return." disabled={status?.closing} onClick={() => void closeApp().catch(err => setError(String(err)))}>Close safely</button></div></footer>
    </main></div>;
}
