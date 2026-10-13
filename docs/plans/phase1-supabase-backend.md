# Aşama 1 — Supabase arka ucu tasarımı

**Durum (2026-10-12):** M5d-server/client and M6 invite-drop migration, Edge Functions, Rust client, settings, and Windows Desktop integration are implemented locally. Nothing has been deployed to Supabase. Live RLS/ACL, Deno/WebCrypto strictness equivalence, global invite abuse limits, one-read races, Windows clipboard/URI behavior, and two-peer workflows remain unverified. Checks actually run are recorded only in [IMPLEMENTATION.md](../IMPLEMENTATION.md).

Bu ek, ürün sahibinin 2026-10-12 tarihli kararıyla Aşama 1 M5d endpoint mailbox ve M6 şifreli davet bırakma için Supabase'i seçilen arka uç yapar. Mevcut yerel davet dosyası ve isteğe bağlı kendi mailbox servisi geçerliliğini korur. Relay 1.0.0 kapsamı dışındadır ve ertelenmiştir. Görev #26 açıkça M5d-server SQL/Edge Function kaynaklarını ve istek imzası spesifikasyonunu onaylamıştır; istemci, M6 invite-drop API ve deployment ayrı iş kapsamındadır.

## 1. Veri modeli ve gizlilik sınırı

Supabase Postgres'te iki ayrı tablo grubu önerilir. Kolon tipleri ve isimleri uygulama öncesi SQL/API tasarım onayı gerektirir.

### `endpoint_records`

Önerilen alanlar: `project_route` (proje UUID'si), `writer_device_id`, `seq`, `ciphertext`, `expires_at`, `created_at`. Benzersiz anahtar `(project_route, writer_device_id, seq)`; sorgu indeksi proje + zaman/sıra ve expiry temizliği için. `ciphertext` yalnız proje anahtarıyla XChaCha20-Poly1305 kullanılarak şifrelenmiş wrapper'dır. Wrapper AAD'si `trackmesh-endpoint-mailbox-v1` olur. İçinde M5 `EndpointRecord` ve imzası bulunur. `project_route`, yazar kimliği, sıra ve zaman sunucu/DB tarafından erişilebilir metadata'dır; yalnız adresler ve imzalı iç kayıt şifreli kalır.

Sunucuda plaintext adres, proje anahtarı veya imza doğrulamasında kullanılan gizli materyal bulunmaz. Edge Function cihaz imzalı isteği ve proje ACL'sini doğrular; alıcı `Engine::apply_endpoint_record` ile M5c'deki aynı güven zinciri, sıra, zaman, adres doğrulama ve atomik yerel kabul sırasını tekrar çalıştırır. Supabase ACL'si yerel `trusted_author`/proje yetkisinin yerine geçmez ve endpoint kaydı transfer yetkisi vermez.

### `invite_drops`

Önerilen alanlar: kriptografik rastgele `id` (locator), `ciphertext`, `expires_at`, `consumed`, `created_at`; davet tablosunda proje route veya davet içeriği tutulmaz. HTTP yanıtları generic olur; ID varlığı, expiry, tüketim ve decrypt başarısızlığını ayırt etmez. Davet blob'u boyutu 64 KiB'den küçük tutulur. Ciphertext, istemcide var olan XChaCha20-Poly1305 ile üretilir; `trackmesh-invite-drop-v1` AAD'si ID ve expiry'yi de bağlar. 256-bit rastgele anahtar yalnız paylaşım URI fragment'inde taşınır; sunucuya gönderilmez. Tek kullanımlık tüketim aşağıdaki §4'teki tek Postgres transaction'ı ile yapılır.

## 2. İstek doğrulama, tablo erişimi ve ACL

İstemciye yalnız Supabase Project URL ve yeni biçimli publishable key (`sb_publishable_...`) dağıtılabilir. Bunlar gizli kabul edilmez; API anahtarı kimlik veya proje üyeliği kanıtı değildir. İki tablo için doğrudan `anon` ve `authenticated` erişim grant'leri kaldırılır, RLS açılır ve hiçbir istemci politikası tanımlanmaz (deny-all). Edge Function dışında tabloya doğrudan erişim kapalıdır. Function'ın kullandığı secret key (`sb_secret_...`) RLS'yi atlar; yalnız `SUPABASE_SECRET_KEYS` JSON ortam değişkeninin `default` girdisinde tutulur, hiçbir TrackMesh binary'sine, depoya, dağıtım paketine, loga veya istemci yanıtına girmez. SQL erişimi kullanılıyorsa yalnız dar yetkili, kontrollü DB işlemleri tercih edilir; secret kullanımı yalnız Function içindeki doğrulanmış işlemlerden sonra yapılır.

`project-claim`, `member-grant`, `member-revoke`, `endpoint-publish` ve `endpoint-list` için cihaz Ed25519 imzalı istek zarfı kullanılır. Kesin wire/signing biçimi §2.1'de tanımlıdır. Project URL + yeni `sb_publishable_...` key yalnız API projesini seçer; üyelik/kimlik kanıtı değildir. Edge Function JWT doğrulaması kapalıdır çünkü kullanıcı JWT akışı yoktur; tüm işlevler uygulama Ed25519 imzası, ACL ve replay kontrollerini kendileri yapar. HTTPS zorunludur.

### 2.1 İmzalı istek wire biçimi

İstemci istekleri yalnız `POST /functions/v1/<operation>` kullanır. İşlem adları tam olarak `project-claim`, `member-grant`, `member-revoke`, `endpoint-publish`, `endpoint-list` olur; URL'deki function adı auth `scope` alanıyla birebir eşleşir. İstek gövdesi UTF-8 JSON'dur ve `x-trackmesh-auth` header'ı mevcut `MailboxAuth` gibi standart padded Base64 ile kodlanmış JSON zarfıdır:

```json
{
  "version": 1,
  "project_route": "lowercase-canonical-uuid",
  "device_id": "lowercase-canonical-uuid",
  "public_key": "64 lowercase hex chars",
  "issued_at": "2026-10-12T12:34:56.789Z",
  "nonce": "lowercase UUIDv4",
  "scope": "endpoint-publish",
  "body_hash": "64 lowercase hex chars",
  "signature": "128 lowercase hex chars"
}
```

`body_hash`, HTTP gövdesinin boşluklar dâhil **ham byte'ları** üzerinde SHA-256'nın küçük harfli hex kodlamasıdır. Mevcut `protocol::hash` BLAKE3 kullandığından bu yeni backend scope'unda BLAKE3 tekrar kullanılmaz. İmza girdisi, UTF-8 ve compact JSON olarak aşağıdaki sıralı tuple'dır; sıra, tip ve tarih gösterimi protokolün parçasıdır:

```text
serde_json::to_vec((
  "trackmesh-supabase-request-v1",
  1_u32,
  project_route,      // UUID, lowercase canonical string
  device_id,          // UUID, lowercase canonical string
  public_key,         // lowercase hex string
  issued_at,          // fixed UTC RFC3339 milliseconds as a string, ending in Z
  nonce,              // lowercase canonical UUIDv4 string
  scope,              // exact function/operation name
  body_hash           // SHA-256 lowercase hex string
))
```

Bu, mevcut `MailboxAuth::signing_bytes`'ın ordered `serde_json` tuple modelini korur; Supabase için yeni domain kullanır. `scope` operation adıdır; operation adı tek bir sabit HTTP POST function path'ine karşılık gelir. UUID'ler lowercase canonical, key/hash/signature hex değerleri lowercase; `nonce` küçük harfli, canonical UUID v4 olmalıdır. `issued_at` tam üç fractional digit ile `YYYY-MM-DDTHH:mm:ss.SSSZ` biçiminde olmalıdır; Rust istemci bu iki biçimi birebir üretmelidir. Timestamp yerel saatle ±120 saniye içinde değilse reddedilir. Body gövdesi 64 KiB üstünde olamaz; auth header 4 KiB üstünde olamaz. Zarf bilinmeyen/eksik alan kabul etmez.

`request_nonces` benzersiz anahtarı `(project_route, device_id, nonce)` olur. Başarılı transaction'da nonce tüketimi ACL kontrolü ve iş değişikliğiyle aynı DB çağrısındadır; en az 10 dakika tutulur (±2 dakika timestamp skew için güvenli replay retention). Aynı nonce farklı operation için de tekrar kullanılamaz. Endpoint payload body alanları: publish `{ "seq": "decimal-u64", "expires_at": "UTC timestamp", "ciphertext": "base64url-no-pad" }`; list `{ "after_cursor": null | "decimal-bigint" }`. Cursor DB'nin monotonik `cursor_id` değeridir; sayfa başına 8 satır. ACL body'leri §2.2'de belirtilir.

İşlem gövdeleri: claim `{ "actor_seq": "1" }`; grant `{ "actor_seq": "decimal-u64", "target_device_id": "uuid", "target_public_key": "64 lowercase hex chars" }`; revoke `{ "actor_seq": "decimal-u64", "target_device_id": "uuid" }`. Grant/revoke sequence'i auth imzasının body hash'iyle imzalanır ve `project_members.last_action_seq` ile artan sırada karşılaştırılır. `project_members.last_endpoint_seq` endpoint satırları TTL nedeniyle silindikten sonra da her üyenin en yüksek kabul edilmiş endpoint sıra numarasını saklar; replay watermark retention'dan bağımsızdır. Başarılı ya da iş kuralı nedeniyle reddedilen imzalı istekte nonce/rate state transaction içinde tutulur; auth, replay, ACL ve yazım ayrıdır/parsiyel değildir.

Başlangıç fixed-window limitleri hem cihaz hem proje bazında uygulanır; cihaz anahtarı imzalı `device_id`, proje anahtarı `project_route`'tur:

| İşlem | Cihaz | Proje |
| --- | ---: | ---: |
| `project-claim` | 3/saat | 10/saat |
| `member-grant`, `member-revoke` | 10/saat | 30/saat |
| `endpoint-publish` | 30/dakika | 120/dakika |
| `endpoint-list` | 60/dakika | 180/dakika |

Ek olarak `project-claim` için Supabase projesinin tüm route'ları boyunca tek bir **global 60/saat** limiti vardır. Supabase resmi belgeleri PostgREST `request.headers` içinden `X-Forwarded-For` okunabildiğini ve Edge loglarında `x-real-ip`/`cf-connecting-ip` header'larının kaydedildiğini gösteriyor; ancak Edge Function handler'ına gelen bu header'ın gateway tarafından client-controlled değerden arındırıldığına dair güvenilirlik garantisi bulamadım. Bu nedenle M5d-server IP başına limit veya IP depolaması yapmaz. Doğrulanmış Edge header kaynağı için `[V]` açık kalır; global claim limiti rastgele anahtar/route ile per-device ve per-project claim limitlerini aşmayı sınırlar.

Global sayaç tüm kullanıcılarca paylaşılır; bir saldırgan saatteki 60 claim hakkını tüketerek meşru claim'leri pencere sıfırlanana kadar geciktirebilir. Beklenen küçük kullanıcı tabanı için bu DoS etkisi kabul edilmiştir. Güvenilir IP kaynağı doğrulanana kadar IP başına limit `[V]` kalır.

Bu uygulama limitleri ilk abuse-control değerleridir; Supabase platform kotası değildir. Limit aşımı, route doluluğu ve diğer iş kuralı reddi aynı generic yanıtı üretir.

### 2.2 Sunucu ACL temsili ve pairing sonrası provisioning

Mevcut pairing/trust state cihazların yerel SQLite depolarındadır; Supabase bunu kendiliğinden bilemez. `project_members(project_route, device_id, public_key, role, status, grant_seq, grant_signature, granted_at, revoke_seq, revocation_signature, revoked_at, last_action_seq, last_endpoint_seq)` sunucu ACL'si olur. Function, isteğin imza anahtarının aynı proje route'unda `active` üye olduğunu kontrol eder. Bir projede en fazla üç aktif cihaz vardır. Revoke edilmiş cihaz yeni endpoint yayımlayamaz veya endpoint listesini okuyamaz. Bu ACL, yalnız metadata servisine erişim iznidir; dosya transfer yetkisi sağlamaz.

**Bootstrap (kullanıcı kararı): TOFU project claim.** Projede Supabase'i ilk etkinleştiren cihaz `project-claim` isteğini kendi Ed25519 imzasıyla gönderir. Function/DB atomik olarak route'ta hiçbir üyelik satırı bulunmadığını kontrol eder ve claim'i yapan device/key çiftini ilk aktif `owner` yapar. Route'ta geçmişte revoke edilmiş satır bile varsa yeni claim reddedilir. Çakışma generic hata döndürür; route varlığını açığa çıkarmaz. Enrollment kodu yoktur. Risk: proje UUID'sini gerçek cihazdan önce öğrenen biri route'u claim edebilir. Gerçek cihaz da aynı generic hata alır; bu durumda ACL kurtarma/route reset tasarlanana kadar elle davet dosyası ve doğrudan peer bağlantısı kullanılabilir. Supabase arka ucu için owner identity reset akışı bu işte yoktur. M5d-server uygulamasındaki SQL `trackmesh_dispatch` bu TOFU claim'i route bazında transaction lock'u altında uygular.

**Üyelik grant/revoke (kullanıcı kararı):** mevcut, aktif herhangi bir üye, yerel pairing challenge/proof ve inviter'ın açık onayından sonra yeni cihaz için `member-grant` imzalı isteği gönderebilir. Grant body'si `actor_seq`, `target_device_id`, `target_public_key` taşır; onaylayan aktif üyenin auth imzası bu body hash'ini kapsar. Bu grant onaylayan cihazın mevcut yerel pairing sonucunu beyan etmesidir; Supabase doğrudan pairing transport'ına bakmaz. Yeni kayıt `member` olur; owner yetkisi grant ile devredilmez. `member-revoke` body'si `actor_seq`, `target_device_id` taşır. Her aktif üye kendini çıkarabilir; başka cihazı yalnız aktif owner çıkarabilir. Her imza sahibinin `actor_seq` değeri proje başına monotonik artmalı ve tabloda kalıcı `last_action_seq` ile atomik olarak ilerlemelidir. Grant/revoke imzası ve signer kimliği üyelik satırında kanıt olarak tutulur. Üçüncü aktif üye sonrası yeni grant reddedilir.

Her SQL/Edge hata yanıtı genel `request rejected` gövdesi döndürür; var olmayan route, dolu route, yanlış key, revoked member, quota ve geçersiz claim ayrıştırılmaz. Claim, grant/revoke, endpoint publish ve endpoint list aynı imzalı istek/replay modelini kullanır. Uygulama onayı artık verilmiştir: M5d-server SQL şeması/migration, Edge Function API, yeni imza scope'ları, ACL modeli ve deploy edilecek servis kaynakları onay kapsamındadır; Rust istemci M5d-client işidir. “automatic service ACL update yok” eski sınırı bu karar ile değişmiştir; yalnız imzalı request ve yerel pairing sonrası üye imzalı grant üzerinden güncellenir.

Owner da dahil bir üyenin kendini revoke etmesine izin verilir. Owner kendini revoke ederse aktif member'lar metadata işlemlerini sürdürebilir ve grant edebilir; owner rolü olan aktif cihaz kalmadığından başka üyeyi revoke edemez ve route yeniden claim edilemez. Bu kurtarma sınırı kullanıcıya gösterilmelidir; transfer-owner / owner recovery ayrı tasarım ister `[V]`. M5d-server bu kararı uygular, otomatik owner recovery içermez.

## 3. Endpoint mailbox akışı

Gönderen Core M5d wrapper'ını proje anahtarıyla XChaCha20-Poly1305 altında şifreler; AAD `trackmesh-endpoint-mailbox-v1` (M5 tasarımı). Sonra kendi cihaz Ed25519 anahtarıyla Supabase isteğini imzalar. Edge Function imzayı, timestamp/nonce replay sınırını, aktif `project_members` kaydını, alan/boyut limitini ve iç ciphertext'in metadata tutarlılığını doğrular; ciphertext'i kaydeder. İç adres/plaintext kaydı çözülmez. Route ve auth doğrulaması transaction içinde kayıt ekleme ve nonce tüketimini atomik yapar.

`endpoint_list` yalnız aynı projedeki aktif cihaz üyesine, küçük sayfalarda ve henüz süresi dolmamış ciphertext döndürür. Alıcı her kaydı native Core içinde decrypt eder ve `Engine::apply_endpoint_record` çağırır; M5c'deki trusted author → key/PeerId binding → imza → seq → time → adres → atomik local store sırası korunur. Supabase'den gelen kayıt doğrudan learned dial listesine yazılmaz. Yerel doğrulama başarısız olursa kayıt yok sayılır ve kısmi endpoint state oluşmaz. Expiry sonrası dial adayı kullanılmaz; eski değer sessizce geri gelmez.

Endpoint kayıtlarının retention süresi en fazla iç `expires_at` kadar olmalı; örnek tasarım limiti M5'teki maksimum 15 dakikadır. Liste cursor'u endpoint tablosuna özeldir ve announcement cursor'uyla paylaşılmaz. Supabase görebildiği metadata'ya `project_route`, `writer_device_id`, `seq`, `created_at`, istek IP'si/zamanı/boyutu ve erişim örüntülerini ekleyebilir; şifreleme bunları gizlemez.

`head_records` offline hint'leri cihazın kendi authored snapshot'ını değil, o cihazın projedeki **mevcut local head**'ini bildirir. Bu nedenle cihaz, başka bir eşten aldığı imzalı snapshot'ı kendi writer kimliği ve mevcut imza/AEAD doğrulama biçimiyle yayımlayabilir; record içindeki author, writer'dan farklı olabilir. Client writer imzasını writer cihazının trusted key'i ile doğrular ve `(project, writer)` başına en yüksek `seq` değerini tutar. Wrapper, AAD, imza byte'ları, sunucu sözleşmesi ve tablo şeması değişmez. Eski kayıtlarda `author == writer` olduğundan geriye dönük doğrulama sürer.

## 4. Şifreli davet drop ve atomik tek okuma

The following first-pass proposal is retained as design history. The concrete local M6 implementation and selected limits are recorded immediately before §5; those implementation details supersede unverified rate-limit and cleanup suggestions below.

Mevcut M6 tasarımı ([phase1-m5-endpoint-design.md](phase1-m5-endpoint-design.md)) korunur: native Rust `ProjectInvite` JSON'unu serileştirir, 256-bit kriptografik anahtar ve AEAD nonce üretir, payload'u XChaCha20-Poly1305 ile şifreler; AAD `trackmesh-invite-drop-v1` domain'i, random ID ve `expires_at` değerine bağlanır. Paylaşım URI'si `trackmesh://invite/<id>#<base64url-key>` biçimindedir. Fragment HTTP'ye gitmez. Key ve plaintext invite hiçbir Supabase API'sine, React/telemetry/log/crash context'ine gönderilmez.

Yükleme için inviter cihazı, mevcut proje route'una aktif ACL üyesi olduğunu kanıtlayan Ed25519 imzalı isteği yollar. Bu yalnız ciphertext drop oluşturma hakkı verir; daveti kabul eden kişiye otomatik proje üyeliği/transfer izni vermez. Upload Function: imzayı ve üyelik durumunu doğrular; ciphertext boyutunu 64 KiB altı ile, expiry'yi en fazla 15 dakika ile sınırlar; ID kriptografik rastgele ve benzersiz olmalıdır. Function, IP başına ve ID başına upload/retrieval denemelerini sınırlar. IP bilgisi yalnız güvenilir edge gateway tarafından sağlanıyorsa kullanılır; istemcinin keyfi `X-Forwarded-For` değerine güvenilmez. Kaba kuvvet ve locator tüketme DoS riski kalır. Rate-limit sayacı ve drop yazımı tek transaction/atomik RPC ile yapılmalıdır. Kesin sayı eşikleri ve IP header kaynağı uygulama öncesi doğrulanır `[V]`.

`invite_consume` Function, ID + expiry + unconsumed koşulunu tek Postgres transaction'ında kontrol eder, satırı kilitler (`SELECT ... FOR UPDATE` veya tek `DELETE ... RETURNING`/conditional update) ve ciphertext'i döndürüp `consumed=true` işaretler; hata/yarışta generic yanıt verir. Transaction'ın atomikliği için DB işlemi Function'ın tek bir SQL RPC/transaction boundary'sinde yapılmalıdır; önce REST select, sonra ayrı update kullanmak güvenli değildir. Tüketim idempotent değildir; aynı anda gelen iki alıcıdan yalnız biri ciphertext alır. ID'yi bilen ama key'i bilmeyen saldırganın önce consume ederek alıcıyı engellemesi hâlâ mümkün ve tasarımda açık DoS riskidir. Kayıt ve nonce/rate-limit satırları için `expires_at` sonrasında `pg_cron` ile periyodik temizleme tercih edilir; extension/kota mevcut projede yoksa kontrollü Function içi batch cleanup yedeği uygulanır. Ne zaman/sıklık/limit `[V]` ve deployment doğrulamasıdır.

M6 implementation (2026-10-12): the ID is 256 random bits encoded as 43 base64url characters; the separate key is 256 bits. The fixed AAD byte sequence is `trackmesh-invite-drop-v1 || 0x00 || id || 0x00 || expires_at` using UTF-8. Ciphertext's base64url representation is <64 KiB. Expiry is generated at 15 minutes and SQL rejects values after that hard TTL. `ProjectInvite` has an optional backend descriptor and signature. New invites sign all ordered contents, including Supabase URL and `sb_publishable_...` key, with the inviter card's device key. Historical invitations without the fields/signature remain accepted; unsigned invitations cannot configure a backend.

The inviter uses signed `invite-create`; SQL checks active ACL membership, consumes the request nonce, enforces expiry/size, and inserts the unique ciphertext ID atomically. `invite-consume` is intentionally unsigned because the recipient has not yet joined the project. It sends only the random ID; a person who learns it may consume first and deny the intended recipient. A single `DELETE ... WHERE id=p_id AND expires_at>clock_timestamp() RETURNING` ensures one winner and removes ciphertext immediately. The `invite_drops` table has no project route or plaintext. RLS is deny-all and direct client grants are revoked; secret-key RPC calls are made only by Edge Functions.

Initial abuse limits: create 60/hour global across the Supabase project; consume 300/hour global and 10/hour per ID. Global counters can be exhausted to delay legitimate use; this is accepted. Edge client-IP provenance is not trusted, and IPs are not stored. Cleanup runs in each RPC transaction and removes at most 500 expired drops, 1,000 old nonces, and 1,000 old rate buckets; no pg_cron extension is required.

New invite links use `trackmesh://invite/v2/<ref>/<publishable_key>/<id>#<key>` and carry the inviter's Supabase service hint; they also carry the inviter's reachable addresses. The receiver does not enter a URL or key for a new-format link. After decryption, the client verifies that the signed invite's Supabase host and publishable key match the service used to consume the drop. If the receiver has no global default service, the link's service becomes the default; an existing different default is left unchanged. The legacy `trackmesh://invite/<id>#<key>` form remains supported and uses the receiver's default service, or asks for the service once if no default exists. Build-time defaults use `TRACKMESH_DEFAULT_SUPABASE_URL` and `TRACKMESH_DEFAULT_SUPABASE_PUBLISHABLE_KEY`. New API keys come from **Project Settings → API Keys**: client requests send only `apikey: sb_publishable_...` and never use Authorization Bearer. `sb_secret_...` keys belong only in the server-side `SUPABASE_SECRET_KEYS` JSON dictionary's `default` entry; RPC requests send only `apikey: sb_secret_...`. A secret key or database password must never be given to a TrackMesh client or user. After decryption, the signed invite carries the project backend URL/key and import stores them only after strict `*.supabase.co` validation. File invites follow the same auto-configuration path.

Supabase GitHub integration uses working directory `services/supabase` and branch `master`; migration files are applied automatically when `master` is pushed. Never edit a migration after it has been deployed; create a new migration for corrections. If the integration does not deploy Edge Functions, use `supabase functions deploy <function-name> --no-verify-jwt` from `services/supabase`.

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

M5d-client URL politikası: yalnız HTTPS `*.supabase.co` origin'leri kabul edilir; kullanıcı tarafından yapılandırılmış özel origin bu sürümde desteklenmez. URL'de kullanıcı bilgisi, path, query, fragment veya özel port kabul edilmez. Backend seçimi, Project URL ve publishable/anon key proje başına versiyonlu `settings.json` içinde atomic replace ile saklanır; SQLite şeması değişmez. `settings.json` bozuksa veya daha yeni bilinmeyen sürümdeyse Supabase seçimi kapalı kalır ve dosya ezilmez. Publishable/anon key sır değildir ancak UI/React loglarına, tracing'e veya hata metnine yazılmaz; hiçbir service-role/secret key alanı yoktur. `NONE` ve `SELF_HOSTED_MAILBOX` mevcut `mailbox_url` davranışını yeniden yorumlamaz.

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
| Sahte cihaz/proje üyeliği | Body hash/time/nonce imzalı isteği ve cihaz/project ACL kontrolü; mevcut aktif üyenin yerel pairing onayı sonrası imzalı grant. | TOFU claim öncesi proje UUID'sini öğrenen kişi route'u claim ederek hizmeti engelleyebilir; owner/admin kurtarma ve key rotation yok. |
| Replay/equivocation | Scope/path/body hash, dar zaman penceresi ve atomik nonce tüketimi; record seq alıcıda kalıcı. | Nonce table cleanup/kota, yedekten restore ve eşzamanlı transaction semantiği doğrulanmalı. |
| Mailbox/DB operatörünün endpoint içeriğini görmesi | Proje anahtarlı AEAD; iç imza ve `Engine::apply_endpoint_record` doğrulaması. | Route, device ID, seq, zaman, boyut, IP ve erişim paterni metadata'sı görünür; servis kaydı silerek/erteleyerek availability'yi etkiler. |
| Invite drop ID'nin tüketilmesi / link sızıntısı | 256-bit key fragment'te, ciphertext-only, kısa TTL, generic response, one-read transaction ve rate-limit. | ID'yi bilen saldırgan key'i bilmeden de tüketerek denial of service yapabilir; clipboard/link capture tamamen engellenemez. |
| Spam/kota tüketme | Device/project/IP limitleri, maksimum body, pagination, retry backoff, TTL cleanup. | Supabase Free sabit request kotası yayımlamıyor; maliyet/availability ve güvenilir IP kaynağı deploymentta yeniden doğrulanmalı. |
| Proje 7 gün sonra duraklar | Client manuel dosya ve direkt bağlantı yollarına düşer; UNKNOWN/OFFLINE, eski endpoint yok. | Free inactivity ölçütü sabit değil; restore Dashboard/owner gerektirir ve uzun duraklamada 1 yıl sınırı var. |
| Kötü niyetli veya yanlış yönetilen ACL | Project route + public key binding; aktif üye onay sonrası grant edebilir; yalnız owner başka üyeyi revoke eder. | Owner kendini revoke ederse başka üyeyi çıkaracak admin kalmayabilir; owner recovery/transfer kararı yok. |

Uygulama/deployment doğrulaması `[V]`: (1) `anon`/`authenticated` SQL grants, RLS deny-all ve Edge Function privilege yolunun canlı Supabase project'te doğrulanması; (2) M5d-client Rust kanonik tuple/signature çıktısının Ed25519 WebCrypto doğrulamasıyla byte-byte eşleşmesi; WebCrypto Ed25519 doğrulamasının Rust `ed25519_dalek::verify_strict` ile aynı canonicality/strictness koşullarını sağladığı ayrıca kanıtlanmadı; malformed/noncanonical imza davranışı staging'de doğrulanmalı; (3) Postgres claim/grant/revoke, nonce/rate-limit transaction yarışları ve üç cihaz sınırı; (4) Supabase project claim TOFU yarışı ve generic conflict; (5) M5d-client URL/key configuration ve secret rotation; (6) Supabase quota/idle pause güncel davranışı; (7) gerçek Windows network outage, fallback UI ve doğrudan kanal davranışı; (8) privacy review, retention, account/billing ownership ve bölge seçimi; (9) Edge Function'a gelen client-IP header'ın spoof edilmeyen kaynağını Supabase resmi doküman/gerçek deployment ile belirleme. Invite/IP rate-limit, pg_cron ve invite-drop ayrıntıları M6 öncesi ayrıca doğrulanır.

## 9. Kilometre taşları ve onay kapıları

1. **M5d-server — Supabase sunucu yüzeyi:** Görev #26 onayı ile uygulanmıştır: SQL tabloları/RLS deny-all, tek transaction dispatch RPC, beş Ed25519 doğrulamalı Edge Function ve signed tuple spec. Deno/Supabase CLI ortamda yoksa kontroller çalıştırılamaz; staging deploy, canlı ACL/RLS, concurrency ve WebCrypto runtime doğrulaması kalır.
2. **M5d-client — Rust istemci ve config:** Yerel olarak uygulanmıştır. Ham body SHA-256, `trackmesh-supabase-request-v1` tuple, UUIDv4 lowercase nonce, tam UTC milis timestamp, function scope, publishable/anon headers, HTTPS `*.supabase.co` URL kuralı ve versiyonlu `settings.json` kullanılır. Endpoint wrapper biçimi bu belgede ve `services/supabase/README.md` içinde tanımlıdır. Canlı imza uyumu, claim/grant ve iki Windows peer e2e işlemleri `[V]` kalır.
3. **M5e — Core apply/expiry:** `Engine::apply_endpoint_record` mailbox kayıtlarında da ortak yol olarak kullanılır. Bellekte cursor ile bounded pagination, trusted card kaynak seçimi, duplicate/stale yutma, retry/backoff ve expiry kontrolleri M5d-client'te uygulanmıştır. Tüm imza/trust/seq/time/address kontrolleri ve atomik store sırası korunur. Canlı Windows peer doğrulaması `[V]` kalır; Supabase ACL'si yerel trust onayının yerine geçmez.
4. **M6 — Supabase invite drop:** `invite_drops` table/Function bu migration'a bilerek eklenmedi. Ayrı görevde signed uploader, 256-bit fragment key, `trackmesh-invite-drop-v1`, 15 dakika hard TTL, 64 KiB limit, ID/IP abuse limitleri, atomik one-read, cleanup, URI/clipboard ve privacy-retention uygulanır. Mevcut native dosya import/export kalır. Supabase'e invite plaintext/proje key hiçbir aşamada gönderilmez.
5. **Her aşama:** Otomatik test veya benchmark eklenmez/çalıştırılmaz. Statik kontroller ve deployment/manuel kontroller yalnız ilgili iş tamamlanıp gerçekten çalıştırıldığında kaydedilir.

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
- [Edge Functions Dashboard quickstart](https://supabase.com/docs/guides/functions/quickstart-dashboard) — Dashboard editor ile function oluşturma/deploy etme.
- [Supabase CLI local development](https://supabase.com/docs/guides/cli/local-development) — `supabase init`, local workflow ve migration/deployment başlangıcı.
- [Supabase CLI reference](https://supabase.com/docs/reference/cli) — `db push`, function deploy ve `--no-verify-jwt` seçenekleri.
- [Web Crypto Ed25519 importKey](https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/importKey) — raw Ed25519 public key formatı; Supabase Deno runtime'da ayrıca doğrulanmalıdır.
- [PostgreSQL JSON functions](https://www.postgresql.org/docs/current/functions-json.html) — `jsonb_object_keys` set-returning function; body key-count SQL'i bu fonksiyonla yapılır.
- [Supabase Securing your API](https://supabase.com/docs/guides/api/securing-your-api) — `request.headers` içindeki `X-Forwarded-For` erişim örneği (PostgREST bağlamı; Edge handler header güvenilirliğini tek başına kanıtlamaz).
- [Supabase log field reference](https://supabase.com/docs/guides/observability/log-field-reference) — Gateway loglarına alınabilen `x-real-ip`/`cf-connecting-ip` header adları; spoof-resistant Edge header garantisi vermez.
- [Project backups](https://supabase.com/docs/guides/platform/backups) — plan/backup farkları; Free plan'de backup sınırları.
