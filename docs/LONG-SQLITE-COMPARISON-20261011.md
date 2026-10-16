# Uzun metin: FDB ve SQLite — 11 Ekim 2026

## Sonuç

**Bu SQLite düzenine karşı yaygın, iki karakterli ve AND sorgularında FDB
üç çiftte de daha hızlı.** Dar sorguda SQLite iki çiftte, FDB bir çiftte daha
hızlı; tekrarlanabilir tek kazanan yok. SQLite tepe working set ve disk boyutunda
daha küçük. Bu sonuç genel SQLite üstünlüğü/yenilgisi olarak yorumlanmaz.

| Sorgu / offset | FDB medyan ms | SQLite medyan ms | Çiftlerde SQLite/FDB süre oranı |
|---|---:|---:|---:|
| `the` / 0 | 7.463,96 | 45.329,06 | 4,32–6,08 |
| `the` / 50 | 7.468,84 | 45.689,89 | 4,55–6,19 |
| `the` / 99.950 | 7.540,53 | 45.684,05 | 4,56–6,15 |
| `abbreviation` / 0 | 182,96 | 178,50 | 0,68–1,08 |
| `abbreviation` / 50 | 176,66 | 182,35 | 0,65–1,05 |
| `of` / 0 | 10.917,76 | 44.251,38 | 4,05–5,07 |
| `of` / 50 | 10.450,18 | 44.879,00 | 3,89–5,08 |
| `the whale` / 0 | 12.890,07 | 42.336,76 | 3,26–3,90 |
| `the whale` / 50 | 10.701,72 | 41.345,15 | 3,20–3,84 |

Medyan süreler her motorun üç prosesinden ayrı hesaplanır; medyanların oranı,
eşleşmiş çift oranlarının medyanı değildir. Son sütun her çiftin süre oranının
min–max aralığıdır; 1 üzeri FDB lehinedir. Dar ilk sayfada SQLite ilk/son
çiftlerde 1,48/1,38 kat hızlı, FDB ikinci çiftte 1,08 kat hızlıdır. İkinci
dar sayfada SQLite ilk/son çiftlerde 1,54/1,31 kat hızlı, FDB ikinci çiftte
1,05 kat hızlıdır. Sadece bağımsız medyanların yakınlığına bakılarak genel
eşitlik veya kesin üstünlük ilan edilmez.

| Kaynak ölçüm | FDB | SQLite |
|---|---:|---:|
| Tepe working set medyanı | 130,223 MiB | 28,375 MiB |
| Örneklenmiş private bytes medyanı | 134,672 MiB | 24,742 MiB |
| Fiziksel kalıcı kaynak + türetilmiş indeks | 6.293.010.040 byte | 5.807.132.672 byte |
| Normal open medyanı (sözleşmeler farklı) | 20.169,72 ms | 9,59 ms |

FDB working set yaklaşık 4,59 kat, fiziksel disk %8,37 daha büyüktür. FDB
private peak üç proseste 141.213.696 / 139.415.552 / 167.444.480 byte;
üçüncü pik gizlenmez. SQLite değerleri 25.501.696 / 25.948.160 / 25.944.064 byte.
FDB'nin `stats.source_bytes` değeri legacy kaynağı iki kez saydığı için fiziksel
disk hesabında kullanılmadı. LOCK, oracle ve corpus hazırlama dosyaları bu
disk tablosuna dahil değildir; SQLite geçici WAL/SHM ve sorter dosyalarının
çalışma anındaki tepe disk alanı ölçülmedi.

SQLite ayrı tam integrity kontrolü 281.006,04 ms sürdü ve geçti. Normal open
bu kontrolü yapmadığından FDB open ile aynı doğrulama maliyetini taşımaz.
SQLite hazırlama (payload eşitliği dahil) 504.525,15 ms, son optimize/checkpoint
21.454,49 ms; bunlar FDB yazma hızıyla karşılaştırılmadı.

SQLite proses wall medyanı 444,36 s, CPU medyanı 209,88 s; FDB 123,93 s ve
119,59 s. Bunlar open, corpus yükleme ve warmup dahil bütün proses penceresidir;
yalnız timed query CPU süreleri değildir. Wall/CPU farkını yalnız serileştirme
veya yalnız I/O'ya atfetmek için ayrı profil yapılmadı. Cache'in tamamen sıcak
olduğu garanti edilmediği için sonuç CPU-only arama üstünlüğü göstermez.

Özellikle SQLite burada count ve page için iki ayrı arama geçişi, FDB aynı
API çağrısında toplam + sınırlı sayfa seçimi kullanır. SQLite aynı row içinde
orijinal payload'ın ardından folded metin saklar. Bu SQL/LOB yerleşimi ve
2 MiB istenmiş page cache ölçümün parçasıdır; tek geçişli window/count,
ayrı normalleştirilmiş metin tablosu, farklı cache/mmap düzenleri ölçülmedi.
Böyle bir değişiklik yeni ölçüm gerektirir; bu sonuçtan genel bir motor
üstünlüğü çıkarılmaz.

[Altı ham proses, medyanlar, eşleşmiş oranlar ve doğruluk kontrolleri](benchmarks/long-sqlite-20261011.summary.json).
[Önceki dosya boyut/hash'leri](benchmarks/long-sqlite-files-20261011.before.json)
ve [sonraki kayıt](benchmarks/long-sqlite-files-20261011.after.json) birebir aynı:
FDB kaynağı/ready/idx ve SQLite ana dosyası değişmedi.

## İş yükü ve eşdeğer sonuçlar

Önceki kısa/tekrarlı corpus karşılaştırmasından ayrıdır. 100.000 benzersiz
SHA-256 gövdeli gerçek kitap pasajı, 2–50 KiB UTF-8, toplam gövde
2.655.160.287 byte; seed 20261009. Sekiz Gutenberg İngilizce kitabından
örtüşebilen pencereler; 100.000 bağımsız eser veya Türkçe performans ölçümü
değildir. [Kaynak ve corpus hash'leri](benchmarks/rkyv-corpus-20261009.provenance.json).

SQLite hazırlamasında yeniden oluşturulan bütün 100.000 StoredDocument JSON
payload'ı, FDB benchmark kaynağını üreten `json.payload-bench` kayıtlarıyla byte
byte karşılaştırılır. Key, title, text, fields, links ve version aynıdır.
SQLite ayrıca normalleştirilmiş title + newline + text saklar. Sorgu terimleri
whitespace ile ayrılıp AND uygulanır; terimler whitespace içeremediği için
başlık/gövde sınırındaki newline üzerinden yeni bir eşleşme oluşmaz.
Bu araç literal `%`, `_`, backslash içeren veya boş sorguyu reddeder; genel
FDB API uygulaması değildir.

Sorgular ve toplamlar: `the` 100.000; `abbreviation` 264; `of` 100.000;
`the whale` 16.537. Her sorguda offset 0 ve 50, `the` için ayrıca 99.950.
Tam toplam + key sıralı 50 tam StoredDocument döner. Her çağrı timer dışında
bağımsız oracle ile tam belge, sürüm, sıra ve toplam bakımından doğrulanır;
snapshot sequence 200'dür. SQL sorgusu bir transaction içinde count + sıralı
rowid/key sayfası + yalnız seçilmiş 50 payload'ın yüklenmesi/JSON çözülmesini
kapsar. Büyük payload'lar sıralama tablosuna taşınmaz. EXPLAIN planları kaydedilir.

## Motorlar ve koşullar

FDB default release, ASCII/scratch normalleştirme optimizasyonu açık;
experimental feature'lar ve diagnostic timing kapalı. Dondurulmuş binary:
`output/fdb-engine-fold-candidate-20261011.exe`. Aynı, mevcut
`data/engine-fold-v2-20261011` kullanılır: eski JSON kaynak, tek SEG03 disk
segmenti, sıfır delta/resident belge. `.fdb`/`.ready` kaynağı açılışta tamamen
doğrulanır; türetilmiş `.idx` kullanılır. Yeni üretim veya format geçişi yoktur.

SQLite ayrı Rust `tools/native_sqlite` paketindeki `passages` binary'si;
FDB runtime bağımlılığı değildir. Mevcut Cargo.lock ile bundled rusqlite
0.40.2. Yeni izole `data/sqlite-passages-20261011`; WAL + synchronous FULL,
cache_size -2048 (2 MiB istek, toplam proses sınırı değil), mmap 0, temp_store
FILE. UNIQUE(scope,collection,id) ve external-content FTS5 trigram,
detail=none/columnsize=0; insert trigger'ı arama indeksini aynı işlemde kurar.
Orijinal JSON payload ve katlanmış metin birlikte saklanır. FDB katlanmış
disk gövdesi saklamaz, sorguda normalleştirir; bu maliyet/disk takası ölçümün
parçasıdır. Alan/link/guard indeksleri bakımından tam motor eşdeğerliği yoktur.

LIKE her terim için ayrı literal `%term%` parametresi kullanır. FTS5 trigram
LIKE'ı destekler; iki karakterli sorgu trigramla hızlandırılamaz ve tarama
gerektirir. ESCAPE kullanılmaz; corpus sorguları wildcard içermez.
[SQLite resmi trigram belgesi](https://www.sqlite.org/fts5.html#the_trigram_tokenizer).

Hazırlama ve optimize/checkpoint biter; ayrı SQLite integrity_check ve
external-content FTS integrity kontrolü yapılır. Ardından sorgu prosesleri
salt okunur SQLite açar. Normal SQLite open tam integrity değildir; FDB open
tam kaynak doğrulaması yapar. Açılış süreleri eşdeğer bir güvenlik sözleşmesi
olarak sunulmaz. Integrity ana sorgu veya RAM timer'ına dahil edilmez.

Üç yeni dönüşümlü proses çifti: FDB/SQLite, SQLite/FDB, FDB/SQLite. Eski FDB
ölçümleriyle yeni SQLite koşuları bölünmez. Her ilk sayfaya bir ölçümsüz warmup;
sonraki sayfalar önceki çağrılardan sonra. Uzun sorgularda proses/sayfa başına
bir timed örnek, dar sorguda on örnek; rapor üç proses p50 medyanıdır.
Güvenilir tail-latency iddiası yoktur. Zorlanmış cache tahliyesi veya dosyaların
tamamının OS cache'inde kaldığı doğrulaması yapılmaz. Derleme/test/profil/hash
taraması ana ölçümle eşzamanlı çalıştırılmaz.

RAM: 20 ms hedef aralıkla örnekli Windows PeakWorkingSet64 high-water ve private
bytes. Bütün native proses (open + aynı corpus planı/kitaplar/oracle + sorgular)
kapsanır; PowerShell monitor ve OS dosya cache'i hariçtir. Bunlar motor heap'i
veya RAM bütçesi değildir. Private örnekleme kısa pikleri kaçırabilir.

FDB hazırlanmış kaynağı okuduğu için ingestion/yazma adil olarak karşılaştırılmaz.
SQLite hazırlama süreleri yalnız fixture üretim kaydıdır. Soğuk I/O, çok
segment/delta, update/delete/guards, çok kullanıcılı yük ve genel SQLite
üstünlüğü bu testin kapsamında değildir.

## Tekrarlama

```powershell
cargo build --release --locked --manifest-path tools/native_sqlite/Cargo.toml --bin passages
Copy-Item tools/native_sqlite/target/release/passages.exe output/sqlite-passages-20261011.exe
# Yeni SQLite dizini gerekir; mevcut dizin ezilmez.
./output/sqlite-passages-20261011.exe prepare data/sqlite-passages-20261011 data/engine-fold-v2-20261011
./output/sqlite-passages-20261011.exe integrity data/sqlite-passages-20261011 data/engine-fold-v2-20261011
./tools/measure-long-sqlite-pairs.ps1 -Prefix yeni-long-sqlite
python tools/summarize-long-sqlite.py docs/benchmarks/yeni-long-sqlite
```

Yeni prefix kullanılır. FDB binary'si önceki doğrulanmış default release
artifact'ıdır; derlemelerle değiştirilmez. Binary kimlikleri:
[SHA-256 kaydı](benchmarks/long-sqlite-20261011.binaries.json).

## Kontroller

Ayrı native pakette üç test geçti. Yeni test, gerçek FTS5 tablo/trigger'ıyla
Türkçe eşlemeyi, iki karakterli sorguyu, başlık ve gövdeye dağılan AND terimlerini,
boş sonucu, offset sırasını ve wildcard reddini doğrular. Eski synthetic
benchmark'ın iki testi de korunur. Benchmark hazırlamasındaki 100.000 tam
payload byte eşitliği ve her timed/untimed sorgudaki tam sayfa oracle kontrolü
ayrıca gerçek corpus üzerinde çalışır.

Ana ölçümden önce root `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` ve ayrı native paketin
`cargo clippy --locked --manifest-path tools/native_sqlite/Cargo.toml --all-targets -- -D warnings`
kontrolleri geçti. FDB runtime, kalıcı formatlar, 0.1 fixture ve dış projeler
bu karşılaştırma görevinde değiştirilmedi.
Ölçüm bittikten sonra root/native fmt ve clippy kontrolleri, native üç test
yeniden geçti: [test günlüğü](benchmarks/long-sqlite-tests-20261011.log).

[Hazırlama](benchmarks/long-sqlite-prepare-20261011.jsonl),
[ayrı tam integrity kontrolü](benchmarks/long-sqlite-integrity-20261011.jsonl),
[makine ve sürümler](benchmarks/long-sqlite-20261011.environment.json).
Hazırlama payload üretimi/normalleştirme/byte karşılaştırma/200 sync batch'i
kapsar; FDB ingestion süresinin eşdeğeri olarak kullanılmaz.
