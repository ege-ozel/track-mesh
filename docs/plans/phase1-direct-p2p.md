# Aşama 1 — Doğrudan P2P uygulama planı

**Durum:** Plan. Uygulanmış özellik listesi veya doğrulama kanıtı **değildir**; ilerleme ve gerçek kontroller yalnızca [IMPLEMENTATION.md](../IMPLEMENTATION.md) içinde tutulur. Her kilometre taşı ayrı küçük iş/PR'dır; Codex sadece sıradakini uygular.

**Varsayımlar** `[V]` ile işaretlidir: kod veya kütüphane davranışı bu planı yazarken çalıştırılarak doğrulanmadı (bu ortam Linux; Windows, router ve ikinci ağ yok). Her `[V]` ilgili kilometre taşının başında doğrulanır.

## 1. Kapsam

İçinde: LAN/doğrudan QUIC+TCP (Noise/Yamux korunur); kullanıcı onaylı UPnP IGD ve ayrı PCP eşlemesi; erişilebilirlik durumunun eşleme sonucundan ayrı gösterimi; ağ değişiminde yeniden deneme/yenileme; JSON'suz yeniden bağlanma ve imzalı endpoint güncellemesi; açılışta peer/snapshot durum yenileme.

Dışında (Aşama 2): relay/VPS/DCUtR doğrulaması ve transfer relay fallback'i, Windows File Watcher, DAW'dan bağımsız dosya izleme, herkese açık DHT, BitTorrent, bulutta proje depolama, DAW açıkken otomatik Receive. Mevcut relay/DCUtR/rendezvous kodu **silinmez** ve Aşama 1 başarı koşulu olmaz.

Değişmezler (her kilometre taşında): imzalı kimlik/sabit `PeerId`, proje yetkisi (`store.authorized`), `gate.rs` pairing penceresi, snapshot/transfer kilit ve journal sırası, yerel SQLite, `SAVE_OBSERVED` dışında snapshot yok, bulutta yalnızca şifreli metadata, davet/anahtar/sır log'a veya ajan bağlamına girmez, **otomatik test/benchmark eklenmez** (yalnızca `cargo fmt/check/clippy`, `npm typecheck/build`, manuel doğrulama).

## 2. Kod tabanı gerçekleri (bu inceleme sırasında okunan)

| Konu | Bulgu | Plan etkisi |
| --- | --- | --- |
| Dinleme | `behaviour.rs:93-94` `0.0.0.0` UDP(QUIC)+TCP, tek `config.port` (43101). | Eşlemenin yerel portu bilinir; TCP ve UDP ayrı kayıt. |
| Dış adres | `config.external_addresses` yalnız başlangıçta `add_external_address` (`behaviour.rs:95`). Çalışırken değiştirilemez; `Command` enum'unda yok. | M3: yeni komut (`Command::ExternalAddress…`) gerekir. |
| Adres kaynağı | `DeviceCard.addresses` **imzalı** (`signing_bytes`), `trusted_peers.card_json` içinde; `Store::trust` aynı `device_id` için kartı **koşulsuz üzerine yazar** (`trust.rs:36`). Kartta zaman/sıra yok. | Eski kart yeniden oynatılarak adres gerilemesi mümkün → M5'te yeni imzalı tip + monotonik sıra şart. `DeviceCard` alanları değiştirilmez (`deny_unknown_fields`, imza biçimi sabit). |
| Dial | `discovery.rs::tick` 6 sn'de bir; kart + `learned` adresleri, QUIC/TCP dönüşümlü, ≥3 denemeden sonra circuit. `learned` yalnız identify/rendezvous kaynaklı. | M4: ağ değişiminde `direct_attempts` sıfırlama; yeni imzalı adres `learned`'e girer. |
| Panel adresi | `runtime.rs::advertised_addresses` yalnız dinleme adresleri; `core/connection.rs` elle girilmiş genel IPv4'ü doğrular (CGNAT/özel aralık reddi). | Elle akış korunur; otomatik değer aynı doğrulayıcıdan geçer. |
| Mailbox | `Envelope` snapshot başına; `ingest` `snapshot_id` nil olamaz, yazar güvenilir olmalı; 30 sn'de publish+refresh (`MailboxRuntime`). Adres taşımaz. | Endpoint duyurusu ayrı kayıt türü/kanal olmalı; mevcut `Envelope` AAD/imza bozulmaz. |
| Settle | `scan_after_save`: ~2 sn sessizlik, 60 sn sınır, 3 deneme; watcher değil. | P5 çoğunlukla mevcut; yalnız doğrulama. |
| Proje yetkisi | İstek yetkisi hem `command` hem gelen istekte `authorized` ile kontrol. | Yeni RPC'ler de proje/cihaz yetkisi ister. |

## 3. Kütüphane kararı (M0 çıktısı; `[V]`)

- `libp2p-upnp` `[V]`: kullanıcı onayı olmadan kendiliğinden eşleme açıp dış adres ekler ve (hatırladığım kadarıyla) yalnız TCP kapsar → **kullanılmaz**; onay, UDP/QUIC, ayrı sonuç durumu ve "başkasının mapping'ine dokunma" kuralıyla çelişir.
- UPnP IGD için ayrı bir istemci (aday: `igd-next`, tokio desteğiyle) `[V]`; PCP (RFC 6887) için ayrı istemci (aday: `crab_nat` veya küçük elle yazılmış UDP istemcisi) `[V]`. NAT-PMP (RFC 6886) bu aşamada **yalnız değerlendirme notu**; UPnP ve PCP ayrı `MappingMethod` değerleridir.
- Seçim ölçütleri: Windows derlenir, tokio uyumlu, `unsafe`/ek C bağımlılığı yok, lisans MIT/Apache uyumlu, `Cargo.lock` kilitli (`--locked --offline` politikası). Bağımlılık yalnız `crates/network` içine eklenir ve yalnız M2'de.
- Windows gateway/arayüz keşfi için kütüphane veya `GetAdaptersAddresses` kullanımı M0'da seçilir; PowerShell'e shell-out önerilmez.

## 4. Ortak durum modeli

`crates/network` içinde yeni `reachability.rs` (private modül; API `lib.rs`'ten yeniden dışa aktarılır, `status.rs` ile birlikte okunur). Alanlar birbirinden bağımsız tutulur:

```text
MappingState:  NOT_ATTEMPTED | UNSUPPORTED | DENIED | MAPPED | EXPIRED | FAILED | UNVERIFIED
MappingMethod: UPNP_IGD | PCP                      (NAT-PMP: ayrı karar)
Mapping { method, protocol(TCP|UDP), local_port, external_port, external_ip?, ip_source(GATEWAY|PEER_OBSERVED|MANUAL), lease_expires_at, gateway, state, detail }
Reachability:  UNVERIFIED | VERIFIED_INBOUND(peer, transport, at) | OUTBOUND_ONLY | FAILED(reason)
Firewall:      profile(PUBLIC|PRIVATE|DOMAIN|UNKNOWN), inbound_rule(PRESENT|ABSENT|UNKNOWN), third_party_product(bool)
```

- `MAPPED` ≠ erişilebilir. `VERIFIED_INBOUND` **yalnızca** farklı ağdan gelen, kimliği doğrulanmış gerçek bir güvenilir-peer bağlantısıyla (Noise el sıkışması + `authorized`) kurulur; aksi `UNVERIFIED` kalır.
- Peer durum etiketleri (`CONNECTED/OFFLINE/REMOTE_AHEAD/UP_TO_DATE/DIVERGED/PORT_UNVERIFIED`) core'da türetilir; `TransferState`/`ApplyPhase` etiketleri çoğaltılmaz, yeni dize literalleri enum'dan gelir.

## 5. Kilometre taşları

Her başlık: **Amaç / Dosyalar / Görevler / Korunacaklar / Kontroller / Elle doğrulama / Riskler**. "Kontroller" yalnız derleme/statik; hiçbir kilometre taşı test dosyası eklemez.

### M0 — Doğrulama ve uyumluluk notu (kod yok)

- **Amaç:** `[V]` maddelerini kapatmak, kütüphane seçimini kesinleştirmek.
- **Görevler:** `AGENTS.md`, `crates/network/AGENTS.md`, `crates/core/AGENTS.md`, `docs/MVP.md` yeniden oku. Aday UPnP/PCP crate'lerinin Windows derlenebilirliğini, tokio entegrasyonunu, lisansını, bakım durumunu resmi kaynaktan doğrula (varsayım yok). Windows ağ/firewall durum API'lerini seç. Sonucu bu dosyanın §3'üne işle.
- **Not:** Kullanıcının ağı (TP-Link VX220-G2u, UPnP açık, WAN IPv4 = görünen IPv4) **ön bilgidir, test sonucu değildir**; port erişilebilirliği doğrulanmadı.
- **Elle doğrulama:** yok. **Çıktı:** güncellenmiş §3, `IMPLEMENTATION.md`'ye yalnız gerçekten yapılan inceleme.

### M1 — Gözlemlenebilir ağ durumu (eşleme yok)

- **Amaç:** Arayüz ve core için erişilebilirlik modelini eklemek; davranış değişmez.
- **Dosyalar:** `crates/network/src/status.rs`, yeni `reachability.rs`, `lib.rs`; `crates/core/src/status.rs`; `apps/desktop/src-tauri/src/runtime.rs` (`DesktopStatus`), `src/api.ts`, `App.tsx` (Connection setup panelinde salt-okunur satırlar).
- **Görevler:** arayüz/yerel IPv4 listesi, varsayılan gateway, tespit edilen "WAN IP görünen IP'ye eşit mi" bilgisi (kaynağı etiketli), `Mapping` listesi `NOT_ATTEMPTED` ile. Çalışan peer bağlantısı varsa hiçbir ağ/router işlemi tetiklenmez.
- **Korunacaklar:** `NetworkStatus` JSON alanları (ekleme yalnız ek alan), elle genel IPv4 akışı.
- **Kontroller:** `cargo fmt --all -- --check`, `cargo check -p trackmesh-network -p trackmesh-core -p trackmesh-desktop --locked --offline`, `cargo clippy … -D warnings`, `npm run typecheck && npm run build`.
- **Elle:** panelde yeni alanların görünmesi (Windows).
- **Risk:** VPN/çoklu arayüzde yanlış arayüzün seçilmesi → gösterilen değerler "gözlem" etiketli, karar vermez.

### M2 — UPnP IGD eşlemesi (onaylı, süreli)

- **Amaç:** Kullanıcı ilk kullanımda açıkça onaylarsa TCP ve UDP için ayrı süreli eşleme.
- **Dosyalar:** `crates/network/Cargo.toml` (tek yeni bağımlılık), yeni `portmap/upnp.rs`; onay bayrağı için `crates/storage` değil **yerel ayar dosyası/mevcut ayar mekanizması** `[V: ayar deposu var mı bak]`; şema değişikliği gerekiyorsa M0'da ayrıca gerekçelendirilir (AGENTS: DB değeri sabit).
- **Görevler:** (a) onay yoksa `NOT_ATTEMPTED`, hiçbir paket gitmez; (b) keşif zaman aşımı sınırlı; (c) TCP ve UDP ayrı `AddPortMapping`, açıklama alanı `TrackMesh` + cihaz kısa kimliği, sonlu lease (ör. 1 saat, `[V]` router desteği); (d) çakışma/zaten-var: **başkasının mapping'ine dokunma**, farklı dış port dene veya `DENIED` bildir; (e) yalnız kendi oluşturduğumuz kayıtları (gateway+dış port+protokol+açıklama) yenile/sil; (f) kapanışta `DeletePortMapping` en iyi çaba, hata `EXPIRED`/log; (g) gateway `GetExternalIPAddress` sonucu **özel/CGNAT aralığındaysa** (`core/connection.rs` kuralları) `MAPPED` ama `UNVERIFIED` + "CGNAT olası" uyarısı.
- **Korunacaklar:** swarm tek sahip aktör; eşleme görevi aktörün komut kanalıyla konuşur, ağ I/O aktör döngüsünü bloklamaz. Sır/davet loglanmaz.
- **Elle (Windows+TP-Link):** UPnP açık/kapalı, zaten kullanılan port, yönlendirici yeniden başlatma, lease bitişi, uygulama kapanınca silinme.
- **Riskler:** router UPnP'yi yalan söyleyebilir ("başarılı" ama erişilemez) → durum `MAPPED`+`UNVERIFIED`; çift NAT/CGNAT'ta IGD yalnız ilk cihazı eşler; bazı cihazlar sabit lease süresi istemez (0 → sonsuz) — sonlu lease zorunlu, reddedilirse yeniden dene.

### M3 — Dinamik dış adres + PCP

- **Amaç:** Eşleme sonucunu çalışan swarm'a yansıtmak; UPnP başarısız/uygunsuzsa **ayrı** PCP.
- **Dosyalar:** `crates/network/src/handle.rs` (+`Command`), `actor.rs`, `behaviour.rs` (başlangıç dışı güncelleme yolu), yeni `portmap/pcp.rs`.
- **Görevler:** `Command::SetExternalAddresses(Vec<Multiaddr>)`; aktör eski/yeni farkını `add_external_address`/`remove_external_address` ile uygular; kaynak etiketi `ip_source` ile saklanır. PCP: PCP sunucusu keşfi (gateway), `MAP` TCP ve UDP için ayrı, nonce+lease, `UNSUPP_VERSION`/`NOT_AUTHORIZED` → `UNSUPPORTED`/`DENIED`. Sıra: çalışan doğrudan bağlantı yoksa UPnP → olmadı/uygunsuzsa PCP. İkisi ayrı `MappingMethod` ve ayrı tanı satırı. NAT-PMP: uygulanmaz, M0 notunda gerekçe.
- **Korunacaklar:** Mevcut `external_addresses` başlangıç davranışı.
- **Elle:** UPnP kapalı + PCP destekleyen cihaz (varsa); PCP yoksa `UNSUPPORTED` göründüğü. **PCP yalnız PCP destekli bir gateway ile doğrulanabilir; yoksa "unverified" kalır.**
- **Risk:** PCP pek çok ev router'ında yoktur; beklenti "UPnP veya hiçbiri" olabilir.

### M4 — Başlangıç/ağ değişimi yeniden değerlendirmesi

- **Amaç:** Açılışta ve ağ değişiminde doğrudan denemelerin hızlanması, lease yenilemesi.
- **Dosyalar:** `discovery.rs` (`Discovery::tick`, yeni `network_changed()`), `actor.rs` (periyodik ağ imzası karşılaştırma, ör. 6 sn'lik mevcut tick içinde), eşleme yenileme görevi (lease/2'de).
- **Görevler:** yerel arayüz/IP kümesi veya gateway değişince: `direct_attempts` sıfırla, eşlemeleri yeniden değerlendir, dış IP/port değiştiyse `SetExternalAddresses`. Yenileme başarısızsa `EXPIRED` ve UI'da gerekçe.
- **Korunacaklar:** altı saniyelik tick, QUIC tercihi, TCP fallback, doğrudan > relayed, `connection_limits`.
- **Elle:** Wi-Fi değiştirme, modem yeniden başlatma, VPN aç/kapa.

### M5 — Güvenli endpoint güncellemesi (JSON'suz yeniden bağlanma)

- **Amaç:** IP/port değişince güvenilir peer'in yeni adresini imzalı, süreli, sıralı kayıtla duyurmak.
- **Dosyalar:** `shared/protocol` (yeni tip + `signing_bytes`, **yeni** etiket `trackmesh-endpoint-v1`; `DeviceCard`/`Envelope` değişmez), `crates/crypto` (imza/doğrulama), `crates/storage` (yeni tablo veya `trusted_peers` yanında ek alan — **şema değişikliği olacak; M5 başında ayrıca onay ve migration planı**), `crates/core` (yayın/alım), `services/mailbox` yalnız opak kayıt türü gerekiyorsa.
- **Tasarım:**
  - Kayıt: `{device_id, peer_id, addresses[≤16], issued_at, expires_at (kısa), seq (monotonik u64)}` cihaz anahtarıyla imzalı.
  - Alıcı: cihaz projede güvenilir mi (`trusted_author`), `peer_id`/anahtar bağı, `seq > saklanan`, süre sınırı, adres biçimi/özel-aralık doğrulaması; geçerse `learned` adres olarak eklenir. **Kart ve yetki yeniden yazılmaz** (böylece `Store::trust` üzerine-yazma gerilemesi yaşanmaz).
  - Kanal öncelikleri: (1) var olan doğrudan bağlantı üzerinden identify/yeni istek, (2) mailbox'ta şifreli kayıt (proje anahtarıyla AEAD, mevcut AAD biçimine dokunmadan **ayrı** `kind`), (3) kısa insan-okur kod için süreli çözüm sunucusu — **yalnız tasarım notu**, bu aşamada uygulanmaz.
  - Sunucu ağ trafiği veya proje dosyası relay etmez; yalnız şifreli küçük kayıt (<64 KiB sınırı).
- **Korunacaklar:** İmzalı kimlik, proje yetkisi, davet ve pairing akışı; sunucuda çözülmüş manifest/anahtar yok.
- **Riskler:** mailbox yoksa iki taraf da aynı anda IP değiştirirse hiçbir kanal kalmaz → UI "adres bilinmiyor, davet yenile" gösterir; replay → `seq`+`expires_at`; uzun URL daveti ID paylaşımıyla yetki vermez (davet hâlâ sahiplik kanıtı ister).

### M6 — Kullanıcı dostu ilk eşleştirme (JSON'suz UX)

- **Amaç:** Dosya alışverişi yerine kopyala/yapıştır davet bağlantısı (+ isteğe QR); kimlik karşılaştırma ekranı.
- **Dosyalar:** `apps/desktop/src-tauri/src/documents.rs`, `commands.rs`, `runtime.rs`, `App.tsx`; `core/src/pairing.rs` yalnız çağrı yüzü.
- **Görevler:** mevcut `ProjectInvite` aynen taşınır (bağlantı = aynı JSON'un kodlanmış hâli, **ağ üzerinde/loglarda yok, panoya yalnız kullanıcı eylemiyle**). Alıcı tarafında "private invitation file path" kuralı korunur: CLAUDE.md gereği ajan bağlamına payload girmez; yapıştırma yalnız Desktop UI'da. Eşleştirme penceresi, istek, açık gönderici onayı mevcut akıştadır.
- **Kısa kod:** yalnız tasarım notu (süreli, rate-limit'li, tahmin zor, tek kullanımlık çözüm; sadece ID yetki vermez). Uygulama kararı sonraki iş.
- **Elle:** bağlantı ile davet → otomatik istek → onay; yanlış kimlik reddi.

### M7 — Otomatik bağlantı ve snapshot durumu

- **Amaç:** Açılışta bilinen adreslerden bağlan, mailbox'ı yenile, peer/snapshot durumunu aç.
- **Dosyalar:** `core/src/mailbox.rs` (`MailboxRuntime` açılışta bir kez anında tur, 30 sn beklemeden), `core/src/status.rs`, `workflows.rs` (`check_peer`), `runtime.rs`.
- **Görevler:** `CONNECTED/OFFLINE` ağ durumundan; `REMOTE_AHEAD/UP_TO_DATE/DIVERGED` yalnız **bağlı peer'in onayladığı** head karşılaştırmasından (mevcut kural: online eşitlik için peer ACK); metadata tek başına eşitlik iddia etmez; `PORT_UNVERIFIED` M1 modelinden. "SEND SNAPSHOT" (yerel commit+publish) ile gerçek dosya teslimi ayrı etiketlenir. Arka plan ön-indirme **yapılmaz** (ayrı güvenlik tasarımı gerekir).
- **Korunacaklar:** receive her zaman kullanıcı onaylı ve DAW kapalıyken kurulur.

### M8 — Gerçek erişilebilirlik kanıtı ve Windows Firewall durumu

- **Amaç:** `VERIFIED_INBOUND` yalnızca gerçek peer bağlantısıyla; firewall salt-okunur rapor.
- **Görevler:** gelen, kimliği doğrulanmış, doğrudan (circuit olmayan) bağlantıda uzak adres özel/loopback değilse ve yerel yönde mapping/dinleme varsa `VERIFIED_INBOUND(peer, transport)`. Firewall: ağ profili (`Public/Private/Domain`) ve TrackMesh için gelen kural var/yok okuma `[V: API seçimi M0]`; üçüncü taraf güvenlik ürünü varsa kesin teşhis iddia edilmez. Kural ekleme yalnız açık kullanıcı onayı + yönetici izniyle ve **ayrı** komutla; güvenlik duvarı kapatılmaz; ilk sürümde yalnız yönerge metni de kabul edilebilir.
- **Risk:** Windows "Public" profilinde kural kapsamı uyuşmazlığı; hairpin/aynı LAN bağlantısı yanlış kanıt sayılmamalı (özel aralık dışlanır).

### M9 — DAW Save → snapshot (doğrulama, kod az)

- `SAVE_OBSERVED` dışında izleme yok; `scan_after_save` korunur. Yalnız: aynı içerik için gereksiz snapshot üretilmediğini (`snapshots.rs` unchanged-content reuse) ve yayın yeniden denemesini **incele ve belgele**; boşluk bulunursa ayrı küçük iş.

### M10 — İki cihazla manuel kabul (kod yok)

`IMPLEMENTATION.md`'ye `verified/unverified` olarak yalnız gerçekten çalıştırılanlar yazılır.

## 6. Manuel doğrulama matrisi (Windows, iki PC)

| # | Senaryo | Beklenen |
| --- | --- | --- |
| 1 | Aynı LAN, UPnP kapalı | Doğrudan QUIC/TCP, `Reachability` LAN; eşleme `NOT_ATTEMPTED` |
| 2 | Farklı ev/ISS; A genel IPv4 + UPnP onaylı, B yalnız çıkış | B→A doğrudan; A `VERIFIED_INBOUND`; B `OUTBOUND_ONLY` |
| 3 | UPnP kapalı | `UNSUPPORTED/DENIED`, net gerekçe, transfer garanti edilmez |
| 4 | PCP destekli/desteksiz gateway | `MAPPED` / `UNSUPPORTED`, UPnP'den ayrı satır |
| 5 | Port başka uygulamada | Çakışma `DENIED/FAILED`, başkasının mapping'i dokunulmaz |
| 6 | Windows Firewall engeli / farklı profil | Eşleme `MAPPED`, erişilebilirlik `UNVERIFIED`; rapor profili gösterir |
| 7 | Dış IP değişimi, modem yeniden başlama | Yeni imzalı endpoint, JSON gerekmeden yeniden bağlanma |
| 8 | Uygulamayı yeniden başlat | İlk davet sonrası tekrar davet istenmez |
| 9 | Yanlış kimlik / eski `seq` | Reddedilir |
| 10 | DAW Save, yarım/kilitli dosya | Snapshot yalnız settle sonrası; yayın offline retry |
| 11 | Büyük aktarım kesintisi | Resume + BLAKE3 doğrulama |
| 12 | Başarısız doğrudan bağlantı | UI gerekçeyi gösterir; relay yok, aktarım garanti edilmez |

## 7. Başarı koşulları

En az bir cihaz erişilebilirken **relay olmadan** doğrulanmış doğrudan transfer; kullanıcı her açılışta IP yazmaz/JSON göndermez; UPnP/PCP "garantili" sunulmaz; bulutta ses/çözülmüş manifest/sır yok; başarısızlık nedeni görünür.

## 8. Açık kararlar

1. UPnP/PCP crate seçimi ve Windows API'leri (M0).
2. Onay tercihinin saklanacağı yer; şema değişikliği gerekip gerekmediği (M2, M5).
3. Endpoint kaydının mailbox'ta ayrı `kind` mı yoksa ayrı uç mu olacağı (M5) — `services/mailbox/AGENTS.md` ile uyumlu.
4. Sürekli kısa kod çözücü sunucu (M6) — Aşama 1'de uygulanmaz, yalnız tasarlanır.

## 9. Codex yürütme protokolü (alt-ajan destekli)

**Bu bölüm bir operatör istemidir; ilerleme kanıtı değildir.** Codex'in alt-ajan (sub-agent) desteği olup olmadığı bu planı yazarken doğrulanmadı `[V]`. Önce Codex'e sorulur; destek yoksa aynı iş sırayla, tek ajanla yürütülür. Model adları kullanıcının Codex hesabına bağlıdır. Codex bildirdi (ajan başlatılmadan): `gpt-6.1-sol` (`max`) güçlü işler ve bağımsız inceleme, `gpt-6-luna` (`medium`) hafif uygulama için seçildi; `gpt-6-astra` kullanılmaz, `ultra` gerekmez. Model gücü sürüm numarasından kesin sıralanamadı; seçici açıklaması esas alınır.

### 9.1 Başlatma sorusu (önce bunu yapıştır)

> Bu repoda `AGENTS.md` ve `docs/plans/phase1-direct-p2p.md` dosyalarını oku. Kod yazma. Önce şunu yanıtla: (1) Bu oturumda alt-ajan başlatabiliyor musun, bunları hangi modelle ve hangi akıl yürütme seviyesiyle çalıştırabiliyorsun? (2) Alt-ajanlar aynı çalışma ağacını mı paylaşıyor yoksa ayrı worktree mi alıyor? (3) Alt-ajan çıktısını nasıl geri alıyorsun? Varsayma; yalnızca gerçekten yapabildiğini söyle. Yapamıyorsan "alt-ajan yok, sıralı yürütürüm" de.

Yanıta göre 9.2 (destek var) veya 9.3 (yok) kullanılır.

### 9.2 Alt-ajan destekli yürütme istemi

> `AGENTS.md`, ilgili crate-local `AGENTS.md` dosyaları ve `docs/plans/phase1-direct-p2p.md` dosyalarını oku. Sen orkestratörsün; **en yüksek akıl yürütme seviyesinde** çalış. Planı **tek seferde değil, bir kilometre taşı (M) bir seferde** uygula; sıradaki M'yi bitirip raporlamadan sonrakine geçme. Her M için:
>
> 1. **Dal:** `AGENTS.md` kuralı: yerel `codex/phase-N-description` dalı; doğrulama sonrası `master`'a `--no-ff` yerel merge; force push yok; faz dalları yerel kalır.
> 2. **Keşif alt-ajanları (salt-okunur, paralel olabilir):** ilgili dosyaları/`[V]` maddelerini ayrı alt-ajanlara dağıt (ör. kütüphane Windows desteği, mevcut çağrı yolları). Alt-ajan **dosya yazmaz**; bulgu + dosya:satır döndürür.
> 3. **Uygulama:** değişikliği **tek yazıcı** yapar (sen veya tek bir `gpt-6.1-sol` (`max`) alt-ajan). Aynı dosyalara paralel yazan iki ajan olmasın. M1, M6 ve M7 aynı `App.tsx`/`runtime.rs` dosyalarına dokunur; bunları paralel yürütme.
> 4. **Bağımsız inceleme:** uygulayıcıdan farklı bir alt-ajan (`gpt-6.1-sol` (`max`), yüksek akıl yürütme), diff'i `AGENTS.md` değişmezlerine karşı eleştirel okur: imzalı kimlik, `authorized`, gate penceresi, snapshot/transfer kilit ve journal sırası, SQLite, sır/davet loglama, `SAVE_OBSERVED` dışı izleme yok, otomatik test/benchmark eklenmedi. Bulguları düzelt, sonra tekrar incelet.
> 5. **Kontroller (yalnız statik):** `cargo fmt --all -- --check`, `cargo check … --locked --offline`, `cargo clippy … -D warnings`, `npm run typecheck && npm run build`. **Test veya benchmark ekleme/çalıştırma yok.** Çalışmayan veya çalıştıramadığın kontrolü "geçti" diye yazma.
> 6. **Kayıt:** `docs/IMPLEMENTATION.md`'ye yalnız gerçekten yaptığın kontrolleri `verified/unverified` ile ekle. `docs/CODEMAP.md`'yi modül değiştiyse güncelle. Rakip durum belgesi oluşturma.
> 7. **Rapor:** görev, değişen dosyalar, korunan değişmezler, çalışan kontroller, **Windows/router olmadan doğrulanamayanlar**, benim elle yapacağım adımlar, kalan riskler, sonraki somut adım.
>
> **Dur ve sor** (varsayma): şema/migration gerekiyorsa (M2, M5), protokol/imza biçimi değişiyorsa, bağımlılık eklenecekse, beklenmedik şekilde ağ davranışı doğrulanamıyorsa. Ağ/NAT/Firewall davranışını doğrulanmış gibi sunma.

Model/seviye eşlemesi (öneri): M0, M2, M3, M4, M5, M8 ve **bağımsız inceleme** → `gpt-6.1-sol` (`max`), en yüksek akıl yürütme. M1, M6, M7, M9 uygulaması → `gpt-6-luna` (`medium`) olabilir, ama **inceleme yine `gpt-6.1-sol` (`max`)**.

Sıra ve bağımlılık: M0 → M1 → M2 → M3 → M4 → M8; M5 M1'den sonra başlayabilir; M6 ve M7 M1'den sonra, ancak birbirleriyle sıralı. M9/M10 sonda.

### 9.3 Alt-ajan yoksa

Aynı 9.2 adımları tek ajanla sırayla uygulanır: önce keşif, sonra uygulama, sonra **ayrı bir oturumda** (yeni sohbet, `gpt-6.1-sol` (`max`)) diff incelemesi. Aynı bağlamın kendi işini onaylaması bağımsız inceleme sayılmaz.
