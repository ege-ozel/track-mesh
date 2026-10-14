import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import type { DesktopStatus } from './api';
import { CHECK_TIMEOUT_SECONDS, displayPath } from './format';

type Job = DesktopStatus['jobs'][number];

export function jobStatus(job: Job, startedAt: number | undefined, collaboratorName?: string) {
  if (!job.running) {
    return job.error ? `${job.label}: ${job.error}`
      : job.label === "Use collaborator's version" && typeof job.outcome?.transfer_id === 'string' ? `Now using ${collaboratorName ?? 'collaborator'}'s version ✓`
      : job.label === "Get collaborator's version" && typeof job.outcome?.snapshot_id === 'string' ? `Received ${collaboratorName ?? 'collaborator'}'s version ✓`
      : job.outcome?.publication_error ? `Ready — ${collaboratorName ?? 'your collaborator'} can now get this version.`
      : typeof job.outcome?.snapshot_id === 'string' ? `Ready — ${collaboratorName ?? 'your collaborator'} can now get this version.`
      : job.outcome?.message ? String(job.outcome.message) : `${job.label} completed`;
  }
  if (job.label === 'Share current version') return 'Preparing current version…';
  if (job.label !== 'Check connection') return `${job.label}…`;
  const elapsed = startedAt ? Math.floor((Date.now() - startedAt) / 1000) : 0;
  const remaining = Math.max(0, CHECK_TIMEOUT_SECONDS - elapsed);
  return `Checking the connection to ${collaboratorName ?? 'your collaborator'}… up to ${remaining}s left`;
}

/** Work this PC is doing right now; an indeterminate line under the text, never a spinning frame. */
export function InProgress({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return <div className="in-progress" role="status"><span>{children}</span>{action}</div>;
}

/** Waiting on another person: a static line, since nothing here is working. */
export function Waiting({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return <div className="waiting-line" role="status"><span>{children}</span>{action}</div>;
}

const STOPPABLE = new Set(['Check connection', 'Share current version', "Get collaborator's version", "Use collaborator's version"]);

/** Pass `onStop` only while stopping is safe; the backend never interrupts installing files. */
export function JobNotice({ job, startedAt, collaboratorName, onStop }: { job: Job; startedAt: number | undefined; collaboratorName?: string; onStop?: () => void }) {
  const text = jobStatus(job, startedAt, collaboratorName);
  if (job.running) return <InProgress action={onStop && STOPPABLE.has(job.label) ? <StopButton onStop={onStop} /> : undefined}>{text}</InProgress>;
  const warning = job.error || job.outcome?.pairing_state === 'retry';
  return <div className={`job ${warning ? 'danger' : ''}`} role="status">
    {text}
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
