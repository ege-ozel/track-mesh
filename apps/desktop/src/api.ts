import { invoke } from '@tauri-apps/api/core';

export interface SnapshotInfo {
  snapshot_id: string; author_device_id: string; created_at: string;
  author_name: string; metadata_status: 'PENDING' | 'PUBLISHED' | 'REMOTE';
  total_size: number; changed_bytes: number; file_count: number;
}
export interface Peer {
  device: { device_id: string; name: string; peer_id: string };
  announcement: SnapshotInfo | null;
  history: SnapshotInfo[];
  known_lineage: Lineage | null;
}
export interface Transfer {
  transfer_id: string; snapshot_id: string; source_device_id: string;
  state: 'QUEUED' | 'RECEIVING' | 'PAUSED' | 'APPLYING' | 'RECOVERY_REQUIRED' | 'COMPLETE';
  completed_bytes: number; total_bytes: number; error: string | null;
}
export interface ProjectCard {
  project: { project_id: string; name: string; root: string; latest_local_snapshot: string | null;
    last_verified_at: string | null; verified_peer_id: string | null; verified_snapshot_id: string | null;
    recovery_required: boolean; mailbox_url: string | null; relay_address: string | null; error: string | null };
  local_snapshot: SnapshotInfo | null; peers: Peer[]; pending_publications: number; transfer: Transfer | null;
}
export interface Connection { online: boolean; route: 'DIRECT' | 'RELAYED' | 'OFFLINE'; transport: string }
export type Lineage = 'EQUAL' | 'REMOTE_AHEAD' | 'LOCAL_AHEAD' | 'DIVERGED' | 'UNKNOWN';
export interface Job { job_id: string; project_id: string | null; label: string; running: boolean; outcome: Record<string, unknown> | null; error: string | null }
export interface CheckView { device_id: string; fresh: boolean; current_known: boolean; age_seconds: number; result: {
  project_id: string; local_snapshot: string | null; remote_snapshot: string | null;
  lineage: Lineage | null;
  verified_equal: boolean; connection: Connection | null; last_verified_at: string | null;
} }
export interface DesktopStatus {
  ready: boolean; closing: boolean; error: string | null;
  device: { device_id: string; name: string; peer_id: string } | null;
  data_directory: string | null;
  network: { addresses: string[]; error: string | null; peers: Record<string, Connection> } | null;
  page: { projects: ProjectCard[]; offset: number; has_more: boolean } | null;
  jobs: Job[]; checks: CheckView[];
}
export type Action =
  | { kind: 'link'; project_id: string; name: string; folder: string; mailbox_url: string | null; relay_address: string | null }
  | { kind: 'join'; folder: string }
  | { kind: 'export_card' }
  | { kind: 'pair' | 'export_invite' | 'send' | 'publish' | 'refresh'; project_id: string }
  | { kind: 'revoke' | 'check'; project_id: string; device_id: string }
  | { kind: 'receive'; project_id: string; device_id: string; snapshot_id: string | null; daw_closed: boolean }
  | { kind: 'recover'; project_id: string; daw_closed: boolean };

export const getStatus = (offset: number) => invoke<DesktopStatus>('get_status', { offset });
export const chooseFolder = () => invoke<string | null>('choose_folder');
export const submitAction = (action: Action) => invoke<string>('submit_action', { action });
export const closeApp = () => invoke<void>('close_app');
