# Benchmark verisi ve çalışma alanı

2026-10-12 kullanıcı kararı: yeni belge üretmek/indirmek ve koşu başına
veritabanı biriktirmek durduruldu. Testleri D: dahil başka diske taşıma yok.

333 eski test veritabanı klasörü temizlendi: 153.608.536.981 bayt.
D: üzerinde önceki görevde arşivlenen iki test kopyası da kaldırıldı:
2.847.073.275 bayt. Gereksiz binary/rkyv codec corpus kopyaları da silindi:
5.398.267.649 bayt. Kod, raporlar, `tests/fixtures/v1` ve ortak JSON corpus korunur.
Silinen veritabanlarının envanteri `docs/benchmarks/test-database-cleanup-20261012-inventory.json`;
temizlik sonucu `docs/benchmarks/test-database-cleanup-20261012.json`.
Eski raporlar ölçüm geçmişidir; artık eski DB yollarının mevcut olduğunu göstermez.

`data/rkyv-corpus-20261009/json.payload-bench` tek ortak JSON belge girdisidir.
İlk 10.000 belge aynı sırada okunur. Corpus hazırlama/indirme script'leri çalıştırılmaz.
Binary/rkyv deneylerinin eski türetilmiş corpus dosyaları temizlendi; yeniden üretilmez.

Aktif bellek/merge ölçüm araçlarında log prefix'i yalnız rapor adını değiştirir;
veritabanı yolu her zaman `data/benchmark-work/fdb` veya
`data/benchmark-work/sqlite` olur. Sahiplik marker'ı olmayan çalışma alanı
silinmez. Sıfırlama yalnız bu iki sabit alt klasöre izin verir; reparse point
ve kapsam dışı yollar reddedilir. Fixture'lar ve corpus sıfırlanmaz.

Okuma/arama testleri mevcut DB üzerinde yürütülür. Örneğin mevcut DB açılış
belleğini ölçmek için `tools/measure-content-memory.ps1 -Mode idle -Prefix read-reuse-001` kullanılır;
bu mod belge yazmaz ve DB sıfırlamaz. İndeks oluşturma/yazma karşılaştırması
gerektiğinde mevcut slot silinip **aynı belgeler** tekrar indekslenir; yeni
belge corpus'u üretilmez. Her sonuç ve oracle doğrulaması slot tekrar
kullanılmadan önce kaydedilir. Son DB'ler sonraki okuma testleri için kalır.

Tarihsel diğer benchmark araçları kullanıcı kararıyla uyumlu hale getirilmeden
çalıştırılmaz. Bu temizlik sırasında performans testi veya yeni DB oluşturma
yapılmadı. Son merge karşılaştırması tamamlanmış kabul edilmez; disk korumasında
kesilen koşu ve henüz doğrulanmamış sonuçlar kabul kararı olarak kullanılmaz.
