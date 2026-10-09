# Akışlı checkpoint ve ortak yazma tampon bütçesi — 08.10.2026

Bu aşama, en büyük geçici yazma allocation'larını kaldırır ve kontrollü I/O
buffer'larını kaynak yazıcısı, builder ve merge worker arasında byte olarak
paylaştırır. `experimental-write-buffer-budget`, mevcut write-pipeline'i içerir;
normal/default build değişmez. Bu **delta dahil toplam motor heap/RSS bütçesi
değildir**. Ölçülüp giderilen tüketici kaynak batch/frame tamponlarıdır.

## Allocation kökeni ölçümü

`diagnostic-memory` ayrı teşhis derlemesinde thread-local köken hook'u açar.
`profile_write_origins` allocator'ı her allocation'a başlık ekler; realloc ve
başka thread'de free, ilk kökeni korur. Kullanıcıda oluşturulup delta'ya taşınan
belge caller_documents olarak kalır. Bu sınıflar güncel sahiplik veya live
nesne türü değil, allocation'ın üretildiği yerdir.

İlk V3 profilindeki bağımsız requested peak'ler: source_batch 23,302 MiB,
frame_encode 21,375 MiB, delta_index 12,906 MiB, delta_tables 6,145 MiB,
caller_documents 9,641 MiB, builder 4,773 MiB, packed_metadata 10,750 MiB.
**Bu bağımsız tepeler toplanmaz**. Örnekleme anları tam aynı zaman değildir.
Header/align ek yükü ve realloc sırasında eski+yeni block örtüşmesi profili
etkiler; bunun süre/RSS rakamları normal native performans değildir. Requested
ölçüm allocator overhead, stacks, OS file cache veya C allocation ölçmez.
Raw: `docs/benchmarks/write-origins-v3-20261008.jsonl`.

## Uygulama ve doğruluk

- Unpublished immutable run için checkpoint JSON doğrudan dosyaya akar.
  8 MiB hedef frame sonunda header uzunluğu/payload CRC/header CRC yazılır;
  footer ve header tamamlanınca sync_all yapılır. Byte kodlaması aynı
  TXN1/FDBDONE1 frame biçimidir; yeni run FDBLOG03 kullanır, eski FDBLOG01
  kaynaklarını okuma desteği korunur; kayıt formatı sürümü değişmez.
- Eski Vec<Change> batch ve bütün kaynak frame Vec'i kaldırılır. Iterator'dan
  bir Change alınır, yazılıp bırakılır. Büyük/composite field gerekiyorsa
  allocation oluşturmadan serialized boyu sayılır; kayıt 64 MiB frame
  sınırını geçirecekse yeni frame açılır. Tek frame sınırı korunur.
- Transaction WAL aynı atomic encode/append yolunu kullanır; bu akışlı writer
  yalnız henüz manifestte yayınlanmamış immutable run kaynağında çalışır.
  Yarım kaynak hiçbir hazır cache ile kabul edilmez; publication protokolü,
  CRC/identity kontrolleri ve authoritative .fdb/.ready düzeni korunur.
- Proses çapında ortak 256 KiB reservation pool vardır. Kaynak payload çıkış
  tamponu tercih 64 KiB, minimum 8 KiB alır. Uygun alan azsa tampon küçülür;
  minimum yoksa alanın serbest kalmasını bekler. Builder payload writer ve
  record spool writer/read buffer birlikte 16 KiB rezervasyon alır.
- Rezervasyon allocation'dan önce alınır ve ilgili buffer'lar bırakıldıktan
  sonra RAII ile iade edilir. Kaynak lease'i builder'a geçmeden bırakılır.
  İç içe lease alınmaz; eski tek aşama kilitlerinin büyük çalışma alanını
  sıraya sokan davranışı geri getirilmez.

Pool yalnız bu kontrollü buffer'ları kapsar. Delta B-tree/Arc/index, caller
batch/belge, WAL frame Vec'i, tekil decoded belge, builder posting/body/token
sözlüğü, spill writer/read cursors, kayıt scratch'i, packed metadata tabloları,
page cache ve çıktı belgeleri bu kotaya dahil değildir. Büyük tekil belge,
8 MiB chunk hedefinden büyük olabilir. Sabit toplam RAM garantisi yoktur.

## Üç çift medyanı

Baseline, önceki write-pipeline V3 frozen binary; candidate, write-buffer V3.
Sıra baseline/candidate, candidate/baseline, baseline/candidate.

| Ölçüm | Önce | Akışlı/bütçeli | Değişim |
| --- | ---: | ---: | ---: |
| Ingest | 8.4912 s | 8.2897 s | -2.4% |
| Son bakım | 5.2815 s | 4.7867 s | -9.4% |
| Yazma + bakım | 13.7726 s | 13.0520 s | -5.2% |
| Proses tepe working set | 67.1055 MiB | 49.0781 MiB | -26.9% |
| Örneklenen private tepe | 66.0742 MiB | 46.6875 MiB | -29.3% |

İlk harness sorgu p50 / p95 medyanları (ms):

| Sorgu | Önce | Sonra |
| --- | ---: | ---: |
| Dar metin | 2.8501 / 3.6914 | 3.1373 / 3.6837 |
| Exact ilk sayfa | 0.1868 / 0.2996 | 0.2101 / 0.3069 |
| İkinci sayfa | 0.1898 / 0.2783 | 0.1986 / 0.3322 |
| Range | 0.3956 / 0.5163 | 0.5429 / 0.7355 |
| Son sayfa | 1.2132 / 1.2649 | 1.8418 / 2.3121 |
| Geniş metin | 209.2222 / 211.9586 | 220.2772 / 233.4293 |
| 100 key | 1.4078 / 1.6227 | 1.8413 / 2.2677 |

RAM %26,9 ve yazma+bakım %5,2 azaldı. Ancak ilk post-ingest harness'te
sorgu p50/p95 gerilemeleri var; bunlar saklanır ve sonuçtan çıkarılmaz.
Değişen kod yazma hattında olsa da bütün hız kapıları geçti iddiası yapılmaz.

## Aynı hazır DB üzerinde daha çok sorgu tekrarı

`measure_ready_queries`, iki derlemeyi aynı sağlıklı candidate-0 DB üzerinde
üç dönüşümlü çiftle çalıştırır. Open ve yazma süreleri dışarıdadır; yeni API
commit/flush çağrısı yoktur. Kontroller bütün belge/total/version'i doğrular.
10 warmup; küçük sorgular 200, son sayfa/get100 100, geniş metin 20 tekrar.
DB 50 update/50 delete sonrasındadır: narrow 50001 (değişmemiş kayıt), son
sayfa offset 99900, get100 keys n*997+1 (değişmemiş kayıtlar). İlk harness'in
çoğu sorgusu update/delete öncesindedir; dolayısıyla **mutlak süreler iki
harness arasında karşılaştırılmaz**. Bu kontrol aynı veri/yerleşim ve daha
çok tekrar ile iki derlemenin okuma yolunu karşılaştırır; önceki farklı DB/
yazma geçmişi/koşu dalgalanmasının nedenini tek başına kanıtlamaz.

| Sorgu | Baseline p50 / p95 ms | Candidate p50 / p95 ms | p50 değişim |
| --- | ---: | ---: | ---: |
| Dar metin | 2.6021 / 3.0564 | 2.6336 / 3.0402 | +1.2% |
| Exact ilk sayfa | 0.2156 / 0.3483 | 0.2152 / 0.2411 | -0.2% |
| İkinci sayfa | 0.2307 / 0.2949 | 0.2275 / 0.2911 | -1.4% |
| Range | 0.3925 / 0.5111 | 0.3971 / 0.5111 | +1.2% |
| Son sayfa | 4.2912 / 4.8503 | 4.2572 / 4.8548 | -0.8% |
| Geniş metin | 198.8292 / 232.3607 | 194.2347 / 230.9852 | -2.3% |
| 100 key | 1.4651 / 1.9148 | 1.5030 / 2.1841 | +2.6% |

Aynı DB kontrolünde büyük gerilemeler yeniden görülmedi; p50 farkları yaklaşık
-2,3%..+2,6% aralığında. Küçük farkların kalıcı regresyon veya gürültü olduğu
bu örneklemle kesinleşmez. İlk harness'teki regresyonların nedenini tam
ayrıştırmadığımız için feature **default olarak kapalı kalır**. SQLite bu turda
ölçülmedi; SQLite'tan düşük RAM veya her sorguda daha yüksek hız hedefi tamamlanmadı.

## Teşhis profili

| Allocation kökeni | Önce bağımsız requested peak MiB | Sonra |
| --- | ---: | ---: |
| other | 0.193 | 0.193 |
| caller_documents | 9.641 | 9.641 |
| delta_index | 12.906 | 12.906 |
| delta_tables | 6.145 | 6.145 |
| source_batch | 23.302 | 0.428 |
| frame_encode | 21.375 | 0.562 |
| builder | 4.773 | 6.142 |
| packed_metadata | 10.750 | 10.750 |

Header allocator toplam requested peak: 65,422,104 → 38,534,240 byte. Bağımsız sınıf peak'leri toplam değildir.
Ayrı, header kullanmayan allocator profilinde requested peak 38,117,037 byte (36.351 MiB); instrumented süreler native karşılaştırmaya katılmaz.

Kontrollü buffer rezervasyonu: limit 262,144, observed peak 81,920, final in-use 0, waits 0, reduced leases 0. Bu corpus kotayı doldurmadı; baskı/shrink/wait davranışı ayrı testlerle doğrulanır.

Ham dosyalar: `write-buffer-v3-20261008-{baseline,candidate}-{0,1,2}` JSONL/memory,
`write-buffer-v3-20261008.summary.json`, `write-buffer-ready-20261008-*`,
`write-buffer-origins-v3-20261008.jsonl`, `write-buffer-heap-v3-20261008.jsonl`.
İlk V2 ölçümleri de saklandı; bunlar son büyük-field preflight düzeltmesinden
önceki binary'dir. Dizin boyutları inaktif kaynak/cache dosyalarını içerir;
canlı veri boyutu olarak sunulmaz. Sonraki ana tüketiciler delta indeksi,
caller'da oluşturulan belgeler ve packed metadata açılış tablolarıdır.


## Doğrulama

- `cargo test --all-targets --all-features`: 73 test geçti (çekirdek + yeni codec/budget/origin/concurrency testleri).
- `cargo test --all-targets`: 51 test geçti.
- `cargo test --features experimental-write-buffer-budget --test write_budget`: 8 DB ortak kota testi yeniden geçti.
- `cargo test --example measure_ready_queries`: hazır DB harness oracle testi ayrıca geçti.
- `cargo fmt --check`: geçti.
- `cargo clippy --all-targets -- -D warnings`: geçti.
- `cargo clippy --all-targets --features experimental-write-buffer-budget -- -D warnings`: geçti.
- `cargo clippy --all-targets --all-features -- -D warnings`: geçti.
- Feature release examples/CLI ve normal release CLI derlendi; normal `target/release/fdb.exe` geri oluşturuldu.

Hazır DB harness'i sonradan eklenmiş tek oracle testini ayrı çalıştırır; yukarıdaki
73/51 sayımlarına dahil değildir. Testler byte uyumlu stream codec, her küçük
frame kesilmesi, 6+61 MiB büyük alanları ayrı frame'e alma, quota shrink/wait ve
hata halinde lease iadesi, alignment/zeroing/realloc origin, 8 DB eşzamanlı
source yazımı/reopen ve rezervasyonların sıfıra dönmesini kapsar. Önceki kaynak/
cache/manifest bozulması, yarım WAL ve process-kill testleri de geçti. v0.1
fixture kaynakları değişmedi. Windows elektrik kesintisinde dizin metadata
kalıcılığı bu turda ölçülmedi.

İlk büyük-field testinde replay'e yanlış checkpoint sınırı verilmişti; immutable
kaynağın gerçek end sınırı kullanılarak düzeltildi. Bu test hatasıydı; kalıcı
format değişikliği yapılmadı.

## Tekrar üretim

Yeni DB dizinlerinde 100 bin kayıt, 256 UTF-8 bayt aynı tekrarlı Türkçe corpus,
batch 500; önceki tam StoredDocument oracle, total/sıra/version, 50 update ve
50 delete + reopen doğrulaması korunur. Sıcak OS/file cache; cold-cache,
milyon kayıt, çeşitli/büyük metin ve gerçek uygulama workload'u ölçülmedi.
Dar/exact/range/ikinci sayfa 30; son sayfa/geniş metin 3 timed tekrar; her
sorgunun untimed warmup'ı vardır. FDB ve native API ölçülür, IPC ölçülmez.
Benchmark'ta diagnostic-memory/timing ve allocator sayaçları kapalıdır.
İstenen memory sample aralığı 20 ms; Windows gerçek aralığı değiştirebilir.
Working set bütün benchmark prosesinin tepesidir; idle RAM veya sistem
file cache dahil toplam sistem RAM'i değildir. Private committed örneklenir.

```powershell
cargo build --release --features experimental-write-buffer-budget --example measure_scale
./tools/measure-native.ps1 -Executable target/release/examples/measure_scale.exe -DatabasePath data/write-buffer-repeat -Count 100000 -TextBytes 256 -OutputPrefix docs/benchmarks/write-buffer-repeat
cargo run --release --features experimental-write-buffer-budget,diagnostic-memory --example profile_write_origins -- data/write-origins-repeat
```

Frozen native/diagnostic SHA-256:

- `output/fdb-write-pipeline-v3-20261008.exe`: `BBA24EB5C3B6DAC78CA69F9DE8292209639693D7546795C53F9508E3A498009B`
- `output/fdb-write-buffer-v3-20261008.exe`: `FFD99D35962EDDC374C28C21297C633E3C0EDA7F9C017F7C10105B9C0EA71C00`
- `output/fdb-write-buffer-cli-v3-20261008.exe`: `D8C0FCCBA7FAB2D4CD0FDC26B4B77422FA63F733258EA28CFB1134771FD8E8D2`
- `output/fdb-write-buffer-heap-v3-20261008.exe`: `26D25F274D247CD32D608F148D1C653850EEC0828E74F7AA0E8A89A36C3EE7C5`
- `output/fdb-write-buffer-origins-v3-20261008.exe`: `F70C053AF4903CDE4C1B82E43F5FB24242DECD7B48439D67FF7939826C83274F`
- `output/fdb-ready-queries-baseline-20261008.exe`: `39176093927305D9DAA65063494677D87C993A4324ED888B2FF34367C976045B`
- `output/fdb-ready-queries-candidate-20261008.exe`: `1E80BD70BDA3279A53B2BDD611D5492860495577EA5B66D4D01C201C412009D5`


Normal CLI `target/release/fdb.exe` SHA-256: `5CC116399AA3951D6BEEDD431F26F3E141629960FC243DA8066A8270E94264F3`.
