# İki işçi — temiz 50k/100k ve gerçek soğuk açılış

**KANIT, sıcak.** İki işçi temiz 50k’da strict-open medyanını **268,78 → 188,94 ms** (%29,7), temiz 100k’da **541,80 → 304,50 ms** (%43,8) indirdi. Her iki ölçekte 12/12 blok daha hızlı. Açılış CPU ve tepe private commit arttı. [İlk 52 run sıcak deneyi](STRICT-OPEN-TWO-WORKERS-50K-20261109.md)

## Sıcak temiz yerleşimler

Cap200k TR `881756DA…` korpusunun ilk 50.000 ve 100.000 belgesi. Tarihsel seed ikilisiyle, tek geçici benchmark slotu iki kez kullanıldı. Yerleşimler 3 ve 7 run, sequence 100 ve 200, delta 0. Seed doğrulaması tamamlandı, aktif 52 run slotu ve geçici temiz yerleşimlerin SHA’ları ölçüm öncesi/sonrası eşit. Geçici slot çalışma sonunda boş. Yeni korpus yok.

Bir ve iki işçi aynı release ikilisinde. Her yerleşim için 12 ABBA/BAAB blok, kol başına 24 ayrı proses, toplam 96 örnek. Blok öncesi mevcut DB dosyaları normal cached I/O ile okundu. WPR ve purge kapalı. TR case0 `olduğu`, melez N=50, strict Verified, 50 hit ve CRC32 4242832620 her örnekte. p95 24 örneğin 23. sırası. CPU süreç user+kernel ve bütün işçileri kapsar. RAM süreç sayaçlarıdır, OS dosya cache’i hariç. Sıcak pencere 18:32:01–18:35:15 UTC. [Pencere ve durum](benchmarks/strict-open-two-workers-followup-20261109/warm-status.json)

| Ölçüm | 50k tek işçi p50 / p95 | 50k iki işçi p50 / p95 | 100k tek işçi p50 / p95 | 100k iki işçi p50 / p95 |
| --- | ---: | ---: | ---: | ---: |
| Strict-open, ms | 268,78 / 283,50 | **188,94 / 224,44** | 541,80 / 576,83 | **304,50 / 320,95** |
| Run yükleme, ms | 267,18 / 281,74 | 187,15 / 222,24 | 540,25 / 575,26 | 302,73 / 319,10 |
| Maske, ms | 0,019 / 0,033 | 0,022 / 0,028 | 0,025 / 0,036 | 0,029 / 0,040 |
| Açılış → ilk sonuç, ms | 271,08 / 285,94 | **191,42 / 227,65** | 545,04 / 580,00 | **307,81 / 324,58** |
| Yalnız ilk sorgu, ms | 2,37 / 3,37 | 2,48 / 3,43 | 3,16 / 3,29 | 3,20 / 3,62 |
| Açılış CPU, ms | 265,63 / 281,25 | 281,25 / 343,75 | 539,06 / 578,13 | 578,13 / 609,38 |
| İlk sonuca kadar CPU, ms | 265,63 / 281,25 | 281,25 / 343,75 | 546,88 / 578,13 | 578,13 / 609,38 |
| Açılış tepe private, MiB | 2,539 / 2,578 | 2,832 / 3,648 | 4,004 / 4,102 | 4,846 / 5,113 |
| İlk sonuca kadar tepe private, MiB | 3,172 / 3,207 | 3,414 / 3,648 | 5,215 / 5,227 | 5,301 / 5,336 |
| Açılış tepe working set, MiB | 6,863 / 6,934 | 7,061 / 7,676 | 8,277 / 8,352 | 8,861 / 9,082 |

**KANIT.** Açılış tepe private p95 artışı **50k +1,070 MiB, 100k +1,012 MiB**. İlk sonuca kadar tepe private p95 artışı sırasıyla **+0,441 ve +0,109 MiB**. Açılış CPU p50 **+15,63 ve +39,06 ms**. Açılış blok farkı medyanı **−76,71 ve −238,94 ms**. Yalnız ilk sorgu p95’i 50k’da +0,06 ms ve 100k’da +0,33 ms. [50k ham](benchmarks/strict-open-two-workers-followup-20261109/clean-50000.jsonl) · [50k özet](benchmarks/strict-open-two-workers-followup-20261109/clean-50000-summary.json) · [100k ham](benchmarks/strict-open-two-workers-followup-20261109/clean-100000.jsonl) · [100k özet](benchmarks/strict-open-two-workers-followup-20261109/clean-100000-summary.json)

## Gerçek soğuk açılış planı

52 run’lı mevcut 50k kopya, aynı probe ve TR case0. Denge **1→2, 2→1**. Üst sınır 25 dk, 4 purge ve 4 WPR kaydı. Her kayıtta RAMMap `-Es` ve `-Et` sonrasında strict-open ve bir sorgu, kayıt ilk sonuçtan sonra durur. 32 MiB collector, önceki izlerle birlikte 320 MiB ETL ve 512 MiB soğuk kanıt bütçesi. CSV named pipe üzerinden süzülür. Açılış başlangıcı → ilk sonuç ve eski 2.000 ms çizgisi aynı. SQLite ve Q30 ayrı 2b kapsamındadır.

Geçerli iz için birleşik kayıp 0, 54/54 hedef dosya FileIO’su, dosya/PID’ye bağlanan fiziksel DiskIO, geçerli oracle ve değişmeyen envanter. **Collector bazı ölçülemedi.** İki kolda da WPR açık. Her kol en fazla iki soğuk örnek verdiği için ham süreler ve eşleşmiş farklar raporlanır. [Soğuk koşu planı](benchmarks/strict-open-two-workers-followup-20261109/cold-plan.json)

## Gerçek soğuk sonuç

**GEÇERSİZ İZ, 0/4 doğrulandı.** İlk tek işçili denemede iki RAMMap çağrısı ve WPR start exit 0. WPR stop **0x800705DE** ile döndü, tek collector dosyası **33.554.432 B** sınırında kaldı ve birleşik ETL oluşmadı. Stop öncesi izlenen en büyük boyut 17.825.792 B idi. Bu nedenle 54/54 dosya atfı, fiziksel DiskIO ve birleşik kayıp ölçülemedi. Kalan üç deneme ve iki işçili cold kolu başlamadı. WPR boşta. [Yönetici kaydı](benchmarks/strict-open-two-workers-followup-20261109/trial-01/admin-result.json) · [Soğuk koşu durumu](benchmarks/strict-open-two-workers-followup-20261109/cold-finished.json)

**SİNYAL.** Ham tek işçi süresi açılış **2.360,45 ms**, sorgu **170,50 ms**, açılıştan ilk sonuç **2.530,95 ms**. 50 hit ve CRC32 725765487 geçti. Bu ham değer doğrulanmış cold kıyasına veya 2.000 ms kapı kararına girmez. [Ham ilk örnek](benchmarks/strict-open-two-workers-followup-20261109/trial-01/cold.jsonl)

Soğuk pencere 18:35:24 UTC’de açıldı. İlk iz 18:49:58–18:50:04 UTC, sonlandırma kuralı uygulandı. Öncesinde Windows yol sıralaması yüzünden envanter karşılaştırıcısı erken durdu. Ayrı SHA kontrolünde 159/159 eşleşti, karşılaştırma sırası düzeltildi ve aynı süre penceresi korundu. Bu ön hata sırasında purge ve WPR yoktu. [Başlatma kaydı](benchmarks/strict-open-two-workers-followup-20261109/cold-pre-start-order-error.json)

**Disk.** Geçersiz collector korundu. Birleşik bütçe 459.688.483 B toplam ve 263.783.721 B ETL ile 512/320 MiB sınırlarının altında. Aynı dosya modunda 56 MiB collector ve bir eş boyutlu birleşik dosya için, diğer geçici paylardan önce, 112 MiB rezerv gerekir. Mevcut ETL bütçesindeki boşluk yaklaşık 68 MiB. Bu yöntemle yeni deneme başlatmak için bütçe veya iz yöntemi kararı gerekiyor. Geçici derleme klasörü yaklaşık 514 MiB. Üretim motoru ve prototipin kabul durumu değişmedi.

10 Kasım’da bu geçersiz collector doğrulanmış ZIP arşivine alındı. 56 MiB collector ile soğuk tekrar 4/4 geçerli sonuç verdi. [Tekrar ve arşiv](STRICT-OPEN-TWO-WORKERS-COLD-RETRY-20261110.md)
