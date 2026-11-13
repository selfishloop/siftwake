# Blok 2b kısa cold ETW ön testi — gerçekleşen koşu — 8 Kasım 2026

**Teknik ön test GEÇTİ.** Cold PID ve açılış penceresinde **54/54**
beklenen kaynak ad/nesne/okuma eşleşmesi sağlandı. 54 beklenen
dosyanın her birine DiskIO da bağlandı. Birleşik kayıp **0**.
İlk sonuç bu tek gözlemde **2.030,38 ms**, 1 s çizgisinin üstünde.
Ana FDB/SQLite serisi henüz başlamadı ve Blok 2b açık.

## Koşu ve kaynak doğrulaması

- UTC pencere **19:12:15–19:27:15**. Yönetici betiği **19:13:14,775**
  UTC’de başladı. `-Es`, `-Et`, WPR start ve stop exit 0.
  Stop isteği **19:13:18,767 UTC**, aktif kayıt isteğinden stop
  isteğine **3,856 s**. WPR sonunda boşta. **1 purge dizisi, 1 kayıt,
  1 FDB açılışı ve 1 sorgu.**
- 881756DA… ilk 50k, 52 run, sequence **529967**. Önceki izole
  okuyucu ikilisi aynı SHA ile kullanıldı. Yeni kopya, ingest veya
  üretim kodu değişikliği yok. Case0 `olduğu`, melez200 N=50,
  50 hit, CRC32 **725765487**, 50.000 belge ve strict Verified
  oracle’ı geçti. Slotun 159 dosya SHA’sı ve model SHA sonunda aynı.
- Tek System Collector, **64 MiB** Sequential sınır. Gözlenen
  geçici collector tepe boyutu **16 MiB**. Başlatma/durdurma ve
  birleşim başarılı, kapasite hatası görülmedi. Sıkıştırılmış
  ETL **8.540.666 B**. Önceki kanıtlar dahil admin kapanışında
  **123.679.812 B ETL**, **302.282.501 B** toplam. Her ikisi de
  256/512 MiB sınırlarının altında.
- Named pipe ile **177.769.976 B** CSV akışta okundu. Ham CSV
  disk dosyası oluşturulmadı. `QueryDosDeviceW` ile cihaz adı,
  Create → FileObject → Read ile PID, DiskIO ReadInit/Read
  ile FileKey/IRP ve thread bağı kuruldu.
[Admin kaydı](benchmarks/cold-etw-short-preflight-live-20261108/admin-result.json) ·
[Okuyucu kaydı](benchmarks/cold-etw-short-preflight-live-20261108/cold.jsonl) ·
[Son envanter](benchmarks/cold-etw-short-preflight-live-20261108/reader-result.json)

## Kanıt ve süreler

| Ölçüm | Sonuç |
| --- | --- |
| Cold PID | 106864 |
| Beklenen FileIO ad/PID/nesne eşleşmesi | **54/54** |
| Fiziksel DiskIO tanığı bulunan beklenen dosya | **54/54** |
| Tüm hedef dosyalara bağlanan storage | **53.059.584 B**, 158 dosya |
| Hedef öncesi yabancı erişim ve belirsiz DiskIO atfı | **0 / 0** |
| Cold PID’de Create bağı kurulmamış diğer Read satırları | 41, 54 dosyanın tanığına sayılmadı |
| Birleşik ETL olay kaybı | **0**, **collector bazı ölçülemedi** |
| Strict-open | **1.917,87 ms** |
| İlk sorgu | **112,51 ms** |
| Open-start→ilk sonuç | **2.030,38 ms** |

DiskIO tanığı dosyaya yapılan fiziksel okumayı gösterir. Bir dosyanın
tüm sayfalarının cache dışında olduğunu ölçmez. SQLite, Q30 ve sıcak
A/A teknik ön test kapsamında çalıştırılmadı. 2.030,38 ms tek gerçek
ön test gözlemidir. Ana serinin **≤1 s** ve 30. arama yakınsama kapıları
değişmedi. [Dosya başına tanıklar ve ilk filtre kararı](benchmarks/cold-etw-short-preflight-live-20261108/decoded/identity.json)

## Dört CSV olayının uzlaştırılması

İlk filtre, kendi ek sayım kontrolünde durdu. `tracerpt` özeti
**426.090** işlenen olay bildirirken CSV **426.086** olay satırı
verdi. İkinci tam akış aynı bayt sayısı ve SHA ile bitti.
Başlık ve boş satır dışındaki tüm CSV satırları zaman damgalı.
Ad/GUID gösterimleri eşlendikten sonra fark tam dört olaydan oluştu.

- Birer **EventTrace Extension, EndExtension ve RDComplete**.
- Bir **FileIO Create**. Aynı ETL’ye yerel `Get-WinEvent` ile
  bakıldı. 22.010 Create’in 22.009’u CSV ile zaman/PID/TID
  anahtarında birebir eşleşti. CSV’de olmayan olay **PID 104012**,
  cold pencere başlangıcından **2.230,22 ms** sonra, yani ilk
  sonuçtan yaklaşık **200 ms sonra**. Purge sonrası ön pencereye
  ve cold PID’ye ait değil.

Birleşik kayıp 0 ve 54/54 tanık korunuyor. Teknik ön test kararı
yalnız bu iz için bağımsız uzlaştırmayla verildi. İlk ham filtre
kararı aynen saklandı. `tracerpt` çıktısında bu dört olayın
neden görünmediği belirlenmedi.
[Olay sayısı uzlaştırması](benchmarks/cold-etw-short-preflight-live-20261108/summary-reconcile.json) ·
[Tek Create farkı](benchmarks/cold-etw-short-preflight-live-20261108/create-reconcile.json) ·
[Nihai teknik karar](benchmarks/cold-etw-short-preflight-live-20261108/preflight-verdict.json)

Ana 2b **0/8 başlatıldı**, **0/8 geçerli**. Ana koşu ve 50k disk
churn ayrı onay bekliyor. C ABI başlamadı.


Rapor kapanışı **19:19:28 UTC**.
