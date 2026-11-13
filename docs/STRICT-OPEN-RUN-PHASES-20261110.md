# İki işçili strict-open — run doğrulama aşamaları

**KANIT, 52 run sıcak yerleşim için.** Run yükleme p50 **265,08 ms**. En büyük alt aşama **SEG05 dizin akışı ile MET04 sayfa/kayıt karşılaştırması**. İki işçide biriken süre p50 **252,39 ms**, iki işçiden en az birinin o aşamada bulunduğu duvar saati p50 **174,29 ms**. MET04 ön açılışı da eklenince **177,18 ms** duvar kapsamı. Kaynak CRC/belge kontrolü **111,94 ms**, indeksin tam CRC’si **33,52 ms** duvar kapsamı. Bu kapsamlar birbirleriyle örtüşebilir.

## Ölçüm ve sayaç maliyeti

52 run, 16 delta anahtarı ve 50.000 Cap200k TR belge. Aynı 159 dosyalı kopya, generation 8185, sequence 529967. 12 ABBA/BAAB blokta 24 sayaç kapalı ve 24 sayaç açık yeni proses. İki kolda aynı release ikilisi ve üretimin iki işçili strict yolu. Sayaç açık 24 örneğin her birinde 52 run, 52 farklı kimlik ve iki ayrı iş parçacığı gözlendi. Kopya blok öncesi normal cached I/O ile ısıtıldı. WPR, ETW ve purge kullanılmadı. Pencere **20:39:58–20:40:29 UTC**, 25 dakika üst sınır. [Ölçüm planı](benchmarks/strict-open-run-phases-20261110/plan.json)

Run başına sekiz çakışmayan zaman aralığı. İşçi yerel kayıtları run sonunda önceden ayrılmış en fazla 64 satırlık dizide toplanır. Record başına zamanlayıcı yok. p95 en yakın üst sırayla hesaplandı. Ölçüm açılış ve `olduğu` melez N=50 ilk sorgusunu kapsıyor. 48/48 strict Verified, 50 hit, CRC32 725765487. Aktif slot ve kopyanın 159 dosyası ile model SHA-256 eşit. Üretim kodu, biçim ve varsayılanlar aynı. Geçici derlemede `rustfmt` ve hedefli Clippy geçti. İlk hazırlıkta iki `read_with` çağrısı aynı ekleme metnini eşledi, hedef yalnız `existing` ile sınırlandı. Bu aşamada örnek alınmadı. Geçici klasör 153.311.690 B (146,21 MiB). [Ham 48 örnek](benchmarks/strict-open-run-phases-20261110/samples.jsonl) · [Ölçüm sonucu](benchmarks/strict-open-run-phases-20261110/status.json)

| Ölçüm | Sayaç kapalı p50 / p95 | Sayaç açık p50 / p95 | Eşleşmiş blok farkı medyanı |
| --- | ---: | ---: | ---: |
| Strict-open (ms) | 430,69 / 466,23 | 430,99 / 458,08 | +1,38 ms |
| Açılış → ilk sonuç (ms) | 529,88 / 567,32 | 530,09 / 565,62 | +0,66 ms |
| Açılış süreç CPU (ms) | 695,31 / 750,00 | 703,13 / 718,75 | 0,00 ms |
| Açılış tepe private (MiB) | 8,289 / 8,617 | 8,379 / 8,766 | +0,002 MiB |
| İlk sonuca kadar tepe private (MiB) | 9,582 / 9,738 | 9,682 / 9,836 | +0,122 MiB |

Strict-open eşleşmiş farkı −18,44…+13,61 ms. Bu dağılım sayaçların küçük maliyetini tek başına ayırmaz. Önceki ağır profildeki yüzlerce ms fark bu düzende görülmedi. [Blok farkları](benchmarks/strict-open-run-phases-20261110/summary.json)

## Run yükleme aşamaları

İşçi toplamı 52 run’da iki işçide geçen sürelerin toplamıdır, CPU sayacı değildir. Duvar kapsamı aynı aşamanın bütün işçilerdeki aralıklarının birleşimidir. Farklı aşamaların duvar kapsamları toplanamaz. Tüm run yükleme duvarı **265,08 / 281,12 ms** p50/p95. Run işçi toplamı **527,26 / 559,80 ms**. Yükleme sonrası ayrı görünürlük maskesi **161,87 / 174,34 ms**.

| Aşama | İşçi toplamı p50 / p95 (ms) | Duvar kapsamı p50 / p95 (ms) |
| --- | ---: | ---: |
| Kaynak handle ve boyut | 51,32 / 56,89 | 49,14 / 54,91 |
| Native kaynak CRC, belge ve sequence | 150,78 / 166,05 | 111,94 / 122,27 |
| SEG05 handle, başlık, footer, bağlama | 4,35 / 5,01 | 4,32 / 4,72 |
| Tam indeks CRC | 43,10 / 45,89 | 33,52 / 35,53 |
| MET04 açma, CRC ve kök dizini | 6,48 / 7,32 | 6,36 / 7,05 |
| **SEG05 parse + MET04 sayfa/kayıt karşılaştırması** | **252,39 / 262,58** | **174,29 / 182,36** |
| Terim sayfası hazırlığı | 16,38 / 17,32 | 14,70 / 16,83 |
| Son kaynak/indeks kimliği ve bind | 0,86 / 1,00 | 0,86 / 1,00 |

**KANIT.** MET04 açılışı ile kayıt karşılaştırmasının birlikte işçi toplamı medyanı 258,58 ms, bütün run işçi zamanının yaklaşık %49’u. Bu, ayrılan en büyük iş alanı. **BİLİNMİYOR.** 252,39 ms içindeki SEG05 parse, MET04 sayfa okuma ve kayıt karşılaştırmasının ayrı payları. [Sayaç yerleşimi](benchmarks/strict-open-run-phases-20261110/met04.diff.txt) · [Ölçüm verisi](benchmarks/strict-open-run-phases-20261110/summary.json)

## Tekrar okunan alanlar ve sıradaki aday

**KANIT, mantıksal bayt aralığı.** Tam SEG05 CRC, toplam 175.419.388 B indeks önekini okuyor. İçindeki **35.732.368 B dizin** sonradan doğrulama akışında yeniden okunuyor. MET04 önek CRC’si **8.237.705 B** kapsıyor. Sonrasında sayfalar, üst dizinler ve kök dizini toplam **8.236.041 B** yeniden okunuyor. Bu iki doğrulama yolunda toplam **41,93 MiB** aynı mantıksal aralık ikinci kez okunuyor. Native kaynak geçişi 283.245.856 B ve kendi tam CRC ile belge kontrolünü bir geçişte yapıyor. Bu sayılar kodun doğruladığı dosya aralıklarıdır. Fiziksel disk trafiği ve ikinci okuma maliyeti ayrı ölçülmedi. [Tam indeks CRC ve dizin](../src/segment.rs#L1287) · [MET04 açma ve doğrulama](../src/segment/paged/streaming.rs#L16) · [Native geçiş](../src/format/native_check.rs#L7)

**Öneri.** Önce 252,39 ms’lik akışta MET04 sayfa okuma ve çözmeyi, SEG05 parse ile kayıt kimliği karşılaştırmasının kalan işinden sayfa düzeyinde ayırmak. Yük sayfa erişiminde yoğunlaşırsa, doğrulanmış sayfayı sıralı okuyucu boyunca yeniden kullanmak ölçülecek aday olur. Parse ve karşılaştırma baskınsa, tam CRC’yi ikinci okumayla birleştirmek tek başına ana maliyeti çözmeyebilir. Her aday için strict CRC, key/version/offset/checksum ve sıra eşitliği ile bozuk cache fallback’i korunmalı. Bu turda optimizasyon uygulanmadı.
