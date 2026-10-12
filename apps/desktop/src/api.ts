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
export type ProjectBackend = 'NONE' | 'SELF_HOSTED_MAILBOX' | 'SUPABASE';
export interface ProjectBackendConfig {
  backend: ProjectBackend;
  supabase_url: string | null;
  supabase_anon_key: string | null;
}
export interface SupabaseDefaultConfig { url: string; publishable_key: string }
export interface Connection { online: boolean; route: 'DIRECT' | 'RELAYED' | 'OFFLINE'; transport: string }
export interface FirewallObservation {
  state: string;
  profiles: { name: string; enabled: boolean | null; block_all_inbound: boolean | null; default_inbound: string | null }[];
  tcp_rules: { allow: string; block: string };
  udp_rules: { allow: string; block: string };
  third_party_products: string;
}
export type Lineage = 'EQUAL' | 'REMOTE_AHEAD' | 'LOCAL_AHEAD' | 'DIVERGED' | 'UNKNOWN';
export interface Job { job_id: string; project_id: string | null; label: string; running: boolean; outcome: Record<string, unknown> | null; error: string | null }
export interface CheckView { device_id: string; fresh: boolean; current_known: boolean; age_seconds: number; result: {
  project_id: string; local_snapshot: string | null; remote_snapshot: string | null;
  lineage: Lineage | null;
  connection_state: 'CONNECTED' | 'OFFLINE';
  snapshot_status: 'REMOTE_AHEAD' | 'UP_TO_DATE' | 'DIVERGED' | 'LOCAL_AHEAD' | 'UNKNOWN';
  verified_equal: boolean; connection: Connection | null; last_verified_at: string | null;
} }
export interface NetworkObservation {
  source: string;
  interfaces: { interface_index: number; name: string; ipv4: string[]; gateways: string[] }[];
  default_gateways: { interface_index: number; address: string; source: string }[];
  wan_visible_ip: { state: 'UNKNOWN'; source: string; value: string | null };
  mappings: { method: 'UPNP_IGD' | 'PCP'; transport: 'TCP' | 'UDP'; state: 'NOT_ATTEMPTED' }[];
}
export interface DesktopStatus {
  pairing: { project_id: string | null; remaining_seconds: number; requests: { request_id: string; device_id: string; name: string; peer_id: string }[] };
  listen_port: number; configured_external_addresses: string[];
  ready: boolean; closing: boolean; error: string | null;
  device: { device_id: string; name: string; peer_id: string } | null;
  data_directory: string | null;
  network: { addresses: string[]; error: string | null; peers: Record<string, Connection>; port_mapping: { consent_enabled: boolean; state: string; tcp: string; udp: string; detail: string | null; external_ip_state: string }; reachability: { state: string; peer: string | null; transport: string | null; at_unix_seconds: number | null } } | null;
  peer_states: Record<string, 'CONNECTED' | 'OFFLINE'>;
  port_status: 'UNVERIFIED' | 'VERIFIED_INBOUND' | null;
  network_observation: NetworkObservation;
  firewall_observation: FirewallObservation;
  page: { projects: ProjectCard[]; offset: number; has_more: boolean } | null;
  jobs: Job[]; checks: CheckView[];
  project_backends: Record<string, ProjectBackendConfig>;
  default_supabase: SupabaseDefaultConfig | null;
  supabase_states: Record<string, 'NOT_CONFIGURED' | 'UNKNOWN' | 'CLAIMED' | 'MEMBER' | 'ROUTE_TAKEN' | 'WAITING_FOR_GRANT' | 'UNREACHABLE'>;
  invite_pending: boolean;
}
export type Action =
  | { kind: 'link'; project_id: string; name: string; folder: string; mailbox_url: string | null; relay_address: string | null }
  | { kind: 'set_project_backend'; project_id: string; backend: ProjectBackend; supabase_url: string | null; supabase_anon_key: string | null }
  | { kind: 'set_default_supabase'; url: string | null; publishable_key: string | null }
  | { kind: 'join'; folder: string }
  | { kind: 'complete_invite_link'; supabase_url?: string; supabase_anon_key?: string }
  | { kind: 'export_card' }
  | { kind: 'open_pairing' | 'close_pairing'; project_id: string }
  | { kind: 'approve_pairing' | 'reject_pairing'; project_id: string; request_id: string }
  | { kind: 'request_pairing'; project_id: string; device_id: string }
  | { kind: 'pair' | 'send' | 'publish' | 'refresh'; project_id: string }
  | { kind: 'export_invite'; project_id: string; public_ip?: string | null; external_port?: number | null }
  | { kind: 'copy_invite_link'; project_id: string }
  | { kind: 'revoke' | 'check'; project_id: string; device_id: string }
  | { kind: 'receive'; project_id: string; device_id: string; snapshot_id: string | null; daw_closed: boolean }
  | { kind: 'recover'; project_id: string; daw_closed: boolean };

export const getStatus = (offset: number) => invoke<DesktopStatus>('get_status', { offset });
export const chooseFolder = () => invoke<string | null>('choose_folder');
export const submitAction = (action: Action) => invoke<string>('submit_action', { action });
export const setUpnpMapping = (enabled: boolean) => invoke<void>('set_upnp_mapping', { enabled });
export const closeApp = () => invoke<void>('close_app');
