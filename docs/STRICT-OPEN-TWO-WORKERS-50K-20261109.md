# Strict-open — iki işçili run doğrulaması

**KANIT.** Aynı 52 run’lı 50k kopyada iki işçi strict-open p50’yi **701,72 → 444,91 ms** (%36,6), açılış dahil ilk sonuç p95’ini **816,53 → 567,01 ms** (%30,6) indirdi. İki ölçüm de 12/12 blokta hızlandı. Açılış tepe private p95 **+0,871 MiB** ve açılış CPU p50 **+15,625 ms**. Deney geçici derlemede kaldı. [Önceki kod incelemesi](STRICT-OPEN-REMAINING-PHASES-20261109.md)

## Kapsam ve yöntem

- Cap200k TR ilk 50.000 belge, aynı 52 run ve 16 aktif delta anahtarı. Generation 8185, sequence 529967. Önceki 159 dosyalı 467.167.854 B kopya yeniden kullanıldı. Yeni korpus, ingest veya DB kopyası yok.
- Aynı release ikilisinde 12 ABBA/BAAB blok, kol başına 24 yeni proses. Kontrol tek işçi. Aday çağıran iş parçacığına bir yardımcı ekliyor. Her blok öncesi kopya normal cached I/O ile tamamen okundu. WPR, ETW ve purge kullanılmadı.
- Pencere 18:21:11–18:22:34 UTC, üst sınır 18:46:11 UTC. Örnek öncesi iki başlatıcı hatası vardı. Windows konsol kodlamasında zaman ayıracı yazılamadı, sonra PowerShell ve Python’un `LOCK` sırası farklı çıktı. 159 dosyanın SHA’sı ayrı kontrol edildi ve eşleşti. İki düzeltme sonrası aynı pencere içinde 48 örnek ilk kez toplandı. [Konsol hatası](benchmarks/strict-open-two-workers-20261109/pre-start-console-error.json) · [Envanter sırası hatası](benchmarks/strict-open-two-workers-20261109/pre-start-inventory-order-error.json)
- İlk sorgu `olduğu`, melez N=50. İlk sonuç saati `Database::open` başlangıcından sorgu dönüşüne. p95 24 örneğin 23. sırası. CPU süreç user+kernel ve iki işçiyi kapsar. Private commit ve working set süreç sayaçlarıdır, OS dosya cache’i hariç. Açılışta hafif run yükleme ve maske zamanlayıcıları kullanıldı.

## Ölçüm

| Ölçüm | 1 işçi p50 / p95 | 2 işçi p50 / p95 |
| --- | ---: | ---: |
| Strict-open (ms) | 701,72 / 717,51 | **444,91 / 464,56** |
| Run yükleme ve doğrulama (ms) | 533,13 / 555,21 | **277,43 / 300,77** |
| Maske (ms) | 160,62 / 166,29 | 161,60 / 166,72 |
| Açılış → ilk sonuç (ms) | 802,03 / 816,53 | **546,07 / 567,01** |
| Yalnız ilk sorgu (ms) | 100,03 / 103,44 | 100,69 / 104,48 |
| Açılış CPU (ms) | 703,13 / 718,75 | 718,75 / 734,38 |
| Run yükleme CPU (ms) | 531,25 / 546,88 | 546,88 / 578,13 |
| Maske CPU (ms) | 156,25 / 171,88 | 156,25 / 171,88 |
| İlk sonuca kadar CPU (ms) | 796,88 / 812,50 | 812,50 / 843,75 |
| Açılış tepe private (MiB) | 8,301 / 8,320 | 8,676 / 9,191 |
| İlk sonuca kadar tepe private (MiB) | 9,729 / 9,738 | 9,721 / 9,906 |
| Açılış tepe working set (MiB) | 12,359 / 12,371 | 12,436 / 12,812 |
| İlk sonuca kadar tepe working set (MiB) | 14,215 / 14,227 | 14,314 / 14,469 |

**KANIT.** Blok ortalamalarının eşleşmiş medyan farkı strict-open **−254,08 ms**, run yükleme **−256,93 ms**, ilk sonuç **−254,39 ms**. Açılış tepe private blok farkı medyanı **+0,310 MiB**, aralık −0,076…+0,770 MiB. Run yüklemesi başlamadan tepe private p95 iki kolda da 0,719 MiB. Ölçülen tepe farkı run yükleme penceresinde oluştu. İlk sonuca kadar private p95 **+0,168 MiB**. Açılış CPU blok farkı medyanı **+23,44 ms**. [Ham 48 örnek](benchmarks/strict-open-two-workers-20261109/samples.jsonl) · [Blok farkları ve min/max](benchmarks/strict-open-two-workers-20261109/summary.json)

**BİLİNMİYOR.** SEG05, kaynak CRC ve dosya açma kalemlerinin paralel kazanç içindeki ayrı payları. Temiz az run’lı yerleşim, 100k, gerçek soğuk, eşzamanlı CPU yükü ve cache onarım yolunun gecikmesi ölçülmedi. 2b’nin gerçek-soğuk sonucu açık.

## Doğruluk

- Paralel kolda her run için salt-okuma doğrulaması yapıldı. Tüm işler bitince sonuçlar manifest sırasına kondu. Eksik veya bozuk cache adayı, o run’ın kaynak doğrulamasını yine yapan seri onarım yoluna verildi. Normal WAL replay ve heap maskesi seri kaldı. Prototip `Pending` sonuç yayımlamıyor. Zamanlı koşularda **0** seri onarım dönüşü. [Prototip](benchmarks/strict-open-two-workers-20261109/probe.rs.txt) · [Geçici bağlantı farkı](benchmarks/strict-open-two-workers-20261109/integration.diff.txt)
- Üç run’lı ayrı fikstürde eksik ve bozuk MET04 seri yolda yeniden üretildi. Kaynakta tek bayt bozulması, sağlam türetilmiş cache varken de açılışı reddettirdi. Geçici modül format ve Clippy kontrolleri geçti. Tam test paketi bu prototip için yeniden koşulmadı. [Fikstür testi](benchmarks/strict-open-two-workers-20261109/fixture-test.log) · [Clippy](benchmarks/strict-open-two-workers-20261109/clippy.log)
- İki işçili aday kill/reopen sonrası 50.000 GET/version ve 41 sorgu oracle’ını geçti. Dondurulmuş eski okuyucu aynı digest’leri verdi. 159 FDB dosyası, ölçüm kopyası ve model SHA’ları korundu. 48/48 zamanlı sorguda 50 hit, CRC32 725765487, strict `Verified`. [Oracle](benchmarks/strict-open-two-workers-20261109/verification.json) · [Envanter ve bitiş](benchmarks/strict-open-two-workers-20261109/run-status.json)

Üretim motoru, varsayılanlar ve kapı eşikleri değişmedi. `C:\Users\Serkan\AppData\Local\Temp\fdb-strict-open-two-workers-20261109` altında **534.755.663 B (510 MiB)** geçici derleme kaldı.
