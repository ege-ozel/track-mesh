import { useCallback, useEffect, useRef, useState } from 'react';
import { AdvancedScreen } from './AdvancedScreen';
import { cancelInviteLink, chooseFolder, chooseInviteParentFolder, closeApp, getStatus, openInviteParentFolder, pasteInviteLink, setUpnpMapping, submitAction } from './api';
import type { Action, DesktopStatus } from './api';
import { JobNotice } from './Jobs';
import { Card } from './ProjectCard';
import type { AdvancedSection } from './ProjectCard';
import { SettingsScreen, ZOOM_LEVELS } from './SettingsScreen';
import { displayPath, short } from './format';

type Screen = 'projects' | 'advanced' | 'settings';

const SCREEN_TITLES: Record<Screen, [string, string]> = {
  projects: ['Projects', 'Each card is one DAW project folder you share with your collaborator.'],
  advanced: ['Advanced', 'Connection, invite service and version details. TrackMesh sends you here when something needs fixing.'],
  settings: ['Settings', 'Your name, sharing and storage.'],
};

function readZoom() {
  try {
    const value = Number(window.localStorage.getItem('trackmesh.zoom'));
    return ZOOM_LEVELS.some(level => level.value === value) ? value : 1;
  } catch { return 1; }
}

export default function App() {
  const [status, setStatus] = useState<DesktopStatus | null>(null);
  const [error, setError] = useState('');
  const [offset, setOffset] = useState(0);
  const [setup, setSetup] = useState<'link' | 'join' | null>(null);
  const [screen, setScreen] = useState<Screen>('projects');
  const [advancedProjectId, setAdvancedProjectId] = useState<string | null>(null);
  const [advancedSection, setAdvancedSection] = useState<AdvancedSection | null>(null);
  const [zoom, setZoom] = useState(readZoom);
  const [folder, setFolder] = useState('');
  const [name, setName] = useState('');
  const [autoName, setAutoName] = useState(false);
  const [uuid, setUuid] = useState('');
  const [mailbox, setMailbox] = useState('');
  const [relay, setRelay] = useState('');
  const [inviteServiceUrl, setInviteServiceUrl] = useState('');
  const [inviteServiceKey, setInviteServiceKey] = useState('');
  const [inviteParentFolder, setInviteParentFolder] = useState<string | null>(null);
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
  // Text size: the root zoom keeps every spacing and font in proportion.
  useEffect(() => {
    document.documentElement.style.zoom = String(zoom);
    try { window.localStorage.setItem('trackmesh.zoom', String(zoom)); } catch { /* per-viewer convenience only */ }
  }, [zoom]);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey) return;
      const index = ZOOM_LEVELS.findIndex(level => level.value === zoom);
      if (event.key === '+' || event.key === '=') setZoom(ZOOM_LEVELS[Math.min(ZOOM_LEVELS.length - 1, index + 1)].value);
      else if (event.key === '-') setZoom(ZOOM_LEVELS[Math.max(0, index - 1)].value);
      else if (event.key === '0') setZoom(1);
      else return;
      event.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [zoom]);
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
  }, [focusProjectId, scrolledFocusId, status?.page, screen]);
  useEffect(() => {
    if (!scrolledFocusId) return;
    const timeout = window.setTimeout(() => { setFocusProjectId(null); setScrolledFocusId(null); }, 2500);
    return () => window.clearTimeout(timeout);
  }, [scrolledFocusId]);
  useEffect(() => {
    if (!advancedSection) return;
    const timeout = window.setTimeout(() => setAdvancedSection(null), 2500);
    return () => window.clearTimeout(timeout);
  }, [advancedSection, advancedProjectId]);
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
  const go = (next: Screen) => { setScreen(next); setAdvancedSection(null); window.scrollTo({ top: 0 }); };
  const openAdvanced = (projectId: string, section: AdvancedSection) => {
    setAdvancedProjectId(projectId);
    setAdvancedSection(section);
    setScreen('advanced');
    window.scrollTo({ top: 0 });
  };
  const startLink = () => { go('projects'); setSetup('link'); setFolder(''); setUuid(crypto.randomUUID()); setName(''); setAutoName(false); setMailbox(''); setRelay(''); };
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
        setScrolledFocusId(null);
        setFocusProjectId(projectId);
      }
    }
  };
  const projects = status?.page?.projects ?? [];
  const projectlessJobs = status?.jobs.filter(j => !j.project_id) ?? [];
  const firstRun = !!status?.ready && !projects.length && !setup && offset === 0;
  const [title, subtitle] = SCREEN_TITLES[screen];
  const runtimeError = error || status?.error || status?.network?.error;
  const nav = (target: Screen, label: string, note?: string) => <>
    <button className={screen === target ? 'rail-nav active' : 'rail-nav'} aria-current={screen === target ? 'page' : undefined} onClick={() => go(target)}>{label}</button>
    {note && <span className="rail-note">{note}</span>}</>;

  return <div className="app-shell">
    <aside className="rail" aria-label="Main">
      <div className="brand"><span className="brand-mark">TM</span><span>TRACKMESH</span></div>
      {nav('projects', 'Projects')}
      {nav('advanced', 'Advanced', 'Connection and technical details')}
      {nav('settings', 'Settings')}
      <div className="rail-bottom">
        <div className="device"><span className={`status-dot ${status?.ready ? 'active' : ''}`} /><span>{status?.device?.name ?? 'Starting…'}</span><small>{short(status?.device?.device_id)}</small></div>
        <button title="Confirm who will lose access to your versions, finish accepted work, then quit TRACKMESH. Closing the window keeps it in the system tray." disabled={status?.closing} onClick={() => void closeApp().catch(err => setError(String(err)))}>Quit TrackMesh</button>
      </div>
    </aside>
    <main>
      <header className="page-head">
        <div><h1>{firstRun && screen === 'projects' ? 'Share a DAW project with one collaborator' : title}</h1>
          <p className="muted">{firstRun && screen === 'projects' ? 'Files go directly between your PCs. Nothing is uploaded to a cloud drive.' : subtitle}</p></div>
        {screen === 'projects' && !firstRun && <div className="actions">
          <button className="primary" disabled={busy} onClick={startLink}>Share a project folder</button>
          <button disabled={busy} onClick={() => void pasteInvite()}>Open an invite link</button></div>}
        {screen === 'advanced' && <button className="text-button" onClick={() => go('projects')}>← Back to Projects</button>}
      </header>
      {runtimeError && <div className="notice danger" role="alert">{runtimeError}</div>}
      {status?.closing && <div className="notice">Work finished. TRACKMESH is shutting down safely.</div>}

      {status?.invite_pending && <div className="invite-overlay"><section className="invite-dialog" role="dialog" aria-modal="true" aria-labelledby="invite-title"><h2 id="invite-title">Open invite link</h2>
        {status.jobs.filter(job => job.label === 'Accept invite link' && job.error).slice(-1).map(job => <div className="notice danger" role="alert" key={job.job_id}><strong>{job.error}</strong><p>The link may have expired or already been used. Ask for a new link, or Cancel.</p></div>)}
        <p>The project will be saved in <button className="path-link" title="Open in Explorer" onClick={() => void openInviteParentFolder().catch(err => setError(String(err)))}>{displayPath(inviteParentFolder ?? status.projects_location)}\[project name]</button>.</p>
        {status.invite_needs_service_fields && <><label className="field">Supabase Project URL<input autoComplete="off" value={inviteServiceUrl} onChange={event => setInviteServiceUrl(event.target.value)} placeholder="https://your-project.supabase.co" /></label>
        <label className="field">Publishable key<input type="password" autoComplete="new-password" value={inviteServiceKey} onChange={event => setInviteServiceKey(event.target.value)} /></label></>}
        <p className="muted">After you accept, the sender approves you and the project downloads by itself.</p>
        <div className="actions">
          <button disabled={busy} onClick={async () => { try { const value = await chooseInviteParentFolder(); if (value) setInviteParentFolder(value); } catch (err) { setError(String(err)); } }}>Choose another folder…</button>
          <span className="spacer" />
          <button disabled={!status.ready || status.closing || status.jobs.some(job => job.running && job.label === 'Accept invite link')} onClick={() => void cancelInviteLink().then(poll).catch(err => setError(String(err)))}>Cancel</button>
          <button className="primary" disabled={busy || (status.invite_needs_service_fields && (!inviteServiceUrl.trim() || !inviteServiceKey.trim()))} onClick={() => {
            setSetup(null); setScreen('projects'); setOffset(0);
            focusBaseline.current = new Set(status.page?.projects.map(item => item.project.project_id) ?? []);
            void act({ kind: 'complete_invite_link', ...(status.invite_needs_service_fields ? { supabase_url: inviteServiceUrl.trim(), supabase_anon_key: inviteServiceKey.trim() } : {}) });
          }}>Accept</button></div>
      </section></div>}
      {upnpConsentPrompt && <div className="invite-overlay"><section className="invite-dialog upnp-consent-dialog" role="dialog" aria-modal="true" aria-labelledby="upnp-consent-title"><h2 id="upnp-consent-title">Router setup</h2>
        <p>Let TrackMesh set up your router automatically? Needed when your collaborator is on a different internet connection. Opens one port only for TrackMesh; only paired devices can use it. You can change this later in Settings.</p>
        <div className="actions"><span className="spacer" /><button onClick={() => void answerUpnpConsent(false)}>Not now</button><button className="primary" onClick={() => void answerUpnpConsent(true)}>Allow (recommended)</button></div>
      </section></div>}

      {screen === 'settings' && <SettingsScreen status={status} busy={busy} act={act} poll={poll} onError={setError} zoom={zoom} setZoom={setZoom} />}
      {screen === 'advanced' && status && <AdvancedScreen status={status} projectId={advancedProjectId} section={advancedSection} jobStartedAt={jobStartedAt} act={act}
        pickProject={projectId => { setAdvancedProjectId(projectId); setAdvancedSection(null); }}
        openInvitationFile={() => { go('projects'); setSetup('join'); setFolder(''); }} onError={setError} />}
      {screen === 'projects' && <>
        {setup && <section className="setup-panel"><h2>{setup === 'join' ? 'Open an invitation file' : 'Share a project folder'}</h2>
          {setup === 'link' && <label className="field">Project name<input value={name} maxLength={320} onChange={e => { setName(e.target.value); setAutoName(false); }} placeholder="Track 07" /></label>}
          <label className="field">{setup === 'join' ? 'Empty folder for the project' : 'Folder with your Ableton or FL project'}<div className="input-row"><input value={folder} onChange={e => setFolder(e.target.value)} placeholder="D:\Music\Track 07" /><button onClick={() => void browse()}>Browse…</button></div></label>
          {setup === 'link' && <details className="form-advanced"><summary>Advanced project settings</summary><div className="field-stack"><label className="field">Permanent project UUID<div className="input-row"><input value={uuid} onChange={e => setUuid(e.target.value)} /><button onClick={() => setUuid(crypto.randomUUID())}>New UUID</button></div></label>
            <label className="field">HTTPS mailbox (optional)<input value={mailbox} onChange={e => setMailbox(e.target.value)} placeholder="https://mailbox.example.com" /></label><label className="field">Pinned relay address (optional)<input value={relay} onChange={e => setRelay(e.target.value)} placeholder="/dns4/relay.example.com/udp/4001/quic-v1/p2p/…" /></label></div></details>}
          {setup === 'join' && <p className="muted">Choose the folder, then open the private invitation file. Your device request is sent automatically; the sender keeps TrackMesh open and approves you. The invitation key stays in the native backend.</p>}
          {setup === 'link' && <p className="muted">Next you invite your collaborator from the project card.</p>}
          <div className="actions"><button className="primary" disabled={busy || !folder || (setup === 'link' && (!name || !uuid))} onClick={() => void saveSetup()}>{setup === 'join' ? 'Open invitation file…' : 'Add project'}</button><button onClick={() => setSetup(null)}>Cancel</button></div>
        </section>}
        {firstRun && <section className="first-run">
          <ol className="first-steps">
            <li><strong>Pick the project folder</strong><span>The folder that holds your Ableton or FL project.</span></li>
            <li><strong>Invite and approve</strong><span>Send a link, then approve your collaborator.</span></li>
            <li><strong>Share and get versions</strong><span>One button each way.</span></li>
          </ol>
          <div className="choices">
            <section className="choice"><span className="who">I have the project</span><h2>Share a project folder</h2>
              <p>Choose the folder with your DAW project. TrackMesh keeps its versions and lets you invite a collaborator.</p>
              <button className="primary" disabled={busy} onClick={startLink}>Choose folder…</button></section>
            <section className="choice"><span className="who">Someone sent me a link</span><h2>Open an invite link</h2>
              <p>Copy the link you received, then press the button. The project downloads after the sender approves you.</p>
              <button disabled={busy} onClick={() => void pasteInvite()}>Paste invite link</button></section>
          </div>
          <p className="muted">Closing the window keeps TrackMesh running next to the clock, so your collaborator can still get your versions.</p>
        </section>}
        <div className="project-list">{status && projects.map(card => <Card key={card.project.project_id} card={card} status={status} jobStartedAt={jobStartedAt} focusProjectId={focusProjectId} act={act} openAdvanced={openAdvanced} />)}</div>
        {(offset > 0 || status?.page?.has_more) && <div className="pagination"><button disabled={!offset || busy} onClick={() => setOffset(Math.max(0, offset - 32))}>Previous</button><span className="muted">Page {offset / 32 + 1}</span><button disabled={!status?.page?.has_more || busy} onClick={() => setOffset(offset + 32)}>Next</button></div>}
      </>}
      {projectlessJobs.map(job => <JobNotice key={job.job_id} job={job} startedAt={jobStartedAt[job.job_id]} />)}
    </main>
  </div>;
}
