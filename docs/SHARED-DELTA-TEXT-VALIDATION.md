# Ortak delta metni doğrulaması — 2026-10-09

## Kapsam ve sonuç

`experimental-shared-delta-text` bağımsız feature olarak eklendi; ölçümde
`experimental-write-buffer-budget` ile birlikte açıktır. Başlık belgeye özgü
String kalır, katlanmış body cache üzerinden Arc<str> ile paylaşılır. Cache
256 KiB tahmini admission sınırını korur; sığmayan body ayrı allocation alır.
Posting B-tree/Arc<Key>, delta belge tabloları, caller belgeleri ve packed
metadata bu değişiklikte küçültülmedi. Flush/merge eşikleri değişmedi.

Üç dönüşümlü çiftte proses tepe working set %4,6 azaldı; yazma+bakım medyanı
%1,1 arttı. Range p50 %4 geriledi. Diğer sorgu medyanları iyileşse de tüm hız
kapıları geçmiş değildir; feature default kapalıdır. SQLite bu turda tekrar
ölçülmedi. SQLite altında RAM ve daha hızlı uçtan uca çalışma hedefi sürüyor.

## Native karşılaştırma

Baseline: frozen write-buffer V3; candidate: aynı yazma bütçesi + ortak delta
metni. Her ikisi allocator/timing teşhisi kapalı native API derlemesidir.

| Ölçüm | Baseline | Candidate | Değişim |
| --- | ---: | ---: | ---: |
| Ingest | 8.2966 s | 8.3262 s | +0.36% |
| Son bakım | 4.7431 s | 4.8917 s | +3.13% |
| Yazma + bakım | 13.0688 s | 13.2179 s | +1.14% |
| Proses tepe working set | 48.5781 MiB | 46.3359 MiB | -4.62% |
| Örneklenen private tepe | 45.9375 MiB | 44.1133 MiB | -3.97% |
| Reopen | 547.6327 ms | 568.2643 ms | +3.77% |

| Sorgu | Baseline p50 / p95 ms | Candidate p50 / p95 ms |
| --- | ---: | ---: |
| Dar metin | 3.1632 / 4.0168 | 2.9726 / 3.8794 |
| Exact ilk sayfa | 0.2434 / 0.3324 | 0.1881 / 0.2682 |
| Range | 0.4451 / 0.6801 | 0.4629 / 0.6446 |
| İkinci sayfa | 0.2008 / 0.2805 | 0.1852 / 0.2350 |
| Son sayfa | 1.9007 / 2.0825 | 1.8727 / 2.0130 |
| Geniş metin | 208.7100 / 227.6449 | 206.3041 / 221.2629 |
| 100 key | 1.6788 / 1.7282 | 1.4278 / 1.6380 |

100 bin kayıt, aynı tekrarlı Türkçe 256 UTF-8 bayt body, batch 500. Yeni DB
dizinleri; baseline/candidate, candidate/baseline, baseline/candidate sırası.
Aynı independent full StoredDocument oracle, total/sıra/version, 50 update/50
delete, reopen kontrolü geçer. Son durumda 99950 görünür belge, iki run,
4589406 posting, source 46428672 ve cache 81627542 byte her koşuda aynıdır.
Klasör diski 431069352 byte: inaktif nesilleri de içerir, canlı disk değildir.

Sıcak OS/file cache; sorgular untimed warmup sonrası çalışır. Dar/exact/range/
ikinci sayfa 30; geniş metin/son sayfa/get100 üç timed tekrar. p50/p95 tabloda
koşu medyanlarının medyanıdır. Örnekleme isteği 20 ms, gerçek aralık Windows
tarafından değişebilir. Working set tüm benchmark prosesinin tepesidir; idle
RAM veya sistem dosya cache dahil toplam sistem RAM değildir. Private tepe
örneklenir. Cold-cache, milyon kayıt ve gerçek/çeşitli içerik ölçülmedi.

Ham dosyalar `docs/benchmarks/shared-delta-v1-20261009-{baseline,candidate}-{0,1,2}`
JSONL/memory ve `shared-delta-v1-20261009.summary.json`.

## Doğruluk ve sınırlar

Sorgu terimleri Unicode whitespace ile ayrılır; hiçbir terim title/body
arasındaki newline separator üzerinden eşleşemez. Ayrı contains kontrolleri
aynı sözleşmeyi sağlar. Trigram posting, key sırası, sürüm ve shadow mantığı
değişmez. Kalıcı format/0.1 fixture ve publication protokolü değişmedi.

`resident_text_bytes` açık feature altında ortak folded gövdeyi allocation
başına bir kez ve cache içinde kalmış folded gövdeyi de sayar. Kaynak belge
title/text baytları ayrı sayılır. B-tree/Arc yönetimi, cache raw anahtarları,
allocator kapasitesi ve RSS bu sayaçta yoktur.

## Binary kimliği

- `output/fdb-write-buffer-v3-20261008.exe`: `FFD99D35962EDDC374C28C21297C633E3C0EDA7F9C017F7C10105B9C0EA71C00`
- `output/fdb-shared-delta-v1-20261009.exe`: `E813D4D4490F8AAB52F9374E3B723C0058C8BCB6B48B6CC495048DF8E13FA810`
- `output/fdb-shared-delta-origins-v1-20261009.exe`: `E97C7D5D44CBFD44FDF8C9B59DC98B34D1E8A18D2AC0916315E0AEBD9F8C385E`

## Ayrı allocation kökeni profili

Aynı 100 bin/batch500 header allocator corpus'u ile `delta_index` bağımsız
requested peak 13,532,994 → 11,496,306 byte (%15,05 düşüş). Toplam requested
peak 38,534,240 → 36,509,808 byte. Profil native zaman ölçümüne katılmaz;
allocation kökeni başlangıçta atanır, sonraki sahip değişimini izlemez.
Bağımsız sınıf peak'leri aynı anda oluşmayabilir ve toplanmaz. Bu byte
sayaçları proses working set veya private committed değildir. Eski profil
önceki turdan kalmadır; aynı corpus/allocator binary kullanır, bu turda
baseline profiler tekrar çalıştırılmadı. Worker örtüşmesi farklı olabilir.

Raw: `write-buffer-origins-v3-20261008.jsonl` ve
`shared-delta-origins-v1-20261009.jsonl`. Kontrollü I/O buffer pool final in-use
0, observed peak 81920 byte; delta paylaşımı bu rezervasyon kotasına dahil
değildir. Toplam heap byte bütçesi tamamlanmadı.

## Doğrulama ve tekrar üretim

- `cargo test --all-targets --features experimental-write-buffer-budget,experimental-shared-delta-text`: geçti (iki yeni test eklenmeden önce tüm mevcut feature testleri).
- `cargo test --features experimental-write-buffer-budget,experimental-shared-delta-text --lib shared_tests`: iki yeni test geçti.
- `cargo test --all-targets --all-features`: 76 test geçti, iki yeni test dahil.
- `cargo test --all-targets`: 52 test geçti.
- `cargo fmt --check`: geçti.
- `cargo clippy --all-targets -- -D warnings`: geçti.
- `cargo clippy --all-targets --features experimental-write-buffer-budget,experimental-shared-delta-text -- -D warnings`: geçti.
- `cargo clippy --all-targets --all-features -- -D warnings`: geçti.

Yeni testler ortak body allocation identity, Türkçe katlama, farklı başlıklar,
güncelleme/silme sonrası kalan referans, cache'e sığmayan ayrı büyük içerik ve
folded byte sayımını kontrol eder. Mevcut tam sorgu oracle, random update/delete,
source/cache corruption, tüm transaction cut noktaları, gerçek process-kill,
0.1 fixture ve disk/delta visibility testleri de geçti. Normal CLI bu turda
değiştirilmedi; native/teşhis example binary'leri output içinde donduruldu.

```powershell
cargo build --release --features experimental-write-buffer-budget,experimental-shared-delta-text --example measure_scale
./tools/measure-native.ps1 -Executable target/release/examples/measure_scale.exe -DatabasePath data/shared-delta-repeat -Count 100000 -TextBytes 256 -OutputPrefix docs/benchmarks/shared-delta-repeat
cargo run --release --features experimental-write-buffer-budget,experimental-shared-delta-text,diagnostic-memory --example profile_write_origins -- data/shared-delta-origins-repeat
```

İlk default Clippy çalıştırması feature-off dalındaki gereksiz String.into
conversion'ını buldu. Conversion yalnız feature-on dalına taşındı; default,
seçilmiş feature ve all-features kontrolleri ardından geçti.

Sonraki çalışma posting listelerinde tekrarlanan Arc<Key> yerine küçük belge
kimlikleri ve anahtar sırasını koruyan çözümleme olacak. Delta/caller/metadata
tüketicileri ve genel byte bütçesi hâlâ açık işlerdir. Bu rapor tamamlanmış bir
SQLite üstünlüğü iddiası değildir.
