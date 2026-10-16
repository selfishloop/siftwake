# Native ikili FDB ve SQLite genel karşılaştırması — 2026-10-11

İki 100k corpus, üçer yeni dönüşümlü process çifti, toplam 12 process.
**978 search ve uzun corpus'ta 2400 get çağrısının doğruluk kontrolleri geçti.**
FDB native OFF1 / FDBLOG04 / SEG04, default feature'larla ölçüldü.
SQLite yalnız ayrı benchmark paketidir; motor bağımlılığı veya entegrasyon değildir.

Sonuç: FDB uzun yaygın, iki karakter ve AND sorgularında 21/21 eşleşmiş
karşılaştırmada hızlı. Dar kelime karışık: SQLite iki çiftte, FDB son çiftte
küçük farkla hızlı. Kısa yazma + bakımda SQLite üç çiftte de hızlı; RAM ve
diskte de SQLite küçük. **Genel SQLite üstünlüğü veya bütün işlemlerde FDB
üstünlüğü gösterilmedi.**

## Kısa corpus: yazma, okuma, alan sorguları

| İşlem | FDB medyan | SQLite medyan | SQLite/FDB süre oranı, eşleşmiş min–max |
|---|---:|---:|---:|
| 100k ingest | 9.345 s | 8.093 s | 0.81–1.00 |
| Son bakım | 6.130 s | 0.310 s | 0.047–0.057 |
| Yazma + son bakım | 15.475 s | 8.458 s | 0.52–0.62 |
| Dar title contains | 4.389 ms | 19.812 ms | 4.49–5.37 |
| Exact ilk sayfa | 0.251 ms | 16.680 ms | 60.59–84.18 |
| Exact ikinci sayfa | 0.266 ms | 17.025 ms | 63.12–65.25 |
| Exact son sayfa | 21.024 ms | 32.164 ms | 1.21–1.61 |
| Exact + numeric range | 0.641 ms | 3.501 ms | 4.05–6.22 |
| 100 tam belge get | 1.078 ms | 1.631 ms | 0.87–2.10, karışık |
| Update/delete sonrası teknoloji contains | 395.959 ms | 470.692 ms | 1.19–1.31 |

Oran 1 üzerindeyse FDB hızlıdır. Medyanlar ayrı hesaplanır; medyanların oranı
eşleşmiş oranların medyanı değildir. Ingest + bakımda SQLite **1.61–1.93 kat**
kısa sürede biter. FDB son bakımının yaklaşık6s maliyeti, karşılaştırmadaki
yazma farkının önemli kısmıdır. Bunun iç flush/merge dağılımı bu koşuda
profille ayrılmadı. Tek commit/batch p50'si tüm yazma maliyeti yerine kullanılmaz.

## Uzun corpus: contains ve tam belge get

| Sorgu / offset | FDB medyan ms | SQLite medyan ms | SQLite/FDB eşleşmiş min–max |
|---|---:|---:|---:|
| the / 0 | 5746.119 | 44634.472 | 5.87–7.85 |
| the / 50 | 5688.526 | 40127.140 | 5.51–7.05 |
| the / 99950 | 5690.737 | 42452.226 | 6.03–7.54 |
| abbreviation / 0 | 175.519 | 174.642 | 0.73–1.03, karışık |
| abbreviation / 50 | 174.302 | 176.176 | 0.78–1.01, karışık |
| of / 0 | 5926.692 | 38695.579 | 5.16–6.83 |
| of / 50 | 6064.955 | 38743.944 | 5.10–6.92 |
| the whale / 0 | 9355.114 | 41927.264 | 3.32–5.22 |
| the whale / 50 | 9855.510 | 41561.277 | 3.31–4.92 |
| 100 dağınık tam belge get | 2.871 | 7.238 | 1.74–4.17 |

Yaygın 5.51–7.85, iki karakter 5.10–6.92, AND 3.31–5.22 kat süre oranı;
bütün geniş sayfalar üç çiftte de FDB lehine. Dar sayfalarda SQLite ilk iki
çiftte kazanır, son çift FDB lehine %1–3 civarında süre oranı farkı gösterir.
Dar sorguda tekrarlanabilir kazanan yok. Get üç çiftte FDB lehine.

FDB tam kaynak doğrulamalı open medyanı15.375s; SQLite normal open12.769ms.
Kısa reopen491.729 /3.136ms; değişiklik sonrası507.489 /10.600ms.
**Açılış doğrulama sözleşmeleri aynı değildir; bu oran motor açılış
üstünlüğü olarak kullanılmaz.** SQLite normal open tam integrity yapmaz.

## RAM ve disk

| Corpus / sayaç | FDB | SQLite |
|---|---:|---:|
| Kısa tepe working set medyanı | 98.926 MiB | 9.762 MiB |
| Kısa örneklenen maksimum private medyanı | 95.156 MiB | 4.762 MiB |
| Kısa retained fiziksel disk | 418320443 byte | 107192320 byte |
| Uzun tepe working set medyanı | 130.238 MiB | 31.613 MiB |
| Uzun örneklenen maksimum private medyanı | 132.379 MiB | 27.648 MiB |
| Uzun retained fiziksel disk | 6182926010 byte | 5807165440 byte |

FDB kısa WS yaklaşık10.13, uzun WS4.12 kat büyüktür. Uzun disk FDB'de
yaklaşık%6.47 büyüktür. SQLite uzun ana dosyası5807132672byte; tabloda
32768byte SHM ve sıfır byte WAL da dahildir. Kısa disk FDB'nin saklanmış
eski source/index nesillerini de içerir; live-only disk veya geçici tepe
disk değildir. SQLite kısa sonuç diski, proses kapandıktan sonraki ana dosyadır.
FDB kaynak ve index içinde payload'ın iki kopyasını tutar; bu test onu değiştirmez.

WS/private Windows native proses sayaçlarıdır; motor heap'i, ortak RAM bütçesi
veya bütün bilgisayar RAM'i değildir. Açılış, corpus/oracle, bütün ordinary
benchmark işlemleri dahildir. PowerShell observer ve OS dosya cache'i hariç.
20ms hedef private örnekleme kısa pikleri kaçırabilir; PeakWorkingSet64 OS
high-water değeridir. SQLite ayrı integrity kontrolleri bu pencerenin dışındadır.

## Yöntem, eşdeğerlik ve sınırlar

- Kısa: aynı Rust generator; 100000 key/title, tekrarlı256byte Türkçe gövde,
  source/day/links/versions, batch500. FDB delta10000key/16MiB, sync açık.
  Aynı count + key sıralı50 tam belge; 50update/50delete ve reopen doğrulaması.
  Dar/exact/range/ikinci sayfa30 timed +1warmup; son sayfa ve broad3+1;
  100-get3+1. Toplam792search. Ingest corpus/serialization dahil,
  final bakım ayrıca ve birlikte raporlandı. Update/delete latency ölçülmedi.
- Uzun: aynı100000 benzersiz SHA-256 gövdeli Gutenberg pasajı, 2–50KiB,
  toplam2655160287 gövdebyte, seed20261009. Sekiz İngilizce kitaptan
  örtüşebilen pencereler; bağımsız100k eser veya Türkçe uzun metin ölçümü değil.
  Aynı fields/links/version; sequence200. the100000, abbreviation264,
  of100000, the whale16537. Count + sıralı50tam StoredDocument; tam
  belge/sıra/version/toplam oracle timer dışında. Uzun toplam186search.
  Her ilk sayfa1warmup; dar10timed, diğerleri1timed. 100-get anahtarları
  `(n*7919+137)%100000`, 1warmup+3timed; iki motorda aynı400belge/proses.
- FDB default release frozen exe; diagnostic ve experimental feature kapalı,
  overlay yok. Tek SEG04 ve sıfır resident/delta. SQLite3.53.2, ayrı Rust
  rusqlite0.40.2 bundled paket; WAL/FULL, cache_size=-2048, mmap0,tempFILE.
  UNIQUE key/source/day indeksleri ve external-content FTS5trigram,
  detail=none/columnsize=0; trigger'lar arama indeksini günceller.
  SQLite katlanmış title/text ve orijinal JSON payload saklar; FDB rawOFF1
  saklar ve aday metni sorguda normalleştirir. Alan/link/guard kapsamı tam eşdeğer değil.
- SQLite SQL aynı transaction'da count ve key sayfasını ayrı geçişte arar;
  uzun sayfa yalnız seçilen50payload'ı decode eder, sort'a LOB taşımaz.
  FDB count + sayfayı tek API çağrısında hesaplar. Alternatif SQLite SQL,
  ayrı folded tablo, büyük cache/mmap/window-count ölçülmedi. Sonuç bu
  SQL/schema/cache düzenine aittir; genel SQLite motor iddiası değildir.
- Sıra FDB/SQLite, SQLite/FDB, FDB/SQLite; kısa ve uzun ayrı üç çift.
  Yeni SQLite zamanları eski FDB zamanlarına bölünmedi. OS cache tahliyesi
  yapılmadı; warmup tüm corpus'un bellekte kaldığını kanıtlamaz. Cold cache,
  concurrency, uzun ingest, compression, source-offset, power loss ve
  geçici disk tepe kullanımı ölçülmedi. Ana sürelerde build/test/hash/profil yoktu.
- Ryzen7 7840HS8/16, Windows11Pro10.0.26200, yaklaşık15.3GiB RAM,
  rustc1.99.0; [önceki aynı makine kaydı](benchmarks/native-general-20261011-machine.json).
  Release thinLTO/codegen-units1, locked ayrı SQLite Cargo.lock.

Uzun SQLite kaynağı önceki prepare'da100000JSONpayload byte eşitliği ve ayrı
SQLite/FTS integrity kontrolünden geçmişti; [önceki hazırlama ve doğrulama](LONG-SQLITE-COMPARISON-20261011.md).
Bu çalışmanın öncesi/sonrası ana SQLite ve FDBsource/index/ready SHA-256
değerleri eşleşti; dört frozen exe değişmedi. Uzun tam integrity yeniden
çalıştırılmadı. Üç yeni kısa SQLite DB için ayrı SQLite+FTS integrity ve
checkpoint geçti. [Integrity0](benchmarks/native-sqlite-general-20261011-short-integrity-0.jsonl),
[1](benchmarks/native-sqlite-general-20261011-short-integrity-1.jsonl),
[2](benchmarks/native-sqlite-general-20261011-short-integrity-2.jsonl).

[Ham sonuçlar ve eşleşmiş oranlar](benchmarks/native-sqlite-general-20261011-summary.json),
[exe kimlikleri](benchmarks/native-sqlite-general-20261011-executables.json),
[kaynak hash kontrolü](benchmarks/native-sqlite-general-20261011-hash-check.json).
Root ve ayrı SQLite fmt/clippy geçti; ayrı paketin3testi geçti.
[Root fmt](benchmarks/native-sqlite-general-20261011-fmt.log),
[root clippy](benchmarks/native-sqlite-general-20261011-clippy.log),
[SQLite fmt](benchmarks/native-sqlite-general-20261011-sqlite-fmt.log),
[SQLite clippy](benchmarks/native-sqlite-general-20261011-sqlite-clippy.log),
[testler](benchmarks/native-sqlite-general-20261011-sqlite-tests.log).
Motor kaynak kodu, kalıcı format, 0.1fixture ve tests/engine.rs değiştirilmedi.

## Tekrar çalıştırma

`tools/measure-native-sqlite-general.ps1 -Phase short|long -Prefix yeni-ad`
mevcut frozenexe ve uzun veri dizinlerini kullanır; kısa yeni DB ister.
`tools/summarize-native-sqlite-general.py docs/benchmarks/yeni-ad` bütün12
process ve oracle kapıları geçmeden rapor üretmez. Benchmark SQLite kısa
integrity'yi `SQLITE_BENCH_SKIP_INTEGRITY=1` ile ayrı pencereye alır; normal
standalone aracın varsayılan integrity davranışı korunur. Ayrı kontrol:

```powershell
./output/sqlite-native-general-long-20261011.exe integrity data/yeni-ad-short-sqlite-0 data/native-general-long-binary-20261011
```

Sonraki öncelik yazma/son bakım maliyetini profille ayırmak; payload kopyasını
azaltmanın arama ve disk etkisini ayrı ölçmek. Tek-kopya layout bu testte
uygulanmadı ve ona ait hız sonucu yoktur.
