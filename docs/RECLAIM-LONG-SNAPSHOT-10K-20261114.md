# Varsayılan reclaim ile 120 s snapshot — 10k

**Doğruluk kapısı KALDI, koşu durduruldu.** Snapshot bırakıldıktan
sonra yeni görünümde 10.000 GET/version ve 500 silinen anahtar kontrolü
geçti. Ardından ilk exact sorgu olan case 0'da (`olduğu`) motor
`Corrupt("checked page slot mismatch")` döndürdü. Sorgu digest'i,
process-kill/reopen ve dondurulmuş okuyucu aşamalarına geçilmedi.
[Ham olaylar](benchmarks/reclaim-followup-20261114/05-long-pin/long-pin-120s.jsonl) ·
[Hata](benchmarks/reclaim-followup-20261114/05-long-pin/long-pin-120s.stderr)

## Ölçülen disk ve snapshot davranışı

| An | Retained disk | Manifest-canlı disk | Pinli emekli dosyalar | Retained / canlı |
| --- | ---: | ---: | ---: | ---: |
| Snapshot başlangıcı | 61,18 MB | 61,18 MB | 0 | 1,000× |
| 5 s | 76,89 MB | 71,55 MB | 5,33 MB | 1,075× |
| 30 s | 76,89 MB | 71,55 MB | 5,33 MB | 1,075× |
| 120 s | 76,89 MB | 71,55 MB | 5,33 MB | 1,075× |
| Snapshot bırakıldıktan sonra | 71,55 MB | 71,55 MB | 0 | 1,000× |

Onaylı disk çizgisi ≤2×, üç zaman noktasında aşılmadı. Geçici
örnekli slot tepesi 81,26 MB. Aynı snapshot **120,12 s** tutuldu.
Generation 56'dan alınmıştı. Yeni manifest en geç 0,247. saniyede
generation 60 olarak görüldü. Eski aktif WAL'ın 5.333.225 baytı
bu örnekten 120. saniyeye kadar pinliydi. Tek dosya snapshot
bırakılınca silindi, silinmiş olduğu 1,68 ms içinde doğrulandı.
Release 1,63 ms, GC alt sayacı 1,46 ms. Release hata kaydı yok.
Eski snapshot sequence 20'de sekiz GET ve `olduğu` sonuç digest'i
korundu. Yeni görünüm sequence 40. Revision rewrite yayımı 0.
Snapshot'ın bir rewrite sonrası eski run dosyaları üzerindeki
etkisi bu koşuda bilinmiyor. Ölçülen pin maliyeti eski WAL'a ait.

120. saniye private **10,02 MiB**, working set **13,01 MiB**.
Proses OS tepe private/working set **19,86 / 24,21 MiB**.
Harness ve model bu sayılara dahil.

## Düzen ve kapanış

FDB 0.4.0, 881756DA… Cap200k korpusun ilk 10.000 belgesi, aynı
sahipli FDB slotu ve doğal sıcak cache. Tek pin, %20 churn, 20
adet 100'lük akışlı commit, yazılar sonrasında mevcut
`poll_maintenance()`. Ölçüm için yalnız geçici harness.
Deney 06:21:09–06:26:21 UTC, 8 dakika ve 32 MiB kanıt sınırı
içinde. Korpus, önceki model ve 0.1 fixture hash'leri korundu.
SQLite slotu aynı. **FDB slotu `blocked_partial_long_snapshot`**,
10k ve sequence 40. İkinci koşu ya da motor düzeltmesi yapılmadı.

Hata checked-page cache'de sorgu için tutulan kaynak/slot çifti
beklenen çiftle uyuşmadığında dönüyor. Bu gözlem fiziksel
dosya bozulmasını veya hatanın snapshot'tan kaynaklandığını
kanıtlamıyor. Neden bilinmiyor. [Koruma kontrolü](../src/segment/checked_page.rs#L72) ·
[Süre ve slot kaydı](benchmarks/reclaim-followup-20261114/05-long-pin/status.json) ·
[Harness](benchmarks/reclaim-followup-20261114/05-harness.rs.txt)
