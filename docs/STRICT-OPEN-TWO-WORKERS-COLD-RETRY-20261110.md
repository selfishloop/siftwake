# İki işçi — 56 MiB collector ile soğuk tekrar

## Arşiv ve disk bütçesi

Dört geçersiz ham collector, özgün göreli yolları ve SHA-256 değerleriyle tek ZIP arşivine alındı. Her giriş arşivden okunup SHA-256’sı özgün dosyayla eşleştirildi. Ardından yalnız bu dört özgün dosya arşivle değiştirildi. Geçerli birleşik ETL’ler, ham sorgu süreleri, oracle ve WPR kayıt özetleri yerinde duruyor.

| Arşive alınan geçersiz collector | Özgün boyut |
| --- | ---: |
| 8 Kasım FDB/SQLite çifti, SQLite | 56 MiB |
| 7 Kasım sıcak WPR deneyi | 48 MiB |
| 8 Kasım 2 saniye soğuk tekrarı | 32 MiB |
| 9 Kasım iki işçi takip koşusu, tek işçi | 32 MiB |

176.160.768 B özgün veri, 18.833.206 B ZIP. **157.327.562 B (150,04 MiB)** yer açıldı. Tekrar bütçesi arşivin gerçek boyutunu da kapsar. Başlangıç envanterinde yaklaşık 296 MiB toplam, 84 MiB doğrudan ETL vardı. Collector ve birleşik kayıt için 116 MiB rezerv. Toplam 512 MiB, doğrudan ETL 320 MiB sınırları aynı. [Arşiv manifesti](benchmarks/strict-open-two-workers-cold-retry-20261110/archive-manifest.json) · [ZIP arşivi](benchmarks/strict-open-two-workers-cold-retry-20261110/failed-collector-archive.zip)

## Tekrar düzeni

Pencere **15:27:01–15:52:01 UTC**, yeni deneme son saati **15:47:01 UTC**. Aynı 52 run’lı 50k FDB kopyası, aynı frozen iki işçi probe, TR case0 `olduğu`, melez N=50. Sıra **1→2, 2→1**. En fazla dört purge ve dört kısa WPR kaydı. Kayıt, `-Es/-Et` ve açılıştan ilk sonuca kadar olan pencereyi kapsar. Yeni DB kopyası veya ingest yapılmadı. Sıcak testler önceki raporda.

Geçerli cold kanıtı için 54/54 hedef FileIO dosya/PID atfı, hedefe bağlanan fiziksel DiskIO, birleşik olay kaybı 0, strict Verified ve sonuç oracle’ı. Collector bazı ölçülemedi. Teknik olarak geçersiz izde sıradaki deneme başlamaz. 2.000 ms ilk sonuç çizgisi ayrı raporlanır. Bu deney iki FDB açılış yolunu karşılaştırır. SQLite ve Q30 önceki 2b kapsamındadır. [Koşu planı](benchmarks/strict-open-two-workers-cold-retry-20261110/cold-plan.json) · [Sıcak sonuçlar](STRICT-OPEN-TWO-WORKERS-FOLLOWUP-20261109.md)

## Sonuç

**KANIT, bu 50k yerleşimi için.** Dört iz geçerli. Açılıştan ilk sonuca, iki işçi birinci çiftte **280,78 ms (%25,2)**, ters çiftte **323,59 ms (%30,6)** kazandırdı. 2.000 ms çizgisini dört deneme de geçti. Her kolda iki örnek olduğu için p95 çıkarılmadı. Her iki kolda da WPR açıktı. WPR’nin mutlak açılış süresine etkisi bu turda ayrıştırılmadı.

| Deneme | İşçi | Strict-open ms | Run yükleme ms | Maske ms | Sorgu ms | Açılış → ilk sonuç ms | Açılış CPU ms | Açılış tepe private MiB | İlk sonuç tepe private MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1 | 1.006,16 | 793,27 | 203,50 | 109,74 | 1.115,91 | 859,38 | 8,191 | 9,496 |
| 2 | 2 | 727,95 | 518,30 | 194,92 | 107,18 | **835,13** | 921,88 | 8,199 | 9,734 |
| 3 | 2 | 632,88 | 450,31 | 169,32 | 100,47 | **733,34** | 812,50 | 9,176 | 9,598 |
| 4 | 1 | 956,85 | 769,58 | 173,47 | 100,08 | 1.056,93 | 765,63 | 8,016 | 9,684 |

İki işçi run yüklemeyi her iki çiftte kısalttı. Açılış CPU farkı **+62,50 ve +46,88 ms**, açılış tepe private farkı **+0,008 ve +1,160 MiB**. Açılış tepe working set farkı **+0,020 ve +0,609 MiB**. Sıcak 3 run 50k ve 7 run 100k’da 12/12 blok kazanımı ile birlikte bu sonuçlar iki işçili prototipin ölçüm kabulünü destekliyor. Üretim kodu, varsayılanlar ve format aynı.

| Deneme | Hedef FileIO | Hedefe atfedilen fiziksel okuma (B) | Birleşik kayıp | CSV toplam farkı | Korunan I/O penceresi eşleşmesi |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 54/54 | 53.304.320 | 0 | 10 | 195.908/195.908 |
| 2 | 54/54 | 54.175.744 | 0 | 4 | 164.303/164.303 |
| 3 | 54/54 | 53.036.032 | 0 | 4 | 142.681/142.681 |
| 4 | 54/54 | 53.036.544 | 0 | 4 | 143.637/143.637 |

İlk koşu, CSV toplamı ETL özetinden 10 düşük çıkınca sonraki denemeyi armlamadan durdu. Mevcut ETL çevrimdışı yeniden okundu. Purge sonundan ilk sonuca kadar FileIO Create/Close/Read/OperationEnd ve DiskIO Read/ReadInit için sağlayıcı, opcode, UTC tick, PID ve TID özeti `Get-WinEvent` ile satır sayısı ve SHA-256 düzeyinde eşleşti. 54 hedef dosya, fiziksel okuma ve sıfır birleşik kayıp koşulları da geçti. Bu yüzden ilk örnek doğrulandı. Aynı pencere kontrolü diğer üç izde de geçti. Ölçüm kaldığı yerden ve aynı süre penceresinde sürdü, ilk örneğin purge veya kaydı tekrarlanmadı. **Collector bazı ölçülemedi.** [İlk duruş](benchmarks/strict-open-two-workers-cold-retry-20261110/cold-initial-stop.json) · [İlk pencere karşılaştırması](benchmarks/strict-open-two-workers-cold-retry-20261110/trial-01/window-reconciliation-io.json) · [Dört iz kararı](benchmarks/strict-open-two-workers-cold-retry-20261110/cold-finished.json)

Kayıtlar **15:42:28 UTC**’de doğrulandı. Dört `-Es/-Et` dizisi ve dört WPR kaydı, tüm start/stop exit 0. WPR boşta. Son yönetici bütçe kontrolü **337.242.555 B toplam, 114.258.034 B ETL**. 512/320 MiB sınırları korundu. Her örnek strict Verified, 50 hit, CRC32 725765487, sequence 529967. Aktif slot, ölçüm kopyası ve model SHA’ları her izden sonra eşleşti. [Ham süreler 1](benchmarks/strict-open-two-workers-cold-retry-20261110/trial-01/cold.jsonl) · [2](benchmarks/strict-open-two-workers-cold-retry-20261110/trial-02/cold.jsonl) · [3](benchmarks/strict-open-two-workers-cold-retry-20261110/trial-03/cold.jsonl) · [4](benchmarks/strict-open-two-workers-cold-retry-20261110/trial-04/cold.jsonl)

Kapanışta iki işçili adayın process-kill ve reopen koşusu, ardından dondurulmuş eski okuyucu geçti. Her ikisi de 50.000 GET/version ve 41 sorgu digest’ini doğruladı. Aktif slotun 159 dosyası ve model SHA’sı aynı. [Kapanış doğrulaması](benchmarks/strict-open-two-workers-cold-retry-20261110/verification.json)
