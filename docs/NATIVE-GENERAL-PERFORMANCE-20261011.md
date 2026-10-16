# Kalıcı ikili motor: genel performans ölçümü

Sonraki [native ikili FDB / SQLite genel karşılaştırması](NATIVE-SQLITE-GENERAL-20261011.md)
tamamlandı: iki100k corpus, 12 yeni process, aynı get ve sorgu setleri.
Aşağıdaki SQLite kapsam dışı ifadeleri bu JSON/binary ölçümünün sınırıdır.

Bu rapor OFF1/FDBLOG04/TXN2/FDBSEG04 motorunu aynı kod tabanından üretilen
default JSON kontrolüyle karşılaştırır. İki düzen de payload'ı authoritative
source ve derived index içinde kopyalar. `.idx` yalnız source offset tutan
yerleşim uygulanmadı/ölçülmedi; iki yerleşimden hangisinin hızlı olduğu hakkında
bu karşılaştırmadan sonuç çıkarılmaz.

JSON kontrolü ayrı ignored `output/native-general-json-reference-20261011`
snapshot'ında yalnız JSON frame/SEG03 encode, JSON size accounting/validation ve
magic seçimi geri alınarak oluşturuldu. Current fold/query/posting/visibility/
maintenance kodu aynıdır. Tarihsel release veya git commit değildir. Default
feature'larla iki harness için release thin-LTO/codegen-units=1 executable'ları
ayrı adlarla freeze edildi. Kaynak checkout ve önceki executable'lar korunur.
[Kontrol patch'i](benchmarks/native-general-20261011-json-reference.patch),
[snapshot SHA-256 listesi](benchmarks/native-general-20261011-json-reference-sha256.json),
[executable SHA-256](benchmarks/native-general-20261011-executables.json).

## Corpus ve işlem kapsamı

Kısa: 100.000 benzersiz key/title, tekrarlı 256 UTF-8 byte Türkçe metin,
source/day alanları, batch500. Her process yeni DB, WAL/sync açık, default
16 MiB delta byte ve explicit default 10000 key watermark. Ingest corpus
oluşturma + commit + foreground maintenance içerir; background işler örtüşebilir.
Final maintenance flush/wait/bütün merge'leri bitirir. Toplam ingest+maintenance
esas yazma ölçüsüdür; saf WAL throughput değildir. Dar metin, exact filtre,
numeric range, offset0/50/99950, 100 get ve yaygın contains sorguları tam payload
oracle'ıyla doğrulanır. 50 update/50 delete ve sonraki reopen ayrıca doğrulanır;
update/delete ayrı gecikmesi ölçülmez.

Uzun: 100.000 farklı gerçek kitap pasajı, 2.048–51.200 byte, toplam gövde
2.655.160.287 byte, seed20261009; sekiz Gutenberg eserinden benzersiz SHA-256
pencereleri, pencereler örtüşebilir. 100k bağımsız eser değildir. Aynı fields/
links/version/key ve oracle kullanılır. JSON hazır source/SEG03 dizini korunur;
binary source canonical OFF1 export'tan yeni bir dizinde hazırlanır, motor tüm
kaynağı doğrular ve SEG04 cache'ini oluşturur. Bu hazırlık **API ingest ölçümü
değildir**. İlk cache kurulumu ana ölçümden ayrıdır; ana koşularda hazır tek
segment ve boş delta vardır. `Database::get` 100 sabit dağınık key için dört
tur (ilk warmup), her sonuç tam oracle kontrolü. `Database::search`: `the`,
`abbreviation`, `of`, `the whale` AND; offset0/50 ve common99950, limit50.
31 search/process, 186 search toplam; count/key order/version/fields/links ve
sequence200 dahil bütün sayfa timer dışında doğrulanır.

Her corpus üç bağımsız process çifti, sıra JSON/binary, binary/JSON, JSON/binary.
Kısa sorgular harness'teki 3/30 örnek + warmup; uzun dar sayfalar 10 örnek,
büyük sayfalar process başına bir timed örnek; ilk query sayfası warmup alır.
Tablolar process p50'lerinin üç koşu medyanı, kazançlar eşleşmiş process
oranlarıdır. Cold cache/forced eviction yoktur. Source/cache doğrulama bütün
dosyaları okur; warmup tam-resident disk garantisi değildir. OS file cache
process working set'e dahil değildir. Ana ölçümde build/test/hash/profile yoktur.

20 ms istenen sample aralığıyla OS PeakWorkingSet64 ve private byte maksimumu
gözlenir. Private gerçek peak garantisi değildir, scheduler aralığı uzatabilir.
Window tüm process: validated opens, corpus/oracle, bütün işlemler. Read ve
write process peak'leri farklı corpus/window'dur; steady-state heap ölçümü
değildir. Disk `.fdb/.idx/.ready` fiziksel byte toplamıdır; kısa corpus'ta
saklanmış eski nesiller dahil retained footprint. Long corpus'ta tek source +
cache + ready; LOCK/oracle/export/executable/plan/kitaplar hariçtir. Legacy
engine source_bytes sayacı double-count ettiği için disk toplamı olarak kullanılmaz.

## Sonuçlar

12 ana process exit0; bütün full-document/count/order/version oracle'ları geçti.
Kısa corpus'ta 792, uzun corpus'ta 186 gerçek search: **978 search** toplam.
Uzun corpus 2400 timed+warmup get, kısa corpus 100-get turları ve ek doğrulamalar.
Hiçbir negatif koşu veya sorgu karşılaştırmadan çıkarılmadı. Aşağıdaki yüzde
aralıkları eşleşmiş üç çiftin kazanç oranlarıdır; negatif değer yavaşlamadır.

### Kısa corpus: yazma ve genel akış

| İşlem | JSON medyan | Binary medyan | Eşleşmiş kazanç aralığı | Tekrar |
|---|---:|---:|---:|---|
| 100k ingest | 10.203 s | 9.975 s | -35.44 .. +10.73% | karışık |
| Final bakım | 7.304 s | 6.326 s | -7.90 .. +17.78% | karışık |
| Yazma + bütün final bakım | 17.507 s | 15.980 s | -23.53 .. +12.01% | karışık |
| Hazır veritabanını açma | 739.867 ms | 552.232 ms | -2.19 .. +29.61% | karışık |
| 50 update / 50 delete sonrası açma | 700.897 ms | 562.037 ms | +11.61 .. +25.31% | 3/3 hızlı |
| Dar title metin | 4.481 ms | 4.183 ms | +2.56 .. +6.84% | 3/3 hızlı |
| Exact ilk sayfa | 0.305 ms | 0.280 ms | -13.20 .. +22.83% | karışık |
| Exact ikinci sayfa | 0.320 ms | 0.307 ms | +1.16 .. +5.37% | 3/3 hızlı |
| Exact son sayfa | 22.343 ms | 22.644 ms | -14.48 .. +1.61% | karışık |
| Exact + numeric range | 0.593 ms | 0.532 ms | -14.85 .. +23.69% | karışık |
| 100 tam belge get | 1.282 ms | 1.091 ms | -1.68 .. +26.56% | karışık |
| Yaygın teknoloji contains | 542.692 ms | 379.519 ms | +24.05 .. +38.33% | 3/3 hızlı |

Yazma+bakım medyanı yaklaşık %8,72 kısa sürer, ancak ilk çift binary'de
%23,53 yavaş, diğerleri %12,01 ve %8,72 hızlıdır. **Tekrarlanabilir yazma
kazancı gösterilmedi**. Exact son sayfa iki çiftte yavaş; exact ilk sayfa,
range ve küçük get de karışıktır. Bütün işlemlerin hızlandığı söylenemez.
Kısa yaygın contains üç çiftte %24–38 hızlandı; dar title ve ikinci exact
sayfa üç çiftte kazanır. Warmup/OS/background/cache farkları kontrol sınırlarıdır;
hangi bileşenin ilk çift yavaşlamasına neden olduğu profillenmedi.

### Uzun corpus: gerçek motor okuma ve içerir

| İşlem | JSON medyan | Binary medyan | Eşleşmiş kazanç aralığı | Tekrar |
|---|---:|---:|---:|---|
| Tam kaynak doğrulamalı açılış | 27.448 s | 18.728 s | +29.31 .. +35.38% | 3/3 hızlı |
| 100 dağınık tam belge get | 6.551 ms | 4.156 ms | +25.84 .. +50.38% | 3/3 hızlı |
| the / offset0 | 11464.877 ms | 7722.219 ms | +25.20 .. +44.93% | 3/3 hızlı |
| the / offset50 | 11096.319 ms | 7615.200 ms | +29.46 .. +45.48% | 3/3 hızlı |
| the / offset99950 | 11094.961 ms | 7665.547 ms | +29.52 .. +42.01% | 3/3 hızlı |
| abbreviation / offset0 | 265.292 ms | 240.620 ms | +8.86 .. +26.85% | 3/3 hızlı |
| abbreviation / offset50 | 255.502 ms | 239.031 ms | +6.45 .. +27.88% | 3/3 hızlı |
| of / offset0 | 11061.521 ms | 7438.433 ms | +28.40 .. +44.35% | 3/3 hızlı |
| of / offset50 | 10986.019 ms | 7498.790 ms | +27.22 .. +33.00% | 3/3 hızlı |
| the whale / offset0 | 15576.017 ms | 12623.654 ms | +18.95 .. +19.98% | 3/3 hızlı |
| the whale / offset50 | 15813.129 ms | 13295.276 ms | +14.70 .. +21.26% | 3/3 hızlı |

Dokuz sayfa üç eşleşmiş çiftte **27/27** hızlıdır. Yaygın %25–45, iki karakter
%27–44, AND %15–21, dar %6–28 kazanır. Açılış %29–35, 100 uzun belge get
%26–50 hızlıdır. Bunlar kendi JSON kontrolümüze göre sonuçlardır; yeni SQLite
karşılaştırması veya genel DB üstünlüğü değildir. Uzun corpus'ta timed ingest
ölçülmedi. İlk source/cache hazırlığı tek binary process'te 259,05 s sürdü;
bu hazırlık uyumsuz eski ilk-build zamanıyla hız karşılaştırmasına alınmadı.

### Bellek ve disk

| Corpus / sayaç | JSON medyan | Binary medyan |
|---|---:|---:|
| Kısa: tepe working set | 93.43 MiB | 99.91 MiB |
| Kısa: maksimum örneklenen private | 92.42 MiB | 96.34 MiB |
| Kısa: retained motor dosyaları | 466.99 MB | 418.32 MB |
| Uzun: tepe working set | 130.30 MiB | 130.24 MiB |
| Uzun: maksimum örneklenen private | 133.98 MiB | 132.07 MiB |
| Uzun: retained motor dosyaları | 6293.01 MB | 6182.93 MB |

Uzun disk **6.293.010.040 → 6.182.926.010 byte (-%1,75)**. Kısa retained disk
**466.991.541 → 418.320.443 byte (-%10,42)**; eski source/index nesilleri dahil.
Uzun WS yaklaşık130MiB'de kaldı: anlamlı toplam RAM kazanımı yok. Kısa WS
93,43→99,91MiB, üç çiftte %5,13–8,81 artış gösterdi. Uzun private medyanı
azalsa da bir çiftte %21 yükselir; kesin heap/RAM kazanımı ilan edilmez.
Bu değerler OS file cache veya bütün bilgisayar RAM'i değildir. Eski20MiB
rakamlarının composite feature/corpus düzeni bu default-only ölçümle aynı değildir.

Altı long source/index/ready dosyasının ölçüm öncesi/sonrası SHA-256'sı eşleşti.
[Önce](benchmarks/native-general-20261011-files-before.json),
[sonra](benchmarks/native-general-20261011-files-after.json).
[Ham medyanlar, eşleşmiş farklar ve doğrulama](benchmarks/native-general-20261011-summary.json).
[Makine/configuration](benchmarks/native-general-20261011-machine.json).
`cargo fmt --check` ve `cargo clippy --all-targets -- -D warnings` geçti;
yeni harness'in get/search oracle'ları 12 native process'te doğrulandı.
[fmt](benchmarks/native-general-20261011-fmt.log),
[clippy](benchmarks/native-general-20261011-clippy.log).

Karar: kalıcı binary sürüm **uzun metin okuma/arama için tekrarlanabilir
kazanç sağladı**, disk gerilemedi. Yazma/küçük sorgu ve RAM sonuçları karışıktır;
koşulsuz genel performans veya eski birleşik hız+RAM kabul kapısı geçti denmez.
Sonraki iki bağımsız çalışma yazma/bakım varyansının profili ve ayrı sürümlü,
default kapalı source-offset cache'in aynı API/corpus üzerinde karşılaştırılmasıdır.
İkinci düzen ölçülmeden payload kopyasını kaldırmanın hızlandırdığı iddia edilmez.


## Yeniden üretim

```powershell
# Fresh snapshot/data/log prefix gerekir; önceki dosyalar ezilmez.
python tools/prepare-json-reference.py
cargo build --release --example measure_scale --example measure_engine_fold
cargo build --release --manifest-path output/native-general-json-reference-20261011/Cargo.toml --target-dir output/native-general-json-target-20261011 --example measure_scale --example measure_engine_fold
# Dört executable'ı ayrı output/fdb-native-general-{json,binary}-{scale,long}-20261011.exe adlarıyla freeze et.
python tools/prepare-native-binary-passages.py
tools/measure-native-general.ps1 -Phase prepare
tools/measure-native-general.ps1 -Phase short
tools/measure-native-general.ps1 -Phase long
python tools/summarize-native-general.py docs/benchmarks/native-general-20261011
```

Uzun corpus ingest, tek-kopya/source-offset düzen, concurrent throughput,
power loss, cold cache, compression ve yeni SQLite karşılaştırması kapsam dışıdır.
Depolama/recovery testleri önceki native format doğrulamasında yapılmıştır;
bu çalışma yalnız harness'e dağınık get ölçümü ekler ve benchmark araçları oluşturur.
