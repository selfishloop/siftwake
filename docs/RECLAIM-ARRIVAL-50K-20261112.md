# Bakım sürerken gelen dosya olayı — 50k

**Bakım sırasında gelen 24 olayın tamamı 1 saniyeyi aştı.** Görünürlük
medyanı **9,07 s**, p95 **12,40 s**. Aynı 24 kontrol olayı için
**66,76 / 145,27 ms**. Toplam görünürlük süresinin **%99,24’ü** senkron
bakımın kalan süresiydi. [Ham sonuç ve eşleşmeler](benchmarks/reclaim-arrival-50k-20261112/summary.json)

## Düzen

Mevcut Cap200k Türkçe 50k sabit FDB slotu. Başlangıç `4-current`, sequence
3101. Slot yeniden kurulmadı. Var olan `idle-only` geçici motoruna yalnız
ayrı ölçüm örneği eklendi. Reclaim ve küçük akışlı WAL testte açık. Bir
yazıcı, tek kısa snapshot okuyucusu, aynı 41 sorgu döngüsü. Eski aday
ikilisinin SHA’sı korundu. [Önceki aday](RECLAIM-IDLE-ONLY-50K-20261112.md) ·
[Test aracı](benchmarks/reclaim-arrival-50k-20261112/harness.rs.txt)

Altı eşleştirilmiş çevrim. Her çevrimde mevcut donörlerle %20 churn,
100 adet 100’lük akışlı commit. Ardından bakım çağrısı içeren ve içermeyen
iki olay dizisi, sırayla bakım/kontrol ve kontrol/bakım. Her dizide
50, 500, 1.500 ve 3.000 ms zamanlarında ayrı bir üretici thread olay
gönderir. Bir çevrimin iki dizisinde aynı dört anahtar ve aynı donör
gövdesi kullanılır. Her olay tek belge update ve yeni bir marker. Sayısal
anahtarlar kullanıldı. Dosya gözleyicisi veya içerik çıkarma gecikmesi bu
saate dahil değil. Üst sınır 15 dakika, altı bakım, 48 olay.

Gerçek gönderim anı bakım başlangıcı/dönüşü arasında ise olay bakım
grubunda. Dışarıda kalanlar ayrı sınıflandırılır. Tek yazıcı bakım dönüşünde
kuyruğu işlemeye başlar. Olay zamanı → commit başlangıcı → commit dönüşü
→ okuyucunun sorguya başlaması → snapshot edinme → ilk doğrulanmış hit.
Eski marker ve tam GET/version kontrolleri zamanlanan olaylar gözlendikten
sonra yapılır. Böylece bu kontroller bir sonraki olayın ilk sorgusuna
eklenmez. [Sürücü](benchmarks/reclaim-arrival-50k-20261112/run.py) ·
[Ham saatler](benchmarks/reclaim-arrival-50k-20261112/queue.jsonl)

## Sonuç

Donör metni **674–49.288 bayt**. Altı bakım çağrısı **7,20–12,48 s**,
medyan **10,04 s**. Planlanan bakım olaylarının 24/24’ü gerçek başlangıç
ve dönüş arasına düştü. Bakım grubunun en hızlı olayı **4,28 s**.
Yüzdelikler en yakın üst sıra. N=24’te p95, 23. sıralı örnek.

| Aşama (ms) | Bakım sürerken p50 / p95 / max | Bakım yokken p50 / p95 / max |
| --- | ---: | ---: |
| Olay → aramada ilk sonuç | **9.070,60 / 12.397,83 / 12.479,59** | **66,76 / 145,27 / 202,05** |
| Olay → commit başlangıcı | 9.026,96 / 12.359,03 / 12.425,13 | 0,035 / 0,078 / 6,60 |
| Kuyruk içinde kalan bakım | 9.011,41 / 12.359,03 / 12.425,13 | — |
| Commit | 3,72 / 20,22 / 29,24 | 19,84 / 29,40 / 33,89 |
| Commit dönüşü → okuyucu başı | 24,41 / 107,21 / 122,91 | 28,67 / 131,85 / 137,10 |
| Snapshot edinme | 0,0004 / 0,0008 / 0,014 | 0,0004 / 0,0005 / 0,0006 |
| Yalnız ilk marker sorgusu | 6,77 / 20,04 / 25,80 | 9,74 / 46,10 / 50,66 |

| Bakım başlangıcından sonraki planlı olay anı | Bakım varken görünürlük p50 / max | Bakım yokken p50 / max |
| --- | ---: | ---: |
| 50 ms | 10,03 / 12,48 s | 35,75 / 106,33 ms |
| 500 ms | 9,59 / 12,04 s | 57,98 / 145,27 ms |
| 1.500 ms | 8,59 / 11,05 s | 58,61 / 89,20 ms |
| 3.000 ms | 7,09 / 9,56 s | 98,45 / 202,05 ms |

**Kanıt.** 24 olay da bakım döndükten sonra commit’e başlayabildi. Kalan
bakım, tüm bakım olaylarının toplam sürelerinin %99,24’ü. Commit ve
snapshot hızlı kalmasına rağmen mevcut senkron çağrı, ölçülen olaylarda
Frunner’ın 1 s görünürlük hedefini karşılamadı. Önceki 30 turdaki
yaklaşık 6,3 s bakım ortalaması başka bir yerleşim ve protokole ait.
Burada olaylar bakımın ilk üç saniyesine dağıtıldı. Bakım bitimine çok
yakın gelen olaylar veya Frunner’ın gerçek olay sıklığı ölçülmedi.

## Doğrulama ve kapanış

48 olayın her biri doğru yeni sequence, güncel GET/version ve eski
marker yokluğu kontrolünü geçti. Ayrı dört işlemlik commit tanığı
sonrası process-kill ve strict reopen **GEÇTİ**. 50.000 tam GET/version,
90.002 silinmiş anahtar, 41 exact sorgu ve 24 örnekli marker geçti.
Dondurulmuş okuyucunun 50.000 GET/version ve 41 sorgu digest’i eşit.
Sabit FDB slotu `ready`, sequence 3750. Üretim kaynağı, korpus, 0.1
fixture, önceki model ve önceki aday ikilisinin SHA’ları aynı. Geçici
derlemede fmt ve Clippy geçti. Geçici klasör bu ölçüm örneğiyle toplam
593.347.671 B (565,86 MiB). Çalıştırma ve doğrulama yaklaşık dört dakika,
18:48:35–18:52:22 UTC. [Yürütme durumu](benchmarks/reclaim-arrival-50k-20261112/status.json) ·
[Eski okuyucu](benchmarks/reclaim-arrival-50k-20261112/queue-legacy.jsonl)
