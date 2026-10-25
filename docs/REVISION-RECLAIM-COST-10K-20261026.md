# Revizyon geri kazanma maliyeti — 10k / 26 Ekim 2026

**Bakım ek kapısı: KALDI.** Politika varsayılan kapalıdır. Kullanıcı önceki disk/oracle/kill sonucunu kabul etti; case34 izleniyor. Bu rapor yeni bakım/yük kapısını ayrı ölçer.

| Kapı | Gözlem | Karar |
|---|---|---|
| 10 tur bakım ≤12s | 13.950/13.500s | KALDI |
| Bakım sonrası disk ≤1,3× | 10 tur 1.2004×; tur30 1.1417× | GEÇTİ (gözlenen) |
| Rewrite sırasında tek commit p95≤100 / max≤250ms | n64, 77.175/82.594ms | GEÇTİ (gözlenen) |
| Yazma başlangıcı → oracle ile görünürlük | p95 185.135, max 202.408ms | Önceki 1s hedefinin altında |
| Rewrite read p95≤2×idle | En yüksek 1.533×, 38 durum/n20 | GEÇTİ (gözlenen) |
| Sıcak38-case p95 kontrol +%5 | case34 hariç aşan 24/37 | KALDI; case34 ayrıca |
| Oracle / kill / eski okuyucu | GEÇTİ (gözlenen) / GEÇTİ (gözlenen) / GEÇTİ (gözlenen) | Yalnız gözlenen koşullar |

## Yöntem ve sınırlar

Mevcut aynı ilk10000, aynı donör/UTF-8 kesme/revizyon marker modeli; yeni corpus yok. Her tur1000 güncelle/500sil/500ekle,100'lük akışlı commit;10k canlı sabit. İki FDB sırası reference→candidate-fixed / candidate-fixed→reference. Reference, kabul edilmiş önceki %20/sabit32MiB/s politikasını aynı binary içinde çalıştırır. Düzeltme yalnız candidate dalındadır. Corpus/0.1 SHA koruması ve yalnız sabit FDB/SQLite slotları kullanılır.

SQLite referansı aynı oturumdaki compare-v2 iki10tur churn/verify koşusudur; frozen executable değişmedi: FTS5trigram/detailnone,FULLWAL,cache2MiB,mmap0,tempFILE. Bu Rust düzeltmesi için SQLite yeniden üretilmedi. Normal38-case SQLite karşılaştırması önceki rapordadır; bu rapor SQLite için yeni60örnekli sorgu p95 iddia etmez. Eski5,6s kontrol, geri kazanma kapalı önceki ölçümdür; yeni reference geri kazanma açık eski32,4s yoludur.

Normal kabul ölçümleri diagnostic feature kapalıdır. Aşama profilleri ayrı diagnostic-timing derlemesidir; yüzdeler CPU değil exclusive wall/critical timeline payıdır. Foreground join süresi o sırada çalışan worker aşamasına atanır. Ham interval kayıtları tutulur; iç içe span'ler iki kez toplanmaz. Trace analiz süresi ayrıca CSV'dedir. Profilin mutlak toplamı normal derlemeyle eşit kabul edilmez.

Posting_remap_write, ID çevirimi+sıra/bounds kontrolü+posting akış yazımını birlikte ölçer; tek scalar map lookup'ı izole edilmiş gibi sunulmaz. Source_validate_replay CRC+şema/native kayıt doğrulaması ve metadata kurulumunu içerir; salt CRC taraması ayrı sütundur.

Yük probunda gerçek source-copy dosyası≥256KiB gözlenir ve worker hâlâ çalışırken çağrı başlar. Her koşuda32aktif yazı, her query için10eşlenmiş warm idle/load örneği; iki koşuda64ve20. Bir gerçek güncelleme her okuma çifti arasında worker'ı preempt edip terk edilen çıktıyı sonraki publication/reclaim ile toplar. Bu yazı ve schedule/poll süreleri query timer'ında yoktur. Bağımsız foreground writer+reader thread/snapshot API yapılmadı; Adım2 kapısı sayılmaz.

Yazı/yük örneklerinin varış noktası kaynak kopyasıdır; tüm CRC, posting, fsync ve install evrelerine rastgele varış dağılımı ölçülmedi. Gösterilen p95/max bu gözlenen örneklerindir, her evre için üst sınır garantisi değildir. Özellikle OS I/O ve install fsync kuyruğunda en kötü gecikme açık kalır. Ham logların 20261025 prefix'i görev başlangıcıdır; rapor tarihi yerel gün değişiminden sonra 26 Ekimdir.

Nearest-rank quantile; kısa query'lerde oran yanında mutlak ms verilir. OS cache sıcak; background/power/CPU izolasyonu yok. Private/WS OS sayaçları ayrı; örnekleme kısa sıçramayı kaçırabilir. Güç kesintisi iddiası yok. Owned/Vec yoluna yeni hız iddiası yok; bu turun kabul yolu streaming'dir.

## Ölçülen teşhis ve seçilen düzeltme

İlk10tur eski profilinde pacing14,023s (%37,84), posting stream6,959s (%18,78), indeks yazma/açma3,791s (%10,23); toplam37,063s. Bu nedenle sabit pacing, düşük kazançlı tekrar yazım ve fazladan doğrulama/açma işi hedeflendi. ID-map tablosunu kurmak <0,001s; asıl posting maliyeti map aritmetiğiyle sınırlı değildir.

Yeni seçim ≥%50 ölü payload'ı önceler; toplam ölü payload görünür payload'ın %20'sini aşarsa ≥%20 segmentleri de kazanç oranıyla seçer. Bir metadata görünürlük maskesi plan ve kopya seçimi için tekrar kullanılır. Tamamen gölgelenmiş, tombstone0 segmentlerden bitişik ≤256MiB grup sadece doğrulanıp manifestten emekli edilir: kaynak/indeks çıktısı yok. En yeni tombstone bariyerleri tutulur.

Boşta kopya pacing beklemez; get/search/search_into aktifken32MiB/s pacing uygulanır. Writer gelince yalnız unpublished revision worker'ına cancellation verilir. CRC64KiB, native entry, posting refill/term sınırlarında kontrol edilir; pacing Condvar ile uyanır. Writer cancellation join'i bitmeden input/builder lease almaz. Tamamlanmış worker install edilebilir. Fsync/OS I/O zorla kesilmez; süre sınırı formal garanti değildir. Sürekli writer bütün revision işlerini iptal ederse bakım ilerlemesi boşta yeniden planlamayı bekler; bu koşuda sürekli yük altında disk üst sınırı kanıtlanmadı.

Aynı yeni kaynağın tekrar açılması kaldırıldı: son tam CRC/şema doğrulaması öncesi immutable SourceLease alınır, doğrulanmış builder/index ve aynı handle korunur. Sequence/length/mtime/binding kontrolü devam eder. WAL/source/manifest/payload/SEG sürümü veya fsync politikası değişmedi; 0.1 fixture korunur.

İlk checked-object uygulamasında index_terms=true yanlışlıkla trigramları yeniden üretip sonra posting reuse yapıyordu: candidate-v4 sonucu başarısız deneme olarak korundu. İlk bool düzeltmesi SourceLease çağrısına yanlış uygulandı; candidate-v5 de gate sayılmadı. Son v6 iki isimlendirilmiş immutable/records-only helper kullanır ve yayımlanan çıktının yazma erişimini reddettiği testlidir; CRC/şema kontrolleri aynı. İlk soğuk-idle/sıcak-load smoke sonucu da gate sayılmadı; untimed warmup iki tarafta eşlendi. Referans yük probunun32yazısı tutuldu, gerekmeyen eski read kalibrasyonu saved model/manifest sequence615 commit sınırında durduruldu ve full oracle yeniden geçti. Bütün kesilen/bad trial logları korunur.

## Aşama tablosu

Hücreler ms (% toplam). Saf kaynak CRC ve native CRC/şema doğrulaması aşağıda birlikte gösterilir; ham CSV her birini ayırır. ID map kurulumu ve kalan worker/other süreleri CSV içinde tutulur; aşağıdaki baskın sütunlar toplamın tamamı değildir. [Bütün aşamalar ve yüzde payları](benchmarks/revisions-cost-10k-20261025-stages.csv).

| Sürüm/tur | Toplam ms | Kopya | Pacing | Kaynak CRC+verify | Posting remap/write | İndeks+CRC | Plan | Fsync+yayım |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| before/1 | 2455.1 | 251.9 (%10.3) | 342.9 (%14.0) | 402.6 (%16.4) | 730.8 (%29.8) | 399.3 (%16.3) | 46.0 (%1.9) | 237.5 (%9.7) |
| before/2 | 3639.5 | 218.3 (%6.0) | 1755.0 (%48.2) | 368.8 (%10.1) | 635.7 (%17.5) | 375.3 (%10.3) | 72.8 (%2.0) | 196.3 (%5.4) |
| before/3 | 3509.3 | 300.9 (%8.6) | 897.5 (%25.6) | 477.9 (%13.6) | 784.3 (%22.3) | 499.3 (%14.2) | 88.9 (%2.5) | 418.5 (%11.9) |
| before/4 | 3496.3 | 231.2 (%6.6) | 1466.3 (%41.9) | 382.0 (%10.9) | 600.9 (%17.2) | 433.0 (%12.4) | 127.2 (%3.6) | 234.2 (%6.7) |
| before/5 | 3067.2 | 301.1 (%9.8) | 813.2 (%26.5) | 410.1 (%13.4) | 718.1 (%23.4) | 441.2 (%14.4) | 87.7 (%2.9) | 248.0 (%8.1) |
| before/6 | 3081.6 | 197.7 (%6.4) | 1287.2 (%41.8) | 336.1 (%10.9) | 552.6 (%17.9) | 395.0 (%12.8) | 91.4 (%3.0) | 201.0 (%6.5) |
| before/7 | 7016.5 | 594.3 (%8.5) | 2853.7 (%40.7) | 926.2 (%13.2) | 1185.6 (%16.9) | 804.7 (%11.5) | 155.0 (%2.2) | 424.8 (%6.1) |
| before/8 | 2556.0 | 167.2 (%6.5) | 1004.6 (%39.3) | 291.1 (%11.4) | 463.9 (%18.1) | 349.2 (%13.7) | 101.5 (%4.0) | 159.7 (%6.2) |
| before/9 | 4666.0 | 366.8 (%7.9) | 1855.7 (%39.8) | 570.8 (%12.2) | 782.1 (%16.8) | 576.2 (%12.3) | 169.2 (%3.6) | 302.8 (%6.5) |
| before/10 | 3575.6 | 248.1 (%6.9) | 1747.0 (%48.9) | 372.8 (%10.4) | 505.6 (%14.1) | 384.6 (%10.8) | 110.1 (%3.1) | 188.5 (%5.3) |
| after/1 | 2009.1 | 251.0 (%12.5) | 0.0 (%0.0) | 388.2 (%19.3) | 750.9 (%37.4) | 390.8 (%19.5) | 43.1 (%2.1) | 139.3 (%6.9) |
| after/2 | 886.1 | 81.2 (%9.2) | 0.0 (%0.0) | 126.3 (%14.3) | 352.4 (%39.8) | 161.8 (%18.3) | 24.5 (%2.8) | 122.2 (%13.8) |
| after/3 | 1059.9 | 105.1 (%9.9) | 0.0 (%0.0) | 169.4 (%16.0) | 395.7 (%37.3) | 236.8 (%22.3) | 58.2 (%5.5) | 76.3 (%7.2) |
| after/4 | 1009.0 | 128.7 (%12.8) | 0.0 (%0.0) | 174.1 (%17.3) | 283.5 (%28.1) | 223.3 (%22.1) | 73.0 (%7.2) | 103.7 (%10.3) |
| after/5 | 542.4 | 58.3 (%10.7) | 0.0 (%0.0) | 105.5 (%19.4) | 140.5 (%25.9) | 124.0 (%22.9) | 34.8 (%6.4) | 66.6 (%12.3) |
| after/6 | 761.5 | 70.7 (%9.3) | 0.0 (%0.0) | 121.5 (%16.0) | 210.3 (%27.6) | 180.6 (%23.7) | 37.4 (%4.9) | 128.5 (%16.9) |
| after/7 | 853.9 | 109.5 (%12.8) | 0.0 (%0.0) | 165.8 (%19.4) | 229.9 (%26.9) | 196.4 (%23.0) | 59.1 (%6.9) | 80.6 (%9.4) |
| after/8 | 1352.1 | 181.1 (%13.4) | 0.0 (%0.0) | 223.5 (%16.5) | 431.3 (%31.9) | 283.6 (%21.0) | 93.6 (%6.9) | 119.6 (%8.8) |
| after/9 | 2400.9 | 314.4 (%13.1) | 0.0 (%0.0) | 467.5 (%19.5) | 776.7 (%32.4) | 429.8 (%17.9) | 149.9 (%6.2) | 226.1 (%9.4) |
| after/10 | 1716.0 | 245.3 (%14.3) | 0.0 (%0.0) | 323.6 (%18.9) | 556.5 (%32.4) | 311.9 (%18.2) | 76.1 (%4.4) | 160.8 (%9.4) |

## Toplam maliyet ve kalan engel

| Motor/run | Commit toplamı s | Bakım toplamı s | İkisi s | Tepe private / WS MiB |
|---|---:|---:|---:|---|
| reference/0 | 42.376 | 35.351 | 77.727 | 18.855 / 23.641 |
| reference/1 | 45.936 | 36.679 | 82.615 | 18.613 / 23.191 |
| candidate-fixed/0 | 46.009 | 13.950 | 59.959 | 19.805 / 24.312 |
| candidate-fixed/1 | 42.901 | 13.500 | 56.401 | 18.168 / 22.527 |
| sqlite/0 | 90.803 | 24.195 | 114.998 | 14.359 / 19.207 |
| sqlite/1 | 95.142 | 25.585 | 120.728 | 14.207 / 18.852 |

Son diagnostic profilin en pahalı aşamaları: `posting_remap_write` 4.127s (%32.78), `index_rewrite_open` 2.051s (%16.29), `source_copy_encode` 1.545s (%12.27), `source_validate_replay` 1.248s (%9.91). Bu mutlak süreler diagnostic derlemenin gözlemidir; normal derlemenin 12s sınırına birebir eklenmez.

Bakım sınırı geçmediği için çalışma burada durur; varsayılan kapalı kalır. Sonraki dar çalışma, bu baskın aşamaların içindeki tekrarları azaltmak olmalıdır. Snapshot API ve küçük commit/dirty open tasarımı bu tur uygulanmadı.

Son bakım pencerelerindeki sayaçlar: 29 posting stream, 82,638,240 giriş ID'si, 61,141,904 çıktı ID'si. Bunlar normal merge ve revision bakımının birlikte sayımıdır; ID'ler kaynak kopyası byte'ı değildir. Sayaçların nested süreleri aşama toplamına eklenmez. [Ayrı sayaç CSV](benchmarks/revisions-cost-10k-20261025-after-counters.csv).

Arama kapısı da kaldı. Onuncu tur sonunda kontrol 7 segment / 4.831MB ölü payload / 183 ölü kayıt; aday 8 segment / 52.637MB / 1951 ölü kayıt tutuyor (iki tekrarda aynı byte sonuçları). Daha seyrek rewrite'ın veri yerleşimi bedeli ölçüldü. Sorgu sürelerinin hangi iç aşamada arttığı bu tur profillenmedi; bütün artışı bu sayılara veya ölçüm dalgalanmasına kesin olarak atfetmiyoruz. Sonraki maliyet çalışmasında arama kapısı korunarak posting/indeks işini azaltmak, yalnız eşiği gevşetmekten daha uygun hedeftir.

## Normal tur sonuçları

| Motor/run/tur | Commit ms | Bakım ms | Retained MB / oran |
|---|---:|---:|---|
| reference/0/1 | 3702.8 | 2326.1 | 349.254 / 0.9967× |
| reference/0/2 | 4740.5 | 3570.6 | 350.195 / 0.9994× |
| reference/0/3 | 4773.3 | 3200.6 | 349.942 / 0.9987× |
| reference/0/4 | 4268.9 | 3260.7 | 350.295 / 0.9997× |
| reference/0/5 | 4182.1 | 2891.3 | 358.539 / 1.0232× |
| reference/0/6 | 4480.9 | 2913.7 | 351.207 / 1.0023× |
| reference/0/7 | 4213.1 | 6835.8 | 350.856 / 1.0013× |
| reference/0/8 | 4190.8 | 2416.4 | 350.740 / 1.0009× |
| reference/0/9 | 3696.3 | 4476.6 | 351.511 / 1.0031× |
| reference/0/10 | 4127.3 | 3459.0 | 357.954 / 1.0215× |
| candidate-fixed/0/1 | 6033.8 | 2455.2 | 349.254 / 0.9967× |
| candidate-fixed/0/2 | 4787.4 | 810.2 | 401.082 / 1.1446× |
| candidate-fixed/0/3 | 5253.5 | 1165.8 | 371.935 / 1.0614× |
| candidate-fixed/0/4 | 4758.9 | 1182.8 | 383.136 / 1.0934× |
| candidate-fixed/0/5 | 4398.8 | 815.9 | 402.704 / 1.1492× |
| candidate-fixed/0/6 | 4246.6 | 1029.2 | 406.555 / 1.1602× |
| candidate-fixed/0/7 | 4252.2 | 1234.7 | 417.954 / 1.1927× |
| candidate-fixed/0/8 | 4359.4 | 1339.9 | 417.759 / 1.1922× |
| candidate-fixed/0/9 | 4297.8 | 2325.7 | 407.932 / 1.1641× |
| candidate-fixed/0/10 | 3620.1 | 1590.8 | 420.644 / 1.2004× |
| sqlite/0/1 | 9411.2 | 2386.1 | 597.185 / 1.0219× |
| sqlite/0/2 | 9622.1 | 2405.7 | 602.604 / 1.0312× |
| sqlite/0/3 | 9594.4 | 2836.7 | 605.680 / 1.0364× |
| sqlite/0/4 | 10276.8 | 2891.7 | 609.071 / 1.0422× |
| sqlite/0/5 | 8568.5 | 1788.2 | 613.429 / 1.0497× |
| sqlite/0/6 | 8051.7 | 1875.5 | 617.443 / 1.0565× |
| sqlite/0/7 | 8010.0 | 1811.3 | 620.564 / 1.0619× |
| sqlite/0/8 | 8015.3 | 1908.8 | 623.157 / 1.0663× |
| sqlite/0/9 | 9052.6 | 3121.7 | 627.442 / 1.0737× |
| sqlite/0/10 | 10200.3 | 3169.3 | 630.485 / 1.0789× |
| reference/1/1 | 3884.4 | 2940.9 | 349.254 / 0.9967× |
| reference/1/2 | 6743.7 | 3528.9 | 350.195 / 0.9994× |
| reference/1/3 | 5557.8 | 3522.5 | 349.942 / 0.9987× |
| reference/1/4 | 4497.9 | 3360.1 | 350.295 / 0.9997× |
| reference/1/5 | 4479.9 | 2891.9 | 358.539 / 1.0232× |
| reference/1/6 | 4984.9 | 3022.3 | 351.207 / 1.0023× |
| reference/1/7 | 4044.4 | 6641.3 | 350.856 / 1.0013× |
| reference/1/8 | 4224.1 | 2633.6 | 350.740 / 1.0009× |
| reference/1/9 | 3836.9 | 4623.7 | 351.511 / 1.0031× |
| reference/1/10 | 3682.3 | 3514.1 | 357.954 / 1.0215× |
| candidate-fixed/1/1 | 4252.6 | 2121.6 | 349.254 / 0.9967× |
| candidate-fixed/1/2 | 4617.3 | 867.1 | 401.082 / 1.1446× |
| candidate-fixed/1/3 | 5203.4 | 968.1 | 371.935 / 1.0614× |
| candidate-fixed/1/4 | 4226.3 | 1137.6 | 383.136 / 1.0934× |
| candidate-fixed/1/5 | 4597.7 | 849.2 | 402.704 / 1.1492× |
| candidate-fixed/1/6 | 4146.4 | 1015.9 | 406.555 / 1.1602× |
| candidate-fixed/1/7 | 4342.2 | 1332.8 | 417.954 / 1.1927× |
| candidate-fixed/1/8 | 4218.3 | 1294.8 | 417.759 / 1.1922× |
| candidate-fixed/1/9 | 3659.7 | 2232.5 | 407.932 / 1.1641× |
| candidate-fixed/1/10 | 3637.6 | 1680.5 | 420.644 / 1.2004× |
| sqlite/1/1 | 9509.1 | 2399.3 | 597.185 / 1.0219× |
| sqlite/1/2 | 9656.6 | 2419.8 | 602.604 / 1.0312× |
| sqlite/1/3 | 8455.1 | 1677.5 | 605.680 / 1.0364× |
| sqlite/1/4 | 7688.9 | 1687.9 | 609.071 / 1.0422× |
| sqlite/1/5 | 7879.6 | 1842.2 | 613.429 / 1.0497× |
| sqlite/1/6 | 9771.1 | 2897.1 | 617.443 / 1.0565× |
| sqlite/1/7 | 10086.6 | 2894.4 | 620.564 / 1.0619× |
| sqlite/1/8 | 11067.7 | 3138.3 | 623.157 / 1.0663× |
| sqlite/1/9 | 10120.7 | 3406.0 | 627.442 / 1.0737× |
| sqlite/1/10 | 10906.9 | 3223.0 | 630.485 / 1.0789× |

## Otuz tur

| Tur | Bakım ms | Retained MB / oran |
|---|---:|---|
| 1 | 2050.7 | 349.254 / 0.9967× |
| 2 | 794.3 | 401.082 / 1.1446× |
| 3 | 979.9 | 371.935 / 1.0614× |
| 4 | 1130.2 | 383.136 / 1.0934× |
| 5 | 815.4 | 402.704 / 1.1492× |
| 6 | 1034.6 | 406.555 / 1.1602× |
| 7 | 1160.0 | 417.954 / 1.1927× |
| 8 | 1309.2 | 417.759 / 1.1922× |
| 9 | 2367.0 | 407.932 / 1.1641× |
| 10 | 1671.9 | 420.644 / 1.2004× |
| 11 | 2816.4 | 421.753 / 1.2036× |
| 12 | 2311.6 | 412.275 / 1.1765× |
| 13 | 2820.0 | 409.118 / 1.1675× |
| 14 | 2205.5 | 404.452 / 1.1542× |
| 15 | 1429.8 | 407.234 / 1.1622× |
| 16 | 1638.7 | 399.351 / 1.1397× |
| 17 | 1654.8 | 393.463 / 1.1229× |
| 18 | 1741.8 | 404.631 / 1.1547× |
| 19 | 1992.6 | 414.744 / 1.1836× |
| 20 | 1658.3 | 413.748 / 1.1807× |
| 21 | 2837.6 | 408.102 / 1.1646× |
| 22 | 1726.8 | 417.448 / 1.1913× |
| 23 | 1649.8 | 419.329 / 1.1967× |
| 24 | 2727.9 | 406.967 / 1.1614× |
| 25 | 2213.1 | 406.754 / 1.1608× |
| 26 | 2085.1 | 419.230 / 1.1964× |
| 27 | 2946.8 | 411.948 / 1.1756× |
| 28 | 2530.5 | 401.727 / 1.1464× |
| 29 | 2317.3 | 399.542 / 1.1402× |
| 30 | 1903.5 | 400.074 / 1.1417× |

5ms örneklenmiş geçici retained tepe: iki normal 10 turda 526.028MB, 30 turda 530.267MB. Bunlar örneklenen en yüksek dosya toplamıdır; kısa sıçrama veya sürekli yazıcı altında sınırsız iptal birikimi için kesin üst sınır değildir.

## Bütün 38 sorgu / yük

| Case / sorgu / offset / limit | Ref p95 ms | Aday p95 ms | Fark | Yük / idle p95 ms | Yük p95/idle | Yük max ms |
|---|---:|---:|---:|---:|---:|---:|
| 0 / quixote / 0 / 50 | 3.563100 | 3.262100 | %-8.45 | 4.962 / 4.792 | 1.035× | 5.042 |
| 1 / zeuglodon / 0 / 50 | 2.551600 | 2.565700 | %+0.55 | 4.050 / 2.885 | 1.404× | 4.106 |
| 2 / beagle / 0 / 50 | 29.098000 | 31.961900 | %+9.84 | 38.181 / 29.678 | 1.287× | 39.463 |
| 3 / mesopotamia / 0 / 50 | 3.904500 | 3.774800 | %-3.32 | 4.690 / 9.393 | 0.499× | 4.830 |
| 4 / transubstantiation / 0 / 50 | 60.967800 | 63.917400 | %+4.84 | 79.069 / 73.487 | 1.076× | 81.667 |
| 5 / harpoon / 0 / 50 | 75.567200 | 78.803700 | %+4.28 | 96.943 / 93.893 | 1.032× | 102.963 |
| 6 / ishmael / 0 / 50 | 9.088800 | 8.867600 | %-2.43 | 12.200 / 11.523 | 1.059× | 15.061 |
| 7 / invertebrata / 0 / 50 | 15.298700 | 13.455400 | %-12.05 | 16.069 / 15.901 | 1.011× | 17.299 |
| 8 / darcy / 0 / 50 | 56.110000 | 70.423600 | %+25.51 | 71.933 / 68.973 | 1.043× | 77.783 |
| 9 / elizabeth / 0 / 50 | 52.603500 | 55.706500 | %+5.90 | 68.235 / 65.590 | 1.040× | 70.189 |
| 10 / watson / 0 / 50 | 58.426400 | 55.124900 | %-5.65 | 59.868 / 66.026 | 0.907× | 101.908 |
| 11 / holmes / 0 / 50 | 31.734500 | 34.124400 | %+7.53 | 36.803 / 35.495 | 1.037× | 39.995 |
| 12 / frankenstein / 0 / 50 | 16.219700 | 18.236500 | %+12.43 | 21.013 / 19.882 | 1.057× | 22.809 |
| 13 / galapagos / 0 / 50 | 10.985200 | 12.348200 | %+12.41 | 15.141 / 13.261 | 1.142× | 15.237 |
| 14 / fossil / 0 / 50 | 35.184000 | 38.219700 | %+8.63 | 41.209 / 40.303 | 1.022× | 41.985 |
| 15 / geologist / 0 / 50 | 21.220000 | 25.442700 | %+19.90 | 26.533 / 27.166 | 0.977× | 39.893 |
| 16 / the / 0 / 50 | 1.557700 | 1.696800 | %+8.93 | 2.230 / 1.860 | 1.199× | 2.366 |
| 17 / man / 0 / 50 | 1.800200 | 1.856700 | %+3.14 | 2.151 / 2.042 | 1.053× | 2.159 |
| 18 / he / 0 / 50 | 1.231400 | 1.530800 | %+24.31 | 2.139 / 1.724 | 1.240× | 2.198 |
| 19 / zzxqnever / 0 / 50 | 0.019400 | 0.047100 | %+142.78 | 0.173 / 0.120 | 1.438× | 0.233 |
| 20 / zx / 0 / 50 | 168.020900 | 183.394800 | %+9.15 | 217.130 / 221.578 | 0.980× | 225.141 |
| 21 / the zx / 0 / 50 | 161.313800 | 180.807600 | %+12.08 | 220.875 / 209.406 | 1.055× | 262.769 |
| 22 / quixote whale / 0 / 50 | 3.114000 | 6.144300 | %+97.31 | 4.564 / 3.570 | 1.279× | 4.858 |
| 23 / holmes elementary / 0 / 50 | 42.028400 | 53.342000 | %+26.92 | 53.793 / 57.461 | 0.936× | 55.862 |
| 24 / fossil geologist / 0 / 50 | 13.079600 | 15.753800 | %+20.45 | 18.370 / 14.932 | 1.230× | 18.646 |
| 25 / whale harpoon / 0 / 50 | 68.423000 | 78.760500 | %+15.11 | 82.072 / 75.590 | 1.086× | 96.208 |
| 26 / QUİXOTE / 0 / 50 | 2.838900 | 3.335200 | %+17.48 | 4.590 / 3.577 | 1.283× | 5.020 |
| 27 / İSHMAEL / 0 / 50 | 8.925100 | 10.295800 | %+15.36 | 10.943 / 10.248 | 1.068× | 11.678 |
| 28 / zx × 150 / 0 / 50 | 0.031000 | 0.047400 | %+52.90 | 0.134 / 0.117 | 1.146× | 0.144 |
| 29 / presumption / 0 / 50 | 87.400900 | 90.288400 | %+3.30 | 120.041 / 112.660 | 1.066× | 139.523 |
| 30 / the / 0 / 1 | 0.190800 | 0.267800 | %+40.36 | 0.770 / 0.754 | 1.021× | 0.775 |
| 31 / the / 0 / 200 | 7.065300 | 6.929700 | %-1.92 | 8.534 / 7.308 | 1.168× | 9.145 |
| 32 / the / 5000 / 50 | 2.142500 | 2.433400 | %+13.58 | 10.837 / 9.374 | 1.156× | 11.206 |
| 33 / the / 9950 / 50 | 1.965700 | 2.194600 | %+11.64 | 15.497 / 14.717 | 1.053× | 16.558 |
| 34 (izlenen) / ishmael / 130 / 1 | 7.434400 | 7.645100 | %+2.83 | 15.524 / 10.125 | 1.533× | 23.785 |
| 35 / ishmael / 60 / 200 | 17.537000 | 15.751900 | %-10.18 | 22.946 / 18.454 | 1.243× | 23.666 |
| 36 / quixote / 28 / 10 | 2.087000 | 1.879900 | %-9.92 | 3.234 / 2.790 | 1.159× | 3.720 |
| 37 / quixote / 57 / 50 | 1.592300 | 1.767100 | %+10.98 | 3.099 / 2.635 | 1.176× | 3.856 |

## Kalibrasyon ve başarısız denemeler

Aşağıdakiler final gate örneklerine katılmadı. Eksik koşunun toplamı tamamlanmış 10 tur süresi değildir. İlk load smoke yazı p95/max66,784/72,717ms idi; soğuk-idle/sıcak-load okuma kıyası geçersiz sayıldı. Ön kalibrasyon sürelerindeki farklar ortam izolasyonu olmadan bütün oturumlara aynı mutlak performans sözü verilemeyeceğini de gösterir.

| Ham deneme | Tur | Bakım s | Son disk MB | Exit | Gate dışında kalma nedeni |
|---|---:|---:|---:|---|---|
| [compare-fixed-r0-candidate-fixed-streamed-revisions-10](benchmarks/revisions-cost-10k-20261025-compare-fixed-r0-candidate-fixed-streamed-revisions-10.jsonl) | 10 | 31.382 | 420.644 | 0 | v5 flag düzeltmesi hatalı; final gate dışında |
| [compare-fixed-r0-reference-streamed-revisions-10](benchmarks/revisions-cost-10k-20261025-compare-fixed-r0-reference-streamed-revisions-10.jsonl) | 10 | 35.525 | 357.954 | 0 | v5 flag düzeltmesi hatalı; final gate dışında |
| [compare-r0-reference-streamed-revisions-10](benchmarks/revisions-cost-10k-20261025-compare-r0-reference-streamed-revisions-10.jsonl) | 8 | 24.951 | 350.740 | yarıda durdu | Warm-idle/load eşleme düzeltmesi için durduruldu; eksik koşu |
| [compare-v2-r0-candidate-streamed-revisions-10](benchmarks/revisions-cost-10k-20261025-compare-v2-r0-candidate-streamed-revisions-10.jsonl) | 10 | 25.268 | 420.644 | 0 | v4 index_terms=true; final gate dışında |
| [compare-v2-r0-reference-streamed-revisions-10](benchmarks/revisions-cost-10k-20261025-compare-v2-r0-reference-streamed-revisions-10.jsonl) | 10 | 29.741 | 357.954 | 0 | Ön kalibrasyon referansı; final dengeli çift yerine kullanılmadı |
| [compare-v2-r1-candidate-streamed-revisions-10](benchmarks/revisions-cost-10k-20261025-compare-v2-r1-candidate-streamed-revisions-10.jsonl) | 10 | 30.882 | 420.644 | 0 | v4 index_terms=true; final gate dışında |
| [pilot-r0-pilot-streamed-churn](benchmarks/revisions-cost-10k-20261025-pilot-r0-pilot-streamed-churn.jsonl) | 10 | 11.771 | 420.644 | 0 | Tek pilot; dengeli tekrar ve yeni kapı kapsamı yok |

## Kanıt ve açık işler

- [Ham özet](benchmarks/revisions-cost-10k-20261025-summary.json), [tur CSV](benchmarks/revisions-cost-10k-20261025-rounds.csv), [yazı örnekleri](benchmarks/revisions-cost-10k-20261025-writes.csv), [38-query CSV](benchmarks/revisions-cost-10k-20261025-queries.csv), [yük CSV](benchmarks/revisions-cost-10k-20261025-reads.csv).
- [Eski kaynak, retirement/cancel/flush/merge/cache process-kill](benchmarks/revisions-cost-kill-10k-20261025-summary.json).
- [Komut/test kayıtları](benchmarks/revisions-cost-10k-20261025-finish.json).
- `cargo fmt --check`, default ve diagnostic `cargo clippy --all-targets -- -D warnings`, `cargo test`, no-default lib ve workload testleri geçti. [Bağımsız kanıt kontrolü](benchmarks/revisions-cost-10k-20261025-proof.json) corpus/fixture SHA, frozen program ve örnek sayılarını doğrular.
- Yeni emeklilik/iptal senaryoları akışlı API'yi kullanır. Önceden var olan dört genel kill modu için CLI etiketi owned'dır: iterator modu zaten streaming iterator'ünü, flush/merge modları owned delta'nın açık flush/yayım yolunu keser. Bunlar owned performans sonucu veya bütün responsive rewrite evrelerinin kill kapsamı değildir.
- İlk `kill-retire-after` hazırlığı otomatik merge nedeniyle tam ölü hedef bırakmadı ve barrier öncesi assertion ile durdu; [stderr](benchmarks/revisions-cost-kill-10k-20261025-kill-retire-after.stderr) ve başarısız komut kaydı korundu. Hazırlık normal merge'leri önce boşaltacak şekilde düzeltildi; `retry1` ayrı logla doğrulandı. Bu ilk deneme kill kapsamı olarak sayılmadı.
- [OPEN-WORK](OPEN-WORK.md).

Case34 kullanıcı kararıyla izlenir. Maliyet kapısı geçmeden snapshot/çok okuyucu veya küçük commit/dirty open adımına geçilmez. C ABI yapılmadı. Genel RAM, gerçek-soğuk, uzun vadeli tombstone ve kill sonrası input/build artıklarının temizliği önceki açık işler olarak kalır.
