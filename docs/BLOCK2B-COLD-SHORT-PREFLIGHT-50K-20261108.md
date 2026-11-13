# Blok 2b kısa cold ETW ön testi — 8 Kasım 2026

**Ölçüm başlamadı.** Yönetici PowerShell betiği ayrılan pencerede
çalıştırılmadı ve okuyucu purge sinyali beklerken süre sınırında çıktı.
**0 purge, 0 WPR kaydı, 0 cold okuyucu açılışı.** Son `wpr -status`
sonucu `WPR is not recording`.

## Hazırlanan düzen

- UTC pencere **18:53:31–19:08:31**. Yönetici başlatma sonu **19:05:31**.
  Salt normal kullanıcı süreci **18:56:30 UTC**’de hazırlandı ve
  **19:06:00 UTC**’de kapandı.
- Mevcut ilk 50k, 52 run, sequence 529967. Kabul edilen 159 dosya
  envanteri ve model SHA önceden doğrulandı. Kopya veya ingest yapılmadı.
  Tek çağrılık case0 benchmark ikilisi izole derlemede oluşturuldu.
  Oracle beklenen sonuçları 50 hit, CRC32 725765487 ve 50.000 belge.
- Yerel `wpr -profiledetails` ile doğrulanan tek 64 MiB Sequential
  System Collector profili `DiskIO`, `DiskIOInit`, `FileIO`,
  `FileIOInit` ve `ProcessThread` olaylarıyla sınırlandı. Plan bir
  `-Es/-Et` dizisi ve tek ilk sonuç. Warm/Q30 bu teknik ön testte yok.
- Yönetici ve okuyucu betiklerinin AST kontrolü geçti. RAMMap 1.63
  imzası geçerli. Koşucu yönetici değil. Yönetici betiği tek satırla
  paylaşılmıştı ve süre kontrolü nedeniyle bu çağrı artık kapalı.
- Çözümleme mevcut named pipe düzeneğini kullanacak şekilde hazırlandı.
  Filtre cold penceresinde aynı PID’de Create → FileObject → Read bağı,
  FileKey/IRP üzerinden hedef DiskIO, birleşik kayıp ve olay sayısını
  kontrol eder. `QueryDosDeviceW` dosya yollarını yerel cihaz adına bağlar.
  Eski izdeki sonraki PID kayıtları yeni cold bağına katılmaz.

| Ön test kapısı | Sonuç |
| --- | --- |
| 54/54 dosya ve cold PID | **ÖLÇÜLMEDİ** |
| Hedef storage DiskIO, birleşik kayıp 0, collector sınırı | **ÖLÇÜLMEDİ** |
| Case0 oracle ve açılıştan ilk sonuç | **ÖLÇÜLMEDİ** |
| Ana 2b | **0/8 geçerli**, kapı ve yakınsama kuralı aynı |

Ana koşu, disk churn ve C ABI başlamadı. Kısa kayıt için yeni
hazırlık penceresi, yönetici PowerShell hazırken açılabilir.
[Okuyucu kapanış kaydı](benchmarks/cold-etw-short-preflight-20261108/reader-result.json) ·
[Doğrulanan WPR profili](benchmarks/cold-etw-short-preflight-20261108/cold-identity.wprp) ·
[Önceki tam iz tanısı](BLOCK2B-COLD-ETW-OFFLINE-50K-20261108.md)


Son salt okunur doğrulamada FDB 159/159 ve model SHA eşleşti. İzole derleme klasörü silinmedi. Boyutu 81986880 B.
C:\Users\Serkan\AppData\Local\Temp\fdb-cold-short-20261108-185331
Rapor kapanışı **19:07:08 UTC**.
[Kapanış kanıtı](benchmarks/cold-etw-short-preflight-20261108/closure.json)
