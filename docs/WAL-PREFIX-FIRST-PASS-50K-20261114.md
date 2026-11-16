# 50k WAL prefix işçisinin ilk geçişi

İlk 50 bin Cap200k belge (`881756DA…`), sıcak doğal cache, 1 snapshot
okuyucu + 1 yazıcı, 41 sorgu, 100 işlem/commit ve 2 saniye tur arası
bakım. Üç tur × %20 churn, 300 commit. Aynı temiz seed düzeni için sabit
FDB slotu bir kez sıfırlandı. Ölçüm ikilisi `ddb263b` tabanlı önceki
profilleme kopyasından türetildi. Önceki merge erteleme deneyi kapalı,
arka plan reclaim ve akışlı WAL ayarları önceki kontrolle aynı. SQLite
ölçümü ve yeni korpus yok. Bu, tek profilli tanı kolu.
[Önceki tanı](WAL-PREFIX-WAIT-DIAGNOSIS-50K-20261114.md) ·
[Sürücü](benchmarks/wal-prefix-first-pass-20261114/run.py) ·
[Geçici ölçüm diff'i](benchmarks/wal-prefix-first-pass-20261114/instrumentation.diff.txt)

## Sonuç

**Kanıt.** 300 commit'in 12'si uzun yola girdi. 12'sinde de aktif WAL
2.000 farklı anahtardaydı, 100 yeni anahtar ile 2.048 sınırı aşıldı ve
prefix işçisi hâlâ çalışıyordu. Başka kısa yol koşulu ihlal edilmedi.
Önceki çevrimdışı çıkarım bu koşuda canlı sayaçla doğrulandı.
[300 kabul kararı](benchmarks/wal-prefix-first-pass-20261114/commits.csv)

**Kanıt.** En az 50 ms join'e yol açan sekiz işçinin **ilk geçişinin
%95,2'si `add_builder` duvar süresi**, %4,8'i WAL satırı edinme
ve doğrulama aralığı. `add_builder` normalleştirme, gram çıkarma,
posting ve diğer indeks alanı eklemelerini kapsar. Bunların tekil CPU
payları ölçülmedi. İlk geçişin toplam işçi CPU'su 2.156 ms, toplam
duvar süresi 2.248 ms. Windows thread CPU sayacı yaklaşık 15,625 ms
adımlarla sonuç verdi, kısa örneklerde CPU değeri duvar süresini
aşabiliyor.
[25 işçi](benchmarks/wal-prefix-first-pass-20261114/workers.csv) ·
[İlk geçiş kodu](../src/engine/light_flush.rs#L53) ·
[Builder](../src/index.rs#L575)

| Sekiz beklemeli işçi, tüm işçi aralığı | Toplam | p50 | p95 |
| --- | ---: | ---: | ---: |
| İlk geçiş, duvar | 2.248,12 ms | 256,57 ms | 503,82 ms |
| WAL satırı okuma, CRC, decode, kimlik/sürüm | 107,88 ms | 13,30 ms | 24,10 ms |
| `add_builder` | 2.140,22 ms | 243,27 ms | 479,71 ms |
| İlk geçiş, thread CPU | 2.156,25 ms | 234,38 ms | 500,00 ms |
| Ardından kaynak hazırlığı | 191,87 ms | 27,92 ms | 35,28 ms |
| Ardından indeks çıktısı hazırlığı | 1.121,25 ms | 131,67 ms | 176,70 ms |
| İşçi toplamı, duvar | 3.588,73 ms | 403,50 ms | 716,69 ms |
| İşçi toplamı, thread CPU | 3.062,50 ms | 328,13 ms | 656,25 ms |

**Kanıt.** Sekiz join'in toplamı 2.226,58 ms. 946,44 ms (%42,5)
ilk geçişle, 1.076,83 ms (%48,4) indeks çıktısı hazırlığıyla,
173,11 ms (%7,8) kaynak hazırlığıyla örtüştü. İlk geçiş içinde belge
başına iz saklanmadığı için join'deki `add_builder` payı yalnız
aralık olarak hesaplandı. Zaman dilimlerinin sınırları %38,7–42,5
(862,70–946,44 ms). Kaynak çalışma alanı beklemesi bu sekiz örnekte
toplam 0,0014 ms. Önceki seq 244'teki 525 ms çalışma alanı beklemesi
bu koşuda tekrarlanmadı. Aynı sequence farklı işçiye denk gelebiliyor.
Örneğin seq 124 bu koşuda seq 116'da başlatılan, 1.300 anahtarlı
işçiye denk geldi. Arşivdeki işçi seq 120'de, 1.700 anahtarla başlamıştı.
[Join ve işçi eşlemesi](benchmarks/wal-prefix-first-pass-20261114/joins.csv)

## Commit uçları

| Sequence | Commit | Prefix join | Normal merge join | Foreground delta flush |
| --- | ---: | ---: | ---: | ---: |
| 217 | 1.020,86 ms | 99,04 ms | 0 ms | 779,11 ms |
| 241 | 1.167,13 ms | 611,10 ms | 0 ms | 393,29 ms |
| 262 | 1.609,57 ms | 41,05 ms | 0 ms | 1.468,29 ms |
| 292 | 2.146,73 ms | 12,23 ms | 1.268,83 ms | 696,39 ms |
| 340 | 1.136,78 ms | 391,39 ms | 0 ms | 548,74 ms |
| 397 | 1.511,39 ms | 255,20 ms | 524,19 ms | 570,42 ms |

Seq 241'de işçinin ilk geçişi 503,82 ms. WAL satırı tarafı 24,10 ms,
`add_builder` 479,71 ms. Seq 292 ve 262 için baskın aşamalar ayrı.
Bunların nedeni prefix ilk geçişine atfedilemez.

| İlk 300 commit | p50 | p95 | p99 | max | >1 s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Arşiv A1 profilsiz | 13,82 ms | 50,27 ms | 1.056,82 ms | 1.629,01 ms | 5 |
| Arşiv A2 profilli | 12,58 ms | 39,35 ms | 1.152,91 ms | 2.200,66 ms | 6 |
| Bu tur ayrıntılı profilli | 14,58 ms | 53,82 ms | 1.167,13 ms | 2.146,73 ms | 6 |

**Sınır.** Arşiv aynı iş yükünün ilk üç turunu gösteriyor. Aynı
oturumda profil kapalı/açık eşleşmesi yapılmadı, ölçüm maliyeti
ayrıştırılamıyor. Yeni hız veya performans kapısı kararı yok.
[Ham koşu](benchmarks/wal-prefix-first-pass-20261114/01-profile/first-pass.jsonl) ·
[Hesaplama](benchmarks/wal-prefix-first-pass-20261114/analyze.py) ·
[Özet](benchmarks/wal-prefix-first-pass-20261114/summary.json)

## Doğrulama ve sınırlar

- 300 frame ve 25 prefix işçi örneği, kayıp 0. Yazma penceresi
  23,63 s. Başlangıç 12:16:11 UTC, slot doğrulaması 12:23:08 UTC,
  son saat 12:36:11 UTC. FDB slotu `ready`, sequence 401. Ölçülen
  disk tepesi 416.015.831 B. SQLite slot envanteri, korpus, önceki
  model ve 0.1 fixture hash kontrolleri geçti.
  [Durum ve zamanlar](benchmarks/wal-prefix-first-pass-20261114/status.json)
- ACK sonrası process kill, strict reopen, tam 50k GET/version, 41-case
  digest ve dondurulmuş eski okuyucu **GEÇTİ**. Fmt, Clippy
  `--all-targets -- -D warnings` ve tüm hedef testleri geçici kopyada
  geçti (269 başarılı, 17 atlandı). Kaynak CRC/bozulma testleri dahil.
  [Strict oracle](benchmarks/wal-prefix-first-pass-20261114/01-profile/first-pass-reopen-oracle-bounded.jsonl) ·
  [Eski okuyucu](benchmarks/wal-prefix-first-pass-20261114/01-profile/first-pass-legacy.jsonl) ·
  [Clippy](benchmarks/wal-prefix-first-pass-20261114/clippy.log) ·
  [Test paketi](benchmarks/wal-prefix-first-pass-20261114/tests.log)
- İlk derleme Cargo çıktısını yanlışlıkla FDB klasörünün üstündeki
  `ChatGPT/build` dizinine yazdı. İlk sürücü, slotu sıfırlamadan önce
  ikili yol kontrolünde durdu. Dosya çalışma dizinine aynı SHA-256 ile
  alındı, koşu bir kez yapıldı. Ölçüm ve doğrulama kapanışında yerel
  geçici klasör 146.458.293 B, üst dizindeki derleme klasörü 83.643.730 B.
  Tam test paketinin debug çıktıları yerel klasörü 2.170.218.791 B'a
  çıkardı. Böylece ölçüm bitmiş olsa da geçici derleme için belirlenen
  2 GiB toplamı 21,68 MiB aşıldı. Üst dizindeki çıktı da sayıldığında
  toplam aşım 101,45 MiB. Ham kanıt yaklaşık 4,14 MiB. Dosyalar saklandı,
  başka ölçüm yapılmadı.
  [İlk sürücü çıktısı](benchmarks/wal-prefix-first-pass-20261114/driver.log)

## Karar için öneri

WAL okumasını hızlandırmak bu izde ilk geçişin küçük bir bölümüne
etki eder. İlk aday alan gram/posting hazırlığı ve ardından indeks
çıktısının oluşturulması. Önce mevcut `body.fold`, `body.grams`
ve `index.gram_posting_loop` sayaçlarını prefix işçisine ayrı
bağlayıp bu üç kalemi ayırmak gerekir. Eşik, işçi sayısı veya
bellek sınırı değişikliği denenmedi. Foreground flush ve normal
merge join uçları ayrı açık işler. Bu kısa koşu uzun disk tavanı,
motor-only RAM ve geniş arama uçları için yeni sonuç üretmedi.
[Mevcut sayaçlar](../src/index.rs#L329) · [Açık işler](OPEN-WORK.md)
