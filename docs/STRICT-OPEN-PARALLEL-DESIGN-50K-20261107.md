# Strict-open paralel maske doğruluk analizi — 7 Kasım 2026

UTC pencere **13:11:20–13:36:20**, yeni örnek sonu **13:33:20**.
Üretim motoru ve kalıcı biçim aynı. Önceki geçici kopya artık disk üzerinde
yok. Aynı sabit 50k slot, kabul edilmiş 159 dosya SHA envanterine göre
tek ölçüm kopyasına alınacak. Yeniden ingest, purge, ETW ve WPR kapsam dışı.

| Aşama | Paralel sınır ve doğruluk koşulu | Bu deney |
| --- | --- | --- |
| Manifest, aktif WAL ve delta shadow | Manifest run sırasını ve kaynak kimliklerini sabitler. Aktif WAL replay sequence ve değişiklik sırası bağımlı. Tam shadow ve delta hazır olmadan maske başlayamaz. | Seri |
| Native kaynak CRC ve belge kontrolü | Her immutable run kendi handle’ı ve manifest bağlamıyla bağımsız. Aynı run içinde frame/CRC birleşimi ve sequence sınırı sıralı. Hata veya eksik indeks onarımı yayıma karışmamalı. | Seri |
| SEG05 ve paged kayıt doğrulama | Run başına ayrı dosya/CRC ve ayrı sidecar bağlamı. Bir run içinde dizin, key/slot sırası, offset ve sayfa doğrulaması sıralı. Run sonuçları manifest sırasına yerleştirilmeli. | Seri |
| Görünürlük maskesi | Her **eski run** kendi bitsetine yazar, aynı değişmez run listesi ile daha yeni run’ları orijinal sırada tarar. Tombstone anahtarları eski revizyonu maskelemeye devam eder. Run baz maskesi cache için shadow öncesi, belge sayımı shadow sonrası alınır. | 1/2/4/8 işçi |
| Maske cache ve yayın | Cache kabulü 64 KiB ve giriş sınırı nedeniyle sıraya duyarlı. Join sonrası maskeler manifest sırasıyla cache’e işlenir. Worker'lar kaynak/index sahiplerini ödünç alır, hiçbir handle’ı emekliye ayırmaz. Snapshot yayımı/pin ve DB dönüşü join sonrasında. | Seri |

`std::thread::scope`, en çok **min(mantıksal işlemci, 8)** toplam worker,
çağıran thread dahil. Aynı run’a bir worker yazar. İş dağıtımı atomik sıra
indeksiyle, cache kurulumu sonuçların run sırası korunarak yapılır. Rayon ve
yeni bağımlılık yok. Cache kilidi değişmez. Paged metadata okuma ve decode
aynı global mutex içinde olduğu için, paralel kollar I/O ve cache çekişmesi
yüzünden yavaşlayabilir. Windows `seek_read` kullanılan paged erişimde
sayfa çağrıları mevcut mutex altında, dosya CRC/seek aşamaları bu deneyde
seri. İşçiler bitmeden dosya cursor’u kullanan sonraki yol başlatılmaz.

Geçici RAM bütçesinde worker stack commit, eşzamanlı kısa ömürlü anahtarlar,
run başına bitset sonuçları ve küçük iş kuyruğu var. Global metadata cache
bütçesi aynı. Bitset tahsisi run slot sayısından hesaplanacak, proses tepe
private commit ile açılış sonu private commit ayrıca ölçülecek. Ayrıntılı
profil, çift başına zamanlama ve örnekleme thread’i yok. Yalnız açılış
ve maske sınırlarında duvar, toplam process CPU ve private commit ölçümü
alınır. Toplam CPU çağıran thread ve scoped worker’ları kapsar.

Önce 1/2/4/8 ayrı oracle açılışı. Run kimliği, shadow öncesi ve sonrası
tüm maskeler, run görünür sayıları ve kurulan cache içeriği doğrudan byte
karşılaştırılır. 50.000 belge, sequence 529967, case0 50 hit ve CRC32
725765487 şart. İlk farkta ölçüm durur. Ardından aynı ikiliyle 12 dengeli
dört kollu sıcak blok, her kolda 12 bağımsız proses. Her örnek query oracle
ve dosya envanteri kontrolünden geçer. Audit buffer’ları süre/RAM
ölçümlerinin dışında. Toplam 4 audit + 48 zamanlı örnek üst sınırı.
Sonuçlar yeni kabul eşiği oluşturmaz.
