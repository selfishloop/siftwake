# Değişmez kaynak ömrü ve küçük sayfa denetimi — 2026-10-15

Bu adım `experimental-builder-budget` içinde, default kapalıdır. Yeni format,
feature veya bağımlılık eklenmez. Windows'ta düz dosya yollarındaki değişmez
Run kaynakları doğrulamadan **önce** yalnız `FILE_SHARE_READ` ile açılır.
Doğrulanan aynı dosya handle'ı `Arc<DiskIndex>` ömrünce tutulur; merge işçisi
de aynı sahibi taşır. Bu süre boyunca diğer süreçlerin yazma, truncate,
silme ve rename işlemleri işletim sistemi tarafından engellenir. Önceden
açılmış bir writer varsa bu açılış hata verir; sessizce güvenilir sayılmaz.

Bu güvenceyle kapsanan aramada source path metadata çağrısı tekrarlanmaz.
Index handle'ın boyut/tarih kontrolü sürer. İlk kaynak CRC/schema/key/version
doğrulaması ve ertelenmiş kontrolün aramadan önce tamamlanması korunur.
Aktif WAL ve legacy kaynak bu korumaya alınmaz. Windows dışı platformlar ve
reparse/junction içeren yollar önceki metadata denetimini kullanır. Yolun
bileşenleri restrictive open öncesi ve sonrası incelenir. Junction dalı canlı
test edilir; symlink oluşturma yetkisine dayanmaz.

İşletim sistemi sözleşmesi: [Rust share_mode](https://doc.rust-lang.org/std/os/windows/fs/trait.OpenOptionsExt.html#tymethod.share_mode),
[Windows CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).
Açık tam integrity API'si de kalır; dosya paylaşım sözleşmesi tam içerik
doğrulamasının yerine sunulmaz.

## Ölçüm kapsamı

Mevcut `data/rkyv-corpus-20261009/json.payload-bench` içindeki aynı ilk10k
belge; yeni corpus yok. FDB `data/benchmark-work/fdb`, SQLite
`data/benchmark-work/sqlite` sabit alanları. Read-only ölçüm mevcut DB'yi
kullanır. Kaynaklar `.fdb/.ready`, `.idx` türetilmiş cache; hiçbirinin biçimi
değişmez. Reader'lar release/builder-budget/record-read, normal ölçümde
diagnostic kapalı. Sıcak OS cache, eviction yapılmaz. P95 CPU süresi değildir.

Kontrol executable'ı `output/source-lease-reads-control-20261015.exe`;
önceki kabul edilmiş normalizer ve metadata guard. Yeni reader yalnız kaynak
ömrü değişikliğini içerir. Sekiz küçük/ileri sayfa: her proses200 örnek,
üç paired proses/600 pooled örnek. Karma38-case matrix: normal20/page5
örnek/proses. Çift sırası control/candidate, candidate/control,
control/candidate. Aynı query/offset/limit/total, tam belge/key/version ve
sequence oracle ile kontrol edilir. Ayrı256 sabit random-get de ölçülür.

## Sonuçlar

Yoğun sayfa testinde5/8 pooled p95 düşük,3/8 yüksek:

| Sorgu / offset / limit | Kontrol p95 ms | Yeni p95 ms | Fark | Hızlı çift |
|---|---:|---:|---:|---:|
| the /0/1 |0,8123 |0,2459 |-%69,73 |3/3 |
| the /0/200 |6,2281 |6,0061 |-%3,56 |2/3 |
| the /5000/50 |2,5960 |2,1216 |-%18,27 |3/3 |
| the /9950/50 |2,3464 |1,8691 |-%20,34 |3/3 |
| ishmael /130/1 |7,2506 |7,5018 |+%3,46 |1/3 |
| ishmael /60/200 |14,0892 |14,0278 |-%0,44 |2/3 |
| quixote /28/10 |2,3741 |2,5193 |+%6,12 |0/3 |
| quixote /57/50 |2,1724 |2,3338 |+%7,43 |1/3 |

Hedef tek-sonuç ve iki ileri sayfa kazancı üç çiftte de tekrarlandı.
Üç artış açık tutulur; önceki grouped-read ölçümüne verilen dalgalanma
kabulü bu yeni sonuçlara taşınmaz. Uzun/kanıtsız sorgularda dosya guard'ı
baskın değildi; o sorgulardaki değişimler bu kısayola doğrudan bağlanmaz.
[600 örnekli sayfa özeti](benchmarks/source-lease-pages-10k-20261015-summary.json).

Geniş38-case matrix'te14 pooled p95 düşük,24 yüksek. Tek-sonuç sayfası
1,3141→0,3656ms (-%72,18),3/3 hızlı; the/man/he ilk50 sonuç da sırasıyla
-%20,66/-%23,24/-%31,20 ve3/3 hızlı. the ileri5000/9950 sayfaları
-%25,75/-%20,32 (2/3 ve3/3 hızlı). Diğer artışlar kapatılmaz: örneğin
zx226,96→245,20ms (+%8,04), quixote /28/10 3,72→6,31ms (+%69,57).
Sonuncusu geniş testte yalnız15 örnek;600 örnekli bağımsız sayfa testinde
de +%6,12/0/3 hızlı olduğu için görmezden gelinmez.

Pooled p95 ile proses p95'lerinin çift karşılaştırması farklı istatistiklerdir.
Örneğin the zx pooled223,41→248,07ms (+%11,04) iken proses p95'leri üç
çiftte de düşük. Bu çelişki otomatik bir başarı veya dalgalanma kararı
değildir; ham dağılımlar korunur. Geniş kapı geçmedi; default'a geçiş yok.
Dar ve tekrar eden kaynak-guard kazancı mevcut deneysel profil içinde tutulur.
[38-case özeti ve tek SQLite referansı](benchmarks/source-lease-matrix-10k-20261015-summary.json).

SQLite3.53.2 mevcut read-only DB ile ayrıca dört sorgu ×30 örnek ölçüldü;
cache2MiB, mmap0, temp_store FILE, aynı fold/contains oracle, sıcak OS cache.
Tek proses ve ayrı zaman aralığıdır; üç çift SQLite karşılaştırması veya
genel SQLite üstünlük kapısı değildir. Açılış4,2611ms full-integrity yapmaz.
Bu adımın kabul edilen hız sayıları FDB'nin önceki sürümüne göredir.

## Maliyet profili

Ayrı release/diagnostic-timing/record-read, üç paired proses ×3 warm iteration.
İç içe wall span'lerdir, CPU attribution değildir; toplanmaz ve normal p95
ile karıştırılmaz. the /0/1 için proses medyanlarının medyanı:

| Span | Kontrol ms | Yeni ms | Çağrı kontrol→yeni |
|---|---:|---:|---:|
| Sorgu toplamı |0,4650 |0,1070 |1→1 |
| Source/index guard |0,3663 |0,0357 |5→5 |
| Source path metadata |0,3336 |0 |5→0 |
| Index handle metadata |0,0313 |0,0345 |5→5 |
| Gövde fold/scan |0 |0 |0→0 |
| Sayfa hydration |0,0254 |0,0232 |1→1 |

Kaynak path çağrıları kaldırıldı; index kontrolü devam ediyor. İlk aramada
her iki executable'ın her prosesinde10.000 belge source doğrulamasından
geçti; pending5→0. İlk kaynak doğrulaması kaldırılmadı. Bu adım açılış veya
ilk arama hızlandırması olarak sunulmaz; bu sürelerdeki farkların tümü
değişikliğe bağlanamaz. Önceki normalizer turundaki +%66,89 farkın nedensel
teşhisi ayrıca kanıtlanmış sayılmaz; güncel hedef sayfa iki iş yükünde
yeniden ölçülüp hızlandı.
[Profil ve doğrulama sayaçları](benchmarks/source-lease-profile-20261015-summary.json).

## RAM, yazma ve disk

Üç eşleştirme, iki sürümde de release/builder-budget/commit_streamed;
aynı ilk10k,500 kayıt/transaction, synced WAL. Her ingest yalnız sabit FDB
slotunu sıfırlar; toplam altı tekrar, yeni corpus veya DB kopyası yok.
SQLite slotu sıfırlanmaz. Ayrı bellek prosesinde corpus tek belgeyle okunur;
books/query oracle resident değildir.20ms proses örneklemesi/100ms phase
poll, idle açılıştan5s sonra, arama yok. Medyanlar:

| Ölçüm | Kontrol | Yeni | Çift durumu |
|---|---:|---:|---|
| Yazma |26,667s |27,445s |1/3 hızlı |
| Bakım |1,007s |0,991s |2/3 hızlı |
| Yazma+bakım |27,609s |28,436s |1/3 hızlı |
| İndeksleme tepe private |9,195MiB |9,129MiB |3/3 düşük |
| Bakım tepe private |8,457MiB |8,145MiB |3/3 düşük |
| OS tepe working set |13,000MiB |13,027MiB |1/3 düşük |
| Idle private |1,695MiB |1,527MiB |2/3 düşük |
| Retained .fdb/.idx/.ready |1.521.041.630B |1.521.041.630B |3/3 aynı |

Toplam süre medyanı +%3,00; genel yazma hızlanması yok. Working set
medyanı +28KiB olduğundan sıfır RAM artışı da iddia edilmez. Private bytes
prosesin özel bellek sayacıdır; working set fiziksel resident sayacıdır.
Kernel handle belleği ve OS dosya cache'i bu sayılarla toplam sistem RAM'i
olarak ölçülmez. Sorgu oracle'ını bellekte tutan matrix prosesinin belleği
engine RAM'i diye sunulmaz; query tepe belleği/normal API ayrı açık.
[Her tekrar ve sayaçlar](benchmarks/source-lease-memory-10k-20261015-summary.json).

Altı ingestin her biri tam belge/query doğrulamasından geçti; yeni yazılan
DB üç kez eski reader ile de doğrulandı. Güncel source/index/WAL+ready
621.589.411B, SQLite583.860.224B; bu adım boyutları değiştirmedi. Eski nesil
birikimi ve canlı alan farkını azaltma işi kapanmadı. Son124 FDB/cache/SQLite
dosyasının hash'i ilk read-only batch öncesiyle aynı. Corpus2.740.862.279B,
SHA256 `11f826fce3f4ae790feb9fc6df9b1169f52c2fd699e89fc4a8a4b77aaa41731b`.
[Son dosya/corpus/fixture/artifact kanıtı](benchmarks/source-lease-final-proof-20261015.json).

## Doğrulama

Kaynak yazma/truncate/silme/file-directory-ancestor rename engelleme,
okuyucuya izin, son Arc bırakılana kadar koruma, önceden açık writer reddi,
WAL'a normal commit ve junction fallback testleri eklendi. Canlı bozulma
testleri Windows'ta kaynağın bozulmasını engellemeyi, kapanış sonrası
bozulmanın sağlam index/cache'le dahi yakalanmasını sınar. Index'e canlı
müdahale testi korunur. Merge kaynak korumasını kullanabilir; process kill
sonrası kaynaklar tekrar açılabilir ve onaylanmış veri kurtarılır.

Default, builder-budget ve all-features all-targets suite'leri geçti.
Kesilme/tek-bayt bozulma/sağlam cache + bozuk source/process kill/merge kill
ve normal-reader uyumluluk testleri dahil. Default ve all-features
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` geçti.
[Test sayıları ve loglar](benchmarks/source-lease-test-summary-20261015.json).
OS düzeyi ilk [yazma/rename probe'u](benchmarks/ownership-os-probe-20261015.log)
da başarılı; yalnız bir baytlık, otomatik temizlenen doğruluk dosyasıdır.

Diagnostic kapalı normal release yeniden build edildi. Son38-case tam
belge/version kontrolü geçti:
[son kontrol](benchmarks/source-lease-final-matrix-check-20261015.jsonl).
Fmt ve diff whitespace kontrolü geçti;0.1 fixture Git durumu boş. Son normal
reader'ın `.text` bölümü ölçülen normal candidate ile byte düzeyinde aynı;
bütün executable SHA'sı aynı değildir (9 farklı byte `.text` dışında).
[PE karşılaştırması](benchmarks/source-lease-final-pe-20261015.json).
Cargo.toml/lock ve normalizer SHA'ları değişmedi; başka proje/servis yok.

## Açık işler

[OPEN-WORK](OPEN-WORK.md): yazma/bakım, uzun/olumsuz normalleştirme-tarama,
diğer arama artışları, query tepe/normal API/genel heap, RAM/disk ve eski
nesiller, eşzamanlı yük ve soğuk cache kapsamı ayrı izlenir. SQLite'a karşı
genel üstünlük veya default'a geçiş bu dar adımdan çıkarılmaz. Dış proje
entegrasyonu yok;0.1 fixture değişmez.
