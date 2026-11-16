# WAL prefix maske düzeltmesi sonrası tekrar ölçümleri

Aynı 881756DA… Cap200k korpus önekleri, akışlı yazma, doğal sıcak işletim
sistemi cache'i ve tek sahipli FDB slotu. 50k ölçümlerinde önceki 41 sorgu
ve bir snapshot okuyucusu. SQLite bu tekrarın kayıtlı FDB yüklerine
katılmadı, slotu korundu. Ölçüm kaynak kopyasında önceki tanı kodu ve
kabul edilmiş maske sıfırlaması var. Üretim ayarları, kapıları ve biçimleri
aynı. Tarihsel kontrol önceki oturumdan geldiği için farklar tek başına
maske reset'inin nedensel maliyeti olarak okunamaz.
[Sürücü](benchmarks/wal-prefix-remeasure-20261114/run.py) ·
[Önceki ölçüm sınırları](RECLAIM-FOLLOWUP-DESIGN-20261113.md)

## Dosya olayı görünürlüğü

**GEÇTİ.** 24/24 aday ve 24/24 kapalı kontrol, ilk doğru marker sonucunu
1 saniye içinde verdi. 50/500/1.500/3.000 ms ve altı çift çevrim korundu.
Bir aday olay rewrite ile örtüştü (sequence 3418), bir olay yayım ile
örtüştü (3747), GC örtüşmesi 0. Süreç sonlandırma sonrası strict
reopen, 50.000 GET/version ve 41 sorgu geçti. Eski okuyucunun 41 digest'i
eşleşti. Kurtarma modeli, önceki koşunun sequence 3750 modeliyle SHA eşit.

Tabloda p50 / p95 / max, ms.

| Aşama | Önce reclaim açık | Sonra reclaim açık | Önce reclaim kapalı | Sonra reclaim kapalı |
| --- | ---: | ---: | ---: | ---: |
| Olay → doğru sonuç | 44,86 / 84,34 / 106,28 | 57,84 / 110,47 / 119,31 | 49,49 / 88,73 / 105,66 | 44,22 / 105,37 / 132,53 |
| Kuyruk | 0,022 / 10,326 / 22,546 | 0,021 / 0,035 / 10,928 | 0,025 / 0,048 / 0,075 | 0,025 / 0,038 / 0,049 |
| Commit | 2,38 / 4,44 / 12,13 | 2,63 / 11,24 / 13,96 | 3,05 / 15,41 / 15,78 | 7,61 / 15,50 / 20,92 |
| Commit dönüşü → okuyucu başlangıcı | 27,77 / 63,43 / 90,48 | 35,52 / 83,46 / 95,85 | 34,28 / 69,96 / 75,05 | 15,47 / 78,07 / 96,68 |
| Snapshot edinme | 0,0002 / 0,0003 / 0,0006 | 0,0002 / 0,0005 / 0,0008 | 0,0003 / 0,0005 / 0,0005 | 0,0003 / 0,0005 / 0,0008 |
| İlk marker sorgusu | 12,18 / 13,92 / 15,33 | 18,31 / 24,68 / 26,85 | 10,82 / 14,47 / 15,17 | 13,97 / 22,46 / 24,85 |

Önceki fiziksel slotun üzerine sonraki deneyler yazılmıştı. Başlangıç,
aynı 50k seed, 30 tur ve dört kurtarma değişikliğiyle tek sabit slotta
yeniden kuruldu. Açılışta sequence 3101 ve model parmak izi aynı.
Yerleşim **52 → 63 run**, WAL delta **104 → 504 anahtar**. Bu koşudaki
p95 artışı iki reclaim kolunda da görüldü. Artışın ne kadarı maske
sıfırlamasından geldiği bilinmiyor.
[Önce](RECLAIM-ARRIVAL-DEFAULT-50K-20261114.md) ·
[Ham sonuç ve yerleşim](benchmarks/wal-prefix-remeasure-20261114/01-arrival/01-arrival-summary.json) ·
[Kurtarma ve eski okuyucu](benchmarks/wal-prefix-remeasure-20261114/01-arrival/status.json)

## 100'lük commit kuyruğu ve aşama profili

**Tamamlandı.** A1/B1/B2/A2, kol başına temiz 50k seed, 30 tur ve
3.000 commit. İki saniyelik tur arası korundu. A profilsiz, B profilli.
Dört kolda da reclaim varsayılan açık. Dört post-ACK process-kill,
strict reopen ve eski okuyucu geçti. Dört model SHA'sı önceki koşularla eşit.

| Kol | p50/p95/p99/p99,9/max önce (ms) | Sonra (ms) | >1 s önce → sonra / 3.000 | Yazı + aralar önce → sonra (işlem/s) | Seq 2511 önce → sonra (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| A1 | 17,63 / 88,67 / 1.239,55 / 2.368,80 / 2.856,43 | 14,25 / 68,95 / 1.020,16 / 1.675,77 / 1.855,02 | 87 → 34 | 1.196 → 1.407 | 17,65 → 15,30 |
| B1 | 16,56 / 79,56 / 1.151,71 / 1.960,83 / 2.555,84 | 14,38 / 69,54 / 988,48 / 1.778,43 / 2.858,11 | 73 → 28 | 1.265 → 1.402 | 60,27 → 15,05 |
| B2 | 16,43 / 81,37 / 1.139,03 / 1.881,53 / 2.250,94 | 16,94 / 84,28 / 1.160,56 / 1.920,61 / 3.109,18 | 71 → 72 | 1.273 → 1.265 | 21,31 → 19,24 |
| A2 | 16,37 / 81,26 / 1.139,06 / 2.072,83 / 2.498,61 | 16,95 / 80,99 / 1.161,74 / 1.933,16 / 2.497,22 | 70 → 83 | 1.256 → 1.238 | 15,55 → 17,82 |

| >1 s commit'te en büyük ayrıştırılmış aşama | Önce | Sonra B1 | Sonra B2 | Sonra toplam |
| --- | ---: | ---: | ---: | ---: |
| WAL prefix join | 68 | 13 | 35 | 48 |
| Normal merge join | 15 | 9 | 7 | 16 |
| Ortak kaynak/bellek bütçesi | 6 | 1 | 1 | 2 |
| Açıklanamayan süre | 55 | 5 | 29 | 34 |
| Toplam | 144 | 28 | 72 | 100 |

6.000 commit trace frame, 12.270 yardımcı işçi/release kaydı, düşen
kayıt 0. Uzun commit ve seq 2511 için join eşlemesi eksik 0.
B1 sequence 2019 (2.858 ms) ve B2 sequence 2599 (3.109 ms) en büyük
ölçülen kalem olarak sırasıyla 1.756 ve 1.892 ms normal merge join
taşıdı. Aynı örneklerde toplam snapshot maske alt sayaçları 54 ve
36 ms. Bunlar bütün snapshot maske hazırlıklarıdır, eklenen reset'in
süresi ayrıca ölçülmedi. A2'de 1 saniye üstü adet ve p99, B2'de
p95/p99/p99,9/max yükseldi. 1 saniye bu tabloda tanı sınıfıdır.
[Önce](RECLAIM-COMMIT-PROFILE-50K-20261114.md) ·
[Kollar ve doğrulamalar](benchmarks/wal-prefix-remeasure-20261114/02-profile-abba/status.json) ·
[Sequence ve aşama dökümleri](benchmarks/wal-prefix-remeasure-20261114/02-profile-abba/outlier-detail.json)

## Kesintisiz yazıcı

**Disk kapısı GEÇTİ 30/30.** 50k temiz seed, 30 × %20 churn,
100'lük commit ve bir snapshot okuyucusu. Tur sonlarında yazmasız
pencere yok. İşletim sistemi dosya örnekleyicisi yazıcı dışında.
50k GET/version, 41 sorgu, process-kill ve eski okuyucu geçti.
Son model önceki kesintisiz koşuyla aynı SHA'da.

| Ölçüm | Önce | Sonra |
| --- | ---: | ---: |
| Başlangıç | 300,84 MB | 300,84 MB |
| En yüksek tur sonu retained / başlangıç | 1,275× | 1,285× |
| Son tur retained / başlangıç | 383,66 MB / 1,275× | 384,64 MB / 1,279× |
| Son manifest-canlı disk | 382,84 MB | 384,02 MB |
| Örnekli geçici tepe / başlangıç | 459,82 MB / 1,528× | 464,30 MB / 1,543× |
| 1,3× tur sonu aşımı | 0/30 | 0/30 |
| Son run / kurulmuş revision çıktısı | 92 / 225 | 92 / 235 |
| Commit p95 / p99 / max (ms) | 94,69 / 1.164,27 / 2.605,41 | 92,45 / 1.184,29 / 2.195,61 |
| Commit >1 s | 78/3.000 | 83/3.000 |
| Yazı ve doğal aralar (işlem/s) | 1.665 | 1.624 |

30 tur örneğinin 26'sında manifest kuşağı tarama boyunca sabit kaldı.
Tur sonu ACK eşlemesinde en büyük zaman farkı 28,47 ms. Yazı bitiminde
bir revizyon işçisi çalışıyordu. p99 ve >1 s adet arttı, max düştü.
[Önce](RECLAIM-CONTINUOUS-DISK-50K-20261114.md) ·
[30 tur ham özet](benchmarks/wal-prefix-remeasure-20261114/03-continuous/disk-summary.json) ·
[Kurtarma ve eski okuyucu](benchmarks/wal-prefix-remeasure-20261114/03-continuous/status.json)

## Orijinal 120 saniye pin tekrarı

**Doğruluk, örnekli disk ve pin temizliği GEÇTİ.** Aynı 10k önek,
bir %20 churn ve 20 adet 100'lük akışlı commit. Orijinal Rust harness
dosyası SHA256 eşit (`A44D8337…`). Aynı 5/30/120 saniye örnekleri,
eski snapshot digest/GET, pin bırakma ve yazıcıdan 41 exact sorgu
kontrolü uygulandı. Okuma ve kaynak CRC doğrulamaları açık.

| Ölçüm | Önce | Sonra |
| --- | ---: | ---: |
| Snapshot ömrü | 120,120 s | 120,095 s |
| 5 / 30 / 120 s retained / manifest-canlı | 1,075× / 1,075× / 1,075× | 1,075× / 1,075× / 1,075× |
| En yüksek izlenen retained / canlı | 1,079× | 1,079× |
| 120 s pinli eski WAL | 5.333.225 B | 5.333.225 B |
| Bırakma → pinli dosyanın kaybolduğunun doğrulanması | 1,683 ms | 1,147 ms |
| Bırakma sonrası retained / canlı | 1,000× | 1,000× |
| Eski snapshot, sequence 20 | 8 GET, case 0 CRC32 4242832620 | Aynı |
| Yeni görünüm, sequence 40 | 10k GET ve 500 silme geçti, ilk exact sorgu slot mismatch ile durdu | 10k GET, 500 silme ve 41 exact sorgu geçti. Case 0 toplam 2.853, CRC32 3863588042 |
| Process kill → strict reopen ve eski okuyucu, sequence 41 | Deney durduğu için ölçülmedi | 10k GET, 501 silme, 41 digest eşit |
| Kurulmuş revizyon rewrite'ı | 0 | 0 |
| 120 s proses private / working set | 10,02 / 13,01 MiB | 8,07 / 12,50 MiB |
| OS proses tepe private / working set | 19,86 / 24,21 MiB | 19,62 / 24,07 MiB |

Disk ≤2× örneklerde korundu. RAM'e harness ve model dahil.
Bu koşunun emekli dosya kanıtı WAL içindir. 120 s pin sırasında bir
revizyon rewrite'ı yayımlanmadı, dolayısıyla uzun pin altında
emekli rewrite dosyalarının disk tavanı bu deneyde ölçülmedi.

İlk açılış denemesinde derleme önbelleği 50k doğrulayıcıyı 10k çıktısı
olarak verdi. 10k seed oracle'ı geçti, yanlış ikili daha churn ve pin
başlamadan belge sayısı önkoşulunda durdu. Aynı sequence 20 seed'i
korundu. 10k derlemesi yenilendi, ikili hash'leri ayrıştırıldı ve tek
120 s ölçümü ilk 8 dakikalık süre penceresi içinde tamamlandı.
[Önce](RECLAIM-LONG-SNAPSHOT-10K-20261114.md) ·
[Derleme aksamasının ham kaydı](benchmarks/wal-prefix-remeasure-20261114/05-long-pin/long-pin-120s.stderr) ·
[Tekrar özeti](benchmarks/wal-prefix-remeasure-20261114/05-long-pin-resume/summary.json) ·
[Ham sorgu sonuçları](benchmarks/wal-prefix-remeasure-20261114/05-long-pin-resume/long-pin-120s.jsonl)

## Kapılar ve alan

| Adım | Sonuç | UTC |
| --- | --- | --- |
| Olay başlangıcını yeniden kurma | Önceki modelle eşit | 07:07:00–07:14:59 |
| Dosya olayı, ≤1 s | **GEÇTİ 24/24** | 07:14:59–07:17:01 |
| ABBA, 4 × 30 tur | **Tamamlandı**, oracle/kurtarma 4/4 ve trace kaybı 0 | 07:17:01–07:38:52 |
| Kesintisiz disk, 30 tur ≤1,3× | **GEÇTİ 30/30** | 07:38:52–07:43:38 |
| Uzun pin, ≤2× ve tam doğruluk | **GEÇTİ** | 07:43:48–07:48:07 |

FDB slotu `ready`, 10k, sequence 41. SQLite slotu, korpus, önceki
model ve 0.1 fixture korundu. Yeni kanıt yaklaşık 83,94 MiB.
Boyutlar dosya uzunlukları toplamıdır.

| Konum | MiB |
| --- | ---: |
| `target/wal-prefix-remeasure-20261114` | 98,41 |
| `target/wal-prefix-publication-oracle-20261114` | 415,78 |
| `target/wal-prefix-visibility-20261114` | 83,83 |
| `target/reclaim-followup-20261114` | 93,18 |
| FDB slotu | 68,25 |
| SQLite slotu | 491,41 |

Geçici derlemeler korundu. Toplu yazmanın prefix/merge beklemeleri ve
geniş taramalar açık. [Yürütme kaydı](benchmarks/wal-prefix-remeasure-20261114/status.json)
