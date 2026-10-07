# TrackMesh: iki Windows bilgisayarda ilk deneme

Bu belge uygulamayı ilk kez deneyecek proje sahibi ve collaborator (peer) içindir. Henüz bir setup/MSI, otomatik firewall/modem ayarı veya doğrulanmış internet dağıtımı yok. Elinizdeki Windows executable ile elle deneme yapıyoruz. Güncel doğrulama durumu [IMPLEMENTATION.md](IMPLEMENTATION.md) içindedir.

İlk denemede **A** daveti ve snapshot'ı gönderen bilgisayar, **B** daveti kabul edip dosyaları alan bilgisayar olsun. Küçük, gözden çıkarılabilir bir proje kopyası kullanın. Gerçek DAW projesiyle başlamayın. İlk aktarım için VST3 kurmanız gerekmez.

## Hızlı deneme: sırayla yapacaklarımız

İki farklı internet bağlantısı için bağlantı yolu: **B → A'nın public IP'si ve dış portu → A'nın modemi → A'nın yerel PC IP'si ve local portu → TrackMesh**. Windows Firewall da bu PC'ye gelen bağlantıya izin vermeli. Bir noktadaki engel tüm yolu kesebilir; Check peer timeout tek başına hangi noktanın hatalı olduğunu söylemez.

1. **İkiniz de uygulamayı açın.** Aynı sürümü kullanın. A küçük bir deneme klasörünü **Link folder** ile bağlasın; B almak için boş bir klasör hazırlasın. İlk denemede mailbox/relay boş kalabilir.
2. **A yerel IP/portu not etsin.** **Collaboration & connection details → Connection setup** içinde örneğin `192.168.1.50:43101` görürsünüz. Bu IP modemde hedef olacak. Kendi IP'nizi kullanın.
3. **A firewall iznini versin.** Gelen TCP `43101` ve UDP `43101` için izin kuralı oluşturun; kurallar aktif Windows ağ profilinde geçerli olsun. Uygulama bunu otomatik yapmaz.
4. **A modemden port yönlendirsin.** TCP `43101 → PC_IP:43101` ve UDP `43101 → PC_IP:43101` kurallarını etkinleştirip kaydedin. Modemin WAN adresinin public IPv4 olup olmadığını kontrol edin. Aynı LAN'daysanız bu adımı atlayın.
5. **Daveti ve karşı kartı değişin.** Ayrı internetlerde A, public IP + dış portu girip **Export invitation with public addresses** kullansın. Aynı LAN'da **Export invitation with current addresses** yeterli. B daveti **Import invitation** ile açsın, kendi kartını **Export my device card** ile A'ya göndersin. A da **Import collaborator card** yapsın. Dosyaları özel kanaldan paylaşın.
6. **A → Send Snapshot.** Yerel snapshot oluşmasını bekleyin. Mailbox yoksa `PENDING` dosya aktarımının kesin başarısız olduğu anlamına gelmez.
7. **B → Check peer → Receive.** Check peer için 30 saniyeye kadar bekleyin. Erişim sağlandıysa B DAW'ı kapatıp onay kutusunu işaretlesin ve Receive yapsın. İkiniz de uygulamayı açık tutun. Bitince dosyaları ve yeniden Check peer sonucunu kontrol edin.

Local portunuz farklıysa `43101` yerine paneldeki portu kullanın. Her değişiklikten sonra B'den Check peer'ı yeniden deneyin; aynı anda birden çok ayarı değiştirmeyin.

### Takılırsak: belirti → sonraki adım

Tabloyu sırayla inceleyin; size uymayan satırı geçin. İnternete geçmeden aynı LAN'da deneme yapabiliyorsanız, LAN'da çalışan aktarım uygulama/pairing tarafını ayırmaya yardımcı olur.

| Takıldığımız yer | Şimdi ne yapacağız? |
| --- | --- |
| **Firewall izni hiç verilmedi / izin penceresi gelmedi** | A'da Windows Güvenliği → Firewall → Gelişmiş ayarlar → Gelen Kuralları üzerinden TCP ve UDP local port izinlerini elle ekleyin. Popup gelmesini beklemeyin; firewall'ı tamamen kapatmayın. |
| **Firewall kuralını ekleyemiyoruz veya hâlâ engelleniyor** | Windows yönetici yetkisini, kuralın etkinliğini ve aktif Private/Public ağ profilini kontrol edin. Kurumsal politika/başka güvenlik yazılımı yönetiyorsa yetkili kişiden izin isteyin. |
| **Modemde port-forward kapalı / menü yok / erişim reddedildi** | Kuralı ve varsa port-forward özelliğini etkinleştirin. Ayar ISP/yönetici tarafından kilitliyse onlardan açılmasını isteyin. Yetki alınamıyorsa erişilebilir başka ağ veya hazır özel relay gerekir; uygulamadan bu kilidi açamayız. |
| **Modem “port kullanımda” diyor** | Çakışan kuralı kontrol edin. Kullanılmayan bir dış port seçebilirsiniz: örneğin TCP ve UDP `45000 → PC_IP:43101`. UI'da External router port `45000` olur; firewall local portu `43101` kalır. Yeni public-address davetini B'ye gönderin. |
| **Kural kaydoldu ama Check peer timeout oluyor** | A açık mı ve pairing iki yönde tamam mı? Sonra hedef LAN IP, iç/dış port, TCP ve UDP kurallarının etkinliği ve firewall profilini kontrol edin. B gerçekten ayrı internetten denesin. Kuralın kaydolması dışarıdan erişildiğini kanıtlamaz. |
| **WAN IP 100.64–100.127 ile başlıyor; ISP CGNAT olduğunu doğruluyor** | Kendi modeminizde port açmak ISP'nin NAT katmanını açmaz. ISP'den erişilebilir public IPv4 isteyin; alamıyorsanız hazır özel relay veya uygun başka ağ kullanın. Aynı port kuralını tekrar tekrar değiştirmek çözmez. |
| **WAN IP 10.x / 172.16–31.x / 192.168.x** | Önde başka router mı var, yoksa ISP özel IP mi veriyor, belirleyin. İki yerel router varsa iki katmanda yönlendirme gerekir; ISP tarafındaysa public IPv4/relay yoluna geçin. |
| **Daha önce çalıştı, bugün çalışmıyor** | A'nın LAN IP'si veya public IP'si değişmiş olabilir. LAN IP için modem hedefini/adres rezervasyonunu düzeltin; public IP/dış port değiştiyse yeni daveti çıkarıp B'de aynı proje klasörüne import edin. |
| **Check peer çalışıyor ama Receive pasif/reddediliyor** | Modem ayarını değiştirmeden A'da snapshot olduğunu, B'de doğru peer ve DAW-closed onayını, yerel değişiklik/divergence/recovery mesajlarını kontrol edin. |

Modem arızalarını ayırmak için [üreticinin port-forward sorun giderme rehberi](https://www.tp-link.com/uk/support/faq/785/) de WAN IP, upstream NAT ve firewall kontrollerini anlatır; menü adları sizin modeminizde farklı olabilir. Aşağıdaki bölümler bu kısa sıranın ayrıntılarıdır. Henüz hazır relay'iniz yoksa bunu anında kullanılabilecek bir fallback gibi düşünmeyin; önce operatör kurulumu ve erişim izinleri gerekir.

## 1. Uygulamayı edinme ve açma — iki kişi de

Proje sahibinden aynı sürüme ait `trackmesh-desktop.exe` ve varsa gerekli native bağımlılıkları alın. Windows 10/11 x64 ve WebView2 gerekir. Bu dosya bir installer değildir; çift tıklayarak çalıştırılır. Dağıtım paketinin başka bir Windows PC'deki çalışma durumu ayrıca doğrulanmalıdır. Eksik DLL veya WebView2 hatası alırsanız hata metnini proje sahibine iletin.

Kaynak koddan hazırlayacak kişi için Rust, Visual Studio C++ Build Tools/Windows SDK ve desteklenen Node/npm gerekir. Bilinen çalışan sürümler Rust 1.98.1 ve Node 24.19'dur. x64 Visual Studio developer shell içinde:

```powershell
Set-Location D:\CodeSpace\track-mesh\apps\desktop
npm ci
npm run typecheck
npm run build
Set-Location D:\CodeSpace\track-mesh
cargo build -p trackmesh-desktop --locked
target\debug\trackmesh-desktop.exe
```

Bu komutlar installer üretmez; frontend'i executable içine dahil eder. İlk dependency indirmesinde internet gerekir. Build ayrıntıları ve statik kontroller [DEVELOPMENT.md](DEVELOPMENT.md), native çalışma bilgileri [PHASE4.md](PHASE4.md) içindedir.

Her bilgisayar kendi `%LOCALAPPDATA%\TrackMesh` profilini oluşturur. Bu klasörü, veritabanını veya `secrets` içeriğini diğer kişiye göndermeyin; Windows kullanıcısına bağlı kimlik/anahtarlar içerir. DAW proje klasörü bu profilden ayrı olmalıdır. Aynı profil üzerinde ayrıca CLI peer/serve başlatmayın.

## 2. İlk projeyi bağlama — A

1. **+ Link folder** seçin; proje adı girin.
2. **New UUID** ile proje kimliğini oluşturun. B aynı projeye davet üzerinden katılacak; kendi başına yeni UUID oluşturmayacak.
3. **Browse…** ile küçük deneme proje klasörünü seçin.
4. Hazır, erişim izni verilmiş altyapınız yoksa HTTPS mailbox ve pinned relay alanlarını boş bırakın. Bu ilk direct-transfer denemesi için servis kurulumu gerekmiyor.
5. **Link project** seçin. Proje kartındaki **Collaboration & connection details → Connection setup** bölümünü açın.

## 3. Yerel IP ve portu bulma — A

Panelde **Local IPv4 / destination port** ve **Local listen port** görünür. Örneğin `192.168.1.50:43101`: IP bu bilgisayarın LAN adresi, `43101` uygulamanın dinlediği porttur. Normal akışta `ipconfig` yazmanız gerekmez.

Wi-Fi, Ethernet, VPN veya sanal adaptörler nedeniyle birden fazla IP görünebilir. Modeminize bağlı adaptörün IP'sini seçin. `0.0.0.0` hedef PC adresi değildir. Panel henüz adres göstermiyorsa uygulamanın açılmasını bekleyin ve Windows ağ bağlantısını kontrol edin. Modemde DHCP reservation/adres rezervasyonu yaparak seçtiğiniz PC adresini sabitleyin.

**Aynı LAN üzerinde deneme:** Public IP ve modem port-forward adımını atlayabilirsiniz. Firewall izninden sonra **Export invitation with current addresses** kullanın. İki cihaz birbirine erişebilen aynı ağda olmalı; guest Wi-Fi/client isolation cihazlar arası erişimi engelleyebilir.

**Farklı internet bağlantıları üzerinde deneme:** Sonraki modem/public IP adımlarını A üzerinde uygulayın. İlk bağlantıyı B, A'ya doğru başlatsın. B'nin dışarı bağlantı kurabilmesi gerekir; bu ilk yön için B modeminde de port-forward zorunlu kabul edilmez. Ters yönde yeni bağlantı başlatmak ayrıca erişilebilir adres/yapılandırma gerektirebilir.

## 4. Windows Firewall izni — A

Windows Güvenliği → Güvenlik duvarı ve ağ koruması → Gelişmiş ayarlar bölümünü açın. **Gelen Kuralları / Inbound Rules → Yeni Kural / New Rule → Bağlantı Noktası / Port** yoluyla iki izin kuralı oluşturun:

| Kural | Protokol | Yerel port | Eylem |
| --- | --- | --- | --- |
| TrackMesh TCP | TCP | 43101 | Bağlantıya izin ver |
| TrackMesh UDP | UDP | 43101 | Bağlantıya izin ver |

Uygulamanın gösterdiği local port farklıysa tabloda o portu kullanın. Kuralları bağlantınızın aktif Windows ağ profiline uygulayın; izinleri TrackMesh denemesi için gereken kapsamda tutun. Bu işlem Windows yönetici yetkisi isteyebilir. Firewall'ı bütünüyle kapatmanız gerekmez. Uygulama bu kuralları otomatik eklemez. [Microsoft'un firewall kuralı rehberi](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/configure) bu adımları açıklar.

## 5. Modem port yönlendirme ve public IP — A

Modem/router yönetim ekranında **Port Forwarding**, **NAT**, **Virtual Server** veya benzer isimli bölümü bulun. Menü/model farklılıkları nedeniyle uygulama bunu sizin yerinize yapmaz. Örneğimizde iki etkin kural gerekir:

| Protokol | Dış/WAN portu | Hedef yerel PC | İç/LAN portu |
| --- | --- | --- | --- |
| TCP | 43101 | 192.168.1.50 | 43101 |
| UDP | 43101 | 192.168.1.50 | 43101 |

Buradaki IP örnektir; kendi **Connection setup** panelinizdeki IP'yi kullanın. Router TCP/UDP birlikte seçmeye izin veriyorsa aynı eşlemeyi ikisine de uygulayabilirsiniz. Kuralı etkinleştirip kaydedin.

Dış port farklı seçilebilir: örneğin TCP/UDP `45000 → 192.168.1.50:43101`. Bu durumda uygulamadaki **External router port** alanı `45000`, Windows Firewall'ın local portu `43101` olur. Bu alanı değiştirmek uygulamanın dinleme portunu değiştirmez.

Modemin internet/WAN durum ekranından public IPv4'ü bulun. Şunlar public IP değildir:

| Aralık | Anlamı |
| --- | --- |
| 100.64.0.0–100.127.255.255 (`100.64.0.0/10`) | ISP paylaşımlı adres alanı; CGNAT göstergesi olabilir |
| 10.x.x.x (`10/8`) | Özel ağ |
| 172.16.x.x–172.31.x.x (`172.16/12`) | Özel ağ |
| 192.168.x.x (`192.168/16`) | Özel ağ |

CGNAT paylaşımlı adres alanı [RFC 6598](https://www.rfc-editor.org/rfc/rfc6598) ile tanımlıdır. Router WAN'ı özel/paylaşımlıysa yalnızca kendi modeminizde port açmak yeterli olmayabilir. WAN'daki özel IP ikinci bir yerel router'a da işaret edebilir; her özel WAN adresini kesin CGNAT teşhisi saymayın.

| Engel | İzlenecek yol |
| --- | --- |
| Port-forward özelliği/oluşturulan kural kapalı | Modemin desteklediği port-forward ayarını ve kuralı etkinleştirin; doğru hedef IP/portu kaydedin. |
| Menü yok, ayar ISP tarafından kilitli veya modem yetkiniz yok | Ağ yöneticisi/ISP'den bu özelliği isteyin ya da başka erişilebilir ağ kullanın. UI tek başına bu kısıtlamayı aşamaz. |
| Evde arka arkaya iki router var | İki NAT katmanındaki yönlendirmeyi ağ yöneticisiyle düzenleyin; yalnızca iç router'daki kural yeterli olmayabilir. |
| ISP CGNAT kullanıyor | Erişilebilir public IPv4 talep edin veya operatörü tarafından hazırlanmış özel relay kullanın. Statik IP şart değildir; erişilebilir olması gerekir. |
| Public IP var ama dışarıdan erişim engelleniyor | Firewall profilini, kuralları ve ISP'nin inbound kısıtlamalarını kontrol edin. IP girmek erişim doğrulaması değildir. |

Relay adres alanını doldurmak relay servisi kurmaz. Hazır servis, doğru pinned PeerId ve iki cihaza verilmiş erişim izni gerekir; işletim adımları [PHASE3.md](PHASE3.md) içindedir.

## 6. Public adresli daveti çıkarma — A

1. **Connection setup → Public IPv4 (router WAN / ISP)** alanına A'nın erişilebilir public IPv4'ünü girin.
2. **External router port** alanını modeminizde seçtiğiniz dış portla eşleştirin.
3. Panelin ürettiği iki adresi kontrol edin:

   ```text
   /ip4/PUBLIC_IP/tcp/PORT
   /ip4/PUBLIC_IP/udp/PORT/quic-v1
   ```

4. A'nın uygulamasını açık tutun. **Export invitation with public addresses** seçin ve native dosya penceresinde yeni bir `.json` dosya adıyla kaydedin.
5. Davet dosyasını yalnızca B'ye özel bir kanaldan gönderin. İçeriğini GitHub'a, loga, issue'ya veya chat'e yapıştırmayın; proje anahtarı içeren özel erişim belgesidir.

Public IP/dış port alanları yalnızca bu export için kullanılır; kalıcı ayar olarak kaydedilmez ve çalışan network actor'ün ilanlarını değiştirmez. **Export invitation with current addresses** aynı manuel alanları kullanmaz. Public IP/dış port değişirse yeni public-address daveti çıkarın; B aynı yerel proje klasörünü seçerek yeni daveti içe aktarsın.

## 7. Daveti kabul etme ve karşılıklı pairing — B, sonra A

1. **B:** **Import invitation** seçin, ayrı bir yerel deneme klasörü seçin ve **Choose invitation…** ile A'nın dosyasını açın. İlk Receive için boş klasör tercih edin; mevcut, kaydedilmemiş yerel değişiklikler Receive'i engelleyebilir.
2. **B:** Alttaki **Export my device card** ile yeni bir `.json` kart dosyası çıkarıp özel kanaldan A'ya gönderin. Bu kartı davet dosyasıyla karıştırmayın.
3. **A:** İlgili proje kartında **Collaboration & connection details → Import collaborator card** ile B'nin kartını içe aktarın.
4. İki taraf da doğru peer'ın göründüğünü kontrol etsin ve uygulamayı açık tutsun.

B daveti import edince A'yı tanır; A'nın da B'nin kartını import etmesi gerekir. Sadece davet göndermek karşılıklı yetkilendirmeyi tamamlamaz.

Mevcut UI'da public alanlar **Export my device card** düğmesine uygulanmaz. İlk internet denemesini bu nedenle B'nin A'ya bağlantıyı başlatmasıyla yürütün. B'nin de bağımsız olarak public adresle erişilebilir olmasını denemek için B aynı projenin **Connection setup** bölümünden kendi public adresli davetini çıkarabilir; A bunu kendi mevcut proje klasörüne import eder. Alternatif başlangıç `--external-address` seçenekleri [PHASE4.md](PHASE4.md) içindedir. Hiçbir yol gerçek NAT/relay başarısını önceden garanti etmez.

## 8. İlk snapshot ve Receive — A, sonra B

1. **A:** Deneme projesini kaydedin; ilk denemede DAW'ı kapatın. **Send Snapshot** seçin ve yerel snapshot'ın oluşmasını bekleyin. Bu işlem B'nin klasörünü otomatik değiştirmez.
2. Mailbox yapılandırılmadıysa metadata publication `PENDING`/hata gösterebilir. Yerel snapshot oluştuysa direct transfer yine denenebilir. `PUBLISHED`, dosyaların B'ye ulaştığı anlamına gelmez.
3. **B:** A'yı seçip **Check peer** yapın. Bağlantı denemesi 30 saniyeye kadar sürebilir; ekrandaki geri sayımı bekleyin. Offline/last-known bilgi dosyaların şu anda indirilebilir olduğu anlamına gelmez.
4. **B:** DAW projesini kapatın ve **I closed this project in the DAW before Receive or Recovery** kutusunu işaretleyin. **Receive** seçin; kurulum tamamlanana kadar uygulamayı ve A'yı açık tutun.
5. B'de tamamlanan transferin snapshot ID'sini ve alınan dosyaları kontrol edin. **Check peer** ile yeni karşılaştırma yapın; fresh doğrulanmış eşitlikte **UP TO DATE** beklenir. DAW açma/eksik sample kontrolü ayrı bir manuel kontroldür.

Mailbox yalnızca encrypted metadata/snapshot announcement tutar; proje dosyası taşımaz. Özel relay direct bağlantı başarısız olduğunda peer bağlantısı için geçiş noktası olabilir. Dosya aktarımı için kaynak peer yine online olmalıdır.

## 9. Sorun olursa ve denemeyi bitirirken

| Belirti | İlk kontrol |
| --- | --- |
| Check peer timeout / offline | Kaynak açık mı, pairing iki yönde tamam mı, davette güncel public IP var mı, modem ve firewall doğru PC/local portu gösteriyor mu? Ayrı ağdan B → A denemesini kullanın; aynı LAN'dan public-IP denemesi router'ın NAT loopback desteğine takılabilir. |
| Public export düğmesi pasif | Public IPv4 ve 1–65535 arası dış port girin. LAN/CGNAT adresi public alanına yazılmaz. |
| Export dosyası kaydedilemiyor | Yeni bir `.json` dosya adı seçin. Mevcut dosyalar üzerine yazılmaz. |
| Receive pasif veya reddediliyor | Doğru peer, online bağlantı, DAW-closed kutusu, yerel değişiklikler ve divergence/recovery mesajlarını kontrol edin. Divergence otomatik birleştirilmez. |
| Recovery gerekiyor | DAW'ı kapatıp **Recover locally** kullanın. Staging/backup klasörlerini veya veritabanını silmeyin. |
| Farklı local portla açmak istiyorsunuz | Runtime'da değişmez. Mevcut owner ve işlerinin güvenli şekilde bitmesini bekleyip `trackmesh-desktop.exe --port 43102` gibi bir başlangıçla yeniden açın; firewall ve iç port eşlemesini güncelleyin. İkinci launch mevcut owner'ın portunu değiştirmez. |

**Close safely** pencereyi gizler; aktif iş/plugin session/pending publication process'i açık tutabilir. İşler bittikten sonra 60 saniye idle grace vardır. Mailbox yoksa/erişilemiyorsa pending publication bu kapanışı geciktirebilir. Yeniden açarak durumu inceleyin; dosya kurulumu sürerken process'i zorla sonlandırmayın.

Proje sahibine executable sürümü/commit, Windows sürümü, aynı LAN mı ayrı internet mi, hangi adımda durduğu, hata metni, transfer/snapshot ID ve ekrandaki bağlantı durumunu bildirin. Davet içeriği, anahtarlar, `secrets` veya veritabanı göndermeyin; ekran görüntüsünde IP, kullanıcı adı ve özel dosya yollarını gerektiğinde gizleyin.

Denemenin başarı ölçütü: iki cihaz karşılıklı paired, B A'ya ulaşabiliyor, A'nın snapshot'ı B'ye tamamlanarak alınıyor ve fresh Check peer sonucu doğrulanıyor. Bu belge bir yönergedir; iki gerçek PC'de bu kontrollerin yapıldığına dair sonuç kaydı değildir.
