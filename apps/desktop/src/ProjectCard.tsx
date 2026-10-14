import { useState } from 'react';
import { chooseFolder, openPreviousVersion, openProjectFolder, stopCheck } from './api';
import type { Action, DesktopStatus, ProjectCard } from './api';
import { InProgress, InviteCopiedNotice, JobNotice, Waiting } from './Jobs';
import { bytes, displayPath, short, time } from './format';

export type AdvancedSection = 'connection' | 'service' | 'manual' | 'history' | 'access';

type Problem = { tone: 'warn' | 'danger'; title: string; body: string; fix: string; section: AdvancedSection };

export function Card({ card, status, jobStartedAt, focusProjectId, act, openAdvanced }: {
  card: ProjectCard; status: DesktopStatus; jobStartedAt: Record<string, number>; focusProjectId: string | null;
  act: (action: Action) => Promise<boolean>; openAdvanced: (projectId: string, section: AdvancedSection) => void;
}) {
  const [selected, setSelected] = useState('');
  const [dawClosed, setDawClosed] = useState(false);
  const [folderError, setFolderError] = useState('');
  const [keepMinePending, setKeepMinePending] = useState(false);
  const project = card.project;
  const projectId = project.project_id;
  const peer = card.peers.find(p => p.device.device_id === selected) ?? card.peers[0];
  const peerId = peer?.device.device_id;
  const peerName = peer?.device.name ?? 'your collaborator';
  const pairingApproval = peer ? status.pairing_approvals.find(item => item.project_id === projectId && item.device_id === peer.device.device_id)?.state ?? 'UNKNOWN' : 'UNKNOWN';
  const online = (peer ? status.network?.peers[peer.device.peer_id]?.online : false) ?? false;
  const job = status.jobs.find(j => j.project_id === projectId);
  const busy = !status.ready || status.closing || status.jobs.some(j => j.running && (!j.project_id || j.project_id === projectId));
  const check = status.checks.find(c => c.result.project_id === projectId && c.device_id === peerId);
  const divergence = (check?.fresh && check.result.snapshot_status === 'DIVERGED') || peer?.known_lineage === 'DIVERGED';
  const blockedCode = status.jobs.find(item => item.project_id === projectId && !!item.outcome?.receive_blocked)?.outcome?.receive_blocked;
  const resolveDivergence = divergence || blockedCode === 'DIVERGED';
  const resolveLocalChanges = blockedCode === 'LOCAL_CHANGES';
  const resolutionJob = status.jobs.find(item => item.project_id === projectId && item.label === "Use collaborator's version");
  const usedTransferId = resolutionJob && !resolutionJob.running && !resolutionJob.error && typeof resolutionJob.outcome?.transfer_id === 'string' ? String(resolutionJob.outcome.transfer_id) : null;
  const peerHasVersion = card.peers.some(item => !!item.announcement || !!item.offline_head || !!item.known_lineage);
  const firstShareBlocked = !card.local_snapshot && (card.root_empty || peerHasVersion);
  const remoteAhead = online && peer?.known_lineage === 'REMOTE_AHEAD';
  const offlineHead = peer?.offline_head;
  const showOfflineHead = !!offlineHead && !!peer && !online && !remoteAhead
    && (!peer.announcement || Date.parse(offlineHead.created_at) > Date.parse(peer.announcement.created_at));
  const transfer = card.transfer;
  const resumable = transfer?.state === 'PAUSED' && peerId === transfer.source_device_id;
  const receiving = transfer?.state === 'RECEIVING' || transfer?.state === 'APPLYING';
  const percent = transfer?.total_bytes ? Math.min(100, Math.round(transfer.completed_bytes * 100 / transfer.total_bytes)) : 0;
  const authoredHere = card.local_snapshot?.author_device_id === status.device?.device_id;
  const receipt = !!authoredHere && !!peer && !!card.local_snapshot && (
    (project.verified_peer_id === peer.device.peer_id && project.verified_snapshot_id === card.local_snapshot.snapshot_id)
    || !!status.network?.received_snapshots.some(item => item.project_id === projectId && item.peer_id === peer.device.peer_id && item.snapshot_id === card.local_snapshot?.snapshot_id)
  );
  const sharedLatest = !!card.local_snapshot && status.jobs.some(item => item.project_id === projectId && item.outcome?.snapshot_id === card.local_snapshot?.snapshot_id);
  const waitingFirstVersion = !!peer && card.root_empty && !card.local_snapshot && !project.latest_local_snapshot
    && !project.recovery_required && !peer.announcement
    && (!transfer || transfer.state === 'COMPLETE')
    && !status.jobs.some(item => item.project_id === projectId && item.running)
    && pairingApproval !== 'PENDING' && pairingApproval !== 'NOT_REACHABLE';
  const rootMissing = !card.root_available;
  const disabledReceive = busy || !peer || !online || !dawClosed || project.recovery_required || divergence || rootMissing;
  const candidates = status.pairing.project_id === projectId ? status.pairing.requests : [];
  const pairingOpen = status.pairing.project_id === projectId;
  const backend = status.project_backends[projectId]?.backend;
  const inviteState = status.supabase_states[projectId] ?? 'NOT_CONFIGURED';
  const inviteReady = backend === 'SUPABASE' && ['CLAIMED', 'MEMBER'].includes(inviteState);
  const inviteJob = status.jobs.find(item => item.project_id === projectId && item.label === 'Copy invite link');
  const copyInviteSuccess = !!inviteJob && !inviteJob.running && !inviteJob.error && inviteJob.outcome?.invite_copied === true;
  const chooseProjectRoot = async () => {
    try {
      const folder = await chooseFolder();
      if (folder) await act({ kind: 'change_project_root', project_id: projectId, folder });
    } catch (err) { setFolderError(String(err)); }
  };
  const openProject = async () => {
    try { await openProjectFolder(projectId); }
    catch (err) { setFolderError(String(err)); }
  };
  const share = () => act({ kind: 'send', project_id: projectId });

  // One problem at a time, most blocking first; each routes to the Advanced section that can fix it.
  let problem: Problem | null = null;
  if (peer && pairingApproval !== 'PENDING' && pairingApproval !== 'NOT_REACHABLE' && !online && !(job?.running && job.label === 'Check connection'))
    problem = { tone: 'warn', title: `TrackMesh can't reach ${peerName}.`, body: `Ask ${peerName} to open TrackMesh. If it still fails, check the connection.`, fix: 'Fix connection', section: 'connection' };
  else if (card.missing_functions.length)
    problem = { tone: 'danger', title: 'The invite service needs an update.', body: 'Some invite service functions are missing.', fix: 'See invite service', section: 'service' };
  else if (card.grant_problem === 'UNREACHABLE' || inviteState === 'UNREACHABLE')
    problem = { tone: 'warn', title: "The invite service can't be reached.", body: 'Direct sharing still works. Invite links and version notices need the service.', fix: 'Check invite service', section: 'service' };
  else if (!peer && !inviteReady && inviteState !== 'UNKNOWN' && inviteState !== 'WAITING_FOR_GRANT')
    problem = { tone: 'warn', title: 'Invite links need the invite service.', body: 'Add your Supabase address once, then Invite collaborator works.', fix: 'Set up invite service', section: 'service' };

  const badge = pairingApproval === 'PENDING' ? { text: 'Waiting for approval', tone: '' }
    : online ? { text: `Connected · ${peerName}`, tone: 'online' }
    : peer ? { text: `Can't reach ${peerName}`, tone: 'warn' }
    : { text: 'No collaborator yet', tone: '' };

  return <article id={`project-${projectId}`} className={`project-card${focusProjectId === projectId ? ' focus-highlight' : ''}`}>
    <header className="card-title">
      <div className="card-name"><h2>{project.name}</h2><button className="path-link" title="Open in Explorer" onClick={() => void openProject()}>{displayPath(project.root)}</button></div>
      <span className={`badge ${badge.tone}`}>{badge.text}</span>
    </header>

    {rootMissing && <section className="notice danger"><strong>Project folder not found</strong><p>It may have been moved, renamed or on a drive that is not connected.</p><div className="actions"><button className="primary" disabled={busy} onClick={() => void chooseProjectRoot()}>Locate folder</button></div></section>}
    {project.recovery_required && <section className="notice danger"><strong>The last Get did not finish.</strong><p>Close this project in your DAW, then recover it before getting another version.</p>
      <label className="confirmation"><input type="checkbox" checked={dawClosed} disabled={busy} onChange={e => setDawClosed(e.target.checked)} /> I closed this project in my DAW</label>
      <div className="actions"><button className="primary" disabled={busy || !dawClosed} onClick={() => act({ kind: 'recover', project_id: projectId, daw_closed: dawClosed })}>Recover</button></div></section>}
    {problem && <section className={`problem ${problem.tone}`} role="alert"><div><strong>{problem.title}</strong><p>{problem.body}</p></div>
      <button className="primary" onClick={() => openAdvanced(projectId, problem.section)}>{problem.fix} →</button></section>}
    {!peer && inviteState === 'WAITING_FOR_GRANT' && <Waiting>Waiting for your collaborator's TrackMesh to confirm this device with the invite service. Keep both apps open.</Waiting>}
    {!peer && backend === 'SUPABASE' && inviteState === 'UNKNOWN' && <Waiting>Connecting to the invite service…</Waiting>}
    {status.presence[projectId]?.map(warning => <p key={warning.device_id} className={`notice ${warning.both_working ? 'danger' : ''}`} role="status">{warning.line}{!warning.both_working && ` (since ${new Date(warning.since * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })})`}</p>)}
    {status.plugin_handoff?.project_id === projectId && <p className="notice" role="status">Save and close this project in your DAW, then press Get {peerName}'s version.</p>}
    {folderError && <p className="error">{folderError}</p>}

    {candidates.length > 0 && <section className="approval-box"><h3>Someone opened your invite link</h3>{candidates.map(candidate => <div key={candidate.request_id}>
      <p><strong>{candidate.name}</strong> · device {short(candidate.device_id)}</p>
      <p className="muted">Approve only if this is the person you invited. Approval gives them this project.</p>
      <div className="actions"><button className="primary" disabled={busy} onClick={() => window.confirm(`Approve ${candidate.name} (${short(candidate.device_id)}) for ${project.name}?`) && act({ kind: 'approve_pairing', project_id: projectId, request_id: candidate.request_id })}>Approve</button>
        <button disabled={busy} onClick={() => act({ kind: 'reject_pairing', project_id: projectId, request_id: candidate.request_id })}>Reject</button></div></div>)}</section>}

    {!peer && <section className="invite-flow"><h3>Invite your collaborator</h3>
      <p className="muted">Copies a private link. Send it to one person; it works once, for 15 minutes.</p>
      <button className="primary" disabled={busy || !inviteReady} onClick={() => act({ kind: 'copy_invite_link', project_id: projectId })}>Invite collaborator</button></section>}
    {pairingOpen && !candidates.length && <Waiting action={<button disabled={busy} onClick={() => act({ kind: 'close_pairing', project_id: projectId })}>Stop waiting</button>}>
      Waiting for your collaborator to open the link{status.pairing.remaining_seconds > 0 ? ` · ${Math.ceil(status.pairing.remaining_seconds / 60)} min left` : ''}. Keep TrackMesh open.</Waiting>}
    {!pairingOpen && !candidates.length && !!inviteJob && !inviteJob.running && <p className="muted">Collaborator opened the link but nothing appeared? <button className="text-button" disabled={busy} onClick={() => act({ kind: 'open_pairing', project_id: projectId })}>Reopen approval window</button></p>}
    {card.grant_problem === 'REJECTED' && peer && <p className="muted">The invite service didn't accept {peerName} (the project may already have 3 devices, or too many requests). Direct sharing still works.</p>}

    {remoteAhead && peer?.announcement && <div className="new-version-notice" role="status">New version from {peerName} · {time(peer.announcement.created_at)}</div>}
    {showOfflineHead && offlineHead && <div className="new-version-notice" role="status">New version from {peerName} · {time(offlineHead.created_at)} · available when {peerName} is online</div>}

    {peer && (resolveDivergence || resolveLocalChanges) && <section className="notice resolution-box" role="region" aria-label="Resolve project versions">
      <h3>{resolveLocalChanges && !resolveDivergence ? "This folder has changes you haven't shared" : `Your version and ${peerName}'s version both changed`}</h3>
      <p>TrackMesh does not merge projects. Choose which version to keep.</p>
      {(!online || !dawClosed) && <p className="muted">{!online ? `Open TrackMesh on both PCs to use ${peerName}'s version.` : 'Close the project in your DAW and tick “I closed this project in my DAW” below.'}</p>}
      <div className="actions">
        <button className="primary" disabled={busy || !online || !dawClosed} onClick={() => window.confirm(`Replace this folder with ${peerName}'s version? Your current files are saved first and kept next to the project.`) && act({ kind: 'use_peer_version', project_id: projectId, device_id: peer.device.device_id, daw_closed: dawClosed })}>Use {peerName}'s version</button>
        <button disabled={busy || rootMissing || project.recovery_required} onClick={() => { setKeepMinePending(true); void share(); }}>Keep mine</button>
      </div>
      {keepMinePending && job?.label === 'Share current version' && !job.running && !job.error && <p role="status">Ask {peerName} to choose ‘Use {status.device?.name ?? 'your device'}'s version’.</p>}
    </section>}
    {peer && usedTransferId && <div className="delivery-receipt" role="status">Now using {peerName}'s version ✓ <button className="text-button" onClick={() => void openPreviousVersion(projectId, usedTransferId).catch(error => setFolderError(String(error)))}>Open my previous files</button></div>}

    {peer && pairingApproval !== 'PENDING' && <>
      {card.peers.length > 1 && <label className="peer-picker">Collaborator <select value={peerId ?? ''} disabled={busy} onChange={e => { setSelected(e.target.value); setDawClosed(false); }}>
        {card.peers.map(p => <option key={p.device.device_id} value={p.device.device_id}>{p.device.name}</option>)}</select></label>}
      <section className="share-flow">
        <div className="pane"><h3>Send my changes</h3>
          <p className="muted">Saves this folder as a new version {peerName} can get.</p>
          {firstShareBlocked
            ? <p className="muted">{card.root_empty && !peerHasVersion ? 'Add your project files to this folder first.' : `Get ${peerName}'s version first.`}</p>
            : <button className={remoteAhead ? '' : 'primary'} disabled={busy || rootMissing || project.recovery_required} onClick={() => void share()}>Share current version</button>}
          {!online && !firstShareBlocked && <p className="muted">{peerName} can get it when you are both online.</p>}
        </div>
        <div className="pane"><h3>Get {peerName}'s changes</h3>
          {status.presence[projectId]?.filter(warning => warning.device_id === peer.device.device_id).map(warning => <p key={warning.device_id} className="muted" role="status">{warning.get_line}</p>)}
          <label className="confirmation"><input type="checkbox" checked={dawClosed} disabled={busy} onChange={e => setDawClosed(e.target.checked)} /> I closed this project in my DAW</label>
          <button className={remoteAhead ? 'primary' : ''} disabled={disabledReceive} onClick={() => act({ kind: 'receive', project_id: projectId, device_id: peer.device.device_id, snapshot_id: resumable ? transfer.snapshot_id : null, daw_closed: dawClosed })}>{resumable ? 'Resume getting' : `Get ${peerName}'s version`}</button>
          {!online && <p className="muted">Available when {peerName} is online.</p>}
        </div>
      </section></>}

    {authoredHere && sharedLatest && peer && card.local_snapshot && !receipt && <Waiting>Shared. {peerName} hasn't got this version yet.</Waiting>}
    {receipt && peer && <div className="delivery-receipt" role="status">Received by {peerName} ✓</div>}
    {!receiving && pairingApproval === 'PENDING' && peer && <Waiting>Waiting for {peerName} to approve this device…</Waiting>}
    {!receiving && pairingApproval === 'NOT_REACHABLE' && peer && <section className="notice" role="status"><p>{peerName} hasn't approved this device yet. Ask them to open TrackMesh; if approval timed out, they can press ‘Reopen approval window’. Then press Try again.</p><div className="actions"><button disabled={busy} onClick={() => act({ kind: 'request_pairing', project_id: projectId, device_id: peer.device.device_id })}>Try again</button></div></section>}
    {waitingFirstVersion && peer && <Waiting>Waiting for {peerName} to share the first version…</Waiting>}
    {transfer && transfer.state !== 'COMPLETE' && <div className="transfer">
      <div><span>{receiving ? (transfer.state === 'APPLYING' ? 'Putting the files in place…' : `Getting ${peerName}'s version…`) : transfer.state === 'PAUSED' ? 'Paused · press Resume getting when you are both online' : transfer.state.replaceAll('_', ' ')}</span>
        <span>{bytes(transfer.completed_bytes)} / {bytes(transfer.total_bytes)}{transfer.total_bytes ? ` · ${percent} %` : ''}</span></div>
      <progress max={Math.max(transfer.total_bytes, 1)} value={transfer.completed_bytes} />
      {transfer.error && <p className="error">{transfer.error}</p>}</div>}
    {inviteJob?.running && <InProgress>Creating invite link…</InProgress>}
    {copyInviteSuccess && inviteJob && <InviteCopiedNotice job={inviteJob} />}
    {copyInviteSuccess && typeof inviteJob?.outcome?.cleared_pending === 'string' && <div className="job" role="status">Pending approvals for {inviteJob.outcome.cleared_pending} were cleared; only one project can accept new devices at a time.</div>}
    {job && !(job.label === 'Copy invite link' && (job.running || copyInviteSuccess)) && !(job.label === "Get collaborator's version" && job.running && receiving)
      && <JobNotice job={job} startedAt={jobStartedAt[job.job_id]} collaboratorName={peer?.device.name} onStop={() => void stopCheck(projectId).catch(err => setFolderError(String(err)))} />}

    <footer className="card-footer">
      {peer && <button className="text-button" disabled={busy || !inviteReady} title={inviteReady ? 'Up to 3 devices per project' : 'Needs the invite service'} onClick={() => act({ kind: 'copy_invite_link', project_id: projectId })}>Invite another collaborator</button>}
      <button className="text-button" onClick={() => openAdvanced(projectId, 'connection')}>Advanced →</button>
    </footer>
  </article>;
}
