import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { StorageSettings } from './StorageSettings';
import { cancelInviteLink, chooseFolder, chooseInviteParentFolder, chooseProjectsLocation, closeApp, getStatus, openInviteParentFolder, openPreviousVersion, openProjectFolder, openProjectsLocation, pasteInviteLink, setUpnpMapping, submitAction } from './api';
import type { Action, CheckView, DesktopStatus, ProjectBackend, ProjectBackendConfig, ProjectCard, SnapshotInfo, SupabaseDefaultConfig } from './api';

const short = (id: string | null | undefined) => id ? id.slice(0, 8) : '—';
const time = (value: string | null | undefined) => value ? new Date(value).toLocaleString() : 'Never';
const bytes = (n: number) => n >= 1024 ** 3 ? `${(n / 1024 ** 3).toFixed(1)} GB` : n >= 1024 ** 2 ? `${(n / 1024 ** 2).toFixed(1)} MB` : n >= 1024 ? `${(n / 1024).toFixed(1)} KB` : `${n} B`;
const CHECK_TIMEOUT_SECONDS = 30;
const displayPath = (path: string) => path.replace(/^\\\\\?\\/, '');

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

function ConnectionSetup({ status, projectId, busy, act }: { status: DesktopStatus; projectId: string; busy: boolean; act: (action: Action) => Promise<boolean> }) {
  const [ip, setIp] = useState('');
  const [port, setPort] = useState(String(status.listen_port));
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
    <p><strong>Reachability (observation)</strong>: {status.network?.reachability.state ?? 'UNVERIFIED'}{status.network?.reachability.peer ? `; peer ${status.network.reachability.peer}` : ''}{status.network?.reachability.transport ? `; ${status.network.reachability.transport}` : ''}{status.network?.reachability.at_unix_seconds ? `; ${new Date(status.network.reachability.at_unix_seconds * 1000).toLocaleString()}` : ''}</p>
    <p>A direct authorized inbound event from a public address may appear below, but reachability remains UNVERIFIED: this observation cannot prove the peer was on a different network. Same-LAN and hairpin connections are not internet reachability proof.</p>
    <p><strong>Windows Firewall startup observation (read only)</strong>: {status.firewall_observation.state}. Active profiles: {status.firewall_observation.profiles.length ? status.firewall_observation.profiles.map(profile => `${profile.name}: ${profile.enabled === null ? 'UNKNOWN' : profile.enabled ? 'enabled' : 'disabled'}, block-all-inbound ${profile.block_all_inbound === null ? 'UNKNOWN' : profile.block_all_inbound}, default inbound ${profile.default_inbound ?? 'UNKNOWN'}`).join('; ') : 'UNKNOWN'}.</p>
    <p>Candidate inbound rules for local port {status.listen_port}: TCP Allow {status.firewall_observation.tcp_rules.allow}, Block {status.firewall_observation.tcp_rules.block}; UDP Allow {status.firewall_observation.udp_rules.allow}, Block {status.firewall_observation.udp_rules.block}. Third-party firewall product registration: {status.firewall_observation.third_party_products} (absence does not rule out other packet filters). Rule candidates do not prove effective permission.</p>
    {(status.firewall_observation.tcp_rules.allow !== 'PRESENT' || status.firewall_observation.udp_rules.allow !== 'PRESENT') && <p>For ABSENT/UNKNOWN details, use <code>docs/setup/AYNI-WIFI-DENEMESI.md</code> → “Takılırsanız” → <code>Win + R → wf.msc</code>. No firewall rule is changed by this observation.</p>}
    {status.network?.port_mapping.detail && <p>{status.network.port_mapping.detail}</p>}
    <p>Local listen port: <strong>{status.listen_port}</strong> (default 43101). This session keeps its listen port; a different local port requires reopening with that startup port after the current owner exits.</p>
    <p>Local IPv4 / destination port: {localIps.length ? localIps.map(localIp => <code key={localIp}>{localIp}:{status.listen_port} </code>) : 'Waiting for a local IPv4 listen address. Do not use 0.0.0.0 as the router destination.'}</p>
    <p>Select the IP on the network connected to your router. Reserve that PC address in the router DHCP settings.</p>
    <div className="connection-inputs"><label className="field">Public IPv4 (router WAN / ISP)<input value={ip} maxLength={15} disabled={busy} onChange={e => setIp(e.target.value)} placeholder="Enter public IPv4" /></label>
      <label className="field">External router port<input type="number" min={1} max={65535} value={port} disabled={busy} onChange={e => setPort(e.target.value)} /></label></div>
    {publicIp && !valid && <p className="error">Enter a public IPv4 and a port from 1 to 65535.</p>}
    <p>CGNAT / private ranges: 100.64.0.0/10, 10/8, 172.16/12 and 192.168/16 are not public IPs. If your router WAN is in these ranges, ask your ISP for a reachable public IPv4. Entering an address does not verify reachability.</p>
    {valid && <div className="connection-addresses"><code>/ip4/{publicIp}/tcp/{port}</code><code>/ip4/{publicIp}/udp/{port}/quic-v1</code></div>}
    <p>Router port forwarding: TCP {validPort ? port : 'PORT'} → this PC, local port {status.listen_port}; UDP {validPort ? port : 'PORT'} → this PC, local port {status.listen_port}. The installer already allows TrackMesh inbound in Windows Firewall; router rules are not changed by this panel.</p>
    <p>Keep TrackMesh open, configure forwarding and firewall, then export. The generated public endpoints are included in this invitation only; these fields are not saved or advertised by the running swarm. Re-export and privately exchange a new invitation if the public IP changes.</p>
    <button disabled={busy || !valid} onClick={() => act({ kind: 'export_invite', project_id: projectId, public_ip: publicIp, external_port: Number(port) })}>Save invitation file with public addresses</button>
    <p>Invite links expire after 15 minutes and can be opened once. The Windows clipboard is marked to exclude history and cloud sync, then cleared after 60 seconds if the link is still there. Share it privately. Invitation files remain available.</p>
    <details><summary>Current advertised / listen addresses</summary>{addresses.map(address => <code className="connection-address" key={address}>{address}</code>)}
      {status.configured_external_addresses.map(address => <code className="connection-address" key={`external-${address}`}>{address} (startup external address)</code>)}</details>
  </details>;
}

function Snapshot({ value, empty, mailboxEnabled }: { value: SnapshotInfo | null; empty: string; mailboxEnabled: boolean }) {
  return value ? <><div className="snapshot-id">{short(value.snapshot_id)} {mailboxEnabled && <span>{value.metadata_status}</span>}</div>
    <p>{value.author_name} · {time(value.created_at)}</p>
    <p>{value.file_count} files · {bytes(value.total_size)} · {bytes(value.changed_bytes)} changed</p></>
    : <p className="muted">{empty}</p>;
}

function DefaultSupabaseSettings({ config, busy, act }: {
  config: SupabaseDefaultConfig | null; busy: boolean; act: (action: Action) => Promise<boolean>;
}) {
  const [url, setUrl] = useState(config?.url ?? '');
  const [publishableKey, setPublishableKey] = useState(config?.publishable_key ?? '');
  useEffect(() => {
    setUrl(config?.url ?? '');
    setPublishableKey(config?.publishable_key ?? '');
  }, [config?.url, config?.publishable_key]);
  return <details className="setup"><summary>Default Supabase service</summary>
    <p className="muted">Saved in this app's settings.json, outside project settings. Used for invite links and prefilled when selecting Supabase for a project.</p>
    <div className="field-stack"><label className="field">Supabase Project URL<input autoComplete="off" spellCheck={false} value={url} disabled={busy} placeholder="https://your-project.supabase.co" onChange={event => setUrl(event.target.value)} /></label>
    <label className="field">Publishable key<input type="password" autoComplete="new-password" spellCheck={false} value={publishableKey} disabled={busy} onChange={event => setPublishableKey(event.target.value)} /></label>
    <div className="actions"><button disabled={busy || !url.trim() || !publishableKey.trim()} onClick={() => void act({ kind: 'set_default_supabase', url: url.trim(), publishable_key: publishableKey.trim() })}>Save default</button>
      <button disabled={busy || !config} onClick={() => void act({ kind: 'set_default_supabase', url: null, publishable_key: null })}>Clear</button></div>
    </div>
  </details>;
}

function ProjectBackendSettings({ projectId, config, defaultSupabase, state, revokeProblem, missingFunctions, busy, act }: {
  projectId: string; config: ProjectBackendConfig | undefined; busy: boolean;
  defaultSupabase: SupabaseDefaultConfig | null;
  state: DesktopStatus['supabase_states'][string];
  revokeProblem: boolean; missingFunctions: string[];
  act: (action: Action) => Promise<boolean>;
}) {
  const [backend, setBackend] = useState<ProjectBackend>(config?.backend ?? 'NONE');
  const [url, setUrl] = useState((config?.backend === 'SUPABASE' ? config.supabase_url : null) ?? defaultSupabase?.url ?? '');
  const [anonKey, setAnonKey] = useState((config?.backend === 'SUPABASE' ? config.supabase_anon_key : null) ?? defaultSupabase?.publishable_key ?? '');
  useEffect(() => {
    if (config?.backend !== 'SUPABASE') {
      setUrl(defaultSupabase?.url ?? '');
      setAnonKey(defaultSupabase?.publishable_key ?? '');
    }
  }, [config?.backend, config?.supabase_url, config?.supabase_anon_key, defaultSupabase?.url, defaultSupabase?.publishable_key]);
  return <section className="backend-settings">
    <h3>ENDPOINT BACKEND</h3>
    <label>Backend<select value={backend} disabled={busy} onChange={event => setBackend(event.target.value as ProjectBackend)}>
      <option value="NONE">NONE · direct connections</option>
      <option value="SELF_HOSTED_MAILBOX">SELF_HOSTED_MAILBOX · existing mailbox URL</option>
      <option value="SUPABASE">SUPABASE</option>
    </select></label>
    {backend === 'SELF_HOSTED_MAILBOX' && <p className="muted">Uses this project's existing mailbox URL without changing its behavior.</p>}
    {backend === 'SUPABASE' && <div className="backend-fields">
      <label className="field">Supabase Project URL<input autoComplete="off" spellCheck={false} value={url} disabled={busy} placeholder="https://your-project.supabase.co" onChange={event => setUrl(event.target.value)} /></label>
      <label className="field">Publishable key<input type="password" autoComplete="new-password" spellCheck={false} value={anonKey} disabled={busy} onChange={event => setAnonKey(event.target.value)} /></label>
      <p className="muted">HTTPS `*.supabase.co` URLs only. This key selects the public API project; membership still requires device signing. Never enter a service role or secret key.</p>
    </div>}
    <div className="actions"><button disabled={busy || (backend === 'SUPABASE' && (!url.trim() || !anonKey.trim()))} onClick={async () => {
      await act({ kind: 'set_project_backend', project_id: projectId, backend,
        supabase_url: backend === 'SUPABASE' ? url.trim() : null,
        supabase_anon_key: backend === 'SUPABASE' ? anonKey.trim() : null });
    }}>Save backend</button><span className="badge">{backend === 'SELF_HOSTED_MAILBOX' ? 'SELF_HOSTED' : state}</span></div>
    {revokeProblem && <p className="muted">The invite service didn't remove a device. Only the device that created the project can remove others; it may also have been removed already.</p>}
    {missingFunctions.length > 0 && <p className="muted">The invite service is missing these functions: {missingFunctions.join(', ')}. Deploy them as described in services/supabase/README.md.</p>}
  </section>;
}

function jobStatus(job: DesktopStatus['jobs'][number], startedAt: number | undefined, collaboratorName?: string) {
  if (!job.running) {
    return job.error ? `${job.label}: ${job.error}`
      : job.label === "Use collaborator's version" && typeof job.outcome?.transfer_id === 'string' ? `Now using ${collaboratorName ?? 'collaborator'}'s version ✓`
      : job.label === "Get collaborator's version" && typeof job.outcome?.snapshot_id === 'string' ? `Received ${collaboratorName ?? 'collaborator'}'s version ✓`
      : job.outcome?.publication_error ? `Ready — ${collaboratorName ?? 'your collaborator'} can now get this version.`
      : typeof job.outcome?.snapshot_id === 'string' ? `Ready — ${collaboratorName ?? 'your collaborator'} can now get this version.`
      : job.outcome?.message ? String(job.outcome.message) : `${job.label} completed`;
  }
  if (job.label === 'Share current version') return 'Preparing current version…';
  if (job.label !== 'Check connection') return `${job.label}…`;
  const elapsed = startedAt ? Math.floor((Date.now() - startedAt) / 1000) : 0;
  const remaining = Math.max(0, CHECK_TIMEOUT_SECONDS - elapsed);
  return `Check connection… trying to reach collaborator for up to ${CHECK_TIMEOUT_SECONDS}s (${remaining}s left)`;
}

function InviteCopiedNotice({ job }: { job: DesktopStatus['jobs'][number] }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const interval = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(interval);
  }, []);
  const remaining = Math.max(0, 60 - Math.floor((now - (job.completed_at ?? Math.floor(now / 1000)) * 1000) / 1000));
  if (!remaining) return <div className="job" role="status">Clipboard cleared. Copy the invite link again if it was not pasted.</div>;
  return <div className="job" role="status">Invite link copied — paste it within 60 s <strong>({remaining}s left)</strong></div>;
}

function JobNotice({ job, startedAt, collaboratorName }: { job: DesktopStatus['jobs'][number]; startedAt: number | undefined; collaboratorName?: string }) {
  const warning = job.error || job.outcome?.pairing_state === 'retry';
  return <div className={`job ${warning ? 'danger' : ''}`} role="status">
    {jobStatus(job, startedAt, collaboratorName)}
    {typeof job.outcome?.path === 'string' && <p>Saved file: <code>{displayPath(job.outcome.path)}</code></p>}
  </div>;
}

function InProgress({ children }: { children: ReactNode }) {
  return <div className="in-progress" role="status"><span>{children}</span></div>;
}

function ConnectionAssistant({ card, status, peer, check, busy, act }: {
  card: ProjectCard; status: DesktopStatus; peer: ProjectCard['peers'][number] | undefined;
  check: CheckView | undefined; busy: boolean; act: (action: Action) => Promise<boolean>;
}) {
  const projectId = card.project.project_id;
  const candidates = status.pairing.project_id === projectId ? status.pairing.requests : [];
  const connection = peer ? status.network?.peers[peer.device.peer_id] : undefined;
  const checkJob = status.jobs.find(job => job.project_id === projectId && job.label === 'Check connection');
  const localIps = localIpv4Addresses(status);
  const runtimeError = status.error ?? status.network?.error;
  const [expanded, setExpanded] = useState(!connection?.online || !!runtimeError);
  let next = 'Invite your collaborator with Invite collaborator; they open the link.';
  if (runtimeError) next = 'The local network session has an error. Reopen TrackMesh after current work finishes.';
  else if (candidates.length) next = 'Compare the pending Device and Identity with the other PC, then approve the expected device in Pair collaborator.';
  else if (!peer && status.pairing.project_id === projectId) next = 'The pairing window is open. Keep TrackMesh running while the other PC accepts the private invitation.';
  else if (peer && checkJob?.running) next = 'Trying the selected collaborator now. This can take up to 30 seconds.';
  else if (peer && checkJob?.error) next = connection?.online
    ? 'The peer connection exists, but the project check failed. Read the error below and confirm the sender approved this device for this project.'
    : 'The connection check failed. Keep both PCs open, compare the sender’s current Wi-Fi IP with the invitation, then check guest isolation and inbound Windows Firewall on the sender. A timeout cannot identify which one blocked traffic.';
  else if (peer && !connection?.online) next = 'Use Check connection when both PCs are open. If approval is pending, the sender can reopen the approval window and you can press Try again.';
  else if (peer && !check?.fresh) next = 'The peer is connected. Use Check connection to verify project access and compare current snapshots.';
  else if (card.project.recovery_required) next = 'Local installation needs recovery. Close the DAW project and use Recover locally before another Receive.';
  else if (check?.fresh && check.result.snapshot_status === 'DIVERGED') next = 'Both versions changed. Choose which version to keep in the resolution box below; TrackMesh does not merge projects.';
  else if (check?.fresh && check.result.snapshot_status === 'UP_TO_DATE') next = 'Both snapshot heads are equal on this live connection.';
  else if (check?.fresh && check.result.snapshot_status === 'LOCAL_AHEAD') next = 'Your local snapshot is ahead. The other PC should use Get collaborator\'s version.';
  else if (check?.fresh && check.result.snapshot_status === 'UNKNOWN') next = 'Snapshot ancestry is unknown. Review the current project warning before receiving.';
  else if (check?.result.remote_snapshot) next = 'A source snapshot is available. Close the DAW project, then use Get collaborator\'s version when the button is enabled.';
  else if (card.local_snapshot) next = 'Your local snapshot is ready. The other PC should use Get collaborator\'s version.';
  else if (peer) next = 'No version yet. Share current version on the PC with the project files.';

  return <details className="connection-assistant" open={expanded} onToggle={event => setExpanded(event.currentTarget.open)}>
    <summary>Connection check · one-click peer check and next step</summary>
    <div className="diagnostic-grid">
      <p><strong>Local app:</strong> {runtimeError ? `Network error: ${runtimeError}` : status.ready ? 'Network session running' : 'Starting'}</p>
      <p><strong>Local address:</strong> {localIps.length ? localIps.map(ip => `${ip}:${status.listen_port}`).join(', ') : `No Wi-Fi IPv4 address shown yet · listen port ${status.listen_port}`}</p>
      <p><strong>Device:</strong> {candidates.length ? `${candidates.length} request awaiting your approval` : peer ? `${peer.device.name} recorded locally; remote approval is verified by Check connection` : status.pairing.project_id === projectId ? 'Pairing window open; waiting for the other PC' : 'No collaborator recorded yet'}</p>
      <p><strong>Connection:</strong> {connection?.online ? `${connection.route} · ${connection.transport}` : checkJob?.running ? 'Checking now' : checkJob?.error ? 'Last check failed' : 'No active peer connection'}</p>
      <p><strong>Peer status:</strong> {peer ? status.peer_states[peer.device.peer_id] ?? 'UNKNOWN' : 'UNKNOWN'}</p>
      <p><strong>Snapshot status:</strong> {check?.fresh ? check.result.snapshot_status.replaceAll('_', ' ') : 'No live comparison · mailbox metadata is last-known only'}</p>
      <p><strong>Port status:</strong> {status.port_status === 'UNVERIFIED' ? 'PORT_UNVERIFIED' : 'UNKNOWN'}</p>
    </div>
    <p className="diagnostic-next"><strong>Next:</strong> {next}</p>
    {peer && <div className="actions"><button disabled={busy} onClick={() => act({ kind: 'check', project_id: projectId, device_id: peer.device.device_id })}>{checkJob?.running ? 'Checking…' : 'Check connection'}</button></div>}
    <p className="muted">This checks the app’s own session and selected peer. Windows Firewall, guest Wi-Fi isolation and the other PC’s current IP cannot be identified separately from a connection timeout.</p>
  </details>;
}

function Card({ card, status, jobStartedAt, focusProjectId, act }: { card: ProjectCard; status: DesktopStatus; jobStartedAt: Record<string, number>; focusProjectId: string | null; act: (action: Action) => Promise<boolean> }) {
  const [selected, setSelected] = useState('');
  const [dawClosed, setDawClosed] = useState(false);
  const [folderError, setFolderError] = useState('');
  const [keepMinePending, setKeepMinePending] = useState(false);
  const project = card.project;
  const peer = card.peers.find(p => p.device.device_id === selected) ?? card.peers[0];
  const peerId = peer?.device.device_id;
  const pairingApproval = peer ? status.pairing_approvals.find(item => item.project_id === project.project_id && item.device_id === peer.device.device_id)?.state ?? 'UNKNOWN' : 'UNKNOWN';
  const connection = peer ? status.network?.peers[peer.device.peer_id] : undefined;
  const online = connection?.online ?? false;
  const job = status.jobs.find(j => j.project_id === project.project_id);
  const busy = !status.ready || status.closing || status.jobs.some(j => j.running && (!j.project_id || j.project_id === project.project_id));
  const check = status.checks.find(c => c.result.project_id === project.project_id && c.device_id === peerId);
  const snapshotStatus = check?.fresh ? check.result.snapshot_status : null;
  const divergence = snapshotStatus === 'DIVERGED' || peer?.known_lineage === 'DIVERGED';
  const blockedJob = status.jobs.find(item => item.project_id === project.project_id && !!item.outcome?.receive_blocked);
  const blockedCode = blockedJob?.outcome?.receive_blocked;
  const resolveDivergence = divergence || blockedCode === 'DIVERGED';
  const resolveLocalChanges = blockedCode === 'LOCAL_CHANGES';
  const resolutionJob = status.jobs.find(item => item.project_id === project.project_id && item.label === "Use collaborator's version");
  const usedTransferId = resolutionJob && !resolutionJob.running && !resolutionJob.error && typeof resolutionJob.outcome?.transfer_id === 'string' ? String(resolutionJob.outcome.transfer_id) : null;
  const firstShareBlocked = !card.local_snapshot && (card.root_empty || card.peers.some(item => !!item.announcement || !!item.offline_head || !!item.known_lineage));
  const remoteAhead = online && peer?.known_lineage === 'REMOTE_AHEAD';
  const offlineHead = peer?.offline_head;
  const showOfflineHead = !!offlineHead && !!peer && !online && !remoteAhead
    && (!peer.announcement || Date.parse(offlineHead.created_at) > Date.parse(peer.announcement.created_at));
  const mappingState = status.network?.port_mapping.state;
  const transfer = card.transfer;
  const resumable = transfer?.state === 'PAUSED' && peerId === transfer.source_device_id;
  const receiving = transfer?.state === 'RECEIVING' || transfer?.state === 'APPLYING';
  const authoredHere = card.local_snapshot?.author_device_id === status.device?.device_id;
  const receipt = !!authoredHere && !!peer && !!card.local_snapshot && (
    (project.verified_peer_id === peer.device.peer_id && project.verified_snapshot_id === card.local_snapshot.snapshot_id)
    || !!status.network?.received_snapshots.some(item => item.project_id === project.project_id && item.peer_id === peer.device.peer_id && item.snapshot_id === card.local_snapshot?.snapshot_id)
  );
  const sharedLatest = !!card.local_snapshot && status.jobs.some(item => item.project_id === project.project_id && item.outcome?.snapshot_id === card.local_snapshot?.snapshot_id);
  const waitingFirstVersion = !!peer && card.root_empty && !card.local_snapshot && !project.latest_local_snapshot
    && !project.recovery_required && !peer.announcement
    && (!transfer || transfer.state === 'COMPLETE')
    && !status.jobs.some(item => item.project_id === project.project_id && item.running)
    && pairingApproval !== 'PENDING' && pairingApproval !== 'NOT_REACHABLE';
  const disabledReceive = busy || !peer || !online || !dawClosed || project.recovery_required || divergence;
  const rootMissing = !card.root_available;
  const chooseProjectRoot = async () => {
    try {
      const folder = await chooseFolder();
      if (folder) await act({ kind: 'change_project_root', project_id: project.project_id, folder });
    } catch (err) { setFolderError(String(err)); }
  };
  const openProject = async () => {
    try { await openProjectFolder(project.project_id); }
    catch (err) { setFolderError(String(err)); }
  };
  const request = (kind: 'send' | 'publish' | 'refresh' | 'pair' | 'export_invite' | 'copy_invite_link') => act({ kind, project_id: project.project_id });
  const candidates = status.pairing.project_id === project.project_id ? status.pairing.requests : [];
  const inviteState = status.supabase_states[project.project_id] ?? 'NOT_CONFIGURED';
  const inviteReady = status.project_backends[project.project_id]?.backend === 'SUPABASE' && ['CLAIMED', 'MEMBER'].includes(inviteState);
  const projectStatus = pairingApproval === 'PENDING' ? 'Waiting for approval' : online ? `Connected: ${peer?.device.name}` : peer ? 'Collaborator offline' : 'Waiting for a collaborator';
  const backendMessage = inviteState === 'UNREACHABLE'
    ? ''
    : inviteState === 'WAITING_FOR_GRANT'
      ? "Waiting for your collaborator's TrackMesh to confirm this device with the invite service. Keep both apps open."
      : status.project_backends[project.project_id]?.backend === 'SUPABASE' && inviteState === 'UNKNOWN'
        ? 'Connecting to the invite service. Try again in a moment.'
      : !status.default_supabase
        ? 'Set a default Supabase service in Settings to create invite links.'
        : 'Configure Supabase for this project in Advanced settings before creating an invite link.';
  const inviteJob = status.jobs.find(item => item.project_id === project.project_id && item.label === 'Copy invite link');
  const copyInviteSuccess = !!inviteJob && !inviteJob.running && !inviteJob.error && inviteJob.outcome?.invite_copied === true;
  return <article id={`project-${project.project_id}`} className={`project-card${focusProjectId === project.project_id ? ' focus-highlight' : ''}`}>
    <header className="card-title"><div><h2>{project.name}</h2><button className="path-link" title="Open in Explorer" onClick={() => void openProject()}>{displayPath(project.root)}</button></div>
      <span className={`badge ${online ? 'online' : ''}`}>{projectStatus}</span></header>
    {status.presence[project.project_id]?.map(warning => <p key={warning.device_id} className={`notice ${warning.both_working ? 'danger' : ''}`} role="status">{warning.line}{!warning.both_working && ` (since ${new Date(warning.since * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })})`}</p>)}
    {rootMissing && <section className="notice danger"><strong>Project folder not found</strong><div className="actions"><button className="primary" disabled={busy} onClick={() => void chooseProjectRoot()}>Locate folder</button></div></section>}
    {status.plugin_handoff?.project_id === project.project_id && <p className="notice" role="status">Save and close this project in your DAW, then press Get collaborator's version.</p>}
    {folderError && <p className="error">{folderError}</p>}
    {candidates.length > 0 && <section className="approval-box"><h3>Approve collaborator</h3>{candidates.map(candidate => <div key={candidate.request_id}>
      <p><strong>{candidate.name}</strong> · device {short(candidate.device_id)}</p>
      <p className="muted">Confirm this is the collaborator you invited before granting project access.</p>
      <div className="actions"><button className="primary" disabled={busy} onClick={() => window.confirm(`Approve ${candidate.name} (${short(candidate.device_id)}) for ${project.name}?`) && act({ kind: 'approve_pairing', project_id: project.project_id, request_id: candidate.request_id })}>Approve</button>
        <button disabled={busy} onClick={() => act({ kind: 'reject_pairing', project_id: project.project_id, request_id: candidate.request_id })}>Reject</button></div></div>)}</section>}
    <section className="invite-flow"><button className={peer ? '' : 'primary'} disabled={busy || !inviteReady} onClick={() => request('copy_invite_link')}>Invite collaborator</button>
      <p className="muted">The link is valid for 15 minutes and can be opened once.</p>{!inviteReady && backendMessage && <p className="error">{backendMessage}</p>}
      {status.pairing.project_id !== project.project_id && (!peer || !!inviteJob) && <><button className="text-button" disabled={busy} onClick={() => act({ kind: 'open_pairing', project_id: project.project_id })}>Reopen approval window</button><p className="muted">Use this if your collaborator opened the link but approval timed out.</p></>}</section>
    {peer && <div className="peer-line"><label>Collaborator <select value={peerId ?? ''} disabled={busy || !card.peers.length} onChange={e => { setSelected(e.target.value); setDawClosed(false); }}>
      {card.peers.map(p => <option key={p.device.device_id} value={p.device.device_id}>{p.device.name}</option>)}</select></label>
      <button className="text-button danger" disabled={busy} onClick={() => window.confirm(`Remove ${peer.device.name}'s access to this project? They keep the files they already received.`) && act({ kind: 'revoke', project_id: project.project_id, device_id: peer.device.device_id })}>Revoke</button>
      <button className="text-button" disabled={busy} onClick={() => act({ kind: 'check', project_id: project.project_id, device_id: peer.device.device_id })}>Check connection</button></div>}
    {card.grant_problem === 'REJECTED' && peer && <p className="muted">The invite service didn't accept {peer.device.name} (the project may already have 3 devices, or too many requests). Direct sharing still works.</p>}
    {project.recovery_required && <div className="notice danger">Close the DAW and recover this project before receiving another version.</div>}
    {remoteAhead && peer?.announcement && <div className="new-version-notice" role="status">New version from {peer.device.name} — {time(peer.announcement.created_at)}</div>}
    {showOfflineHead && offlineHead && peer && <div className="new-version-notice" role="status">New version from {peer.device.name} — {time(offlineHead.created_at)} · available when {peer.device.name} is online</div>}
    {peer && status.network?.port_mapping.consent_enabled && mappingState === 'MAPPED' && <p className="muted">Router set up ✓</p>}
    {peer && status.network?.port_mapping.consent_enabled && mappingState === 'FAILED' && <p className="muted">Router didn't allow automatic setup — connection may still work</p>}
    {peer && (resolveDivergence || resolveLocalChanges) && <section className="notice resolution-box" role="region" aria-label="Resolve project versions">
      <h3>{resolveLocalChanges && !resolveDivergence ? "This folder has changes you haven't shared" : `Your version and ${peer.device.name}'s version both changed`}</h3>
      <p>TrackMesh does not merge projects. Choose which version to keep.</p>
      {(!online || !dawClosed) && <p className="muted">{!online ? `Open TrackMesh on both PCs to use ${peer.device.name}'s version.` : 'Close the project in your DAW and tick “I closed this project in the DAW” below.'}</p>}
      <div className="actions">
        <button className="primary" disabled={busy || !online || !dawClosed} onClick={() => window.confirm(`Replace this folder with ${peer.device.name}'s version? Your current files are saved first and kept next to the project.`) && act({ kind: 'use_peer_version', project_id: project.project_id, device_id: peer.device.device_id, daw_closed: dawClosed })}>Use {peer.device.name}'s version</button>
        <button disabled={busy || rootMissing || project.recovery_required} onClick={() => { setKeepMinePending(true); void act({ kind: 'send', project_id: project.project_id }); }}>Keep mine</button>
      </div>
      {keepMinePending && job?.label === 'Share current version' && !job.running && !job.error && <p role="status">Ask {peer.device.name} to choose ‘Use {status.device?.name ?? 'your device'}'s version’.</p>}
    </section>}
    {peer && usedTransferId && <div className="delivery-receipt" role="status">Now using {peer.device.name}'s version ✓ <button className="text-button" onClick={() => void openPreviousVersion(project.project_id, usedTransferId).catch(error => setFolderError(String(error)))}>Open my previous files</button></div>}
    {peer && pairingApproval !== 'PENDING' && <section className="share-flow"><div><h3>Share current version</h3><p className="muted">Save a version here for your collaborator to receive.</p>
      {firstShareBlocked
        ? <p className="muted">{card.root_empty && !card.peers.some(item => !!item.announcement || !!item.offline_head || !!item.known_lineage) ? 'Add your project files to this folder first.' : `Get ${peer.device.name}'s version first.`}</p>
        : <button className={remoteAhead ? '' : 'primary'} disabled={busy || rootMissing || project.recovery_required} onClick={() => request('send')}>Share current version</button>}</div>
      <div><h3>Get collaborator's version</h3><p className="muted">Receive copies the selected collaborator's snapshot into this folder.</p>
        {status.presence[project.project_id]?.filter(warning => warning.device_id === peer.device.device_id).map(warning => <p key={warning.device_id} className="muted" role="status">{warning.get_line}</p>)}
        <label className="confirmation"><input type="checkbox" checked={dawClosed} disabled={busy} onChange={e => setDawClosed(e.target.checked)} /> I closed this project in the DAW</label>
        <button className={remoteAhead ? 'primary' : ''} disabled={disabledReceive || rootMissing} onClick={() => act({ kind: 'receive', project_id: project.project_id, device_id: peer.device.device_id, snapshot_id: resumable ? transfer.snapshot_id : null, daw_closed: dawClosed })}>{resumable ? 'Resume receive' : "Get collaborator's version"}</button>
        {!online && <p className="muted">Open TrackMesh on both PCs to receive files.</p>}</div></section>}
    {authoredHere && sharedLatest && peer && card.local_snapshot && !receipt && <InProgress>Waiting for {peer.device.name} to get this version…</InProgress>}
    {!receiving && pairingApproval === 'PENDING' && peer && <InProgress>Waiting for {peer.device.name} to approve this device…</InProgress>}
    {!receiving && pairingApproval === 'NOT_REACHABLE' && peer && <section className="notice" role="status"><p>{peer.device.name} hasn't approved this device yet. Ask them to open TrackMesh; if approval timed out, they can press ‘Reopen approval window’. Then press Try again.</p><button disabled={busy} onClick={() => act({ kind: 'request_pairing', project_id: project.project_id, device_id: peer.device.device_id })}>Try again</button></section>}
    {waitingFirstVersion && peer && <InProgress>Waiting for {peer.device.name} to share the first version…</InProgress>}
    {receipt && peer && <div className="delivery-receipt" role="status">Received by {peer.device.name} ✓</div>}
    {transfer && <div className="transfer"><div><span>Transfer {transfer.state.replaceAll('_', ' ')}</span><span>{bytes(transfer.completed_bytes)} / {bytes(transfer.total_bytes)}{transfer.total_bytes ? ` · ${Math.min(100, Math.round(transfer.completed_bytes * 100 / transfer.total_bytes))} %` : ''}</span></div>
      <progress max={Math.max(transfer.total_bytes, 1)} value={transfer.completed_bytes} />
      {transfer.error && <p className="error">{transfer.error}</p>}{receiving && <InProgress>Receiving project… {transfer.total_bytes ? Math.min(100, Math.round(transfer.completed_bytes * 100 / transfer.total_bytes)) : 0} %</InProgress>}</div>}
    {inviteJob?.running && <InProgress>Creating invite link…</InProgress>}
    {copyInviteSuccess && inviteJob && <InviteCopiedNotice job={inviteJob} />}
    {copyInviteSuccess && typeof inviteJob?.outcome?.cleared_pending === 'string' && <div className="job" role="status">Pending approvals for {inviteJob.outcome.cleared_pending} were cleared; only one project can accept new devices at a time.</div>}
    {job && !(job.label === 'Copy invite link' && (job.running || copyInviteSuccess)) && <JobNotice job={job} startedAt={jobStartedAt[job.job_id]} collaboratorName={peer?.device.name} />}
    <details className="setup advanced-card"><summary>Advanced</summary>
      <section className="advanced-group"><h3>Connection details</h3>
      <div className="snapshots"><section><h3>LOCAL SNAPSHOT</h3><Snapshot value={card.local_snapshot} empty="No local snapshot yet." mailboxEnabled={card.mailbox_enabled} /></section>
        <section><h3>KNOWN REMOTE METADATA</h3><Snapshot value={peer?.announcement ?? null} empty="No remote snapshot metadata yet." mailboxEnabled={card.mailbox_enabled} />{peer && peer.history.length > 1 && <details><summary>{peer.history.length} recent snapshots</summary><ol>{peer.history.map(snapshot => <li key={snapshot.snapshot_id}><code>{short(snapshot.snapshot_id)}</code> · {time(snapshot.created_at)}</li>)}</ol></details>}</section></div>
      <div className={`verification ${divergence ? 'danger' : ''}`}>{check?.fresh ? `Current comparison: ${snapshotStatus?.replaceAll('_', ' ') ?? 'not available'}` : 'No current comparison for this collaborator'}</div>
      <ConnectionAssistant card={card} status={status} peer={peer} check={check} busy={busy} act={act} />
      {project.recovery_required && <button disabled={busy || !dawClosed} onClick={() => act({ kind: 'recover', project_id: project.project_id, daw_closed: dawClosed })}>Recover locally</button>}
      <ProjectBackendSettings projectId={project.project_id} config={status.project_backends[project.project_id]} defaultSupabase={status.default_supabase} state={status.supabase_states[project.project_id] ?? 'NOT_CONFIGURED'} revokeProblem={card.revoke_problem} missingFunctions={card.missing_functions} busy={busy} act={act} />
      {(card.grant_problem === 'UNREACHABLE' || inviteState === 'UNREACHABLE') && <p className="muted">Invite service is unreachable. Check its URL and network connection in Advanced settings.</p>}
      <p>UUID <code>{project.project_id}</code></p><p>Mailbox <code>{project.mailbox_url ?? 'Not configured'}</code></p><p>Relay <code>{project.relay_address ?? 'Not configured'}</code></p>
      {card.mailbox_enabled
        ? <p>Mailbox stores encrypted metadata and snapshot announcements only. The relay field is a legacy setting outside the supported setup.</p>
        : <p className="muted">Version notices use the invite service; the legacy mailbox is off for this project.</p>}
      <button disabled={busy} onClick={() => void chooseProjectRoot()}>{rootMissing ? 'Locate folder' : 'Change folder'}</button>
      {peer && <p>Device ID <code>{peer.device.device_id}</code> · Identity <code>{peer.device.peer_id}</code></p>}
      </section>
      <section className="advanced-group"><h3>Manual invitation (without invite service)</h3>
      <details className="pairing-setup"><summary>Collaboration & connection details</summary>
        <ol>
          <li><strong>Send an invitation.</strong> Use a private invitation file. Export opens a temporary pairing window.</li>
          <li><strong>Your collaborator accepts it.</strong> Their signed device identity and invitation proof arrive automatically over an encrypted connection. They do not send a card file back.</li>
          <li><strong>Verify and approve their device here.</strong> Confirm the expected device identity. Approval grants project access.</li>
        </ol>
        <div className="actions"><button disabled={busy} onClick={() => request('export_invite')}>Export invitation with current addresses</button>
          <button disabled={busy || status.pairing.project_id === project.project_id} onClick={() => act({ kind: 'open_pairing', project_id: project.project_id })}>Open pairing window</button>
          <button disabled={busy || !peer} onClick={() => peer && act({ kind: 'request_pairing', project_id: project.project_id, device_id: peer.device.device_id })}>Retry automatic pairing</button></div>
        {status.pairing.project_id === project.project_id && <div role="status">
          <p>Waiting for pairing requests. One project window is open at a time; closing, expiry or restart removes unapproved requests.</p>
          <button disabled={busy} onClick={() => act({ kind: 'close_pairing', project_id: project.project_id })}>Close pairing window</button>
        </div>}
        <p>Encryption protects message contents, not IP addresses or traffic timing. Invitation keys stay in the native backend. Unapproved devices cannot fetch project files.</p>
        <details><summary>Legacy file pairing</summary><p>For an older build, explicitly exchange signed device cards. This grants access only after the inviter imports the card for this project.</p>
          <div className="actions"><button disabled={busy} onClick={() => request('pair')}>Open pairing reply</button>
            <button disabled={busy} onClick={() => act({ kind: 'export_card' })}>Save pairing reply</button></div></details>
      </details>
      <ConnectionSetup status={status} projectId={project.project_id} busy={busy} act={act} />
      </section>
    </details>
  </article>;
}

export default function App() {
  const [status, setStatus] = useState<DesktopStatus | null>(null);
  const [error, setError] = useState('');
  const [offset, setOffset] = useState(0);
  const [setup, setSetup] = useState<'link' | 'join' | null>(null);
  const [screen, setScreen] = useState<'projects' | 'settings'>('projects');
  const [folder, setFolder] = useState('');
  const [name, setName] = useState('');
  const [autoName, setAutoName] = useState(false);
  const [uuid, setUuid] = useState('');
  const [mailbox, setMailbox] = useState('');
  const [relay, setRelay] = useState('');
  const [inviteServiceUrl, setInviteServiceUrl] = useState('');
  const [inviteServiceKey, setInviteServiceKey] = useState('');
  const [inviteParentFolder, setInviteParentFolder] = useState<string | null>(null);
  const [projectsLocation, setProjectsLocation] = useState('');
  const [jobStartedAt, setJobStartedAt] = useState<Record<string, number>>({});
  const [focusProjectId, setFocusProjectId] = useState<string | null>(null);
  const [scrolledFocusId, setScrolledFocusId] = useState<string | null>(null);
  const pluginHandoffRevision = useRef(0);
  useEffect(() => {
    const handoff = status?.plugin_handoff;
    if (!handoff || handoff.revision === pluginHandoffRevision.current) return;
    pluginHandoffRevision.current = handoff.revision;
    focusBaseline.current = null;
    setScreen('projects');
    setSetup(null);
    setOffset(handoff.offset);
    setScrolledFocusId(null);
    setFocusProjectId(handoff.project_id);
  }, [status?.plugin_handoff]);
  const [upnpConsentPrompt, setUpnpConsentPrompt] = useState(false);
  const pendingConsentAction = useRef<Action | null>(null);
  const polling = useRef(false);
  const mounted = useRef(true);
  const generation = useRef(0);
  const submitted = useRef(false);
  const focusBaseline = useRef<Set<string> | null>(null);
  const poll = useCallback(async () => {
    if (polling.current) return;
    polling.current = true;
    const request = generation.current;
    try { const value = await getStatus(offset); if (mounted.current && generation.current === request) { setStatus(value); } }
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
  useEffect(() => {
    if (!error) return;
    const timeout = window.setTimeout(() => setError(''), 8000);
    return () => window.clearTimeout(timeout);
  }, [error]);
  useEffect(() => {
    if (!status?.page || !focusBaseline.current) return;
    const added = status.page.projects.find(item => !focusBaseline.current?.has(item.project.project_id));
    if (added) {
      focusBaseline.current = null;
      setScrolledFocusId(null);
      setFocusProjectId(added.project.project_id);
    }
  }, [status?.page]);
  // Scroll once per focus request; status polling must not re-scroll or restart the highlight timer.
  useEffect(() => {
    if (!focusProjectId || scrolledFocusId === focusProjectId) return;
    const frame = window.requestAnimationFrame(() => {
      const card = document.getElementById(`project-${focusProjectId}`);
      if (!card) return;
      card.scrollIntoView({ behavior: 'smooth', block: 'center' });
      setScrolledFocusId(focusProjectId);
    });
    return () => window.cancelAnimationFrame(frame);
  }, [focusProjectId, scrolledFocusId, status?.page]);
  useEffect(() => {
    if (!scrolledFocusId) return;
    const timeout = window.setTimeout(() => { setFocusProjectId(null); setScrolledFocusId(null); }, 2500);
    return () => window.clearTimeout(timeout);
  }, [scrolledFocusId]);
  const executeAction = async (action: Action): Promise<boolean> => {
    if (submitted.current) return false;
    submitted.current = true;
    setError('');
    try { await submitAction(action); await poll(); return true; }
    catch (err) { setError(String(err)); return false; }
    finally { submitted.current = false; }
  };
  const act = async (action: Action): Promise<boolean> => {
    const asksForConsent = action.kind === 'copy_invite_link' || action.kind === 'join' || action.kind === 'complete_invite_link';
    if (asksForConsent && status && !status.upnp_mapping_asked) {
      pendingConsentAction.current = action;
      setUpnpConsentPrompt(true);
      return true;
    }
    return executeAction(action);
  };
  const answerUpnpConsent = async (allow: boolean) => {
    const action = pendingConsentAction.current;
    pendingConsentAction.current = null;
    setUpnpConsentPrompt(false);
    try { await setUpnpMapping(allow); await poll(); }
    catch (err) { setError(`Could not save router preference: ${String(err)}`); }
    if (action) await executeAction(action);
  };
  useEffect(() => setProjectsLocation(status?.projects_location ?? ''), [status?.projects_location]);
  const pasteInvite = async () => {
    setSetup(null);
    setScreen('projects');
    setOffset(0);
    focusBaseline.current = new Set(status?.page?.projects.map(item => item.project.project_id) ?? []);
    try { await pasteInviteLink(); setError(''); await poll(); } catch (err) { setError(String(err)); }
  };
  const busy = !status?.ready || status.closing || status.jobs.some(j => j.running && !j.project_id);
  const browse = async () => {
    try {
      const value = await chooseFolder();
      if (value) {
        setFolder(value);
        const base = value.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? '';
        if (setup === 'link' && (!name.trim() || autoName)) { setName(base); setAutoName(true); }
      }
    } catch (err) { setError(String(err)); }
  };
  const saveSetup = async () => {
    if (setup === 'join') {
      setOffset(0);
      focusBaseline.current = new Set(status?.page?.projects.map(item => item.project.project_id) ?? []);
      if (await act({ kind: 'join', folder })) setSetup(null);
    } else {
      const projectId = uuid.trim();
      setOffset(0);
      focusBaseline.current = null;
      if (await act({ kind: 'link', project_id: projectId, name: name.trim(), folder, mailbox_url: mailbox.trim() || null, relay_address: relay.trim() || null })) {
        setSetup(null);
        setFocusProjectId(projectId);
      }
    }
  };
  return <div className="app-shell"><aside className="rail"><div className="brand-mark">TM</div><button className={screen === 'projects' ? 'rail-nav active' : 'rail-nav'} onClick={() => setScreen('projects')}>PROJECTS</button><button className={screen === 'settings' ? 'rail-nav active' : 'rail-nav'} onClick={() => setScreen('settings')}>SETTINGS</button><div className="rail-bottom">PRIVATE<br />BY DESIGN</div></aside>
    <main><header className="topbar"><div><div className="eyebrow">YOUR STUDIO, CONNECTED</div><h1>TRACKMESH</h1><p>Immutable snapshots. Trusted collaborators.</p></div>
      <div className="device"><span className={`status-dot ${status?.ready ? 'active' : ''}`} />{status?.device?.name ?? 'Starting device…'}<small>{short(status?.device?.device_id)}</small></div></header>
      {(error || status?.error || status?.network?.error) && <div className="notice danger" role="alert">{error || status?.error || status?.network?.error}</div>}
      {status?.invite_pending && <div className="invite-overlay"><section className="invite-dialog" role="dialog" aria-modal="true" aria-labelledby="invite-title"><h2 id="invite-title">Accept invite link</h2>
        {status.jobs.filter(job => job.label === 'Accept invite link' && job.error).slice(-1).map(job => <div className="notice danger" role="alert" key={job.job_id}><strong>{job.error}</strong><p>The link may have expired or already been used. Ask for a new link, or Cancel.</p></div>)}
        <p>The invitation will create a project folder under <button className="path-link" title="Open in Explorer" onClick={() => void openInviteParentFolder().catch(err => setError(String(err)))}>{displayPath(inviteParentFolder ?? status.projects_location)}\[project name]</button>. You can choose a different parent folder below.</p>
        {status.invite_needs_service_fields && <><label className="field">Supabase Project URL<input autoComplete="off" value={inviteServiceUrl} onChange={event => setInviteServiceUrl(event.target.value)} placeholder="https://your-project.supabase.co" /></label>
        <label className="field">Publishable key<input type="password" autoComplete="new-password" value={inviteServiceKey} onChange={event => setInviteServiceKey(event.target.value)} /></label></>}
        <div className="actions"><button className="primary" disabled={busy || (status.invite_needs_service_fields && (!inviteServiceUrl.trim() || !inviteServiceKey.trim()))} onClick={() => {
          setSetup(null); setScreen('projects'); setOffset(0);
          focusBaseline.current = new Set(status.page?.projects.map(item => item.project.project_id) ?? []);
          void act({ kind: 'complete_invite_link', ...(status.invite_needs_service_fields ? { supabase_url: inviteServiceUrl.trim(), supabase_anon_key: inviteServiceKey.trim() } : {}) });
        }}>Accept</button>
          <button disabled={!status.ready || status.closing || status.jobs.some(job => job.running && job.label === 'Accept invite link')} onClick={() => void cancelInviteLink().then(poll).catch(err => setError(String(err)))}>Cancel</button>
          <button disabled={busy} onClick={async () => { try { const value = await chooseInviteParentFolder(); if (value) setInviteParentFolder(value); } catch (err) { setError(String(err)); } }}>Choose another folder</button></div>
      </section></div>}
      {upnpConsentPrompt && <div className="invite-overlay"><section className="invite-dialog upnp-consent-dialog" role="dialog" aria-modal="true" aria-labelledby="upnp-consent-title"><h2 id="upnp-consent-title">Router setup</h2>
        <p>Let TrackMesh set up your router automatically? Needed when your collaborator is on a different internet connection. Opens one port only for TrackMesh; only paired devices can use it.</p>
        <div className="actions"><button className="primary" onClick={() => void answerUpnpConsent(true)}>Allow (recommended)</button><button onClick={() => void answerUpnpConsent(false)}>Not now</button></div>
      </section></div>}
      {status?.closing && <div className="notice">Work finished. TRACKMESH is shutting down safely.</div>}
      {screen === 'settings' ? <section className="settings-screen"><div className="section-heading"><div><h2>Settings</h2><span className="muted">Device identity and optional connection services.</span></div></div>
        <StorageSettings busy={busy} act={act} />
        <section className="settings-block"><h3>DAW plugin</h3>
          <p className="muted">Off: share only with Share current version. On: every save in the DAW shares a new version, including autosaves in some DAWs.</p>
          {status?.daw_plugin_settings.map(project => <div key={project.project_id}><p><strong>{project.name}</strong></p>
            <label><input type="checkbox" checked={project.auto_share} disabled={busy || !status.ready} onChange={event => void act({ kind: 'set_auto_share', project_id: project.project_id, enabled: event.currentTarget.checked })} /> Share automatically when the DAW project is saved</label>
          </div>)}
          {status?.ready && !status.daw_plugin_settings.length && <p className="muted">Link a project's folder in TrackMesh first.</p>}
        </section>
        <section className="settings-block"><h3>Connection</h3><label><input type="checkbox" checked={status?.network?.port_mapping.consent_enabled ?? false} disabled={busy} onChange={event => { void setUpnpMapping(event.currentTarget.checked).then(poll).catch(err => setError(String(err))); }} /> Set up router automatically (UPnP)</label><p className="muted">Opens one time-limited router port for TrackMesh. Only paired devices can use it.</p></section>
        <details className="setup"><summary>Advanced</summary>
          <DeviceNameSettings name={status?.device?.name ?? ''} busy={busy} act={act} />
          <div className="settings-block"><h3>Projects location</h3><p className="muted">New invitations create a named project folder here.</p><div className="input-row"><button className="path-link" title="Open in Explorer" onClick={() => void openProjectsLocation().catch(err => setError(String(err)))}>{displayPath(projectsLocation)}</button><button disabled={busy} onClick={async () => { try { const value = await chooseProjectsLocation(); if (value) { setProjectsLocation(value); await poll(); } } catch (err) { setError(String(err)); } }}>Browse…</button></div></div>
          <DefaultSupabaseSettings config={status?.default_supabase ?? null} busy={busy} act={act} />
          <ManualPortForwardingSettings endpoint={status?.manual_public_endpoint ?? null} listenPort={status?.listen_port ?? 43101} busy={busy} act={act} />
          <p className="muted">Supabase publishable key is public project configuration. Never enter a secret key here.</p>
          <section className="global-advanced"><h3>Advanced tools</h3><p className="muted">For older builds or custom network setups.</p><div className="actions"><button disabled={busy} onClick={() => { setScreen('projects'); setSetup('join'); setFolder(''); }}>Accept invitation file</button></div></section>
        </details>{status?.jobs.filter(j => !j.project_id).map(job => <JobNotice key={job.job_id} job={job} startedAt={jobStartedAt[job.job_id]} />)}</section> : <>
      <div className="section-heading"><div><h2>Projects</h2><span className="muted">Link a folder, invite a collaborator, then share or receive a version.</span></div><div className="actions">
        <button className="primary" disabled={busy} onClick={() => { setSetup('link'); setFolder(''); setUuid(crypto.randomUUID()); setName(''); setMailbox(''); setRelay(''); }}>+ Link folder</button><button className="primary" disabled={busy} onClick={() => void pasteInvite()}>Paste invite link</button></div></div>
      {setup && <section className="setup-panel"><h2>{setup === 'join' ? 'Pair with an inviter' : 'Link a local project'}</h2>
        {setup === 'link' && <><label className="field">Project name<input value={name} maxLength={320} onChange={e => { setName(e.target.value); setAutoName(false); }} placeholder="Track 07" /></label>
          </>}
        <label className="field">Local project folder<div className="input-row"><input value={folder} onChange={e => setFolder(e.target.value)} placeholder="D:\Projects\Track 07" /><button onClick={() => void browse()}>Browse…</button></div></label>
        {setup === 'link' && <details className="form-advanced"><summary>Advanced project settings</summary><div className="field-stack"><label className="field">Permanent project UUID<div className="input-row"><input value={uuid} onChange={e => setUuid(e.target.value)} /><button onClick={() => setUuid(crypto.randomUUID())}>New UUID</button></div></label>
          <label className="field">HTTPS mailbox (optional)<input value={mailbox} onChange={e => setMailbox(e.target.value)} placeholder="https://mailbox.example.com" /></label><label className="field">Pinned relay address (optional)<input value={relay} onChange={e => setRelay(e.target.value)} placeholder="/dns4/relay.example.com/udp/4001/quic-v1/p2p/…" /></label></div></details>}
        {setup === 'join' && <p className="muted">Choose your local folder, then open the private invitation. TrackMesh sends your pairing request automatically; keep the inviter online with their approval window open. Wait for approval before Check connection and Get collaborator's version. The invitation key stays in the native backend.</p>}
        <div className="actions"><button className="primary" disabled={busy || !folder || (setup === 'link' && (!name || !uuid))} onClick={() => void saveSetup()}>{setup === 'join' ? 'Accept invitation & connect…' : 'Link project'}</button><button onClick={() => setSetup(null)}>Cancel</button></div>
      </section>}
      <div className="project-list">{status?.page?.projects.map(card => <Card key={card.project.project_id} card={card} status={status} jobStartedAt={jobStartedAt} focusProjectId={focusProjectId} act={act} />)}</div>
      {status?.ready && !status.page?.projects.length && !setup && <div className="empty"><div className="empty-icon">↗</div><h2>Your first shared project</h2><p>Link a folder or accept an invitation link to get a project.</p><div className="actions"><button className="primary" disabled={busy} onClick={() => { setSetup('link'); setFolder(''); setUuid(crypto.randomUUID()); setName(''); }}>+ Link folder</button><button className="primary" disabled={busy} onClick={() => void pasteInvite()}>Paste invite link</button></div><p>Snapshots stay immutable; files stay on your PCs.</p></div>}
      <div className="pagination"><button disabled={!offset || busy} onClick={() => setOffset(Math.max(0, offset - 32))}>Previous</button><span className="muted">Page {offset / 32 + 1}</span><button disabled={!status?.page?.has_more || busy} onClick={() => setOffset(offset + 32)}>Next</button></div>
      {status?.jobs.filter(j => !j.project_id).map(job => <JobNotice key={job.job_id} job={job} startedAt={jobStartedAt[job.job_id]} />)}
      </>}
      <footer><div><span className="muted">Data directory</span><code>{status?.data_directory ?? 'Opening protected local storage…'}</code></div><div className="actions"><button title="Confirm who will lose access to your versions, finish accepted work, then quit TRACKMESH. Closing the window keeps it in the system tray." disabled={status?.closing} onClick={() => void closeApp().catch(err => setError(String(err)))}>Close safely</button></div></footer>
    </main></div>;
}

function ManualPortForwardingSettings({ endpoint, listenPort, busy, act }: {
  endpoint: { public_ipv4: string; external_port: number } | null;
  listenPort: number;
  busy: boolean;
  act: (action: Action) => Promise<boolean>;
}) {
  const [ip, setIp] = useState(endpoint?.public_ipv4 ?? '');
  const [port, setPort] = useState(endpoint ? String(endpoint.external_port) : '43101');
  useEffect(() => {
    setIp(endpoint?.public_ipv4 ?? '');
    setPort(endpoint ? String(endpoint.external_port) : '43101');
  }, [endpoint?.public_ipv4, endpoint?.external_port]);
  const validPort = /^\d{1,5}$/.test(port) && Number(port) > 0 && Number(port) <= 65535;
  const valid = publicIpv4(ip.trim()) && validPort;
  return <section className="settings-block"><h3>Manual port forwarding</h3>
    <p className="muted">Use only when TCP and UDP listen port {listenPort} are forwarded on your router by hand; update this when your public IP changes.</p>
    <div className="connection-inputs"><label className="field">Public IPv4<input value={ip} maxLength={15} disabled={busy} onChange={event => setIp(event.target.value)} placeholder="203.0.113.1" /></label>
      <label className="field">External port<input type="number" min={1} max={65535} value={port} disabled={busy} onChange={event => setPort(event.target.value)} /></label></div>
    <div className="actions"><button className="primary" disabled={busy || !valid} onClick={() => void act({ kind: 'set_manual_public_endpoint', public_ipv4: ip.trim(), external_port: Number(port) })}>Save</button>
      <button disabled={busy || !endpoint} onClick={() => void act({ kind: 'set_manual_public_endpoint', public_ipv4: null, external_port: null })}>Clear</button></div>
  </section>;
}

function DeviceNameSettings({ name, busy, act }: { name: string; busy: boolean; act: (action: Action) => Promise<boolean> }) {
  const [value, setValue] = useState(name);
  useEffect(() => setValue(name), [name]);
  return <details className="settings-block"><summary>Device name</summary><div className="field-stack"><label className="field">Name shown to collaborators<input maxLength={80} value={value} disabled={busy} onChange={event => setValue(event.target.value)} /></label>
    <button disabled={busy || !value.trim() || value.trim() === name} onClick={() => void act({ kind: 'set_device_name', name: value.trim() })}>Save device name</button></div></details>;
}
