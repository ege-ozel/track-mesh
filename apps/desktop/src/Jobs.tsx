import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import type { DesktopStatus } from './api';
import { CHECK_TIMEOUT_SECONDS, bytes, displayPath } from './format';
import { lang, named, t } from './i18n';

type Job = DesktopStatus['jobs'][number];

/** Backend job labels are stable identifiers; these are the words the buttons use. */
export function jobLabel(label: string, collaboratorName?: string) {
  const who = collaboratorName ?? 'your collaborator';
  switch (label) {
    case 'Delete old backups': return t('Delete old backups', 'Eski yedekleri sil');
    case 'Link folder': return t('Add project', 'Proje ekle');
    case 'Change project folder': return t('Change project folder', 'Proje klasörünü değiştir');
    case 'Save project backend': return t('Save invite service', 'Davet servisini kaydet');
    case 'Save default Supabase settings': return t('Save default invite service', 'Varsayılan davet servisini kaydet');
    case 'Save device name': return t('Save your name', 'Adını kaydet');
    case 'Save DAW auto-share setting': return t('Save share-on-save', 'Kaydedince paylaş ayarı');
    case 'Save Projects location': return t('Save projects location', 'Proje konumunu kaydet');
    case 'Save manual port forwarding': return t('Save manual port forwarding', 'Elle port yönlendirmeyi kaydet');
    case 'Accept invitation and request pairing': return t('Open invitation file', 'Davet dosyasını aç');
    case 'Accept invite link': return t('Accept invite link', 'Davet bağlantısını kabul et');
    case 'Complete pairing': return t('Open pairing reply', 'Eşleşme yanıtını aç');
    case 'Revoke device': return t('Remove access', 'Erişimi kaldır');
    case 'Export invitation': return t('Save invitation file', 'Davet dosyasını kaydet');
    case 'Copy invite link': return t('Copy invite link', 'Davet bağlantısını kopyala');
    case 'Save pairing reply': return t('Save pairing reply', 'Eşleşme yanıtını kaydet');
    case 'Open pairing window': return t('Open approval window', 'Onay penceresini aç');
    case 'Close pairing window': return t('Close approval window', 'Onay penceresini kapat');
    case 'Approve pairing': return t('Approve', 'Onayla');
    case 'Reject pairing': return t('Reject', 'Reddet');
    case 'Request automatic pairing': return t('Ask for approval', 'Onay iste');
    case 'Open invite link': return t('Invite link', 'Davet bağlantısı');
    case 'Share current version': return t('Share current version', 'Güncel sürümü paylaş');
    case 'Check connection': return t('Check connection', 'Bağlantıyı kontrol et');
    case "Get collaborator's version": return t(`Get ${who}'s version`, `${named(collaboratorName, 'gen')} sürümünü al`);
    case "Use collaborator's version": return t(`Use ${who}'s version`, `${named(collaboratorName, 'gen')} sürümünü kullan`);
    case 'Recover installation': return t('Recover', 'Kurtar');
    default: return label;
  }
}

/** Turkish for the result messages the backend writes in English; unknown text stays as it is. */
const TURKISH_MESSAGES: Record<string, string> = {
  "This folder has changes you haven't shared. Choose which version to keep.": 'Bu klasörde paylaşmadığın değişiklikler var. Hangi sürümün kalacağını seç.',
  'Both versions changed. Choose which version to keep.': 'İki sürüm de değişti. Hangi sürümün kalacağını seç.',
  'Connection check stopped': 'Bağlantı kontrolü durduruldu',
  'Sharing stopped. Nothing was shared.': 'Paylaşım durduruldu. Hiçbir şey paylaşılmadı.',
  'Getting stopped. Anything already downloaded is kept for Resume getting.': 'Alma durduruldu. İnen kısım, Almaya devam et için saklandı.',
  'Sharing cancelled': 'Paylaşım iptal edildi',
  'Collaborator approved. Add your project files, then Share.': 'İş arkadaşın onaylandı. Proje dosyalarını ekle, sonra paylaş.',
  'Collaborator approved. First version shared and ready to receive.': 'İş arkadaşın onaylandı. İlk sürüm paylaşıldı, alınmaya hazır.',
  'Collaborator approved for this project.': 'İş arkadaşın bu proje için onaylandı.',
  'Local recovery completed': 'Kurtarma tamamlandı',
  'Folder linked': 'Proje eklendi',
  'Project folder updated': 'Proje klasörü güncellendi',
  'Project backend settings saved': 'Davet servisi ayarları kaydedildi',
  'Default Supabase settings saved': 'Varsayılan davet servisi kaydedildi',
  'Device name saved': 'Adın kaydedildi',
  'Automatic sharing is on': 'Kaydedince paylaş açık',
  'Automatic sharing is off': 'Kaydedince paylaş kapalı',
  'Projects location saved': 'Proje konumu kaydedildi',
  'Manual port forwarding settings saved': 'Elle port yönlendirme ayarları kaydedildi',
  'Approval window open for 15 minutes. Ask your collaborator to press Ask for approval, then verify the arriving device before approving.': 'Onay penceresi 15 dakika açık. İş arkadaşından Onay iste\'ye basmasını iste; onaylamadan önce gelen cihazı kontrol et.',
  'Pairing window closed; unapproved requests removed': 'Onay penceresi kapandı; onaylanmamış istekler silindi',
  'Pairing request rejected; no project access was granted': 'İstek reddedildi; projeye erişim verilmedi',
  'Pairing reply accepted. This device is authorized for the selected project. Tell your collaborator pairing is complete, create a snapshot, and keep TrackMesh open.': 'Eşleşme yanıtı kabul edildi. Bu cihaz projeye yetkili. İş arkadaşına eşleşmenin bittiğini söyle, bir sürüm paylaş ve TrackMesh\'i açık tut.',
  'Pairing reply saved. Send this file privately to the inviter to complete pairing.': 'Eşleşme yanıtı kaydedildi. Eşleşmeyi bitirmek için bu dosyayı davet edene özel olarak gönder.',
  'Private invitation saved. Automatic pairing window open for 15 minutes; share the invitation privately, then verify and approve your collaborator here.': 'Özel davet dosyası kaydedildi. Onay penceresi 15 dakika açık; daveti özel olarak gönder, sonra iş arkadaşını burada kontrol edip onayla.',
  'Pairing request sent automatically. The inviter must verify and approve your device. No card file needs to be sent; wait for their confirmation.': 'İstek gönderildi. Davet eden kişi cihazını kontrol edip onaylamalı; dosya göndermene gerek yok, onayı bekle.',
  'The inviter approved this device for the project.': 'Davet eden kişi bu cihazı projeye onayladı.',
  'Automatic pairing could not complete. Your project remains imported. Ask the inviter to open the approval window and keep TrackMesh open, then press Ask for approval. Do not reimport or create a new UUID. Both devices need a build supporting automatic pairing.': 'Onay isteği tamamlanamadı. Proje bu bilgisayarda duruyor. Davet edenden onay penceresini açmasını ve TrackMesh\'i açık tutmasını iste, sonra Onay iste\'ye bas. Daveti yeniden açma.',
};

export function backendMessage(message: string) {
  if (lang() !== 'tr') return message;
  const known = TURKISH_MESSAGES[message];
  if (known) return known;
  let match = message.match(/^Deleted (\d+) bytes of old backups$/);
  if (match) return `${bytes(Number(match[1]))} eski yedek silindi`;
  match = message.match(/^(Private invitation saved\. )?Pending approvals for (.+) were cleared; only one project can accept new devices at a time\.$/);
  if (match) return `${match[1] ? 'Özel davet dosyası kaydedildi. ' : ''}${match[2]} için bekleyen onaylar silindi; aynı anda yalnızca bir proje yeni cihaz kabul edebilir.`;
  match = message.match(/^Removed (.+)'s access on this PC\.(.*)$/);
  if (match) {
    const who = match[1];
    const rest = match[2].includes('will also remove') ? ` TrackMesh, ${named(who, 'acc')} davet servisinden de çıkaracak.`
      : match[2].includes('Only the device') ? ` ${named(who, 'acc')} davet servisinden yalnızca projeyi oluşturan cihaz çıkarabilir.` : '';
    return `${named(who, 'gen')} bu bilgisayardaki erişimi kaldırıldı.${rest}`;
  }
  return message;
}

/** Plain-language reading of the backend errors people actually hit; null keeps the raw text. */
export function friendlyError(raw: string, collaboratorName?: string): string | null {
  const who = collaboratorName ?? 'your collaborator';
  if (/offline or unreachable|request timed out|dial|connection (refused|reset|closed)|network stopped|no addresses|not connected/i.test(raw))
    return t(`Couldn't reach ${who}. Make sure TrackMesh is open on both PCs, then try again.`, `${named(collaboratorName, 'dat')} ulaşılamadı. TrackMesh'in iki bilgisayarda da açık olduğundan emin ol, sonra tekrar dene.`);
  if (/Close the DAW project before/i.test(raw))
    return t('Close this project in your DAW, then try again.', "Projeyi DAW'da kapat, sonra tekrar dene.");
  if (/still being written or locked|changed (before|during) scan|directory changed during scan|did not settle|changed while saving/i.test(raw))
    return t('Your DAW is still saving or has a file open. Wait a moment, then try again.', "DAW hâlâ kaydediyor ya da bir dosya açık. Biraz bekle, sonra tekrar dene.");
  if (/recovery must finish|recover the existing interrupted/i.test(raw))
    return t('The last Get did not finish. Press Recover on this card first.', 'Son alma işlemi bitmedi. Önce bu karttaki Kurtar\'a bas.');
  if (/original changed or is locked/i.test(raw))
    return t('A project file was open or changed while TrackMesh put the new files in place. Close the project in your DAW, then press Recover.',
      "TrackMesh yeni dosyaları yerleştirirken bir proje dosyası açıktı ya da değişti. Projeyi DAW'da kapat, sonra Kurtar'a bas.");
  if (/already (has an )?active|Snapshot work is already active|Wait for the current operation/i.test(raw))
    return t('TrackMesh is already working on this project. Wait for it to finish.', 'TrackMesh bu projede zaten çalışıyor. Bitmesini bekle.');
  if (/not paired|pairing rejected|not authorized/i.test(raw))
    return t(`This device isn't approved for this project on ${who}'s PC.`, `Bu cihaz ${named(collaboratorName, 'gen')} bilgisayarında bu proje için onaylı değil.`);
  if (/local head advanced/i.test(raw))
    return t('You shared a newer version while this Get was paused. Press Get again.', 'Alma duraklatılmışken daha yeni bir sürüm paylaştın. Tekrar Al\'a bas.');
  if (/An invite is being accepted/i.test(raw))
    return t('An invite is being accepted. Wait for it to finish before opening another link.', 'Bir davet kabul ediliyor. Başka bir bağlantı açmadan önce bitmesini bekle.');
  if (/This invite link could not be opened/i.test(raw))
    return t('This invite link could not be opened. Check the link and try again.', 'Bu davet bağlantısı açılamadı. Bağlantıyı kontrol edip tekrar dene.');
  if (/Backup deletion cancelled/i.test(raw))
    return t('Nothing was deleted.', 'Hiçbir şey silinmedi.');
  if (/Download cancelled/i.test(raw))
    return t('Download cancelled. What was already downloaded is kept for Resume getting.', 'İndirme iptal edildi. İnen kısım, Almaya devam et için saklandı.');
  if (/exceeds MVP size limit|too many project files/i.test(raw))
    return t('This project is larger than TrackMesh supports yet.', 'Bu proje TrackMesh\'in şimdilik desteklediğinden büyük.');
  if (/BLAKE3|unexpected .*response|ancestry|signature/i.test(raw))
    return t(`${who}'s PC sent data that didn't verify. Try again; if it keeps failing, both of you should update TrackMesh.`,
      `${named(collaboratorName, 'gen')} bilgisayarından gelen veri doğrulanamadı. Tekrar dene; sürerse ikiniz de TrackMesh'i güncelleyin.`);
  return null;
}

/** `online` is the collaborator's live state, so a Share result stays true as the connection changes. */
export function jobStatus(job: Job, startedAt: number | undefined, collaboratorName?: string, online?: boolean) {
  const who = collaboratorName ?? 'your collaborator';
  const label = jobLabel(job.label, collaboratorName);
  if (!job.running) {
    if (job.error) return friendlyError(job.error, collaboratorName) ?? `${label}: ${job.error}`;
    if (job.label === "Use collaborator's version" && typeof job.outcome?.transfer_id === 'string') return t(`Now using ${who}'s version ✓`, `Artık ${named(collaboratorName, 'gen')} sürümü kullanılıyor ✓`);
    if (job.label === "Get collaborator's version" && typeof job.outcome?.transfer_id === 'string') return t(`Got ${who}'s version ✓`, `${named(collaboratorName, 'gen')} sürümü alındı ✓`);
    if (job.label === 'Share current version' && typeof job.outcome?.snapshot_id === 'string') {
      if (!collaboratorName) return t('Saved as a new version.', 'Yeni sürüm olarak kaydedildi.');
      return (online ? t(`Shared — ${who} can get this version now.`, `Paylaşıldı — ${who} bu sürümü şimdi alabilir.`)
        : t(`Saved as a new version. ${who} can get it when you are both online.`, `Yeni sürüm olarak kaydedildi. İkiniz de bağlı olunca ${who} alabilir.`));
    }
    return job.outcome?.message ? backendMessage(String(job.outcome.message)) : `${label} ✓`;
  }
  if (job.label === 'Share current version') return t('Preparing current version…', 'Güncel sürüm hazırlanıyor…');
  if (job.label !== 'Check connection') return `${label}…`;
  const elapsed = startedAt ? Math.floor((Date.now() - startedAt) / 1000) : 0;
  const remaining = Math.max(0, CHECK_TIMEOUT_SECONDS - elapsed);
  return t(`Checking the connection to ${who}… up to ${remaining}s left`, `${who} ile bağlantı kontrol ediliyor… en fazla ${remaining} sn kaldı`);
}

/** Work this PC is doing right now; an indeterminate line under the text, never a spinning frame. */
export function InProgress({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return <div className="in-progress" role="status"><span>{children}</span>{action}</div>;
}

/** Waiting on another person: a static line, since nothing here is working. */
export function Waiting({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return <div className="waiting-line" role="status"><span>{children}</span>{action}</div>;
}

/** The raw backend text stays one click away whenever a friendly line replaced it. */
export function ErrorDetails({ raw, collaboratorName }: { raw: string; collaboratorName?: string }) {
  if (!friendlyError(raw, collaboratorName)) return null;
  return <details className="error-details"><summary>{t('Details', 'Ayrıntılar')}</summary><code>{raw}</code></details>;
}

const STOPPABLE = new Set(['Check connection', 'Share current version', "Get collaborator's version", "Use collaborator's version"]);

/** Pass `onStop` only while stopping is safe; the backend never interrupts installing files. */
export function JobNotice({ job, startedAt, collaboratorName, online, onStop, children }: {
  job: Job; startedAt: number | undefined; collaboratorName?: string; online?: boolean; onStop?: () => void; children?: ReactNode;
}) {
  const text = jobStatus(job, startedAt, collaboratorName, online);
  if (job.running) return <InProgress action={onStop && STOPPABLE.has(job.label) ? <StopButton onStop={onStop} /> : undefined}>{text}</InProgress>;
  const sharedOffline = job.label === 'Share current version' && typeof job.outcome?.snapshot_id === 'string' && !!collaboratorName && !online;
  const tone = job.error || job.outcome?.pairing_state === 'retry' ? 'danger' : sharedOffline ? 'warn' : '';
  // A finished job is a green result line; boxes are only for warnings and errors.
  return <div className={tone ? `job ${tone}` : 'delivery-receipt'} role="status">
    {text}{children}
    {job.error && <ErrorDetails raw={job.error} collaboratorName={collaboratorName} />}
    {typeof job.outcome?.path === 'string' && <p>{t('Saved file:', 'Kaydedilen dosya:')} <code>{displayPath(job.outcome.path)}</code></p>}
  </div>;
}

/** Stays pressed after a click: the job ends at its next safe point, which can take a moment. */
export function StopButton({ onStop }: { onStop: () => void }) {
  const [stopping, setStopping] = useState(false);
  return <button disabled={stopping} onClick={() => { setStopping(true); onStop(); }}>{stopping ? t('Stopping…', 'Durduruluyor…') : t('Stop', 'Durdur')}</button>;
}

export function InviteCopiedNotice({ job }: { job: Job }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const interval = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(interval);
  }, []);
  const remaining = Math.max(0, 60 - Math.floor((now - (job.completed_at ?? Math.floor(now / 1000)) * 1000) / 1000));
  if (!remaining) return <div className="job" role="status">{t('Clipboard cleared. Copy the invite link again if it was not pasted.', 'Pano temizlendi. Bağlantı yapıştırılmadıysa yeniden kopyala.')}</div>;
  return <div className="job" role="status">{t('Invite link copied — paste it within 60 s', 'Davet bağlantısı kopyalandı — 60 sn içinde yapıştır')} <strong>({remaining}{t('s left', ' sn kaldı')})</strong></div>;
}
