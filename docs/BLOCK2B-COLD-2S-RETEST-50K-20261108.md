# Blok 2b, 2.000 ms kapısıyla tekrar — 50k, 8 Kasım 2026

**İlk tekrar teknik olarak geçersiz.** FDB ham ilk sonuç **1.895,46 ms**.
Bu değer yeni **2.000 ms** çizgisinin **104,54 ms** altında, ancak
System Collector **32 MiB** sınırında doldu. WPR birleşik izi
kaydedemedi. **1/8 başlatıldı, 0/8 geçerli**, SQLite ve warm kolu
başlamadı.

## Kapı ve düzen

Kullanıcı 8 Kasım’da Open-start → ilk sonuç kapısını **1.000→2.000 ms**
olarak değiştirdi. Sürenin başlangıcı ve bitişi, cold dosya/PID,
DiskIO ve birleşik kayıp koşulları ile Q30 yakınsama kararı aynı.
Önceki **1.893,59 ms** geçerli FDB ölçümü eski 1.000 ms kapısında
KALDI olarak durur. Yeni kapı altında sayı 2.000 ms’nin altındadır.
Kullanıcı yeni koşu istediği için tarihsel örnek tekrar sayılmadı.
[Önceki ana rapor](BLOCK2B-COLD-SHORT-MAIN-50K-20261108.md)

UTC başlangıç **19:58:36**, deneme kesimi **20:15:36**, deadline
**20:28:36**. Aynı 881756DA… ilk 50k, 52 run, sequence 529967,
case0 `olduğu`, melez N=50. Aynı frozen FDB ölçüm ikilisi.
50k GET/version ve 41 sorgu hazırlık oracle’ı geçti. DB kopyası,
ingest, bakım veya motor değişikliği yapılmadı. Kullanıcı SQLite’ın
kendi WAL/SHM yan dosyalarını oluşturmasına izin verdi. SQLite kolu
başlamadığı için bu izin kullanılmadı.
[Koşu planı](benchmarks/cold-etw-2s-main-20261108/run-plan.json) ·
[Hazırlık oracle’ı](benchmarks/cold-etw-2s-main-20261108/fdb-before.jsonl)

## Ham FDB

| Ölçüm | Geçersiz cold kol, ms |
| --- | ---: |
| Strict-open | 1.789,32 |
| İlk sorgu | 106,14 |
| **Open-start → ilk sonuç** | **1.895,46** |
| 30. sorgu | 95,72 |

30 ham sorguda sequence 529967, 50 hit, CRC32 725765487 ve strict
Verified oracle’ı geçti. Warm ve N60 A/A yok, yakınsama ölçülmedi.
[Ham 30 sorgu](benchmarks/cold-etw-2s-main-20261108/cold.jsonl)

## Collector ve bütçe

- Dar profilin provider/keyword listesi aynı. Önceki ana koşunun
  **56 MiB** tavanı, eski ETL’leri koruyarak 8 denemeye daha fazla
  disk payı bırakmak için bu tekrarda **32 MiB** seçildi. Önceki
  kısa kayıtlarda gözlenen tepe **12–16 MiB** idi. Bu seçim yeni
  kayıtta yetersiz kaldı.
- WPR start, RAMMap `-Es` ve `-Et` exit 0. Stop isteğine
  **3,457 s**. Stop **0x800705DE**, Windows açıklaması
  “The event log file is full.” Tek geçici System Collector
  **33.554.432 B**, tam 32 MiB. Canlı örneklemede en çok 11 MiB
  görülmüştü. WPR son kontrolde boşta.
- Birleşik ETL **0 B**, 54/54 ve birleşik kayıp **ÖLÇÜLMEDİ**.
  Kısmi collector dosyası saklandı. 1 purge dizisi ve 1 WPR
  oturumu. Yeni deneme başlatılmadı.
- Tüm önceki cold kanıtları dahil kapanışta **163.768.532 B ETL**
  ve **359.375.837 B** kanıt (admin kapanışındaki sayaçlar).
  256/512 MiB tavanları aşılmadı. Mevcut ETL birikimiyle eski
  56 MiB tavanının 128 MiB kayıt/merge rezervi, ETL bütçesini
  **29.550.804 B** aşar. Eski kanıt ve bütçe kuralı korunuyor.
[Admin ve disk kaydı](benchmarks/cold-etw-2s-main-20261108/admin-result.json) ·
[Okuyucu durma kaydı](benchmarks/cold-etw-2s-main-20261108/reader-result.json)

## Son doğrulama ve durum

FDB kill/reopen 50k GET/version ve 41 sorgu, frozen eski okuyucu
50k/41 digest, FDB slot 159/159 SHA ve model SHA geçti. SQLite’a
bağlantı açılmadı, ana dosya SHA ve önceki tek dosyalı envanter aynı.
**2b açık**, güncel kapı **2.000 ms**, bu tur için ölçüm kararı
**GEÇERSİZ**. Disk churn ve C ABI başlamadı.
[Son doğrulama](benchmarks/cold-etw-2s-main-20261108/post-verification.json) ·
[Nihai durum](benchmarks/cold-etw-2s-main-20261108/final-verdict.json)


Rapor kapanışı **20:04:45 UTC**.
