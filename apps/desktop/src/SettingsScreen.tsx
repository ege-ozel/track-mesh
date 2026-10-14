import { useEffect, useState } from 'react';
import { chooseProjectsLocation, openProjectsLocation, setUpnpMapping } from './api';
import type { Action, DesktopStatus } from './api';
import { StorageSettings } from './StorageSettings';
import { displayPath } from './format';

type Act = (action: Action) => Promise<boolean>;

export const ZOOM_LEVELS = [{ value: 1, label: 'Normal' }, { value: 1.15, label: 'Large' }, { value: 1.3, label: 'Larger' }] as const;

function DeviceName({ name, busy, act }: { name: string; busy: boolean; act: Act }) {
  const [value, setValue] = useState(name);
  useEffect(() => setValue(name), [name]);
  return <section className="settings-block"><h3>Your name</h3>
    <label className="field">Shown to your collaborators<div className="input-row"><input maxLength={80} value={value} disabled={busy} onChange={event => setValue(event.target.value)} />
      <button disabled={busy || !value.trim() || value.trim() === name} onClick={() => void act({ kind: 'set_device_name', name: value.trim() })}>Save</button></div></label>
  </section>;
}

export function SettingsScreen({ status, busy, act, poll, onError, zoom, setZoom }: {
  status: DesktopStatus | null; busy: boolean; act: Act; poll: () => Promise<void>; onError: (message: string) => void;
  zoom: number; setZoom: (value: number) => void;
}) {
  const [projectsLocation, setProjectsLocation] = useState('');
  useEffect(() => setProjectsLocation(status?.projects_location ?? ''), [status?.projects_location]);
  const upnp = status?.network?.port_mapping;
  return <section className="settings-screen">
    <DeviceName name={status?.device?.name ?? ''} busy={busy} act={act} />

    <section className="settings-block"><h3>Share when I save in the DAW</h3>
      <p className="muted">Needs the TRACKMESH plugin in your DAW project. When on, every save in the DAW (Ctrl+S) sends a new version to your collaborator by itself; some DAWs also autosave, which counts as a save. When off, nothing is sent until you press Share current version. You can also switch this in the plugin window.</p>
      {status?.daw_plugin_settings.map(project => <div className="toggle-row" key={project.project_id}>
        <div><strong>{project.name}</strong><span className="muted">{project.auto_share ? 'On · every DAW save is shared' : 'Off · shared only when you press Share'}</span></div>
        <label className="switch"><input type="checkbox" checked={project.auto_share} disabled={busy || !status.ready} onChange={event => void act({ kind: 'set_auto_share', project_id: project.project_id, enabled: event.currentTarget.checked })} /> Share on save</label>
      </div>)}
      {status?.ready && !status.daw_plugin_settings.length && <p className="muted">Share a project folder first.</p>}
    </section>

    <section className="settings-block"><h3>Connection</h3>
      <label className="switch"><input type="checkbox" checked={upnp?.consent_enabled ?? false} disabled={busy} onChange={event => { void setUpnpMapping(event.currentTarget.checked).then(poll).catch(err => onError(String(err))); }} /> Set up my router automatically (UPnP)</label>
      <p className="muted">Needed when your collaborator is on a different internet connection. Opens one port for TrackMesh only; it closes when you quit TrackMesh.{upnp?.consent_enabled && upnp.state === 'MAPPED' ? ' Router set up ✓' : upnp?.consent_enabled && upnp.state === 'FAILED' ? " The router didn't allow it — see Advanced → Connection." : ''}</p>
    </section>

    <section className="settings-block"><h3>Where new projects go</h3>
      <p className="muted">Projects you get from an invite link are saved in this folder.</p>
      <div className="input-row"><button className="path-link" title="Open in Explorer" onClick={() => void openProjectsLocation().catch(err => onError(String(err)))}>{displayPath(projectsLocation)}</button>
        <button disabled={busy} onClick={async () => { try { const value = await chooseProjectsLocation(); if (value) { setProjectsLocation(value); await poll(); } } catch (err) { onError(String(err)); } }}>Change…</button></div>
    </section>

    <section className="settings-block"><h3>Text size</h3>
      <div className="segmented" role="radiogroup" aria-label="Text size">{ZOOM_LEVELS.map(level => <button key={level.label} role="radio" aria-checked={zoom === level.value} className={zoom === level.value ? 'on' : ''} onClick={() => setZoom(level.value)}>{level.label}</button>)}</div>
      <p className="muted">Also Ctrl + plus, Ctrl + minus and Ctrl + 0.</p>
    </section>

    <StorageSettings busy={busy} act={act} />
    <p className="muted">Connection details, invite service and manual setups are in Advanced.</p>
  </section>;
}
