# WAL commit senkronlaması — 2026-10-09

## Ölçülen değişiklik

`experimental-wal-sync-control` feature'ı deneysel Rust API
`Database::set_wal_sync(false)` açar. Feature açıkken de her yeni/reopened Database
varsayılan olarak sync kullanır. JSON Lines CLI bu seçeneği açmaz.
WAL write_all, JSON/framing/CRC, guard, atomik batch, delta indeks güncellemesi
korunur; yalnız commit append sonrası sync_all atlanır. WAL tamamen kapatılmadı.
Checkpoint/source/manifest senkronlaması ve kalıcı codec değişmez.

`set_wal_sync(true)` bekleyen WAL byte'larını önce sync eder; başarısızlık motoru
poisoned yapar. Mod seçimi kalıcı formata yazılmaz. Sync kapalıyken onaylanmış son
işlemler sistem/elektrik kesintisinde kaybolabilir. OS cache'i hayatta bırakan
process-kill testi elektrik kesintisi güvenliği kanıtı değildir.

## Ölçüm sözleşmesi

Aynı frozen uninstrumented binary, on/off,off/on,on/off üç çift; yeni DB dizinleri.
Seçilmiş composite: wal-sync-control,merge-postings,shared-documents,
shared-delta-text,delta-bitmaps; builder-budget/WAL-delta/diagnostic kapalı.
Native API ve sıcak OS cache; ölçüm sırasında build/test/profile yok.
İki ayrı pencere:

* Native scale:100k,tekrarlı256 UTF-8 byte Türkçe gövde,benzersiz title/key,
  source/day,batch500,delta10k/16MiB. Corpus üretimi ingest'e dahil; final bakım
  ayrıca ve toplamıyla raporlanır. Query full total + sıralı50 tam StoredDocument,
 50update/50delete/reopen oracle timer dışında. Sync seçimi ilk ingest için;
  yeniden açılan DB varsayılan sync ile sonraki update/delete işlemlerini yapar.
* İzole commit:5000 aynı corpus, batch1/50/500,delta5001/64MiB. Timer sırasında
  flush/merge yok; indeks ve serde/CRC işi var. Ingest corpus üretimini içerir;
  batch quantiles commit timer'ıdır. Drop/reopen/tüm5000 payload+sürüm oracle
  timer dışında. Bu pencere nihai durable flush maliyeti içermez.

Her iki mod aynı veri işi yapar fakat aynı kalıcılık garantisini vermez.
Bu deney SQLite karşılaştırması değildir; eski SQLite FULL rakamı ile buffered
mod için eşit kalıcılıklı üstünlük iddiası yapılmaz. WAL'in tamamını kapatmanın
kazancı ölçülmedi. Cold disk, güç kesintisi,uzun/unique gövde ve eşzamanlı yazıcı
kapsam dışıdır. RAM bu deneyin karar kapısı değildir.

## Sonuçlar

Üç çift medyanı,5000 kayıt; flush/merge hariç ingest:

| Batch | Commit sync açık | Commit sync kapalı | Süre azalması |
| --- | ---: | ---: | ---: |
| 1 | 2305.0760 ms | 116.1456 ms | %94.96 (19.85x) |
| 50 | 139.0752 ms | 80.7563 ms | %41.93 (1.72x) |
| 500 | 93.1389 ms | 84.7659 ms | %8.99 (1.10x) |

Tek tek commit'te sync maliyeti baskın. Batch arttıkça aynı sync daha çok kayda
paylaşılır. Bu izole pencere bütün indeks bakımını bitirmiş durable DB oluşturma
süresi değildir. Özellikle sync-off işlemleri kalıcı diske taşınmayı beklemeden
onaylanır;19.85x aynı kalıcılık garantisiyle kazanılmış hız değildir.

100k,batch500, bakım dahil medyanlar:

| Pencere | Açık | Kapalı |
| --- | ---: | ---: |
| Ingest | 6.2047 s | 5.8489 s |
| Son bakım | 4.1907 s | 4.4257 s |
| Ingest + son bakım | 10.3954 s | 10.2658 s |

Ingest %5.73 azalır; toplamda %1.25 fark kalır. Küçük toplam farkı kesin
genel hız artışı sayılmaz; worker/son bakım süreleri koşular arasında değişir.
Bu batch500 iş yükünde sync kaldırılması bakım darboğazını çözmez. Tek kayıt
commit kullanan iş yükü için ise izole fark bütün üç çiftte belirgindir.

Scale altı ve commit18 koşuda full oracle/reopen kontrolleri geçti. Commit
on/off çiftlerinin tüm WAL dosyaları SHA256 eşit; scale çiftlerinin final ready,
iki fdb ve iki idx dosyası byte eşit. Özetler:
`benchmarks/wal-sync-20261009-{scale,commit}.summary.json`,
`benchmarks/wal-sync-20261009-scale.byte-equality.json`.

## Tekrar üretim komutları

```powershell
cargo build --release --example measure_scale --example measure_wal_sync --features experimental-wal-sync-control,experimental-merge-postings,experimental-shared-documents,experimental-shared-delta-text,experimental-delta-bitmaps
# Frozen binary kopyalarını output/fdb-wal-sync-{scale,commit}-20261009.exe olarak sakla.
./tools/measure-wal-sync.ps1 -Prefix wal-sync-20261009
python tools/summarize-native-pairs.py docs/benchmarks/wal-sync-20261009-scale --engines on off
python tools/summarize-wal-sync.py wal-sync-20261009
python tools/verify-native-publications.py data/wal-sync-20261009-scale --engines on off
```

Scale SHA256 `7115553B915ADE91EF855FEE0D29778EC7CF88ECA5B5809CD5C13E89C94DB4A0`.
Commit SHA256 `3FB37C9CFE729111659A27A7E71A79739C543BD7A389D6F6F64109696CB8C593`.
Raw `benchmarks/wal-sync-20261009-{scale,commit}-*`; sonuç özeti oracle kontrolü
ve eş WAL/publication byte kontrolünden sonra kaydedilir.

## Doğrulama

İki mod için WAL byte eşitliği, buffered frame'in her eksik byte kesiminde
önceki commit'e dönüş, bozuk tamamlanmış frame reddi ve sync'i tekrar açma
sonrası reopen testleri eklendi. Buffered subprocess acknowledgement sonrası
kill/reopen testi de geçti. Mevcut varsayılan process-kill, kaynak/cache bozulma
ve kurtarma suite'leri ayrıca geçti. Default49,seçilmiş composite87,all-features92
test geçti; buffered process-kill ayrıca standalone çalıştırıldı. `cargo fmt
--check`, default/composite/all-features `cargo clippy --all-targets -- -D warnings`
geçti. Varsayılan release CLI yeniden derlendi. Test alt süreci helper'ı normal suite'de
ignored olup parent test tarafından açıkça çağrılır.
