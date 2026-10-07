# TrackMesh: gönderen ve alıcı için deneme kontrol listesi

Bu belge ilk uzak iki-PC aktarımının işlem sırasıdır. **A = projeyi gönderen bilgisayar**, **B = arkadaşın/alıcı bilgisayar**. Kutuları kendi denemenizi takip etmek için kullanabilirsiniz; gerçek doğrulama sonuçlarının kaydı [IMPLEMENTATION.md](IMPLEMENTATION.md) içinde tutulur.

**Karşı tarafı beklerken:** A TrackMesh'i açık tutsun, test projesini kaydetsin ve doğru public adresli davetin B'ye özel kanaldan gönderildiğini kontrol etsin. B'nin cihaz kartı geldiğinde **Bölüm 5**'ten devam edin. Henüz kartı gelmediyse yeni proje UUID'si oluşturmanız veya aynı klasörü tekrar bağlamanız gerekmez.

## 1. İki tarafın sorumluluğu

Dosyalar A'dan B'ye gider; bağlantıyı ilk denemede **B, A'ya doğru başlatır**. A'nın modem/firewall ayarları A'yı erişilebilir kaynak yapar. B normal dışarı bağlantı kurabiliyorsa kendi modeminde port açması bu ilk alma akışının önkoşulu değildir.

| A: gönderen | B: alıcı |
| --- | --- |
| Test klasörünü link et, public IPv4 ve yerel PC IP'sini belirle | ZIP'i çıkar, daveti import et, boş hedef klasör seç |
| Modemde TCP/UDP yönlendirmesini ve Windows gelen izinlerini aç | Kendi cihaz kartını çıkarıp A'ya özel gönder |
| Daveti gönder, B'nin kartını import et, snapshot oluştur | A hazır olunca Check peer, Receive ve son check yap |
| Aktarım boyunca TrackMesh'i açık tut | DAW'ı kapalı tut; Receive/apply işleminin bitmesini bekle |

B'nin dışarı bağlantısı kurumsal politika/güvenlik yazılımıyla engelleniyorsa o engel ayrıca çözülmelidir. Daha sonra B kaynak olup A yeni bağlantı başlatacaksa B'nin erişilebilirliği de ayrıca hazırlanıp doğrulanır; port yönlendirme veya hazır özel relay gerekebilir.

## 2. A: proje ve adresler

- [ ] Küçük deneme projesini ayrı bir proje klasörüne kaydet. Yeni Ableton dosyası, bağladığın klasörün içinde olsun.
- [ ] TrackMesh'te **Link folder → New UUID → Browse… → Link project** ile ilk kez bağla. Mevcut projenin UUID'sini koru. Aynı klasörü yeni UUID ile tekrar link etmek reddedilir; aynı UUID ile relink mevcut kaydı günceller.
- [ ] **Collaboration & connection details → Connection setup** aç. Local listen portu ve router'a bağlı PC IPv4'ünü kontrol et.
- [ ] Bu denemede görülen PC adresi **`192.168.1.200`**, local port **`43101`** idi. Diğer adaptörün `172.17.192.1` adresi yerine modemle aynı LAN'daki adresi kullan. IP değişirse aşağıdaki hedefi güncelle; mümkünse modemde DHCP adres rezervasyonu yap.
- [ ] `ipconfig` içindeki **Default Gateway** adresini tarayıcının adres çubuğuna yaz: örneğin `http://192.168.1.1`. Bu modem/router yönetim adresidir, PC hedefi veya public IP değildir.
- [ ] Yönetici giriş bilgilerini cihaz/ISP bilgileriyle edin. Wi-Fi şifresiyle aynı olmak zorunda değildir. İnternet ayarlarını korumak için şifre ararken modem fabrika resetine gitme.
- [ ] Modemin bağlı **Internet/WAN** satırındaki **IP/Maske-MAC** sütunundan WAN IPv4'ü bul; DNS, Ağ Geçidi veya MAC değerini alma. IPTV/telefon satırını seçme.
- [ ] WAN adresi `100.64.0.0/10`, `10/8`, `172.16/12` veya `192.168/16` içindeyse public değildir. ISP CGNAT veya upstream router ihtimalini ayır; erişilebilir public IPv4 ya da önceden hazırlanmış özel relay gerekir.
- [ ] TrackMesh **Public IPv4** alanına kendi WAN IPv4'ünü, **External router port** alanına **`43101`** gir. Tam public IP'yi bu belgeye, public issue'ya veya davet içeriğiyle birlikte chat'e koyma.

`\\?\D:\tm-test\a-project` biçimi Windows'un canonical/uzun yol önekidir; normal `D:\tm-test\a-project` klasörünü ifade eder. Link ederken normal yolu kullanabilirsin; veritabanındaki yolu elle değiştirme.

## 3. A: modem port yönlendirmesi

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

## 4. A: Windows Firewall ve özel davet

- [ ] **Win + R → `wf.msc` → Gelen Kuralları → Yeni Kural → Bağlantı Noktası / Port** aç.
- [ ] **TCP → Belirli yerel bağlantı noktaları: `43101` → Allow the connection** seç.
- [ ] Aktif ağ profilini seç. Bu denemede Wi-Fi kategorisi salt okunur kontrolle **Public** olarak görüldü; Public kutusu seçildi. Başka bilgisayarda/başka ağda kendi aktif profilini kontrol et; Domain kurumsal domain profili içindir.
- [ ] Kuralı **`TrackMesh-TCP`** adıyla bitir. Aynı adımları **UDP `43101`**, **`TrackMesh-UDP`** adıyla tekrarla; iki kural etkin olsun.
- [ ] **Allow the connection if it is secure** seçeneğini bu denemede kullanma: ayrıca Windows IPsec yapılandırması ister. TrackMesh'in şifrelemesi/peer doğrulaması farklıdır ve normal Allow ile çalışmaya devam eder. [Windows kuralı rehberi](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/configure), [IPsec seçeneğinin açıklaması](https://techcommunity.microsoft.com/blog/coreinfrastructureandsecurityblog/securing-rdp-with-ipsec/259108).
- [ ] TrackMesh'e dön; public IPv4/dış portu ve `/ip4/PUBLIC_IPV4/tcp/43101`, `/ip4/PUBLIC_IPV4/udp/43101/quic-v1` önizlemelerini kontrol et.
- [ ] **Export invitation with public addresses** ile native dosya penceresinde yeni bir `.json` adına kaydet. Manuel alanları kullanmak için bu düğmeyi seç; **Export invitation with current addresses** bu public alanları kullanmaz.
- [ ] B'ye özel kanaldan **iki dosya gönder:** `TrackMesh-PeerTrial-20261007-162413.zip` dosyasının tamamı ve export ettiğin özel davet `.json` dosyası. ZIP yerel build çıktısıdır; GitHub'a binary olarak yüklenmedi. Göndericide `.build/peer-packages` altında bulunur.

Davet private capability/proje anahtarı içerir. Payload'ı chat, clipboard, log, repo veya agente verilen dosya içeriğine koyma. Public alanlar export başına geçerlidir; kalıcı network ayarı değildir. Public IP/dış port değişirse yeni daveti özel paylaş ve B'de aynı hedef klasörüne import et.

## 5. B: ZIP, Claude ve cihaz kartı

- [ ] ZIP'i normal, yazılabilir bir klasöre çıkar. **EXE'ler, `vcruntime140.dll` ve `CLAUDE.md` birlikte kalsın.** Sadece iki dosyayı ayıklama; ZIP'in içinden doğrudan çalıştırma. Rust/Node/repo gerekmez. Desktop WebView2 gerektirir; CLI kullanımı bunu gerektirmez.
- [ ] Özel daveti yerelde sakla; Claude'a yalnızca dosya yolunu ver, içeriğini ekleme.
- [ ] Paket klasöründe terminal aç; Claude Code hazırsa `claude` başlat. Şunu söyle: **“CLAUDE.md yönergesini uygula. Davet dosyamın yolunu ve boş hedef klasörümü sor; önce cihaz kartımı hazırlayalım.”**
- [ ] [Claude terminal yönergesine](PEER_TRIAL_CLAUDE.md) göre `join`, sonra `card --output` işlemlerini yaptır. Ayrı `%LOCALAPPDATA%\TrackMeshPeerTrial` profilini koru; aynı profilde Desktop ve CLI network işi aynı anda çalışmasın.
- [ ] Oluşan `peer-card-....json` dosyasını A'ya özel kanaldan gönder. Bu dosya özel davetten farklıdır; B'nin kamuya açık cihaz kimliğini taşır.
- [ ] A'nın kartı import ettiğini ve snapshot'ın hazır olduğunu teyit etmesini bekle.

**Bekleme noktası:** B'nin kartı A'ya gelmeden karşılıklı yetkilendirme tamamlanmaz. A'yı tanıyan daveti import etmek tek başına A'nın B'yi yetkilendirmesini sağlamaz.

## 6. Kart geldiğinde A, ardından B

1. **A:** Mevcut proje kartında **Collaboration & connection details → Import collaborator card** ile B'nin kartını import et.
2. **A:** Ableton test projesini kaydet; ilk denemede DAW'ı kapat. **Send Snapshot** yap; LOCAL SNAPSHOT tarihini/dosya sayısını kontrol et. Kaynak TrackMesh'i açık tut.
3. **A → B:** “Kartını import ettim, snapshot hazır, TrackMesh açık” bilgisini ilet.
4. **B:** Claude'a “Gönderen hazır. DAW kapalı; seçtiğim hedef klasöre Receive yap” de. Önce **Check peer / CLI check** ile bağlantıyı denesin; bağlantı bekleme penceresi 30 saniyeye kadar sürebilir.
5. **B:** Kaynak snapshot mevcut ve uygun olduğunda **Receive / CLI receive** çalıştırsın. GUI kullanıyorsa DAW-closed kutusunu işaretlesin. Apply bitmeden uygulamayı/process'i zorla kapatmasın.
6. **B:** Başarılı Receive ardından yeniden **Check peer / CLI check** yapsın. Alınan snapshot ID'sini/dosyaları ve fresh eşitlik sonucunu kontrol etsin. CLI'de `verified_equal: true`, GUI'de güncel doğrulanmış **UP TO DATE** beklenir; kaynak bu sırada yeni snapshot oluşturursa eşitlik ayrıca değerlendirilir.

Sonuçları A/B Windows sürümü, uygulama revision'ı, tamamlanan adım, hata metni ve snapshot/transfer ID ile kaydet. Native Receive, direct/relayed route ve DAW'da dosyaların açılması farklı kontrollerdir; biri diğerinin yapıldığını kanıtlamaz. Anahtar, davet payload'ı, secrets veya veritabanı paylaşma.

## 7. Karıştırılmaması gerekenler ve hatada sıra

| Durum | Anlamı / yapılacak işlem |
| --- | --- |
| Klasöre yeni `.als`/sample eklemek | Desktop klasör watcher'ı/continuous sync yoktur. Bağlı klasörde değişiklikten sonra **Send Snapshot** gerekir. VST save tetiklemesi ayrı akıştır; gerçek DAW entegrasyonu henüz doğrulanmadı. |
| `2 publications pending` | İki authored snapshot'ın metadata duyurusu mailbox yayını/onayı bekler. Dosya sayısı veya B'ye teslim sonucu değildir. Mailbox yoksa pending kalabilir; direct Receive yine denenebilir. |
| Mailbox / `PUBLISHED` | Mailbox yalnızca encrypted metadata/snapshot announcement tutar. `PUBLISHED` dosyaların B'ye alındığı anlamına gelmez. |
| Check timeout / offline | Önce A açık mı, doğru proje/peer mı, pairing iki yönde tamam mı, davet güncel mi? Ardından A'nın local IP, TCP/UDP modem kuralı ve aktif firewall profilini kontrol et. Bir seferde bir ayarı değiştir; timeout'u kesin modem teşhisi sayma. |
| CGNAT / upstream NAT | Yerel modem kuralını tekrarlamak ISP NAT'ını açmaz. ISP'den erişilebilir public IPv4 veya hazırlanmış private relay gerekir. İki yerel router varsa iki katmandaki mapping kontrol edilir. |
| Receive pasif/reddediliyor | Kaynak online/snapshot hazır mı, DAW kapalı mı, yerel değişiklik/divergence/recovery var mı? Veri silerek veya otomatik merge ile engeli aşma. |
| PAUSED transfer | Aynı snapshot/session için Resume receive veya `receive --snapshot-id` kullan. |
| Recovery gerekli | DAW kapalıyken Recover locally / CLI recover. Journal, staging ve backup'ı koru. |
| Pencere kapandı ama process kaldı | Close safely pencereyi gizleyebilir; işler/pending publication owner'ı tutabilir. Idle grace 60 saniyedir. Dosya kurulumu sürerken zorla sonlandırma. |

Ayrıntılı genel adımlar: [MANUAL_TRIAL.md](MANUAL_TRIAL.md). Karşı tarafta terminal komutları: [PEER_TRIAL_CLAUDE.md](PEER_TRIAL_CLAUDE.md). Güncel gerçek doğrulama durumu: [IMPLEMENTATION.md](IMPLEMENTATION.md).
