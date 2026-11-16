# Varsayılan arka plan reclaim ile dosya olayı — 50k

**Görünürlük kapısı GEÇTİ.** FDB 0.4.0 varsayılan reclaim ile 24/24
dosya olayı 1 saniye içinde aramada göründü. 24 eşlenik kontrol de geçti.

| Aşama (ms) | Varsayılan p50 / p95 / max | Reclaim kapalı kontrol p50 / p95 / max |
| --- | ---: | ---: |
| Olay → ilk doğru sonuç | 44,86 / 84,34 / 106,28 | 49,49 / 88,73 / 105,66 |
| Kuyruk | 0,022 / 10,33 / 22,55 | 0,025 / 0,048 / 0,075 |
| Commit | 2,38 / 4,44 / 12,13 | 3,05 / 15,41 / 15,78 |
| Commit dönüşü → okuyucu başlangıcı | 27,77 / 63,43 / 90,48 | 34,28 / 69,96 / 75,05 |
| Snapshot edinme | 0,0002 / 0,0003 / 0,0006 | 0,0003 / 0,0005 / 0,0005 |
| İlk marker sorgusu | 12,18 / 13,92 / 15,33 | 10,82 / 14,47 / 15,17 |

Rewrite ile örtüşen sequence 3203, kuyruk **0,02 ms**, görünürlük
**48,86 ms**. Yayım ile örtüşen 3419/3634, kuyruk 5,13/22,55 ms,
görünürlük 32,28/64,71 ms. 11 olay planlamayla örtüştü. GC ile
örtüşen olay yok. Ölçüm sürecinde 46 revizyon çıktısı yayımlandı.
Olay örneklerinde gözlenen üst sınırlar işletim sözleşmesi değildir.
[48 ham olay ve sequence](benchmarks/reclaim-followup-20261114/arrival.jsonl) ·
[Özet](benchmarks/reclaim-followup-20261114/01-arrival-summary.json)

Aynı 881756DA… Türkçe 50k, yoklama başlangıcı sequence 3101, yeniden
ingest veya DB kopyası yok. 1 akışlı yazıcı ve kısa snapshot'lı 1
okuyucu, aynı 41 sorgu ve doğal sıcak cache. Altı çevrimde dört
eşlenik belge/donör olayı, kaydırmalar 50/500/1.500/3.000 ms. Sıra
dönüşümlü. Önceki arka plan prototipi başka başlangıç sequence'i
ile ölçülmüştü. Bu koşuda üretim 0.4.0 kaynaklarının geçici
derlemesine yalnız işçi/yazıcı fazı tanığı ve harness eklendi.
Dosya izleyicisi ve içerik çıkarma gecikmesi kapsam dışı.

**Doğruluk GEÇTİ.** 48 olay marker/snapshot oracle, process-kill sonrası
strict reopen ve dondurulmuş eski okuyucu geçti. Recovery sequence
3750, tam 50.000 GET/version ve 41 sorgu. Korpus ve 0.1 fixture aynı.
Ölçüm prosesi 69,88 s, örnekli slot tepe 432,36 MB. Derleme ve
doğrulama 22:07:27–22:11:41 UTC, 14 Kasım İstanbul. Kanıt dosyaları
yaklaşık 7,0 MiB. 15 dakika/64 MiB kanıt ve slot korumaları içinde.
[Yürütme ve uyumluluk](benchmarks/reclaim-followup-20261114/status.json)
