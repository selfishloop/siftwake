# Yazma hattı doğrulaması — 08.10.2026

Bu belge V1/V2 frozen binary sonuçlarını anlatır. Güncel feature seçiminde
bakım kilitleri ayrıca açılır; sonraki [bileşen karşılaştırmasına](WRITE-COMPONENTS-VALIDATION.md) bakın.

Mevcut motorun flush/merge hattı iyileştirildi; bağımsız yeni motor veya kalıcı
format değişikliği yapılmadı. `experimental-write-pipeline` streaming-open ve
paged-metadata deneylerini içerir ve hâlâ varsayılan olarak kapalıdır.

## Uygulama

- Otomatik merge dört benzer boyuttaki bitişik run için bekler (max/min <= 2).
  Açık bakım 2..4 run birleştirir; 100 bin key, 64 MiB kaynak ve max/min <= 4
  sınırları korunur. Seçim keys/(run_count-1) maliyetini küçültür. Bu politika
  büyük bir eski run'ın her küçük flush ile yeniden işlenmesini azaltır.
- Builder kayıt metadata'sı Vec<Record> yerine geçici, uzunluk/CRC kontrollü
  spool'da saklanır. Finish en fazla 256 KiB entry scratch ile aynı SEG03 JSON
  records dizisini üretir. Posting sözlüğü ve açılıştaki packed tablo kalır.
- Foreground/worker proses çapında bir kaynak yazımı slotu ve ayrı bir builder
  slotu paylaşır. Kaynak/checksum tamamlanınca ilk kilit bırakılır, builder
  kilidi sonra alınır. Bir yazıcı ve bir builder örtüşebilir; iki aynı aşama
  örtüşemez. Kilitler birlikte tutulmaz. Başka DB'lerin bakımı da bu sınıra tabidir.
- Merge payload'ı seçilmiş slot'tan okur; tekrar key araması kaldırılır.
  Payload CRC/key/version kontrolleri korunur.
- Posting eklemesindeki contains_key + entry çift ağaç araması tek entry
  aramasına indirildi. Bu küçük değişiklik normal build'de de uygulanır;
  posting sırası ve kalıcı kodlaması değişmez.

`.fdb` ve `.ready` kalıcı kaynaktır. FDBLOG01/03, MAN03 ve SEG03 değişmedi;
sağlam cache bozuk kaynağı kabul ettirmez. Spool türetilmiş geçici veridir;
normal dönüş/drop temizler, süreç sonlandırma kalıntıları otomatik GC edilmez.
Spool ve eski serde records dizisi Unicode/escape verisiyle byte düzeyinde
eşleştirilir; her kesilme noktası ve payload bozulması hata verir.

## Ölçüm sözleşmesi

Windows native release, yeni DB dizinleri, 100.000 belge, 256 UTF-8 bayt tekrarlı
Türkçe metin, 500'lük batch. Üç ardışık çiftin sırası baseline/candidate,
candidate/baseline, baseline/candidate. Baseline önceki streaming-open native
binary'sidir; candidate write-pipeline v2'dir. Normal ölçümde diagnostic-timing
ve allocation sayaçları kapalıdır. Corpus/sorgular önceki
[aynı sonuç sözleşmesi](WORKLOAD-COST-VALIDATION.md) ile aynıdır.

Exact total + sıralı, tam StoredDocument sayfaları bağımsız oracle ile timer
dışında doğrulanır. Dar/exact/range/ikinci sayfa 1 warmup + 30 tekrar;
son sayfa/geniş metin 1 warmup + 3 tekrar. 100 farklı key ile belge okuma,
50 güncelleme, 50 silme ve reopen de doğrulanır. Cache ingest/warmup ile sıcak;
cold-cache testi değildir. Ingest corpus üretimini içerir; son bakım dahil
toplam yazma süresi ayrıca gösterilir.

Working set OS proses sayacıdır; private committed 20 ms istenen aralıkla
örneklenir (Windows scheduler gerçek aralığı değiştirebilir). İkisinin penceresi
bütün benchmark prosesidir, idle RAM değildir. OS dosya cache'i bunlara dahil
bir toplam sistem belleği ölçümü değildir. Allocator requested byte ayrı,
instrumented çalışmadır; RSS veya allocator overhead ölçmez.

## Sonuçlar

Üç run medyanı; sorgularda her run p50 medyanı.

| Ölçüm | Önce | V2 | Değişim |
| --- | ---: | ---: | ---: |
| Ingest | 11.4119 s | 12.3537 s | +8.3% |
| Son bakım | 9.7741 s | 6.3029 s | -35.5% |
| Ingest + son bakım | 21.4858 s | 18.7912 s | -12.5% |
| Dar metin p50 | 4.0862 ms | 3.8777 ms | -5.1% |
| Exact ilk sayfa p50 | 0.2789 ms | 0.2852 ms | +2.3% |
| Exact ikinci sayfa p50 | 0.2675 ms | 0.3086 ms | +15.4% |
| Range p50 | 0.6066 ms | 0.7703 ms | +27.0% |
| Son sayfa p50 | 2.1033 ms | 1.6959 ms | -19.4% |
| Geniş metin p50 | 284.3588 ms | 329.9212 ms | +16.0% |
| 100 key okuma p50 | 2.0124 ms | 2.4887 ms | +23.7% |
| Proses tepe working set | 88.1758 MiB | 66.9531 MiB | -24.1% |
| Örneklenen private tepe | 88.8867 MiB | 67.0547 MiB | -24.6% |

Sorgu p95 medyanları ayrıca aşağıdadır; üç tekrarlı sorgularda p95 çok sınırlı bir kuyruk ölçüsüdür.

| Sorgu | Önce p95 ms | V2 p95 ms |
| --- | ---: | ---: |
| Dar metin | 5.1525 | 4.5361 |
| Exact ilk sayfa | 0.4065 | 0.4018 |
| Exact ikinci sayfa | 0.3851 | 0.3980 |
| Range | 0.9971 | 0.8811 |
| Son sayfa | 2.6060 | 2.3349 |
| Geniş metin | 299.0772 | 340.0256 |
| 100 key okuma | 2.5110 | 2.5830 |

Toplam yazma+bakım %12,5 ve tepe working set %24,1 düştü. Ancak ingest
%8,3, range p50 %27,0, geniş metin p50 %16,0 ve key okuma p50 %23,7 arttı.
Bütün hız kapıları geçilmedi; tek ilk çiftin daha iyi sonucuna dayanarak
başarı iddiası yapılmaz. İki native binary son bakımda iki run ve aynı
payload/posting sayılarıyla sonuçlanır. Regresyonun tüm nedenleri henüz
ayrıştırılmadı; çalışma sınırları ve farklı iş sıralaması olası etkenlerdir.

V1 tek birleşik bakım slotuyla ölçüldü: toplam 21,486 → 22,340 s;
working set 89,344 → 66,352 MiB. Hız gerilemesi nedeniyle V2 kaynak yazımı
ve builder slotlarını ayırdı; posting eklemesindeki çift arama da kaldırıldı.
V1/V2, bu iki değişiklik için ayrı ablation değildir.

Ayrı wall-time profilinde ilk ingest+son bakım builder.add çağrıları
340.000 → 300.000, yani %11,8 azaldı. Sonraki 50 güncelleme her iki profilde
50 ek çağrıdır. V2 pencereleri 170.000 ingest ve 130.000 son bakım;
aktif worker span'ları pencere sınırlarını aşabildiği için ilk iki pencere
birlikte değerlendirilir. İç içe süreler CPU yüzdesi değildir.

Ayrı instrumented yazma ölçümünde requested Rust heap tepe 58,806,102 byte (56.082 MiB); önceki streaming v4 profilinde 81.323.150 byte idi. Bu sayaç toplam proses RAM'i değildir.


## Sınırlar ve sonraki adım

İki çalışma slotu toplam byte bütçesi değildir. Delta, caller batch, çıktı
belgeleri, token sözlüğü, kaynak frame ve açılıştaki packed tablo ayrıca bellek
kullanır. Options.max_delta_bytes serileştirilmiş delta watermark'ıdır; heap
kotası değildir. Write_run dışındaki open/cache rebuild bu kilitlere tabi değildir.
Delta dahil ortak byte bütçesi ve bütün workload'larda SQLite'tan düşük RAM
hedefi tamamlanmadı.

Bu tur yalnız aynı FDB baseline/candidate karşılaştırmasıdır. SQLite yeniden
ölçülmedi; önceki 100 bin belge testinde SQLite yazma+bakım 8,229 s ve tepe
working set 8,949 MiB idi. Bu sonuçlar SQLite hedefinin tamamlandığı anlamına
gelmez. Milyon kayıt, farklı/büyük metin, cold-cache ve çoklu DB/client yükü
bu turda ölçülmedi. Otomatik merge'in daha çok run bırakmasının sorgu etkisi
bu corpus dışında ayrıca ölçülmelidir. Deney bu nedenle default yapılmadı.

Bir sonraki iş, delta/worker/builder'ın gerçek allocation tüketimini ortak
byte bütçesinde yönetmek ve merge sırasında değişmeyen posting'lerin yeniden
üretimini azaltmaktır. Kalıcı formatın bütünüyle değiştirilmesi bu adımın
önkoşulu değildir.

## Doğrulama ve yeniden üretim

- `cargo fmt --check`: geçti.
- `cargo test --all-targets --all-features`: 68 test geçti.
- `cargo clippy --all-targets --all-features -- -D warnings`: geçti.
- `cargo test --all-targets`: 50 test geçti.
- `cargo clippy --all-targets -- -D warnings`: geçti.
- `cargo build --release --bin fdb`: geçti; normal CLI yeniden oluşturuldu.

Testler byte-uyumlu spool, her spool kesilmesi, payload bozulması, merge seçimi,
kaynak/cache/manifest bozulması, yarım WAL işlemi, süreç sonlandırma, worker
sürerken yazma ve 0.1 fixture kaynaklarının değişmeden açılmasını kapsar.
Elektrik kesintisinde Windows dizin metadata kalıcılığı bu turda ölçülmedi.

Yeni bir ölçüm diziniyle örnek tekrar:

```powershell
cargo build --release --features experimental-write-pipeline,experimental-write-maintenance-slots --example measure_scale
./tools/measure-native.ps1 -Executable target/release/examples/measure_scale.exe -DatabasePath data/write-pipeline-repeat -Count 100000 -TextBytes 256 -OutputPrefix docs/benchmarks/write-pipeline-repeat
```

Üç çift medyanını yeniden üretme:

```powershell
python tools/summarize-native-pairs.py docs/benchmarks/write-pipeline-v2-20261008 --engines baseline candidate --runs 3
```

Ham ölçümler `docs/benchmarks/write-pipeline-v2-20261008-{baseline,candidate}-{0,1,2}`
prefix'leriyle JSONL ve memory JSON dosyalarındadır. Medyanlar
`write-pipeline-v2-20261008.summary.json`; teşhis ve heap ayrı
`write-pipeline-profile-v2-20261008.jsonl` ve
`write-pipeline-heap-v2-20261008.jsonl` dosyalarındadır. V1'in başarısız hız
ölçümleri de saklanır; eski kullanıcı/benchmark kaynakları silinmedi.

Frozen release SHA-256:

- `output/fdb-equivalent-native-v2-20261008.exe`: `D84D82E6FDC14C7A0BB95F309287AD90C5C3E2E59C5C4CE58953076D229FF22F`
- `output/fdb-write-pipeline-v1-20261008.exe`: `5D6F139B3D0072B96C9ECE1DC4DCECAC4FA0E2884F007538DA23ADC6E392A4EE`
- `output/fdb-write-pipeline-v2-20261008.exe`: `D6BF0E28F6BE69DFB88CC368C738C50B3F9C50FF645C6E20B6E89250175E65F1`
- `output/fdb-write-pipeline-cli-v2-20261008.exe`: `3EAA69B2834E97C706A011DD5DCF2D4807F392DA79558310387564961C7B4D02`
- `output/fdb-write-pipeline-profile-v2-20261008.exe`: `B2353D565533D4CB4D6A6B06DD21C33E11BB742D555EBA1BC55B5E63A3F0D09D`


Normal CLI `target/release/fdb.exe` SHA-256: `51C385A1FDCC7154AF332651AA859F0F336E3C26201C8D27BF878BCB15E70A7C`.
