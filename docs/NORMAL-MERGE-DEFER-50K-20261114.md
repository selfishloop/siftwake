# Akışlı commit'te delta flush sonrası merge ertelemesi

Geçici derleme tabanı `ddb263b` ve önceki profilli 50k ölçüm kaynağı.
Aynı ikili, değişken kapalıyken kontrolü, açıkken adayı çalıştırıyor.
Aday, yalnız akışlı commit'in uzun yoldaki ilk delta flush'ında otomatik
merge başlatmayı erteliyor. Yeni iş, başarılı transaction yayımı ve
çalışma alanının bırakılması sonrasındaki mevcut planlama adımına kalıyor.
Kısa WAL yolu, ilk worker drain, CRC, kalıcı biçim ve üretim ayarları aynı.
[Tasarım](NORMAL-MERGE-WAIT-DESIGN-50K-20261114.md) ·
[Sürücü](benchmarks/normal-merge-defer-20261114/run.py)

## Doğruluk

| Kontrol | Sonuç |
| --- | --- |
| Aynı 129 işlem ve durdurulmuş merge işçisi | **GEÇTİ.** Kontrol sequence 4'te bekledi. Aday, işçi bırakılmadan ACK ve snapshot sequence 5 yayımladı. İşçi bırakılınca merge kuruldu ve transaction run'ı korundu. |
| GET, exact/capped ve pin | **GEÇTİ.** Güncel update ve silme doğru. Eski snapshot önceki sürümleri gördü. Strict reopen ve kaynak bütünlük kontrolü geçti. |
| Aday process kill | **GEÇTİ 2/2.** ACK sonrası merge'den önce ve merge yayımından sonra, strict reopen ve oracle eşit. |
| Fmt, Clippy ve tüm hedefler | **GEÇTİ.** Aday açıkken 269 geçti, 17 atlandı. Kaynak bozulma, mevcut publication oracle ve kurtarma testleri dahil. |

Arşiv ölçüm kopyasındaki kullanılmayan import ve iki eşdeğer chunk
döngüsü Clippy için düzeltildi. İlk Clippy çıktısı da saklandı.
[Bariyer ve kill](benchmarks/normal-merge-defer-20261114/controlled-tests.log) ·
[Clippy](benchmarks/normal-merge-defer-20261114/clippy-final.log) ·
[Tam test paketi](benchmarks/normal-merge-defer-20261114/candidate-tests.log)

## 50k, 1R+1W, dört 30 turluk kol

Aynı Cap200k ilk 50 bin, sıcak doğal cache, 41 sorgu, akışlı 100'lük
gruplar ve iki saniyelik tur arası. Her kol aynı temiz seed
politikasından başlıyor. Sıra kontrol → aday → profilli aday → profilli
kontrol. 12.000 commit, dört post-ACK kill/reopen, 50k GET ve 41 exact
digest ile dört dondurulmuş eski okuyucu geçti. Son model hash'leri eşit.

| Kol | p50 / p95 / p99 / p99,9 / max (ms) | >1 s / 3.000 | Yazı ve aralar (işlem/s) | Seq 2511 (ms) |
| --- | ---: | ---: | ---: | ---: |
| A1 kontrol | 16,50 / 80,38 / 1.288,93 / 2.033,03 / 2.258,32 | 72 | 1.233 | 21,35 |
| B1 aday | 14,58 / 68,91 / 1.054,80 / 1.701,89 / 1.806,76 | 45 | 1.370 | 15,15 |
| B2 aday, profilli | 17,29 / 83,09 / 1.328,24 / 1.829,65 / 2.509,80 | 74 | 1.216 | 16,16 |
| A2 kontrol, profilli | 17,28 / 84,23 / 1.318,26 / 2.068,01 / 2.521,44 | 76 | 1.211 | 17,13 |

Profilsiz aday p99 %18,2 daha düşük ve yazı hızı %11,1 daha yüksek.
Profilli çiftte p99 %0,8 daha yüksek, p99,9 %11,5 daha düşük ve
throughput farkı yaklaşık %0,4. İki çift farklı büyüklükte kazanç
verdi. 1 s, önceki gibi tanı sınıfı.

| Profilli toplam, 3.000 commit | Kontrol | Aday |
| --- | ---: | ---: |
| Flush sonrası normal merge join | 3 / 2.747,69 ms | 0 / 0 ms |
| İlk drain, normal merge join | 14 / 5.935,04 ms | 24 / 9.967,58 ms |
| Diğer normal merge join | 14 / 0,12 ms | 18 / 0,15 ms |
| Tüm normal merge join | 31 / 8.682,85 ms | 42 / 9.967,73 ms |
| WAL prefix join süresi | 40.836,43 ms | 42.180,08 ms |
| Foreground rezervasyon süreleri toplamı | 4.591,01 ms | 3.272,98 ms |
| Kaynak workspace mutex | 1.300,88 ms | 424,91 ms |
| Builder workspace mutex | 0,02 ms | 0,01 ms |
| Tamamlanmış merge işçi aralıkları | 101 | 120 |

Faz süreleri kendi sütunlarında toplanır. İç içe fazları toplayıp
commit duvar süresi saymıyoruz. Trace kapasite kaybı 0, iki profilli
kolda 6.000 frame ve 12.543 yardımcı kayıt. Adayın en uzun örneği
sequence 923, 2.510 ms. 762 ms prefix ve 766 ms ilk normal merge join.
Sequence 2547, 2.199 ms, bunun 912 ms'si ilk normal merge join.
Örnek olarak işçi 826'nın çalışma aralığı seq 916 içinde başladı ve
seq 923'te join edildi. Arşiv profili spawn çağrısının sahibini değil
işçinin çalışma aralığını kaydediyor. Bu eşleme belirli bir ertelenen
işçinin hangi sonraki commit'e taşındığını kanıtlamıyor. Adayın toplam
normal merge beklemesi profilli örnekte **%14,8 arttı**. İlk drain'deki
artış, kaldırılan ikinci beklemenin toplamını aştı.

[Dört kol ve doğrulamalar](benchmarks/normal-merge-defer-20261114/01-commit/status.json) ·
[Kontrol fazları](benchmarks/normal-merge-defer-20261114/01-commit/A2-control-joins.json) ·
[Aday fazları](benchmarks/normal-merge-defer-20261114/01-commit/B2-defer-joins.json)

## Kesintisiz disk ve dosya olayı

Kesintisiz disk aynı 50k temiz seed, bir snapshot okuyucusu ve
30 × %20 churn ile ölçüldü. Tur arası yazmasız pencere yok.
Karşılaştırma kontrolü, önceki doğruluk düzeltmeli oturumdan.
Bu tur kesintisiz yazıda yalnız aday kolu çalıştırıldı.

| Kesintisiz yazı | Önceki kontrol | Aday |
| --- | ---: | ---: |
| 1,3× tur sonu aşımı | 0/30 | 0/30 |
| En yüksek tur sonu / geçici tepe | 1,2847× / 1,5434× | 1,2974× / 1,5140× |
| Son tur retained / manifest-canlı MB | 384,64 / 384,02 | 379,10 / 377,92 |
| Son run | 92 | 86 |
| p95 / p99 / max (ms) | 92,45 / 1.184,29 / 2.195,61 | 77,99 / 1.081,74 / 1.861,60 |
| >1 s / 3.000 | 83 | 50 |
| İşlem/s | 1.624 | 1.844 |

En yüksek örnek 29. turda. ACK'ten 24,68 ms önce, manifest kuşağı
değişirken ölçülen retained toplamı 390,32 MB. Örneğin 1,3× çizgisine
uzaklığı yaklaşık 0,77 MB. Bu örnekte manifest-canlı toplamı
hesaplanamadı. Geçici tepe ve tur sonu çizgisi ayrı ölçümlerdir.
[Disk örnekleri ve tur özeti](benchmarks/normal-merge-defer-20261114/02-continuous/disk-summary.json)

Dosya olayında 24 aday ve 24 oturumluk reclaim-kapalı kontrol, aynı
50/500/1.500/3.000 ms takvimiyle işlendi. Her iki kol da prototip
ikilisini kullandı. Enqueue→doğru marker sonucu her olayda ≤1 s.

| Olay görünürlüğü, p50 / p95 / max (ms) | Önceki kontrol oturumu | Bu tur |
| --- | ---: | ---: |
| Reclaim açık | 57,84 / 110,47 / 119,31 | 58,79 / 122,56 / 131,67 |
| Reclaim kapalı | 44,22 / 105,37 / 132,53 | 61,07 / 137,92 / 217,34 |

Bu tur görünürlük başlangıcı kesintisiz kolun sequence 3101'deki
modeliydi. Önceki modelle hash eşit, fiziksel düzen 63 → 86 run ve
504 → 1.904 delta anahtarı. 64 reclaim yayımı oldu. Zamanlanan
aday olayların 11'i planlamaya, 0'ı rewrite/yayım/GC'ye denk geldi.
Sürücünün mevcut bileşik sonucu bu yüzden `gate=false`. Genel olay
görünürlüğü geçti, rewrite altında görünürlük **DOĞRULANAMADI**.
Bu noktada duruldu. Motor-only RAM kolu çalışmadı.
[Olaylar, aşamalar ve örtüşmeler](benchmarks/normal-merge-defer-20261114/03-visibility/01-arrival-summary.json)

## Kapılar ve karar

| Kontrol | Sonuç |
| --- | --- |
| Kontrollü testi, exact/capped, oracle ve process-kill | **GEÇTİ** |
| 50k commit, disk ve görünürlük kollarında 50k GET/version, 41 sorgu, kill/reopen ve eski okuyucu | **GEÇTİ 6/6** |
| Kesintisiz disk | **GEÇTİ 30/30** |
| Dosya olayında genel ≤1 s | **GEÇTİ 24/24**, kapalı kontrol 24/24 |
| Rewrite örtüşmesinde dosya olayı | **DOĞRULANAMADI**, örnek 0 |
| Motor-only ingest/idle private | **BAŞLAMADI**, 60/15 MiB kapısı bu aday için ölçülmedi |
| Flush sonrası beklemeyi kaldırma | **GEÇTİ**, bariyer ve profilde 0 join |
| Eski normal merge beklemeleri dahil toplam maliyet | Tekrarlı net kazanç kanıtlanmadı. Profilli toplam join arttı |

**Aday geçici derlemede kaldı.** Profilsiz kazanç ve disk koşusu
olumlu. Profilli çiftte toplam normal merge beklemesi arttı ve
rewrite altında görünürlük doğrulanamadı. Üretim varsayılanı için
bu kanıt yeterli değil. Yeni aday veya ölçüm açılmadı.

FDB slotu `ready`, 50.000 belge, sequence 3750. SQLite slotu,
korpus, önceki model ve 0.1 fixture korundu. Ölçümler
11:28:16–11:57:30 UTC. Tek release derlemesi 87,73 MiB,
kaynak kopyası yaklaşık 1,66 MiB. Tüm geçici prototip klasörü debug
test çıktılarıyla 2.429,74 MiB mantıksal dosya uzunluğu. Debug
doğrulama çıktısı planlamada öngörülenden büyük kaldı. Yeni kanıt
yaklaşık 75,09 MiB. Son FDB slotu 332,51 MiB, SQLite 491,41 MiB.
Geçici derleme klasörleri korundu.
[Yürütme kaydı](benchmarks/normal-merge-defer-20261114/status.json)
