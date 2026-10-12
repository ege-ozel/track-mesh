# Aşama 1 — Supabase arka ucu tasarımı

**Durum:** Tasarım eki; kod, şema, servis veya çalışan Supabase projesi değildir. Gerçek deployment ve operasyon doğrulanmadı. Kotalar 2026-10-12 tarihinde Supabase'in resmi belgelerinden okunmuştur ve değişebilir; kurulumdan önce yeniden kontrol edilmelidir. Aşamaların ilerleme ve gerçek kontrolleri yalnızca [IMPLEMENTATION.md](../IMPLEMENTATION.md) içinde tutulur.

Bu ek, ürün sahibinin 2026-10-12 tarihli kararıyla Aşama 1 M5d (endpoint mailbox) ve M6 (şifreli davet bırakma) için Supabase'i seçilen arka uç yapar. Mevcut yerel davet dosyası ve isteğe bağlı kendi mailbox servisi geçerliliğini korur. Relay 1.0.0 kapsamı dışındadır ve ertelenmiştir. Bu ek yalnız tasarımdır; yeni wire, servis API'si, SQL şeması veya bağımlılık uygulanmasına izin vermez.

## 1. Veri modeli ve gizlilik sınırı

Supabase Postgres'te iki ayrı tablo grubu önerilir. Kolon tipleri ve isimleri uygulama öncesi SQL/API tasarım onayı gerektirir.

### `endpoint_records`

Önerilen alanlar: `project_route` (proje UUID'si), `writer_device_id`, `seq`, `ciphertext`, `expires_at`, `created_at`. Benzersiz anahtar `(project_route, writer_device_id, seq)`; sorgu indeksi proje + zaman/sıra ve expiry temizliği için. `ciphertext` yalnız proje anahtarıyla XChaCha20-Poly1305 kullanılarak şifrelenmiş wrapper'dır. Wrapper AAD'si `trackmesh-endpoint-mailbox-v1` olur. İçinde M5 `EndpointRecord` ve imzası bulunur. `project_route`, yazar kimliği, sıra ve zaman sunucu/DB tarafından erişilebilir metadata'dır; yalnız adresler ve imzalı iç kayıt şifreli kalır.

Sunucuda plaintext adres, proje anahtarı veya imza doğrulamasında kullanılan gizli materyal bulunmaz. Edge Function cihaz imzalı isteği ve proje ACL'sini doğrular; alıcı `Engine::apply_endpoint_record` ile M5c'deki aynı güven zinciri, sıra, zaman, adres doğrulama ve atomik yerel kabul sırasını tekrar çalıştırır. Supabase ACL'si yerel `trusted_author`/proje yetkisinin yerine geçmez ve endpoint kaydı transfer yetkisi vermez.

### `invite_drops`

Önerilen alanlar: kriptografik rastgele `id` (locator), `ciphertext`, `expires_at`, `consumed`, `created_at`; davet tablosunda proje route veya davet içeriği tutulmaz. HTTP yanıtları generic olur; ID varlığı, expiry, tüketim ve decrypt başarısızlığını ayırt etmez. Davet blob'u boyutu 64 KiB'den küçük tutulur. Ciphertext, istemcide var olan XChaCha20-Poly1305 ile üretilir; `trackmesh-invite-drop-v1` AAD'si ID ve expiry'yi de bağlar. 256-bit rastgele anahtar yalnız paylaşım URI fragment'inde taşınır; sunucuya gönderilmez. Tek kullanımlık tüketim aşağıdaki §4'teki tek Postgres transaction'ı ile yapılır.

## 2. İstek doğrulama, tablo erişimi ve ACL

İstemciye yalnız Supabase Project URL ve publishable/legacy anon key dağıtılabilir. Bunlar gizli kabul edilmez; API anahtarı kimlik veya proje üyeliği kanıtı değildir. İki tablo için doğrudan `anon` ve `authenticated` erişim grant'leri kaldırılır, RLS açılır ve hiçbir istemci politikası tanımlanmaz (deny-all). Edge Function dışında tabloya doğrudan erişim kapalıdır. Function'ın kullandığı secret/service-role anahtarı RLS'yi atlar; yalnız Edge Function ortamında tutulur, hiçbir TrackMesh binary'sine, depoya, dağıtım paketine, loga veya istemci yanıtına girmez. SQL erişimi kullanılıyorsa yalnız dar yetkili, kontrollü DB işlemleri tercih edilir; service-role kullanımı kaçınılmazsa Function içindeki doğrulanmış işlemlerden sonra çalıştırılır.

`endpoint_publish`, `endpoint_list` ve `invite_create` için cihaz Ed25519 imzalı istek zarfı tasarlanır. Zarf en az `device_id`, `project_route` (invite upload için route kapsamı), method/operation, canonical path, UTC timestamp, random nonce, body hash ve signature taşır. İmza baytları ayrı ve domain-separated scope kullanır; örneğin `trackmesh-supabase-endpoint-request-v1` ve `trackmesh-supabase-invite-request-v1`. İstek gövdesi ve route/path kanonik biçimde hash'e bağlanır. Function saat skew sınırı uygular ve nonce'ı kısa replay penceresi boyunca atomik olarak bir kez kabul eder. Aynı imza farklı body, route, method veya zaman damgası için geçerli olmaz. HTTPS zorunludur. Her Function auth gateway'inin davranışına körü körüne güvenmek yerine uygulama imzasını ayrıca doğrular.

### Sunucu ACL temsili ve pairing sonrası provisioning

Mevcut pairing/trust state cihazların yerel SQLite depolarındadır; Supabase bunu kendiliğinden bilemez. Tasarım, her Supabase TrackMesh projesi için `project_members(project_route, device_id, ed25519_public_key, role, status, granted_at, grant_signature)` gibi bir sunucu ACL'si gerektirir. Function, isteğin imza anahtarının aynı proje route'unda `active` üye olduğunu kontrol eder. `device_id` ve public key eşleşmesi TrackMesh kimlik bağlamından doğrulanır. Revoke edilmiş cihaz eski endpoint kayıtlarını yayımlayamaz veya okuyamaz. Bu ACL, yalnız metadata servisine erişim iznidir; dosya transfer yetkisi sağlamaz.

Seçenekler:

1. **Dashboard/SQL ile elle ACL ekleme:** en az protokol değişikliği, fakat proje sahibi her eşleşmede güvenilir cihazın public key'ini ayrıca kopyalar; ölçeklenmez ve kullanıcı hatasına açık.
2. **Ürün sahibi imzalı üyelik grant'i (öneri):** proje sahibi cihazı ilk kez Supabase ACL yöneticisi olarak güvenli bir bootstrap adımıyla kaydeder. Bootstrap için tek seferlik SQL/Dashboard işlemi ya da ayrı ve kısa ömürlü, gizli tutulması gereken enrollment kodu gerekir. Sonrasında pairing isteği tek başına üyelik yaratmaz. Davet alan peer, mevcut challenge/proof ile cihaz anahtarının sahipliğini kanıtlar; inviter kullanıcı adayı inceler ve mevcut açık onay düğmesiyle eşleştirmeyi onaylar. Onaydan sonra owner cihaz `project_route + yeni device_id + public key + role + expiry/seq + action` kapsamlı grant'i kendi Ed25519 anahtarıyla imzalar ve Edge Function'a yollar. Function grant imzasını kayıtlı proje yöneticisi anahtarına göre doğrular, challenge/proof sonucunu ve yazar üyeliğini doğrular, ardından `project_members` güncellemesini tek DB transaction'ında yapar. Revocation da imzalı ve monotonic sequence'li olur. Bu protokol ve UI, ayrıca wire/service API onayı almadan uygulanmaz.
3. **Invite linki veya proje anahtarıyla otomatik üyelik:** önerilmez. Bearer linkin çalınması veya yalnız proje anahtarını bilen birinin isteği sunucu ACL'sine dönüşür; mevcut açık inviter onayı ve güvenilir device binding atlanmış olur.

Önerilen yöntemde de sahibin cihaz anahtarı/DB yedeği kaybı, ilk bootstrap ve cihaz iptali için kurtarma planı gerekir. Çoklu admin/delegasyon, owner cihaz offline iken yeni üyelik ve ACL'nin başka bir cihaza aktarımı henüz kararlaştırılmamıştır `[V]`. Açıkça belirtmek gerekir: “automatic service ACL update yok” önceki ürün sınırı bu karar ile değişmiştir. Otomatik güncelleme yalnız cihaz anahtarıyla imzalı, explicit local approval sonrası üretilen sınırlı ACL grant'i anlamında kabul edilebilir; invite import, proje anahtarı veya Supabase anon key tek başına ACL yazamaz.

## 3. Endpoint mailbox akışı

Gönderen Core M5d wrapper'ını proje anahtarıyla XChaCha20-Poly1305 altında şifreler; AAD `trackmesh-endpoint-mailbox-v1` (M5 tasarımı). Sonra kendi cihaz Ed25519 anahtarıyla Supabase isteğini imzalar. Edge Function imzayı, timestamp/nonce replay sınırını, aktif `project_members` kaydını, alan/boyut limitini ve iç ciphertext'in metadata tutarlılığını doğrular; ciphertext'i kaydeder. İç adres/plaintext kaydı çözülmez. Route ve auth doğrulaması transaction içinde kayıt ekleme ve nonce tüketimini atomik yapar.

`endpoint_list` yalnız aynı projedeki aktif cihaz üyesine, küçük sayfalarda ve henüz süresi dolmamış ciphertext döndürür. Alıcı her kaydı native Core içinde decrypt eder ve `Engine::apply_endpoint_record` çağırır; M5c'deki trusted author → key/PeerId binding → imza → seq → time → adres → atomik local store sırası korunur. Supabase'den gelen kayıt doğrudan learned dial listesine yazılmaz. Yerel doğrulama başarısız olursa kayıt yok sayılır ve kısmi endpoint state oluşmaz. Expiry sonrası dial adayı kullanılmaz; eski değer sessizce geri gelmez.

Endpoint kayıtlarının retention süresi en fazla iç `expires_at` kadar olmalı; örnek tasarım limiti M5'teki maksimum 15 dakikadır. Liste cursor'u endpoint tablosuna özeldir ve announcement cursor'uyla paylaşılmaz. Supabase görebildiği metadata'ya `project_route`, `writer_device_id`, `seq`, `created_at`, istek IP'si/zamanı/boyutu ve erişim örüntülerini ekleyebilir; şifreleme bunları gizlemez.

## 4. Şifreli davet drop ve atomik tek okuma

Mevcut M6 tasarımı ([phase1-m5-endpoint-design.md](phase1-m5-endpoint-design.md)) korunur: native Rust `ProjectInvite` JSON'unu serileştirir, 256-bit kriptografik anahtar ve AEAD nonce üretir, payload'u XChaCha20-Poly1305 ile şifreler; AAD `trackmesh-invite-drop-v1` domain'i, random ID ve `expires_at` değerine bağlanır. Paylaşım URI'si `trackmesh://invite/<id>#<base64url-key>` biçimindedir. Fragment HTTP'ye gitmez. Key ve plaintext invite hiçbir Supabase API'sine, React/telemetry/log/crash context'ine gönderilmez.

Yükleme için inviter cihazı, mevcut proje route'una aktif ACL üyesi olduğunu kanıtlayan Ed25519 imzalı isteği yollar. Bu yalnız ciphertext drop oluşturma hakkı verir; daveti kabul eden kişiye otomatik proje üyeliği/transfer izni vermez. Upload Function: imzayı ve üyelik durumunu doğrular; ciphertext boyutunu 64 KiB altı ile, expiry'yi en fazla 15 dakika ile sınırlar; ID kriptografik rastgele ve benzersiz olmalıdır. Function, IP başına ve ID başına upload/retrieval denemelerini sınırlar. IP bilgisi yalnız güvenilir edge gateway tarafından sağlanıyorsa kullanılır; istemcinin keyfi `X-Forwarded-For` değerine güvenilmez. Kaba kuvvet ve locator tüketme DoS riski kalır. Rate-limit sayacı ve drop yazımı tek transaction/atomik RPC ile yapılmalıdır. Kesin sayı eşikleri ve IP header kaynağı uygulama öncesi doğrulanır `[V]`.

`invite_consume` Function, ID + expiry + unconsumed koşulunu tek Postgres transaction'ında kontrol eder, satırı kilitler (`SELECT ... FOR UPDATE` veya tek `DELETE ... RETURNING`/conditional update) ve ciphertext'i döndürüp `consumed=true` işaretler; hata/yarışta generic yanıt verir. Transaction'ın atomikliği için DB işlemi Function'ın tek bir SQL RPC/transaction boundary'sinde yapılmalıdır; önce REST select, sonra ayrı update kullanmak güvenli değildir. Tüketim idempotent değildir; aynı anda gelen iki alıcıdan yalnız biri ciphertext alır. ID'yi bilen ama key'i bilmeyen saldırganın önce consume ederek alıcıyı engellemesi hâlâ mümkün ve tasarımda açık DoS riskidir. Kayıt ve nonce/rate-limit satırları için `expires_at` sonrasında `pg_cron` ile periyodik temizleme tercih edilir; extension/kota mevcut projede yoksa kontrollü Function içi batch cleanup yedeği uygulanır. Ne zaman/sıklık/limit `[V]` ve deployment doğrulamasıdır.

## 5. Ücretsiz katman, kota ve erişilememe davranışı

Resmi Supabase belgelerinde 2026-10-12 tarihinde görünen Free plan değerleri:

| Kullanım | Free plan değeri | Tasarım etkisi |
| --- | --- | --- |
| Veritabanı | 500 MB / proje | Endpoint ciphertext ve ACL/nonce/rate-limit satırları küçüktür; TTL cleanup ve büyüme gözlemi şarttır. |
| Dosya Storage | 1 GB | Bu tasarım Supabase Storage kullanmaz; ciphertext Postgres'te. Storage kotası davet upload limiti değildir. |
| Egress | 5 GB | Endpoint polling/pagination ve invite ciphertext indirmesi tüketir; küçük sayfa ve makul refresh sıklığı gerekir. |
| Edge Function çağrısı | 500.000 / ay | Her publish/list/invite upload/consume çağrısı kota tüketir; retry/backoff sınırlanmalı. |
| Edge Function runtime | 150 s worker wall-clock; istek başına 2 s CPU | İşlemler kısa, bounded ve tek DB transaction'ı olmalı; ağır iş/crypto loop yok. İmzaların sınırlı sayıda doğrulanması için uygulama hedefi oluşturulur. |
| API istekleri | Pricing sayfası “unlimited API requests” der; bu sınırsız CPU, egress veya abuse koruması anlamına gelmez. | TrackMesh Function başına/IP/device/project başına uygulama limitleri ve payload sınırları koymalı. |
| Hareketsizlik | Free projeler yetersiz DB etkinliğiyle 7 günlük pencerede otomatik duraklatılabilir; karar eşiği sabit istek sayısı olarak yayımlanmamış. | Proje duraklatılırsa otomatik endpoint ve invite hizmeti kullanılamaz. Ürün arka planda projeyi canlı tutmak için sahte ping üretmez; proje sahibi Dashboard'dan geri yükler veya ücretli plana geçer. |

Kotalar Supabase organizasyonu/proje konfigürasyonu ve ürün değişikliklerine bağlıdır. Bu tablo fiyat/availability garantisi değildir. Functions CPU/wall-clock sınırı ücretsiz katman sayılarını aşmaktan bağımsız runtime limitidir. Free plan'de 1 yıllık restore penceresi sonrası Dashboard'dan geri yükleme garantisi yoktur; davet drop'ları zaten 15 dakika sonra geçersiz olmalıdır.

Supabase timeout, quota, project pause, HTTP 5xx veya ağ hatasında TrackMesh durable/local snapshot, doğrudan açık peer bağlantısı ve manuel davet dosyası akışını çalışır tutar. Endpoint mailbox fallback'i erişilemezse status `UNKNOWN/OFFLINE` kalır; eski endpoint taze gibi sunulmaz. Invite linki kullanılamıyorsa kullanıcı hâlâ mevcut native davet dosyasını seçebilir. Sessiz retry sınırlı exponential backoff ve jitter ile, yalnız kullanıcı akışını bozmadan yapılmalıdır; erişilebilirlik garantisi verilmez.

## 6. İstemci yapılandırması ve bağımlılıklar

Proje bazında arka uç seçimi önerisi: `NONE` (yalnız doğrudan yol), `SELF_HOSTED_MAILBOX` (mevcut kendi mailbox servisi) veya `SUPABASE`. Seçim proje link/config alanında saklanır; `mailbox_url` ile aynı alanı yeniden yorumlamak yerine tipli backend seçeneği tasarlanır. Supabase seçeneği Project URL + publishable key (legacy anon key uyumluluğu gerekirse) alır. URL HTTPS ve Supabase origin/allow-list kurallarıyla doğrulanır. Invite file import/export ve doğrudan peer bağlantısı backend seçeneğinden bağımsızdır.

`service_role` / yeni `secret` key istemci konfigürasyonuna hiçbir koşulda girmez. Edge Function privilege anahtarları Dashboard secret store veya CLI ile set edilir; değerleri shell history'ye veya repoya yazdırmayan kurulum yöntemi seçilir. Örnek `.env` varsa ignore edilir ve yalnız placeholder içerir. Secret, Function loglarından ve hata yanıtlarından çıkarılır; olası sızıntıda Supabase Dashboard üzerinden döndürülür.

Mevcut kod incelemesi: Rust workspace'te `reqwest` (rustls ile) ve XChaCha20-Poly1305/Ed25519 altyapısı zaten var; M5d/M6 istemci HTTP ve kriptografisi için yeni Rust bağımlılığı ilk tasarımda gerekmiyor. Edge Function'lar TypeScript + Deno uyumlu Supabase Edge Runtime'da çalışır. TypeScript kodu ve `supabase/config.toml` gerekecektir; `supabase-js` eklemek zorunlu değil, Edge Function Postgres erişimi için mevcut Supabase SDK ile veya kontrollü SQL/RPC HTTP çağrısıyla tasarlanabilir. Rust tarafında config, imza kanonikleştirme, endpoint wrapper, client adapter ve UI seçimi gerekir. Hiçbiri bu belge kapsamında eklenmemiştir. Ed25519 imza verify API/TypeScript kütüphanesi ve SQL transaction yolunun sürüm/uyumluluğu `[V]`.

## 7. Depo yerleşimi ve kurulum akışı

Önerilen ek depo yolu `services/supabase/`:

- `supabase/config.toml` — yerel geliştirme proje ayarları (secret yok).
- `supabase/migrations/` — Postgres ACL, endpoint ciphertext, invite drop, nonce/rate-limit, indeks ve retention SQL migration'ları.
- `supabase/functions/endpoint-publish/`, `endpoint-list/`, `invite-create/`, `invite-consume/`, gerekirse `acl-grant/` — TypeScript Edge Functions.
- `README.md` — proje oluşturma, secret kurma, deploy, migration, kota/restore ve temizleme talimatları.

Kurulum ya Supabase Dashboard'da yeni proje açıp SQL Editor / Edge Function deploy ekranı kullanarak, ya da geliştirici makinesinde resmi Supabase CLI ile `supabase init`, `supabase link`, migration push, `functions deploy` adımlarından geçerek yapılır. Repo bu işe başlamadan yerel Docker/Supabase CLI kurulum varsayımı yapmaz. Project URL + publishable key istemci ayarına girer; Function secret'ları yalnız Dashboard veya `supabase secrets set` ile gönderilir. Her ortam (dev/prod) ayrı Supabase project/ref ve secret kullanır. Önce test verisiyle staging, sonra açıkça seçilmiş production projesi. Gerçek RLS deny, ACL grant/revoke, replay, transaction yarışları, expiry cleanup, kota/restore ve log redaction deployment kontrolüdür `[V]`; tasarım belgesi bunları doğrulamaz.

## 8. Tehdit modeli ve açık doğrulamalar `[V]`

| Tehdit | Tasarım önlemi | Kalan sınır |
| --- | --- | --- |
| Anon key veya doğrudan REST ile tablo erişimi | Grant kaldırma + RLS deny-all; tek giriş Function. | SQL role/grant, exposed schema ve Data API ayarı canlı projede doğrulanmalı. |
| Supabase secret/service-role sızıntısı | Yalnız Function secret; istemci/binary/depoya/loga koymama, rotation prosedürü. | Supabase Function güvenlik/operasyon hesabı ele geçirilirse ciphertext silinebilir/değiştirilebilir ve ACL manipüle olabilir; imzalı iç endpoint kaydı bunu sahte adres olarak kabul etmeyi engeller. |
| Sahte cihaz/proje üyeliği | Body hash/time/nonce imzalı isteği ve cihaz/project ACL kontrolü; explicit owner onayı ile imzalı grant. | Bootstrap/admin kaybı, key rotation, revoke gecikmesi ve cihaz saat skew politikası tasarlanmalı. |
| Replay/equivocation | Scope/path/body hash, dar zaman penceresi ve atomik nonce tüketimi; record seq alıcıda kalıcı. | Nonce table cleanup/kota, yedekten restore ve eşzamanlı transaction semantiği doğrulanmalı. |
| Mailbox/DB operatörünün endpoint içeriğini görmesi | Proje anahtarlı AEAD; iç imza ve `Engine::apply_endpoint_record` doğrulaması. | Route, device ID, seq, zaman, boyut, IP ve erişim paterni metadata'sı görünür; servis kaydı silerek/erteleyerek availability'yi etkiler. |
| Invite drop ID'nin tüketilmesi / link sızıntısı | 256-bit key fragment'te, ciphertext-only, kısa TTL, generic response, one-read transaction ve rate-limit. | ID'yi bilen saldırgan key'i bilmeden de tüketerek denial of service yapabilir; clipboard/link capture tamamen engellenemez. |
| Spam/kota tüketme | Device/project/IP limitleri, maksimum body, pagination, retry backoff, TTL cleanup. | Supabase Free sabit request kotası yayımlamıyor; maliyet/availability ve güvenilir IP kaynağı deploymentta yeniden doğrulanmalı. |
| Proje 7 gün sonra duraklar | Client manuel dosya ve direkt bağlantı yollarına düşer; UNKNOWN/OFFLINE, eski endpoint yok. | Free inactivity ölçütü sabit değil; restore Dashboard/owner gerektirir ve uzun duraklamada 1 yıl sınırı var. |
| Kötü niyetli veya yanlış yönetilen ACL | Project route + public key binding; sadece owner imzalı explicit grant/revoke. | Owner cihaz/private key kaybı için recovery/rekey kararı yok. |

Uygulama öncesi `[V]`: (1) `anon`/`authenticated` SQL grants, RLS deny-all ve Edge Function privilege yolunun canlı Supabase project'te doğrulanması; (2) Rust `MailboxAuth` kanonik imza formatını yeni scope ile tekrar kullanma/ayırma kararı; (3) TypeScript Ed25519 verify API; (4) Postgres conditional consume/nonce/rate-limit transaction yarışları; (5) IP'nin Edge gateway'den güvenilir biçimde alınması ve per-IP quota; (6) pg_cron plan/extension/free-tier davranışı ve cleanup; (7) Supabase quota/idle pause güncel davranışı; (8) URL/key project configuration, secret rotation ve local owner bootstrap; (9) gerçek Windows network outage, fallback UI ve doğrudan kanal davranışı; (10) privacy review, retention, account/billing ownership ve bölge seçimi.

## 9. Kilometre taşları ve onay kapıları

1. **M5d — Supabase endpoint mailbox tasarımı/uygulaması:** Bu belge API/ACL/SQL detaylarının tasarım incelemesi için hazırlar. Uygulama öncesi ayrıca onay: endpoint Function API'si ve imza scope/wire, Supabase tablosu/migration/retention, project ACL bootstrap ve imzalı grant/revoke, Supabase provider hesabı/bölgesi ve deployment. Onaydan sonra ciphertext-only publish/list, ayrı cursor, nonce/replay sınırı, rate-limit, cleanup ve alıcı M5c ortak Core doğrulaması uygulanır. Kontroller ve iki Windows peer elle doğrulaması ayrıca raporlanır.
2. **M5e — Core apply/expiry:** `Engine::apply_endpoint_record` zaten M5c ortak yoludur. Mailbox taşıması sonrası kaynak işaretleme, fetch pagination, dedupe/retry ve expiry tetikleme detayları için API onayı gerekir; tüm imza/trust/seq/time/address kontrolleri ve atomik store sırası aynen korunur. Supabase ACL onayı yerel trust onayının yerine geçmez.
3. **M6 — Supabase invite drop:** Ayrı uygulama onayı gerekir: public route davranışı, Postgres tabloları/atomik one-read, upload membership, 15 dakika hard TTL, 64 KiB limit, ID/IP abuse limitleri, cleanup, link/URI handling, clipboard API/flags ve privacy-retention kararı. Mevcut native dosya import/export kalır. Supabase'e invite plaintext/proje key hiçbir aşamada gönderilmez.
4. **Her aşama:** Otomatik test veya benchmark eklenmez/çalıştırılmaz. Statik kontroller ve deployment/manuel kontroller yalnız ilgili iş tamamlanıp gerçekten çalıştırıldığında kaydedilir. Bu ek hiçbir uygulama kontrolünün geçtiğini iddia etmez.

## 10. Kaynaklar (resmi Supabase; erişim 2026-10-12)

- [Billing and quotas](https://supabase.com/docs/guides/platform/billing-on-supabase) — Free DB 500 MB/proje, file Storage 1 GB, egress 5 GB, Functions 500k; kota planı.
- [Edge Function pricing](https://supabase.com/docs/guides/functions/pricing) — Free 500k çağrı/ay; aşım fiyatlandırması.
- [Edge Function limits](https://supabase.com/docs/guides/functions/limits) — Free worker 150 s, 2 s CPU/request, memory ve platform limitleri.
- [Free project pausing](https://supabase.com/docs/guides/platform/free-project-pausing) — düşük DB aktivitesiyle 7 günlük pencerede duraklatma; eşik sabit değil, Dashboard restore ve 1 yıl penceresi.
- [API keys](https://supabase.com/docs/guides/getting-started/api-keys) — publishable key'in dağıtılabilir oluşu, secret key'in backend'de tutulması.
- [Securing data](https://supabase.com/docs/guides/database/secure-data) — istemci için RLS/least privilege; secret/service role'u asla istemciye vermeme.
- [Row Level Security](https://supabase.com/docs/guides/database/postgres/row-level-security) — RLS ve service role'un RLS bypass özelliği.
- [Edge Function secrets](https://supabase.com/docs/guides/functions/secrets) — production secret'larını Dashboard/CLI ile ayarlama.
- [Edge Functions overview](https://supabase.com/docs/guides/functions) — TypeScript/Deno runtime, CLI ve deployment yolları.
- [Supabase CLI local development](https://supabase.com/docs/guides/cli/local-development) — `supabase init`, local workflow ve migration/deployment başlangıcı.
- [Project backups](https://supabase.com/docs/guides/platform/backups) — plan/backup farkları; Free plan'de backup sınırları.
