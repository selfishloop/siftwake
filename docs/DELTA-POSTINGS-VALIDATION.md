# Delta posting doğrulaması — 2026-10-09

## Uygulama

`experimental-compact-delta-postings` Arc<Key> posting yerine u32 RAM ID
kullanır. Key→ID B-tree, slot Vec ve free-ID listesi anahtar/folded metni taşır.
Eski posting tamamen çıkarıldıktan sonra ID geri kullanılır. Query çıktısı
anahtar sırasıyla tekrar sıralanır; slot sırası public sıralama değildir.

`experimental-delta-bitmaps` ayrıca <=4 ID inline array, düşük yoğunlukta
sorted Vec, yüksek yoğunlukta bitmap kullanır. En az 64 ID ve bitmap byte
sayısı <= ID sayısı*2 olduğunda dense olur. Uzak ID aşırı bitmap allocation
yerine sparse temsile geçirir; dense <32, sparse <=4 olduğunda küçülür.
Sparse insertion/removal lineer kaydırma yapabilir. Genel hard RAM kotası yoktur.

ID kapasitesi WAL append öncesi konservatif batch üst sınırıyla kontrol
edilir. Index kurulum hataları Result; WAL sonrası beklenmeyen index hatası
poisoned motor ile yeniden açılmayı gerektirir. Kalıcı format/source/cache
CRC, manifest publication, v0.1 fixture değişmez. Flush eşikleri aynı kalır.

İki feature bağımsız ölçülebilir ve default kapalıdır. Ölçümde ortak delta
metni ve write-buffer-budget iki derlemede de açıktır.

## Dönüşümlü native karşılaştırma

Üç sürüm üç koşuda her pozisyonu bir kez aldı: baseline/compact/bitmap,
compact/bitmap/baseline, bitmap/baseline/compact. Baseline önceki ortak delta
metni frozen binary; compact u32 B-tree posting; bitmap adaptive posting.

| Ölçüm | Baseline | u32 B-tree | Adaptive bitmap |
| --- | ---: | ---: | ---: |
| Ingest | 8.2672 s | 6.1163 s | 5.7718 s |
| Son bakım | 4.8427 s | 4.9525 s | 5.0242 s |
| Yazma + bakım | 13.1099 s | 11.1578 s | 10.7960 s |
| Proses tepe working set | 46.4102 MiB | 42.7461 MiB | 38.0859 MiB |
| Örneklenen private tepe | 43.8789 MiB | 40.9766 MiB | 36.0312 MiB |
| Reopen | 558.8456 ms | 563.2787 ms | 582.1447 ms |

Baseline→bitmap: ingest %30,2, yazma+bakım %17,7, proses tepe working set
%17,9 azaldı. Son bakım %3,7 arttı. u32 ID tek başına yazma+bakımı %14,9
azaltır; bitmap ek olarak RAM tüketimini düşürür.

| Sorgu | Baseline p50 / p95 ms | u32 B-tree | Adaptive bitmap |
| --- | ---: | ---: | ---: |
| Dar metin | 2.9107 / 3.8862 | 2.9635 / 3.8528 | 3.3239 / 4.2807 |
| Exact ilk sayfa | 0.2194 / 0.3059 | 0.1959 / 0.3061 | 0.1954 / 0.3066 |
| Range | 0.4485 / 0.6795 | 0.4665 / 0.6725 | 0.4930 / 0.6759 |
| İkinci sayfa | 0.1970 / 0.3000 | 0.1998 / 0.3043 | 0.2720 / 0.3837 |
| Son sayfa | 1.6352 / 1.9704 | 1.4765 / 1.6772 | 1.4164 / 1.7779 |
| Geniş metin | 232.2989 / 241.3436 | 223.4816 / 228.3957 | 205.2857 / 219.6810 |
| 100 key | 1.4980 / 1.9847 | 1.9355 / 1.9451 | 1.9613 / 2.2326 |

İlk post-ingest harness narrow, range, ikinci sayfa ve get100 regresyonları
gösterir; hız kapıları geçmiş değildir. Bu farklar saklanır. Aynı hazır DB
üzerinde daha fazla tekrar kontrolü aşağıda ayrı verilir; yazma geçmişi ve
farklı DB koşullarının etkisini tek başına kesin ayrıştıramaz.

## Ölçüm sözleşmesi

100 bin belge, aynı tekrarlı Türkçe body 256 UTF-8 byte, batch 500. Yeni DB
dizinleri. Exact full StoredDocument oracle, total/sıra/version, 50 update/50
delete, reopen kontrolleri tüm dokuz koşuda geçti. Son durum 99950 belge,
iki run, 4589406 posting, source 46428672 ve cache 81627542 byte aynıdır.
Dizin toplamı baseline/compact 431069352; bitmap 431069691 byte. Eski
inaktif manifest/run nesilleri dahil toplam disk, canlı veri boyutu değildir.
Worker completion zamanlaması farklı arşiv manifestleri üretebilir; final
source/cache fiziksel ölçümleri aynı kalır.

Warm OS/file cache, query untimed warmup. Dar/exact/range/ikinci sayfa 30;
geniş metin/son sayfa/get100 üç timed tekrar. p50/p95 koşu istatistiklerinin
medyanıdır. Native API; IPC, cold-cache, milyon kayıt, çeşitli gerçek/büyük
içerik ve concurrent-client benchmark bu turda ölçülmedi. PeakWorkingSet
tüm benchmark prosesinin tepesidir, idle RAM veya sistem file-cache değildir.
Private committed 20 ms istenen aralıkta örneklenir; Windows gerçek aralığı
uzatabilir. Teşhis allocator/timing normal native ölçümde kapalıdır.

Ham: `docs/benchmarks/delta-postings-v1-20261009-{baseline,compact,bitmap}-{0,1,2}`
JSONL/memory ve `delta-postings-v1-20261009.summary.json`.

## Binary kimliği

- `output/fdb-shared-delta-v1-20261009.exe`: `E813D4D4490F8AAB52F9374E3B723C0058C8BCB6B48B6CC495048DF8E13FA810`
- `output/fdb-compact-delta-v1-20261009.exe`: `48583BE98921FC5CABB44FFCFE05598CC20423D869D19A3C9029CCA4EEBC45D0`
- `output/fdb-delta-bitmaps-v1-20261009.exe`: `16868A7BB18AED7C611DEC01098694DAD44F4EBD8E28E81745EAD1D804CF1844`

## Aynı hazır DB üzerinde sorgu kontrolü

İki derleme `data/delta-postings-v1-20261009-bitmap-0` sağlıklı DB üzerinde
üç dönüşümlü çiftle çalıştı. API commit/flush yapılmaz; open ölçüm dışında.
DB update/delete sonrası 99950 belgedir. 10 warmup, dar/exact/range/ikinci
sayfa 200, son sayfa/get100 100, geniş metin 20 timed tekrar. Aynı independent
full StoredDocument oracle kullanılır. Narrow key 50001, last offset99900,
get keys n*997+1; ilk harness sorgularıyla mutlak süre kıyaslanmaz.

| Sorgu | Baseline p50 / p95 ms | Bitmap p50 / p95 ms | p50 farkı |
| --- | ---: | ---: | ---: |
| Dar metin | 2.7047 / 3.0955 | 2.6866 / 3.1189 | -0.67% |
| Exact ilk sayfa | 0.2180 / 0.2408 | 0.2195 / 0.2667 | +0.69% |
| Range | 0.3892 / 0.5067 | 0.3963 / 0.5450 | +1.82% |
| İkinci sayfa | 0.2263 / 0.2692 | 0.2250 / 0.2658 | -0.57% |
| Son sayfa | 4.4140 / 5.0640 | 4.5986 / 5.3588 | +4.18% |
| Geniş metin | 196.7489 / 232.8392 | 199.2749 / 223.2026 | +1.28% |
| 100 key | 1.5042 / 2.0161 | 1.5013 / 1.9980 | -0.19% |

Aynı DB kontrolünde p50 farkları -0,7%..+4,2% aralığına indi. Fakat exact
p95 yaklaşık %10,8, range %7,6, last-page %5,8 yüksektir. Bu örneklem
gürültü/regresyon ayrımını kesinleştirmez. İlk harness farkları silinmez ve
tüm hız kapıları geçti iddiası yapılmaz. Feature default kapalı kalır.

Ham: `delta-postings-ready-20261009-{baseline,bitmap}-{0,1,2}.jsonl` ve
`delta-postings-ready-20261009.summary.json`.

- `output/fdb-delta-ready-baseline-20261009.exe`: `FC3D094A739651E13FA5E2CB2B866FE056C24682E123A955D76AC1391ACDA124`
- `output/fdb-delta-ready-bitmap-20261009.exe`: `92A61459B7572E9137BAD245CF6AC9389E43A44FCF5A38B81DA67620BE9ECD21`

## Ayrı allocation kökeni profili

Aynı 100 bin/batch500 teşhis corpus ve header allocator; süreler native
ölçümlere katılmaz. Köken ilk allocation sırasında atanır; sahip devri
izlenmez. Requested byte sayacı RSS/private committed değildir. Sınıf
peak değerleri bağımsız oluşur ve toplanmaz. Önceki baseline profiler bu
turda tekrar çalıştırılmadı; worker örtüşmesi farklı olabilir.

| Köken | Baseline requested peak MiB | Bitmap |
| --- | ---: | ---: |
| other | 0.1925 | 0.1925 |
| caller_documents | 9.6407 | 9.6407 |
| delta_index | 10.9637 | 2.8162 |
| delta_tables | 6.1448 | 6.1448 |
| source_batch | 0.4277 | 0.4300 |
| frame_encode | 0.5625 | 0.5625 |
| builder | 4.7996 | 6.1690 |
| packed_metadata | 10.7496 | 10.7496 |

Delta index requested peak 11,496,306 → 2,952,978 byte; yaklaşık %74,3 düşüş.
Toplam requested peak 36,509,808 → 27,527,626 byte. Body paylaşımı önceki baseline içinde zaten vardır.

Raw `shared-delta-origins-v1-20261009.jsonl` /
`delta-bitmaps-origins-v1-20261009.jsonl`. Ortak controlled I/O pool observed
peak 131072 byte, final in-use0, waits0, reduced0. Posting/slot/belge/packed
metadata bu kota dışındadır; genel shared heap bütçesi hâlâ tamamlanmadı.

Artık büyük tüketiciler caller kökeninde kalan StoredDocument nesneleri,
delta belge/anahtar tabloları ve packed metadata açılış/builder tablolarıdır.
Sonraki adım transient packed tablo üretimini sayfa akışına dönüştürmektir.
SQLite altı toplam proses RAM ve daha hızlı tamamlanmış iş yükü hedefi
henüz doğrulanmadı; yalnız bu ara optimizasyon başarıyla ölçüldü.

- `output/fdb-delta-bitmaps-origins-v1-20261009.exe`: `DC76B95A3877E8804E5E4E8B880A42BF41E1B517474DA567FCF653CFFDC75DF9`

## Doğrulama

- `cargo test --all-targets --features experimental-write-buffer-budget,experimental-shared-delta-text,experimental-compact-delta-postings`: 78 test geçti (bitmap eklenmeden önce u32 yolu).
- `cargo test --features experimental-delta-bitmaps,experimental-shared-delta-text --lib`: 12 test geçti.
- `cargo test --all-targets --features experimental-write-buffer-budget,experimental-shared-delta-text,experimental-delta-bitmaps`: 80 test geçti; bu ölçülen feature kombinasyonudur, eski bakım slot kilitlerini içermez.
- `cargo test --all-targets --all-features`: 80 test geçti.
- `cargo test --all-targets`: 52 test geçti.
- `cargo fmt --check`: geçti.
- `cargo clippy --all-targets -- -D warnings`: geçti.
- Clippy -D warnings u32 seçili kombinasyon, bitmap seçili kombinasyon ve all-features için geçti.
- Feature release native/ready-query/origin example binary'leri donduruldu; normal release CLI yeniden derlendi.

Yeni testler reverse-order ingest, 1000 update boyunca slot sayısının sabit
kalması, tüm posting ID'lerinin live slot'a karşılığı, stale trigramların
kalkması, full delete, kapasite/duplicate reddinde değişmemiş index, inline/
sparse/dense geçişleri, distant u32::MAX ID, 20000 random insert/delete,
duplicate union ve sorted iteration'ı BTreeSet referansıyla doğrular.
Mevcut random query oracle, scope/collection/field/range/link, update/delete,
visibility/page ordering, source/cache corruption, kesilme/process-kill ve
v0.1 uyumluluk testleri de geçer. Yeni kalıcı format yoktur.

İlk kontroller borrowed-key lifetimes ve Copy u32 clone kullanımını buldu;
varsayılan ve ID/bitmap iterator dalları açıkça ayrıldı. Son default ve
seçilmiş/all-feature kontrolleri geçti. Benchmark binary'leri bu onarımlar
sonrasında üretildi; native sonuçlara teşhis allocator karıştırılmadı.

Normal CLI SHA-256:
`BF275AC7717A2247A734E3C3907CB79D89FCECD41DB50D45D2C030690EFECD3C`.

## Tekrar üretim

```powershell
cargo build --release --features experimental-write-buffer-budget,experimental-shared-delta-text,experimental-delta-bitmaps --example measure_scale
./tools/measure-native.ps1 -Executable target/release/examples/measure_scale.exe -DatabasePath data/delta-postings-repeat -Count 100000 -TextBytes 256 -OutputPrefix docs/benchmarks/delta-postings-repeat
cargo run --release --features experimental-write-buffer-budget,experimental-shared-delta-text,experimental-delta-bitmaps,diagnostic-memory --example profile_write_origins -- data/delta-postings-origins-repeat
```

Bu turda SQLite binary'si tekrar ölçülmedi. SQLite altı toplam RAM hedefi,
ortak delta/builder/worker bütçesi, büyük/çeşitli corpus ve tüm hız kapıları
tamamlanmış değildir. Bu ara sonuç hedefin tamamlanması yerine geçmez.
