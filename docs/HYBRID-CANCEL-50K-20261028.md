# Melez capped ve query cancellation — 50k, 28 Ekim 2026

**Sonraki kullanıcı kararı:** negatif p95 için 4 işçiye göre +%10, bol-pozitif
CPU için serial'e göre +%5 tolerans kabul edildi. Bu raporun +%6,50 / +%1,93
sonucu yeni kapıyla **KABUL EDİLDİ**; bütçe 200, melez/iptal opt-in kaldı.
Aşağıdaki ret ve eski DB durumu, karar öncesi ölçümün tarihsel kaydıdır.
Yeni Türkçe corpus ölçümü ayrı [TRWiki ölçek raporundadır](TRWIKI-SCALE-20261028.md).

**200 adaylı melez prototip REDDEDİLDİ / kapı KALDI.** Negatifte4 işçiden
kötü olmama şartını sağlamadı: transubstantiation p95 **84,412→89,896ms
(+5,484ms / +%6,50)**. Bu fark “dalgalanma” diye kapatılmadı.200 aday içinde
ilk50'yi bulan pozitifler serial iş miktarını koruyor, ancak bol-pozitif CPU
bloklarında harpoon **+%1,93**; birebir ucuzluk garanti edilmiş sayılmaz.

İptal işlevi ölçülen kapsamda çalışıyor:240/240 çalışan sorgu Cancelled döndü,
kısmi sayfa yok, arama işçileri join olmadan cevap yok. Melez/iptal opt-in;
normal `search` exact/serial, `search_first` serial/opt-in. Join ve CRC reuse
varsayılan kapalı. Kaynak/WAL/manifest/SEG/payload biçimleri ve0.1 fixture aynı.

**Kapı nedeniyle sonraki DB aşamasına geçilmedi.** Yeni Türkçe corpus100k
CRC/SHA/oracle/offline analiz olarak hazır;50k/100k engine ingest/open/query
henüz ölçülmedi. Yeni1s/2s açılış sınırları kaydedildi; kaynak başına lazy açma
ve ön filtre uygulaması yapılmadı. Çalışma alanları hâlâ aynı eski50k DB'ler.

## Veri, sıra, semantik

Aynı mevcut ilk50k kitap havuzu, aynı sequence104/50k canlı/100 silinmiş ledger;
sabit FDB/SQLite slotları. DB reset/kopyası veya yeni belge corpus'u yok.
Yeni TR corpus bu ölçümle karıştırılmadı:7563/0 yanlış-pozitif eski negatif
stress sorgusu kontrol olarak korundu. Tek bir kelimeye özel engine dalı yok.
Melez bütçesi bütün sorgularda200, N50, offset0; title/text/fields/links/version
hydration, CRC/schema/key/version kontrolleri aynı. Sonuçlar exact ilk50 ve
50k tam get/deleted oracle'ıyla karşılaştırıldı.

İki dengeli sıra FDB/SQLite ve SQLite/FDB. FDB her kolda20 tekrar, pooled40
örnek/sorgu/seçenek; seçenek/sorgu sırası döndürülür. Her proses önce full get/
version oracle ve seçeneklerin bir ısınma turu. OS eviction yapılmadı; sıcak
OS/engine prosedürü, gerçek-soğuk değil. SQLite FTS5 trigram/detail=none/
columnsize=0, WAL/FULL,2MiB cache,mmap0,tempFILE. Bu yeni referans kolunda3+3
örnek var; n6 p95=max, üretim p95 kanıtı değil. Önceki n20 referansı ayrı kalır.
SQL capped COUNT(*)'yi çıkarır, id sıralaması nedeniyle tüm adayları sıralama
öncesi tüketebilir; FDB global erken durmayla aynı algoritma değildir.

Normal release zamanlaması; diagnostic ayrı iki tekrar yalnız sayaç için.
CPU GetProcessTimes bütün process query thread'lerinin user+kernel toplamıdır,
15,625ms kuantalama tek sorguda0 veya aşırı görünen pay verebilir. Bu yüzden
pozitif CPU kararında ayrıca1000 sorguluk bloklar kullanıldı. CPU ve duvar p95
farklı büyüklüktür; tanılama span'leri CPU diye sunulmaz.

## Pooled p95/ms — bütün sonuçlar

| Sorgu | Serial |4 işçi | Melez200 | Serial+token |4+token | Melez+token | SQLite exact | SQLite capped |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| harpoon | 4.908 | 31.010 | 8.452 | 8.003 | 33.513 | 9.282 | 2058.180 | 1011.434 |
| presumption | 13.390 | 61.144 | 12.450 | 13.160 | 60.888 | 13.704 | 2429.105 | 1158.807 |
| darcy | 5.232 | 26.235 | 5.298 | 3.464 | 26.320 | 5.221 | 1664.176 | 872.133 |
| transubstantiation | 233.912 | 84.412 | 89.896 | 245.197 | 84.345 | 91.638 | 2181.602 | 1078.805 |
| holmes elementary | 36.478 | 59.859 | 64.895 | 34.171 | 59.942 | 68.501 | 1342.015 | 644.217 |

Token sütunlarında iptal edilmeyen normal token vardır; paylaşımlı progress
sayacı yok. Query variant token kurulumunu da kapsar. Cancel latency probu
bundan ayrıdır. Harpoon melez p95 **4,908→8,452ms**; bu kötü örnek korunur.
CPU blok ortalamaları daha yakındır; bu iki metrik eşitmiş gibi yorumlanmaz.
Holmes elementary'de282 sonuç olmasına karşın200 adayda ilk50 dolmaz:
melez64,895ms, serial36,478ms. “Sonuç sayısı bol” tek başına erken dolmayı
öngörmez; false-positive oranı ve key sırası belirleyici.

## Normal örneklerde CPU

Hücre ortalama CPU ms /40 sorgu toplam CPU ms. Kuantize tek-query atfı nedeniyle
çok kısa sorgudaki CPU farkı tek başına eşdeğerlik testi değildir.

| Sorgu | Serial |4 işçi | Melez | Serial+token |4+token | Melez+token |
|---|---:|---:|---:|---:|---:|---:|
| harpoon | 5.078 / 203.125 | 103.125 / 4125.000 | 7.422 / 296.875 | 4.688 / 187.500 | 107.031 / 4281.250 | 5.078 / 203.125 |
| presumption | 12.109 / 484.375 | 214.062 / 8562.500 | 10.938 / 437.500 | 11.719 / 468.750 | 209.375 / 8375.000 | 11.328 / 453.125 |
| darcy | 1.953 / 78.125 | 92.969 / 3718.750 | 3.125 / 125.000 | 1.172 / 46.875 | 89.844 / 3593.750 | 4.297 / 171.875 |
| transubstantiation | 221.094 / 8843.750 | 287.109 / 11484.375 | 278.125 / 11125.000 | 223.047 / 8921.875 | 281.641 / 11265.625 | 277.344 / 11093.750 |
| holmes elementary | 32.031 / 1281.250 | 199.219 / 7968.750 | 188.672 / 7546.875 | 32.812 / 1312.500 | 199.609 / 7984.375 | 191.016 / 7640.625 |

## Bol-pozitif CPU blokları

Her kolda5 döndürülmüş blok×100 sorgu; ters seçenek sıralı ikinci kol,
**1000 sorgu/sorgu/seçenek**. CPU aralığında API+önceki owned sonucu bırakma ve
loop maliyeti; model oracle yok. Blok son sayfa tam karşılaştırıldı;40 normal
örneğin her sayfası tam oracle'dan geçti. CPU bloklarının her ara çağrısında
ayrı full-payload oracle çalıştırıldığı iddia edilmez. Aday sayısı ve handoff
sayaçları ayrıca aşağıda. Windows CPU kuantumu1000'e bölününce ortalama
çözünürlüğü0,015625ms; zamanlama/scheduler/ısınma belirsizliği bunun dışında.

| Sorgu | Serial ort.CPU ms |4 ort.CPU ms | Melez ort.CPU ms | Melez−serial | Toplam CPU serial/melez ms |
|---|---:|---:|---:|---:|---:|
| harpoon | 4.046875 | 106.093750 | 4.125000 | +0.078125ms / +1.93% | 4046.875 / 4125.000 |
| presumption | 12.562500 | 212.218750 | 12.343750 | -0.218750ms / -1.74% | 12562.500 / 12343.750 |
| darcy | 2.843750 | 87.281250 | 2.843750 | +0.000000ms / +0.00% | 2843.750 / 2843.750 |

Harpoon+%1,93, presumption−%1,74, darcy eşit. İş miktarı serialle aynı olsa da
“her pozitifte CPU≤serial” literal koşulu harpoon'da sağlanmadı. Yeni bir%5
hoşgörü sessizce eklenmedi. Negatif gecikme kapısı zaten bağımsız olarak kaldı.

## Handoff ve I/O kanıtı

| Sorgu | Serial aday |4 aday | Melez aday | Melez handoff | Serial/4/melez payload MiB |
|---|---:|---:|---:|---:|---:|
| harpoon | 93 | 3237 | 93 | 0 | 2.973 / 102.740 / 2.973 |
| presumption | 160 | 6198 | 160 | 0 | 5.564 / 212.312 / 5.564 |
| darcy | 92 | 3248 | 92 | 0 | 2.805 / 97.691 / 2.805 |
| transubstantiation | 7563 | 7563 | 7563 | 1 | 267.810 / 267.810 / 267.810 |
| holmes elementary | 673 | 4620 | 4620 | 1 | 21.659 / 146.998 / 146.998 |

Melez harpoon93/presumption160/darcy92 adayda50'yi bulup worker açmaz.
Negatifte toplam7563 aday/267,81MiB değişmez: serial200 yeniden taranmıyor.
Buna rağmen ilk200'ün serial doğrulanması ve global cursor hazırlığının serial
kalması paralel kuyruğun başlamasını geciktirir. Kalan source işleri dinamik
paylaştırılır, ancak kritik yol uzar. CPU negatifte azalabilirken duvar p95
artabilir. Bu profil200-aday serial fazının ms'sini ayrı ölçmedi;5,484ms farkın
hepsi tek bir alt faza atfedilmez.200→daha küçük eşik veya metadata ile baştan
strateji seçimi yeni deney önerisidir; **kapı kaldıktan sonra uygulanmadı**.

## İptal gecikmesi

DB strict doğrulanmış ve kaynaklar sıcak. Üç uzun sorgu×serial/4/melez/exact4×
32 veya256 aday başlangıcı hedefi×10 tekrar=240 deneme. Observed token yalnız bu
probda candidate-start sayacı taşır; sayaç cache-line/atomic maliyeti latency
probunun sınırıdır, normal CPU tablosunda yok. Controller100µs polling isteği
OS scheduler nedeniyle daha geç uyanabilir; ham kayıtta gerçek cancel anındaki
aday sayısı var. Bu hedefler tam32/256'da kesildi iddiası değildir.

Cancel→**dış query thread ve bütün nested arama işçileri join** süresi ölçüldü;
sampler/background process thread'lerini durdurma iddiası yok. Her cevap
Cancelled; partial hit/Exact(0) dönmedi. Biten/race sorgular ayrı sınıfa ayrılırdı,
bu koşuda0. Sonunda DB integrity/health tekrar geçti. Token resetlenmez; bir UI
isteğine bir token, önceki iptal/yeni token ve caller request-ID kontrolü gerekir.

| Sorgu | Yol | İptal/deneme | p95 ms | Max ms |
|---|---|---:|---:|---:|
| the zx | exact-four | 20/20 | 0.154200 | 0.236400 |
| the zx | four | 20/20 | 0.151800 | 0.172000 |
| the zx | hybrid | 20/20 | 0.171000 | 0.228100 |
| the zx | serial | 20/20 | 0.137800 | 0.157200 |
| transubstantiation | exact-four | 20/20 | 0.298600 | 0.320300 |
| transubstantiation | four | 20/20 | 0.228600 | 0.304300 |
| transubstantiation | hybrid | 20/20 | 0.488200 | 1.436200 |
| transubstantiation | serial | 20/20 | 0.415500 | 0.492700 |
| zx | exact-four | 20/20 | 0.164200 | 0.194400 |
| zx | four | 20/20 | 0.171300 | 0.233200 |
| zx | hybrid | 20/20 | 0.186000 | 0.189400 |
| zx | serial | 20/20 | 0.101900 | 0.135200 |

240 olay pooled p95 **0.410800ms**, max **1.436200ms**.

Bu sonuç warm2–50KiB kitap gövdeleri içindir. Yeni Türkçe corpus'taki200–444KB
uzun metinlerde henüz measured cancel iddiası yok. Senkron OS okuması, tek
belge CRC/schema/fold, metadata/visibility hazırlığının içi zorla kesilmez;
checkpoint beklenir. Soğuk/duran disk veya büyük field/range hazırlığında gerçek
zaman üst sınırı garanti edilmez. Pending/deferred source iptalli API'de açık
Invalid; doğrulama atlanmaz. Background açılış query token'ıyla iptal edilmez.
Permit beklemesi2ms kontrolle iptal olur; ayrı unit testinde bütün permit'ler
meşgulken doğrulandı. Çok-DB/çok-okuyucu admission latency ölçümü yapılmadı.

## Doğruluk ve sağlamlaştırma

Çok-run key sırası, budget1/20/200 handoff, N1/10/50/200, range/link/AND,
aktif owned+WAL delta güncelleme/silme, sabit snapshot epoch/pin ve reopen
oracle'ı genişletildi. Aynı38 özgün metin için N50 prefix oracle; eski okuyucuda
ayrıca38 özgün pagination case'i. Yeni API'ler process-kill recovery sonrası
oracle'a dahil edildi; kaynak kesilme/bozulma/fixture/kill aileleri tekrar koştu.

Kontrolde yeni iptalli delta yolunda+0.0/-0.0 exact uyumsuzluğu bulundu:
Value equality kullanımı owned indeksin JSON-literal anahtarından farklıydı.
[Önce başarısız olan regresyon kanıtı](benchmarks/hybrid-cancel-20261028-signed-zero-pre-fix.log)
korundu; iptalli yol mevcut owned ve light davranışlarını ayrı ayrı izler şekilde
düzeltildi. **Mevcut normal light/owned yollarının signed-zero eşitlik farkı bu
tur birleştirilmedi**, açık semantik maddesidir. Normal search davranışı değişmedi.
Bu son düzeltme yalnız token+aktif delta+exact field yoluna aittir; performans
DB'si delta0 ve sorgular fields/ranges boştu, yayımlanan zamanlama yolu değişmedi.

Son komut doğrulaması v3'tedir; v1 geçti, ek signed-zero testi sonrası v2'de
clippy cmp_owned uyarısı çıktı, serde JSON projection ile düzeltildi. Önceki
loglar silinmedi. Son normal239 test/diagnostic145, fmt/clippy/no-default/
all-features/native kontrolleri ve donmuş24 Ekim okuyucu50k/100 deleted/38 case
sonuçları aşağıdaki doğrulama ekindedir. Güç kesintisi garantisi yok.

## Kapı tablosu

| Kapı | Durum |
|---|---|
| Varsayılan exact/serial; first-page, join/CRC reuse opt-in | GEÇTİ; ayarlar değişmedi |
|200 içinde N dolan pozitif: worker0 / aynı aday-I/O | GEÇTİ |
| Bol-pozitif CPU her sorguda≤serial | KALDI; harpoon+%1,93, tolerans eklenmedi |
| Sıfır sonuçlu p95≤4 işçi | **KALDI**,84,412→89,896ms |
| Serial prefix tekrar doğrulanmasın | GEÇTİ; negatif7563 toplam aday |
| Query cancel çalışsın, kısmi sonuç/DB poison olmasın | GEÇTİ;240/240, ölçülen latency kapsamıyla |
| Yeni corpus50k≤1000ms /100k≤2000ms ilk arama | ÖLÇÜLMEDİ; melez kapısı sonrası duruldu |
| Yeni TR offline aday/gerçek | GEÇTİ; [tam41-case analiz](TRWIKI-OFFLINE-20261028.md) |

Kapı kaldığından melez varsayılan yapılmadı, sonraki ingest/open/cold/4R veya
daha küçük budget deneyine geçilmedi. Kod inceleme/tekrar için opt-in prototip
olarak kalır. Ön filtre ve per-source lazy uygulanmadı. Yeni açılış sınırları
önceki300ms kapısını değiştirir; eski başarısızlığı yeni corpus'ta ölçmüş saymayız.

[Program hash/sıra/koruma](benchmarks/hybrid-cancel-book50k-20261028-v1-design.json),
[bütün p50/p95/max, CPU blokları, cancel ve sayaç analizi](benchmarks/hybrid-cancel-book50k-20261028-v1-analysis.json).
Ham loglar `hybrid-cancel-book50k-20261028-v1-{pair0|pair1}-*-scale-query-experiments.jsonl`,
`...-cpu{0|1}-normal-scale-query-cpu-blocks.jsonl`,
`...-cancel-normal-scale-cancel-probe.jsonl`, `...-counters-diagnostic-*.jsonl`.
[Tasarım/API ve iptal sınırları](HYBRID-CANCEL-DESIGN-20261028.md).

Açık: yeni corpus50k/100k ingest/query/open,50k tekli/dirty500/4R/100'lü profil,
uzun snapshot, gerçek-soğuk cache, sorgu RAM'i ve geniş tarama/parçalı okumalar.
Case34 izlenir; reclaim default kapalı. Blok3/4,C ABI ve Frunner entegrasyonu yok.

## Son doğrulama kanıtı

- [v3 komut/exit-code tablosu](benchmarks/snapshot-10k-20261026-validation-hybrid-cancel-20261028-v3.json): cargo fmt --check, clippy all-targets -D warnings,239 default test/0 failed/14 ignored yardımcı; no-default ve all-features lib check; native SQLite fmt/clippy/test geçti.
- [Diagnostic lib145/0 failed/12 ignored](benchmarks/hybrid-cancel-20261028-v3-diagnostic-tests.log), [diagnostic clippy](benchmarks/hybrid-cancel-20261028-v3-diagnostic-clippy.log) geçti.
- [Yeni API38-case prefix oracle](benchmarks/hybrid-cancel-book50k-20261028-v1-oracle38-normal-scale-first-oracle.jsonl), [eski okuyucu50k/100 deleted/38 özgün case](benchmarks/hybrid-cancel-book50k-20261028-v1-compat-old-reader-utf8-v2.json) geçti.
- Process-kill/reopen aileleri default pakette yeniden geçti; streamed recovery sonrası melez ve iptalli melez hit oracle'ı eklenmiştir. Bu tur yeni50k churn/merge-kill DB yükü oluşturulmadı, power-loss iddiası yok.
- Corpus/fixture/working-source/model SHA eşitliği design kaydında. Yeni TR source SHA audit ile eşit; sabit DB slotları hâlâ eski50k/sequence104'tür. D: yalnız okunmuştur; yeni corpus/DB kopyası yok.
