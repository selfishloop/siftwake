# Varsayılan reclaim ile 100'lük commit profili — 50k

0.4.0, aynı 881756DA… ilk 50k, temiz seed, 1 kısa snapshot okuyucusu
ve 1 akışlı yazıcı. Aynı 41 sorgu, doğal sıcak cache. Her kolda
30 tur, 100 adet 100'lük commit/tur ve iki saniye yazmasız pencere.
Tek geçici ikili, A/B/B/A. A'da trace kapalı, B'de sınırlı commit
ve işçi trace'i açık. Motorun üretim kaynağına değişiklik yok.
[Sürücü](benchmarks/reclaim-followup-20261114/02-run.py) ·
[Geçici derleme farkı](benchmarks/reclaim-followup-20261114/02-measurement.diff.txt)

İlk hazırlıkta sampler reset'ten hemen sonra henüz oluşturulmamış
slotu açmaya çalıştı. İkinci hazırlıkta önceden oluşturulan boş
klasörü seed'in sahiplik kontrolü reddetti. İki hazırlıkta 0 ölçülen
commit ve 0 seed model. Seed mevcut sahiplik reset'iyle, disk
sampler yalnız slot oluşmuş churn kolunda. Son deney aynı ilk
başlangıç/bitiş penceresinde sürdü.
[İlk koşu kaydı](benchmarks/reclaim-followup-20261114/02-profile/status.json) ·
[İkinci koşu kaydı](benchmarks/reclaim-followup-20261114/02-profile-run/status.json)

| Kol | p95 / p99 / p99,9 / max (ms) | >1 s / 3.000 | Yazı + aralar (işlem/s) | Seq 2511 (ms) |
| --- | ---: | ---: | ---: | ---: |
| A1 · Kapalı | 88,67 / 1.239,55 / 2.368,80 / 2.856,43 | 87 | 1.196 | 17,65 |
| B1 · Açık | 79,56 / 1.151,71 / 1.960,83 / 2.555,84 | 73 | 1.265 | 60,27 |
| B2 · Açık | 81,37 / 1.139,03 / 1.881,53 / 2.250,94 | 71 | 1.273 | 21,31 |
| A2 · Kapalı | 81,26 / 1.139,06 / 2.072,83 / 2.498,61 | 70 | 1.256 | 15,55 |

Kol adlarındaki açık/kapalı trace'i belirtir. Reclaim dört kolda
varsayılan açık. Profilli aralık profilsiz tekrarlarla örtüşüyor.
Bu dört koldan profil maliyetinin bağımsız büyüklüğü çıkarılamıyor.
Önceki üretim doğrulamasındaki 23/3.000 uç oranı bu oturumda
tekrarlanmadı. Profilsiz kollar 70–87, profilli kollar 71–73.
1 s çizgisi bu adımda tanı sınıfıdır.

## Aşama bulguları

| En büyük ayrıştırılmış aşama, yalnız >1 s commit | B1 | B2 | Toplam |
| --- | ---: | ---: | ---: |
| WAL prefix join | 32 | 36 | 68 |
| Normal merge join | 6 | 9 | 15 |
| Ortak kaynak/bellek bütçesi | 3 | 3 | 6 |
| Açıklanamayan süre | 32 | 23 | 55 |
| Toplam | 73 | 71 | 144 |

Foreground aşamaları zaman aralıklarına ayrılarak iç içe kalemler bir
kez sayıldı. Worker süresi foreground'a tekrar eklenmedi. Hazır
revizyon sonucu join, en fazla 0,02 ms. WAL kopya/fsync, GC ve yayım
hiçbir >1 s commit'te en büyük kalem olmadı. Aşamaya bağlanamayan
süre ayrı bırakıldı. Snapshot maskesi gibi yalnız süreli alt sayaçlar
ham kayıtta ve ayrı tabloda, bu aralık toplamında yok. İşçi eşlemesi
çıktı kimliğiyle, snapshot release yalnız ≥1 ms aralıklarda.

| Örnek | Commit | Ölçülen aşamalar |
| --- | ---: | --- |
| B1 seq 824 | 2.555,84 ms | Prefix join 671,72 ms, iki normal merge join toplam 1.431,80 ms, indeks 156,88 ms. |
| B1 seq 797 | 2.066,28 ms | Prefix join 821,75 ms, bunun 474,79 ms'sinde prefix işçisi kaynak workspace bekledi. Yazıcıda bağımsız bütçe beklemeleri toplam 510,41 ms. |
| B2 seq 767 | 2.240,50 ms | Foreground ortak bütçe toplamında en uzun uç. Prefix join 12,9 ms. |
| B1/B2 seq 2511 | 60,27 / 21,31 ms | Önceki 2.093,48 ms tekrarlanmadı. Eski olayın nedeni bilinmiyor. |

[Bütün uzun olaylar, sequence, aşamalar ve worker kimlikleri](benchmarks/reclaim-followup-20261114/02-profile-abba/outlier-detail.json)

## Doğrulama ve sınırlar

Dört kolda 50.000 GET/version ve 41 exact digest eşit. Dört
process-kill ve strict reopen ile dört dondurulmuş okuyucu **GEÇTİ**.
6.000 trace frame, 12.172 yardımcı işçi/release kaydı, kayıp 0.
Uzun olaylar ve seçilen seq 2511 içinde join eşlemesi kaybı 0.
Korpus, önceki model ve 0.1 fixture korundu. SQLite slotuna
yazılmadı. 50 ms örnekli en yüksek slot 464,72 MB. Profil kanıtı
yaklaşık 60 MiB. İlk hazırlıktan son uyumluluk kontrolüne kadar
22:14:53–22:39:04 UTC, 45 dakikalık sınır içinde. FDB slotu hazır,
sequence 3101. [Doğrulama ve tam kol özeti](benchmarks/reclaim-followup-20261114/02-profile-abba/status.json)
