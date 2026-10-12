# TrackMesh: arkadaş bilgisayarında ilk Receive

Bu belge dağıtım paketinde `CLAUDE.md` adıyla bulunur. Paket bir installer değildir: Windows x64 için `trackmesh-desktop.exe`, terminal işlemleri için `trackmesh-agent.exe`, gereken app-local Visual C++ runtime DLL'leri ve bu yönergeyi içerir. Aynı Wi-Fi denemesi için paketteki `AYNI-WIFI-DENEMESI.md` belgesini izleyebilirsiniz. EXE/DLL dosyalarını aynı klasörde tutun. Rust/Node/repo arkadaş bilgisayarında gerekmez. Desktop [Microsoft WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) ister; CLI WebView2 kullanmaz. Eksik DLL veya açılış hatası varsa hatayı gönderene bildirin; rastgele DLL indirmeyin.

## Arkadaşın başlayacağı yer

1. ZIP'i normal, yazılabilir bir klasöre çıkarın. ZIP'in içinden doğrudan çalıştırmayın.
2. Gönderenden özel davet `.json` dosyasını ayrıca alın. Dosyayı Claude'a eklemeyin/içeriğini yapıştırmayın; yalnızca yerel dosya yolunu verin.
3. Bu paket klasöründe PowerShell açın. Claude Code kurulu ve giriş yapılmışsa `claude` komutuyla başlatın. Proje dizinindeki `CLAUDE.md` başlangıçta talimat olarak okunur; MD bir executable değildir. [Claude Code belgesi](https://code.claude.com/docs/en/memory).
4. Claude'a şunu söyleyin: **“CLAUDE.md yönergesine göre TrackMesh denemesini başlat. Davet dosyamın yolunu ve boş hedef klasörümü sor; eşleştirme isteğini otomatik gönderelim. Gönderen cihazımı onaylayıp snapshot oluşturunca dosyaları alacağız.”**
5. Gönderen cihaz isteğini **Pair collaborator → Approve device** ile onaylasın ve Send Snapshot yapıp uygulamasını açık tutsun. Kart dosyası göndermezsiniz. Sonra Claude'a **“Gönderen cihazımı onayladı, snapshot hazır. DAW kapalı; seçtiğim hedef klasöre Receive yap.”** deyin.

Bağlantı başarılı olursa Receive proje dosyalarını seçilen klasöre indirir, doğrular ve yerleştirir. Paket tek başına projeye erişim vermez; davet, karşılıklı pairing, kaynak snapshot ve erişilebilir/online kaynak gerekir. Sürekli sync veya otomatik DAW açma yoktur.

## Birleşik eşleştirme akışı

Yeni Desktop daveti kabul edince cihaz isteğini şifreli bağlantıdan otomatik gönderir. CLI'de `join`, güvenli `status` ve yeni `request-pairing` aynı işi yapar. Gönderenin 10 dakikalık eşleştirme penceresi açık olmalı; son yetki için cihaz kimliğini doğrulayıp onaylaması gerekir. Alıcı onayı 60 saniyeye kadar sorgular; sonra retry yapılabilir. Proje anahtarı pairing mesajıyla gönderilmez ve agent context'ine girmez; ilk davet dosyası özel kanaldan paylaşılır ve yalnızca native import tarafından okunur. Her iki tarafta otomatik eşleştirme build'i gerekir; `20261007` paketi `request-pairing` içermez. `20261010` ConnectionCheck paketi yeni Desktop bağlantı yardımcısını içerir; önceki `20261009` paketinde bu panel yoktur. Kullanıcı eski build ile kalmayı seçerse manuel `card --output`/Import collaborator card eski akışı kullanılabilir; otomatik başarı diye raporlama.

## Göndereni nasıl buluruz; panel ne gösterir?

Gönderenin uygulamayı açması bildirim/public kişi listesi oluşturmaz. Davetin native import'u proje adını/UUID'sini, gönderenin imzalı cihaz kimliğini/PeerId'sini ve adreslerini yerelde kaydeder. Desktop'ta `Collaborator` listesinde, CLI'de güvenli `status` içinde gönderen görünür. Native network komutu bu adreslere bağlanır. Kaydın görünmesi online bağlantı veya dosya teslimi değildir.

Gönderende yalnızca geçerli davet proof'u üreten onaysız adaylar Pair collaborator altında Device/Identity ile görünür; canlı bağlantı göstergesi ve kalıcı giriş geçmişi değildir. Beklenen public kimlikleri karşılaştırıp Approve device seçilir. Onaydan sonra Desktop seçili cihaz için DIRECT / RELAYED / OFFLINE gösterir; fresh Check peer sonucu ayrı doğrulamadır. Daveti açmayan kişiler ve bütün başarısız/yetkisiz denemeler listelenmez.

Yerel SQLite vardır; Supabase, browser deployment veya herkese açık adres rehberi bu paketin parçası değildir. Gönderenin güncel public IP/dış portu ve erişilebilirliği gerekir. Public IP/dış port değiştiyse gönderenden yeni adresli özel davet iste; aynı veri profili/proje/yerel hedefe native join ile adres güncelle. Bu, yalnızca bağlantı timeout olduğunda projeyi yeniden import etmeye izin veren genel bir adım değildir. Telefon hotspot'u/CGNAT'ta erişilebilir ağ veya operatörün hazırladığı private relay gerekir; database NAT engelini kaldırmaz.

Onaylı bir cihaz her yeni bağlantıda eşleştirme penceresi istemez. CLI request-pairing/check/receive yalnızca komut boyunca online kalır; bitince runtime kapanır. Açık bağlantı iki yönlüdür. Alıcı daha sonra kaynak olacaksa aynı profille Desktop veya açık serve oturumu kullanır; alacak taraf DAW kapalıyken check/receive/check yapar, kaynak snapshot oluşturur. Aynı profilde iki network owner açma; divergence kontrollerini aşma. Offline kaynağın dosyaları mailbox'tan gelmez; mailbox snapshot duyurusu tutar ve offline pairing kuyruğu bu build'de yoktur.

## Claude için görev ve sınırlar

Bu Windows kullanıcısına küçük bir test projesini terminalden aldır. Kullanıcıya Türkçe ve kısa konuş. Her aşamadaki somut sonucu bildir; bağlantı hatasında tüm akışı baştan başlatma.

- Davet dosyasını araçla/metin olarak okuma; `Get-Content`, JSON parse, clipboard, attachment veya log ile payload/project key'i açığa çıkarma. Dosyayı yalnızca native CLI'nin `join --invite-file` argümanı olarak geçir.
- `secrets`, device/project keys ve veritabanını okuma/paylaşma. Davet çıktısı üreten `invite` komutunu bu alma akışında kullanma. Kimlikleri güvenli `status` çıktısından öğren.
- Bilgisayardaki mevcut proje/veri klasörlerini silme, sıfırlama veya otomatik conflict merge yapma. Hedefi kullanıcı seçsin; ilk deneme için boş klasör kullan. Hedef/data/paket klasörleri birbirinden ayrı olsun.
- İlk denemede ayrı `%LOCALAPPDATA%\TrackMeshPeerTrial` veri profili kullan; profil zaten varsa kimliği koru. Komutlar Windows kullanıcısının kendi hesabında çalışsın; başka hesap altında DPAPI anahtarları açılamaz.
- CLI `request-pairing`/`check`/`receive` çalışırken aynı profilde Desktop, `serve` veya plugin owner açma. Tek network runtime lease vardır. Önceden açık owner varsa onu zorla öldürme; güvenli kapanış/işlerin bitmesini bekle veya kullanıcıyla ayrı bir profil seç.
- Receive öncesinde hedef klasör ve DAW'ın kapalı olduğu kullanıcı tarafından belirtilmiş olsun. Belirtildiyse aynı onayı tekrar sorma. Apply sırasında process'i zorla sonlandırma.
- Firewall/router ayarlarını otomatik değiştirme, admin/UAC veya servis kurma. Gerekirse aşağıdaki rehberle kullanıcıyı yönlendir. Claude'un normal komut izinlerini atlatma.
- Tamamlandı demek için Receive'in başarıyla bitmesini ve son fresh check sonucunu gör. Timeout/PENDING/last-known durumunu başarı sayma. Kalan engeli açıkça bildir.

## Terminal akışı — PowerShell

Bu bloklar yönergedir; her komutun sonucunu kontrol ederek ilerle. Hata varsa sonraki bağımlı adımı çalıştırma. Claude farklı shell kullanıyorsa PowerShell komutlarını olduğu gibi bash'e yapıştırmasın. Native komutlardan sonra `$LASTEXITCODE` kontrol et.

### A. Executable'ları ve yolları hazırla

Paket klasöründe:

```powershell
$trialAgent = (Resolve-Path -LiteralPath '.\trackmesh-agent.exe').Path
$trialData = Join-Path $env:LOCALAPPDATA 'TrackMeshPeerTrial'
& $trialAgent --help
& $trialAgent join --help
& $trialAgent request-pairing --help
& $trialAgent receive --help
```

Kullanıcıdan `$trialInvite` (özel davetin tam yolu) ve `$trialFolder` (boş hedef klasörün tam yolu) için değer al. Klasör yoksa belirtilen yerde oluştur; varsa boş olduğunu dosya isimlerini/metadata'yı kontrol ederek doğrula. Dolu klasör seçilmişse ilk deneme için başka boş klasör iste; içerik silme. Sadece davet dosyasının varlığını kontrol et, içeriğini açma.

### B. Daveti native olarak import et

```powershell
& $trialAgent --data-dir $trialData --device-name 'Peer trial PC' join --invite-file $trialInvite --folder $trialFolder
```

Mevcut join çıktısı manuel kart önerir; yeni akışta aşağıdaki request-pairing kullanılır. Kart dosyası üretmen/göndermen gerekmez. Proje zaten aynı hedefe import edilmişse kimliği koru; sıradan retry için tekrar join veya New UUID yapma. Gönderenin adresi gerçekten değişmiş ve güncel davet alınmışsa aynı proje/hedefe native join ile adres güncellemesi yapılabilir.

### C. Proje ve gönderen kimliğini güvenli status'tan seç

```powershell
$trialStatus = & $trialAgent --data-dir $trialData status | ConvertFrom-Json
```

`projects[].project.project_id` proje UUID'si, o projenin `trusted_peers[].device_id` alanı gönderenin cihaz UUID'sidir. Birden fazla proje/peer varsa adları/UUID'leri kullanıcıyla doğrula; ilk kaydı rastgele seçme. Uygun kayıtlardan `$trialProjectId` ve `$trialSenderId` değişkenlerini doldur. Davet dosyasından bu alanları okumak gerekmez.

### D. Eşleştirme isteğini otomatik gönder

Gönderen kendi Desktop'ında daveti export etmiş (pencereyi açar) veya **Pair collaborator → Open pairing window** seçmiş olsun. Uygulaması açık kalsın. Her komutun sonucunu kontrol et:

```powershell
& $trialAgent --data-dir $trialData request-pairing $trialProjectId --device-id $trialSenderId --connect-timeout 30
```

Native CLI imzalı kartı ve tek kullanımlık davet sahipliği kanıtını doğrudan gönderir; anahtar/proof çıktısını agent okumaz. Gönderen gelen Device/Identity değerlerini beklediği kişiyle doğrulayıp **Approve device** seçer. Alıcı bağlantı için 30 saniyeye, istek sonrası onay için 60 saniyeye kadar bekleyebilir. `pairing_state: approved` yetki verildiğini, `pending` yalnızca isteğin teslim edildiğini gösterir. Pending'i dosya teslimi veya onay olarak raporlama. Daha geç onayda aynı komutu tekrar çalıştır; kaynak snapshot'ını bekle. Offline/kapalı pencere/timeout'ta önce bu koşulları düzelt; profil veya projeyi sıfırlama. Eski executable komutu tanımıyorsa yeni paketi iste.

### E. Bağlantıyı dene, sonra Receive

```powershell
& $trialAgent --data-dir $trialData check $trialProjectId --device-id $trialSenderId --connect-timeout 30
```

30 saniyeye kadar bağlantı beklenebilir. Başarılı check'te `remote_snapshot` yoksa gönderenden önce snapshot iste. Divergence/unknown ancestry gibi engeller varsa bu ilk denemede ilerleme; yerel veriyi silerek aşmaya çalışma. Her başarılı check eşitlik anlamına gelmez; boş hedef henüz dosyaları almamıştır.

Kaynak hazır, hedef doğru ve DAW kapalıysa:

```powershell
& $trialAgent --data-dir $trialData receive $trialProjectId --device-id $trialSenderId --connect-timeout 30
& $trialAgent --data-dir $trialData check $trialProjectId --device-id $trialSenderId --connect-timeout 30
& $trialAgent --data-dir $trialData status
```

Receive bitmeden diğer komutu başlatma. Başarıyı son check'in `verified_equal` değeriyle değerlendir; false ise snapshot kimliklerini/bağlantı sonucunu bildir. Sonradan kaynak değişmiş olabilir. İlk Receive sırasında ağ kesilirse status'taki PAUSED session/snapshot ID'sini kullanarak aynı hedefi `receive ... --snapshot-id <UUID>` ile sürdür. Recovery gerekiyorsa DAW kapalıyken `recover <project-UUID>` kullan; backup/journal verisini koru.

## Bağlantı olmadıysa kısa kontrol sırası

1. Gönderen online mı, eşleştirme penceresini açtı mı ve cihaz isteğini onayladı mı, snapshot oluşturdu mu? Davet güncel public IP/dış port içeriyor mu? İlk uzak bağlantıyı alıcı başlatsın.
2. Gönderende Windows Firewall TCP ve UDP local portu (varsayılan 43101) aktif ağ profilinde izinli mi?
3. Gönderen modeminde TCP ve UDP dış port → doğru PC LAN IP/local port eşlemesi etkin mi? Dış port 45000 seçilmişse iç port 43101 kalabilir; davette 45000 olmalı.
4. Port-forward kapalıysa kuralı etkinleştirin. Menü kilitli/yetki yoksa ISP/ağ yöneticisinden izin isteyin. Port çakışıyorsa boş dış port seçip yeni davet çıkarın.
5. WAN IP 100.64.0.0–100.127.255.255 ise CGNAT olabilir. 10.x, 172.16–31.x, 192.168.x de public değildir; ikinci router/ISP NAT olabilir. Yerel modem kuralı upstream NAT'ı açmaz. Erişilebilir public IPv4 veya önceden hazırlanmış özel relay gerekir. [Üretici sorun giderme rehberi](https://www.tp-link.com/uk/support/faq/785/).
6. Her değişiklikten sonra bir check yapın; aynı timeout'u sürekli tekrarlamayın. Mailbox proje dosyası taşımaz ve kapalı kaynağın yerine geçmez. Relay otomatik kurulmaz.

İlk yönde alıcı CLI geçici bir listen portu seçer; alıcının modeminde port-forward açılması bu akışın önkoşulu değildir. Gönderen erişilebilir olmalı ve alıcının dışarı bağlantısı engellenmemelidir. Gerçek iki-PC internet bağlantısı bu paketle henüz doğrulanmış sayılmaz.

## Desktop'ı daha sonra açmak

CLI işi tamamlandıktan sonra aynı deneme profilini göstermek için:

```powershell
Start-Process -FilePath '.\trackmesh-desktop.exe' -ArgumentList @('--data-dir', ('"' + $trialData + '"')) -WindowStyle Hidden
```

Kullanıcı açıkça Desktop penceresi görmek istiyorsa normal çift tıklama yerine bu profil seçimiyle aç; aksi halde çift tıklama varsayılan `%LOCALAPPDATA%\TrackMesh` profilini gösterir ve deneme projesi görünmeyebilir. CLI alma akışı için Desktop açmak şart değildir. GUI kontrol araçların yoksa butonlara bastığını iddia etme.

`\\?\D:\...` görünen yollar Windows'un canonical/uzun yol biçimidir; aynı `D:\...` klasörünü ifade eder. Veri/proje yolunu düzeltmek için veritabanını elle değiştirme.
