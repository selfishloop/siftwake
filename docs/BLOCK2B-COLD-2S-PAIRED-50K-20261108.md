# Blok 2b, 2.000 ms ve iki dengeli çift — 50k, 8 Kasım 2026

**FDB ilk deneme GEÇTİ, SQLite izi GEÇERSİZ.** Yeni matris
**2/4 başlatıldı, 1/4 teknik geçerli, 1/4 süre geçişi**. İlk FDB
cold sonucu **1.868,88 ms**, 2.000 ms sınırının **131,12 ms** altında.
SQLite’ın ham ilk sonucu **4.622,54 ms**. SQLite kaydında collector
doldu ve birleşik iz oluşmadı. Durma kuralıyla ters çift başlamadı.

## Düzen

Kullanıcı iki çift (FDB→SQLite, SQLite→FDB), **56 MiB** collector ve
**320 MiB** toplam ETL sınırını onayladı. Genel kanıt tavanı **512 MiB**,
merge rezervi **128 MiB**. UTC pencere **20:07:38–20:37:38**, yeni
deneme kesimi **20:24:38**. Aynı Cap200k SHA 881756DA… ilk 50k,
sabit slotlar, yeniden ingest veya DB kopyası yok. FDB aynı frozen
ikili, strict open, melez200 N=50. SQLite aynı FTS5 trigram sorgu
planı, salt okunur tek read transaction ve önceki pragmalar. Aynı TR
case0 `olduğu`, önceki digest’ler her sorguda kontrol edildi.

Bir yönetici betiği her trial için hazır sinyalini ve önceki kapıyı
bekledi. Her iki denemede birer `-Es/-Et` dizisi ve birer WPR kaydı.
Kayıt stop isteği ilk sonuçtan sonra, cold 30 sorgu aynı süreçte.
Warm 30 + iki N60 A/A tamamen WPR kapalı, yalnız FDB’de tamamlandı.
[Koşu planı](benchmarks/cold-etw-paired-2s-20261108/run-plan.json)

## Süreler

| Ölçüm, ms | FDB cold, geçerli | FDB warm | SQLite cold, ham/geçersiz |
| --- | ---: | ---: | ---: |
| Açılış | 1.758,16 | 1.395,40 | 11,10 |
| İlk sorgu | 110,72 | 111,82 | 4.611,44 |
| **Açılıştan ilk sonuca** | **1.868,88** | **1.507,22** | **4.622,54** |
| Q30 | 131,50 | 102,81 | 365,16 |

SQLite warm, sıcak A/A ve ters sıra **ÖLÇÜLMEDİ**. Ham SQLite
süresi geçerli cold kıyasına girmez. Bu ham örnekte süre ilk
sorguda birikti. WPR etkisi ayrıştırılmadı.

| FDB sıcak A/A | N | p50 ms | p95 ms | Max ms |
| --- | ---: | ---: | ---: | ---: |
| A | 60 | 107,47 | 137,45 | 150,00 |
| B | 60 | 122,58 | 168,93 | 193,63 |

FDB cold Q30 WPR stop döndükten **2.554,19 ms** sonra başladı.
Q30 sayısal yakınsama kararı kullanıcıda. FDB cold ilk sonuç ve
warm ilk sonuç arasındaki fark 361,67 ms. Cold kolda WPR açıktı.
[Ham FDB cold](benchmarks/cold-etw-paired-2s-20261108/trial-01/cold.jsonl) ·
[Ham FDB warm](benchmarks/cold-etw-paired-2s-20261108/trial-01/warm.jsonl) ·
[A/A ve kapanış çakışması](benchmarks/cold-etw-paired-2s-20261108/trial-01/timing-summary.json) ·
[Ham SQLite cold](benchmarks/cold-etw-paired-2s-20261108/trial-02/cold.jsonl)

## İz ve doğruluk

| Kapı | FDB | SQLite |
| --- | --- | --- |
| Start, RAMMap ve stop | Exit 0, birleşik ETL 7.740.501 B | Start ve RAMMap exit 0. Stop **0x800705DE**, birleşik ETL yok |
| Collector | Gözlenen 12 MiB, sınır 56 MiB | Tek System Collector **58.720.256 B**, tam 56 MiB |
| Hedef FileIO ve fiziksel DiskIO | **54/54 + 54/54**, 52.888.064 B hedef storage | **ÖLÇÜLMEDİ** |
| Birleşik kayıp | **0**, collector bazı ölçülemedi | **ÖLÇÜLMEDİ** |
| 2.000 ms cold ilk sonuç | **GEÇTİ** | **GEÇERSİZ** |
| Zamanlı oracle | Cold 30 + warm 150 **GEÇTİ** | Ham 30 sorgu **GEÇTİ** |

FDB CSV akışı diske ham CSV yazmadan **174.125.445 B** işlendi.
Tracerpt özetinde 417.693, CSV’de 417.692 olay vardı. Fark bir
FileIO Create idi. Purge sonu→ilk sonuç penceresinde Create,
Read ve OperationEnd kimlikleri `Get-WinEvent` ile doğrulandı.
**147.098/147.098** olayın opcode, zaman, PID ve TID özeti birebir
eşleşti. Başlangıç sayım uyarısı bu iz için uzlaştırıldı.
[FDB dosya ve iz kanıtı](benchmarks/cold-etw-paired-2s-20261108/trial-01/decoded/identity.json) ·
[Olay kimliği uzlaştırması](benchmarks/cold-etw-paired-2s-20261108/trial-01/window-reconciliation.json) ·
[SQLite admin kaydı](benchmarks/cold-etw-paired-2s-20261108/trial-02/admin-result.json)

İki motorun başlangıç 50k GET/version ve 41 sorgu oracle’ı,
kill/reopen ve kapanış SHA kontrolleri geçti. FDB frozen eski
okuyucusu ayrıca geçti. SQLite kendi **0 B WAL** ve **32.768 B
SHM** dosyalarını ön doğrulama sırasında oluşturdu. Kullanıcı
izinli bu iki dosya kapanışta da vardı. Ana `sqlite.db` ve
iki model SHA’sı aynı, yan dosyalara elle işlem yapılmadı.
[Kill/reopen](benchmarks/cold-etw-paired-2s-20261108/post-verification.json) ·
[Eski okuyucu ve SHA](benchmarks/cold-etw-paired-2s-20261108/frozen-reader-verification.json) ·
[SQLite envanteri](benchmarks/cold-etw-paired-2s-20261108/sqlite-inventory.json)

WPR boşta. Başarısız SQLite collector dosyası korundu. Mevcut
bütçeler aşılmadı. 2b **KISMEN**, FDB’de bir geçerli süre geçişi
var, dengeli SQLite kıyası açık. Yeni deneme, disk churn ve C ABI
başlatılmadı.
[Nihai durum](benchmarks/cold-etw-paired-2s-20261108/final-verdict.json)


Kapanış **20:21:18 UTC**. Birleşik kanıt bütçesinde **426131800 B** toplam ve **230229289 B** ETL.
