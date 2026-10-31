# Güncel kabul kapıları — 27 Ekim 2026

Kullanıcının son kararı esas alınır. İstenen yeni ölçüm turu tamamlandı;
önceki/kötü sonuçlar korunur. GEÇTİ yalnız ilgili ölçüm kapsamıdır.

| İş | Güncel kapı / kanıt | Durum |
|---|---|---|
| 1a, normal tekli | Önceki 4×768: 0/3072 commit >50 ms; max 26,366 ms | GEÇTİ, kabul edildi |
| 1b, tekli throughput | Yeni ≥%75 ve SQLite pooled üstü: önceki FDB %77,22 > SQLite %70,38 | GEÇTİ, yeni ölçüte göre |
| 1c, gerçek 10k /30 tur | İki dengeli tekrar; bakım sonrası max 1,199×, son 1,107×; son ölü payload 29,75 MB | GEÇTİ, gözlenen 30 tur |
| 1d, tombstone ve artıklar | Tepe 1201→son 600 kayıt; yeni 7 kill/eski okuyucu/pin geçti; artık 0 byte | GEÇTİ, gözlenen 30 tur |
| 1b, 100'lük throughput | Tek eşit 128×100 çift; FDB %76,59 ≥%75 ve SQLite %61,23 üstü | GEÇTİ; tek çift |
| Uzun 4R gecikme kapısı | Profil kapalı 3×4096; 0 adet >50 ms, max 29,022 ms; eşleştirmeli 3 profil de 0 aşım | GEÇTİ, ölçülen koşular |
| Eski 419 ms'nin kök nedeni | Yeniden oluşmadı; o örnek için WAL open nedeni kanıtlanmadı; koşullu handle değişikliği yapılmadı | İZLENİYOR; düzeltildi denmez |
| Ortak doğrulama | Üç adımda ayrı 7 kill (21 toplam), oracle/eski okuyucu/pin; son 233 test/fmt/clippy | GEÇTİ |
| Diğer açık arama/RAM/100'lü uçlar | 100'lü 4R'de 20/38 sorgu eski p95 sınırını aştı; proses peak 77,94 MiB; 100'lü commit p95≈1004 ms | AÇIK/KALDI; bu tur çözülmedi |
| Blok 2/3/4 ve C ABI | Yeni kullanıcı onayı olmadan başlanmaz | BAŞLANMADI |

İlk 10k corpus ve sabit FDB/SQLite slotları kullanıldı; yeni corpus veya kalıcı
biçim değişikliği yok. Küçük aktif-WAL katmanı/reclaim ürün varsayılanında
kapalı, ölçümde opt-in. Kalıcı WAL ve sync politikası değişmedi.
case34, geniş zx/the zx, arama/RAM, soğuk yük ve uzun snapshot pinleri açıktır.
[30tur ayrıntısı](RECLAIM-IDLE30-10K-20261027.md).100'lük çift nihai kod üzerinde
ölçüleceği için sıraya önce sequence profili alındı.
[Eşleştirmeli profil ve sınırlar](WAL-SEQUENCE-PROFILE-10K-20261027.md).
[100'lük çift ve bütün kötü sonuçlar](HUNDRED-PAIR-10K-20261027.md).
