# İki işçili strict-open — SEG05 ve MET04 ayrımı

**KANIT.** Önceki yaklaşık 174 ms duvar kapsamlı bölümün en pahalı işi **SEG05 terim/posting döngüsü**. İki işçide biriken p50 süre **212,42 ms**, alt bölümün **%83,0’ı**. MET04 üst dizin ve yaprak okuma, CRC ve sayfa kontrolleri toplam yaklaşık **5,32 ms (%2,1)**. MET04 kayıt karşılaştırması ve slot muhasebesi **8,81 ms (%3,45)**. Bu değerler işçilerde biriken duvar süreleridir. Açılış gecikmesine doğrudan eklenemez. [Önceki run fazları](STRICT-OPEN-RUN-PHASES-20261110.md)

## Ölçüm düzeni ve sayaç maliyeti

Aynı 52 run’lı 50k Cap200k TR kopyası, 16 aktif delta anahtarı, 159 dosya. Tek release ikilisi, iki işçi. Hafif sayaç ve ayrıntılı sayaç 10 ABBA/BAAB blok, kol başına 20 yeni proses. Her blok öncesi cached I/O ile ısıtma, WPR/ETW/purge yok. Pencere 20:45:53–20:47:10 UTC, deadline 21:10:53 UTC. p95, 20 örnekte en yakın üst sıra olan 19. gözlem. 40/40 strict Verified, case0 `olduğu`, melez N=50, 50 hit, CRC32 725765487, sequence 529967. Aktif slot, kopya ve model SHA’ları aynı. Yeni korpus, yeni DB kopyası veya motor kodu değişikliği yok.

İlk hazırlıkta süre kontrolü her run’ın terim döngüsünün pozitif zaman alacağını varsaydı. 52 run’ın 12’sinde terim sayısı sıfır. Bu varsayım ilk bloğu durdurdu. Yarım blok ve tek tanı açılışı ayrı tutuldu, süre tavanı korundu ve kalan bütçeyle 10 dengeli blok alındı. [İlk kontrol kaydı](benchmarks/strict-open-met04-phases-20261110/setup-status.json) · [SEG05 dizin sayaçları](benchmarks/strict-open-met04-phases-20261110/seg05-directory-counts.json) · [40 zamanlı örnek](benchmarks/strict-open-met04-phases-20261110/samples.jsonl)

| Ölçüm | Hafif sayaç p50 / p95 | Ayrıntılı sayaç p50 / p95 |
| --- | ---: | ---: |
| Strict-open, ms | 430,97 / 439,77 | 431,12 / 446,68 |
| Açılış → ilk sonuç, ms | 527,33 / 539,19 | 530,72 / 544,68 |
| Açılış CPU, ms | 687,50 / 718,75 | 703,13 / 734,38 |
| Açılış tepe private, MiB | 8,271 / 9,105 | 8,270 / 9,223 |
| SEG05/MET04 bölümü, işçi toplamı ms | 251,33 / 264,34 | 256,42 / 277,36 |
| SEG05/MET04 bölümü, duvar kapsamı ms | 173,18 / 179,95 | 174,67 / 187,97 |

Ayrıntılı sayacın eşleşmiş blok medyanı açılışta **+2,19 ms**, aralık −0,38…+14,66 ms. Ölçülen bölümün işçi toplamı için medyan fark **+4,05 ms**. Ayrıntılı kol her `Validator::push` çağrısını saatliyor. SEG05 kayıt döngüsünden bu çağrı süresi çıkarıldığında sayaç çağrısının bir miktar bedeli dışarıda kalan süreye karışabilir. Üretim süresi için hafif kol esas alınır. [Ham farklar ve dağılımlar](benchmarks/strict-open-met04-phases-20261110/summary.json)

## İşin ayrımı

52 run’ın dizin başlıklarında **789.189 terim/posting girdisi** ve **82.072 fiziksel kayıt** var. Ayrıntılı kolda her açılış 82.072 kayıt callback’i, 86 üst dizin ve 1.316 yaprak işledi. Aşağıdaki p50/p95 işçilerde biriken süredir. Oranlar örnek başına payların medyanıdır. İki işçinin aynı anda yaptığı alt işler bu tabloda ayrı duvar kapsamı olarak ölçülmedi.

| Aşama | İşçi toplamı p50 / p95 (ms) | Bölümde pay p50 |
| --- | ---: | ---: |
| **SEG05 terim/posting döngüsü** | **212,42 / 228,44** | **%83,01** |
| SEG05 kayıt döngüsü, MET04 callback’i dışında | 27,06 / 32,01 | %10,56 |
| MET04 üst dizin okuma, CRC, decode ve kontrol | 0,96 / 1,08 | |
| MET04 yaprak okuma, CRC ve sayfa sınırları | 4,36 / 5,28 | %2,08, iki sayfa satırı toplamı |
| MET04 slot sayımı, sayısal alanlar ve anahtar eşitliği | 8,81 / 10,00 | %3,45 |
| Parse öncesi/sonrası kalan | 2,22 / 2,54 | |

MET04 sayfa kontrolleri **tam MET04 önek CRC’sini** içermez. O CRC ve kök dizin yüklemesi önceki fazda ayrı tutuldu. `numeric_metadata` ve `compare_leaf_key` aynı sayfa başı ve slot konumunu ayrı çözüyor. Bu tekrar mevcut, ancak 8,81 ms’lik MET04 karşılaştırmasının yalnız bir parçası. İlk açılış hedefi olarak terim döngüsü daha büyük. [SEG05 terim döngüsü](../src/segment/compact.rs#L310) · [MET04 kayıt karşılaştırması](../src/segment/paged/streaming.rs#L118)

**BİLİNMİYOR.** 212,42 ms içindeki `Input::posting` decode ve UTF-8/alloc, posting sırası ve sınır kontrolü, Bloom/özet, `push_fence` payları. Kodda `push_fence` sayfanın son terimini her girdide klonluyor. Önce bu payları seyrek örnekleyerek ayırmak, kopyalama baskınsa son terimi yalnız sayfa kapanışında materyalleştiren dar bir aday denemek mantıklı. Sıra, UTF-8, posting offset/count, Bloom ve fence eşitliği korunmalı. Bu rapor herhangi bir optimizasyonun kazancını ölçmedi. [Terim çözümü](../src/segment/compact.rs#L205) · [Sayfa sınırı hazırlığı](../src/segment/term_pages.rs#L66)

Geçici derleme `C:\Users\Serkan\AppData\Local\Temp\fdb-open-met04-phase-20261110`, 153.487.117 B (146,38 MiB). Hedefli Clippy ve Rust biçimleme geçti. [Faz özeti](benchmarks/strict-open-met04-phases-20261110/summary.json) · [Sayaç farkı](benchmarks/strict-open-met04-phases-20261110/compact.diff.txt)
