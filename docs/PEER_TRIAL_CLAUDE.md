# Peer terminal yönergesi

Dağıtım paketinin agent yönergesi [setup/peer/CLAUDE.md](setup/peer/CLAUDE.md) dosyasına taşındı. Pakette kök CLAUDE.md olarak kullanılır. Eski bağlantıları korumak için bu sayfa yönlendirme olarak tutulur.

Güncel template native `join`, güvenli `status`, `request-pairing` ve gönderen onayından sonra check/receive/check kullanır. Davet içeriğini agente okutmaz, normal akışta kart dosyası geri göndermez. Template değiştiğinde yeni yönergeli paket/checksum hazırlanır; eski ZIP değiştirilmez.

- [Ortak sıra ve sorun giderme](setup/README.md)
- [Ana bilgisayar / gönderen](setup/host/README.md)
- [Peer / alıcı](setup/peer/README.md)
- [Peer terminal agent yönergesi](setup/peer/CLAUDE.md)

Gerçek doğrulama sonuçları [IMPLEMENTATION.md](IMPLEMENTATION.md) içinde tutulur.
