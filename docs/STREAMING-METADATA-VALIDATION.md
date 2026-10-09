# Akışlı metadata doğrulaması — 2026-10-09

## Sonuç ve kapsam

`experimental-streaming-metadata` feature eklendi. SEG03 kayıt metadata'sı
PackedRecord/ID/group tam tablosuna alınmadan MET04 ile karşılaştırılır veya
64 kayıtlık yapraklarla üretilir. Kalıcı format ve source doğrulamaları korunur.

Açılış+sorgu proses tepe RAM medyanı 10,73 → 6,86 MiB (%36,1 azalma).
Bununla birlikte ingest+bakım+query tüm proses penceresinde RAM yalnız %0,5
azaldı; yazma+bakım %1,7 yavaşladı. Requested toplam allocation peak de
düşmedi. Bu aşama yazma RAM hedefini çözmüş değildir. Feature default kapalıdır.
SQLite bu turda tekrar ölçülmedi; farklı pencerenin RAM'iyle SQLite üstünlüğü
iddiası yapılmaz. Orijinal toplam proses RAM/hız hedefi açık kalır.

## Uygulama ve güvenilirlik

SEG03 header/footer/global checksum, bounds ve bağlama kontrolü önce koşar.
Streaming parser aynı entry limitinde kayıt key/order/offset/len/version
kontrollerini yapar. MET04 header/footer/full CRC/root count, leaf/root identity,
kontigüite ve her numeric metadata + key karşılaştırması doğrulanır. Anahtar
karşılaştırması mevcut compressed byte comparator ile allocation üretmeden
yapılır. Cache ancak bütün kayıt/dictionary parse'ı ve slot eşitliği biterse
kullanılır. .fdb prefix checksum/replay Source.open içinde ayrıca korunur.

Eksik/bozuk cache 64-record leaf, 32-fence directory ve root ile temporary
file'a üretilir. Count header finish'te patch edilir; CRC/footer/sync/rename
sonra SEG03 ve her cache slot'u bağımsız olarak tekrar doğrulanır. İlk geçiş
dictionary parse eder ve bırakır; ikinci geçiş yeniden parse eder. Bu ek
maliyet timer kapsamındadır. Cached count kadar bitmap peşinen ayrılmaz;
deletion bitmap doğrulanmış kayıt akışıyla büyür. Runtime root/deletion ve
token dictionary hâlâ N ile büyüyebilir; toplam hard RAM kotası yoktur.

Builder SEG03 rename'ından önce sidecar ortaya çıkabilir. Bu dosya türetilmiş
cache'tir; manifest/source publication yetkisi taşımaz. DiskIndex.open paged
sonuç verir; enable_paging idempotenttir. Record vector/ID pool/group allocation
kapasitesi yeni yolda sıfırdır. MET04 eski writer ile bayt bayt uyumludur.

## Ingest dahil native ölçüm

100 bin kayıt, 256 UTF-8 bayt aynı tekrarlı Türkçe body, batch500. Baseline
frozen delta-bitmaps V1; candidate stream-metadata V2. Önceki ortak metin,
delta bitmaps ve write-buffer-budget iki tarafta da açık. Diagnostic allocator/
timing kapalı. Üç dönüşümlü çift: B/C, C/B, B/C; her işlem yeni DB dizininde.

| Ölçüm | Baseline | V2 | Değişim |
| --- | ---: | ---: | ---: |
| Ingest | 5.6900 s | 5.7855 s | +1.68% |
| Son bakım | 5.0445 s | 5.1339 s | +1.77% |
| Yazma + bakım | 10.7345 s | 10.9194 s | +1.72% |
| Tüm proses tepe working set | 37.3438 MiB | 37.1445 MiB | -0.53% |
| Örneklenen private tepe | 35.2930 MiB | 35.2109 MiB | -0.23% |
| Reopen | 548.0804 ms | 517.0970 ms | -5.65% |

| Sorgu | Baseline p50 / p95 ms | V2 p50 / p95 ms |
| --- | ---: | ---: |
| Dar metin | 2.9324 / 3.9253 | 2.8473 / 3.7236 |
| Exact ilk sayfa | 0.1967 / 0.2674 | 0.2065 / 0.3442 |
| Range | 0.4110 / 0.6617 | 0.4252 / 0.6612 |
| İkinci sayfa | 0.1950 / 0.3605 | 0.1899 / 0.2961 |
| Son sayfa | 1.4844 / 1.5800 | 1.3620 / 1.7222 |
| Geniş metin | 212.5452 / 218.6158 | 224.6685 / 228.7268 |
| 100 key | 1.6781 / 1.9881 | 1.5437 / 1.6849 |

Exact p95 yaklaşık %28,7, last-page p95 %9,0 ve broad-text p50 %5,7
yüksektir; hız kapıları geçmiş değildir. Bu farklar saklanır. Final sources
46428672, cache81627542 byte, posting4589406, görünür99950 belge/iki run
tüm koşularda aynı. Klasör431069691 byte inaktif nesilleri de içerir.
Her koşu independent full StoredDocument oracle, total/order/version,
50 update/50 delete ve reopen kontrolünden geçti. İlk harness küçük sorgu
30; geniş metin/last/get100 üç timed repeat + untimed warmup kullanır.

V1 decoded cache key'lerini her slotta oluşturuyordu. Ayrı V1 üç çift ham
sonucu korunur: RAM %2,0 düştü fakat yazma+bakım %3,5 arttı. V2 numeric/key
byte comparison bu geçici String kopyalarını kaldırır. V1 sonuçları V2 ile
birleştirilmez: `stream-metadata-v1-20261009.*` / `stream-metadata-v2-20261009.*`.

## Aynı hazır DB açılışı ve daha çok sorgu tekrarı

Sağlıklı `data/stream-metadata-v2-20261009-candidate-0` kullanılır. API commit/
flush yok; engine gerekirse derived cache kurabilir, bu deneyde cache sağlıklı.
`measure-ready-native.ps1` tüm open+query prosesini izler. Query timer open'ı
dışarıda bırakır; **bellek penceresi open'ı içerir**. Baseline frozen delta-ready-
bitmap, candidate stream-metadata-ready V2. Üç dönüşümlü çift, warm OS/file
cache. 10 warmup; dar/exact/range/ikinci200, last/get100100, text20 timed tekrar.
DB değişiklik sonrası: narrow50001, last offset99900, get keys n*997+1.
İlk ingest harness ile mutlak sorgu süreleri karşılaştırılmaz.

| Sorgu | Baseline p50 / p95 ms | V2 p50 / p95 ms |
| --- | ---: | ---: |
| Dar metin | 2.6968 / 3.2294 | 2.6986 / 3.1186 |
| Exact ilk sayfa | 0.2248 / 0.2537 | 0.2138 / 0.2849 |
| Range | 0.3955 / 0.5190 | 0.3872 / 0.5081 |
| İkinci sayfa | 0.2283 / 0.2555 | 0.1957 / 0.2620 |
| Son sayfa | 4.3792 / 5.3298 | 4.4307 / 5.1476 |
| Geniş metin | 209.1459 / 246.0447 | 206.6858 / 226.8129 |
| 100 key | 1.6686 / 2.4504 | 1.5681 / 2.1787 |

| Açılış+sorgu belleği | Baseline | V2 |
| --- | ---: | ---: |
| Proses tepe working set | 10.7266 MiB | 6.8555 MiB |
| Örneklenen private tepe | 7.7266 MiB | 3.1953 MiB |

Exact p95 bu kontrolde de yaklaşık %12,3 yüksektir. Diğer çoğu süre küçük
farklar veya iyileşme gösterir; gürültü/kalıcı regresyon ayrımı kesin değildir.
Bu kontrol ilk harness sonuçlarını geçersiz kılmaz. İki bellek penceresi
ayrıdır; 6,86 MiB bütün ingest tepesinin yerine sunulmaz.

## Ayrı requested allocation profili

| Allocation kökeni | Baseline requested peak MiB | V2 |
| --- | ---: | ---: |
| other | 0.1925 | 0.1925 |
| caller_documents | 9.6407 | 9.6407 |
| delta_index | 2.8162 | 2.8162 |
| delta_tables | 6.1448 | 6.1448 |
| source_batch | 0.4300 | 0.4121 |
| frame_encode | 0.5625 | 0.5625 |
| builder | 6.1690 | 6.1970 |
| packed_metadata | 10.7496 | 1.3740 |

Metadata kökeni bağımsız peak 11,271,734 → 1,440,716 byte. Bu etiket yeni yolda root/page/token allocation'larını da kapsar.
Toplam requested peak 27,527,626 → 27,570,581 byte: azalmadı.
Bağımsız sınıf peak'leri toplanmaz. Header allocator/realloc davranışı ve
worker örtüşmesi native allocator'dan farklıdır; bu byte sayaçları RSS/private
committed değildir. Baseline profiler önceki turdan, aynı corpus/allocator.
Caller kökeni StoredDocument alanları ve delta tabloları toplam proses tepesine
baskın kalır; ownership ilk allocation kökenidir, mevcut sahip değildir.

Controlled I/O pool final in-use0, peak81920 byte; yeni metadata reader/leaf/
fence/root/deletion/dictionary allocation'ları bu rezervasyon dışındadır.

## Ham sonuçlar ve binary'ler

Native `stream-metadata-v2-20261009-{baseline,candidate}-{0,1,2}.jsonl/.memory.json`,
ready aynı etiketli `stream-metadata-ready-v2-20261009-*`, özetler `.summary.json`;
origin `stream-metadata-origins-v2-20261009.jsonl`. V1 native ölçümler korunur.
Cold-cache, milyon kayıt, çeşitli/büyük içerik ve gerçek uygulama workload'u
ölçülmedi. Native API, IPC yok. Requested20ms sampling Windows tarafından
uzatılabilir; PeakWorkingSet proses tepesidir, OS file cache dahil sistem RAM
veya idle RAM değildir. Private tepe örneklenir.

- `output/fdb-delta-bitmaps-v1-20261009.exe`: `16868A7BB18AED7C611DEC01098694DAD44F4EBD8E28E81745EAD1D804CF1844`
- `output/fdb-stream-metadata-v1-20261009.exe`: `0724B7FB8F94800AC61DE4A52A3A8A1F8B99DEA2A1E9CBDEBAA13A70848E2C04`
- `output/fdb-stream-metadata-v2-20261009.exe`: `A9B88E57A45D780BEE11EF4E578CA502BEFD80F093872374664A47F253FCBABA`
- `output/fdb-stream-metadata-origins-v2-20261009.exe`: `15A29BCD1E31BEC1D60754AE30F887CE427A84298F6A1CF6964AD81547087585`
- `output/fdb-delta-ready-bitmap-20261009.exe`: `92A61459B7572E9137BAD245CF6AC9389E43A44FCF5A38B81DA67620BE9ECD21`
- `output/fdb-stream-metadata-ready-v2-20261009.exe`: `A2307F24815C15D7B249A0F72D26BE2FD0FA15B2DCDBE306FAD483EEFE48FFC0`

## Doğrulama

Metadata V2 ölçümünden sonra `cargo test --all-targets --all-features`
83 test, default `cargo test --all-targets` 52 test geçti. `cargo fmt --check`,
default ve all-feature `cargo clippy --all-targets -- -D warnings` başarılıydı.
Cache kesilmeleri, checksum/identity bozulmaları, kaynak bozulması, süreç
sonlandırma ve değiştirilmeyen 0.1 fixture testleri bu kümelerdedir. Yeni
metadata üretim noktasına özel process-kill failpoint eklenmedi; küçük MET04
dosyası için bütün kesilme konumları ve mevcut engine kill testleri çalıştı.

Normal CLI bu aşamada default feature ile tekrar derlendi: SHA256
`5D300F27FAE48431ECC86455874DAA2626370CD71DE7D9AE6EDD1B729CB22763`.
Daha sonraki delta-WAL aşaması ayrı belgede raporlanır; bu ölçümler
frozen metadata V2 binary'lerine aittir.

## Tekrar üretim

```powershell
cargo build --release --features experimental-write-buffer-budget,experimental-shared-delta-text,experimental-delta-bitmaps,experimental-streaming-metadata --example measure_scale
./tools/measure-native.ps1 -Executable target/release/examples/measure_scale.exe -DatabasePath data/metadata-repeat-new -Count 100000 -TextBytes 256 -OutputPrefix docs/benchmarks/metadata-repeat-new
cargo build --release --features experimental-write-buffer-budget,experimental-shared-delta-text,experimental-delta-bitmaps,experimental-streaming-metadata --example measure_ready_queries
./tools/measure-ready-native.ps1 -Executable target/release/examples/measure_ready_queries.exe -DatabasePath data/metadata-repeat-new -OutputPrefix docs/benchmarks/metadata-ready-repeat-new
```

İlk komut için olmayan yeni DB klasörü; ikinci için tamamlanmış, sağlıklı
100k benchmark DB gerekir. Dönüşümlü üç çift için frozen binary'leri ve
her ingest koşusunda farklı yeni klasör kullanın.
