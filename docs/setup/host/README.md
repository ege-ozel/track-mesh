# Ana bilgisayar: projeyi gönderen A

Bu rehber ilk denemede kaynak olan bilgisayar içindir. [Ortak sıra ve sorun giderme](../README.md), [peer tarafının adımları](../peer/README.md). Kutuları kendi işlemlerini takip etmek için kullan; gerçek doğrulama kaydı [IMPLEMENTATION.md](../../IMPLEMENTATION.md) içindedir.

## 1. Uygulamayı hazırla

Windows x64 Desktop executable'ını aç. Varsayılan veri profili `%LOCALAPPDATA%\TrackMesh` olur; proje klasörünü bundan ayrı tut. Hazır paket varsa Rust/Node kurulumu gerekmez. Desktop WebView2 ister. İlk denemede küçük bir proje kopyası kullan.

Kaynak koddan hazırlıyorsan x64 Visual Studio developer shell içinde:

```powershell
Set-Location D:\CodeSpace\track-mesh\apps\desktop
npm ci
npm run typecheck
npm run build
Set-Location D:\CodeSpace\track-mesh
cargo build -p trackmesh-core -p trackmesh-desktop --locked
target\debug\trackmesh-desktop.exe
```

Bilinen çalışan toolchain Rust 1.98.1 / Node 24.19'dur; ayrıntılar [DEVELOPMENT.md](../../DEVELOPMENT.md) ve [PHASE4.md](../../PHASE4.md). Bu build installer üretmez. İlk dependency indirmesi internet ister. Aynı profilde ikinci CLI/serve/plugin network owner başlatma.

## 2. Proje, yerel IP ve public IPv4

- [ ] Küçük deneme projesini ayrı bir proje klasörüne kaydet. Yeni Ableton dosyası, bağladığın klasörün içinde olsun.
- [ ] TrackMesh'te **Link folder → New UUID → Browse… → Link project** ile ilk kez bağla. Mevcut projenin UUID'sini koru. Aynı klasörü yeni UUID ile tekrar link etmek reddedilir; aynı UUID ile relink mevcut kaydı günceller.
- [ ] **Collaboration & connection details → Connection setup** aç. Local listen portu ve router'a bağlı PC IPv4'ünü kontrol et.
- [ ] Önceki denemede görülen PC adresi **`192.168.1.200`**, local port **`43101`** idi. Diğer adaptörün `172.17.192.1` adresi yerine modemle aynı LAN'daki adresi kullan. IP değişirse aşağıdaki hedefi güncelle; mümkünse modemde DHCP adres rezervasyonu yap.
- [ ] `ipconfig` içindeki **Default Gateway** adresini tarayıcının adres çubuğuna yaz: örneğin `http://192.168.1.1`. Bu modem/router yönetim adresidir, PC hedefi veya public IP değildir.
- [ ] Yönetici giriş bilgilerini cihaz/ISP bilgileriyle edin. Wi-Fi şifresiyle aynı olmak zorunda değildir. İnternet ayarlarını korumak için şifre ararken modem fabrika resetine gitme.
- [ ] Modemin bağlı **Internet/WAN** satırındaki **IP/Maske-MAC** sütunundan WAN IPv4'ü bul; DNS, Ağ Geçidi veya MAC değerini alma. IPTV/telefon satırını seçme.
- [ ] WAN adresi `100.64.0.0/10`, `10/8`, `172.16/12` veya `192.168/16` içindeyse public değildir. ISP CGNAT veya upstream router ihtimalini ayır; erişilebilir public IPv4 ya da önceden hazırlanmış özel relay gerekir.
- [ ] TrackMesh **Public IPv4** alanına kendi WAN IPv4'ünü, **External router port** alanına **`43101`** gir. Tam public IP'yi bu belgeye, public issue'ya veya davet içeriğiyle birlikte chat'e koyma.

`\\?\D:\tm-test\a-project` biçimi Windows'un canonical/uzun yol önekidir; normal `D:\tm-test\a-project` klasörünü ifade eder. Link ederken normal yolu kullanabilirsin; veritabanındaki yolu elle değiştirme.

## 3. Modem port yönlendirmesi

**Port Yönlendirme / Sanal Sunucular** ekranında iki kural oluştur. Aşağıdaki LAN IP bu denemenin örneğidir; başka PC'de kendi Connection setup adresini kullan.

| Alan | İlk kural | İkinci kural |
| --- | --- | --- |
| Kural Adı | `TrackMesh-TCP` | `TrackMesh-UDP` |
| Adresleme Türü | IP Adresi | IP Adresi |
| Hedef IP veya MAC | `192.168.1.200` | `192.168.1.200` |
| Kaynak IP | Varsayılan `0.0.0.0` | Varsayılan `0.0.0.0` |
| WAN Arayüzü | Bağlı Internet arayüzü; ekranda `DSL_INTERNET` | Aynı arayüz |
| Kaynak Port (bu yönlendirme ekranında dış port) | `43101` | `43101` |
| Hedef Port (PC'nin local portu) | `43101` | `43101` |
| Protokol | TCP | UDP |
| Durum | Etkin | Etkin |
| Yaygın Servis Portları | Seçmeden bırak | Seçmeden bırak |

- [ ] İki kuralı **Kaydet** ve listede etkin olduklarını kontrol et.
- [ ] Dış port çakışırsa kullanılmayan bir dış port seçebilirsin: örneğin **TCP/UDP 45000 → PC_IP:43101**. TrackMesh External router port da `45000` olur; Windows local portu `43101` kalır. Adres değişirse yeni davet çıkar.
- [ ] Menü/kural kapalıysa etkinleştir; ISP/yönetici kilidi varsa yetkiliden izin iste. İki router/NAT katmanı varsa upstream yönlendirmesini de düzenlemek gerekir. Kuralın kaydolması tek başına dışarıdan erişimi doğrulamaz.

Bu alan adları kullanılan TP-Link/Türk Telekom ekranına aittir; başka firmware'lerde isimler değişebilir. [Modem kılavuzu](https://bireysel.turktelekom.com.tr/tt-cihazlar/Documents/modem-ve-internet/tplink_vx220_g2u_kullanma_kilavuzu.pdf), [port-forward sorun giderme](https://www.tp-link.com/uk/support/faq/785/).

## 4. Windows Firewall ve özel davet

- [ ] **Win + R → `wf.msc` → Gelen Kuralları → Yeni Kural → Bağlantı Noktası / Port** aç.
- [ ] **TCP → Belirli yerel bağlantı noktaları: `43101` → Allow the connection** seç.
- [ ] Aktif ağ profilini seç. Önceki denemede Wi-Fi kategorisi salt okunur kontrolle **Public** olarak görüldü; Public kutusu seçildi. Başka bilgisayarda/başka ağda kendi aktif profilini kontrol et; Domain kurumsal domain profili içindir.
- [ ] Kuralı **`TrackMesh-TCP`** adıyla bitir. Aynı adımları **UDP `43101`**, **`TrackMesh-UDP`** adıyla tekrarla; iki kural etkin olsun.
- [ ] **Allow the connection if it is secure** seçeneğini bu denemede kullanma: ayrıca Windows IPsec yapılandırması ister. TrackMesh'in şifrelemesi/peer doğrulaması farklıdır ve normal Allow ile çalışmaya devam eder. [Windows kuralı rehberi](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/configure), [IPsec seçeneğinin açıklaması](https://techcommunity.microsoft.com/blog/coreinfrastructureandsecurityblog/securing-rdp-with-ipsec/259108).
- [ ] TrackMesh'e dön; public IPv4/dış portu ve `/ip4/PUBLIC_IPV4/tcp/43101`, `/ip4/PUBLIC_IPV4/udp/43101/quic-v1` önizlemelerini kontrol et.
- [ ] **Export invitation with public addresses** ile native dosya penceresinde yeni bir `.json` adına kaydet. Manuel alanları kullanmak için bu düğmeyi seç; **Export invitation with current addresses** bu public alanları kullanmaz.
- [ ] B'ye özel kanaldan **iki dosya gönder:** `TrackMesh-PeerTrial-20261007-162413.zip` dosyasının tamamı ve export ettiğin özel davet `.json` dosyası. ZIP yerel build çıktısıdır; GitHub'a binary olarak yüklenmedi. Göndericide `.build/peer-packages` altında bulunur. Yeni build/paket varsa onun güncel dosyasını gönder; kaynak yönerge [peer/CLAUDE.md](../peer/CLAUDE.md) dosyasıdır.

Davet private capability/proje anahtarı içerir. Payload'ı chat, clipboard, log, repo veya agente verilen dosya içeriğine koyma. Public alanlar export başına geçerlidir; kalıcı network ayarı değildir. Public IP/dış port değişirse yeni daveti özel paylaş ve B'de aynı hedef klasörüne import et.

## 5. Aynı LAN ve port seçenekleri

Aynı LAN'daysanız public IPv4/modem yönlendirmesini atlayıp firewall izninden sonra **Export invitation with current addresses** kullanabilirsiniz. Guest Wi-Fi/client isolation erişimi engelleyebilir. Farklı internetlerde **Export invitation with public addresses** kullanın.

Şu WAN aralıkları public değildir:

| Aralık | Anlamı |
| --- | --- |
| `100.64.0.0/10` (100.64–100.127) | ISP paylaşımlı adres alanı; CGNAT olabilir |
| `10/8` | Özel ağ |
| `172.16/12` (172.16–172.31) | Özel ağ |
| `192.168/16` | Özel ağ |

[RFC 6598](https://www.rfc-editor.org/rfc/rfc6598) paylaşımlı adres alanını tanımlar. Her özel WAN adresi kesin CGNAT değildir; önde başka router olabilir. Modem menüsü kilitliyse ISP/yöneticiden izin iste; ISP NAT'ında erişilebilir public IPv4 veya hazırlanmış özel relay gerekir. Statik IP şart değildir. [Ortak sorun giderme tablosu](../README.md) ayrıntılı sırayı verir.

Public alanlar **Export my device card** düğmesine uygulanmaz. İlk denemeyi B'nin A'ya bağlanmasıyla yapın. Ters yönde bağımsız erişim için B kendi public-address davetini çıkarabilir; A mevcut proje klasörüne import eder. Startup `--external-address` seçeneği [PHASE4.md](../../PHASE4.md) içindedir. Listen port runtime'da değişmez; yeni portla açmak için mevcut owner'ın güvenli çıkışı gerekir.

## 6. Peer kartı gelince snapshot hazırla

- [ ] B'nin cihaz kartı `.json` dosyasını al. Bu davetten farklıdır; B'nin kamuya açık cihaz kimliğini taşır.
- [ ] Mevcut proje kartında **Collaboration & connection details → Import collaborator card** ile import et. Yalnızca davet göndermek karşılıklı yetkilendirmeyi tamamlamaz.
- [ ] Ableton test projesini kaydet; ilk denemede DAW'ı kapat. **Send Snapshot** yap; LOCAL SNAPSHOT tarihini/dosya sayısını kontrol et.
- [ ] B'ye “Kartını import ettim, snapshot hazır, TrackMesh açık” bilgisini ilet.
- [ ] B Check peer → Receive → tekrar Check peer yaparken TrackMesh'i açık tut. Dosya eklemek tek başına otomatik sync yapmaz; sonraki değişiklik için yeni Send Snapshot gerekir.
- [ ] B'den tamamlanan snapshot/transfer ID ve fresh eşitlik sonucunu al. Kaynak bu sırada yeni snapshot oluşturursa eşitlik ayrıca değerlendirilir. DAW'da açılış/sample kontrolü ayrı bir manuel kontroldür.

Mailbox yoksa PENDING metadata uyarısı kalabilir; snapshot oluştuysa direct Receive denenebilir. Bu uyarı B'nin dosyaları alıp almadığını göstermez. Denemenin bitişi, güvenli kapanış ve hata bildirimleri için [ortak rehbere](../README.md) bak.
