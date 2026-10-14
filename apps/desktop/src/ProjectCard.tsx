import { useState } from 'react';
import { ask, chooseFolder, openPreviousVersion, openProjectFolder, stopOperation } from './api';
import type { Action, DesktopStatus, ProjectCard } from './api';
import { ErrorDetails, InProgress, InviteCopiedNotice, JobNotice, StopButton, Waiting, friendlyError } from './Jobs';
import { bytes, displayPath, short, time } from './format';
import { named, t } from './i18n';

export type AdvancedSection = 'connection' | 'service' | 'manual' | 'history' | 'access';

type Problem = { tone: 'warn' | 'danger'; title: string; body: string; fix: string; section: AdvancedSection };

const RECEIVE_LABELS = ["Get collaborator's version", "Use collaborator's version", 'Recover installation'];

type TabTone = 'attention' | 'new' | 'warn' | 'ok' | 'idle';

// What a project tab's dot says, most urgent first, so hidden projects still show what needs you.
export function projectState(card: ProjectCard, status: DesktopStatus): { tone: TabTone; label: string } {
  const id = card.project.project_id;
  const online = (peer: ProjectCard['peers'][number]) => status.network?.peers[peer.device.peer_id]?.online ?? false;
  const job = status.jobs.find(j => j.project_id === id);
  if (status.pairing.project_id === id && status.pairing.requests.length) return { tone: 'attention', label: t('Someone is waiting for your approval', 'Onayını bekleyen biri var') };
  if (!card.root_available) return { tone: 'attention', label: t('Project folder not found', 'Proje klasörü bulunamadı') };
  if (card.project.recovery_required) return { tone: 'attention', label: t('The last Get did not finish', 'Son alma işlemi bitmedi') };
  if (card.peers.some(peer => peer.known_lineage === 'DIVERGED') || (!job?.running && job?.outcome?.receive_blocked)) return { tone: 'attention', label: t('Choose which version to keep', 'Hangi sürümün kalacağını seç') };
  const newVersion = card.transfer?.state === 'PAUSED' || card.peers.some(peer => online(peer) ? peer.known_lineage === 'REMOTE_AHEAD'
    : !!peer.offline_head && (!peer.announcement || Date.parse(peer.offline_head.created_at) > Date.parse(peer.announcement.created_at)));
  if (newVersion) return { tone: 'new', label: t('New version to get', 'Alınacak yeni sürüm var') };
  if (!card.peers.length) return { tone: 'idle', label: t('No collaborator yet', 'Henüz iş arkadaşı yok') };
  if (status.pairing_approvals.some(item => item.project_id === id && item.state === 'PENDING')) return { tone: 'idle', label: t('Waiting for approval', 'Onay bekleniyor') };
  return card.peers.some(online) ? { tone: 'ok', label: t('Connected', 'Bağlı') } : { tone: 'warn', label: t("Can't reach your collaborator", 'İş arkadaşına ulaşılamıyor') };
}

export function ProjectTabs({ projects, status, selectedId, pick }: {
  projects: ProjectCard[]; status: DesktopStatus; selectedId: string; pick: (projectId: string) => void;
}) {
  return <div className="project-chips" role="tablist" aria-label={t('Projects', 'Projeler')}>{projects.map(item => {
    const id = item.project.project_id;
    const state = projectState(item, status);
    return <button key={id} role="tab" aria-selected={id === selectedId} className={id === selectedId ? 'chip on' : 'chip'} title={state.label} aria-label={`${item.project.name} · ${state.label}`} onClick={() => pick(id)}>
      <span className={`tab-dot ${state.tone}`} aria-hidden="true" />{item.project.name}</button>;
  })}</div>;
}

export function Card({ card, status, jobStartedAt, focusProjectId, act, openAdvanced }: {
  card: ProjectCard; status: DesktopStatus; jobStartedAt: Record<string, number>; focusProjectId: string | null;
  act: (action: Action) => Promise<boolean>; openAdvanced: (projectId: string, section: AdvancedSection) => void;
}) {
  const [selected, setSelected] = useState('');
  const [folderError, setFolderError] = useState('');
  const [keepMinePending, setKeepMinePending] = useState(false);
  const project = card.project;
  const projectId = project.project_id;
  const peer = card.peers.find(p => p.device.device_id === selected) ?? card.peers[0];
  const peerId = peer?.device.device_id;
  const collab = peer?.device.name;
  const peerName = collab ?? 'your collaborator';
  const pairingApproval = peer ? status.pairing_approvals.find(item => item.project_id === projectId && item.device_id === peer.device.device_id)?.state ?? 'UNKNOWN' : 'UNKNOWN';
  const online = (peer ? status.network?.peers[peer.device.peer_id]?.online : false) ?? false;
  // The backend keeps one job per project: the running one or the latest result.
  const job = status.jobs.find(j => j.project_id === projectId);
  const busy = !status.ready || status.closing || status.jobs.some(j => j.running && (!j.project_id || j.project_id === projectId));
  const check = status.checks.find(c => c.result.project_id === projectId && c.device_id === peerId);
  const divergence = (check?.fresh && check.result.snapshot_status === 'DIVERGED') || peer?.known_lineage === 'DIVERGED';
  const blockedCode = job?.outcome?.receive_blocked;
  const resolveDivergence = divergence || blockedCode === 'DIVERGED';
  const resolveLocalChanges = blockedCode === 'LOCAL_CHANGES';
  const usedTransferId = job && job.label === "Use collaborator's version" && !job.running && !job.error && typeof job.outcome?.transfer_id === 'string' ? String(job.outcome.transfer_id) : null;
  const peerHasVersion = card.peers.some(item => !!item.announcement || !!item.offline_head || !!item.known_lineage);
  const firstShareBlocked = !card.local_snapshot && (card.root_empty || peerHasVersion);
  const remoteAhead = online && peer?.known_lineage === 'REMOTE_AHEAD';
  const offlineHead = peer?.offline_head;
  const showOfflineHead = !!offlineHead && !!peer && !online && !remoteAhead
    && (!peer.announcement || Date.parse(offlineHead.created_at) > Date.parse(peer.announcement.created_at));
  const transfer = card.transfer;
  const resumable = transfer?.state === 'PAUSED' && peerId === transfer.source_device_id;
  const receiving = transfer?.state === 'RECEIVING' || transfer?.state === 'APPLYING';
  const upToDate = online && !resumable && (peer?.known_lineage === 'EQUAL' || peer?.known_lineage === 'LOCAL_AHEAD');
  const percent = transfer?.total_bytes ? Math.min(100, Math.round(transfer.completed_bytes * 100 / transfer.total_bytes)) : 0;
  // A version made only of empty files has 0 bytes to download: show it as done once download is
  // over, and as indeterminate while TrackMesh still works out the size.
  const noFileData = !!transfer && transfer.total_bytes === 0;
  const transferDone = transfer?.state === 'APPLYING' || transfer?.state === 'COMPLETE' || transfer?.state === 'RECOVERY_REQUIRED';
  const authoredHere = card.local_snapshot?.author_device_id === status.device?.device_id;
  const receipt = !!authoredHere && !!peer && !!card.local_snapshot && (
    (project.verified_peer_id === peer.device.peer_id && project.verified_snapshot_id === card.local_snapshot.snapshot_id)
    || !!status.network?.received_snapshots.some(item => item.project_id === projectId && item.peer_id === peer.device.peer_id && item.snapshot_id === card.local_snapshot?.snapshot_id)
  );
  const sharedLatest = !!card.local_snapshot && job?.outcome?.snapshot_id === card.local_snapshot.snapshot_id;
  const waitingFirstVersion = !!peer && card.root_empty && !card.local_snapshot && !project.latest_local_snapshot
    && !project.recovery_required && !peer.announcement
    && (!transfer || transfer.state === 'COMPLETE')
    && !job?.running
    && pairingApproval !== 'PENDING' && pairingApproval !== 'NOT_REACHABLE';
  const rootMissing = !card.root_available;
  const disabledReceive = busy || !peer || !online || project.recovery_required || divergence || rootMissing;
  const candidates = status.pairing.project_id === projectId ? status.pairing.requests : [];
  const pairingOpen = status.pairing.project_id === projectId;
  const backend = status.project_backends[projectId]?.backend;
  const inviteState = status.supabase_states[projectId] ?? 'NOT_CONFIGURED';
  const inviteReady = backend === 'SUPABASE' && ['CLAIMED', 'MEMBER'].includes(inviteState);
  const inviteJob = job?.label === 'Copy invite link' ? job : undefined;
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
  const stop = () => void stopOperation(projectId).catch(err => setFolderError(String(err)));
  // One question right before files change, instead of a checkbox to tick first.
  const dawClosed = (what: string) => ask(t('Is this project closed in your DAW?', "Proje DAW'da kapalı mı?"), what, t("Yes, it's closed", 'Evet, kapalı'), t('Cancel', 'Vazgeç'))
    .catch(err => { setFolderError(String(err)); return false; });
  const receive = async () => peer && await dawClosed(t(`TrackMesh will replace the files in this folder with ${peerName}'s version.`, `TrackMesh bu klasördeki dosyaları ${named(collab, 'gen')} sürümüyle değiştirecek.`))
    && act({ kind: 'receive', project_id: projectId, device_id: peer.device.device_id, snapshot_id: resumable ? transfer.snapshot_id : null, daw_closed: true });
  const usePeerVersion = async () => peer && await dawClosed(t(`This folder will be replaced with ${peerName}'s version.\n\nYour current files are saved first and kept next to the project.`,
    `Bu klasör ${named(collab, 'gen')} sürümüyle değiştirilecek.\n\nŞu anki dosyaların önce kaydedilir ve projenin yanında saklanır.`))
    && act({ kind: 'use_peer_version', project_id: projectId, device_id: peer.device.device_id, daw_closed: true });
  const recover = async () => await dawClosed(t('TrackMesh will finish putting the files in place.', 'TrackMesh dosyaları yerleştirmeyi bitirecek.'))
    && act({ kind: 'recover', project_id: projectId, daw_closed: true });

  // One problem at a time, most blocking first; each routes to the Advanced section that can fix it.
  let problem: Problem | null = null;
  if (peer && pairingApproval !== 'PENDING' && pairingApproval !== 'NOT_REACHABLE' && !online && !(job?.running && job.label === 'Check connection'))
    problem = { tone: 'warn', title: t(`TrackMesh can't reach ${peerName}.`, `TrackMesh ${named(collab, 'dat')} ulaşamıyor.`),
      body: t(`Ask ${peerName} to open TrackMesh. If it still fails, check the connection.`, `${named(collab, 'dat')} TrackMesh'i açmasını söyle. Yine olmazsa bağlantıyı kontrol et.`),
      fix: t('Fix connection', 'Bağlantıyı düzelt'), section: 'connection' };
  else if (card.missing_functions.length)
    problem = { tone: 'danger', title: t('The invite service needs an update.', 'Davet servisinin güncellenmesi gerekiyor.'), body: t('Some invite service functions are missing.', 'Davet servisinde bazı fonksiyonlar eksik.'),
      fix: t('See invite service', 'Davet servisine bak'), section: 'service' };
  else if (card.grant_problem === 'UNREACHABLE' || inviteState === 'UNREACHABLE')
    problem = { tone: 'warn', title: t("The invite service can't be reached.", 'Davet servisine ulaşılamıyor.'),
      body: t('Direct sharing still works. Invite links and version notices need the service.', 'Doğrudan paylaşım çalışmaya devam eder. Davet bağlantıları ve sürüm bildirimleri için servis gerekir.'),
      fix: t('Check invite service', 'Davet servisini kontrol et'), section: 'service' };
  else if (!peer && !inviteReady && inviteState !== 'UNKNOWN' && inviteState !== 'WAITING_FOR_GRANT')
    problem = { tone: 'warn', title: t('Invite links need the invite service.', 'Davet bağlantıları için davet servisi gerekir.'),
      body: t('Add your Supabase address once, then Invite collaborator works.', 'Supabase adresini bir kez ekle, sonra davet etme çalışır.'),
      fix: t('Set up invite service', 'Davet servisini kur'), section: 'service' };

  const badge = pairingApproval === 'PENDING' ? { text: t('Waiting for approval', 'Onay bekleniyor'), tone: '' }
    : online ? { text: t(`Connected · ${peerName}`, `Bağlı · ${peerName}`), tone: 'online' }
    : peer ? { text: t(`Can't reach ${peerName}`, `${named(collab, 'dat')} ulaşılamıyor`), tone: 'warn' }
    : { text: t('No collaborator yet', 'Henüz iş arkadaşı yok'), tone: '' };

  const transferError = transfer?.error ?? (job?.error && RECEIVE_LABELS.includes(job.label) ? job.error : null);
  const transferBox = transfer && <div className="transfer">
    <div><span>{receiving
      ? (transfer.state === 'APPLYING' ? t("Putting the files in place… this can't be stopped", 'Dosyalar yerleştiriliyor… bu durdurulamaz') : t(`Getting ${peerName}'s version…`, `${named(collab, 'gen')} sürümü alınıyor…`))
      : transfer.state === 'PAUSED' ? t('Paused · press Resume getting to continue', "Duraklatıldı · devam etmek için Almaya devam et'e bas")
      : transfer.state === 'RECOVERY_REQUIRED' ? t('Not finished · press Recover above', "Bitmedi · yukarıdaki Kurtar'a bas")
      : t('Waiting to start…', 'Başlaması bekleniyor…')}</span>
      <span>{noFileData ? (transferDone ? t('Only empty files · nothing to download', 'Yalnızca boş dosyalar · indirilecek bir şey yok') : '') : `${bytes(transfer.completed_bytes)} / ${bytes(transfer.total_bytes)} · ${percent} %`}</span></div>
    {noFileData && !transferDone ? <progress /> : <progress max={noFileData ? 1 : transfer.total_bytes} value={noFileData ? 1 : transfer.completed_bytes} />}
    {transfer.state === 'RECEIVING' && job?.running && <div className="actions"><StopButton key={job.job_id} onStop={stop} /></div>}
    {transferError && <><p className="error">{friendlyError(transferError, collab) ?? transferError}</p><ErrorDetails raw={transferError} collaboratorName={collab} /></>}</div>;

  // A single status line per card, by priority: work running now, an unfinished Get, the latest
  // result while it is fresh, then what the card is waiting for. Errors stay until the next action.
  const nowSeconds = Math.floor(Date.now() / 1000);
  const resultFresh = !!job && !job.running && !job.outcome?.receive_blocked && (!!job.error || job.outcome?.pairing_state === 'retry' || !!usedTransferId
    || nowSeconds - (job.completed_at ?? nowSeconds) < (copyInviteSuccess ? 75 : 20));
  const statusLine = transfer && receiving ? transferBox
    : inviteJob?.running ? <InProgress>{t('Creating invite link…', 'Davet bağlantısı oluşturuluyor…')}</InProgress>
    : job?.running ? <JobNotice job={job} startedAt={jobStartedAt[job.job_id]} collaboratorName={collab} online={online} onStop={stop} />
    : transfer && transfer.state !== 'COMPLETE' ? transferBox
    : job && resultFresh ? (usedTransferId
        ? <div className="delivery-receipt" role="status">{t(`Now using ${peerName}'s version ✓`, `Artık ${named(collab, 'gen')} sürümü kullanılıyor ✓`)} <button className="text-button" onClick={() => void openPreviousVersion(projectId, usedTransferId).catch(error => setFolderError(String(error)))}>{t('Open my previous files', 'Önceki dosyalarımı aç')}</button></div>
        : copyInviteSuccess ? <><InviteCopiedNotice job={job} />
          {typeof job.outcome?.cleared_pending === 'string' && <div className="job" role="status">{t(`Pending approvals for ${job.outcome.cleared_pending} were cleared; only one project can accept new devices at a time.`, `${job.outcome.cleared_pending} için bekleyen onaylar silindi; aynı anda yalnızca bir proje yeni cihaz kabul edebilir.`)}</div>}</>
        : <JobNotice job={job} startedAt={jobStartedAt[job.job_id]} collaboratorName={collab} online={online} />)
    : peer && pairingApproval === 'PENDING' ? <Waiting>{t(`Waiting for ${peerName} to approve this device…`, `${named(collab, 'gen')} bu cihazı onaylaması bekleniyor…`)}</Waiting>
    : peer && waitingFirstVersion ? <Waiting>{t(`Waiting for ${peerName} to share the first version…`, `${named(collab, 'gen')} ilk sürümü paylaşması bekleniyor…`)}</Waiting>
    : peer && receipt ? <div className="delivery-receipt" role="status">{t(`Received by ${peerName} ✓`, `${peerName} aldı ✓`)}</div>
    : peer && authoredHere && sharedLatest && online ? <Waiting>{t(`Shared. ${peerName} hasn't got this version yet.`, `Paylaşıldı. ${peerName} bu sürümü henüz almadı.`)}</Waiting>
    : null;

  return <article id={`project-${projectId}`} className={`project-card${focusProjectId === projectId ? ' focus-highlight' : ''}`}>
    <header className="card-title">
      <div className="card-name"><h2>{project.name}</h2><button className="path-link" title={t('Open in Explorer', "Gezgin'de aç")} onClick={() => void openProject()}>{displayPath(project.root)}</button></div>
      <span className={`badge ${badge.tone}`}>{badge.text}</span>
    </header>

    {rootMissing && <section className="notice danger"><strong>{t('Project folder not found', 'Proje klasörü bulunamadı')}</strong><p>{t('It may have been moved, renamed or on a drive that is not connected.', 'Taşınmış, adı değişmiş ya da bağlı olmayan bir sürücüde olabilir.')}</p><div className="actions"><button className="primary" disabled={busy} onClick={() => void chooseProjectRoot()}>{t('Locate folder', 'Klasörü bul')}</button></div></section>}
    {project.recovery_required && <section className="notice danger"><strong>{t('The last Get did not finish.', 'Son alma işlemi bitmedi.')}</strong><p>{t('Close this project in your DAW, then recover it before getting another version.', "Başka bir sürüm almadan önce projeyi DAW'da kapat ve kurtar.")}</p>
      <div className="actions"><button className="primary" disabled={busy} onClick={() => void recover()}>{t('Recover', 'Kurtar')}</button></div></section>}
    {problem && <section className={`problem ${problem.tone}`} role="alert"><div><strong>{problem.title}</strong><p>{problem.body}</p></div>
      <button className="primary" onClick={() => openAdvanced(projectId, problem.section)}>{problem.fix} →</button></section>}
    {!peer && inviteState === 'WAITING_FOR_GRANT' && <Waiting>{t("Waiting for your collaborator's TrackMesh to confirm this device with the invite service. Keep both apps open.", "İş arkadaşının TrackMesh'inin bu cihazı davet servisine onaylaması bekleniyor. İki uygulamayı da açık tut.")}</Waiting>}
    {!peer && backend === 'SUPABASE' && inviteState === 'UNKNOWN' && <Waiting>{t('Connecting to the invite service…', 'Davet servisine bağlanılıyor…')}</Waiting>}
    {status.presence[projectId]?.map(warning => <p key={warning.device_id} className={`notice ${warning.both_working ? 'danger' : ''}`} role="status">
      {warning.both_working ? t(warning.line, `${warning.name} de bu projede çalışıyor. İkiniz de kaydederseniz sürümleriniz ayrılır.`) : t(warning.line, `${warning.name} şu an bu projede çalışıyor`)}
      {!warning.both_working && ` (${t('since', 'başlangıç')} ${new Date(warning.since * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })})`}</p>)}
    {status.plugin_handoff?.project_id === projectId && <p className="notice" role="status">{t(`Save and close this project in your DAW, then press Get ${peerName}'s version.`, `Projeyi DAW'da kaydedip kapat, sonra “${named(collab, 'gen')} sürümünü al” düğmesine bas.`)}</p>}
    {folderError && <p className="error">{folderError}</p>}

    {candidates.length > 0 && <section className="approval-box"><h3>{t('Someone opened your invite link', 'Biri davet bağlantını açtı')}</h3>{candidates.map(candidate => <div key={candidate.request_id}>
      <p><strong>{candidate.name}</strong> · {t('device', 'cihaz')} {short(candidate.device_id)}</p>
      <p className="muted">{t('Approve only if this is the person you invited. Approval gives them this project.', 'Yalnızca davet ettiğin kişiyse onayla. Onay, bu projeyi ona verir.')}</p>
      <div className="actions"><button className="primary" disabled={busy} onClick={async () => await ask(t('Approve this device?', 'Bu cihaz onaylansın mı?'),
          t(`${candidate.name} (device ${short(candidate.device_id)}) will get ${project.name}.\n\nApprove only if this is the person you invited.`, `${candidate.name} (cihaz ${short(candidate.device_id)}) ${project.name} projesini alacak.\n\nYalnızca davet ettiğin kişiyse onayla.`),
          t('Approve', 'Onayla'), t('Cancel', 'Vazgeç')).catch(() => false) && act({ kind: 'approve_pairing', project_id: projectId, request_id: candidate.request_id })}>{t('Approve', 'Onayla')}</button>
        <button disabled={busy} onClick={() => act({ kind: 'reject_pairing', project_id: projectId, request_id: candidate.request_id })}>{t('Reject', 'Reddet')}</button></div></div>)}</section>}

    {!peer && <section className="invite-flow"><h3>{t('Invite your collaborator', 'İş arkadaşını davet et')}</h3>
      <p className="muted">{t('Copies a private link. Send it to one person; it works once, for 15 minutes.', 'Özel bir bağlantı kopyalar. Tek bir kişiye gönder; bir kez ve 15 dakika boyunca çalışır.')}</p>
      <button className="primary" disabled={busy || !inviteReady} onClick={() => act({ kind: 'copy_invite_link', project_id: projectId })}>{t('Invite collaborator', 'Davet et')}</button></section>}
    {pairingOpen && !candidates.length && <Waiting action={<button disabled={busy} onClick={() => act({ kind: 'close_pairing', project_id: projectId })}>{t('Stop waiting', 'Beklemeyi bırak')}</button>}>
      {t('Waiting for your collaborator to open the link', 'İş arkadaşının bağlantıyı açması bekleniyor')}{status.pairing.remaining_seconds > 0 ? ` · ${Math.ceil(status.pairing.remaining_seconds / 60)} ${t('min left', 'dk kaldı')}` : ''}. {t('Keep TrackMesh open.', "TrackMesh'i açık tut.")}</Waiting>}
    {!pairingOpen && !candidates.length && !!inviteJob && !inviteJob.running && <p className="muted">{t('Collaborator opened the link but nothing appeared?', 'İş arkadaşın bağlantıyı açtı ama burada bir şey çıkmadı mı?')} <button className="text-button" disabled={busy} onClick={() => act({ kind: 'open_pairing', project_id: projectId })}>{t('Reopen approval window', 'Onay penceresini yeniden aç')}</button></p>}
    {card.grant_problem === 'REJECTED' && peer && <p className="muted">{t(`The invite service didn't accept ${peerName} (the project may already have 3 devices, or too many requests). Direct sharing still works.`, `Davet servisi ${named(collab, 'acc')} kabul etmedi (projede zaten 3 cihaz olabilir ya da çok fazla istek gelmiş olabilir). Doğrudan paylaşım çalışmaya devam eder.`)}</p>}

    {remoteAhead && peer?.announcement && <div className="new-version-notice" role="status">{t(`New version from ${peerName}`, `${named(collab, 'abl')} yeni sürüm`)} · {time(peer.announcement.created_at)}</div>}
    {showOfflineHead && offlineHead && <div className="new-version-notice" role="status">{t(`New version from ${peerName}`, `${named(collab, 'abl')} yeni sürüm`)} · {time(offlineHead.created_at)} · {t(`available when ${peerName} is online`, `${peerName} bağlanınca alınabilir`)}</div>}

    {peer && (resolveDivergence || resolveLocalChanges) && <section className="notice resolution-box" role="region" aria-label={t('Resolve project versions', 'Sürüm seçimi')}>
      <h3>{resolveLocalChanges && !resolveDivergence ? t("This folder has changes you haven't shared", 'Bu klasörde paylaşmadığın değişiklikler var') : t(`Your version and ${peerName}'s version both changed`, `Senin sürümün de ${named(collab, 'gen')} sürümü de değişti`)}</h3>
      <p>{t('TrackMesh does not merge projects. Choose which version to keep.', 'TrackMesh projeleri birleştirmez. Hangi sürümün kalacağını seç.')}</p>
      {!online && <p className="muted">{t(`Open TrackMesh on both PCs to use ${peerName}'s version.`, `${named(collab, 'gen')} sürümünü kullanmak için TrackMesh'i iki bilgisayarda da aç.`)}</p>}
      <div className="actions">
        <button className="primary" disabled={busy || !online} onClick={() => void usePeerVersion()}>{t(`Use ${peerName}'s version`, `${named(collab, 'gen')} sürümünü kullan`)}</button>
        <button disabled={busy || rootMissing || project.recovery_required} onClick={() => { setKeepMinePending(true); void share(); }}>{t('Keep mine', 'Benimki kalsın')}</button>
      </div>
      {keepMinePending && job?.label === 'Share current version' && !job.running && !job.error && <p role="status">{t(`Ask ${peerName} to choose ‘Use ${status.device?.name ?? 'your device'}'s version’.`, `${named(collab, 'dat')} “${named(status.device?.name, 'gen')} sürümünü kullan” seçeneğini seçmesini söyle.`)}</p>}
    </section>}

    {peer && pairingApproval !== 'PENDING' && <>
      {card.peers.length > 1 && <label className="peer-picker">{t('Collaborator', 'İş arkadaşı')} <select value={peerId ?? ''} disabled={busy} onChange={e => setSelected(e.target.value)}>
        {card.peers.map(p => <option key={p.device.device_id} value={p.device.device_id}>{p.device.name}</option>)}</select></label>}
      <section className="share-flow">
        <div className="pane"><h3>{t('Send my changes', 'Değişikliklerimi gönder')}</h3>
          <p className="muted">{t(`Saves this folder as a new version ${peerName} can get.`, `Bu klasörü ${peerName} için yeni bir sürüm olarak kaydeder.`)}</p>
          {firstShareBlocked
            ? <p className="muted">{card.root_empty && !peerHasVersion ? t('Add your project files to this folder first.', 'Önce proje dosyalarını bu klasöre ekle.') : t(`Get ${peerName}'s version first.`, `Önce ${named(collab, 'gen')} sürümünü al.`)}</p>
            : <button className={remoteAhead ? '' : 'primary'} disabled={busy || rootMissing || project.recovery_required} onClick={() => void share()}>{t('Share current version', 'Güncel sürümü paylaş')}</button>}
          {!online && !firstShareBlocked && <p className="muted">{t(`${peerName} can get it when you are both online.`, `İkiniz de bağlı olunca ${peerName} alabilir.`)}</p>}
        </div>
        <div className="pane"><h3>{t(`Get ${peerName}'s changes`, `${named(collab, 'gen')} değişikliklerini al`)}</h3>
          {status.presence[projectId]?.filter(warning => warning.device_id === peer.device.device_id).map(warning => <p key={warning.device_id} className="muted" role="status">{t(warning.get_line, `${warning.name} yakında daha yeni bir sürüm paylaşabilir.`)}</p>)}
          {upToDate && <p className="muted">{t(`You have ${peerName}'s latest version ✓`, `${named(collab, 'gen')} son sürümü sende ✓`)}</p>}
          <button className={remoteAhead || resumable ? 'primary' : ''} disabled={disabledReceive} onClick={() => void receive()}>{resumable ? t('Resume getting', 'Almaya devam et') : t(`Get ${peerName}'s version`, `${named(collab, 'gen')} sürümünü al`)}</button>
        </div>
      </section></>}

    {!receiving && pairingApproval === 'NOT_REACHABLE' && peer && <section className="notice" role="status"><p>{t(`${peerName} hasn't approved this device yet. Ask them to open TrackMesh; if approval timed out, they can press ‘Reopen approval window’. Then press Try again.`,
      `${peerName} bu cihazı henüz onaylamadı. TrackMesh'i açmasını söyle; onay süresi dolduysa “Onay penceresini yeniden aç” düğmesine basabilir. Sonra Tekrar dene'ye bas.`)}</p>
      <div className="actions"><button disabled={busy} onClick={() => act({ kind: 'request_pairing', project_id: projectId, device_id: peer.device.device_id })}>{t('Try again', 'Tekrar dene')}</button></div></section>}
    {statusLine}

    <footer className="card-footer">
      {peer && <button className="text-button" disabled={busy || !inviteReady} title={inviteReady ? t('Up to 3 devices per project', 'Proje başına en fazla 3 cihaz') : t('Needs the invite service', 'Davet servisi gerekir')} onClick={() => act({ kind: 'copy_invite_link', project_id: projectId })}>{t('Invite another collaborator', 'Başka birini davet et')}</button>}
      <button className="text-button" onClick={() => openAdvanced(projectId, 'connection')}>{t('Advanced →', 'Gelişmiş →')}</button>
    </footer>
  </article>;
}
