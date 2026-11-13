# Blok 2b — 50k sıcak kolun tamamlanması

8 Kasım 2026. Başlangıç 20:25:24 UTC, purge son saati 20:38:24 UTC, pencere sonu 20:45:24 UTC. Kullanıcı, eksik örnekleri toplamak için performans ve iz doğrulama kapılarında durulmamasını istedi. Önceki cold izlerinin geçerlilik sınıfları ve 2.000 ms karşılaştırma çizgisi korundu.

Cap200k korpusunun ilk 50.000 belgesi, mevcut sabit slotlar ve önceki ölçüm ikilileri kullanıldı. TR case0 `olduğu`, ilk 50 sonuç, FDB melez ve SQLite FTS5 trigram. Eksik SQLite sıcak kolunda 30 ana sorgu ve iki 60 örnekli sıcak A/A bloğu tamamlandı. SQLite kolu FDB sıcak kolundan daha sonraki bir zaman penceresinde alındı. İki sıcak kolda da WPR kapalıydı.

| Sıcak ölçüm (ms) | FDB | SQLite |
| --- | ---: | ---: |
| Açılış | 1395,40 | 2,60 |
| Açılış başlangıcı → ilk sonuç | 1507,22 | 302,03 |
| İlk sorgu | 111,82 | 299,43 |
| 30. sorgu | 102,81 | 444,59 |
| 2–30. sorgular p95 | 109,01 | 605,44 |
| Sıcak A/A p95, A / A | 137,45 / 168,93 | 573,90 / 627,78 |

Önceki cold ölçümünde FDB ilk sonucu **1868,88 ms** ve Q30 **131,50 ms**, ETW doğrulaması geçerli. SQLite ham ilk sonucu **4622,54 ms** ve Q30 **365,16 ms**. SQLite collector sınırına ulaştığı için ham süreler doğrulanmış cold karşılaştırmasına girmez.

Ters çift SQLite → FDB için yönetici purge adımı 20:38:24 UTC’ye kadar çalıştırılmadı. İki kol da başlamadı. Bu pencerede yeni purge, WPR kaydı ve cold açılış sayısı sıfır. 2b **KISMEN**.

İki motor da kill/reopen sonrası 50.000 GET/version ve 41 sorguluk oracle’ı geçti. FDB dosyaları, SQLite ana dosyası ve iki modelin SHA kontrolleri geçti. Onaylı SQLite yan dosya envanteri 0 B WAL ve 32 KiB SHM olarak doğrulandı. Eski FDB okuyucu kontrolü önceki koşuda geçti. Bu tur aynı ikilileri kullandı.

[İlk çift raporu](BLOCK2B-COLD-2S-PAIRED-50K-20261108.md) · [Ham SQLite sıcak örnekleri](benchmarks/cold-paired-completion-20261108/trial-02/warm.jsonl) · [İlk çift sayısal özeti](benchmarks/cold-paired-completion-20261108/first-pair-timings.json) · [Tamamlama durumu](benchmarks/cold-paired-completion-20261108/final-verdict.json)
