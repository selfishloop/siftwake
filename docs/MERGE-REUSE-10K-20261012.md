# 10 bin uzun belgede bağımsız merge posting yeniden kullanımı

## Sonuç ve karar

Mevcut `experimental-merge-postings` yalnız merge algoritmasını seçer;
write-buffer-budget/streaming-metadata feature'larını otomatik açmaz.
Yeni feature, kalıcı format veya dış proje entegrasyonu yoktur.
İki eşleşmede yazma+bakım toplamı %17,35 ve %24,12 azaldı. Medyan
112,03 → 89,04 saniye (%20,52 azalma); son bakım 24,85 → 4,55 saniye
(%81,71 azalma). Yazma aşaması ilk eşleşmede yavaşladı, ikincide hızlandı:
tek başına yazma için tekrarlanabilir kazanç iddiası yoktur.

Feature default kapalı kalır. Bu ölçüm bakım kazancını doğrular;
arama p95/gecikme karşılaştırması veya bütün ürün hedeflerinin kabulü değildir.
SQLite yazma+bakımda hâlâ daha hızlıdır. Profil, sonraki işin delta posting
ekleme maliyeti ve flush sırasında tekrar terim çıkarımı olduğunu gösterir.

## Corpus, sorgular, cache ve çalışma alanı

Mevcut `data/rkyv-corpus-20261009/json.payload-bench` dosyasının aynı ilk
10.000 kaydı kullanıldı; yeni belge/corpus üretimi veya indirme yapılmadı.
Metin toplamı 264.842.858 bayt; metin uzunlukları 2.057–51.197 bayt.
Sekiz Gutenberg kitabının benzersiz, kısmen örtüşen pencereleri;
10 bin bağımsız eser veya Türkçe uzun metin corpus'u değildir.
Girdi header/kayıt/payload CRC'leri doğrulanır, 500 belge/batch işlenir.

Koşu sırası kontrol0 → aday0 → SQLite → aday1 → kontrol1.
Kontrol default feature'lar, aday yalnız `experimental-merge-postings`.
Release executable hash'leri koşu öncesi/sonrası doğrulandı:
`benchmarks/merge-isolated-10k-20261012-executables.json`.
OS cache boşaltılmadı; idle ölçümünde sorgu ısıtması yapılmadı.
Proses private/working set 20 ms aralıkla örneklendi, faz marker'ları 100 ms
aralıkla okundu. Örneklenen tepeler anlık tepeyi kaçırabilir; private ve
working set farklı OS sayaçlarıdır, heap kotası veya dosya cache toplamı değildir.

Yalnız `data/benchmark-work/fdb` ve `data/benchmark-work/sqlite` kullanıldı.
Her koşu doğrulandıktan sonra aynı FDB slotu sıfırlanıp aynı belgeler tekrar
indekslendi. Koşu başına DB kopyası tutulmadı; başka diske taşıma yapılmadı.
Profil de aynı FDB slotunu kullanır, normal süre karşılaştırmasına dahil değildir.
Son DB'ler sonraki okuma testleri için durur; `data/` toplamı 4.857.959.763 bayttır.

## Normal ölçüm sonuçları

FDB değerleri iki koşunun medyanı; SQLite tek yazma koşusudur.

| Ölçüm | Kontrol FDB | Merge reuse FDB | SQLite |
|---|---:|---:|---:|
| Yazma | 87,17 s | 84,50 s | 51,20 s |
| Son bakım | 24,85 s | 4,55 s | 1,66 s |
| Yazma + bakım | 112,03 s | 89,04 s | 52,86 s |
| Örneklenen tepe private | 176,46 MiB | 166,49 MiB | 8,56 MiB |
| Örneklenen tepe working set | 178,48 MiB | 166,46 MiB | 13,37 MiB |
| Boşta private, 3 açılış medyanı | 20,53 MiB | 20,52 MiB | 1,31 MiB |
| Açılış, 3 proses medyanı | 1.162,71 ms | 1.093,73 ms | Ayrı doğrulama sözleşmesi |
| Tutulan DB dosyaları | 1.513.745.887 B | 1.513.744.522 B | 583.860.224 B |

FDB tepe private yaklaşık %5,65, working set %6,73 azaldı; boşta private
farkı ihmal edilebilir. İki FDB'nin canlı source/index boyutu aynıdır:
267.900.709 + 353.242.581 = 621.143.290 bayt (ayrıca 45 bayt aktif log).
Tutulan boyut eski nesilleri de içerir; 1.365 bayt fark anlamlı disk kazancı değildir.
Geçici disk tepesi ölçülmedi. SQLite dosya sayaçlarında sonraki read-only
açılışlarda 32.768 bayt shared-memory dosyası oluşabilir.

SQLite isolated comparator: bundled SQLite, FTS5 trigram, external content,
JSON payload + önceden folded text; 2 MiB cache, mmap kapalı, temp FILE,
WAL + synchronous FULL, optimize + checkpoint dahil. FDB alan/link ve kaynak
doğrulama kapsamı farklıdır. Bu kurulumda aday FDB toplamı SQLite'ın yaklaşık
1,68 katıdır; genel SQLite/FDB üstünlüğü veya eşdeğer açılış doğrulaması iddiası yoktur.

## Profil: kaldırılan ve kalan işler

Her varyantta bir ayrı `diagnostic-timing` koşusu; ingest ve son bakım
snapshot'larının çağrı sayıları birleştirildi. İç içe/worker süreleri örtüşür,
CPU payı olarak toplanmaz ve normal benchmark süreleriyle karşılaştırılmaz.

| İşlem | Kontrol | Merge reuse |
|---|---:|---:|
| Builder terim çıkarımı | 20.000 | 10.000 |
| Builder posting ekleme | 20.000 | 10.000 |
| Gövde fold | 30.000 | 20.000 |
| Gövde trigram çıkarımı | 30.000 | 20.000 |
| Merge worker | 5 | 5 |
| Builder payload/kayıt ekleme | 20.000 | 20.000 |
| Binary batch decode | 76 | 111 |
| Positional disk read çağrısı | 10.045 | 217.297 |

Payload/kayıt yazımı ve kaynak doğrulama korunur; yalnız merge'deki terim
üretimi kalkmıştır. Reuse, doğrulanmış posting pencerelerini okuyarak daha çok
küçük positional read yapar. Böylece decode/read maliyeti artmasına rağmen
toplam bakım süresi düşmüştür; doğrulama kapatılmamıştır.

Aday profilde ingest sırasında `commit.apply_delta_index` 37,96 s,
`body.grams` 19,87 s, `builder.extract_terms` 12,69 s,
`builder.insert_postings` 9,34 s span süresi kaydetti. Bunlar örtüşen ham
span değerleridir. Fold 2,19 s; binary encode/CRC 0,68 s.
Son bakımda ek body grams 1,09 s vardır. Kalan darboğaz yalnız JSON çözümü
değildir: delta ve flush indeks inşası ayrı geçişler yapmaktadır.

## Doğrulama ve sınırlar

Dört normal FDB koşusu ve iki profil koşusunun her biri, ayrı doğrulama
prosesinde corpus ile **10.000 tam `get` eşitliği** kontrolünü geçti.
Bağımsız Unicode/Türkçe fold + contains scan oracle'ına karşı `the`,
`quixote`, `he`, `the man` sorgularının toplamı ve sıralı ilk 50 tam belge/
version eşitliği doğrulandı: toplamlar 10.000 / 56 / 10.000 / 9.932;
sequence her koşuda 20. Bu bir sorgu gecikmesi benchmark'ı değildir.
SQLite integrity_check ve external-content FTS integrity-check geçti.

Kaynak değişikliğinin önceki doğrulama logları: default 63, bağımsız feature
66, all-features 116 test geçti (`benchmarks/merge-isolated-10k-20261012-tests*.log`).
Kesilme, tek bayt bozulması, süreç sonlandırma, eski fixture okuma ve
bozuk kaynak/sağlam cache davranışı kapsanır. Reuse byte eşitliği/remap/CRC
ve legacy SEG03 footer kesilme/bozulma testleri vardır. 0.1 fixture değişmedi.
Bu koşu sonunda `cargo fmt --check`, default ve bağımsız feature için
`cargo clippy --all-targets -- -D warnings` yeniden geçti.

Ölçülmeyenler: soğuk cache, 30 sorguluk p95, concurrent okuyucu/yazıcı,
DLL/derived profil, büyük corpus ve geçici disk tepesi. Feature'ın diğer
optimizasyonlarla birleşimi bu ölçümde yoktur.

Ham sonuç/özet: `benchmarks/merge-reuse-10k-20261012-*.jsonl`,
`benchmarks/merge-reuse-10k-20261012-*.memory.json`,
`benchmarks/merge-reuse-10k-20261012-summary.json`.
