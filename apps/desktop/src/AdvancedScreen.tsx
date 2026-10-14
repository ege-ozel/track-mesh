import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { ask, chooseFolder, stopOperation } from './api';
import type { Action, CheckView, DesktopStatus, ProjectBackend, ProjectBackendConfig, ProjectCard, SnapshotInfo, SupabaseDefaultConfig } from './api';
import { JobNotice } from './Jobs';
import { ProjectTabs } from './ProjectCard';
import type { AdvancedSection } from './ProjectCard';
import { bytes, displayPath, localIpv4Addresses, publicIpv4, short, time } from './format';
import { named, t } from './i18n';

type Act = (action: Action) => Promise<boolean>;
type Peer = ProjectCard['peers'][number];

function Section({ id, title, summary, tone, open, highlight, onToggle, children }: {
  id: AdvancedSection | string; title: string; summary: string; tone?: 'ok' | 'bad'; open: boolean; highlight: boolean;
  onToggle: (open: boolean) => void; children: ReactNode;
}) {
  const ref = useRef<HTMLDetailsElement>(null);
  useEffect(() => { if (highlight) ref.current?.scrollIntoView({ behavior: 'smooth', block: 'start' }); }, [highlight]);
  return <details ref={ref} id={`advanced-${id}`} className={`adv-section${highlight ? ' focus-highlight' : ''}`} open={open} onToggle={event => onToggle(event.currentTarget.open)}>
    <summary><span className="adv-title">{title}</span><span className={`adv-summary ${tone ?? ''}`}>{summary}</span></summary>
    <div className="adv-body">{children}</div>
  </details>;
}

function nextStep(card: ProjectCard, status: DesktopStatus, peer: Peer | undefined, check: CheckView | undefined) {
  const projectId = card.project.project_id;
  const candidates = status.pairing.project_id === projectId ? status.pairing.requests : [];
  const connection = peer ? status.network?.peers[peer.device.peer_id] : undefined;
  const checkJob = status.jobs.find(job => job.project_id === projectId && job.label === 'Check connection');
  const runtimeError = status.error ?? status.network?.error;
  const collab = peer?.device.name;
  const name = collab ?? 'your collaborator';
  if (runtimeError) return t('The local network session has an error. Quit TrackMesh from the tray after current work finishes, then open it again.', 'Yerel ağ oturumunda bir hata var. Süren işler bitince TrackMesh\'ten saatin yanındaki simgeden çık, sonra yeniden aç.');
  if (candidates.length) return t('Someone opened your invite link. Approve them on the project card in Projects.', "Biri davet bağlantını açtı. Projeler'deki proje kartından onayla.");
  if (!peer && status.pairing.project_id === projectId) return t('Waiting for your collaborator to open the invite link. Keep TrackMesh open.', "İş arkadaşının davet bağlantısını açması bekleniyor. TrackMesh'i açık tut.");
  if (!peer) return t('Invite your collaborator from the project card in Projects.', "İş arkadaşını Projeler'deki proje kartından davet et.");
  if (checkJob?.running) return t(`Trying to reach ${name}. This can take up to 30 seconds.`, `${named(collab, 'dat')} ulaşılmaya çalışılıyor. Bu 30 saniye kadar sürebilir.`);
  if (checkJob?.error) return connection?.online
    ? t(`${name}'s PC answers, but the project check failed. Make sure ${name} approved this device for this project.`, `${named(collab, 'gen')} bilgisayarı yanıt veriyor ama proje kontrolü başarısız. ${named(collab, 'gen')} bu cihazı bu proje için onayladığından emin ol.`)
    : t(`${name}'s PC didn't answer. Make sure TrackMesh is open on both PCs. If you are on different internet connections, at least one router must allow TrackMesh (automatic router setup or manual port forwarding below).`,
      `${named(collab, 'gen')} bilgisayarı yanıt vermedi. TrackMesh'in iki bilgisayarda da açık olduğundan emin ol. Farklı internet bağlantılarındaysanız en az bir modem TrackMesh'e izin vermeli (otomatik modem ayarı ya da aşağıdaki elle port yönlendirme).`);
  if (!connection?.online) return t(`Press Check connection while TrackMesh is open on both PCs. If it fails, ask ${name} to press Check connection too.`, `TrackMesh iki bilgisayarda da açıkken Bağlantıyı kontrol et'e bas. Olmazsa ${named(collab, 'dat')} de basmasını söyle.`);
  if (!check?.fresh) return t(`Connected to ${name}. Check connection also compares both versions.`, `${named(collab, 'dat')} bağlı. Bağlantıyı kontrol et iki sürümü de karşılaştırır.`);
  if (card.project.recovery_required) return t('The last Get did not finish. Recover it from the project card in Projects.', "Son alma işlemi bitmedi. Projeler'deki proje kartından kurtar.");
  switch (check.result.snapshot_status) {
    case 'DIVERGED': return t('Both versions changed. Choose which one to keep on the project card; TrackMesh does not merge projects.', 'İki sürüm de değişti. Hangisinin kalacağını proje kartında seç; TrackMesh projeleri birleştirmez.');
    case 'UP_TO_DATE': return t('Both PCs have the same version.', 'İki bilgisayarda da aynı sürüm var.');
    case 'LOCAL_AHEAD': return t(`Your version is newer. ${name} can press Get.`, `Senin sürümün daha yeni. ${name} Al'a basabilir.`);
    case 'REMOTE_AHEAD': return t(`${name}'s version is newer. Close the project in your DAW, then press Get.`, `${named(collab, 'gen')} sürümü daha yeni. Projeyi DAW'da kapat, sonra Al'a bas.`);
    default: return t('Version history is unknown. Look at Version history below before getting a version.', 'Sürüm geçmişi bilinmiyor. Bir sürüm almadan önce aşağıdaki Sürüm geçmişine bak.');
  }
}

function comparison(value: string, collab: string | undefined) {
  switch (value) {
    case 'UP_TO_DATE': return t('same version', 'aynı sürüm');
    case 'LOCAL_AHEAD': return t('your version is newer', 'senin sürümün daha yeni');
    case 'REMOTE_AHEAD': return t(`${collab ?? 'your collaborator'}'s version is newer`, `${named(collab, 'gen')} sürümü daha yeni`);
    case 'DIVERGED': return t('both versions changed', 'iki sürüm de değişti');
    default: return t('unknown', 'bilinmiyor');
  }
}

function Snapshot({ value, empty, mailboxEnabled }: { value: SnapshotInfo | null; empty: string; mailboxEnabled: boolean }) {
  return value ? <><div className="snapshot-id">{short(value.snapshot_id)} {mailboxEnabled && <span>{value.metadata_status}</span>}</div>
    <p>{value.author_name} · {time(value.created_at)}</p>
    <p>{value.file_count} {t('files', 'dosya')} · {bytes(value.total_size)} · {bytes(value.changed_bytes)} {t('changed', 'değişti')}</p></>
    : <p className="muted">{empty}</p>;
}

function RawConnectionDetails({ status, projectId, busy, act }: { status: DesktopStatus; projectId: string; busy: boolean; act: Act }) {
  const [ip, setIp] = useState('');
  const [port, setPort] = useState(String(status.listen_port));
  const publicIp = ip.trim();
  const validPort = /^\d{1,5}$/.test(port) && Number(port) > 0 && Number(port) <= 65535;
  const valid = publicIpv4(publicIp) && validPort;
  const observation = status.network_observation;
  const firewall = status.firewall_observation;
  const mapping = status.network?.port_mapping;
  const reach = status.network?.reachability;
  return <details><summary>{t('Technical details', 'Teknik ayrıntılar')}</summary>
    <div className="raw-grid">
      <p><strong>{t('Local interfaces', 'Yerel arayüzler')}</strong> {observation.interfaces.length ? observation.interfaces.map(iface => `${iface.name || `Interface ${iface.interface_index}`} [${iface.ipv4.join(', ') || 'UNKNOWN'}]`).join('; ') : 'UNKNOWN'}</p>
      <p><strong>{t('Default gateway', 'Varsayılan ağ geçidi')}</strong> {observation.default_gateways.length ? observation.default_gateways.map(gateway => `${gateway.address} (interface ${gateway.interface_index}; ${gateway.source})`).join('; ') : 'UNKNOWN'}</p>
      <p><strong>{t('Listen port', 'Dinleme portu')}</strong> {status.listen_port} {t('(default 43101). Changing it needs a restart with that startup port.', '(varsayılan 43101). Değiştirmek için o başlangıç portuyla yeniden başlatmak gerekir.')}</p>
      <p><strong>UPnP</strong> {mapping?.state ?? 'NOT_ATTEMPTED'} · TCP {mapping?.tcp ?? 'NOT_ATTEMPTED'} · UDP {mapping?.udp ?? 'NOT_ATTEMPTED'} · router external IPv4 {mapping?.external_ip_state ?? 'UNKNOWN'}{mapping?.detail ? ` · ${mapping.detail}` : ''}</p>
      <p><strong>{t('Reachability', 'Erişilebilirlik')}</strong> {reach?.state ?? 'UNVERIFIED'}{reach?.peer ? ` · peer ${reach.peer}` : ''}{reach?.transport ? ` · ${reach.transport}` : ''}{reach?.at_unix_seconds ? ` · ${new Date(reach.at_unix_seconds * 1000).toLocaleString()}` : ''}. {t('Same-network connections do not prove internet reachability.', 'Aynı ağdaki bağlantı, internetten erişilebildiğini kanıtlamaz.')}</p>
      <p><strong>Windows Firewall</strong> {firewall.state}. {firewall.profiles.map(profile => `${profile.name}: ${profile.enabled === null ? 'UNKNOWN' : profile.enabled ? 'on' : 'off'}, block all inbound ${profile.block_all_inbound ?? 'UNKNOWN'}, default inbound ${profile.default_inbound ?? 'UNKNOWN'}`).join('; ')}. Rules for port {status.listen_port}: TCP allow {firewall.tcp_rules.allow}, block {firewall.tcp_rules.block}; UDP allow {firewall.udp_rules.allow}, block {firewall.udp_rules.block}. Third-party firewall: {firewall.third_party_products}.</p>
      <p><strong>{t('Port status', 'Port durumu')}</strong> {status.port_status === 'UNVERIFIED' ? 'Port PORT_UNVERIFIED' : 'Port UNKNOWN'}</p>
    </div>
    <details><summary>{t('Advertised and listen addresses', 'Duyurulan ve dinlenen adresler')}</summary>{(status.network?.addresses ?? []).map(address => <code className="connection-address" key={address}>{address}</code>)}
      {status.configured_external_addresses.map(address => <code className="connection-address" key={`external-${address}`}>{address} ({t('startup external address', 'başlangıç dış adresi')})</code>)}</details>
    <details><summary>{t('Invitation file with public addresses', 'Genel adresli davet dosyası')}</summary>
      <p className="muted">{t('For a one-off invitation when you forwarded a router port by hand. The addresses go into this file only.', 'Modemde portu elle yönlendirdiysen tek seferlik bir davet için. Adresler yalnızca bu dosyaya yazılır.')}</p>
      <div className="connection-inputs"><label className="field">{t('Public IPv4', 'Genel IPv4')}<input value={ip} maxLength={15} disabled={busy} onChange={e => setIp(e.target.value)} placeholder="203.0.113.1" /></label>
        <label className="field">{t('External router port', 'Modemin dış portu')}<input type="number" min={1} max={65535} value={port} disabled={busy} onChange={e => setPort(e.target.value)} /></label></div>
      {publicIp && !valid && <p className="error">{t('Enter a public IPv4 and a port from 1 to 65535. 10.x, 172.16–31.x, 192.168.x and 100.64–127.x are not public.', "Genel bir IPv4 ve 1–65535 arası bir port gir. 10.x, 172.16–31.x, 192.168.x ve 100.64–127.x genel adres değildir.")}</p>}
      <div className="actions"><button disabled={busy || !valid} onClick={() => act({ kind: 'export_invite', project_id: projectId, public_ip: publicIp, external_port: Number(port) })}>{t('Save invitation file', 'Davet dosyasını kaydet')}</button></div>
    </details>
  </details>;
}

function ProjectBackendSettings({ projectId, config, defaultSupabase, state, revokeProblem, missingFunctions, unreachable, busy, act }: {
  projectId: string; config: ProjectBackendConfig | undefined; busy: boolean;
  defaultSupabase: SupabaseDefaultConfig | null;
  state: DesktopStatus['supabase_states'][string];
  revokeProblem: boolean; missingFunctions: string[]; unreachable: boolean;
  act: Act;
}) {
  const [backend, setBackend] = useState<ProjectBackend>(config?.backend ?? 'NONE');
  const [url, setUrl] = useState((config?.backend === 'SUPABASE' ? config.supabase_url : null) ?? defaultSupabase?.url ?? '');
  const [anonKey, setAnonKey] = useState((config?.backend === 'SUPABASE' ? config.supabase_anon_key : null) ?? defaultSupabase?.publishable_key ?? '');
  useEffect(() => setBackend(config?.backend ?? 'NONE'), [config?.backend]);
  useEffect(() => {
    if (config?.backend !== 'SUPABASE') {
      setUrl(defaultSupabase?.url ?? '');
      setAnonKey(defaultSupabase?.publishable_key ?? '');
    }
  }, [config?.backend, config?.supabase_url, config?.supabase_anon_key, defaultSupabase?.url, defaultSupabase?.publishable_key]);
  return <div className="backend-settings">
    <p className="muted">{t('Invite links and version notices use a small Supabase project you own. It stores only encrypted invite drops, addresses and version hints — never project files.', 'Davet bağlantıları ve sürüm bildirimleri sana ait küçük bir Supabase projesini kullanır. Orada yalnızca şifreli davetler, adresler ve sürüm ipuçları durur — proje dosyaları asla.')}</p>
    <label className="field">{t('Service', 'Servis')}<select value={backend} disabled={busy} onChange={event => setBackend(event.target.value as ProjectBackend)}>
      <option value="SUPABASE">{t('Supabase (recommended)', 'Supabase (önerilen)')}</option>
      <option value="NONE">{t('None · direct connections only', 'Yok · yalnızca doğrudan bağlantı')}</option>
      <option value="SELF_HOSTED_MAILBOX">{t("Self-hosted mailbox · this project's mailbox URL", 'Kendi sunucundaki mailbox · bu projenin mailbox adresi')}</option>
    </select></label>
    {backend === 'SELF_HOSTED_MAILBOX' && <p className="muted">{t("Uses this project's existing mailbox URL without changing its behavior.", 'Bu projenin mevcut mailbox adresini, davranışını değiştirmeden kullanır.')}</p>}
    {backend === 'SUPABASE' && <div className="backend-fields">
      <label className="field">Supabase Project URL<input autoComplete="off" spellCheck={false} value={url} disabled={busy} placeholder="https://your-project.supabase.co" onChange={event => setUrl(event.target.value)} /></label>
      <label className="field">Publishable key<input type="password" autoComplete="new-password" spellCheck={false} value={anonKey} disabled={busy} onChange={event => setAnonKey(event.target.value)} /></label>
      <p className="muted">{t('HTTPS', 'Yalnızca HTTPS')} <code>*.supabase.co</code> {t('addresses only. Never enter a service role or secret key.', 'adresleri. Asla service role ya da secret key girme.')}</p>
    </div>}
    <div className="actions"><button className="primary" disabled={busy || (backend === 'SUPABASE' && (!url.trim() || !anonKey.trim()))} onClick={() => void act({ kind: 'set_project_backend', project_id: projectId, backend,
      supabase_url: backend === 'SUPABASE' ? url.trim() : null,
      supabase_anon_key: backend === 'SUPABASE' ? anonKey.trim() : null })}>{t('Save', 'Kaydet')}</button><span className="badge">{backend === 'SELF_HOSTED_MAILBOX' ? 'SELF_HOSTED' : state}</span></div>
    {unreachable && <p className="error">{t("The invite service can't be reached. Check the address above and this PC's internet connection.", 'Davet servisine ulaşılamıyor. Yukarıdaki adresi ve bu bilgisayarın internet bağlantısını kontrol et.')}</p>}
    {revokeProblem && <p className="muted">{t("The invite service didn't remove a device. Only the device that created the project can remove others; it may also have been removed already.", 'Davet servisi bir cihazı çıkarmadı. Başkalarını yalnızca projeyi oluşturan cihaz çıkarabilir; cihaz zaten çıkarılmış da olabilir.')}</p>}
    {missingFunctions.length > 0 && <p className="error">{t('The invite service is missing these functions:', 'Davet servisinde şu fonksiyonlar eksik:')} {missingFunctions.join(', ')}. {t('Deploy them as described in services/supabase/README.md.', 'services/supabase/README.md içindeki gibi kur.')}</p>}
  </div>;
}

function DefaultSupabaseSettings({ config, busy, act }: { config: SupabaseDefaultConfig | null; busy: boolean; act: Act }) {
  const [url, setUrl] = useState(config?.url ?? '');
  const [publishableKey, setPublishableKey] = useState(config?.publishable_key ?? '');
  useEffect(() => {
    setUrl(config?.url ?? '');
    setPublishableKey(config?.publishable_key ?? '');
  }, [config?.url, config?.publishable_key]);
  return <div className="field-stack">
    <p className="muted">{t("Used for new projects and prefilled in each project's Invite service. Saved in this app's settings, outside your projects. The publishable key is public configuration; never enter a secret key.", 'Yeni projelerde kullanılır ve her projenin davet servisine önceden doldurulur. Projelerinin dışında, bu uygulamanın ayarlarında saklanır. Publishable key genel bir ayardır; asla secret key girme.')}</p>
    <label className="field">Supabase Project URL<input autoComplete="off" spellCheck={false} value={url} disabled={busy} placeholder="https://your-project.supabase.co" onChange={event => setUrl(event.target.value)} /></label>
    <label className="field">Publishable key<input type="password" autoComplete="new-password" spellCheck={false} value={publishableKey} disabled={busy} onChange={event => setPublishableKey(event.target.value)} /></label>
    <div className="actions"><button className="primary" disabled={busy || !url.trim() || !publishableKey.trim()} onClick={() => void act({ kind: 'set_default_supabase', url: url.trim(), publishable_key: publishableKey.trim() })}>{t('Save', 'Kaydet')}</button>
      <button disabled={busy || !config} onClick={() => void act({ kind: 'set_default_supabase', url: null, publishable_key: null })}>{t('Clear', 'Temizle')}</button></div>
  </div>;
}

function ManualPortForwardingSettings({ endpoint, listenPort, localIps, busy, act }: {
  endpoint: { public_ipv4: string; external_port: number } | null; listenPort: number; localIps: string[]; busy: boolean; act: Act;
}) {
  const [ip, setIp] = useState(endpoint?.public_ipv4 ?? '');
  const [port, setPort] = useState(endpoint ? String(endpoint.external_port) : String(listenPort));
  useEffect(() => {
    setIp(endpoint?.public_ipv4 ?? '');
    setPort(endpoint ? String(endpoint.external_port) : String(listenPort));
  }, [endpoint?.public_ipv4, endpoint?.external_port, listenPort]);
  const validPort = /^\d{1,5}$/.test(port) && Number(port) > 0 && Number(port) <= 65535;
  const valid = publicIpv4(ip.trim()) && validPort;
  const shownPort = validPort ? port : 'PORT';
  const target = localIps.length ? localIps.map(local => `${local}:${listenPort}`).join(t(' or ', ' ya da ')) : t(`local port ${listenPort}`, `yerel port ${listenPort}`);
  return <div className="field-stack">
    <p className="muted">{t(`Only when automatic router setup fails. In your router, forward TCP and UDP ${shownPort} to this PC (${target}) and reserve this PC's address in the router's DHCP settings. Then enter your public IPv4 here. Update it when your public IP changes.`,
      `Yalnızca otomatik modem ayarı çalışmazsa. Modeminde TCP ve UDP ${shownPort} portunu bu bilgisayara (${target}) yönlendir ve modemin DHCP ayarlarında bu bilgisayarın adresini sabitle. Sonra genel IPv4 adresini buraya gir. Genel IP'n değişince güncelle.`)}</p>
    <div className="connection-inputs"><label className="field">{t('Public IPv4', 'Genel IPv4')}<input value={ip} maxLength={15} disabled={busy} onChange={event => setIp(event.target.value)} placeholder="203.0.113.1" /></label>
      <label className="field">{t('External port', 'Dış port')}<input type="number" min={1} max={65535} value={port} disabled={busy} onChange={event => setPort(event.target.value)} /></label></div>
    {ip.trim() && !valid && <p className="error">{t("Enter a public IPv4 and a port from 1 to 65535. If your router's WAN address starts with 100.64–127, your internet provider shares it (CGNAT) and port forwarding can't work.", "Genel bir IPv4 ve 1–65535 arası bir port gir. Modeminin WAN adresi 100.64–127 ile başlıyorsa internet sağlayıcın onu paylaştırıyordur (CGNAT) ve port yönlendirme çalışmaz.")}</p>}
    <div className="actions"><button className="primary" disabled={busy || !valid} onClick={() => void act({ kind: 'set_manual_public_endpoint', public_ipv4: ip.trim(), external_port: Number(port) })}>{t('Save', 'Kaydet')}</button>
      <button disabled={busy || !endpoint} onClick={() => void act({ kind: 'set_manual_public_endpoint', public_ipv4: null, external_port: null })}>{t('Clear', 'Temizle')}</button></div>
  </div>;
}

export function AdvancedScreen({ status, projectId, section, jobStartedAt, act, pickProject, openInvitationFile, onError }: {
  status: DesktopStatus; projectId: string | null; section: AdvancedSection | null; jobStartedAt: Record<string, number>;
  act: Act; pickProject: (projectId: string) => void; openInvitationFile: () => void; onError: (message: string) => void;
}) {
  const projects = status.page?.projects ?? [];
  const card = projects.find(item => item.project.project_id === projectId) ?? projects[0];
  const [open, setOpen] = useState<Record<string, boolean>>({ connection: true });
  const [selectedPeer, setSelectedPeer] = useState('');
  useEffect(() => { if (section) setOpen(current => ({ ...current, [section]: true })); }, [section, projectId]);
  const toggle = (id: string) => (value: boolean) => setOpen(current => current[id] === value ? current : { ...current, [id]: value });
  const globalBusy = !status.ready || status.closing || status.jobs.some(j => j.running && !j.project_id);
  const localIps = localIpv4Addresses(status);
  const mapping = status.network?.port_mapping;
  const thisPc = <>
    <h2 className="group-title">{t('This PC', 'Bu bilgisayar')}</h2>
    <Section id="default-service" title={t('Default invite service', 'Varsayılan davet servisi')} summary={status.default_supabase ? t('Saved', 'Kayıtlı') : t('Not set', 'Ayarlanmadı')} tone={status.default_supabase ? 'ok' : undefined} open={!!open['default-service']} highlight={false} onToggle={toggle('default-service')}>
      <DefaultSupabaseSettings config={status.default_supabase} busy={globalBusy} act={act} />
    </Section>
    <Section id="port-forwarding" title={t('Manual port forwarding', 'Elle port yönlendirme')} summary={status.manual_public_endpoint ? `${status.manual_public_endpoint.public_ipv4}:${status.manual_public_endpoint.external_port}` : t('Off', 'Kapalı')} open={!!open['port-forwarding']} highlight={false} onToggle={toggle('port-forwarding')}>
      <ManualPortForwardingSettings endpoint={status.manual_public_endpoint} listenPort={status.listen_port} localIps={localIps} busy={globalBusy} act={act} />
    </Section>
    <Section id="invitation-file" title={t('Open an invitation file', 'Davet dosyası aç')} summary={t("When invite links don't work", 'Davet bağlantıları çalışmazsa')} open={!!open['invitation-file']} highlight={false} onToggle={toggle('invitation-file')}>
      <p className="muted">{t('For an invitation saved as a file (Invite without the service). Choose an empty folder for the project, then open the file.', 'Dosya olarak kaydedilmiş bir davet için (servis olmadan davet). Proje için boş bir klasör seç, sonra dosyayı aç.')}</p>
      <div className="actions"><button disabled={globalBusy} onClick={openInvitationFile}>{t('Open invitation file…', 'Davet dosyasını aç…')}</button></div>
    </Section>
    <Section id="data" title={t('App data', 'Uygulama verisi')} summary={t('Where TrackMesh keeps its own files', "TrackMesh'in kendi dosyalarının yeri")} open={!!open.data} highlight={false} onToggle={toggle('data')}>
      <p className="muted">{t('Keys, history and file data for your projects. Do not share or delete this folder.', 'Projelerinin anahtarları, geçmişi ve dosya verisi. Bu klasörü paylaşma ve silme.')}</p>
      <code>{status.data_directory ?? t('Opening protected local storage…', 'Korumalı yerel depolama açılıyor…')}</code>
    </Section>
  </>;
  if (!card) return <section className="advanced-screen"><p className="muted">{t('Share a project folder first. Project connection details appear here.', 'Önce bir proje klasörü paylaş. Projenin bağlantı ayrıntıları burada görünür.')}</p>{thisPc}</section>;

  const project = card.project;
  const id = project.project_id;
  const peer = card.peers.find(p => p.device.device_id === selectedPeer) ?? card.peers[0];
  const collab = peer?.device.name;
  const name = collab ?? 'your collaborator';
  const connection = peer ? status.network?.peers[peer.device.peer_id] : undefined;
  const online = connection?.online ?? false;
  const busy = globalBusy || status.jobs.some(j => j.running && j.project_id === id);
  const check = status.checks.find(c => c.result.project_id === id && c.device_id === peer?.device.device_id);
  const checkJob = status.jobs.find(job => job.project_id === id && job.label === 'Check connection');
  const inviteState = status.supabase_states[id] ?? 'NOT_CONFIGURED';
  const backend = status.project_backends[id];
  const serviceConnected = backend?.backend === 'SUPABASE' && inviteState !== 'UNREACHABLE' && ['CLAIMED', 'MEMBER'].includes(inviteState);
  const serviceSummary = backend?.backend === 'SELF_HOSTED_MAILBOX' ? t('Self-hosted mailbox', 'Kendi sunucundaki mailbox')
    : backend?.backend !== 'SUPABASE' ? t('Not set up', 'Kurulmadı')
    : inviteState === 'UNREACHABLE' ? t("Can't reach the service", 'Servise ulaşılamıyor')
    : serviceConnected ? t('Connected', 'Bağlı') : t('Connecting…', 'Bağlanıyor…');
  const firewallAllowed = status.firewall_observation.tcp_rules.allow === 'PRESENT' && status.firewall_observation.udp_rules.allow === 'PRESENT';
  const runtimeError = status.error ?? status.network?.error;
  const chooseProjectRoot = async () => {
    try {
      const folder = await chooseFolder();
      if (folder) await act({ kind: 'change_project_root', project_id: id, folder });
    } catch (err) { onError(String(err)); }
  };
  return <section className="advanced-screen">
    <ProjectTabs projects={projects} status={status} selectedId={id} pick={pickProject} />
    {card.peers.length > 1 && <label className="peer-picker">{t('Collaborator', 'İş arkadaşı')} <select value={peer?.device.device_id ?? ''} onChange={e => setSelectedPeer(e.target.value)}>{card.peers.map(p => <option key={p.device.device_id} value={p.device.device_id}>{p.device.name}</option>)}</select></label>}

    <Section id="connection" title={t('Connection', 'Bağlantı')} summary={runtimeError ? t('Network error', 'Ağ hatası') : !peer ? t('No collaborator yet', 'Henüz iş arkadaşı yok') : online ? `${t('Connected', 'Bağlı')} · ${connection?.route === 'RELAYED' ? t('relayed', 'aktarmalı') : t('direct', 'doğrudan')}` : t(`Can't reach ${name}`, `${named(collab, 'dat')} ulaşılamıyor`)} tone={online ? 'ok' : peer ? 'bad' : undefined}
      open={!!open.connection} highlight={section === 'connection'} onToggle={toggle('connection')}>
      <div className="status-rows">
        <div><span>{t('This PC', 'Bu bilgisayar')}</span><span className={runtimeError ? 'bad' : 'ok'}>{runtimeError ? runtimeError : status.ready ? t('Running', 'Çalışıyor') : t('Starting', 'Başlatılıyor')}</span></div>
        {peer && <div><span>{name}</span><span className={online ? 'ok' : 'bad'}>{online ? `${t('Reachable', 'Ulaşılabilir')} · ${connection?.transport}` : checkJob?.running ? t('Checking…', 'Kontrol ediliyor…') : checkJob?.error ? t('Last check failed', 'Son kontrol başarısız') : t('Not connected', 'Bağlı değil')}</span></div>}
        <div><span>{t('Router (automatic setup)', 'Modem (otomatik ayar)')}</span><span className={mapping?.state === 'MAPPED' ? 'ok' : mapping?.state === 'FAILED' ? 'bad' : ''}>{!mapping?.consent_enabled ? t('Off · turn on in Settings', "Kapalı · Ayarlar'dan aç") : mapping.state === 'MAPPED' ? t('Port opened', 'Port açıldı') : mapping.state === 'FAILED' ? t('Router refused', 'Modem reddetti') : t('Trying…', 'Deneniyor…')}</span></div>
        <div><span>{t('Manual port forwarding', 'Elle port yönlendirme')}</span><span>{status.manual_public_endpoint ? `${status.manual_public_endpoint.public_ipv4}:${status.manual_public_endpoint.external_port}` : t('Off', 'Kapalı')}</span></div>
        <div><span>Windows Firewall</span><span className={firewallAllowed ? 'ok' : ''}>{firewallAllowed ? t('TrackMesh allowed', "TrackMesh'e izin var") : t('No TrackMesh rule found · reinstall TrackMesh', "TrackMesh kuralı yok · TrackMesh'i yeniden kur")}</span></div>
      </div>
      <p className="next-step"><strong>{t('Next step', 'Sonraki adım')}</strong>{nextStep(card, status, peer, check)}</p>
      {checkJob && <JobNotice job={checkJob} startedAt={jobStartedAt[checkJob.job_id]} collaboratorName={collab} onStop={() => void stopOperation(id).catch(err => onError(String(err)))} />}
      <div className="actions">
        {peer && <button className="primary" disabled={busy} onClick={() => act({ kind: 'check', project_id: id, device_id: peer.device.device_id })}>{t('Check connection', 'Bağlantıyı kontrol et')}</button>}
        <button onClick={() => { setOpen(current => ({ ...current, 'port-forwarding': true })); window.requestAnimationFrame(() => document.getElementById('advanced-port-forwarding')?.scrollIntoView({ behavior: 'smooth', block: 'start' })); }}>{t('Manual port forwarding…', 'Elle port yönlendirme…')}</button>
      </div>
      <p className="muted">{t("A failed check can't tell which of these blocked the connection: the other PC being closed, a firewall, guest Wi-Fi isolation or a router.", 'Başarısız bir kontrol, bağlantıyı neyin engellediğini ayırt edemez: diğer bilgisayarın kapalı olması, güvenlik duvarı, misafir Wi-Fi yalıtımı ya da modem.')}</p>
      <RawConnectionDetails status={status} projectId={id} busy={busy} act={act} />
    </Section>

    <Section id="service" title={t('Invite service', 'Davet servisi')} summary={serviceSummary} tone={serviceConnected ? 'ok' : inviteState === 'UNREACHABLE' || card.missing_functions.length ? 'bad' : undefined}
      open={!!open.service} highlight={section === 'service'} onToggle={toggle('service')}>
      <ProjectBackendSettings key={id} projectId={id} config={backend} defaultSupabase={status.default_supabase} state={inviteState} revokeProblem={card.revoke_problem} missingFunctions={card.missing_functions}
        unreachable={card.grant_problem === 'UNREACHABLE' || inviteState === 'UNREACHABLE'} busy={busy} act={act} />
    </Section>

    <Section id="manual" title={t('Invite without the service', 'Servis olmadan davet')} summary={t('Invitation file', 'Davet dosyası')} open={!!open.manual} highlight={section === 'manual'} onToggle={toggle('manual')}>
      <ol className="steps-list">
        <li><strong>{t('Save an invitation file', 'Davet dosyasını kaydet')}</strong> {t('and send it privately. Saving opens a temporary approval window.', 've özel olarak gönder. Kaydetmek geçici bir onay penceresi açar.')}</li>
        <li><strong>{t('Your collaborator opens it', 'İş arkadaşın dosyayı açar:')}</strong> {t('in Advanced → Open an invitation file. Their device request arrives here automatically.', 'Gelişmiş → Davet dosyası aç. Cihaz isteği buraya kendiliğinden gelir.')}</li>
        <li><strong>{t('Approve them', 'Onayla')}</strong> {t('on the project card.', '(proje kartında).')}</li>
      </ol>
      <div className="actions"><button disabled={busy} onClick={() => act({ kind: 'export_invite', project_id: id })}>{t('Save invitation file', 'Davet dosyasını kaydet')}</button>
        {status.pairing.project_id === id
          ? <button disabled={busy} onClick={() => act({ kind: 'close_pairing', project_id: id })}>{t('Close approval window', 'Onay penceresini kapat')}</button>
          : <button disabled={busy} onClick={() => act({ kind: 'open_pairing', project_id: id })}>{t('Open approval window', 'Onay penceresini aç')}</button>}
        {peer && <button disabled={busy} onClick={() => act({ kind: 'request_pairing', project_id: id, device_id: peer.device.device_id })}>{t('Retry automatic pairing', 'Onay isteğini tekrar gönder')}</button>}</div>
      <p className="muted">{t('Only one project accepts new devices at a time; closing, expiry or restart removes unapproved requests. Encryption protects contents, not IP addresses or traffic timing.', 'Aynı anda yalnızca bir proje yeni cihaz kabul eder; kapatmak, sürenin dolması ya da yeniden başlatmak onaylanmamış istekleri siler. Şifreleme içeriği korur; IP adreslerini ya da trafik zamanlamasını değil.')}</p>
      <details><summary>{t('Legacy file pairing', 'Eski usul dosyayla eşleşme')}</summary><p className="muted">{t('For an older build: exchange signed device cards by hand. Access is granted only after the inviter opens the card for this project.', 'Eski bir sürüm için: imzalı cihaz kartlarını elle değiştirin. Erişim, davet eden kartı bu proje için açınca verilir.')}</p>
        <div className="actions"><button disabled={busy} onClick={() => act({ kind: 'pair', project_id: id })}>{t('Open pairing reply', 'Eşleşme yanıtını aç')}</button>
          <button disabled={busy} onClick={() => act({ kind: 'export_card' })}>{t('Save pairing reply', 'Eşleşme yanıtını kaydet')}</button></div></details>
    </Section>

    <Section id="history" title={t('Version history', 'Sürüm geçmişi')} summary={peer && peer.history.length ? t(`${peer.history.length} recent from ${name}`, `${named(collab, 'abl')} son ${peer.history.length} sürüm`) : card.local_snapshot ? t('Local version only', 'Yalnızca yerel sürüm') : t('No versions yet', 'Henüz sürüm yok')}
      open={!!open.history} highlight={section === 'history'} onToggle={toggle('history')}>
      <div className="snapshots"><section><h3>{t('This PC', 'Bu bilgisayar')}</h3><Snapshot value={card.local_snapshot} empty={t('No version yet.', 'Henüz sürüm yok.')} mailboxEnabled={card.mailbox_enabled} /></section>
        <section><h3>{peer ? name : t('Collaborator', 'İş arkadaşı')}</h3><Snapshot value={peer?.announcement ?? null} empty={t('No version seen yet.', 'Henüz sürüm görülmedi.')} mailboxEnabled={card.mailbox_enabled} /></section></div>
      <p className="muted">{check?.fresh ? `${t('Last comparison:', 'Son karşılaştırma:')} ${comparison(check.result.snapshot_status, collab)}` :t('No live comparison. Check connection compares both versions.', 'Canlı karşılaştırma yok. Bağlantıyı kontrol et iki sürümü de karşılaştırır.')}</p>
      {peer && peer.history.length > 1 && <ol className="history-list">{peer.history.map(snapshot => <li key={snapshot.snapshot_id}><code>{short(snapshot.snapshot_id)}</code> · {snapshot.author_name} · {time(snapshot.created_at)} · {snapshot.file_count} {t('files', 'dosya')}</li>)}</ol>}
    </Section>

    <Section id="access" title={t('Folder, ID and access', 'Klasör, kimlik ve erişim')} summary={card.peers.length ? t(`${card.peers.length} collaborator${card.peers.length > 1 ? 's' : ''}`, `${card.peers.length} iş arkadaşı`) : t('Folder', 'Klasör')}
      open={!!open.access} highlight={section === 'access'} onToggle={toggle('access')}>
      <div className="status-rows">
        <div><span>{t('Folder', 'Klasör')}</span><code>{displayPath(project.root)}</code></div>
        <div><span>{t('Project ID', 'Proje kimliği')}</span><code>{project.project_id}</code></div>
        <div><span>Mailbox</span><code>{project.mailbox_url ?? t('Not configured', 'Ayarlanmadı')}</code></div>
        <div><span>{t('Relay (legacy)', 'Relay (eski)')}</span><code>{project.relay_address ?? t('Not configured', 'Ayarlanmadı')}</code></div>
      </div>
      <div className="actions"><button disabled={busy} onClick={() => void chooseProjectRoot()}>{card.root_available ? t('Change folder…', 'Klasörü değiştir…') : t('Locate folder…', 'Klasörü bul…')}</button></div>
      {card.peers.map(item => <div className="access-row" key={item.device.device_id}>
        <div><strong>{item.device.name}</strong><p className="muted">{t('Device', 'Cihaz')} <code>{item.device.device_id}</code> · {t('Identity', 'Kimlik')} <code>{item.device.peer_id}</code></p></div>
        <button className="danger" disabled={busy} onClick={async () => await ask(t('Remove access?', 'Erişim kaldırılsın mı?'),
          t(`${item.device.name} will no longer get versions of ${project.name}.\n\nThey keep the files they already received.`, `${item.device.name} artık ${project.name} sürümlerini alamayacak.\n\nDaha önce aldığı dosyalar onda kalır.`),
          t('Remove access', 'Erişimi kaldır'), t('Cancel', 'Vazgeç')).catch(() => false) && act({ kind: 'revoke', project_id: id, device_id: item.device.device_id })}>{t('Remove access', 'Erişimi kaldır')}</button>
      </div>)}
      <p className="muted">{card.mailbox_enabled ? t('The mailbox stores encrypted metadata and version notices only.', 'Mailbox yalnızca şifreli bilgileri ve sürüm bildirimlerini tutar.') : t('Version notices use the invite service; the legacy mailbox is off for this project.', 'Sürüm bildirimleri davet servisini kullanır; bu projede eski mailbox kapalı.')}</p>
    </Section>
    {thisPc}
  </section>;
}
