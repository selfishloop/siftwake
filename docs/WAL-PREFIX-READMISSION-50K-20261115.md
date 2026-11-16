# Prefix yayımından sonra kısa WAL yolunu yeniden değerlendirme

Kısa WAL için biçim ve grup boyutu uygun olduğunda, hazır grup yalnız
etkin delta nedeniyle sığmıyorsa aday önce mevcut prefix işçisinin sonucunu
devralır. Yazıcının elindeki staging dosyası, sequence ve doğrulanmış
sürüm koşulları korunur. Başka bir yazıcı araya giremez. Prefix'in
kurulması WAL satırlarının fiziksel adresini değiştirir. Önceden görünür
olan belge, tombstone ve sürümler aynı commit sınırında kalır.

Yayım sonrası etkin WAL aynı 2.048 anahtar, 64 MiB WAL, delta bütçesi,
128 işlem ve 8 MiB grup sınırları altında yeniden değerlendirilir.
Grup sığarsa mevcut kısa WAL yolu aynı staged frame'i yeni aktif loga
tek seferde ekler ve senkronlar. Sığmazsa uzun yol sürer. İşçi
beklemeden önce intake izni mevcut fallback gibi bırakılır ve yalnızca
kısa yol onaylanınca yeniden edinilir. Normal merge, prefix sonucunun
yayımlanmasından sonra mevcut arka plan iş akışında ilerler.
[Prototip diff'i](benchmarks/wal-prefix-readmission-20261115/prototype.diff.txt)

## Kontrollü doğrulama

Bekletilmiş prefix işçisi ile 2.000 etkin anahtar ve 100 yeni işlem.
Grup hem prefix'te hem sonraki WAL kısmında olan kayıtları sürüm
koşullarıyla güncelliyor ve siliyor. Aday sequence 21'i bir kez
yayımladı, senkron delta flush'a girmedi. Kontrol delta flush'a girdi.
Eski snapshot sequence 20'de kaldı ve eski sürümleri gördü. Yeni
durumda 2.094 belge ve 98 yeni marker sonucu. Yazıcıda exact ile capped
ilk sayfası eşit. Strict reopen ve bütünlük denetimi geçti.
[Kontrollü test](benchmarks/wal-prefix-readmission-20261115/readmission-tests.rs.txt) ·
[Test çıktısı](benchmarks/wal-prefix-readmission-20261115/focused.log)

## Ölçüm düzeni

Aynı Cap200k ilk 50k, aynı temiz seed, sıcak doğal cache, 1 snapshot
okuyucu + 1 yazıcı, 41 sorgu ve 100 işlem/commit. Kısa performans
sırası A1 → B1 → B2 → A2, kol başına 3 tur × %20 churn ve
2 saniye tur arası bakım. Uzun disk karşılaştırması aynı ikiliyle
kontrol → aday, kol başına 30 tur ve tur arasında yazmasız bekleme
penceresi yok. Sınırlı profil yalnız kısa kolda. Posting prototipi
bu derlemeye alınmadı. SQLite referansı aynı yükün arşiv koşusu.
[Sürücü](benchmarks/wal-prefix-readmission-20261115/run.py)

## 1.200 profilli commit

| Kol | Commit p50 / p95 / p99 / max (ms) | >1 s / 300 | İşlem/s, tur arası dahil | ≥50 ms prefix join | Prefix join toplamı | Senkron delta flush | Süreç tepe private |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| A1 kontrol | 11,92 / 52,12 / 1.055,07 / 1.540,79 | 4 | 1.503 | 7 | 2,53 s | 12 | 45,96 MiB |
| B1 aday | 12,33 / 294,83 / 789,44 / 1.124,21 | 1 | 1.505 | 19 | 9,12 s | 0 | 44,61 MiB |
| A2 kontrol | 12,23 / 45,32 / 1.116,11 / 1.457,83 | 5 | 1.485 | 6 | 2,69 s | 12 | 43,92 MiB |
| B2 aday | 11,83 / 177,90 / 759,50 / 1.441,52 | 2 | 1.556 | 18 | 9,11 s | 0 | 46,68 MiB |
| SQLite arşiv | 131,85 / 298,30 / 435,62 / 791,00 | 0 | — | — | — | — | — |

**Kanıt.** Aday, B1'de 22/22 ve B2'de 21/21 grubu prefix yayımından
sonra kısa WAL yoluna geri aldı. Kontroldeki 24 senkron delta flush
toplam 12,14 s. Adayın prefix beklemeleri daha sık ve daha uzun.
P99 iki çiftte %25,2 ve %32,0 azaldı. P95 sırasıyla 5,66 ve 3,93
katına çıktı. Pooled okuma p95'i 112,51 → 111,46 ms ve
112,58 → 110,19 ms. Bu toplu sayaç, 41 sorgunun her biri için
ayrı performans kabulü sayılmaz. Private değerler model ve okuyucuyu
da içeren benchmark sürecine ait.

Kontrollerin en uzun seq 262 commit'inde prefix join 27,74/32,68 ms,
yazıcının delta flush'ı 1.395,52/1.342,78 ms. Adayda 1 saniyeyi aşan
üç commit'in tümünde yalnız prefix join de 1 saniyeyi aştı. En uzun
aday commit B2 seq 329, 1.441,52 ms. Bunun 1.400,42 ms'si prefix
join. Beklemenin sıklığı ile pahalı uçların yeri değişti.

| Aday kol / sequence | Prefix join | Örtüşen en uzun alt aşama |
| --- | ---: | --- |
| B1 / 243 | 1.083,32 ms | Kaynak çalışma alanı edinimi 576,37 ms |
| B2 / 245 | 1.115,06 ms | Kaynak çalışma alanı edinimi 516,37 ms |
| B2 / 329 | 1.400,42 ms | İşçi çalışma bütçesi rezervasyonu 768,75 ms |

Alt aşama süreleri join ile kesişen aralıklardan hesaplandı. Bunlar iç
içe ölçümler, commit toplamına eklenmez. Hangi başka işçinin ilgili
izni ne kadar süre tuttuğu bu izden kesin olarak çıkarılamıyor.
[Tam sequence listesi ve SQLite arşiv istatistikleri](benchmarks/wal-prefix-readmission-20261115/summary-ab.json) ·
[SQLite ham arşiv](benchmarks/live-disk-50k-20261111/5-sqlite-churn-01-10.jsonl)

## Kesintisiz yazıda 30 tur

Her kolun başlangıç boyutu 300.835.962 B. Disk örnekleri yaklaşık
50 ms aralıklarla alındı. Tur sonu tablosu her ACK'e en yakın örneği
kullanıyor. Eşleme sapması kontrol/adayda en çok 26,23/25,54 ms.
Geçici tepe ve önceki 1,3× tur sonu kapısı ayrı değerlendirildi.

| Ölçüm | Kontrol | Aday |
| --- | ---: | ---: |
| 1,3× disk kapısı | **GEÇTİ 30/30** | **KALDI 4/30**, ilk aşım tur 2 |
| En yüksek tur sonu retained | 1,284× | 1,835× |
| Son tur / geçici tepe | 1,243× / 1,545× | 1,779× / 2,018× |
| Son tur retained / manifest-canlı | 373,91 / 364,78 MB | 535,05 / 519,50 MB |
| Son tur kaynak / indeks sayacı | 187,80 / 177,33 MB | 285,92 / 233,93 MB |
| Son tur run sayısı | 87 | 39 |
| Yayımlanan revizyon işleri | 226 | 93 |
| Commit p95 / p99 / max | 79 / 1.092 / 2.244 ms | 391 / 872 / 2.504 ms |
| >1 s commit / 3.000 | 55 | 23 |
| İşlem/s | 1.775 | 1.678 |

**Kanıt.** Son turda retained farkı 161,14 MB, manifest-canlı farkı
154,72 MB. Farkın yaklaşık %96'sı manifestin referans verdiği
dosyalarda. Aday daha az run ile daha büyük toplam kaynak ve indeks
taşıdı. **Sinyal.** Daha az revizyon işi yayımlandı. Yayımlanan iş
sayısı tek başına geri kazanılan byte miktarını göstermiyor. Hangi
işlerin geride kaldığı ve hangi bütçe/çalışma alanı sahibinin uzun
beklemeyi oluşturduğu **bilinmiyor**. Uzun koşu ince aşama profili
kapalı olduğundan kısa koşudaki üç uç uzun koşuya atfedilmedi.
[Kontrolün 30 tur ham özeti](benchmarks/wal-prefix-readmission-20261115/DA/disk-summary.json) ·
[Adayın 30 tur ham özeti](benchmarks/wal-prefix-readmission-20261115/DB/disk-summary.json)

## Karar

**Prototip geçici kopyada.** Aynı 100'lük işlemi senkron delta
flush olmadan bitirebildiği doğrulandı. 1 saniye üstü commit'ler
azaldı. Commit p95 hem kısa hem kesintisiz koşuda arttı ve mevcut
disk kapısı kaldı. Sonraki araştırma gerekirse prefix ve reclaim
işlerinin izin edinme/bırakma sırasını, yayınlanan işe karşı kazanılan
baytı ve sıra durumunu aynı zaman çizgisine bağlamak daha doğrudan.
Üretim motoru, sınırlar, format ve varsayılanlar aynı. Geniş arama,
motor-only RAM ve kalan yazma uçları açık.

## Kapanış

Altı ölçüm kolunda 7.200 commit. Fmt, Clippy `--all-targets -- -D
warnings` ve aday açıkken 270 test geçti, 17 test atlandı. Process kill
sonrası strict reopen, 50k GET/version, 41 exact digest ve dondurulmuş
eski okuyucu 6/6. Kısa kolların son model hash'i eşit, 30 turluk
iki kolun son model hash'i eşit. SQLite slotu, korpus ve 0.1 fixture
kontrolleri geçti.

Başlangıç 06:26:10 UTC, kapanış 06:49:36 UTC. Geçici derleme
462.175.731 B (440,77 MiB), ham kanıt yaklaşık 32,6 MiB. FDB sabit
slotu aday yerleşimiyle `ready`, 50.000 belge, sequence 3101 ve
532.154.690 B. Deneyin dosyaları korundu.
[Test paketi](benchmarks/wal-prefix-readmission-20261115/tests.log) ·
[Clippy](benchmarks/wal-prefix-readmission-20261115/clippy.log) ·
[Kısa ölçüm doğrulamaları](benchmarks/wal-prefix-readmission-20261115/status-ab.json) ·
[Kesintisiz ölçüm doğrulamaları](benchmarks/wal-prefix-readmission-20261115/status-disk.json) ·
[Açık işler](OPEN-WORK.md)
