import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { chooseFolder, stopCheck } from './api';
import type { Action, CheckView, DesktopStatus, ProjectBackend, ProjectBackendConfig, ProjectCard, SnapshotInfo, SupabaseDefaultConfig } from './api';
import { JobNotice } from './Jobs';
import type { AdvancedSection } from './ProjectCard';
import { bytes, displayPath, localIpv4Addresses, publicIpv4, short, time } from './format';

type Act = (action: Action) => Promise<boolean>;
type Peer = ProjectCard['peers'][number];

function Section({ id, title, summary, tone, open, highlight, onToggle, children }: {
  id: AdvancedSection | string; title: string; summary: string; tone?: 'ok' | 'bad'; open: boolean; highlight: boolean;
  onToggle: (open: boolean) => void; children: ReactNode;
}) {
  const ref = useRef<HTMLDetailsElement>(null);
  useEffect(() => { if (highlight) ref.current?.scrollIntoView({ behavior: 'smooth', block: 'start' }); }, [highlight]);
  return <details ref={ref} id={`advanced-${id}`} className={`adv-section${highlight ? ' focus-highlight' : ''}`} open={open} onToggle={event => onToggle(event.currentTarget.open)}>
    <summary><span className="adv-title">{title}</span><span className={`adv-summary ${tone ?? ''}`}>{summary}</span></summary>
    <div className="adv-body">{children}</div>
  </details>;
}

function nextStep(card: ProjectCard, status: DesktopStatus, peer: Peer | undefined, check: CheckView | undefined) {
  const projectId = card.project.project_id;
  const candidates = status.pairing.project_id === projectId ? status.pairing.requests : [];
  const connection = peer ? status.network?.peers[peer.device.peer_id] : undefined;
  const checkJob = status.jobs.find(job => job.project_id === projectId && job.label === 'Check connection');
  const runtimeError = status.error ?? status.network?.error;
  const name = peer?.device.name ?? 'your collaborator';
  if (runtimeError) return 'The local network session has an error. Quit TrackMesh from the tray after current work finishes, then open it again.';
  if (candidates.length) return 'Someone opened your invite link. Approve them on the project card in Projects.';
  if (!peer && status.pairing.project_id === projectId) return 'Waiting for your collaborator to open the invite link. Keep TrackMesh open.';
  if (!peer) return 'Invite your collaborator from the project card in Projects.';
  if (checkJob?.running) return `Trying to reach ${name}. This can take up to 30 seconds.`;
  if (checkJob?.error) return connection?.online
    ? `${name}'s PC answers, but the project check failed. Make sure ${name} approved this device for this project.`
    : `${name}'s PC didn't answer. Make sure TrackMesh is open on both PCs. If you are on different internet connections, at least one router must allow TrackMesh (automatic router setup or manual port forwarding below).`;
  if (!connection?.online) return `Press Check connection while TrackMesh is open on both PCs. If it fails, ask ${name} to press Check connection too.`;
  if (!check?.fresh) return `Connected to ${name}. Check connection also compares both versions.`;
  if (card.project.recovery_required) return 'The last Get did not finish. Recover it from the project card in Projects.';
  switch (check.result.snapshot_status) {
    case 'DIVERGED': return 'Both versions changed. Choose which one to keep on the project card; TrackMesh does not merge projects.';
    case 'UP_TO_DATE': return 'Both PCs have the same version.';
    case 'LOCAL_AHEAD': return `Your version is newer. ${name} can press Get.`;
    case 'REMOTE_AHEAD': return `${name}'s version is newer. Close the project in your DAW, then press Get.`;
    default: return 'Version history is unknown. Look at Version history below before getting a version.';
  }
}

function Snapshot({ value, empty, mailboxEnabled }: { value: SnapshotInfo | null; empty: string; mailboxEnabled: boolean }) {
  return value ? <><div className="snapshot-id">{short(value.snapshot_id)} {mailboxEnabled && <span>{value.metadata_status}</span>}</div>
    <p>{value.author_name} · {time(value.created_at)}</p>
    <p>{value.file_count} files · {bytes(value.total_size)} · {bytes(value.changed_bytes)} changed</p></>
    : <p className="muted">{empty}</p>;
}

function RawConnectionDetails({ status, projectId, busy, act }: { status: DesktopStatus; projectId: string; busy: boolean; act: Act }) {
  const [ip, setIp] = useState('');
  const [port, setPort] = useState(String(status.listen_port));
  const publicIp = ip.trim();
  const validPort = /^\d{1,5}$/.test(port) && Number(port) > 0 && Number(port) <= 65535;
  const valid = publicIpv4(publicIp) && validPort;
  const observation = status.network_observation;
  const firewall = status.firewall_observation;
  const mapping = status.network?.port_mapping;
  const reach = status.network?.reachability;
  return <details><summary>Technical details</summary>
    <div className="raw-grid">
      <p><strong>Local interfaces</strong> {observation.interfaces.length ? observation.interfaces.map(iface => `${iface.name || `Interface ${iface.interface_index}`} [${iface.ipv4.join(', ') || 'UNKNOWN'}]`).join('; ') : 'UNKNOWN'}</p>
      <p><strong>Default gateway</strong> {observation.default_gateways.length ? observation.default_gateways.map(gateway => `${gateway.address} (interface ${gateway.interface_index}; ${gateway.source})`).join('; ') : 'UNKNOWN'}</p>
      <p><strong>Listen port</strong> {status.listen_port} (default 43101). Changing it needs a restart with that startup port.</p>
      <p><strong>UPnP</strong> {mapping?.state ?? 'NOT_ATTEMPTED'} · TCP {mapping?.tcp ?? 'NOT_ATTEMPTED'} · UDP {mapping?.udp ?? 'NOT_ATTEMPTED'} · router external IPv4 {mapping?.external_ip_state ?? 'UNKNOWN'}{mapping?.detail ? ` · ${mapping.detail}` : ''}</p>
      <p><strong>Reachability</strong> {reach?.state ?? 'UNVERIFIED'}{reach?.peer ? ` · peer ${reach.peer}` : ''}{reach?.transport ? ` · ${reach.transport}` : ''}{reach?.at_unix_seconds ? ` · ${new Date(reach.at_unix_seconds * 1000).toLocaleString()}` : ''}. Same-network connections do not prove internet reachability.</p>
      <p><strong>Windows Firewall</strong> {firewall.state}. {firewall.profiles.map(profile => `${profile.name}: ${profile.enabled === null ? 'UNKNOWN' : profile.enabled ? 'on' : 'off'}, block all inbound ${profile.block_all_inbound ?? 'UNKNOWN'}, default inbound ${profile.default_inbound ?? 'UNKNOWN'}`).join('; ')}. Rules for port {status.listen_port}: TCP allow {firewall.tcp_rules.allow}, block {firewall.tcp_rules.block}; UDP allow {firewall.udp_rules.allow}, block {firewall.udp_rules.block}. Third-party firewall: {firewall.third_party_products}.</p>
      <p><strong>Port status</strong> {status.port_status === 'UNVERIFIED' ? 'Port PORT_UNVERIFIED' : 'Port UNKNOWN'}</p>
    </div>
    <details><summary>Advertised and listen addresses</summary>{(status.network?.addresses ?? []).map(address => <code className="connection-address" key={address}>{address}</code>)}
      {status.configured_external_addresses.map(address => <code className="connection-address" key={`external-${address}`}>{address} (startup external address)</code>)}</details>
    <details><summary>Invitation file with public addresses</summary>
      <p className="muted">For a one-off invitation when you forwarded a router port by hand. The addresses go into this file only.</p>
      <div className="connection-inputs"><label className="field">Public IPv4<input value={ip} maxLength={15} disabled={busy} onChange={e => setIp(e.target.value)} placeholder="203.0.113.1" /></label>
        <label className="field">External router port<input type="number" min={1} max={65535} value={port} disabled={busy} onChange={e => setPort(e.target.value)} /></label></div>
      {publicIp && !valid && <p className="error">Enter a public IPv4 and a port from 1 to 65535. 10.x, 172.16–31.x, 192.168.x and 100.64–127.x are not public.</p>}
      <div className="actions"><button disabled={busy || !valid} onClick={() => act({ kind: 'export_invite', project_id: projectId, public_ip: publicIp, external_port: Number(port) })}>Save invitation file</button></div>
    </details>
  </details>;
}

function ProjectBackendSettings({ projectId, config, defaultSupabase, state, revokeProblem, missingFunctions, unreachable, busy, act }: {
  projectId: string; config: ProjectBackendConfig | undefined; busy: boolean;
  defaultSupabase: SupabaseDefaultConfig | null;
  state: DesktopStatus['supabase_states'][string];
  revokeProblem: boolean; missingFunctions: string[]; unreachable: boolean;
  act: Act;
}) {
  const [backend, setBackend] = useState<ProjectBackend>(config?.backend ?? 'NONE');
  const [url, setUrl] = useState((config?.backend === 'SUPABASE' ? config.supabase_url : null) ?? defaultSupabase?.url ?? '');
  const [anonKey, setAnonKey] = useState((config?.backend === 'SUPABASE' ? config.supabase_anon_key : null) ?? defaultSupabase?.publishable_key ?? '');
  useEffect(() => setBackend(config?.backend ?? 'NONE'), [config?.backend]);
  useEffect(() => {
    if (config?.backend !== 'SUPABASE') {
      setUrl(defaultSupabase?.url ?? '');
      setAnonKey(defaultSupabase?.publishable_key ?? '');
    }
  }, [config?.backend, config?.supabase_url, config?.supabase_anon_key, defaultSupabase?.url, defaultSupabase?.publishable_key]);
  return <div className="backend-settings">
    <p className="muted">Invite links and version notices use a small Supabase project you own. It stores only encrypted invite drops, addresses and version hints — never project files.</p>
    <label className="field">Service<select value={backend} disabled={busy} onChange={event => setBackend(event.target.value as ProjectBackend)}>
      <option value="SUPABASE">Supabase (recommended)</option>
      <option value="NONE">None · direct connections only</option>
      <option value="SELF_HOSTED_MAILBOX">Self-hosted mailbox · this project's mailbox URL</option>
    </select></label>
    {backend === 'SELF_HOSTED_MAILBOX' && <p className="muted">Uses this project's existing mailbox URL without changing its behavior.</p>}
    {backend === 'SUPABASE' && <div className="backend-fields">
      <label className="field">Supabase Project URL<input autoComplete="off" spellCheck={false} value={url} disabled={busy} placeholder="https://your-project.supabase.co" onChange={event => setUrl(event.target.value)} /></label>
      <label className="field">Publishable key<input type="password" autoComplete="new-password" spellCheck={false} value={anonKey} disabled={busy} onChange={event => setAnonKey(event.target.value)} /></label>
      <p className="muted">HTTPS <code>*.supabase.co</code> addresses only. Never enter a service role or secret key.</p>
    </div>}
    <div className="actions"><button className="primary" disabled={busy || (backend === 'SUPABASE' && (!url.trim() || !anonKey.trim()))} onClick={() => void act({ kind: 'set_project_backend', project_id: projectId, backend,
      supabase_url: backend === 'SUPABASE' ? url.trim() : null,
      supabase_anon_key: backend === 'SUPABASE' ? anonKey.trim() : null })}>Save</button><span className="badge">{backend === 'SELF_HOSTED_MAILBOX' ? 'SELF_HOSTED' : state}</span></div>
    {unreachable && <p className="error">The invite service can't be reached. Check the address above and this PC's internet connection.</p>}
    {revokeProblem && <p className="muted">The invite service didn't remove a device. Only the device that created the project can remove others; it may also have been removed already.</p>}
    {missingFunctions.length > 0 && <p className="error">The invite service is missing these functions: {missingFunctions.join(', ')}. Deploy them as described in services/supabase/README.md.</p>}
  </div>;
}

function DefaultSupabaseSettings({ config, busy, act }: { config: SupabaseDefaultConfig | null; busy: boolean; act: Act }) {
  const [url, setUrl] = useState(config?.url ?? '');
  const [publishableKey, setPublishableKey] = useState(config?.publishable_key ?? '');
  useEffect(() => {
    setUrl(config?.url ?? '');
    setPublishableKey(config?.publishable_key ?? '');
  }, [config?.url, config?.publishable_key]);
  return <div className="field-stack">
    <p className="muted">Used for new projects and prefilled in each project's Invite service. Saved in this app's settings, outside your projects. The publishable key is public configuration; never enter a secret key.</p>
    <label className="field">Supabase Project URL<input autoComplete="off" spellCheck={false} value={url} disabled={busy} placeholder="https://your-project.supabase.co" onChange={event => setUrl(event.target.value)} /></label>
    <label className="field">Publishable key<input type="password" autoComplete="new-password" spellCheck={false} value={publishableKey} disabled={busy} onChange={event => setPublishableKey(event.target.value)} /></label>
    <div className="actions"><button className="primary" disabled={busy || !url.trim() || !publishableKey.trim()} onClick={() => void act({ kind: 'set_default_supabase', url: url.trim(), publishable_key: publishableKey.trim() })}>Save</button>
      <button disabled={busy || !config} onClick={() => void act({ kind: 'set_default_supabase', url: null, publishable_key: null })}>Clear</button></div>
  </div>;
}

function ManualPortForwardingSettings({ endpoint, listenPort, localIps, busy, act }: {
  endpoint: { public_ipv4: string; external_port: number } | null; listenPort: number; localIps: string[]; busy: boolean; act: Act;
}) {
  const [ip, setIp] = useState(endpoint?.public_ipv4 ?? '');
  const [port, setPort] = useState(endpoint ? String(endpoint.external_port) : String(listenPort));
  useEffect(() => {
    setIp(endpoint?.public_ipv4 ?? '');
    setPort(endpoint ? String(endpoint.external_port) : String(listenPort));
  }, [endpoint?.public_ipv4, endpoint?.external_port, listenPort]);
  const validPort = /^\d{1,5}$/.test(port) && Number(port) > 0 && Number(port) <= 65535;
  const valid = publicIpv4(ip.trim()) && validPort;
  return <div className="field-stack">
    <p className="muted">Only when automatic router setup fails. In your router, forward TCP and UDP {validPort ? port : 'PORT'} to this PC ({localIps.length ? localIps.map(local => `${local}:${listenPort}`).join(' or ') : `local port ${listenPort}`}) and reserve this PC's address in the router's DHCP settings. Then enter your public IPv4 here. Update it when your public IP changes.</p>
    <div className="connection-inputs"><label className="field">Public IPv4<input value={ip} maxLength={15} disabled={busy} onChange={event => setIp(event.target.value)} placeholder="203.0.113.1" /></label>
      <label className="field">External port<input type="number" min={1} max={65535} value={port} disabled={busy} onChange={event => setPort(event.target.value)} /></label></div>
    {ip.trim() && !valid && <p className="error">Enter a public IPv4 and a port from 1 to 65535. If your router's WAN address starts with 100.64–127, your internet provider shares it (CGNAT) and port forwarding can't work.</p>}
    <div className="actions"><button className="primary" disabled={busy || !valid} onClick={() => void act({ kind: 'set_manual_public_endpoint', public_ipv4: ip.trim(), external_port: Number(port) })}>Save</button>
      <button disabled={busy || !endpoint} onClick={() => void act({ kind: 'set_manual_public_endpoint', public_ipv4: null, external_port: null })}>Clear</button></div>
  </div>;
}

export function AdvancedScreen({ status, projectId, section, jobStartedAt, act, pickProject, openInvitationFile, onError }: {
  status: DesktopStatus; projectId: string | null; section: AdvancedSection | null; jobStartedAt: Record<string, number>;
  act: Act; pickProject: (projectId: string) => void; openInvitationFile: () => void; onError: (message: string) => void;
}) {
  const projects = status.page?.projects ?? [];
  const card = projects.find(item => item.project.project_id === projectId) ?? projects[0];
  const [open, setOpen] = useState<Record<string, boolean>>({ connection: true });
  const [selectedPeer, setSelectedPeer] = useState('');
  useEffect(() => { if (section) setOpen(current => ({ ...current, [section]: true })); }, [section, projectId]);
  const toggle = (id: string) => (value: boolean) => setOpen(current => current[id] === value ? current : { ...current, [id]: value });
  const globalBusy = !status.ready || status.closing || status.jobs.some(j => j.running && !j.project_id);
  const localIps = localIpv4Addresses(status);
  const mapping = status.network?.port_mapping;
  const thisPc = <>
    <h2 className="group-title">This PC</h2>
    <Section id="default-service" title="Default invite service" summary={status.default_supabase ? 'Saved' : 'Not set'} tone={status.default_supabase ? 'ok' : undefined} open={!!open['default-service']} highlight={false} onToggle={toggle('default-service')}>
      <DefaultSupabaseSettings config={status.default_supabase} busy={globalBusy} act={act} />
    </Section>
    <Section id="port-forwarding" title="Manual port forwarding" summary={status.manual_public_endpoint ? `${status.manual_public_endpoint.public_ipv4}:${status.manual_public_endpoint.external_port}` : 'Off'} open={!!open['port-forwarding']} highlight={false} onToggle={toggle('port-forwarding')}>
      <ManualPortForwardingSettings endpoint={status.manual_public_endpoint} listenPort={status.listen_port} localIps={localIps} busy={globalBusy} act={act} />
    </Section>
    <Section id="invitation-file" title="Open an invitation file" summary="When invite links don't work" open={!!open['invitation-file']} highlight={false} onToggle={toggle('invitation-file')}>
      <p className="muted">For an invitation saved as a file (Invite without the service). Choose an empty folder for the project, then open the file.</p>
      <div className="actions"><button disabled={globalBusy} onClick={openInvitationFile}>Open invitation file…</button></div>
    </Section>
    <Section id="data" title="App data" summary="Where TrackMesh keeps its own files" open={!!open.data} highlight={false} onToggle={toggle('data')}>
      <p className="muted">Keys, history and file data for your projects. Do not share or delete this folder.</p>
      <code>{status.data_directory ?? 'Opening protected local storage…'}</code>
    </Section>
  </>;
  if (!card) return <section className="advanced-screen"><p className="muted">Share a project folder first. Project connection details appear here.</p>{thisPc}</section>;

  const project = card.project;
  const id = project.project_id;
  const peer = card.peers.find(p => p.device.device_id === selectedPeer) ?? card.peers[0];
  const name = peer?.device.name ?? 'your collaborator';
  const connection = peer ? status.network?.peers[peer.device.peer_id] : undefined;
  const online = connection?.online ?? false;
  const busy = globalBusy || status.jobs.some(j => j.running && j.project_id === id);
  const check = status.checks.find(c => c.result.project_id === id && c.device_id === peer?.device.device_id);
  const checkJob = status.jobs.find(job => job.project_id === id && job.label === 'Check connection');
  const inviteState = status.supabase_states[id] ?? 'NOT_CONFIGURED';
  const backend = status.project_backends[id];
  const serviceSummary = backend?.backend === 'SELF_HOSTED_MAILBOX' ? 'Self-hosted mailbox'
    : backend?.backend !== 'SUPABASE' ? 'Not set up'
    : inviteState === 'UNREACHABLE' ? "Can't reach the service"
    : ['CLAIMED', 'MEMBER'].includes(inviteState) ? 'Connected' : 'Connecting…';
  const firewallAllowed = status.firewall_observation.tcp_rules.allow === 'PRESENT' && status.firewall_observation.udp_rules.allow === 'PRESENT';
  const runtimeError = status.error ?? status.network?.error;
  const chooseProjectRoot = async () => {
    try {
      const folder = await chooseFolder();
      if (folder) await act({ kind: 'change_project_root', project_id: id, folder });
    } catch (err) { onError(String(err)); }
  };
  return <section className="advanced-screen">
    <div className="project-chips" role="tablist" aria-label="Project">{projects.map(item => <button key={item.project.project_id} role="tab" aria-selected={item.project.project_id === id} className={item.project.project_id === id ? 'chip on' : 'chip'} onClick={() => pickProject(item.project.project_id)}>{item.project.name}</button>)}</div>
    {card.peers.length > 1 && <label className="peer-picker">Collaborator <select value={peer?.device.device_id ?? ''} onChange={e => setSelectedPeer(e.target.value)}>{card.peers.map(p => <option key={p.device.device_id} value={p.device.device_id}>{p.device.name}</option>)}</select></label>}

    <Section id="connection" title="Connection" summary={runtimeError ? 'Network error' : !peer ? 'No collaborator yet' : online ? `Connected · ${connection?.route === 'RELAYED' ? 'relayed' : 'direct'}` : `Can't reach ${name}`} tone={online ? 'ok' : peer ? 'bad' : undefined}
      open={!!open.connection} highlight={section === 'connection'} onToggle={toggle('connection')}>
      <div className="status-rows">
        <div><span>This PC</span><span className={runtimeError ? 'bad' : 'ok'}>{runtimeError ? runtimeError : status.ready ? 'Running' : 'Starting'}</span></div>
        {peer && <div><span>{name}</span><span className={online ? 'ok' : 'bad'}>{online ? `Reachable · ${connection?.transport}` : checkJob?.running ? 'Checking…' : checkJob?.error ? 'Last check failed' : 'Not connected'}</span></div>}
        <div><span>Router (automatic setup)</span><span className={mapping?.state === 'MAPPED' ? 'ok' : mapping?.state === 'FAILED' ? 'bad' : ''}>{!mapping?.consent_enabled ? 'Off · turn on in Settings' : mapping.state === 'MAPPED' ? 'Port opened' : mapping.state === 'FAILED' ? 'Router refused' : 'Trying…'}</span></div>
        <div><span>Manual port forwarding</span><span>{status.manual_public_endpoint ? `${status.manual_public_endpoint.public_ipv4}:${status.manual_public_endpoint.external_port}` : 'Off'}</span></div>
        <div><span>Windows Firewall</span><span className={firewallAllowed ? 'ok' : ''}>{firewallAllowed ? 'TrackMesh allowed' : 'No TrackMesh rule found · reinstall TrackMesh'}</span></div>
      </div>
      <p className="next-step"><strong>Next step</strong>{nextStep(card, status, peer, check)}</p>
      {checkJob && <JobNotice job={checkJob} startedAt={jobStartedAt[checkJob.job_id]} collaboratorName={peer?.device.name} onStop={() => void stopCheck(id).catch(err => onError(String(err)))} />}
      <div className="actions">
        {peer && <button className="primary" disabled={busy} onClick={() => act({ kind: 'check', project_id: id, device_id: peer.device.device_id })}>Check connection</button>}
        <button onClick={() => { setOpen(current => ({ ...current, 'port-forwarding': true })); window.requestAnimationFrame(() => document.getElementById('advanced-port-forwarding')?.scrollIntoView({ behavior: 'smooth', block: 'start' })); }}>Manual port forwarding…</button>
      </div>
      <p className="muted">A failed check can't tell which of these blocked the connection: the other PC being closed, a firewall, guest Wi-Fi isolation or a router.</p>
      <RawConnectionDetails status={status} projectId={id} busy={busy} act={act} />
    </Section>

    <Section id="service" title="Invite service" summary={serviceSummary} tone={serviceSummary === 'Connected' ? 'ok' : inviteState === 'UNREACHABLE' || card.missing_functions.length ? 'bad' : undefined}
      open={!!open.service} highlight={section === 'service'} onToggle={toggle('service')}>
      <ProjectBackendSettings key={id} projectId={id} config={backend} defaultSupabase={status.default_supabase} state={inviteState} revokeProblem={card.revoke_problem} missingFunctions={card.missing_functions}
        unreachable={card.grant_problem === 'UNREACHABLE' || inviteState === 'UNREACHABLE'} busy={busy} act={act} />
    </Section>

    <Section id="manual" title="Invite without the service" summary="Invitation file" open={!!open.manual} highlight={section === 'manual'} onToggle={toggle('manual')}>
      <ol className="steps-list">
        <li><strong>Save an invitation file</strong> and send it privately. Saving opens a temporary approval window.</li>
        <li><strong>Your collaborator opens it</strong> in Advanced → Open an invitation file. Their device request arrives here automatically.</li>
        <li><strong>Approve them</strong> on the project card.</li>
      </ol>
      <div className="actions"><button disabled={busy} onClick={() => act({ kind: 'export_invite', project_id: id })}>Save invitation file</button>
        {status.pairing.project_id === id
          ? <button disabled={busy} onClick={() => act({ kind: 'close_pairing', project_id: id })}>Close approval window</button>
          : <button disabled={busy} onClick={() => act({ kind: 'open_pairing', project_id: id })}>Open approval window</button>}
        {peer && <button disabled={busy} onClick={() => act({ kind: 'request_pairing', project_id: id, device_id: peer.device.device_id })}>Retry automatic pairing</button>}</div>
      <p className="muted">Only one project accepts new devices at a time; closing, expiry or restart removes unapproved requests. Encryption protects contents, not IP addresses or traffic timing.</p>
      <details><summary>Legacy file pairing</summary><p className="muted">For an older build: exchange signed device cards by hand. Access is granted only after the inviter opens the card for this project.</p>
        <div className="actions"><button disabled={busy} onClick={() => act({ kind: 'pair', project_id: id })}>Open pairing reply</button>
          <button disabled={busy} onClick={() => act({ kind: 'export_card' })}>Save pairing reply</button></div></details>
    </Section>

    <Section id="history" title="Version history" summary={peer && peer.history.length ? `${peer.history.length} recent from ${name}` : card.local_snapshot ? 'Local version only' : 'No versions yet'}
      open={!!open.history} highlight={section === 'history'} onToggle={toggle('history')}>
      <div className="snapshots"><section><h3>This PC</h3><Snapshot value={card.local_snapshot} empty="No version yet." mailboxEnabled={card.mailbox_enabled} /></section>
        <section><h3>{peer ? name : 'Collaborator'}</h3><Snapshot value={peer?.announcement ?? null} empty="No version seen yet." mailboxEnabled={card.mailbox_enabled} /></section></div>
      <p className="muted">{check?.fresh ? `Last comparison: ${check.result.snapshot_status.replaceAll('_', ' ').toLowerCase()}` : 'No live comparison. Check connection compares both versions.'}</p>
      {peer && peer.history.length > 1 && <ol className="history-list">{peer.history.map(snapshot => <li key={snapshot.snapshot_id}><code>{short(snapshot.snapshot_id)}</code> · {snapshot.author_name} · {time(snapshot.created_at)} · {snapshot.file_count} files</li>)}</ol>}
    </Section>

    <Section id="access" title="Folder, ID and access" summary={card.peers.length ? `${card.peers.length} collaborator${card.peers.length > 1 ? 's' : ''}` : 'Folder'}
      open={!!open.access} highlight={section === 'access'} onToggle={toggle('access')}>
      <div className="status-rows">
        <div><span>Folder</span><code>{displayPath(project.root)}</code></div>
        <div><span>Project ID</span><code>{project.project_id}</code></div>
        <div><span>Mailbox</span><code>{project.mailbox_url ?? 'Not configured'}</code></div>
        <div><span>Relay (legacy)</span><code>{project.relay_address ?? 'Not configured'}</code></div>
      </div>
      <div className="actions"><button disabled={busy} onClick={() => void chooseProjectRoot()}>{card.root_available ? 'Change folder…' : 'Locate folder…'}</button></div>
      {card.peers.map(item => <div className="access-row" key={item.device.device_id}>
        <div><strong>{item.device.name}</strong><p className="muted">Device <code>{item.device.device_id}</code> · Identity <code>{item.device.peer_id}</code></p></div>
        <button className="danger" disabled={busy} onClick={() => window.confirm(`Remove ${item.device.name}'s access to ${project.name}? They keep the files they already received.`) && act({ kind: 'revoke', project_id: id, device_id: item.device.device_id })}>Remove access</button>
      </div>)}
      <p className="muted">{card.mailbox_enabled ? 'The mailbox stores encrypted metadata and version notices only.' : 'Version notices use the invite service; the legacy mailbox is off for this project.'}</p>
    </Section>
    {thisPc}
  </section>;
}
