import { useCallback, useEffect, useState } from 'react';
import { getStorage, openPreviousVersion } from './api';
import type { Action, StorageSummary } from './api';
import { t } from './i18n';

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
  return <section className="settings-block"><h3>{t('Storage', 'Depolama')}</h3>
    <p>{t("The app automatically keeps file data for your latest version, each collaborator's latest received version and unfinished Gets. Signed history is kept.",
      'Uygulama son sürümünün, her iş arkadaşından alınan son sürümün ve bitmemiş almaların dosya verisini kendisi tutar. İmzalı geçmiş saklanır.')}</p>
    {storage?.cache_bytes != null && <p>{t('App file data:', 'Uygulama dosya verisi:')} <strong>{size(storage.cache_bytes)}</strong></p>}
    <p>{t("Previous files next to your project are removed only here. Each project's newest backup is kept.", 'Projenin yanındaki önceki dosyalar yalnızca buradan silinir. Her projenin en yeni yedeği tutulur.')}</p>
    <button disabled={busy || loading} onClick={() => void refresh()}>{loading ? t('Reading backup sizes…', 'Yedek boyutları okunuyor…') : t('Refresh backup sizes', 'Yedek boyutlarını yenile')}</button>
    {error && <p className="error">{error}</p>}
    {storage?.problems.map(problem => <p className="error" key={problem}>{problem}</p>)}
    {storage && !storage.backups.length && <p className="muted">{t('No completed backups available.', 'Tamamlanmış yedek yok.')}</p>}
    {!!storage?.backups.length && <><p><label><input type="checkbox" disabled={busy || loading} checked={storage.backups.some(b => !b.protected) && storage.backups.filter(b => !b.protected).every(b => selected.includes(b.transfer_id))}
      onChange={event => setSelected(event.target.checked ? storage.backups.filter(b => !b.protected).map(b => b.transfer_id) : [])} /> {t('Select all old backups', 'Tüm eski yedekleri seç')}</label></p>
      <ul>{storage.backups.map(backup => <li key={backup.transfer_id}>
        <label><input type="checkbox" disabled={busy || loading || backup.protected} checked={selected.includes(backup.transfer_id)} onChange={event => setSelected(ids => event.target.checked ? [...ids, backup.transfer_id] : ids.filter(id => id !== backup.transfer_id))} />
          {backup.project_name} · {size(backup.bytes)}{backup.modified_at != null && ` · ${new Date(backup.modified_at * 1000).toLocaleString()}`} · {backup.protected ? t('Kept', 'Tutuluyor') : t('Old backup', 'Eski yedek')}</label>
        <button disabled={busy || loading} onClick={() => void openPreviousVersion(backup.project_id, backup.transfer_id).catch(err => setError(String(err)))}>{t('Open', 'Aç')}</button>
        <p className="muted"><code>{backup.path.replace(/^\\\\\?\\/, '')}</code></p>
      </li>)}</ul>
      <button className="danger" disabled={busy || loading || !chosen.length || chosen.length > 1024} onClick={() => void act({ kind: 'delete_backups', selected: chosen.map(({ transfer_id, path, bytes }) => ({ transfer_id, path, bytes })) })}>{t('Delete selected backups', 'Seçili yedekleri sil')} · {size(total)}</button>
    </>}
    <p className="muted">{t('Get asks before downloading more than 1 GB of new data. Verified local chunks do not count toward that download.', "Alma, 1 GB'tan fazla yeni veri indirmeden önce sorar. Bilgisayarda zaten doğrulanmış parçalar buna sayılmaz.")}</p>
  </section>;
}
