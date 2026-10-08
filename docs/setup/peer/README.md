# Peer bilgisayarı: dosyaları alan B

Bu rehber arkadaşın/alıcı bilgisayar içindir. [Ortak sıra ve sorun giderme](../README.md), [ana bilgisayarın adımları](../host/README.md).

İlk bağlantıyı **sen, A'ya doğru başlatırsın**. A erişilebilir olmalı; normal dışarı bağlantın varsa bu ilk Receive için kendi modeminde port-forward açman gerekmez. Kurumsal politika/güvenlik yazılımı dışarı bağlantıyı engelliyorsa ayrıca çözülmelidir. Daha sonra kaynak sen olacaksan erişilebilirliğin ayrıca hazırlanır.

## 1. Paketi ve hedef klasörü hazırla

- [ ] A'dan iki dosya al: **TrackMesh peer ZIP'in tamamı** ve **ayrı özel davet `.json` dosyası**.
- [ ] ZIP'i normal, yazılabilir bir klasöre çıkar. `trackmesh-agent.exe`, `trackmesh-desktop.exe`, `vcruntime140.dll` ve `CLAUDE.md` birlikte kalsın; ZIP içinden çalıştırma. `package-info.json` ve `SHA256SUMS.txt` build bilgisi/doğrulama içindir.
- [ ] Windows 10/11 x64 kullan. Rust/Node/repo gerekmez. Desktop [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) ister; CLI istemez. Eksik DLL/açılış hatasını A'ya bildir; rastgele DLL indirme.
- [ ] Paket, veri profili ve DAW hedef klasörü birbirinden ayrı olsun. İlk Receive için boş bir test klasörü seç; mevcut dosyaları silerek boşaltma.
- [ ] Davet içeriğini Claude'a, chat'e, clipboard'a veya loga koyma. Native import veya CLI join yalnızca dosya yolu ile çalışır.

## 2. Bir yol seç: Desktop veya Claude/CLI

### Desktop ile

1. `trackmesh-desktop.exe` aç. Varsayılan profil `%LOCALAPPDATA%\TrackMesh` olur; bu profili, secrets/veritabanını A'ya gönderme.
2. **Import invitation** seç; boş hedef klasörünü seçip **Choose invitation…** ile A'nın özel davetini aç. Kendi başına New UUID/Link folder yapma; projeye davet üzerinden katıl.
3. **Export my device card** ile yeni bir `.json` kart dosyası çıkar; A'ya özel kanaldan geri gönder. Davet ve cihaz kartı farklı belgelerdir.
4. A'nın **Import collaborator card** ve **Send Snapshot** yapmasını, uygulamasını açık tutmasını bekle.

### Claude ile terminalden

1. Claude Code kurulu ve giriş yapılmışsa paket klasöründe terminal açıp `claude` çalıştır. MD bir executable değildir; agent talimatı olarak okunur.
2. Şunu söyle: **“CLAUDE.md yönergesini uygula. Davet dosyamın yolunu ve boş hedef klasörümü sor; önce cihaz kartımı hazırlayalım.”**
3. Agent [CLAUDE.md](CLAUDE.md) akışına göre native `join` ve `card --output` çalıştırır. Davet payload'ını okumaz; yalnızca dosya yolunu alır.
4. Oluşan kartı A'ya özel gönder ve A'nın hazır bilgisini bekle.

CLI yolu ayrı, kalıcı `%LOCALAPPDATA%\TrackMeshPeerTrial` profili kullanır. Aynı profilde Desktop/serve/plugin owner ile CLI check/receive aynı anda çalışmasın. Mevcut kimliği/profili sıfırlama. CLI sonrasında Desktop ile devam etmek istersen aynı profille açma komutu [CLAUDE.md](CLAUDE.md) sonunda bulunur; normal çift tıklama başka profili gösterebilir.

## 3. A hazır olduğunda al ve doğrula

- [ ] A kartını import edip snapshot'ın hazır olduğunu bildirsin. Daveti import etmek tek başına karşılıklı yetkilendirmeyi tamamlamaz.
- [ ] Desktop'ta doğru A peer'ını seçip **Check peer** yap. Bağlantı denemesi **30 saniyeye kadar** sürebilir; geri sayımı bekle. Offline/last-known bilgi dosyaların şu an indirilebilir olduğunu göstermez.
- [ ] Kaynak snapshot mevcut ve Receive uygun olsun. DAW projesini kapat; Desktop'ta **I closed this project in the DAW before Receive or Recovery** kutusunu işaretle, **Receive** seç.
- [ ] CLI kullanıyorsan “Gönderen kartımı import etti, snapshot hazır. DAW kapalı; seçtiğim hedef klasöre Receive yap” de. Agent önce check, sonra receive ve son check çalıştırır.
- [ ] Apply bitmeden uygulamayı/process'i zorla kapatma. A online kalsın. Bağlantı kesilirse PAUSED session için aynı snapshot'tan Resume receive / `receive --snapshot-id` kullan.
- [ ] Başarılı Receive ardından yeniden **Check peer** yap. Alınan dosyaları/snapshot ID'yi ve fresh eşitliği kontrol et: CLI `verified_equal: true`, Desktop güncel doğrulanmış **UP TO DATE**. A yeni snapshot oluşturmuşsa farklılık ayrıca değerlendirilir.
- [ ] Dosyaları DAW'da açıp sample eksikliği/açılışı ayrıca kontrol et; transfer başarısı bu DAW kontrolünü tek başına kanıtlamaz.

## 4. Hata olursa

Timeout'ta önce A açık mı, doğru proje/peer mı, kart iki yönde import edildi mi, davet güncel mi kontrol edin. Sonra A kendi firewall, hedef LAN IP ve modem TCP/UDP eşlemesini kontrol etsin. Bir seferde bir ayar değiştirin. Ayrıntılı [belirti → sonraki adım tablosu](../README.md) ortaktır.

Receive reddedilirse doğru peer/snapshot, DAW kapanışı, yerel değişiklik, divergence ve recovery durumuna bak. Verileri silerek veya otomatik merge ile aşma. Recovery gerekiyorsa DAW kapalıyken Recover locally / CLI recover kullan; journal, staging, backup ve veritabanını koru.

Mailbox dosya taşımaz; PENDING/PUBLISHED teslim sonucu değildir. Relay hazır altyapı gerektirir ve offline A'nın yerine geçmez. A'ya sürüm/commit, Windows sürümü, son tamamlanan adım, hata metni ve snapshot/transfer ID bildir; invitation payload, keys, secrets veya veritabanı gönderme. Güncel gerçek doğrulama kaydı [IMPLEMENTATION.md](../../IMPLEMENTATION.md) içindedir.
