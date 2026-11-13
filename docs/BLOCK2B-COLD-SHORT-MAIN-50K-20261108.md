# Blok 2b dar ETW ana koşusu — 50k, 8 Kasım 2026

**İlk geçerli FDB denemesinde süre kapısı KALDI.** Open-start → ilk
sonuç **1.893,59 ms**, sınır **1.000 ms**. Ana seri **1/8 başlatıldı,
1/8 teknik olarak geçerli, 0/8 süre kapısını geçti**. Durma kuralı
uygulandı ve SQLite performans kolu başlamadı.

## Düzen ve süre

UTC başlangıç **19:33:26**, yeni deneme kesimi **19:50:26**, toplam
deadline **20:03:26**. İlk admin penceresi **19:42:09,396–19:42:14,323**.
Aynı Cap200k 881756DA… ilk 50k, 52 run, FDB sequence 529967. Aynı
`olduğu` case0 ve melez200 N=50. Yeni DB kopyası, ingest, format veya
motor değişikliği yok. **1 purge dizisi, 1 WPR, 2 açılış, 180 zamanlı
sorgu.** Dört dengeli çiftin planı ve kapı [run-plan.json](benchmarks/cold-etw-short-main-20261108/run-plan.json) içinde.

WPR dar tek System Collector profilinde. Ön testten yalnız collector
tavanı **64→56 MiB** değişti. Önceki ETL’ler korunarak 256 MiB toplam
ETL sınırına 128 MiB kayıt/merge rezerviyle sığmak için bu tavan
kullanıldı. Gözlenen geçici collector **12 MiB**. RAMMap `-Es`, `-Et`,
WPR start ve stop exit 0. Kayıt/merge başarıyla kapandı. WPR stop
isteği ilk sonuçtan **41,38 ms** sonra, stop dönüşü **1.155,34 ms**
sonra. Q2–Q30 aynı süreçte bekleme eklenmeden sürdü. 10 sorgu WPR
kapanışı sırasında, 19 sorgu stop döndükten sonra bitti. Warm
açılış WPR kapandıktan ve **467.167.854 B** dosya okunduktan sonra
ayrı proseste çalıştı. [Yönetici kaydı](benchmarks/cold-etw-short-main-20261108/admin-result.json) ·
[Okuyucu kaydı](benchmarks/cold-etw-short-main-20261108/reader-result.json)

| FDB, ms | Cold | Warm |
| --- | ---: | ---: |
| Strict-open | **1.785,28** | **1.306,04** |
| İlk sorgu | 108,31 | 110,91 |
| **Open-start → ilk sonuç** | **1.893,59** | **1.416,95** |
| Q30 | **96,09** | **91,52** |

Cold ilk sonuç sınırı **893,59 ms** aştı. Cold ve warm farkı
**476,64 ms**. Açılış farkı **479,24 ms**. WPR yalnız cold kolda
olduğu için bu farkı cache’e veya WPR’ye tek başına atfetmiyoruz.

| Warm A/A, N=60 | p50 ms | p95 ms | max ms |
| --- | ---: | ---: | ---: |
| A | 93,48 | 96,12 | 97,31 |
| B | 93,40 | 94,99 | 96,30 |

Cold Q30, WPR stop döndükten **1.811,13 ms** sonra başladı.
Q30 sayısal yakınsama kararı kullanıcıda. Karşılaştırma tek çiftten.
[Cold 30 ham örnek](benchmarks/cold-etw-short-main-20261108/cold.jsonl) ·
[Warm 150 ham örnek](benchmarks/cold-etw-short-main-20261108/warm.jsonl) ·
[Zaman sınırları ve A/A](benchmarks/cold-etw-short-main-20261108/timing-summary.json)

## Cold kanıtı ve CSV uzlaştırması

| Kontrol | Sonuç |
| --- | --- |
| Beklenen dosya adı/FileObject/Read ve cold PID | **54/54**, PID 99900 |
| Fiziksel DiskIO bağı bulunan beklenen dosya | **54/54** |
| Tüm hedef dosyalara atfedilen storage | **54.369.792 B**, 158 dosya |
| Hedef öncesi yabancı erişim / belirsiz DiskIO | **0 / 0** |
| Birleşik kayıp | **0**, **collector bazı ölçülemedi** |
| Sıkıştırılmış ETL | **6.534.288 B** |
| Named pipe CSV | **143.572.252 B**, diske ham CSV yok |
| Tüm cold kanıtları dahil kapanış bütçesi | **130.214.100 B ETL / 325.771.732 B toplam** |

İlk filtre, ek CSV sayım kontrolü nedeniyle KALDI verdi. Tracerpt
özeti **341.099** işlenen olay, CSV **341.087** olay satırı.
Tekrar okunan tam CSV akışı aynı SHA. Provider GUID/ad gösterimleri
eşlendiğinde fark birer **Extension**, **EndExtension** ve **RDComplete**
ile **1 FileIO Create, 4 Read, 4 OperationEnd**. Yerel `Get-WinEvent`
ile aynı ETL’den 193.359 olayın zaman/PID/TID/opcode anahtarı
karşılaştırıldı. CSV’deki 193.350 olayın tümü eşleşti. Eksik dokuz
FileIO olayı ilk sonuçtan **190,82–191,04 ms sonra**. Dördü Read,
dördü OperationEnd cold PID’deydi ve ilk-sonuç penceresi dışındaydı.
Tek Create başka PID’deydi. Purge→ilk sonuç penceresinde bu üç
FileIO türünden CSV’de eksik olay yok. Bu iz için ilk filtre
sayım farkı bağımsız uzlaştırmayla kapatıldı. Neden bu satırların
CSV çıktısında görünmediği belirlenmedi.
[İlk filtre ve 54 dosya](benchmarks/cold-etw-short-main-20261108/decoded/identity.json) ·
[Sayım farkı](benchmarks/cold-etw-short-main-20261108/summary-reconcile.json) ·
[Dokuz olayın zamanları](benchmarks/cold-etw-short-main-20261108/fileio-reconcile.json)

## Doğruluk ve durma

- Hazırlıkta frozen verifier varsayılan kitap corpus’unu açtı.
  Yanlış giriş reddedildi ve zamanlı deneme başlamadan 881756DA…
  SHA’lı yol ve katalog sabitlendi. Tam **50k GET/version ve 41**
  sorgu oracle’ı geçti. Hatalı hazırlık logu ayrı saklandı.
- **180/180 case0 oracle** geçti. FDB okuyucu kill tanığı ve
  reopen 50k/41 oracle’ı geçti. İlk eski okuyucu koşusu farklı
  satır şeması verdi. Doğru frozen eski okuyucu yoluyla tam
  **50k/41 digest** ayrıca doğrulandı.
- FDB **159/159** dosya SHA’sı ve model SHA eşleşti. SQLite
  ana DB SHA ve mevcut tek dosyalı envanter eşleşti. SQLite’a
  bağlantı açılmadı, WAL/SHM yan dosyaları yeniden oluşturulmadı.
- İzole probe release, rustfmt, clippy `-D warnings` ve PowerShell
  AST geçti. WPR kapalı. Motor, eşik ve varsayılanlar aynı.

**Blok 2b KISMEN, cold süre kapısı KALDI.** Ana seri devam ettirilmedi.
50k disk churn ve C ABI başlamadı.
[Son bütünlük](benchmarks/cold-etw-short-main-20261108/post-verification.json) ·
[Nihai karar](benchmarks/cold-etw-short-main-20261108/final-verdict.json)


Rapor kapanışı **19:49:16 UTC**.
