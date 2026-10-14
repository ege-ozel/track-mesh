import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import type { DesktopStatus } from './api';
import { CHECK_TIMEOUT_SECONDS, displayPath } from './format';

type Job = DesktopStatus['jobs'][number];

/** Backend job labels are stable identifiers; these are the words the buttons use. */
export function jobLabel(label: string, collaboratorName?: string) {
  const who = collaboratorName ?? 'your collaborator';
  switch (label) {
    case 'Link folder': return 'Add project';
    case 'Save project backend': return 'Save invite service';
    case 'Save default Supabase settings': return 'Save default invite service';
    case 'Save device name': return 'Save your name';
    case 'Save DAW auto-share setting': return 'Save share-on-save';
    case 'Accept invitation and request pairing': return 'Open invitation file';
    case 'Complete pairing': return 'Open pairing reply';
    case 'Revoke device': return 'Remove access';
    case 'Export invitation': return 'Save invitation file';
    case 'Open pairing window': return 'Open approval window';
    case 'Close pairing window': return 'Close approval window';
    case 'Approve pairing': return 'Approve';
    case 'Reject pairing': return 'Reject';
    case 'Request automatic pairing': return 'Ask for approval again';
    case 'Publish metadata': return 'Post version notice';
    case 'Refresh metadata': return 'Refresh version notices';
    case "Get collaborator's version": return `Get ${who}'s version`;
    case "Use collaborator's version": return `Use ${who}'s version`;
    case 'Recover installation': return 'Recover';
    default: return label;
  }
}

/** Plain-language reading of the backend errors people actually hit; null keeps the raw text. */
export function friendlyError(raw: string, collaboratorName?: string): string | null {
  const who = collaboratorName ?? 'your collaborator';
  if (/offline or unreachable|request timed out|dial|connection (refused|reset|closed)|network stopped|no addresses|not connected/i.test(raw))
    return `Couldn't reach ${who}. Make sure TrackMesh is open on both PCs, then try again.`;
  if (/Close the DAW project before/i.test(raw))
    return 'Close this project in your DAW and tick “I closed this project in my DAW” first.';
  if (/still being written or locked|changed (before|during) scan|directory changed during scan|did not settle|changed while saving/i.test(raw))
    return 'Your DAW is still saving or has a file open. Wait a moment, then try again.';
  if (/recovery must finish|recover the existing interrupted/i.test(raw))
    return 'The last Get did not finish. Press Recover on this card first.';
  if (/original changed or is locked/i.test(raw))
    return 'A project file was open or changed while TrackMesh put the new files in place. Close the project in your DAW, then press Recover.';
  if (/already (has an )?active|Snapshot work is already active|Wait for the current operation/i.test(raw))
    return 'TrackMesh is already working on this project. Wait for it to finish.';
  if (/not paired|pairing rejected|not authorized/i.test(raw))
    return `This device isn't approved for this project on ${who}'s PC.`;
  if (/local head advanced/i.test(raw))
    return 'You shared a newer version while this Get was paused. Press Get again.';
  if (/exceeds MVP size limit|too many project files/i.test(raw))
    return 'This project is larger than TrackMesh supports yet.';
  if (/BLAKE3|unexpected .*response|ancestry|signature/i.test(raw))
    return `${who}'s PC sent data that didn't verify. Try again; if it keeps failing, both of you should update TrackMesh.`;
  return null;
}

/** `online` is the collaborator's live state, so a Share result stays true as the connection changes. */
export function jobStatus(job: Job, startedAt: number | undefined, collaboratorName?: string, online?: boolean) {
  const who = collaboratorName ?? 'your collaborator';
  const label = jobLabel(job.label, collaboratorName);
  if (!job.running) {
    if (job.error) return friendlyError(job.error, collaboratorName) ?? `${label}: ${job.error}`;
    if (job.label === "Use collaborator's version" && typeof job.outcome?.transfer_id === 'string') return `Now using ${who}'s version ✓`;
    if (job.label === "Get collaborator's version" && typeof job.outcome?.transfer_id === 'string') return `Got ${who}'s version ✓`;
    if (job.label === 'Share current version' && typeof job.outcome?.snapshot_id === 'string') {
      const notice = job.outcome?.publication_error ? ' The version notice could not be posted to the invite service.' : '';
      if (!collaboratorName) return `Saved as a new version.${notice}`;
      return (online ? `Shared — ${who} can get this version now.` : `Saved as a new version. ${who} can get it when you are both online.`) + notice;
    }
    return job.outcome?.message ? String(job.outcome.message) : `${label} ✓`;
  }
  if (job.label === 'Share current version') return 'Preparing current version…';
  if (job.label !== 'Check connection') return `${label}…`;
  const elapsed = startedAt ? Math.floor((Date.now() - startedAt) / 1000) : 0;
  const remaining = Math.max(0, CHECK_TIMEOUT_SECONDS - elapsed);
  return `Checking the connection to ${who}… up to ${remaining}s left`;
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
  return <details className="error-details"><summary>Details</summary><code>{raw}</code></details>;
}

const STOPPABLE = new Set(['Check connection', 'Share current version', "Get collaborator's version", "Use collaborator's version"]);

/** Pass `onStop` only while stopping is safe; the backend never interrupts installing files. */
export function JobNotice({ job, startedAt, collaboratorName, online, onStop }: { job: Job; startedAt: number | undefined; collaboratorName?: string; online?: boolean; onStop?: () => void }) {
  const text = jobStatus(job, startedAt, collaboratorName, online);
  if (job.running) return <InProgress action={onStop && STOPPABLE.has(job.label) ? <StopButton onStop={onStop} /> : undefined}>{text}</InProgress>;
  const sharedOffline = job.label === 'Share current version' && typeof job.outcome?.snapshot_id === 'string' && !!collaboratorName && !online;
  const tone = job.error || job.outcome?.pairing_state === 'retry' ? 'danger' : sharedOffline || job.outcome?.publication_error ? 'warn' : '';
  return <div className={`job ${tone}`} role="status">
    {text}
    {job.error && <ErrorDetails raw={job.error} collaboratorName={collaboratorName} />}
    {typeof job.outcome?.path === 'string' && <p>Saved file: <code>{displayPath(job.outcome.path)}</code></p>}
  </div>;
}

/** Stays pressed after a click: the job ends at its next safe point, which can take a moment. */
export function StopButton({ onStop }: { onStop: () => void }) {
  const [stopping, setStopping] = useState(false);
  return <button disabled={stopping} onClick={() => { setStopping(true); onStop(); }}>{stopping ? 'Stopping…' : 'Stop'}</button>;
}

export function InviteCopiedNotice({ job }: { job: Job }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const interval = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(interval);
  }, []);
  const remaining = Math.max(0, 60 - Math.floor((now - (job.completed_at ?? Math.floor(now / 1000)) * 1000) / 1000));
  if (!remaining) return <div className="job" role="status">Clipboard cleared. Copy the invite link again if it was not pasted.</div>;
  return <div className="job" role="status">Invite link copied — paste it within 60 s <strong>({remaining}s left)</strong></div>;
}
