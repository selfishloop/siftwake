# WAL konumlu delta deneyi — 2026-10-09

## Karar

`experimental-wal-delta` uygulanıp ölçüldü. **Varsayılan motora alınmadı:**
tam belge nesneleri yerine active WAL konumlarını tutmak yazma RAM'ini azaltıyor,
fakat flush edilmemiş delta sorgularını yavaşlatıyor. Hızdan ödün vermeden
SQLite'tan düşük toplam proses belleği hedefi tamamlanmış değildir.
SQLite bu aşamada yeniden ölçülmedi; eski SQLite ölçümünü bu yeni eşleşmiş
FDB karşılaştırmasına dahil ederek üstünlük iddia etmiyoruz.

## Uygulama ve kalıcı veri

RAM delta tablosunda Key + offset/u32 length/CRC/version saklanır; tam
StoredDocument, fields ağaçları, original title/text/links burada tutulmaz.
Index'in folded metni ve posting'leri RAM'de kalır. Shadow tablosu/anahtarlar
hâlâ çoğalır, root/dictionary/builder ortak toplam byte bütçesinde değildir.

Commit JSON'unu aynı TXN1/JSON biçiminde yazarken her StoredDocument byte
aralığı kaydedilir. Önce batch/guard/capacity ve maintenance tamamlanır;
konumlar **maintenance sonrası güncel WAL uzunluğuna** eklenir. Append +
sync başarılı olunca index ve delta referansları güncellenir. Kalıcı append
sonrası bir index/old-payload hatası motoru poisoned yapar. Guard/version ve
byte watermark için tam belge okunmaz veya yeniden serialize edilmez.

Recovery tüm frame CRC/footer/schema/sequence kontrollerini korur. Validated
Batch yanında borrowed RawValue'larla özgün StoredDocument JSON byte
aralıkları bulunur. Whitespace/field order değiştirilmez; offset bounds
kontrol edilir. Bu aşamada hâlâ bir tam WAL frame Vec'i ve decoded Batch'i
geçicidir; bounded streaming WAL recovery değildir. Final delta referansları
okunarak index kurulur. Canonical commit/recovery watermark aynı byte boyutunu
kullanır; eski noncanonical JSON'da özgün StoredDocument aralığının boyutudur.

Get/search/flush seçilen belgeyi positional I/O ile okur, CRC, key ve version
kontrol eder; tek belge scratch ve decoded payload oluşturur. Runtime source
bozulması sağlıklı resident index veya önceki bir get sonucu ile gizlenmez.
Flush referanslar okunmadan WAL değiştirmez; tamamlanmış kaynak+manifest
publication sonrasında active log/reference tabloları değiştirilir.

FDBLOG01/03, TXN1, MAN03, SEG03 ve MET04 biçimleri değişmez. 0.1 source
fixture'ları değiştirilmedi. `serde_json/raw_value` yalnız genel JSON parser
kabiliyetidir; motorun DB/arama bağımlılığı yoktur. Feature bağımsızdır ve
normal build'de kapalıdır. Normal yol hâlâ resident StoredDocument tutar.
Deneyde `resident_documents=0` tam resident belge sayısıdır; pending belge
sayısı değildir. `resident_text_bytes` retained index folded byte sayısını
korur; toplam proses RAM sayacı değildir.

## Yazma dahil native proses: üç eşleşmiş çift

Baseline frozen streaming-metadata V2; candidate buna WAL delta ekler.
Sıra B/C, C/B, B/C. Her koşu ayrı yeni DB, 100000 belge, 256 UTF-8 body
byte, batch500. Başlık `rapor-{id:010}`, key 10 hane, shared/contents,
day=id%30, source=synthetic, tekrar eden Türkçe içerik. Warm OS/file cache,
untimed query warmup; diagnostic allocator/timing yok. CPU test/build/profile
aynı anda çalıştırılmadı. Hidden proses, hedef 20ms örnekleme; Windows bunu
uzatabilir. PeakWorkingSet64 tüm open/ingest/bakım/reopen/query/50 update ve
50 delete penceresidir; OS file cache dahil toplam sistem RAM değildir.
Private peak örneklenir. Her sonuç independent full StoredDocument oracle,
exact total/order/version ve değişiklik sonrası reopen kontrolünden geçti.

| Ölçüm medyanı | Baseline | WAL delta |
| --- | ---: | ---: |
| Yazma | 5.8647 s | 6.1042 s |
| Son bakım | 5.1305 s | 5.3058 s |
| Yazma + bakım | 10.9833 s | 11.4063 s |
| Açılış | 527.1040 ms | 511.2863 ms |
| Proses tepe working set | 37.4375 MiB | 23.1055 MiB |
| Örneklenen private tepe | 35.2422 MiB | 20.4141 MiB |

RAM %38,28 azaldı; yazma+bakım %3,85 arttı. Maintenance ayrı medyanı,
sum ise her koşunun toplamının medyanıdır. Final 99950 görünür belge,
2 segment, 4589406 posting, source46428672/cache81627542 byte;
klasör431069691 byte inaktif nesilleri de içerir. Bütün koşularda aynı.

| Sorgu | Baseline p50 / p95 ms | WAL delta p50 / p95 ms |
| --- | ---: | ---: |
| narrow | 2.7961 / 3.7100 | 2.9367 / 3.7631 |
| broad | 0.1925 / 0.2102 | 0.1937 / 0.2390 |
| range | 0.4181 / 0.6363 | 0.4007 / 0.6100 |
| second_page | 0.1941 / 0.2943 | 0.1897 / 0.2247 |
| last_page | 1.3699 / 1.4224 | 1.9871 / 2.0271 |
| broad_text | 233.7568 / 238.1892 | 208.9780 / 209.1875 |
| get_100 | 1.5298 / 1.6145 | 1.4197 / 1.4922 |

Last-page p50 %45,05, narrow p50 %5,03 ve broad p95 %13,70 arttı.
Bunlar regresyon olarak korunur. Küçük sorgular30, broad-text/last/get100
üç timed repeat + warmup kullanır. Query'ler flush edilmiş DB üzerinde
olduğu için yeni WAL hydration maliyetini ayrıca ölçmek gerekir.

## Flush edilmemiş delta: yeni bağımsız hız kapısı

`measure_delta_queries` yeni DB'ye 10000 belgeyi batch500 ile yazar.
max_delta_keys10001/max_delta_bytes64MiB; segments0, delta_keys10000,
sequence20 doğrulanır. API flush yok, writes/open query timer dışında.
Aynı corpus, 10 warmup; narrow/exact/range/second200, last/get100100,
broad-text20 timed tekrar. Get id=n*97+1, narrow id5000, last offset9950.
B/C, C/B, B/C üç çift; oracle full page/version/order/total karşılaştırır.
Bu harness proses RAM ölçmez; yazma native harness'ının yerine geçmez.
Baseline mevcut source ağacında WAL feature kapalı; diğer dört feature aynı.

| Delta sorgusu | Baseline p50 / p95 ms | WAL delta p50 / p95 ms |
| --- | ---: | ---: |
| narrow | 0.0040 / 0.0045 | 0.0071 / 0.0075 |
| broad | 1.3422 / 2.1449 | 1.9023 / 2.6731 |
| range | 0.2279 / 0.3115 | 0.4016 / 0.5559 |
| second_page | 1.5504 / 2.3809 | 2.0932 / 3.3165 |
| last_page | 1.3692 / 2.0194 | 2.4234 / 3.4110 |
| broad_text | 2.0203 / 3.1081 | 2.5874 / 3.7445 |
| get_100 | 0.0762 / 0.1057 | 0.4532 / 0.6320 |

100 get p50 yaklaşık 5,95 kat; exact/range/last p50 %42/%76/%77 daha yavaş.
Mutlak farklar küçük olsa da “hızdan ödün yok” kapısını geçmez. Warm OS cache
JSON decode/positional read maliyetini kaldırmaz. Cold-cache ölçülmedi.

## Ayrı requested allocation profili

Aynı 100k corpus + tagged origin allocator. Medyan değildir, tek teşhis koşusu;
allocator header/alignment/realloc ve worker örtüşmesi native'den farklıdır.
İlk allocation kökenini ölçer, mevcut sahibi değil. Sınıfların bağımsız peak'leri
toplanmaz. Bu değerler RSS/private committed değildir.

| Origin | Metadata V2 requested peak byte | WAL delta |
| --- | ---: | ---: |
| other | 201903 | 201891 |
| caller_documents | 10109000 | 569000 |
| delta_index | 2952978 | 2952978 |
| delta_tables | 6443328 | 3657776 |
| source_batch | 432112 | 432040 |
| frame_encode | 589824 | 701664 |
| builder | 6497980 | 6486576 |
| packed_metadata | 1440716 | 1440710 |

Toplam requested peak 27570581 → 15312101 byte (%44,46 azalma).
Caller-document kökeninde tepe 10109000 → 569000 byte: tam belge nesneleri
artık commit sonrasında tutulmuyor. Builder/index tepe maliyeti kaldı.
Kontrollü I/O pool final0, peak131072/limit262144 byte; toplam heap kotası değil.

## Sınırlar ve sonraki karar

Bu deneyi hız sonucu nedeniyle default'a almak doğru değildir. Tam belge
nesnelerini azaltmanın RAM etkisi doğrulandı; diskten her delta hit'ini decode
etmek hız şartına uymadı. Sonraki aday, sıcak delta erişimini koruyan paylaşımlı
belge alanları/kompakt yerleşim ve **merge sırasında mevcut posting'lerin
birleştirilmesi** olmalıdır. Ortak delta/builder/worker bütçesi hâlâ açık.
Cache ekleyerek bu ilk maliyeti ölçümden gizlemek kabul kapısı değildir.

Milyon kayıt, çeşitli/büyük/benzersiz içerik, cold-cache, gerçek uygulama ve
SQLite eşleşmiş delta-query koşusu ölçülmedi. Genel hard RSS garantisi yoktur.
SDB/Fundops entegrasyonu yapılmadı.

## Ham sonuçlar ve frozen binary'ler

`docs/benchmarks/wal-delta-v1-20261009-{baseline,candidate}-{0,1,2}` native
JSONL/memory/stderr; `wal-delta-v1-20261009.summary.json`.
`wal-delta-query-v2-20261009-*` delta query JSONL/stderr + summary;
`wal-delta-origins-v1-20261009.jsonl` ayrı origin profili.
Query binary V1'in metadata bayrakları yanlış olduğundan ölçümde kullanılmadı;
bayrakları düzeltilmiş V2 her iki tarafta aynı query kodudur.

- `output/fdb-stream-metadata-v2-20261009.exe`: `A9B88E57A45D780BEE11EF4E578CA502BEFD80F093872374664A47F253FCBABA`
- `output/fdb-wal-delta-v1-20261009.exe`: `09CE668DCC9E08A98A8B0CC0FF8B2DF0C3E3CC24C9C01DE0B3B0BCC39C3D171B`
- `output/fdb-wal-delta-query-baseline-v2-20261009.exe`: `1FD9B193F2E18FD1FB6EAE13DD830098EDE1CD04CF9131131B3558CEAEB6147A`
- `output/fdb-wal-delta-query-candidate-v2-20261009.exe`: `35AE9058CFFB54EBDF61F48096420B8078E0800B7F2B2CAFF9574D74C1AAE528`
- `output/fdb-wal-delta-origins-v1-20261009.exe`: `9094EF0F6F67B8EEEC2CE2F8D0989DBDD122C327492AA673B5380A591E43936D`

## Doğrulama

Ölçülen feature birleşimi ile `cargo test --all-targets --features
experimental-write-buffer-budget,experimental-shared-delta-text,experimental-delta-bitmaps,experimental-streaming-metadata,experimental-wal-delta`
89 test geçti; default `cargo test --all-targets` 53 test geçti. Yeni üç unit
test located encode byte eşitliği, borrowed/reordered JSON konumları ve CRC
sağlam olsa da key/version/short-read kontrolünü doğrular. İki integration test
unflushed update/delete/search/recovery/flush ve runtime WAL bozulmasını
kontrol eder. Eski atomic guard, her frame kesilmesi, byte bozulması, process
kill, background merge/reopen ve 0.1 fixture testleri korunur.

İlk default clippy feature kapalıyken kullanılmayan enumerate index'ini
reddetti; index yerine konum iterator'ü kullanılarak düzeltildi. Sonra default
ve all-feature clippy `--all-targets -- -D warnings`, `cargo fmt --check` geçti.
Bu düzeltmeden sonra all-feature engine/disk/segments/wal_delta kümeleri
yeniden çalıştırıldı: 46 test geçti. Frozen ölçüm binary'leri korunur; bu
iterator düzeltmesi konum sırası/byte biçimini değiştirmez.

## Tekrar üretim

```powershell
cargo build --release --features experimental-write-buffer-budget,experimental-shared-delta-text,experimental-delta-bitmaps,experimental-streaming-metadata,experimental-wal-delta --example measure_scale
./tools/measure-native.ps1 -Executable target/release/examples/measure_scale.exe -DatabasePath data/wal-delta-repeat-new -Count 100000 -TextBytes 256 -OutputPrefix docs/benchmarks/wal-delta-repeat-new
cargo build --release --features experimental-write-buffer-budget,experimental-shared-delta-text,experimental-delta-bitmaps,experimental-streaming-metadata,experimental-wal-delta --example measure_delta_queries
./target/release/examples/measure_delta_queries.exe data/wal-delta-query-repeat-new
```

İki örnek de olmayan yeni klasör ister. Baseline query için yalnız
`experimental-wal-delta` çıkarılır; binary'ler ayrı isimle dondurulur. Query
harness kendi 10k/256 byte corpus ve watermark'larını belirler. Origin allocator
profili ayrı diagnostic-memory build ve yeni klasörde çalıştırılmalıdır.

Normal CLI son olarak `cargo build --release --bin fdb` ile experimental
feature'sız tekrar üretildi. SHA256:
`E9EFBADB71B694B7C4CECFDD9C977BCA4205E75EEEDBDD6FB47C44C2F383DADF`.
