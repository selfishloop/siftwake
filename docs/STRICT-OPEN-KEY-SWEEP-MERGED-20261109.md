# Strict-open — sıralı anahtar taramasının motora alınması

**KANIT.** 52 run’lı 50k yerleşimde sıcak strict-open p50 **1.322,84 → 697,11 ms** (%47,3). Açılış ve açılış dahil ilk sonuç 12/12 eşleşmiş blokta iyileşti. Temiz 50k ve 100k açılış sürelerinde tutarlı kötüleşme görülmedi. **100k açılış tepe private p95 4,121 → 4,270 MiB** ve blok farkı 12/12 aynı yönde. Bu fark maske çağrısından önce vardı. RAM sonucunu kullanıcı kararına bırakarak ayrı salt-okuma incelemesinden önce duruldu.

**Sonraki kullanıcı kararı.** Ölçülen 100k RAM farkı kabul edildi. Farkın nedeni bilinmiyor. [Kalan aşamaların salt-okuma incelemesi](STRICT-OPEN-REMAINING-PHASES-20261109.md)

## Motor ve doğruluk

- **KANIT.** Açılışta örtüşen run aralıklarının sıralı anahtarları heap ile bir kez gezilir. Ayrık aralıkların anahtarları taranmaz. Aynı anahtarın eski slotları, en yeni slot tombstone olsa da gizlenir. MaskCache manifest sırasıyla ve WAL shadow öncesi doldurulur. Ardından mevcut shadow, görünür sayım, source CRC, indeks CRC ve kaynak kimliği kontrolleri aynı sırayla uygulanır. Kalıcı biçim, seçeneklerin varsayılan değerleri ve kapı eşikleri aynı. [Uygulama](../src/engine/open_masks.rs)
- **KANIT.** Eski `hide_keys` açılış algoritması test referansı. İki deterministik rastgele seed, 1–60 run’ın her sayısı, tekrarlanan anahtarlar, 64 slotlu yaprak sınırları, boş ve tek slotlu run, tombstone ve WAL shadow var/yok test edildi. Shadow öncesi ve sonrası bitler, run görünür sayıları ve MaskCache’in sıra, içerik ve bayt hesabı eşit. [Property testi](../src/engine/open_masks/tests.rs)
- **KANIT.** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` ve `cargo test --all-targets` geçti. 259 test geçti, 15 ignore. Kaynak bozulması, 0.1 fixture ve yazı sırasında process-kill testleri pakete dahil. Birleşik release okuyucusunu 50k oracle bariyerinde sonlandırıp yeniden açınca 50.000 GET/version ve 41 digest eşit çıktı. Ayrı dondurulmuş okuyucu (SHA `B7845966…`) da aynı oracle’ı geçti. 159 sabit slot dosyası ve model SHA değişmedi. [Test](benchmarks/strict-open-key-sweep-merged-20261109/final-test.log) · [Kill/reopen ve eski okuyucu](benchmarks/strict-open-key-sweep-merged-20261109/verification.json)

## Sıcak ölçüm

**KANIT, bu yerleşimler için.** 17:53:06–18:00:11 UTC, **7 dk 5 sn**, 25 dk üst sınır içinde. 52 run kopyası önceki SHA envanteriyle aynı. Temiz veriler, aynı `881756DA…` korpusunun 50k ve 100k önekleriyle, tarihsel akışlı ingest ikilisi kullanılarak tek geçici benchmark slotunda yeniden kuruldu. 50k’da tarihsel temiz tabanla aynı **3 run**, 100k’da **7 run** oluştu. Eski 310 ms tabanı 3 run’a aittir. Canlı slot korunarak geçici slot yeniden kullanıldı ve deney sonunda boşaltıldı.

Aynı ölçüm ikilisinde eski seri maske ve birleşik aday, her gerçek yerleşimde **12 ABBA/BAAB blok, kol başına 24 yeni proses**. Her bloktan önce mevcut dosyalar normal cached I/O ile okundu. `olduğu`, melez N=50, strict `Verified`. Her örneğin sonuç digest’i, sequence’i ve görünür belge sayısı kendi yerleşiminde eşit. WPR, ETW ve purge kullanılmadı. p95 en yakın üst sıra. CPU süreç user+kernel. RAM süreç private commit ve working set sayaçları, OS dosya cache’i hariç. İlk sonuç saati `Database::open` başlangıcından sorgu dönüşüne, oracle digest’i saat dışında.

| Yerleşim | Ölçüm | Seri p50 / p95 | Aday p50 / p95 |
| --- | --- | ---: | ---: |
| 52 run, 50k | Strict-open ms | 1.322,84 / 1.641,00 | **697,11 / 829,80** |
| | Maske ms | 762,46 / 935,17 | **163,18 / 241,23** |
| | Açılış → ilk sonuç ms | 1.422,05 / 1.746,19 | **798,89 / 955,30** |
| | CPU açılış / maske / ilk sonuç, ms | 1.296,88/1.546,88 · 757,81/906,25 · 1.406,25/1.640,63 | 687,50/812,50 · 171,88/218,75 · 781,25/921,88 |
| | Tepe private, açılış / ilk sonuç, MiB | 7,723/8,090 · 9,328/9,637 | 8,174/8,211 · 9,676/9,680 |
| Temiz 50k, 3 run | Strict-open ms | 276,58 / 320,64 | 279,80 / 301,70 |
| | Maske ms | 0,011 / 0,013 | 0,019 / 0,029 |
| | Açılış → ilk sonuç ms | 279,14 / 323,17 | 283,26 / 304,33 |
| | CPU açılış / maske / ilk sonuç, ms | 281,25/312,50 · 0/0 · 281,25/312,50 | 273,44/296,88 · 0/0 · 281,25/312,50 |
| | Tepe private, açılış / ilk sonuç, MiB | 2,586/2,609 · 3,379/3,387 | 2,574/2,594 · 3,391/3,398 |
| Temiz 100k, 7 run | Strict-open ms | 578,31 / 630,31 | 572,93 / 629,53 |
| | Maske ms | 0,015 / 0,022 | 0,025 / 0,034 |
| | Açılış → ilk sonuç ms | 581,71 / 633,66 | 576,13 / 632,93 |
| | CPU açılış / maske / ilk sonuç, ms | 578,13/609,38 · 0/0 · 578,13/625,00 | 570,31/609,38 · 0/0 · 578,13/609,38 |
| | Tepe private, açılış / ilk sonuç, MiB | 3,889/4,121 · 5,176/5,242 | 4,160/4,270 · 5,230/5,242 |

**SİNYAL.** Temiz 50k açılışta aday 6/12 blok daha hızlı, 100k’da 7/12. 100k en uzun açılış **630,35 → 665,80 ms**. Temiz 100k’da eşleşmiş açılış tepe private farkı medyan **+0,211 MiB**, min–max **+0,035…+0,473 MiB**. İlk sonuca kadar p95 eşit. 52 run’da açılış tepe private p95 **+0,121 MiB**, açılış dahil ilk sonuç p95 **+0,043 MiB**. İki yerleşimde de açılış tepe private farkı maske çağrısından önce oluştu. Maske boyunca süreç tepe sayacı yeni bir zirve kaydetmedi. **BİLİNMİYOR.** Kollar arasındaki maske öncesi tepe farkının nedeni ve heap’in bu farktaki payı. Sıcak ölçümler gerçek-soğuk kapısını ölçmez. [52 run ham ve özet](benchmarks/strict-open-key-sweep-merged-20261109/churn52-summary.json) · [50k özet](benchmarks/strict-open-key-sweep-merged-20261109/clean-50000-summary.json) · [100k özet](benchmarks/strict-open-key-sweep-merged-20261109/clean-100000-summary.json)

## Sentetik run sayısı

**KANIT, sentetik sınır için.** Her yerleşimde 82.000 fiziksel slot, 64 ortak anahtar, iç içe anahtar aralıkları ve tombstone. Run sayısı değiştikçe görünür belge sayısı da değişiyor. Yalnız birleşik yol, 10 sıcak proses/yerleşim. N=10’da p95 maksimumdur. Maske penceresinde tepe artışı tüm sürecin high-water farkıdır ve heap tahsislerini ayrı sınıflandırmaz. Açılış tepesinde run/index metadata ve dosya handle maliyetleri de vardır. Sentetik metinler yalnız yerleşim fikstürüne ait, gerçek korpusun yerine kullanılmadı.

| Run | Açılış / maske / ilk sonuç wall p50/p95, ms | Açılış / maske / ilk sonuç CPU p50/p95, ms | Açılış / ilk sonuç tepe private p50/p95, MiB | Maske sırasında tepe artışı p50/p95, MiB |
| ---: | ---: | ---: | ---: | ---: |
| 52 | 724,93/819,23 · 583,36/668,74 · 729,76/823,35 | 718,75/828,13 · 578,13/671,88 · 718,75/828,13 | 5,117/5,121 · 7,754/7,758 | 0,395/0,441 |
| 200 | 1.183,37/1.482,81 · 779,17/1.060,39 · 1.197,38/1.497,54 | 1.179,69/1.437,50 · 773,44/1.015,63 · 1.195,31/1.453,13 | 15,203/15,207 · 19,359/19,379 | 0,557/0,746 |
| 500 | 1.547,24/1.755,98 · 836,28/969,25 · **9.281,52/10.416,32** | 1.515,63/1.734,38 · 828,13/953,13 · 8.984,38/10.171,88 | 35,332/35,340 · 42,887/42,969 | 0,754/0,758 |

500 run’da ilk sorgu tek başına p95 **8.840,87 ms**. **BİLİNMİYOR.** Bu yerleşimde sorgu süresinin bileşenleri. Seri sentetik karşılaştırması veya gerçek 500 run ölçümü yapılmadı. [Sentetik 52](benchmarks/strict-open-key-sweep-merged-20261109/synthetic-52-summary.json) · [200](benchmarks/strict-open-key-sweep-merged-20261109/synthetic-200-summary.json) · [500](benchmarks/strict-open-key-sweep-merged-20261109/synthetic-500-summary.json)

`C:\Users\Serkan\AppData\Local\Temp\fdb-open-sweep-merged-20261109` derleme klasörü saklandı. Toplam **1.962.800.524 B (1,83 GiB)**, yaklaşık 1,80 GB’ı kopyalanan araç ağacında. Önceki 52 run kopyası ayrıca korundu. [Koşu durumu ve süre](benchmarks/strict-open-key-sweep-merged-20261109/run-status.json) · [Ham örnek dizini](benchmarks/strict-open-key-sweep-merged-20261109/)
