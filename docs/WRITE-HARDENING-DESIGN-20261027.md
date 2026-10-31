# Blok1 arka plan WAL kesiti —27 Ekim2026

Durum: prototip; kabul ölçümleri sürüyor. Yeni küçük WAL yolu ve reclaim
varsayılan kapalı kalır. C ABI/entegrasyon yok.

İlk ayrı diagnostic koşusunda 513. commit446,804ms: light indeks hazırlığı
268,484ms, source/index hazırlığı113,027ms, active/manifest2,901ms.
Fallback worker drain0,0054ms; sonraki transaction yayını38,730ms.
İç içe sayaçlar toplanmaz: source/index içindeki fsync veya CRC tekrar
üst toplamın üzerine eklenemez. Ham kanıt `write-hardening-profile-10k-20261027-*`.

Seçilen yaklaşım sınırı yalnız ötelemek yerine256 anahtar/8MiB'de bir WAL
kesitini dondurarak source/index hazırlığını worker'a taşımaktır. Her kayıt
CRC/key/version ile okunur. Anahtarlar/adresler/volatile proof Arc'ları
kopyalanır; belge gövdeleri saklanmaz. Worker mevcut8MiB ortak çalışma
bütçesine girer;6MiB akışlı intake ile aynı16MiB havuzunu paylaşır.

Yeni standart WAL boş checkpoint'i kesitin sequence'inde oluşturulur. İş
bitene kadar tail frame'leri hem eski authoritative WAL'a hem yeni WAL'a
sync edilir. Install tek MAN03 yayımıyla hazır run+tail WAL'i seçer. Önceki
root bütün işlemleri eski WAL'dan, yeni root kesit run'ını ve tail'i okur.
Eski okuyucunun anlayamadığı yeni kalıcı semantik/magic/codec yok. Eski
okuyucuyla gerçek veri doğrulaması ayrıca zorunludur. Elektrik kesintisi
garantisi çıkarılmaz.

Install sırasında yalnız kesitten sonra değişen anahtarlar aktif katmanda
kalır, adresler tail WAL'a taşınır. Eski snapshot kendi adres haritasını ve
eski WAL pinini korur; son okuyucu bırakınca eski WAL temizlenebilir.
Yayımlanmayan output ve mirror WAL, normal GC'nin canlı listesine eklenir.
Worker başarısızlığı writer'ı poison eder; eski durable root kurtarılabilir.

Hard backpressure2048 anahtar/64MiB WAL ve kullanıcının Options delta
sınırlarıyla sınırlıdır. Worker yetişmezse fallback beklemesi hâlâ mümkün;
768'lik koşuda aşım olmaması genel bir gerçek zaman garantisi değildir.
Owned/büyük fallback/explicit flush mevcut senkron sınırlarında worker'ı
bekler. Birden fazla worker ortak bütçeyi bekleyebilir; tamamlandığı söylenen
işler için ayrı coverage/peak/gate ölçümü gerekir.

İlk hedefli test: tail update/delete/insert, prefix/old snapshot tutarlılığı,
son snapshot sonrası WAL temizliği, reopen integrity; build/before-publish/
after-publish/ack dört process-kill sınırı geçti. Bunlar10k performans/oracle
ve eski okuyucu koşularının yerine geçmez.
