# Varsayılan reclaim ile motor-only RAM — 50k

**Ingest ve temiz idle kapısı GEÇTİ.** Varsayılan iki kolda ingest
tepe private commit **15,49–16,36 MiB**, temiz idle private
**2,20–2,26 MiB**. Geçerli sınırlar 60 ve 15 MiB.

| Kol | Ingest yazı bitimine kadar / bakım dahil private tepe | Ingest tepe WS | Temiz idle private / WS | %20 churn private tepe / WS |
| --- | ---: | ---: | ---: | ---: |
| A1 · Varsayılan | 15,49 / 15,49 MiB | 20,32 MiB | 2,26 / 5,96 MiB | 23,64 / 27,72 MiB |
| B1 · Reclaim kapalı | 14,37 / 14,54 MiB | 18,78 MiB | 2,20 / 5,93 MiB | 22,64 / 26,72 MiB |
| B2 · Reclaim kapalı | 13,97 / 13,97 MiB | 18,26 MiB | 2,11 / 6,02 MiB | 23,13 / 27,02 MiB |
| A2 · Varsayılan | 16,36 / 16,36 MiB | 20,77 MiB | 2,20 / 5,96 MiB | 23,58 / 27,62 MiB |

| İşçi tanığı, varsayılan | A1 | A2 |
| --- | ---: | ---: |
| Ingest planlama örneği / o örneklerde en yüksek proses private | 322 / 14,28 MiB | 340 / 15,55 MiB |
| Churn planlama örneği / o örneklerde en yüksek proses private | 99 / 18,04 MiB | 110 / 17,97 MiB |
| Rewrite örneği veya yayımlanan revizyon, ingest/churn | 0 | 0 |

**Tam rewrite sırasında motor-only tepe doğrulanamadı.** Planlama
işçisi varsayılan kollarla örtüştü. İki varsayılan kolda da rewrite
örneği ve revizyon yayımı yok. Kapalı kol ile fark paylaşılan
allocator ve diğer normal işçileri de içeren proses farkı.
Belirli bir thread'e RAM atfedilemez. Ek churn turu yapılmadı.

50k Cap200k korpusu, temiz ingest 100×500, ayrı strict-open temiz
idle ve temiz idle sonrasında bir %20 churn turu. Dört kol A/B/B/A.
Reclaim yalnız B oturumlarında kapatıldı. WAL prefix ve normal merge
korundu. Windows OS lifetime peak private/working set ve idle current
sayaçları. İşçi tanığı 10 ms örnekli. Korpus, model, katalog, arama
okuyucusu, oracle ve geçmiş motor prosesinin dışında. Runtime, 8 KiB
girdi tamponu, bir belge decode ve ölçüm için küçük sampler thread
proses ölçümünde. Query yok, doğal sıcak OS cache. Bu ölçüm genel
eşzamanlı arama belleği veya rewrite tepesi için sınır koymaz.
[Ham kollar ve OS sayaçları](benchmarks/reclaim-followup-20261114/04-ram/ram-summary.json) ·
[Sürücü](benchmarks/reclaim-followup-20261114/04-run.py)

**Doğruluk GEÇTİ 4/4.** Dört kill/reopen ve eski okuyucuda
50.000 GET/version ve 41 digest eşit, sequence 201. FDB slotu
hazır, korpus, eski model ve 0.1 fixture hash'leri aynı.
22:46:04–22:53:44 UTC, yaklaşık 10,01 MiB kanıt ve 436,31 MB
örnekli slot tepesi. 25 dk / 32 MiB ve slot sınırları içinde.
[Süre ve doğrulama](benchmarks/reclaim-followup-20261114/04-ram/status.json)
