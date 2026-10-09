# Merge posting yeniden kullanımı — 2026-10-09

## Sonuç

`experimental-merge-postings` uygulandı. Merge mevcut SEG03 posting listelerini
birleştiriyor; title/body/field/link terms yeniden üretilmiyor. Temiz V2 üç eşleşmiş çiftte
100k yazma+bakım medyanı 10,98 → 9,32 s (%15,13 azalma), son bakım 5,09 →
3,58 s (%29,76 azalma), tepe working set %7,38 azaldı. **Hedef tamamlanmadı:**
SQLite bu turda yeniden ölçülmedi, toplam RAM hâlâ yüksek, bazı query quantile'ları
yükseldi. Varsayılan feature'lar değişmedi. Hızdan ödün vermeden düşük RAM kapısı
henüz geçmiş sayılmaz. WAL delta deneyi bu karşılaştırmada kapalıdır; sıcak
belge erişimi mevcut resident StoredDocument yolunu kullanır.

## Algoritma ve veri bütünlüğü

Worker önce seçilen input kaynakların published prefix CRC'sini ve immutable
Run length'ini doğrular; legacy source checkpoint prefix'inden uzun olabilir,
sonraki WAL append'i kabul edilir. SEG03 footer/global CRC positional I/O ile
8 KiB scratch kullanılarak yeniden doğrulanır. Böylece runtime bozulmuş cache'in
posting'leri veya sağlam cache arkasında bozulmuş source yayınlanmaz.

MergeKeys en güncel run'ın kayıtlarını seçer. Her input fiziksel kayıt için
u32 old→new ID map tutulur; eski sürüm/tombstone u32::MAX ile çıkarılır. Output
ID'leri tombstone slotlarını da sayar. Live mapping input sırasına göre artar.
Mevcut checkpoint encoder yeni `.fdb` kaynağını tamamlar/sync eder. Yeni source
replay'i payload ve record metadata'yı Builder'a ekler; terms üretmez.

Input dictionary iterator'ları term heap'i ile birleştirilir. Eş term için her
input posting'i 8 KiB penceresiyle okunur; source ID strict order/bounds ve
mapped order doğrulanır. Output ID heap'i sıralı/deduplicate kontrolüyle SEG03'e
yazar; count0 term atılır. Bir term'in bütün posting'leri Vec'e alınmaz.
Directory/footer, record spool ve MET04 aynı codec'leri kullanır. Normal
open_source sonunda `.fdb` replay/bounds/checksum/schema/identity doğrulamasını
yeniden çalıştırır. Bu ek JSON geçişi ölçüm süresine dahildir. Worker manifest
publish etmez; eski publication ve kaynaklar başarılı install'e kadar korunur.

Kalıcı FDBLOG01/03, TXN1, MAN03, SEG03 ve MET04 biçimleri değişmedi. 0.1
fixture kaynak dosyaları değiştirilmedi. Yeni bir DB/arama bağımlılığı yok.
Map input fiziksel kayıt başına4 byte; dictionary/root/deletion bitmap/scratch
ortak I/O kotası dışında. Bu aşama ortak delta/builder/worker heap bütçesi değil.
Flush hâlâ terms üretir; yalnız merge'deki tekrar kaldırılır.

## Eşleşmiş native ölçüm

Baseline frozen streaming-metadata V2; candidate buna merge-postings ekler.
Feature birleşimi write-buffer-budget/shared-delta-text/delta-bitmaps/
streaming-metadata/merge-postings. diagnostic-timing/memory ve WAL delta kapalı.
B/C, C/B, B/C üç çift, ayrı yeni DB. 100000 kayıt, 256 UTF-8 body byte,
batch500; shared/contents,10 haneli key, rapor-{id:010}, day=id%30,
source=synthetic, tekrar eden Türkçe metin. Default delta keys10000/bytes16MiB.
Warm OS/file cache + untimed query warmup. Build/test/profile benchmark'larla
aynı anda çalıştırılmadı. Native API; IPC/servis/uygulama entegrasyonu yok.

Process PeakWorkingSet64 bütün open/ingest/maintenance/reopen/query/50update/
50delete penceresidir. OS file cache dahil toplam sistem RAM değildir.
Private peak hedef20ms örneklemeden gelir; Windows süreyi uzatabilir.

| Medyan | Baseline | Candidate V2 |
| --- | ---: | ---: |
| Yazma | 5.8530 s | 5.7947 s |
| Son bakım | 5.0940 s | 3.5777 s |
| Yazma + bakım | 10.9821 s | 9.3200 s |
| Açılış | 543.1068 ms | 542.8842 ms |
| Tepe working set | 37.1055 MiB | 34.3672 MiB |
| Örneklenen private tepe | 34.9336 MiB | 31.8516 MiB |

Her koşu independent full StoredDocument oracle, exact total/ordered page/
versions, 50 update/delete ve reopen kontrolünden geçti. Final99950 görünür belge,
2run,4589406posting,source46428672/cache81627542 byte her koşuda aynı. Sum,
her koşu toplamının medyanı; bileşen medyanlarının toplamı değildir.

| Sorgu | Baseline p50/p95 ms | Candidate V2 p50/p95 ms |
| --- | ---: | ---: |
| narrow | 2.8421 / 3.7782 | 3.2166 / 4.2081 |
| broad | 0.1964 / 0.2600 | 0.2027 / 0.2521 |
| range | 0.4061 / 0.6419 | 0.4269 / 0.6173 |
| second_page | 0.1899 / 0.2154 | 0.2010 / 0.3414 |
| last_page | 2.0277 / 2.1158 | 1.5185 / 2.0314 |
| broad_text | 220.8578 / 227.4527 | 228.7332 / 255.6171 |
| get_100 | 1.4113 / 1.4452 | 1.4223 / 1.5172 |

Bu ilk harness küçük sorgu30, broad-text/last/get100 üç timed repeat +
warmup kullanır. Quantile farkları korunur; daha çok tekrarlı kontrol aşağıdadır.

## Byte eşitliği ve hata testleri

Yeni unit test eski bağımsız Builder terms üretimine karşı merged SEG03
byte'larını karşılaştırır: Unicode, exact/numeric, nested alanların dışlanması,
aynı link'in yinelenmesi, yeni sürüm, tombstone ve gizlenen eski ID'ler.
Bad/short map, duplicate output ID ve bozulmuş input global CRC reddedilir.

100k V2 her üç baseline/candidate çiftinin final manifest JSON'u eşittir;
yayınlanan iki run'ın `.fdb` ve `.idx` SHA256'ları birebir eşittir (12 karşılaştırma).
Bağımsız ham kanıt: `merge-postings-v2-20261009.final-byte-equality.json`. Inaktif dosyalar dahil
klasör byte sayısı baseline431069691/candidate431069352; bu fark final
publication/source/index eşitliğini değiştirmez.

Yeni integration test open sonrası input `.fdb` veya `.idx` byte bozulmasını
merge başlamadan oluşturur. Worker hata verir; `.ready` ve bozulmuş kaynak
aynı kalır. Sonraki open sağlam `.fdb` ile bozuk cache'i yeniden kurar; bozuk
`.fdb` sağlıklı cache olsa da reddedilir. Eski 0.1 fixture testi gerçek merge'in
sonucunu `wait_for_compaction` ile bekleyecek şekilde güçlendirildi; kaynak
published prefix'i byte byte korunur. Fixture dosyası değişmedi.

## Aynı DB üzerinde daha çok tekrarlı V2 query kontrolü

Her iki engine sağlıklı `data/merge-postings-v2-20261009-candidate-0` açar.
API write/flush yok; cache sağlıklı, gerektiğinde derived rebuild yeteneği vardır.
Warm OS/file cache,10 warmup; narrow/exact/range/second200,last/get100100,
broad-text20 timed repeat. Query timer open'ı dışlar; memory window open'ı içerir.
10k delta sorgusu değildir: changed100k DB99950doc, narrow50001,last offset99900,
get n*997+1. İlk ingest harness ile mutlak last süreleri karşılaştırılmaz.

| Ready sorgu | Baseline p50/p95 ms | Candidate V2 p50/p95 ms |
| --- | ---: | ---: |
| narrow | 2.6952 / 3.1311 | 2.6926 / 3.1376 |
| broad | 0.2178 / 0.2714 | 0.2245 / 0.2643 |
| range | 0.4000 / 0.5773 | 0.4082 / 0.5356 |
| second_page | 0.2030 / 0.2477 | 0.2088 / 0.2750 |
| last_page | 4.6505 / 5.7777 | 4.4703 / 5.3762 |
| broad_text | 204.9835 / 225.8690 | 207.8355 / 238.1689 |
| get_100 | 1.5645 / 2.2762 | 1.5558 / 2.1666 |

Second-page p95 %11,0, broad-text p95 %5,4 arttı; bunlar regresyon kapısında
korunur. Last/get100 p50/p95 iyileşti, diğer çoğu p50 farkı%0–3. Hazır query
working set baseline6,7734/candidate6,7656 MiB; bütün ingest RAM'i değildir.
Bu sonuç ilk harness'ı geçersiz kılmaz, cold-cache/çeşitli corpus kanıtı değildir.

## V1 tarihçesi ve build isolation

İlk V1 üç çiftte sum10,95→9,10s/WS%8,49 düşüş görülmüştü; ham veriler korunur.
Ancak önceki WAL deneyi için `serde_json/raw_value` global dependency flag'i
etkindi; frozen eski baseline bunu açmıyordu. Bu yüzden V1 saf merge ablation
kanıtı olarak kullanılmaz ve V2 ile birleştirilmez. `raw_value` artık yalnız
`experimental-wal-delta` dependency feature'ıdır; normal/merge derleme bunu
açmaz. V2 her iki tarafta raw_value kapalı aynı serde_json sürümünü kullanır.
WAL deneyi/ilgili unit testler all-feature derlemede bu özelliği kullanır.

## Küçük delta: kabul edilmeyen V1 pilot

Aynı algoritma/corpus/batch500, delta watermark500 ile tek keşif koşusu:
proses WS15376384 byte (14,6641 MiB), sampled private11005952 byte;
ingest16545,9192 + maintenance4715,0536 =21260,9728 ms. Tam oracle/update/delete/
reopen geçti. Klasör687293847 byte; generation537. Yeni kaynak sayısı, sync,
dictionary ve merge geçişleri arttı. Bu pilot üç çift medyanı değildir;
RAM düşüşü karşılığında hız kaybı kabul edilmez, default ayar değiştirilmedi.

`measure_scale` isteğe bağlı dördüncü delta-key argümanını raporlar ve tüm
reopen'larda aynı Options'u kullanır. `measure-native.ps1 -DeltaKeys` bu argümanı
isteğe bağlı geçirir; belirtilmeyince eski üç argüman sözleşmesi korunur.
Pilot binary V2 yalnız harness watermark seçimini ekler; motor V1 ile aynıdır.

Sonraki bellek adımı, sıcak delta belge alanlarını paylaşarak büyük deltayı
daha ucuza tutmak ve builder/worker ortak bütçesini tamamlamaktır. Körlemesine
küçük delta veya WAL hydration'a dönmek iki ayrı hız kapısını bozdu.

## Sınırlar, ham dosyalar ve tekrar üretim

Cold-cache,1m kayıt, çeşitli/benzersiz/büyük metin, gerçek proje corpus'u ve
SQLite yeni eşleşmiş koşusu ölçülmedi. Tüm RAM/hız hedefi açık. `.fdb` JSON
kaynak kopyalama/doğrulama geçişleri sürer; bu aşama tüm serialization veya
indeks üretimini kaldırmaz. Varsayılan motor/feature'lar değiştirilmedi.

`docs/benchmarks/merge-postings-v2-20261009-{baseline,candidate}-{0,1,2}`
JSONL/memory/stderr + `.summary.json`; ready `merge-postings-ready-v2-20261009-*`
+ summary. V1 ham dosyalar ve pilot `merge-delta500-v2-20261009-fdb-0` korunur.

- `output/fdb-stream-metadata-v2-20261009.exe`: `A9B88E57A45D780BEE11EF4E578CA502BEFD80F093872374664A47F253FCBABA`
- `output/fdb-merge-postings-v1-20261009.exe`: `CDD71189161B08766D348B5B9A4D41559AD785C6EEAE7413724C2012110B43E9`
- `output/fdb-merge-postings-watermarks-v2-20261009.exe`: `42ACD3E347FEB44E02F3C33399D09AE6742C8B1071291E48CC3E4CBCB9E3FC5A`

```powershell
cargo build --release --features experimental-write-buffer-budget,experimental-shared-delta-text,experimental-delta-bitmaps,experimental-streaming-metadata,experimental-merge-postings --example measure_scale
./tools/measure-native.ps1 -Executable target/release/examples/measure_scale.exe -DatabasePath data/merge-repeat-new -Count 100000 -TextBytes 256 -OutputPrefix docs/benchmarks/merge-repeat-new
# Yalnız keşif: ölçülen hız kaybı nedeniyle önerilen/default ayar değildir.
./tools/measure-native.ps1 -Executable target/release/examples/measure_scale.exe -DatabasePath data/merge-small-repeat-new -Count 100000 -TextBytes 256 -DeltaKeys 500 -OutputPrefix docs/benchmarks/merge-small-repeat-new
```

Her koşu olmayan yeni klasör ister. Karşılaştırma için binary'leri ayrı isimle
dondurun, üç dönüşümlü çift kullanın. Kaynak fixture/data klasörlerini silmeyin.

- `output/fdb-merge-postings-v2-20261009.exe`: `50D135907024282ED733708C549D715F950D2F7D68B7343B7C08DC33A2E1EA87`

- `output/fdb-merge-postings-ready-v2-20261009.exe`: `BE45EF5E3723C2E779A604DBEF494030096DAE34A403DB581E97633EB847214B`

- `output/fdb-stream-metadata-ready-v2-20261009.exe`: `A2307F24815C15D7B249A0F72D26BE2FD0FA15B2DCDBE306FAD483EEFE48FFC0`

## Son doğrulama

`cargo test --all-targets --all-features`: 91 test geçti. Default
`cargo test --all-targets`: 53 test geçti. `cargo fmt --check`, default ve
all-feature `cargo clippy --all-targets -- -D warnings` başarılı. Süreç
sonlandırma, tüm frame kesilmeleri, kaynak/cache/manifest bozulmaları,
random model, background write/merge ve 0.1 uyumluluk bu kümelerde korundu.
RawValue isolation sonrası default 53 test ve merge-reuse unit, gerçek 0.1
merge, runtime source/index corruption testleri raw_value kapalı tekrar geçti.
All-feature yeniden reuse unit de geçti. Normal CLI son olarak default
`cargo build --release --bin fdb` ile üretildi; SHA256:
`4A542314B301DB14C78DE7EFD22645884FE3A8BEDFB40F92AB0D1EB0BF997123`.
