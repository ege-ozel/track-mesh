import { useCallback, useEffect, useRef, useState } from 'react';
import { chooseFolder, closeApp, getStatus, submitAction } from './api';
import type { Action, DesktopStatus, ProjectCard, SnapshotInfo } from './api';

const short = (id: string | null | undefined) => id ? id.slice(0, 8) : '—';
const time = (value: string | null | undefined) => value ? new Date(value).toLocaleString() : 'Never';
const bytes = (n: number) => n >= 1024 ** 3 ? `${(n / 1024 ** 3).toFixed(1)} GB` : n >= 1024 ** 2 ? `${(n / 1024 ** 2).toFixed(1)} MB` : n >= 1024 ? `${(n / 1024).toFixed(1)} KB` : `${n} B`;

function Snapshot({ value, empty }: { value: SnapshotInfo | null; empty: string }) {
  return value ? <><div className="snapshot-id">{short(value.snapshot_id)} <span>{value.metadata_status}</span></div>
    <p>{value.author_name} · {time(value.created_at)}</p>
    <p>{value.file_count} files · {bytes(value.total_size)} · {bytes(value.changed_bytes)} changed</p></>
    : <p className="muted">{empty}</p>;
}

function Card({ card, status, act }: { card: ProjectCard; status: DesktopStatus; act: (action: Action) => Promise<void> }) {
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
  const divergence = check?.current_known && check.result.lineage === 'DIVERGED';
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
        {!online && peer?.announcement && <p className="offline-note">Source offline. Project files stay on collaborator PCs.</p>}</section></div>
    <div className="peer-line"><label>Collaborator <select value={peerId ?? ''} disabled={busy || !card.peers.length} onChange={e => { setSelected(e.target.value); setDawClosed(false); }}>
      {!card.peers.length && <option value="">No paired devices</option>}{card.peers.map(p => <option key={p.device.device_id} value={p.device.device_id}>{p.device.name}</option>)}</select></label>
      <span className="muted">{online ? connection?.transport.toUpperCase() : 'Last-known status'}</span>
      <button className="text-button" disabled={busy || !peer} onClick={() => peer && act({ kind: 'check', project_id: project.project_id, device_id: peer.device.device_id })}>Check peer</button></div>
    <div className={`verification ${divergence ? 'danger' : ''}`}>
      {fresh && check?.result.verified_equal ? `UP TO DATE · verified ${check.age_seconds}s ago`
        : check?.current_known && check.result.lineage && check.result.lineage !== 'EQUAL' ? `${check.result.lineage.replaceAll('_', ' ')} · ${fresh ? 'checked' : 'last checked'} ${check.age_seconds}s ago`
        : `Last verified: ${time(project.last_verified_at)} · Check peer for a fresh comparison`}
    </div>
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
    {job && <div className={`job ${job.error ? 'danger' : ''}`} role="status">{job.running ? `${job.label}…` : job.error ? `${job.label}: ${job.error}`
      : job.outcome?.publication_error ? `Snapshot committed locally. Publication pending: ${String(job.outcome.publication_error)}`
      : job.outcome?.message ? String(job.outcome.message) : `${job.label} completed`}</div>}
    <details className="setup"><summary>Collaboration & connection details</summary>
      <p className="muted">Pair both directions. Invitations contain a private project capability; exchange files privately.</p>
      <div className="actions"><button disabled={busy} onClick={() => request('export_invite')}>Export private invitation</button><button disabled={busy} onClick={() => request('pair')}>Import collaborator card</button>
        <button className="text-button danger" disabled={busy || !peer} onClick={() => peer && window.confirm(`Remove ${peer.device.name}'s local access to this project? Infrastructure ACLs also need an operator update.`) && act({ kind: 'revoke', project_id: project.project_id, device_id: peer.device.device_id })}>Revoke selected peer</button></div>
      <p>UUID <code>{project.project_id}</code></p><p>Mailbox <code>{project.mailbox_url ?? 'Not configured'}</code></p><p>Relay <code>{project.relay_address ?? 'Not configured'}</code></p>
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
        <button disabled={busy} onClick={() => { setSetup('join'); setFolder(''); }}>Import invitation</button>
        <button className="primary" disabled={busy} onClick={() => { setSetup('link'); setFolder(''); setUuid(''); setName(''); }}>+ Link folder</button></div></div>
      {setup && <section className="setup-panel"><h2>{setup === 'join' ? 'Join a private project' : 'Link a local project'}</h2>
        {setup === 'link' && <><label>Project name<input value={name} maxLength={320} onChange={e => setName(e.target.value)} placeholder="Track 07" /></label>
          <label>Permanent project UUID<div className="input-row"><input value={uuid} onChange={e => setUuid(e.target.value)} placeholder="Existing UUID, or create one" /><button onClick={() => setUuid(crypto.randomUUID())}>New UUID</button></div></label></>}
        <label>Local project folder<div className="input-row"><input value={folder} onChange={e => setFolder(e.target.value)} placeholder="D:\Projects\Track 07" /><button onClick={() => void browse()}>Browse…</button></div></label>
        {setup === 'link' && <><label>HTTPS mailbox (optional)<input value={mailbox} onChange={e => setMailbox(e.target.value)} placeholder="https://mailbox.example.com" /></label><label>Pinned relay address (optional)<input value={relay} onChange={e => setRelay(e.target.value)} placeholder="/dns4/relay.example.com/udp/4001/quic-v1/p2p/…" /></label></>}
        {setup === 'join' && <p className="muted">Select the private invitation file after choosing the local folder. Its key stays in the native backend.</p>}
        <div className="actions"><button className="primary" disabled={busy || !folder || (setup === 'link' && (!name || !uuid))} onClick={() => void saveSetup()}>{setup === 'join' ? 'Choose invitation…' : 'Link project'}</button><button onClick={() => setSetup(null)}>Cancel</button></div>
      </section>}
      <div className="project-list">{status?.page?.projects.map(card => <Card key={card.project.project_id} card={card} status={status} act={act} />)}</div>
      {status?.ready && !status.page?.projects.length && !setup && <div className="empty"><div className="empty-icon">↗</div><h2>Your first shared project</h2><p>Link a project folder or import a collaborator’s invitation.<br />Snapshots stay immutable; files stay on your PCs.</p></div>}
      <div className="pagination"><button disabled={!offset || busy} onClick={() => setOffset(Math.max(0, offset - 32))}>Previous</button><span className="muted">Page {offset / 32 + 1}</span><button disabled={!status?.page?.has_more || busy} onClick={() => setOffset(offset + 32)}>Next</button></div>
      {status?.jobs.filter(j => !j.project_id).map(job => <div key={job.job_id} className={`notice ${job.error ? 'danger' : ''}`} role="status">{job.running ? `${job.label}…` : job.error ?? String(job.outcome?.message ?? `${job.label} completed`)}</div>)}
      <footer><div><span className="muted">Data directory</span><code>{status?.data_directory ?? 'Opening protected local storage…'}</code></div><div className="actions"><button disabled={busy} onClick={() => act({ kind: 'export_card' })}>Export my device card</button><button title="Close the window. Active DAW sessions, transfers and pending publication keep TRACKMESH running; it exits 60 seconds after work finishes. Reopen desktop to return." disabled={status?.closing} onClick={() => void closeApp().catch(err => setError(String(err)))}>Close safely</button></div></footer>
    </main></div>;
}
