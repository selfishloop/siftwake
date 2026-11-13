# 50k strict-open paralel görünürlük maskesi — 7 Kasım 2026

**Seri kol daha hızlı.** 2 işçi eşleşmiş blokta medyan **+258 ms**,
4 ve 8 işçi **+907 ve +908 ms** açılış gecikmesi ekledi. Her aday
**0/12** blokta daha hızlıydı. Dört oracle açılışında maskeler bayt bayt
eşit, 48/48 zamanlı sonuç oracle’ı geçti.
[Uygulamadan önce kaydedilen doğruluk analizi](STRICT-OPEN-PARALLEL-DESIGN-50K-20261107.md)

## Kapsam ve düzen

Başlangıç **13:11:20 UTC**, örnek sonu **13:33:20**, deadline **13:36:20**.
48 zamanlı ve dört oracle açılışı **13:16:03,893–13:17:50,961 UTC**.
16 mantıksal işlemci, tek geçici kopya, 159 dosya ve 467.167.854 B.
50k Cap200k TR kimliği 881756DA…, 52 run, generation 8185, sequence
529967. Önceki kopya bulunmadığı için aynı sabit slot, kabul edilmiş
159 dosya SHA envanterine göre yeniden kopyalandı. Ingest yapılmadı.

Aynı default release ikilisi, her kolda 12 ayrı proses. Dört sıra
`1,2,8,4`, `2,4,1,8`, `4,8,2,1`, `8,1,4,2` üç kez.
Her blok öncesi kopyanın tamamı normal cached I/O ile okundu.
WPR, ETW, purge ve ağır profiler kullanılmadı. İşçi sayısı çağıran
thread’i içerir. Yalnız maskenin run başına işi ve işçi sonuçlarının
birleştirilmesi değişti. SEG05, kaynak CRC ve WAL yolları seri kaldı.
İşçi sonuçları manifest sırasıyla mevcut maske cache’ine alındı.

p95 en yakın üst sıra ile N=12’de maksimumdur. CPU, Windows
`GetProcessTimes` user+kernel farkı ve tüm scoped thread’leri kapsar.
Maske süresi spawn, join ve sıralı cache kurulumunu içerir. Sorgu
oracle’ı ve audit çıktısı zamanlı örneklerde açılış dışında. Bellek,
proses tepe private commit ve working set. OS dosya cache’ini içermez.

## Süre ve CPU

| Toplam işçi | Açılış wall p50 / p95 (ms) | Maske wall p50 / p95 (ms) | Açılış CPU p50 / p95 (ms) | Maske CPU p50 / p95 (ms) | Eşleşmiş açılış farkı medyan (ms) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| **1, seri** | **1.274,55 / 1.298,58** | **756,40 / 774,00** | **1.265,63 / 1.296,88** | **750,00 / 765,63** | — |
| 2 | 1.528,16 / 1.588,27 | 1.009,17 / 1.045,90 | 2.460,94 / 2.578,13 | 1.945,31 / 2.046,88 | **+258,14** |
| 4 | 2.176,85 / 2.239,90 | 1.659,59 / 1.728,88 | 3.468,75 / 3.562,50 | 2.953,13 / 3.062,50 | **+907,16** |
| 8 | 2.172,01 / 2.243,31 | 1.657,98 / 1.722,19 | 3.593,75 / 3.718,75 | 3.062,50 / 3.203,13 | **+908,17** |

Seriye göre açılış p50 yaklaşık **+%20 / +%71 / +%70**. Eşleşmiş en küçük
açılış kaybı 2/4/8 işçide **+232 / +859 / +871 ms**. Yavaşlama maske
penceresinde görülüyor. `PagedRecords::page` aynı global mutex altında
cache erişimi, sayfa okuma ve decode yapıyor. Bu yol paylaşıldı.
Mutex bekleme, page miss ve scheduler payları ölçülmedi.
[Paylaşılan paged cache kodu](../src/segment/paged.rs#L300)

## Geçici RAM

| İşçi | Tepe private commit p50 / p95 (MiB) | Açılış sonu private p50 / p95 (MiB) | Tepe WS p50 / p95 (MiB) | Eşleşmiş tepe private farkı medyan (MiB) |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 7,818 / 7,828 | 7,381 / 7,391 | 11,799 / 11,812 | — |
| 2 | 7,816 / 7,824 | 7,365 / 7,379 | 11,865 / 11,891 | −0,002 |
| 4 | 7,816 / 7,984 | 7,393 / 7,426 | 11,906 / 11,969 | 0,000 |
| 8 | 7,826 / 8,227 | 7,469 / 7,727 | 11,998 / 12,207 | +0,008 |

Seri p95’e göre 2/4/8 işçi tepe private farkı **−0,004 / +0,156 /
+0,398 MiB**. Maske sırasında süreç ömrü tepesindeki en büyük ek artış
2/4/8 için **0 / 0 / 0,211 MiB**. Önceki açılış aşaması daha yüksek
tepeye ulaştıysa sonraki geçici tahsis bu sayaçta görünmez.
52 ham bitsetin veri yükü **10.528 B**. İşçi stack commit’i,
eşzamanlı anahtarlar ve sonuç vektörü başlıkları bu hesap dışında.
Global metadata ve kalıcı cache bütçesi aynı.

## Doğruluk ve kapanış

- 1/2/4/8 işçinin 97.306 B açılış oracle çıktısı doğrudan byte
  karşılaştırmasında eşit. 52 run kimliği, 82.072 slotun maskeleri,
  shadow öncesi/sonrası bütün bitler, görünür sayılar ve cache sırası
  dahil. Cache 46 giriş ve 63.720 B.
- Bu yerleşimde shadow adımı hiçbir maske sözcüğünü değiştirmedi.
  Shadow yeni bit ekleyen bir fixture bu turda çalıştırılmadı.
- Dört audit ve 48 zamanlı örnek case0 `olduğu`, CRC32 725765487,
  50 hit, sequence 529967 ve 50.000 belge oracle’ını geçti.
  Her örnekte kopya envanteri aynı. Sonda canlı slot/kopya 159/159
  SHA-256 ve model SHA eşit. Önceki kill ve eski okuyucu kapsamı
  değişmedi, bu deneyde yeniden koşulmadı.
- İzole release build, yeni Rust dosyalarının rustfmt kontrolü,
  izole probe clippy `-D warnings` ve runner AST geçti. Üretim motoru,
  dosya biçimi ve kapılar aynı. Bu prototip için birleştirme yapılmadı.

[Ham 48 örnek](benchmarks/strict-open-parallel-50k-20261107/samples.jsonl) ·
[Bloklar ve özet](benchmarks/strict-open-parallel-50k-20261107/analysis.json) ·
[Seri maske oracle’ı](benchmarks/strict-open-parallel-50k-20261107/audit-1.json) ·
[SHA ve zaman doğrulaması](benchmarks/strict-open-parallel-50k-20261107/validation.json) ·
[Frunner hazırlık tablosu](FRUNNER-READINESS-20261107.md)


Rapor kapanışı **13:21:28 UTC**. Bu deneye ait tek geçici kopya ve derleme klasörü **620099198 B**. Klasör kullanıcı temizliği için bırakıldı.
C:\Users\Serkan\AppData\Local\Temp\fdb-strict-parallel-20261107-131120
