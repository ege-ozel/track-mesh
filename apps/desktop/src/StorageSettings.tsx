import { useCallback, useEffect, useState } from 'react';
import { getStorage } from './api';
import type { Action, StorageSummary } from './api';

const size = (value: number) => value >= 1_000_000_000 ? `${(value / 1_000_000_000).toFixed(2)} GB` : `${(value / 1_000_000).toFixed(2)} MB`;

export function StorageSettings({ busy, act }: { busy: boolean; act: (action: Action) => Promise<boolean> }) {
  const [storage, setStorage] = useState<StorageSummary | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const refresh = useCallback(async () => {
    setLoading(true);
    try { setStorage(await getStorage()); setSelected([]); setError(''); }
    catch (error) { setError(String(error)); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { if (!busy) void refresh(); }, [busy, refresh]);
  const chosen = storage?.backups.filter(backup => !backup.protected && selected.includes(backup.transfer_id)) ?? [];
  const total = chosen.reduce((sum, backup) => sum + backup.bytes, 0);
  return <section className="settings-block"><h3>Storage</h3>
    <p>The app automatically keeps file data for your latest version, each collaborator's latest received version and unfinished Gets. Signed history is kept.</p>
    {storage?.cache_bytes != null && <p>App file data: <strong>{size(storage.cache_bytes)}</strong></p>}
    <p>Previous files next to your project are removed only here. Each project's newest backup is kept.</p>
    <button disabled={busy || loading} onClick={() => void refresh()}>{loading ? 'Reading backup sizes…' : 'Refresh backup sizes'}</button>
    {error && <p className="error">{error}</p>}
    {storage?.problems.map(problem => <p className="error" key={problem}>{problem}</p>)}
    {storage && !storage.backups.length && <p className="muted">No completed backups available.</p>}
    {!!storage?.backups.length && <><p><label><input type="checkbox" disabled={busy || loading} checked={storage.backups.some(b => !b.protected) && storage.backups.filter(b => !b.protected).every(b => selected.includes(b.transfer_id))}
      onChange={event => setSelected(event.target.checked ? storage.backups.filter(b => !b.protected).map(b => b.transfer_id) : [])} /> Select all old backups</label></p>
      <ul>{storage.backups.map(backup => <li key={backup.transfer_id}>
        <label><input type="checkbox" disabled={busy || loading || backup.protected} checked={selected.includes(backup.transfer_id)} onChange={event => setSelected(ids => event.target.checked ? [...ids, backup.transfer_id] : ids.filter(id => id !== backup.transfer_id))} />
          {backup.project_name} · {size(backup.bytes)} · {backup.protected ? 'Kept' : 'Old backup'}</label>
        <p className="muted"><code>{backup.path.replace(/^\\\\\?\\/, '')}</code></p>
      </li>)}</ul>
      <button className="danger" disabled={busy || loading || !chosen.length || chosen.length > 1024} onClick={() => void act({ kind: 'delete_backups', selected: chosen.map(({ transfer_id, path, bytes }) => ({ transfer_id, path, bytes })) })}>Delete selected backups · {size(total)}</button>
    </>}
    <p className="muted">Get asks before downloading more than 1 GB of new data. Verified local chunks do not count toward that download.</p>
  </section>;
}
