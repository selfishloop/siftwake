# CRC ve payload doğrulamasını yakın parçalarla yürütme — 2026-10-18

Sonuç: prototip reddedildi, varsayılan motora alınmadı. Normal4-case
kontrolde zx bütün4 süreç çiftinde, holmes3/4 çiftte daha yüksek p95 verdi.
Parçalı kontrol hipotezi bu uygulamada hız kazancı sağlamadı. Runtime
cb2c19e'a byte-eşit döndü; private unsafe helper final engine'de bırakılmadı.
[Denenen kod ve testler](benchmarks/validation-chunk-20261018-prototype.patch).
Gruplama denemeleri de kapalıdır. Bu adım payload I/O penceresini, aday
seçimini veya metin eşleşmesini değiştirmedi.

## Önce mevcut sürümün ölçümü

cb2c19e runtime'ı önceki f6d18dd ile aynıdır. Önceki immutable build'deki
SHA-256 doğrulanmış diagnostic okuyucu iki etikette aynı program olarak
2 dengeli süreç çiftiyle çalıştırıldı. Araç bütün38 senaryoyu profiller;
karar için seçilen4 sorgunun12 örneği birlikte aşağıda özetlendi.
Bu bir eski/yeni hız karşılaştırması veya normal p95 değildir.

| Sorgu | Toplam ms | Payload read span ms | CRC ms | Yapı/UTF-8 ms | Metin eşleşme ms | Sonuç hazırlama ms |
|---|---:|---:|---:|---:|---:|---:|
| quixote,0/50 |2,688|0,408|0,184|0,142|0,130|1,342|
| holmes,0/50 |29,717|12,455|3,491|4,215|5,528|1,176|
| zx,0/50 |158,631|55,062|26,849|26,569|40,780|0,002|
| the,0/200 |6,314|0|0|0|0|6,055|

Span'ler iç içedir ve timer/mutex maliyetini içerir; satırları toplayarak
CPU veya exclusive toplam çıkarılmaz. Read span, mevcut pencere hit'lerini
de kapsar; zx'te10.000 span/1.104 gerçek read API çağrısı vardır. API read
sayısı fiziksel disk transfer sayısı değildir. UTF-8 ve metadata yapısı aynı
doğrulama span'indedir; UTF-8'e tek başına süre atfedilmez. the/200'de aday
contains doğrulaması yoktur; dönen belgelerin CRC/decode'u sonuç aşamasındadır.
quixote'ta sonuç hazırlama baskındır; tek değişikliğin hepsini aynı oranda
hızlandıracağı varsayılmaz. [12 örnek/4 sorgu](benchmarks/validation-baseline-four-20261018.json),
[38-case kaynak profil](benchmarks/validation-baseline-10k-20261018-summary.json).

## Tek prototip

CRC ile UTF-8 iki farklı zorunlu kontroldür; biri kaldırılmaz. Sadece
bounded-engine contains yolundaki binary payload'larda yeni checked_with_crc
fonksiyonu denenir. Uzun stringler8KiB parçalarla UTF-8 doğrulanır ve hemen
IEEE CRC durumuna eklenir; header ve metadata dahil bütün payload byte'ları
sırayla CRC'ye katılır. Her field/link yapı kontrolü, key/version ve source
kontrolleri sürer.8KiB altı payload eski CRC+checked yolunu kullanır.

Hipotez, büyük veride aynı byte'lara yakın zamanda dokunmanın işlemci
önbelleği maliyetini düşürmesidir; henüz normal hızla kanıtlanmadı. CRC ve
UTF-8 mantıksal olarak yine her byte'ı kontrol eder; tek aritmetik işlem veya
tek kontrol diye sunulmaz. Yeni heap tamponu/cache/format/feature/bağımlılık yok.
Normal get/hydration, kaynak açılışı, yazıcı ve no-default reference aynı yoldadır.

Özel helper, her biri geçerli UTF-8 olan bitişik parçaların tamamını doğruladıktan
sonra bütün slice'ı str olarak ödünç verir. Tek private unsafe dönüşümün gerekçesi
budur; değiştirilmez byte'lar aralıksız kapsanır. En fazla3 continuation byte
geri alınır, bozuk veri geriye sınırsız tarama veya sıfır-uzunluk parçası üretemez.
Hatalı kayıtlarda ilk hata metni değişebilir; sınıf yine Corrupt'tır.

Yeni diagnostic sürüm CRC/yapı kontrolünü tek
query.payload_crc_and_validation span'inde tutar. Eski iki ayrı span'le
aynı isim altında karşılaştırılmaz. Diğer iş sayıları ve normal p95 ayrı ölçülür.

## Dar kabul kontrolü

Mevcut38-case oracle'dan0=quixote,11=holmes,20=zx,31=the/200 alınır.
Her biri her tur ölçülür:4 süreç çifti×4 strict blok×25 tur=400 örnek/
sorgu/variant. Sıra dengeli, diagnostic ve ek thread sayaçları kapalıdır.
Motor sorgu sözcüğüne göre özel yol seçmez. Baseline önceki immutable
normal programdır; runtime/bağımlılık/harness aynıdır. Yeni kaynaklar ve
derleme [manifestte](benchmarks/validation-chunk-20261018-build-manifest.json) dondurulur.

Dar kontrolde tekrarlanan kazanç görülürse38-case ve ayrı query/get bellek
koşusuna geçilir; sonuç belirsiz/yetersizse terfi verilmez. Pooled p95 ile
tipik blok farkları ayrı raporlanır;16 blok16 bağımsız süreç değildir.
Yeni iş yükü/corpus veya DB kopyası gerekmez.

## Veri ve sınırlar

Aynı mevcut ilk10.000 belge/8 kitap;264.842.858B text,2.057–51.197B pasajlar.
FDB349.805.698B/18 dosya, sequence20/delta0/5 Run. SQLite583.860.224B ve
0.1 fixture de snapshot ile izlenir. Yeni corpus/DB/reset/repack/kopya yok.
Normal strict open tam kaynak/indeks kontrolü yapar; pending0. Açılış ve
bağımsız corpus oracle'ı süre dışındadır. Oracle tam belge/alan/link/version/
total/sıra eşitliğini denetler. OS cache sıcak, eviction yok. Windows11/
Ryzen7 7840HS, Rust1.99/MSVC release thin-LTO/cgu1, affinity4/normal priority/
Balanced; frekans/SMT/arka plan izolasyonu, concurrent ve gerçek-cold yok.

Üç hedefli test geçti: Unicode scalar/parça sınırları ve belge/CRC eşitliği;
invalid UTF-8'in sınır/tail davranışı ve std ile karşılaştırma; encoded kayıtta
her cut ve her byte değişimi (hem eski CRC hem yeniden hesaplanmış CRC ile)
eski schema kabul/red sonucuna karşı. All-features/all-targets Clippy ön kontrolü
temiz. İlk derlemedeki helper görünürlük hatası düzeltildi; başarılı test
kaydı [buradadır](benchmarks/validation-chunk-targeted-fixed-20261018.log).

## Normal4-case sonucu ve karar

Her sorgu/variantta400 örnek;4 süreç çifti×4 strict blok×25 tur.
Diagnostic ve ek thread sayaçları kapalı.16 blok16 bağımsız süreç değildir.

| Sorgu, offset/limit | Kontrol p95 ms | Aday p95 ms | Fark | Tipik blok fark | Hızlı çift/blok |
|---|---:|---:|---:|---:|---:|
| quixote,0/50 |3,7941|3,8407|+%1,23|-%7,91|3/4;13/16|
| holmes,0/50 |35,1585|35,7911|+%1,80|+%0,79|1/4;3/16|
| zx,0/50 |189,3793|189,9893|+%0,32|+%1,77|0/4;5/16|
| the,0/200 |6,0823|5,9391|-%2,35|-%7,01|3/4;13/16|

zx süreç çiftlerinde +%7,44/+%4,72/+%3,94/+%2,08 p95 verir; pooled farkın
%0,32 olması süreç farklarının aritmetik ortalaması olduğu anlamına gelmez.
holmes3/4 çift ve13/16 tipik blokta yavaştır. quixote p95 ve tipik blok farklı
yönlerdedir. Yeni helper'a girmeyen the/200'de de fark vardır; bunu yeni
doğrulama algoritmasının kazancı saymak doğru olmaz. Frekans/yerleşim/cache/
diğer süreç payları tamamen ayrılmadı; küçük farklara kesin CPU nedeni atanmaz.

Hedef doğrulama yolunda tekrarlanan kazanç yoktur; kabul verilmedi. Ek CRC
update/UTF-8 çağrıları ve sınır hazırlığı maliyet oluşturabilir, fakat bunların
tek tek CPU payı bu deneyde ölçülmedi. Parçalama boyutu ayar turuna girilmedi.
Yeni geniş38-case aday, ayrı aday diagnostic veya RAM koşusu yapılmadı;
bu alanlarda kazanç/gerilemesizlik iddiası yok. Baseline38-case profili vardır,
bu bir yeni/eski38-case hız karşılaştırması değildir.
[Normal ham sonuç ve bütün çift/blok farkları](benchmarks/validation-chunk-focus-10k-20261018-summary.json).

## Geri dönüş ve final doğrulama

Arşiv patch'i hem tracked değişiklikleri hem yeni helper/test kaynağını
içerir. Geri alma öncesi ters, sonrası ileri `git apply --check` geçti;
patch final kontrolünde uygulanmadı. segment.rs ve payload.rs cb2c19e'a
byte-eşit döndü; helper modül dosyası kaldırıldı. Format/feature/bağımlılık,
protokol ve0.1 fixture değişmedi. Üç prototip testi başarıyla çalıştı.
Geri dönülen motorda7 mevcut window/corruption testi, fmt ve default/
all-features/all-targets Clippy geçti. Normal release CLI ve iki okuyucu
yeniden derlendi. Bu tur tam-suite yeniden çalıştırılmadı; aynı cb2c19e
runtime'ının önceki180 test sonucu yeni koşu gibi sunulmaz. İlgili üç
prototip testi ve yedi geri dönüş testi bu turun yeni doğrulamasıdır.

Final gerçek CLI:10.000 belge/sequence20; quixote56/50 hit, holmes1222/50,
zx0/0, the10000/200; total/hit/version eşitlikleri geçti. Baseline profil,
normal4-case ve final snapshot21 dosyada SHA-eşit:18 FDB dosyası349.805.698B,
SQLite ve iki0.1 fixture. Corpus'un tamamı2.740.862.279B, SHA-256
`11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B`
ile akışlı yeniden kontrol edildi. Yeni veri dizini yok; kaynak/indeks/fixture
byte'ları değişmedi. [Final kanıt](benchmarks/validation-chunk-final-proof-20261018.json).

## Sonraki sınır

CRC ve UTF-8 aynı kontrolün tekrarı değildir. İkisini küçük parçalarla
çalıştırmak bu deneyde daha hızlı olmadı. Bundan sonraki ayrı hipotez,
pozitif aramada aynı belgenin eşleşme kontrolü sonrası sonuç hazırlanırken
tekrar okunup doğrulanmasıdır. quixote profilinde sonuç aşaması1,342ms;
bu maliyetin tamamının kaldırılabileceği varsayılmaz. Aynı12 profil örneğinde
quixote56 candidate CRC ve50 payload.decode_crc_identity, holmes1285+50,
zx10000+0, the/2000+200 çağrı verir. Dolayısıyla dönen pozitif belgelerdeki
ikinci CRC/decode işi somuttur; negatif sorguda bu hedef yoktur. Aynı doğrulanmış
byte'ları tekrar kullanmak, görünürlük/sürüm/sıralama ve küçük RAM bütçesini
birlikte koruyan ayrı bir tasarım/ölçüm gerektirir; bu tur uygulanmadı.

## Açık işler

Genel arama sapmaları, sonuç hazırlama, yazma/bakım, normal Vec/query/caller
RAM'i, açılış, disk sıkıştırma, eşzamanlı/gerçek-cold yük açık. Bu dar deney
bu başlıklarda yeni performans sonucu değildir.
