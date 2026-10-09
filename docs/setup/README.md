# TrackMesh: iki bilgisayarda kurulum ve ilk deneme

Henüz setup/MSI yok; Windows executable ile elle deneme yapıyoruz. Küçük bir test projesi kullanın. İlk aktarım için VST3 kurulumu gerekmez. Gerçek doğrulama sonuçları yalnızca [IMPLEMENTATION.md](../IMPLEMENTATION.md) içinde tutulur.

## Önce kendi rolünü seç

| Rol | Okunacak belge | Yapılacak işler |
| --- | --- | --- |
| **Ana bilgisayar / A / gönderen** | [host/README.md](host/README.md) | Projeyi link et, adresleri belirle, modem/firewall hazırla, daveti gönder, gelen cihaz isteğini doğrulayıp onayla, snapshot oluştur |
| **Peer / B / alıcı** | [peer/README.md](peer/README.md) | Paketi aç, daveti import et, otomatik eşleştirme isteğini gönder, Check peer → Receive → son check yap |
| **Peer tarafında terminal agentı** | [peer/CLAUDE.md](peer/CLAUDE.md) | Davet içeriğini okumadan native CLI ile join/request-pairing/check/receive |

“Ana bilgisayar” ilk aktarımın kaynağıdır; merkezi sunucu veya kalıcı master rolü değildir. İlk bağlantıyı B, A'ya başlatır. B'nin normal dışarı bağlantısı varsa bu ilk Receive için kendi modeminde port açması gerekmez. Kurulmuş bağlantı iki yönlü kullanılabilir; A'nın sonradan B'ye bağımsız yeni bağlantı başlatması için B'nin erişilebilirliği ayrıca hazırlanır.

**Karşı tarafı beklerken:** A uygulamayı açık tutsun ve doğru davetin B'ye özel kanaldan ulaştığını kontrol etsin. B daveti kabul edince cihaz isteği otomatik gelir. A kimliği kontrol edip **Approve device** ve Send Snapshot yapar; B check/receive/check ile devam eder. Aynı klasörü yeniden link etmek veya yeni UUID oluşturmak gerekmez.

## Peer benim bilgisayarımı ve projemi nasıl görecek?

1. **A güncel Desktop'ı açar ve projeyi link eder.** Uygulamayı açmak B'ye otomatik bildirim göndermez; kullanıcıların listelendiği merkezi bir rehber yoktur.
2. **A güncel adreslerle davet export eder ve dosyayı özel gönderir.** Davet proje UUID/adı, A'nın imzalı cihaz kimliği/PeerId'si ve bağlantı adreslerini içerir. Native uygulamanın okuyacağı özel proje capability'sini de taşır; B veya agent dosya içeriğini chat'e yapıştırmaz.
3. **B daveti boş yerel hedefe import eder.** Kendi proje kartında aynı proje adını/UUID'sini ve `Collaborator` listesinde A'nın cihazını görür. Bu yerel kayıt, A'nın o anda online olduğu veya dosyaların indirildiği anlamına gelmez.
4. **B'nin uygulaması davetteki adreslerden A'ya bağlanır.** Ayrı internetlerde yol public IP/dış port → modem → A'nın LAN IP/local portudur. A online ve eşleştirme penceresi açık olmalı. Bağlantı için 30 saniyeye kadar beklenebilir.
5. **A gelen cihaz isteğini doğrular ve onaylar.** B kart dosyası geri göndermez. B'nin onay bekleme süresi 60 saniyeye kadardır; geç onayda Retry automatic pairing kullanılabilir.
6. **A snapshot oluşturur; B Check peer → Receive → son check yapar.** Onay, bağlantı, snapshot hazır olması ve dosya teslimi ayrı durumlardır. Receive tamamlanmadan B'de proje dosyaları hazır sayılmaz.

İlk davet import edildikten sonra aynı veri profili ve güncel bağlantı adresleri korunuyorsa tekrar New UUID/davet import gerekmez. İkiniz de online olduğunuzda B, bilinen A adreslerine yeniden bağlanmayı dener. Yeni eşleştirme penceresi yalnızca onaysız bir cihazı kaydetmek içindir; onaylanmış cihaz her açılışta tekrar onay istemez. B CLI kullanıyorsa `request-pairing`/`check`/`receive` yalnızca kendi komutu boyunca network session tutar; komut bitince kalıcı online kalmaz. Sürekli açık deneme için aynı profille Desktop veya açık `serve` oturumu kullanılır; aynı anda iki owner çalıştırılmaz.

## Panelde hangi cihaz ve durum görünür?

| Görünen bilgi | Anlamı |
| --- | --- |
| Pair collaborator altında isim, Device ve Identity ile istek | Davet sahipliği kanıtı doğrulanmış, onay bekleyen aday; dosya yetkisi yoktur. İsim yerine public kimliği arkadaşınla karşılaştır. |
| Collaborator listesindeki cihaz | Yerelde kayıtlı/eşleştirilmiş cihaz. B import sonrası A'yı görür; A onay sonrası B'yi görür. |
| Seçili cihazda `DIRECT` / `RELAYED` ve transport | Şu an kurulu peer bağlantısı. Dosya teslimi veya fresh eşitlik kanıtı değildir. |
| `OFFLINE` / last-known | Şu an bağlantı yok; varsa geçmiş metadata gösterilir. Check peer en fazla 30 saniyelik bağlantı denemesi yapar. |
| Fresh `UP TO DATE` / CLI `verified_equal: true` | Kontrol sırasında karşılaştırılan snapshot'ların güncel bağlantı üzerinden eşitliği doğrulanmıştır. |

Onay bekleyen aday, bağlantısı kesilse de pencere kapanana kadar listede kalabilir; aday satırında canlı online göstergesi yoktur. Henüz daveti açmayan kişiyi, başarısız/yetkisiz bütün giriş denemelerini veya kalıcı bağlantı geçmişini gösteren bir panel yoktur. Eski native pencere açıksa yeni EXE'yi başlatmak eski owner'ı öne getirebilir; iki tarafta güncel build'in gerçekten çalıştığını kontrol edin.

## Açık bağlantıda iki yöne dosya göndermek

B → A bağlantısı kurulmuş ve iki cihaz aynı proje için yetkiliyse, aynı bağlantı üzerinden A → B ve B → A peer istekleri taşınabilir. B'nin mevcut bağlantıda kaynak olması için ayrıca modem portu açması önkoşul değildir. B değişikliğini kaydedip Send Snapshot yapar; A doğru B cihazını seçip DAW kapalıyken Check peer → Receive → son check yapar. Yerel değişiklik, ancestry/divergence ve recovery kontrolleri iki yönde de geçerlidir; aynı anda iki tarafın bağımsız değişikliklerini otomatik birleştirmeyin.

Bağlantı kapanmışsa B, erişilebilir A'ya yeniden bağlanabilir. A'nın B'ye tek başına yeni inbound yol açması için B'nin public port yönlendirmesi veya hazırlanmış relay yolu gerekir. B'nin CLI komutu bittiyse o cihazın açık Desktop/serve session'ı olmadan kaynak online sayılmaz.

## IP veya internet bağlantısı değişince

- **Yalnızca A'nın LAN IP'si değişti:** modem hedefini yeni PC IP'sine yönelt; local port/firewall aynı kalsın. Public IP/dış port da değişmediyse bunlar için yeni davet gerekmez. LAN'dan bağlanan B'nin eski yerel adresi güncel değilse güncel adresli davet çıkar.
- **A'nın public IP'si veya dış portu değişti:** erişilebilirliği yeniden hazırla, yeni adreslerle davet export et; B aynı profil/proje/hedef klasörüne native import ile adresleri güncellesin. Proje UUID'sini veya kimlik profilini sıfırlama. Yalnızca pairing timeout nedeniyle tekrar import yapılmaz; bu adım değişen adres içindir.
- **A telefon hotspot'una veya başka router'a geçti:** eski modemdeki port yönlendirmesi yeni ağa uygulanmaz. CGNAT/private WAN nedeniyle yeni public IP bilinmesi de yeterli olmayabilir; erişilebilir ağ veya hazırlanmış özel relay gerekir.
- **B alıcı olarak ağ değiştirdi:** A'nın güncel adreslerine yeni dışarı bağlantı deneyebilir. B'ye bağımsız gelen bağlantı gerektiğinde B'nin yeni ağ erişimi ayrıca değerlendirilir.

## Browser deployment ve database sınırı

TrackMesh Windows Desktop olarak çalışır; bu deneme için UI'ı tarayıcıya deploy etmek gerekmez. Yerel SQLite zaten proje, trust, snapshot ve transfer kayıtlarını tutar. İki bilgisayarın açık olması tek başına erişimi kanıtlamaz; güncel adres, doğru yönlendirme/firewall, onay ve hazır snapshot da gerekir.

| Parça | Şu anki rol / sınır |
| --- | --- |
| Davet + direct bağlantı | Merkezi database olmadan ilk adresi/kimliği taşır; varsayılan denemede değişen public adres yeni davetle güncellenir. |
| Private rendezvous | Yapılandırılmış, erişilebilir ve yetkilendirilmiş servis üzerinden trusted cihazlar adres keşfi yapabilir. Repo desteği vardır; public deployment ve ağ değişimi entegrasyonu doğrulanmamıştır. |
| Private relay | Direct yol kurulamadığında şifreli trafiğe geçiş noktası sağlar; iki uç yine online olmalı. Database kaydı NAT engelini tek başına aşmaz. |
| Mailbox + servis SQLite | Şifreli snapshot duyurularını tutar; proje dosyaları ve offline eşleştirme istekleri için storage değildir. |
| Supabase / başka merkezi database | Şu an entegre değildir. Güvenli cihaz yetkilendirmesi ve kayıt güncelleme akışı ayrıca tasarlanır; DB seçmek kendiliğinden keşif veya relay kurmaz. |

Altyapı işletimi [PHASE3.md](../PHASE3.md) içinde; varsayılan trial'da hazır servis olmadan relay/mailbox alanını doldurmayın. Yeni peer'ın altyapıya ilk erişim yetkisi de operatör tarafından hazırlanmalıdır; uygulamadaki onay servis ACL'lerini otomatik güncellemez.

## Hızlı deneme: sırayla yapacaklarımız

İki farklı internet bağlantısı için bağlantı yolu: **B → A'nın public IP'si ve dış portu → A'nın modemi → A'nın yerel PC IP'si ve local portu → TrackMesh**. Windows Firewall da bu PC'ye gelen bağlantıya izin vermeli. Bir noktadaki engel tüm yolu kesebilir; Check peer timeout tek başına hangi noktanın hatalı olduğunu söylemez.

1. **İkiniz de uygulamayı açın.** Aynı sürümü kullanın. A küçük bir deneme klasörünü **Link folder** ile bağlasın; B almak için boş bir klasör hazırlasın. İlk denemede mailbox/relay boş kalabilir.
2. **A yerel IP/portu not etsin.** **Collaboration & connection details → Connection setup** içinde örneğin `192.168.1.50:43101` görürsünüz. Bu IP modemde hedef olacak. Kendi IP'nizi kullanın.
3. **A firewall iznini versin.** Gelen TCP `43101` ve UDP `43101` için izin kuralı oluşturun; kurallar aktif Windows ağ profilinde geçerli olsun. Uygulama bunu otomatik yapmaz.
4. **A modemden port yönlendirsin.** TCP `43101 → PC_IP:43101` ve UDP `43101 → PC_IP:43101` kurallarını etkinleştirip kaydedin. Modemin WAN adresinin public IPv4 olup olmadığını kontrol edin. Aynı LAN'daysanız bu adımı atlayın.
5. **Daveti gönderin ve eşleştirin.** Ayrı internetlerde A, public IP + dış portu girip **Export invitation with public addresses** kullansın. Aynı LAN'da **Export invitation with current addresses** yeterli. Export, A'da 10 dakikalık eşleştirme penceresini açar. B **Accept invitation & connect…** ile daveti kabul edince imzalı cihaz kimliği ve davet sahipliği kanıtı otomatik gönderilir. A **Pair collaborator → Approve device** ile beklenen kimliği doğrulayıp onaylasın. B kart dosyası göndermez; yalnızca ilk daveti özel paylaşın.
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

Modem arızalarını ayırmak için [üreticinin port-forward sorun giderme rehberi](https://www.tp-link.com/uk/support/faq/785/) de WAN IP, upstream NAT ve firewall kontrollerini anlatır; menü adları sizin modeminizde farklı olabilir. Ayrıntılar rolünüze ait rehberdedir. Henüz hazır relay'iniz yoksa bunu anında kullanılabilecek bir fallback gibi düşünmeyin; önce operatör kurulumu ve erişim izinleri gerekir.

## Tek eşleştirme akışı

Yeni Desktop yanıtı şifreli peer bağlantısından otomatik gönderir; A'nın son onayı korunur. Alıcı, istek gönderildikten sonra onayı 60 saniyeye kadar sorgular. A offline/pencere kapalıysa import korunur; A **Open pairing window**, B **Retry automatic pairing** ile devam eder. Tek projede 10 dakikalık pencere vardır; süre bitince, kapanınca veya restart olunca onaysız istekler temizlenir. Bu süre davet/proje anahtarını iptal etmez. Eski invitation/card formatları ve transfer protokolü korunur; ayrı `/trackmesh/pairing/1` kanalı eklenir. İki tarafta da otomatik eşleştirme build'i gerekir: `20261007` ZIP'i bunu içermez, `20261008` AutoPairing paketi içerir, `20261009` paketi aynı EXE'lerle güncel yönergeyi taşır. Eski cihazlarla manuel kart yolu **Legacy file pairing** altında bulunur; `join`/`card` komutları değişmez, CLI'ye `request-pairing` eklenir. [Güvenlik sınırları](PAIRING_SECURITY.md).

## Ortak çalışma kuralları

- İki taraf aynı uygulama sürümünü kullansın. Windows 10/11 x64 ve Desktop için WebView2 gerekir; CLI WebView2 kullanmaz. Installer veya otomatik firewall/router ayarı yoktur.
- Desktop varsayılan profili `%LOCALAPPDATA%\TrackMesh`, peer CLI deneme profili `%LOCALAPPDATA%\TrackMeshPeerTrial` olur. Aynı profilde Desktop/CLI serve/check/receive/plugin owner aynı anda network runtime açmasın. DAW proje klasörü, veri profili ve executable paketi ayrı olsun.
- Davet private capability/proje anahtarı içerir. Yalnızca özel kanaldan dosya olarak paylaşın; payload'ı clipboard, chat, agent context, log veya repoya koymayın. Native dialog veya CLI `join --invite-file` kullanın. Profil, veritabanı ve secrets paylaşılmaz.
- Dosya eklemek otomatik sync değildir; A **Send Snapshot**, B **Receive** yapar. `2 publications pending` iki authored snapshot'ın metadata duyurusunun ACK beklemesidir; dosya sayısı veya teslim sonucu değildir.
- Mailbox yalnızca encrypted metadata/snapshot announcement tutar. `PUBLISHED` dosyaların B'ye ulaştığını göstermez. Mailbox yokken yerel snapshot hazırsa direct Receive denenebilir.
- Özel relay direct bağlantı başarısız olursa peer bağlantısına geçiş noktası olabilir. Hazır servis, pinned PeerId ve izinler gerekir; bir alan doldurmak relay kurmaz. Kaynak yine online olmalıdır. Operatör rehberi: [PHASE3.md](../PHASE3.md).

## Sorun olursa ve denemeyi bitirirken

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

## Belge ve paket bakımı

Rol rehberleri bu klasörde tutulur; geliştirme/build ayrıntıları [DEVELOPMENT.md](../DEVELOPMENT.md), mimari sınırlar [ARCHITECTURE.md](../ARCHITECTURE.md) içindedir. Eski MANUAL_TRIAL, TWO_PC_CHECKLIST ve PEER_TRIAL_CLAUDE adresleri yönlendirme sayfalarıdır.

Peer paketine `peer/CLAUDE.md` dosyasını **paket kökünde `CLAUDE.md`** olarak kopyalayın. Repo kökündeki `CLAUDE.md` geliştirme girişidir; pakete onu koymayın. Paket yalnızca aynı build'in EXE/DLL'leri, yönerge, public build metadata ve checksum dosyalarını içersin; davet ayrı gönderilir. Dağıtım yönergesi değiştiğinde yeni paket adı/metadata ve checksum üretin; daha önce gönderilmiş ZIP'i sessizce değiştirmeyin.
