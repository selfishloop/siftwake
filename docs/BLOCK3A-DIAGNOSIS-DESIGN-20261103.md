# Blok 3a — önce tanı tasarımı, 3 Kasım 2026

**Kullanıcı 3 Kasım'da yalnız 50k sınırlı tanıyı onayladı.**
[Yürütme sınırları](benchmarks/block3a-20261103-plan.md) ve
[tanı raporu](BLOCK3A-DIAGNOSIS-20261103.md) geçerlidir.
Payload optimizasyonu ve 10k owner reset onaylı değildir.
İlk sınırlı koşu harness hatasında durdu; A/A/maintenance tamamlanmadı.
Sonraki [yalnız çalışma alanı onarımı](BLOCK3A-REPAIR-20261103.md) tamamlandı,
slotlar ready. Bu belge yeniden koşma/harness düzeltmesi/optimizasyon onayı
değildir; sonraki blok kullanıcı kararına bırakıldı.
[Brife göre durum tablosu](BRIEF-BLOCK-STATUS-20261103.md) geçerlidir.
Ek-TC kapanışı özgün Blok 2'yi veya 3a'yı kapatmaz. Yeni cache adayı,
kapasite ayarı, tek-uçuş veya sharding turu önerilmiyor.

## Soru ve mevcut kanıt

İki ayrı soruyu ölçümde birbirine karıştırmayacağız:

1. **Payload parçalanması:** aynı 10k canlı aday / 268,304 MB istenen payload için
   `the zx` okuma isteği referansta 1112, adayda 2013. Ölü kayıtlar zaten posting
   kesişimi/payload doğrulamasından önce görünürlük maskesinde eleniyor.
   [Eski profil](REVISION-VISIBILITY-SEARCH-10K-20261026.md) read/CRC/schema/text
   paylarını ayırdı; artan istek sayısı bütün p95 farkının tek nedeni değildir.
2. **Run/sözlük maliyeti:** [Ek-TC](TERM-CACHE-50K-20261102.md) sırasında canlı
   run sayısı başlangıç 44 → normal matris sonrası 7 → havuz sonrası 52;
   K görünürlük gap p95'i bloklarda 12,338 → 23,735 → 30,184 → 32,385 ms.
   Bunlar birlikte değişti, fakat run sayısının gap'e nedensel katkısı
   **bilinmiyor**. Aynı mantıksal 50k, aynı fiziksel sürüm/yerleşim/cache değildir.
   İşlem tipleri ve dokunulan owner/sayfa sayıları da değişti. Önceki seyrek
   profilli sequence'ler kollar arasında aynı işlem tipini seçmediği için
   bunlardan saf eşleştirmeli neden sonucu çıkarılamaz.

Payload okumasını birleştirmek, görünürlükte baskın çıkan terim sözlüğü
hazırlığını kendiliğinden çözmez. Tanı bu iki maliyeti ayrı raporlayacak.

## Kapsam, veri ve yerleşim

İlk tanı mevcut sabit FDB/SQLite slotlarında **881756DA… ilk 50k** ile yapılır;
yeniden ingest/DB kopyası/corpus üretimi yok. `private/` okunmaz. K varsayılan
try/seri, A/D kapalı, reclaim KAPALI; WAL/sync/256 KiB/format/CRC aynı kalır.
Cache sıcaklığı “aynı süreçte ısıtılmış” ve “yeni süreç/boş uygulama cache'i”
olarak ayrı kaydedilir; ikincisi gerçek OS-soğuk cache diye sunulmaz.

Her örnek grubu öncesi manifest generation, snapshot sequence, run ID'leri,
boyutlar, görünür/örtülü/tombstone sayıları kaydedilir. Yazmasız eşleştirme
sırasında yayım/bakım ile yerleşim değişirse grup karşılaştırılabilir sayılmaz;
ham örnek korunur, gizli tekrar veya hedef run sayısına ulaşana kadar döngü yok.
Tek snapshot içinde run başına maliyet ayrılabilir; az run benzetmek için
run atlamak, tombstone/barrier veya shadow maskesini eksiltmek yasaktır.

İkinci bir yerleşim karşılaştırması onaylanırsa yalnız mevcut
`maintain_idle()` bir kez çağrılır; reclaim/ayar değiştirilmez. Önce/sonra
tam model oracle'ı, gerçek run sayısı ve bakım bedeli kaydedilir. Bakım run
sayısını azaltmazsa “karşılaştırılabilir ikinci yerleşim oluşmadı” diye durulur;
yeni rewrite politikası veya otomatik reclaim açma yok. Bu karşılaştırma da
run sayısını diğer yerleşim etkilerinden tek başına ayırmış sayılmaz.

**Özgün 3a kabul korpusu ayrıdır:** mevcut kitap havuzunun ilk 10k belgesi ve
38-case sorgu/offset/limit/API sözleşmesi. TR 41-case bunun yerine geçmez.
Şu an sabit slotlar 50k TR tuttuğu için özgün kabul koşusu, 50k slotun neden
sıfırlanacağını açıklayan **ayrı owner-reset/ilk 10k yeniden ingest onayı**
gerektirir. Bu tasarım o sıfırlamayı veya sonradan 50k'yı yeniden kurmayı
yetkilendirmez. Kitap case34 `ishmael /130/1`; TR case34 `kendisine çıkacağını`dır.

## Tanı sayaçları ve görünürlük deneyi

| Katman | Ayrı ölçülecek kanıt |
|---|---|
| Yayım/edinme | commit başlangıç/dönüşü, sequence, snapshot acquire, maske/delta hazırlığı, ilk sorgu başlangıç/bitiş; commit içindeki hazırlık gap'e yeniden eklenmez |
| Terim/posting | Run/owner başına lookup ve probe, hit/miss, capacity/oversized/drop tahliyesi, mutex edinimi/beklemesi, read/CRC/decode, izin beklemesi ve posting intersection |
| Payload | Görünür aday sayısı; read çağrısı, istenen/gerçekte API'den alınan byte, pencere isabeti, aynı dosyadaki komşuluk ve atlanan byte aralıkları; dosya geçişi |
| Doğrulama/sonuç | Payload CRC/kimlik/schema, normalleştirme/contains, sonuç hazırlığı; exact total ile capped sınırı ayrı |
| Süre/bellek | Wall ve bütün işçileri kapsayan process CPU; model/oracle/history dışı motor-only idle/peak ve ayrı harness peak |

I/O çağrısı ve istenen byte, fiziksel disk/OS-cache miss diye adlandırılmaz.
Parent/child span'ler üst üste toplanmaz; ölçülemeyen pay “bilinmiyor” kalır.
Profil açık örnekler normal p95'e karıştırılmaz; frozen normal/tanı ikililerinin
hashleri ve profil maliyeti ayrı kaydedilir. Sayaçlar bounded toplam/histogram,
en çok 64 owner ve taşanlar toplamı biçimindedir; belge başına kalıcı metin/key
haritası veya sınırsız sequence geçmişi motor belleğine eklenmez.

Run/gap ilişkisi için aynı havuzdaki aynı anahtar/gövde/işlem tipi eşleştirilir.
Yeni havuz/corpus yok; mevcut donörlerden tek seferlik küçük havuz hazırlığı
gerekirse zamanlama dışında, ayrı raporlanır. En çok 16 anahtarda 64 adet
2–4 KiB update (toplam yeni gövde ≤256 KiB) ile N60+ gap örneği alınır; 4096 işlem/kol yeniden
oynatılmaz. İşlem sayısı run oluşturmak için büyütülmez. Kontrol/SQLite sırası
döndürülür, 4 okuyucunun sorguları aynı kalır. Küçük update tanısı eski karma
Frunner B iş yükünün mutlak performans tekrarı diye sunulmaz. Snapshot/run
değişimi varsa o örnekler ayrıca işaretlenir; aynı yerleşim iddiası yapılmaz.
Ana 50k korunur, havuzun mantıksal değerleri oracle ile geri yüklenir; sequence
ilerlemesi ve fiziksel bakımın geri alınmadığı açık yazılır.

## Tanıdan sonra değerlendirilebilecek tek yön

**Ardışık canlı payload pencerelerini birleştirme**, ancak tanı yeterli aynı-dosya
komşuluğu ve önemli read maliyeti gösterirse önerilir. Bu tur uygulanmaz.
Mevcut immutable source/owner içinde sınırlı read-ahead penceresi kullanılabilir;
farklı dosyalar tek aralık yapılamaz. Her belgenin mevcut CRC, key/version,
schema ve bounds kontrolü korunur; aradaki ölü byte okunması, ölü belgenin
sonuç/aday sayılması değildir. Fazladan byte ve bellek bedeli de ölçülür.
Key sırası, exact total, capped N/offset, full shadow, eski snapshot/pin ve
cancel zinciri değişmez; yeni worker/izin/bütçe veya format eklenmez.
Yerinde kaynak yeniden yazımı gerekiyorsa bu önerinin dışındadır: gerekçe,
sürüm/uyumluluk planı ve ayrı kullanıcı onayı gerekir.

Metadata/CPU baskınsa veya komşuluk yoksa pencere değişikliğinin beklenen
kazancı kanıtlanmamış olarak raporlanır; otomatik başka optimizasyon seçilmez.

## Kanıt planı, başarı ve durma kuralı

Tanı onayından sonra önce mevcut 50k'da run/gap ve zx/the zx aşama tablosu
yayımlanır. Bu bilgi amaçlıdır; **3a GEÇTİ kararı vermez**. Uygulama önerisi
ve özgün 10k kabul ölçümü için yeniden onay istenir.

İleride uygulama onaylanırsa mevcut 50k normal 41-case/gap kontrolü de N60 ve
dört dengeli çiftle korunur: idle p95 ≤30 ms ise loaded ≤30 ms; aksi halde
loaded ≤2×idle. Bu kapı önceki KALDI kaydıyla beraber raporlanır; özgün 38-case
3a kapısının yerine geçirilmez. Gap başarısı yalnız toplam gap/ilk sorguda
eşleştirmeli net kazanımla değerlendirilir; yeni sayısal eşik eklenmez.

Özgün kabul koşusunun değişmeyen kapıları:

- `zx` ve `the zx` **her birinde** p95 en az %30 azalır: aday/kontrol ≤0,70.
- Diğer **36 sorgunun her birinde** p95 kontrolün ±%5 aralığındadır.
  Daha hızlı sonuç da ham ±%5 tablosunda olduğu gibi gösterilir; kural yeniden
  yorumlanmaz. Offset/limit ve exact/capped sözleşmesi değiştirilmez.
- A/A ile ortam yayılımı ayrıca verilir; A/A ±%5 dışına taşarsa kapı gevşetilmez
  veya otomatik gürültü muafiyeti verilmez. Sonuç kararsız olarak kullanıcıya döner.

Önerilen düzen: sorgu başına en az N60, nearest-rank p95; dört dengeli
kontrol/aday çifti, her çiftte SQLite FTS5 trigram mevcut ayarlarıyla referans.
Sıra K/A/SQLite, SQLite/A/K dönüşümlü; ayrıca K/K A/A. Her sorgunun ham örnek,
p50/p95/max, CPU, byte/istek, cache durumu, her çift kararı ve pooled sonucu
raporlanır. Sonuç havuzlayarak tek çiftteki başarısızlık saklanmaz.
Kontrol/aday aynı mantıksal ve fiziksel DB'yi kullanır; yerleşim deneyi ayrıca
etiketlenir. Yeni kopya tutulmaz.

Oracle (exact/capped, key sırası, eski/yeni revizyon), CRC/bozulma, cancel,
snapshot sequence/pin, kill/reopen ve eski okuyucu her ilgili değişiklikte
yeniden doğrulanır. Güç kesintisi iddiası yok. Kaynak bozukluğu sağlam cache
ile kabul edilmez. Kod değişirse ilgili testler, fmt ve clippy gerekir.

**Doğruluk bozulursa hemen dur.** Tanı net iyileşme yönünü kanıtlamazsa
uygulama yapılmaz. Uygulama için ayrı onay verilse de performans kapısı kalırsa
veya net toplam sorgu/gap kazancı kanıtlanmazsa varsayılan korunur, sonuç
yazılıp kullanıcıya dönülür; yeni aday/ayar/tekrar turuna kendiliğinden geçilmez.
Kilit sayacının veya run sayısının azalması tek başına başarı değildir.

**İstenen sonraki onay:** yalnız mevcut 50k üzerinde bounded tanı sayaçları,
yukarıdaki küçük eşleştirmeli görünürlük ölçümü ve tek mevcut bakım çağrısıyla
önce/sonra gözlem. Payload optimizasyonu, 10k owner reset, Blok 3b/4, 100k,
gerçek-soğuk cache, uzun snapshot ve C ABI bu onayın dışında kalır.
