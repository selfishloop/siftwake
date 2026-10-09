# Doğrulama — 0.3.1 — 08.10.2026

0.3'ün SQLite karşılaştırması hız ve bellek hedefini karşılamıyordu. Bu sürüm
ölçülen tekrarları azaltır; genel bir SQLite üstünlüğü iddiası değildir.
Motor bağımsız Rust uygulamasıdır, hazır DB/arama motoru kullanılmaz ve dış
projelere entegrasyon yapılmadı. SQLite yalnız karşılaştırma aracındadır.

## Değişen uygulama

- RAM delta posting'leri aynı Arc<Key> referansını paylaşır; her trigram için
  scope/collection/id metinlerinin tam kopyası oluşturulmaz.
- Disk dizini RAM'de sabit metadata vektörü + UTF-8 ID havuzu + ortak
  scope/collection çiftleri halinde tutulur. Anahtar ve konumlar hâlâ RAM'dedir.
- Sorgu başında yeni run/delta tarafından gizlenen ID'ler bit maskesine işaretlenir.
  Aday başına bütün yeni run'larda yeniden anahtar araması yapılmaz.
- Unicode whitespace içeren, sorguların hiçbir zaman istemediği trigramlar
  yeni indekslerde üretilmez. AND alt-dize ve kısa terim davranışı korunur.
- Yazma indeksinde ve segment builder'da aynı içerik için folded metin/trigram
  kümeleri tahmini 256 KiB bütçeli önbelleklerde tekrar kullanılır. Başlık
  trigramları ayrı üretilip union edilir; link token'ları tekilleştirilir.
- Metin sorgusunda aynı içerik için folded metin tekrar kullanılır; mantıksal
  özgün+folded metin bütçesi 256 KiB, allocator/map ek maliyeti ayrıca vardır.
- CRC32 IEEE slicing-by-8 ile hesaplanır; checksum sonucu ve kalıcı biçim aynı.
- Segment builder geçici dizin ve JSON buffer'ını index açılmadan serbest bırakır.

FDBLOG03/FDBMAN03/FDBSEG03 biçimleri değişmedi. 0.3 cache'leri doğrudan okunur;
eski cache'lerdeki fazladan whitespace trigramları zararsızdır. Kaynak/cache
bozulma kontrolleri, sync_all ve guard sözleşmesi korunur. Cache hâlâ türetilmiştir.
0.1 fixture kaynakları değiştirilmedi.

## Kontroller

| Kontrol | Sonuç |
| --- | --- |
| `cargo test --all-targets` | 41 geçti: 3 unit, 11 disk, 17 engine, 10 segment |
| `cargo fmt --check` | geçti |
| `cargo clippy --all-targets -- -D warnings` | geçti |
| Release binary/örnekler | derlendi |
| Bir milyon kayıt, 50 update/50 delete, reopen | geçti |
| Gerçek owner process kill sonrası kabul edilmiş sentinel | geçti |

Yeni testler IEEE CRC bilinen vektörünü ve farklı streaming sınırlarını,
tekrar eden içerikte farklı başlıkları, Unicode ID'leri, bütçeyi aşan uzun
içeriği, Unicode whitespace/AND/kısa terimleri RAM ve disk üzerinde doğrular.
Mevcut bütün kesilme, bozulma, yanlış/kayıp cache, tombstone, guard, link,
random model ve 0.1 uyumluluk testleri de çalıştı.

## Aynı corpus ile tam ölçüm

Windows 11 Pro / Ryzen 7 7840HS, Rust 1.99.0/MSVC release. Önceki FDB ve SQLite
ölçümleriyle aynı makine. 1.000.000 kayıt, 256 bayt tekrarlı Türkçe içerik,
pad edilmiş 10 haneli ID, benzersiz başlık, source/day alanları, 500 kayıtlık
2.000 batch. Delta eşikleri 10.000 anahtar/16 MiB; kalıcılık kapatılmadı.
Ingest bittikten sonra mevcut ve uygun sınırlı bakım işleri beklenip motor
yeniden açıldı. Sorgular sıcak yerel file/OS cache ile, count+50 payload için
yapıldı; SQLite konfigürasyonu [karşılaştırma belgesinde](SQLITE-COMPARISON.md).

| Ölçüm | FDB 0.3 | FDB 0.3.1 | SQLite 3.53.1 |
| --- | --- | --- | --- |
| Tepe working set | 416,70 MiB | **162,22 MiB** | 20,21 MiB |
| Tepe private committed bytes, örneklenen | 517,48 MiB | 163,39 MiB | 13,82 MiB |
| Ingest + otomatik bakım | 276,530 s | **108,086 s** | 137,404 s |
| Son bakım | 15,983 s | 7,578 s | 3,672 s |
| Ingest + son bakım toplam | 292,513 s | **115,664 s** | 141,076 s |
| Commit medyan / p95 / maksimum | 67,450 / 482,561 / 2041,788 ms | 22,868 / 78,504 / 818,791 ms | 42,113 / 80,156 / 1032,933 ms |
| Dar başlık araması, 30 tekrar medyan | 96,900 ms | **55,557 ms** | 214,597 ms |
| source eşitliği, 5 tekrar medyan | 176,629 ms | **9,332 ms** | 114,262 ms |
| source + day 2..5, 5 tekrar medyan | 38,346 ms | **4,086 ms** | 39,790 ms |
| Geniş teknoloji alt-dize, 3 tekrar medyan | 9,779 s | **4,887 s** | 5,346 s |

Ingest yaklaşık 9.252 kayıt/s. Bu tek tam çalışmada SQLite ingest'inden
yaklaşık %21 daha kısa sürdü; son bakım dahil toplam da yaklaşık %18 daha kısa.
Metadata yaklaşık 12,2 kat, range yaklaşık 9,7 kat, dar başlık yaklaşık 3,9 kat
daha hızlı ölçüldü. Geniş içerik farkı yalnız yaklaşık %9; tekrarlar/makine
koşulları bakımından daha temkinli değerlendirilmelidir.

FDB 0.3.1 dar arama p95 57,365 ms; metadata p95 10,654 ms; range p95 5,535 ms.
Dar arama `rapor-0000500000`: 1 eşleşme; source: 1.000.000;
range: 133.336 eşleşme. Son sayfanın ilk anahtarı doğrulandı, ayrıca zamanlanmadı.
50 update/50 delete sonrası 999.950 kayıt ve sequence 2001 korundu.

Sıcak yeniden açılış kaynak/cache/frame doğrulaması dahil 4,970 s;
değişikliklerden sonra 6,733 s. SQLite normal 3,915 ms açılışı bütünlük
doğrulaması yapmadığından aynı ölçüm değildir; ayrı tam kontrolü 21,507 s idi.
Hiçbir açılış kontrolü hız elde etmek için atlanmadı.

## Bellek, disk ve kurtarma

Ana ölçüm 200 ms aralıkla, 597 örnek ve 129,599 saniye izlendi.
Gözlenen PeakWorkingSet64: **170.098.688 bayt**. Örneklenen tepe
PrivateMemorySize64: **171.327.488 bayt**. Bunlar mantıksal RAM metin sayacı
değildir; gerçek proses sayaçlarıdır. OS toplam file cache'i dahil değildir.
Son örnek sonrası kısa tepe kaçabilir. Working set paylaşılan resident sayfaları
içerir; private committed bytes resident fiziksel RAM'e eşit değildir.

Yeni CLI prosesinde aynı DB yeniden açılıp stats yanıtından 1 saniye sonra:

| FDB boşta, yeni proses | Ölçüm |
| --- | --- |
| Kayıt / sequence / segment | 999.951 / 2002 / 15 |
| Working set | **58,81 MiB** (61.669.376 bayt) |
| Private committed bytes | 63,14 MiB |
| Açılışta gözlenen tepe working set | 86,67 MiB |
| Açılış + stats | 5,992 s |

Önceki yeni-proses boşta ölçümü 176,29 MiB idi. Tepe working set yaklaşık
%61, boşta working set yaklaşık %67 azaldı. **RAM hedefi tamamlanmış değildir:**
162,22 MiB tepe, SQLite'ın 20,21 MiB proses tepesinden hâlâ yaklaşık 8 kat fazla.
Anahtar/konum tablosu ve token sözlüğü disk sayfalarına taşınmadı; kompakt RAM
temsilindedir. Bunların disk tabanlı ve sınırlı cache ile tutulması sonraki
bellek aşamasıdır. Sürekli yaşayan/yazmış prosesin boşta belleği ölçülmedi.

| Disk, update/delete sonrası ve sentinel öncesi | Bayt |
| --- | --- |
| Referans kaynaklar + aktif WAL | 465.132.539 |
| Referans cache'ler | 777.425.389 |
| Arşivler dahil toplam klasör | 3.763.105.197 |

Eski dosyalar silinmez; otomatik GC yoktur. Yarım/intermediate benchmark
klasörleri bu toplamın dışında ayrı tutuldu, temizlenmedi. Bunlar nihai ölçüm
değildir. En son ölçümün log/kanıtları aşağıdadır.

Başarılı sequence=2002 sentinel yanıtından sonra gerçek CLI prosesi kill edildi.
Yeni owner 999.951 kayıt, sentinel sürümü ve önceki 50 update/50 delete'i
doğruladı. `teknoloji` sorgusu 999.900 eşleşme verdi; sıcak tekrarlar stdio
istemcisi dahil 5,014 / 4,887 / 4,857 s. Bu ayrı kurtarma prosesinin belleği
ana ingest ölçümüne dahil değildir; SQLite geniş sorgusu da ayrı çalışmıştı.

## Kanıt ve sınırlar

[Tam JSON Lines](benchmarks/scale-0.3.1.jsonl),
[RAM/disk](benchmarks/scale-0.3.1.memory.json),
[kill ve geniş arama](benchmarks/scale-0.3.1.recovery.json),
[boşta bellek](benchmarks/fdb-0.3.1.idle.json).

```powershell
cargo build --release --bins --examples
.\tools\measure-scale.ps1 -DatabasePath data/new-031 -OutputPrefix output/new-031
python -X utf8 tools/verify-scale-recovery.py target/release/fdb.exe data/new-031 1000000
.\tools\measure-fdb-idle.ps1 -DatabasePath data/new-031
```

Yazma/sorgu cache'leri tekrar eden corpus'tan yararlanır. Çeşitli veya büyük
içerikler bütçeyi aştığında yeniden hesaplama yapılır; aynı kazanç varsayılmaz.
Maskeler çakışan segmentlerde anahtar taraması gerektirir; güncelleme yoğun
corpus ayrıca ölçülmedi. Kaynak formatı değişmedi; disk açılışı JSON dizini ve
source replay nedeniyle hâlâ geçici RAM kullanır. Cache bütçeleri bütün proses
RAM limiti değildir. Soğuk I/O, farklı donanım, gerçek uzun belge corpus'u,
çok kullanıcılı concurrency, elektrik kesintisi ve disk dolması ölçülmedi.
Ölçülen bu corpus'ta yazma/arama hedefi iyileşti; genel DB performansı ve
SQLite düzeyinde bellek kullanımı hedefinin bütünü sağlanmış sayılmaz.
