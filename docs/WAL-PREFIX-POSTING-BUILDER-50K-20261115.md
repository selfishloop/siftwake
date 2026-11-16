# 50k WAL prefix posting hazırlığı

Cap200k korpusunun ilk 50 bini (`881756DA…`), sıcak doğal dosya cache'i,
1 snapshot okuyucu ve 1 yazıcı, 41 sorgu. Her kol aynı temiz seed'den
başladı. Üç tur × %20 churn, 100 işlem/commit, tur aralarında 2 sn
bakım adımı. Sadece motorun sahipli sabit FDB slotu kullanıldı.
[Önceki ilk geçiş profili](WAL-PREFIX-FIRST-PASS-50K-20261114.md) ·
[Sürücü](benchmarks/wal-prefix-builder-20261115/run.py)

## Tanı

Mevcut zamanlayıcılar yalnız prefix işçisindeki ilk geçişe bağlandı.
Her işçi için aşama toplamı var. Belge başına iz satırı yok. Profildeki
23 işçinin 10'u commit'i en az 50 ms bekletti.

| Aşama | 23 işçi toplamı | 10 beklemeli işçi toplamı | 10 işçinin indeks oluşturma payı |
| --- | ---: | ---: | ---: |
| Posting ekleme | 2.664,14 ms | 2.068,34 ms | %57,5 |
| Gövde trigramı çıkarma | 1.344,06 ms | 1.044,93 ms | %29,0 |
| Gövde normalleştirme | 161,16 ms | 125,89 ms | %3,5 |
| Tüm indeks oluşturma | 4.629,16 ms | 3.600,04 ms | %100 |

Süreler duvar saati. Metin başlığı ve indeksin diğer alanlarının maliyeti
ayrı gösterilmedi. Tanı kolu ayrıntılı profil maliyetini taşıdığı için
performans çiftlerine dahil edilmedi.
[Tanı özeti](benchmarks/wal-prefix-builder-20261115/summary-detail.json)

## Prototip

İlk scope'un en fazla 4.096 trigramı prefix işçisi içinde geçici hash
tablosunda tutuluyor. Taşan terimler ve diğer scope'lar mevcut ağaçta.
İşçi ilk geçiş sonunda hash tablosundaki terim ve posting sahipliğini
sıralı ağaca taşıyor. Karşılaştırma için sıralamaya dönüşün süresi de
kaydedildi. İndeks yazıcısı, tekil terim ve posting sırası, disk
biçimi ve kaynak doğrulaması mevcut yoldan.
[Geçici diff](benchmarks/wal-prefix-builder-20261115/prototype.diff.txt)

Özel test 4.096 terimin üstünü, tekrar eden ve sırasız ID'leri,
birden çok scope'u ve posting temsil geçişlerini kapsadı. Aday
sonuçta aynı sıralı terimleri, posting ID'lerini ve builder muhasebesini
üretti. Aday açıkken `cargo fmt --check`, Clippy `-D warnings` ve
275 test geçti, 17 test atlandı. Test paketi CRC, prefix kill, snapshot
ve yayın oracle'ını kapsadı.
[Clippy](benchmarks/wal-prefix-builder-20261115/clippy.log) ·
[Test paketi](benchmarks/wal-prefix-builder-20261115/test-candidate.log)

## Dengeli karşılaştırma

Sıra A1 → B1 → B2 → A2. Aynı ikili ve hafif sequence profili,
ayrıntılı işçi sayaçları kapalı. A mevcut yol, B prototip. Arşiv
SQLite FTS5 trigram aynı yükün ilk üç turu ve önceki oturumdan.
Yazma hızı tur içi yazmayı ve tur aralarını içeriyor.

| Kol | Commit p50 / p95 / p99 (ms) | Max (ms) | >1 s / 300 | İşlem/s | ≥50 ms prefix join adet / toplam | Süreç tepe private | Pooled okuma p95 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| A1 | 13,88 / 56,93 / 1.193,95 | 1.773,30 | 7 | 1.358,0 | 8 / 3.820,80 ms | 48,16 MiB | 134,05 ms |
| B1 | 13,58 / 44,84 / 1.004,72 | 2.122,64 | 4 | 1.421,1 | 9 / 3.251,56 ms | 47,94 MiB | 132,83 ms |
| A2 | 13,55 / 45,18 / 1.082,86 | 1.597,14 | 6 | 1.444,0 | 9 / 4.105,11 ms | 50,83 MiB | 136,15 ms |
| B2 | 13,64 / 54,87 / 1.037,55 | 1.686,85 | 5 | 1.440,7 | 6 / 1.693,23 ms | 46,30 MiB | 134,41 ms |
| SQLite, arşiv | 131,85 / 298,30 / 435,62 | 791,00 | 0 | — | — | — | — |

| Kontrol → aday | Prefix join toplamı | İlk geçiş duvar toplamı | İlk geçiş toplamı / işlenen MiB | Commit p95 | Commit p99 | Max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A1 → B1 | −%14,9 | −%21,8 | yaklaşık 94,5 → 76,0 ms/MiB | −%21,2 | −%15,8 | +349,35 ms |
| A2 → B2 | −%58,8 | −%41,6 | yaklaşık 95,4 → 73,3 ms/MiB | +%21,4 | −%4,2 | +89,71 ms |

Her kolda 24 tamamlanmış prefix işçisi ve 12 uzun yol commit'i.
Term tablosunu sıralıya taşımanın toplam maliyeti B1/B2'de
32,47/29,27 ms. Prefix işçileri farklı sequence'lerde dondurduğu için
işledikleri bayt ve anahtar sayıları değişiyor. MiB başına oran kontrol
amaçlı, eşit içerikli mikro ölçüm yapılmadı. Okuma p95 farklı sorguları
havuzluyor, sorgu başına kabul kapısı çıkarılamaz. Private değerler
yazıcı, okuyucu ve benchmark modelini içeren süreç ölçümleri.
Motor-only ingest/idle kapısı bu deneyde ölçülmedi.
[Eşleşmiş hesap ve uçların tamamı](benchmarks/wal-prefix-builder-20261115/summary-ab.json) ·
[SQLite arşiv kaydı](benchmarks/live-disk-50k-20261111/5-sqlite-churn-01-10.jsonl)

En uzun aday B1/217, 2.122,64 ms. 330,07 ms prefix join,
856,32 ms normal merge join ve 769,92 ms yazıcının delta flush'ı.
B2/262, 1.686,85 ms. Prefix join 7,92 ms, yazıcının delta flush'ı
1.553,56 ms. Aynı sequence A2'de 1.597,14 ms, delta flush 1.485,50 ms.
Bu izler uç gecikmede indeks dışındaki iki beklemenin baskın
olabildiğini gösteriyor. Adayın bu farkları doğurduğuna ilişkin
eşleşmiş işçi veya yerleşim kanıtı yok.

**Karar önerisi.** Aday kısa koşuda prefix beklemesini, commit p99'u
ve 1 saniye üstü adedi iki çiftte de düşürdü. p95 bir çiftte, max iki
çiftte kötüleşti. Prototip geçici kopyada kalsın. Bu sonuç yazma
kuyruğu kapısına veya uzun süreli disk ve bakım davranışına yeni
bir kabul getirmiyor. Açık öncelik, yazıcının kendi delta flush'ı
ve normal merge beklemesinin bu uçları nasıl oluşturduğunu ayırmak.

## Kapanış ve kaynak kullanımı

Tanı ve dört karşılaştırma kolunda 1.500 ölçülen commit. Beşinde de
ACK sonrası process kill, strict reopen, tam 50k GET/version, 41 exact
digest ve dondurulmuş eski okuyucu geçti. Dört karşılaştırma kolunun
son model hash'leri tanı koluyla aynı. FDB slotu `ready`,
50.000 belge, sequence 401. SQLite slot envanteri ve kaydı, corpus
SHA-256 ve 0.1 fixture hash'leri korundu.

14 Kasım 22:09:21–22:26:06 UTC, Türkiye saatiyle 15 Kasım
01:09:21–01:26:06. 25 dk tavanı tutuldu. Geçici klasör yaklaşık
448,26 MiB (470.035.125 B), ölçüm kanıtı yaklaşık 20,23 MiB, azami
ölçülmüş FDB slotu 404,55 MiB. Bu deneyin klasörü tutuldu. Üretim
motoru, kalıcı biçim, varsayılanlar ve kapı eşikleri aynı.
[Tanı kapanışı](benchmarks/wal-prefix-builder-20261115/status-detail.json) ·
[Karşılaştırma kapanışı](benchmarks/wal-prefix-builder-20261115/status-ab.json) ·
[Açık işler](OPEN-WORK.md)
