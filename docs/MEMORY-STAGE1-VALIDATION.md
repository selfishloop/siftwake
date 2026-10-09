# Bellek planı — aşama 1 doğrulaması — 08.10.2026

Bu belge 0.3.1 üzerindeki geliştirme prototipini anlatır; SQLite'tan düşük toplam
proses RAM'i hedefinin tamamlandığını iddia etmez. Hazır motor yalnız izole native
SQLite karşılaştırma paketindedir. FDB manifesti/lockfile'ına DB bağımlılığı
eklenmedi; SDB/Fundops'a bağlantı, veri taşıma veya entegrasyon yapılmadı.

## Uygulananlar

- `tools/native_sqlite`: bağımsız Rust + SQLite uygulaması. Ortak Rust corpus
  üreticisi, aynı StoredDocument JSON, scope/collection/id, source/day ve FTS5.
- Native prosesleri aynı PowerShell monitörüyle sırayla izleme; istenen aralık
  20 ms. Working set ve private committed bytes ayrı; OS file cache hariç.
- Opt-in Rust heap profilleri: `profile_memory` ve `profile_writes`. Normal
  motor allocator'ına atomic sayaç eklenmedi.
- `Database::index_memory_layout()` ile seçilmiş vektör/String capacity
  miktarlarının teşhisi; BTreeMap düğümleri veya toplam RAM sayacı değildir.
- `offset+limit <= 4096` için tüm sonuç ID'leri yerine run başına en fazla bu
  kadar ID; max heap ile en küçük slot'lar, ardından sıralı sayfa birleştirme.
  Toplam sonuç sayısı, sıralama, sürümler ve payload'lar korunur. Checked addition
  taşarsa veya offset büyürse önceki tam-listeli yol kullanılır.
- Metinsiz sorgunun her filtresi run'ın bütün canlı kayıtlarını kapsıyorsa
  görünürlük maskesinden exact total; gizlenmiş tombstone'lar canlı sayımdan düşülmez.
  Metin adayları özgün davranışla doğrulanır. Cache/source açılış kontrolleri korunur.
- ID havuzunun tam uzunlukla ayrılması; gereksiz capacity ve büyütme kopyaları azalır.

FDBLOG01/03, FDBMAN03, FDBSEG03 değişmedi. Cache ve kalıcı kaynak yeniden formatlanmadı.
0.1 fixture kaynakları değiştirilmedi. Sync, guard, atomik commit ve kurtarma
kuralları korunur. Büyük offset fallback'i ayrı tutuldu; onun hot loop'una
sınırlı collector/ek count overhead'i eklenmez.

## Native SQLite: yeni referans

Windows / Ryzen 7 7840HS / release, bir milyon kayıt, tekrarlı 256 UTF-8 bayt
Türkçe içerik, 500 kayıtlık 2000 batch. Aynı Rust üreticisi ve payload tipi.
WAL + FULL, requested cache 2 MiB, mmap=0, temp_store=FILE, unique anahtar,
source/range indeksleri ve external-content FTS5 trigram detail=none/columnsize=0.
Normal reopen, count+50 decoded payload, update/delete, geniş metin ve ayrı
tam SQLite/FTS bütünlük kontrolü aynı benchmark prosesinde ölçüldü.

| SQLite 3.53.2, native Rust proses | Tek tam koşu |
| --- | --- |
| Gözlenen tepe working set | 9.4765625 MiB — 9.936.896 bayt |
| Örneklenen tepe private committed | 5.21875 MiB — 5.472.256 bayt |
| Ingest | 68,845 s |
| Son bakım | 2,167 s |
| Ingest + son bakım | 71,011 s |
| Metadata count+50 medyan | 131,496 ms |
| Range count+50 medyan | 36,855 ms |
| Dar başlık medyan | 154,248 ms |
| Geniş metin medyan, değişikliklerden sonra | 3,159 s |
| SQLite + FTS bütünlük kontrolü | 16,053 s |

50 update/50 delete ve yeniden açılış doğrulandı. Ayrı benchmark unit test'i
Türkçe katlama, trigger update/delete davranışı, duplicate batch rollback ve
FTS content integrity'yi doğrular. Bu native ölçümde büyük corpus üzerinde
SQLite owner process kill testi ayrıca yapılmadı; önceki Pythonlu kill testi
bu native koşunun testi olarak sunulmaz.

Native paket root motordan ayrıdır; `cargo tree --edges normal` root yalnız
serde/serde_json zincirini gösterir. SQLite 3.53.2 ile önceki Pythonlu 3.53.1
sonucu aynı sürüm/harness değildir. Önceki 20,21 MiB Python+SQLite ve 137,404 s
ingest sonucu native motor performansı/belleği olarak kullanılmaz. Özellikle
FDB'nin önceki Pythonlu karşılaştırmada hızlı olması native üstünlük değildir.

Native SQLite commit p50/p95, işlem öncesinde yapılan JSON/fold hazırlığını
hariç tutar; FDB commit'in bir bölümü bunu içerir. Bu yüzden birincil yazma
ölçütü tüm generation/serialization dahil ingest + bütün son bakımdır.
Native schema genel FDB field/link/guard API'sinin tam uygulaması değildir;
karşılaştırma bu source/day/literal substring corpus'uyla sınırlıdır.

## Heap profili: büyük maliyetin yeri

İlk profil, sağlıklı eski milyonluk benchmark DB'sinde 15 run, 999.951 canlı
kayıt, sequence=2002 üzerinde alındı. Normal open source/cache doğrulamasını
yapar; profiler commit/compact göndermedi.

| İlk profilde Rust tarafından istenen allocation | Bayt |
| --- | --- |
| Open sonrası canlı heap | 60.318.610 |
| Open'da heap tepe | 91.532.658 |
| Sabit kayıt konumu vektörlerinin capacity'si | 40.004.000 |
| ID havuzlarının capacity'si | 18.351.360 |
| Bu iki yapının toplamı | 58.355.360 |
| İlk sayfa sorgusunda ek heap tepe | yaklaşık 64.154 |
| Önceki tam-listeli offset yolunda ek heap tepe | yaklaşık 7.401.442 |

İki kapasite, open sonrası istenen heap'in yaklaşık %96,7'sidir. Bu corpus'ta
token sözlüğü nispeten küçük; farklı/uzun metinlerde böyle kalacağı varsayılmaz.
ID havuzunun kullanılan baytları 10.001.000; tam rezervasyonla yaklaşık
8.350.360 bayt gereksiz kapasite kaldırılır. Gerçek proses working set'i veya
private bytes bu capacity toplamına eşit değildir.

100.000 kayıtta ayrı instrumented yazma profilinde gözlenen cumulative requested
heap tepe 92.432.867 bayt, son bakım sonrası canlı heap 5.450.723 bayt idi.
Ön plan delta + worker/builder transient'leri birlikte önemlidir. Bu ilk yazma
profili tam ID rezervasyonu değişikliğinden önce alındı. Atomic instrumentation
süreleri release performans karşılaştırması olarak kullanılmaz.

Allocation wrapper istenen Rust heap boyutlarını izler. Allocator metadata,
fragmentation, realloc'ın içerideki geçici kopyası, C allocation, stack ve OS
file cache dahil değildir. Phase ek tepe, o aşamanın başlangıç canlı heap'i
üzerindeki artıştır; toplam proses RAM'i diye sunulmaz.

## Geliştirme ve ölçüm kapısı

İlk sıralı iterator denemesi tüm sonuç vektörlerini kaldırdı fakat exact/range
hot loop'u yavaşlattı; kabul edilmedi. Blok taraması ve küçük bounded seçim
kullanıldı. Sonraki ortak collector'ın offset fallback'inde ölçülen ek maliyeti
de kabul edilmedi; eski full-list loop'u ayrı tutuldu. Küçük offset'ler de bounded
prefix yoluna alındı; bu, ilk sayfanın yanında normal numaralı sayfaları kapsar.

Eski 0.3.1 binary ile aday aynı DB'de, sırayla ve dönüşümlü test edilir.
Prosesler aynı anda çalışmaz. Her sorguya bir untimed warmup; her binary için
beş bağımsız proses koşusu. Dar başlık `rapor-0000500001`, exact `source=synthetic`,
range exact+day2..5, ikinci sayfa offset50, son sayfa offset999900. Count ve dönen
payload/sürüm/sıralama JSON sonuçları birebir karşılaştırılır. Dar başlık önceki
`...500000` ile aynı sorgu değildir; güncellenmiş corpus'ta o kayıt silinmiştir.

Süreler yerel sıcak OS cache ile JSON Lines istemcisi dahil wall time'dır.
Kaynak doğrulaması yapan open süreleri ayrı tutulur. Native SQLite dar başlık
önceki query/corpus aşamasında alındığı için burada hız oranı hesaplanmaz.
Geniş metinde beş koşulu ara aday karşılaştırması yaklaşık 3,722 vs 3,742 s
verdi; belirgin hız artışı ölçülmedi. Son bounded-prefix değişikliği sonrası
geniş metin, process-kill recovery kontrolünde tekrar doğrulanır.

İlk adayda tek tam FDB
ingest 107,044 s ve son bakım 7,738 s idi; tepe proses working set 159,27 MiB.
Bu, son prefix/ID rezervasyonu değişikliklerinden önceki adaydır; yeni son tepe
ölçümü yerine sunulmaz. Native ingest/bellek hedefi henüz karşılanmış değildir.

### Son bounded-prefix aday: beş proses koşusu

Tablo koşu medyanlarının medyanıdır; p95 de her koşunun p95 değerlerinin
medyanıdır. Sıcak OS cache ve stdio istemcisi dahil, aynı sağlıklı milyonluk DB.
Son sayfa her koşuda 3 kez, diğer sorgular 30 kez zamanlandı; warmup hariç.

| Sorgu | Eski p50 | Son aday p50 | Eski p95 | Son aday p95 |
| --- | --- | --- | --- | --- |
| source exact, ilk sayfa | 11,506 ms | 0,939 ms | 12,497 ms | 1,315 ms |
| source + range | 4,436 ms | 3,089 ms | 5,038 ms | 4,191 ms |
| source exact, ikinci sayfa | 11,549 ms | 0,978 ms | 12,466 ms | 1,304 ms |
| Dar başlık | 9,041 ms | 8,853 ms | 9,995 ms | 9,432 ms |
| Son sayfa | 201,682 ms | 201,387 ms | 201,779 ms | 204,641 ms |

Metadata ilk sayfa yaklaşık 12,3 kat, ikinci sayfa yaklaşık 11,8 kat hızlı;
range medyanı yaklaşık %30 daha kısa. Dar başlık/son sayfada belirgin hız artışı
iddiası yok. Son sayfa p95 biraz yüksek ve az tekrar içerir; tüm iş yükleri için
kesin non-regression garantisi değildir. JSON snapshot sonuçları, toplamlar,
payload'lar ve sıra her istekte eski binary ile birebir eşleşti.

Son aday SHA-256: `97A92D73691421615DDEB963742AEBFB913DD26B2973A21CCE55D79F3CFE8511`.
Eski 0.3.1 SHA-256: `76EFCCF7E242FA3B65229B834B8915E725EE82231A04ABA4552C2A78DBE9F902`.
Binary'ler yerel output altında korunur; Root package sürümü henüz 0.3.1,
bu değişiklikler ayrı yayımlanmış release değildir.

### Son heap ve proses belleği

Yeni requested canlı heap 51.968.250 bayt; open tepe 83.182.578 bayt.
Kayıt konumu capacity'si 40.004.000, ID capacity'si 10.001.000 bayt: birlikte
50.005.000 bayt, yaklaşık 47,69 MiB. Bu iki yapı henüz RAM'dedir. İlk sayfa exact,
range ve geniş metinde ek requested heap tepe yaklaşık 64 KiB; offset5000
fallback'inde yaklaşık 7,40 MB. Sınırlı prefix, toplam query RAM kotası değildir:
run sayısı, maskeler, token grup vektörleri ve payload'lar ayrıca vardır.

Yeni açılmış normal CLI, stats'tan 1 saniye sonra:

| Proses sayacı | Son aday |
| --- | --- |
| Boşta working set | 58,49 MiB — 61.329.408 bayt |
| Boşta private committed | 57,35 MiB — 60.137.472 bayt |
| Açılışta gözlenen tepe working set | 85,68 MiB — 89.837.568 bayt |
| Açılış + stats | 5,129 s |

8,35 MB capacity azalması fiziksel working set'te aynı miktarda düşüş değildir.
Önceki 0.3.1 boşta WS 58,81 MiB idi; buradaki fark küçüktür. Private bytes
önceki 63,14 MiB'den azdır; tek koşu ve allocator/OS durumu nedeniyle tam bir
kapasite→RSS dönüşümü çıkarılmaz. Boşta FDB ile SQLite ingest tepesini eş fazlı
RAM karşılaştırması gibi sunmayız. Toplam proses RAM hedefi hâlâ sağlanmadı.

Son binary ayrı milyonluk benchmark DB'sinde sentinel sequence=2002 yanıtını
verdikten sonra gerçek owner process kill edildi. Reopen 999.951 kayıt,
sentinel ve önceki 50 update/delete'i doğruladı. `teknoloji` 999.900 eşleşme:
3,749 / 3,753 / 3,687 saniye stdio süreleri. Bu üç tekrar geniş metinde belirgin
iyileşme iddiası için yeterli değildir; semantik ve kurtarma doğrulamasıdır.

## Kontroller ve kalan kapsam

Root: 44 test (3 unit, 11 disk, 17 engine, 13 segment), fmt ve clippy -D warnings.
Yeni testler numeric union + birden fazla collection/run + delta/update/delete
ile sınırlı sayfa sıralamasını ve canlı sayımdan tombstone dışlamasını doğrular.
Ayrıca 4096 prefix sınırının iki tarafı ve usize::MAX offset taşması doğrulanır.
Mevcut byte corruption, her transaction cut, process kill, randomized reference,
cache yeniden üretimi ve 0.1 uyumluluk kontrolleri çalışır. Native comparator:
ayrı unit test, fmt ve clippy -D warnings geçti.

Henüz uygulanmayanlar: diskte sayfalı anahtar/konum ve token sözlüğü, tek ortak
allocation bütçesi, akışlı builder/replay, normalize metin blokları, büyük offset
rank cursor ve büyük transaction/sonuç streaming API'si. Bu aşama query ID
belleğini azaltır ve doğru native referansı kurar; bütün planı tamamlamaz.
Soğuk I/O, çeşitli uzun gerçek belge corpus'u, disk dolması, gerçek elektrik
kesintisi, çok kullanıcılı concurrency ve native tüm iş yüklerinde üstünlük
ölçülmedi. Başlangıç hedefi native ölçüm nedeniyle 8–9 MiB proses working set ve
native SQLite'ın altında private bytes olacak şekilde plana geri işlendi;
bu yeni bütçe henüz motor içinde uygulanmış limit değildir.

## Kanıtlar ve yeniden çalıştırma

[Native SQLite koşusu](benchmarks/native-sqlite-memory-stage1.jsonl),
[native proses belleği](benchmarks/native-sqlite-memory-stage1.memory.json),
[ilk FDB tam aday koşusu](benchmarks/fdb-memory-stage1-initial-candidate.jsonl),
[ilk aday proses belleği](benchmarks/fdb-memory-stage1-initial-candidate.memory.json),
[son beş koşu](benchmarks/fdb-memory-stage1-reads.jsonl),
[son süre özeti](benchmarks/fdb-memory-stage1-reads.summary.json),
[son heap profili](benchmarks/fdb-memory-stage1-heap.jsonl),
[ilk yazma heap profili](benchmarks/fdb-memory-stage1-initial-write-heap.jsonl),
[son boşta proses](benchmarks/fdb-memory-stage1-idle.json),
[son kill recovery](benchmarks/fdb-memory-stage1-recovery.json).

```powershell
cargo test --all-targets
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --release --bins --examples
cargo test --manifest-path tools/native_sqlite/Cargo.toml
cargo clippy --all-targets --manifest-path tools/native_sqlite/Cargo.toml -- -D warnings
cargo build --release --locked --manifest-path tools/native_sqlite/Cargo.toml
# Yeni benchmark klasörleri kullanın; işler sırayla çalıştırılır.
.\tools\measure-native.ps1 -Executable tools/native_sqlite/target/release/fdb-native-sqlite-bench.exe -DatabasePath data/new-native -OutputPrefix output/new-native
.\tools\measure-native.ps1 -Executable target/release/examples/measure_scale.exe -DatabasePath data/new-fdb -OutputPrefix output/new-fdb
.\target\release\examples\profile_memory.exe data/healthy-million
.\target\release\examples\profile_writes.exe data/new-profile-writes 100000
python -X utf8 tools/compare-fdb-reads.py output/fdb-0.3.1-before-memory.exe target/release/fdb.exe data/healthy-million --runs 5 --skip-text > output/new-read-comparison.jsonl
python -X utf8 tools/summarize-fdb-reads.py output/new-read-comparison.jsonl
```

Full comparator (skip-text olmadan) geniş metni de kapsar. Kaynak benchmark DB
ve eski binary path'i doğru verilmelidir; helper son sayfa ve başlık sorgularını
bu bir milyonluk, 50 update/delete + sentinel corpus'uyla eşler. FDB normal open
cache rebuild/recovery yapabilir; sağlıklı test verisi kullanılır. Başka bir
query/corpus/cache durumu için bu rakamlar genellenmez.
