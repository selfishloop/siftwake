# İkili payload: gerçek Database::search ölçümü

Bu ölçüm önceki payload-only deneyinden farklıdır: aday üretimi, posting
kesimleri, görünürlük, toplam, sıralama ve tam sayfa çözme normal motor API'sinin
içindedir. Binary için kalıcı format değiştirilmedi; açıkça bağlanan atılabilir
payload export'u kullanıldı. Default kapalıdır. Karar: bu geçici motor overlay'i default için
**reddedildi**. Büyük sorgularda tekrarlanabilir kazanç var; native JSON'a göre
bir dar sayfa/tur kaybetti, toplam disk arttı ve private bellek medyanı yükseldi.
Bu karar önceki bağımsız codec kapısını geri almaz; kalıcı binary formatın
performansı bu deneyde ölçülmedi.

## Üç yol ve kapsam

- `native`: normal SEG03 JSON payload yolu, bağlama yok.
- `json`: aynı SEG03 posting/metadata, JSON benchmark export'u; positional
  okumada tekrar kullanılan byte tamponu. İkili codec için I/O kontrolüdür.
- `binary`: aynı posting/metadata ve kontrolle aynı positional okuyucu; OFF1
  borrowed title/text/key/version, metadata doğrulaması ve sonuçta tam çözme.

Üç yol da **aynı frozen release executable** içinde çalışır. Yalnız mevcut
`experimental-rkyv-payload` feature'ı açıktır; yeni feature eklenmedi. Env
seçimi yalnız `measure_engine_fold` example'ına aittir; normal Database açılışı
overlay keşfetmez. Açık `attach_payload_benchmark` çağrısı tek immutable
segment, boş delta, tombstone/paged metadata olmaması koşullarını doğrular.
Genel/kalıcı payload API'si değildir; bu corpus'un ordinal key/version
sözleşmesine bağlıdır. WAL, manifest, SEG03 ve protokol değiştirilmedi.

Önce normal açılış `.fdb` kaynağı ve SEG03 indeksini doğrular. Sonra export'un
tam CRC/schema/identity doğrulaması yapılır; bütün 100k tam belge mevcut
indeksin belgeleriyle eşitlikten geçirilir. Bozuk kaynak, sağlam cache var diye
kabul edilmez. Eksik/uyuşmayan export bağlanmaz. Her aday ve seçilen sonuçta
CRC, yapısal doğrulama ve key/version kimliği kontrol edilir. Binary doğrulama
alan ağacı oluşturmaz; UTF-8 ve metadata grameri yine taranır. Normalleşmiş
metinler saklanmaz; her yol aynı reusable fold ve 256 KiB query body cache'ini
kullanır. JSON kontrolü ve binary bir Mutex ile korunan reusable payload
tamponunu kullanır; eşzamanlı throughput ölçülmedi.

## Corpus, sorgular ve cache

100.000 farklı gerçek kitap pasajı, 2.048–51.200 byte, toplam gövde
2.655.160.287 byte, seed 20261009. Sekiz Gutenberg eserinden örtüşebilen,
SHA-256'sı benzersiz pencereler; İngilizce ağırlıklıdır, 100k bağımsız eser
değildir. [Provenance](benchmarks/rkyv-corpus-20261009.provenance.json).

Var olan `data/engine-fold-v2-20261011`: tek legacy JSON kaynağa bağlı SEG03,
sequence 200, delta/resident document 0. Belge sürümleri 1–200. Yeni veri
üretimi/indeks inşası yapılmadı; kaynak ve indeks dosyaları korundu.

Sorgular aynı: `the` (100000 toplam), `abbreviation` (264), `of` (100000),
`the whale` AND (16537). Her sorguda offset 0 ve 50, ayrıca `the` için 99950;
limit 50. Her tekrar toplam, sıra, sequence, version, fields/links dahil **tam
50 belge oracle'ını** doğrular. Sadece key/count performansı değildir.

Üç turda dokuz native proses seri çalışır; sıra native/json/binary,
json/binary/native, binary/native/json. Her yol her konumda bir kez çalışır.
Her sorgunun ilk sayfasında bir untimed warmup; sonraki sayfalar onu izler.
Büyük sayfalarda proses başına bir timed örnek, dar sayfalarda 10 örneğin p50'si.
Rapor medyanı üç proses p50'sinin medyanıdır; kazanç aralıkları eşleşmiş tur
oranlarıdır. Cache OS cache, zorla eviction yok. Ek bind bütün kayıtları okur
ve kendi cache etkisini yaratır; ilk query warmup bu farkı tamamen yok ettiğini
kanıtlamaz. Soğuk disk veya tüm dosyaların RAM'de olduğu garantisi yoktur.

Aynı Windows/Ryzen makinesi, release thin-LTO/codegen-units=1. Ana ölçüm
sırasında build/test/hash/profil çalıştırılmadı; yalnız bellek gözlemcisi ve
hafif log okumaları vardı. PeakWorkingSet64 ve private bellek 20 ms istenen
aralıkta gözlendi; gerçekleşen örnek aralığı scheduling nedeniyle daha uzun
olabilir. Private değer gerçek tepe garantisi olmayan maksimum örnektir.
Window: kaynak doğrulamalı open, tam overlay bağlama, corpus/books/oracle ve
bütün sorgular. OS file cache bu proses sayacına dahil değildir. Önceki
payload-only 12,41 MiB ile aynı bellek penceresi değildir.

## Sonuçlar

Dokuz proses sıfır exit code ile tamamlandı. **279 gerçek search çağrısının**
tamamında toplam, sequence ve 50 tam belge oracle'ı doğrulandı. Native JSON'a
göre 27 eşleşmiş sayfa/tur karşılaştırmasının 26'sında binary hızlıdır. Büyük
sorguların yedi sayfası her turda kazanır; dar ikinci sayfa da üç turda kazanır.
Dar ilk sayfa ilk turda 186,35→192,03 ms (%3,05 yavaş), diğer iki turda hızlıdır.
Bütün dokuz sayfanın her turda kazanması şeklindeki katı hız kapısı geçmedi.

Aşağıdakiler ms olarak üç prosesin p50 medyanlarıdır. Rare p50, her prosesin
10 örneği için sorted samples[len/2] olarak alınır. Eşleşmiş oran aralıkları
medyanların bölümü değildir.

| Sorgu / offset | Native JSON | JSON kontrol | Binary | Native'a göre üç tur kazancı |
|---|---:|---:|---:|---:|
| `the` / 0 | 7.729,05 | 7.736,81 | 5.891,14 | %22,26–26,72 |
| `the` / 50 | 7.614,06 | 7.910,57 | 5.675,55 | %22,22–25,55 |
| `the` / 99950 | 7.760,60 | 7.748,92 | 5.783,84 | %25,47–26,50 |
| `abbreviation` / 0 | 185,65 | 185,23 | 170,49 | %3,05 yavaş–%8,28 hızlı |
| `abbreviation` / 50 | 190,56 | 189,90 | 169,25 | %2,16–12,04 |
| `of` / 0 | 7.728,00 | 8.407,05 | 5.678,97 | %25,76–26,75 |
| `of` / 50 | 7.661,08 | 8.783,71 | 5.662,86 | %25,45–27,33 |
| `the whale` / 0 | 11.084,74 | 11.259,13 | 9.264,47 | %13,67–16,42 |
| `the whale` / 50 | 11.098,82 | 11.311,65 | 9.303,21 | %14,09–18,98 |

Aynı positional/reusable okuyucuyu kullanan **JSON kontrole göre 27/27**
eşleşmiş sayfa/tur hızlıdır: yaygın %23–29,5, dar %1,5–11,7, iki karakter
%25–36, AND %13–19. Bu kontrol, gerçek motor içinde codec/selektif çözme
kazancını destekler; ayrı dosya yerleşimi/cache farklarını tamamen ayıran bir
mikrobenchmark değildir. Native'a göre daha zayıf dar sorgu sonucunu örtmez.

| Medyan ölçü | Native JSON | JSON kontrol | Binary |
|---|---:|---:|---:|
| Normal kaynak doğrulamalı open | 19.635,76 ms | 19.663,98 ms | 20.468,31 ms |
| Ek export doğrulama/tam eşitlik bağlama | 0 ms | 22.733,99 ms | 13.988,47 ms |
| Toplam query'ye hazır olma | 19.635,76 ms | 42.397,97 ms | 34.456,78 ms |
| Proses tepe working set | 130,32 MiB | 130,32 MiB | 130,31 MiB |
| Örneklenen private bellek | 113,55 MiB | 132,29 MiB | 132,25 MiB |
| Ek export disk byte'ları | 0 | 2.740.862.279 | 2.686.823.501 |
| Toplam kullanılan motor dosyaları, byte | 6.293.010.040 | 9.033.872.319 | 8.979.833.541 |

Toplam disk `.fdb` + `.idx` + `.ready` ve seçilen export'tur; LOCK,
benchmark oracle/kitap/plan/executable/diğer kullanılmayan export'lar dahil
edilmez. Engine stats.source_bytes legacy kaynağı iki kez saydığı için bu
alan toplam disk olarak kullanılmadı. Ek export stats'a dahil değildir.

Working set farkı yalnız 8 KiB; **anlamlı toplam RAM azalması gösterilmedi**.
Private bellek medyanı native'a göre yaklaşık %16,47 yükseldi. Private
örneklerin aralığı geniştir: native 113,01–159,75 MiB, kontrol 131,78–159,76,
binary 102,29–134,07; medyanı tek başına kesin heap/allocator nedenselliği
olarak yorumlamayız. Bütün proses ve kaynak açılışı bu pencereye dahildir;
steady-state veya yalnız query RAM'i ayrı ölçülmedi. Önceki codec-only
16,51→12,41 MiB sonucundan bütün motorun 12 MiB olacağı çıkarılamaz.

Ek dosya nedeniyle native'a göre disk yaklaşık %42,70 büyür. Source/SEG03
JSON kopyaları halen vardır. Medyan toplam hazır olma yaklaşık 19,64→34,46 s;
ikili formatın bağımsız daha hızlı açılması, mevcut motorun kaynak doğrulamasını
ortadan kaldırmaz. Bu maliyetleri kalıcı formatın tahmini kazancı gibi sunmayız.

Kabul kapıları native'a göre: tüm sayfalarda tekrarlanabilir hız **hayır**,
working set medyanında gerileme yok **evet** (anlamlı azalma yok), sampled
private gerileme yok **hayır**, toplam disk gerileme yok **hayır**. Kapı
geçmedi; default'a alınmadı. [Ham koşular, medyanlar ve kapılar](benchmarks/engine-binary-20261011-summary.json).

Sorgu kazancı gerçek API'ye taşındı. Depolama/bellek kazancını göstermek için
ayrı bir kalıcı codec sürümü ve kaynak uyumluluğu/kurtarma çalışması gerekir;
metadata ve açılış belleği mevcut düzende kaldığından aynı kazanım garanti
edilemez. Bu görevde o geçiş yapılmadı.

## Doğrulama ve sınırlar

Gerçek API testinde sayfalama, title/body AND, Unicode/Türkçe, alan filtresi,
boş text ve scope ayrımı doğrulanır. Doğru CRC/key/version taşıyan fakat
metni farklı olan export reddedilir; başarısız bağlama normal okuyucuyu korur.
Bağlama sonrası backing file gerçekten bozulur, seçilen get CRC hatası verir.
Prefetched eski byte'ların bu bozulmayı gizlememesi için motor overlay'i
normal segment ile aynı positional I/O'yu kullanır. Normal reopen export'u
otomatik bağlamaz. Sağlam indeks yanında bozuk immutable `.fdb` reddedilir.

Binary bağlanmış child süreç başarılı sorgudan sonra öldürülür; `.fdb`,
`.idx`, `.ready` byte'ları değişmez, normal reopen/get çalışır. Eski taşıyıcı
kesilme/tek-byte-bozulma/export process-kill ve 0.1 JSON fixture testleri de
çalıştırılır. Yeni kalıcı format olmadığı için fixture değiştirilmedi.

`cargo test --features experimental-rkyv-payload`: **61 geçti, 0 hata**.
İki ignored child helper parent süreç-sonlandırma testlerinde ayrıca
çalıştırıldı. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
feature açık clippy ve feature+diagnostic-timing açık clippy geçti.
[Default clippy](benchmarks/engine-binary-20261011-clippy-default.log),
[feature clippy](benchmarks/engine-binary-20261011-clippy-feature.log),
[diagnostic clippy](benchmarks/engine-binary-20261011-clippy-timing.log).
`tests/engine.rs` ve fixture'larda diff yoktur.

Üç kaynak/cache dosyasının ve iki payload export'unun önce/sonra SHA-256'sı
aynı. [Kaynak önce](benchmarks/engine-binary-20261011-source-before.json),
[kaynak sonra](benchmarks/engine-binary-20261011-source-after.json),
[export önce](benchmarks/engine-binary-20261011-overlay-before.json),
[export sonra](benchmarks/engine-binary-20261011-overlay-after.json).

Bu yalnız bir segment/boş delta/okuma deneyidir. Kalıcı kaynak formatının
ikiliye geçirilmesi, WAL yazma/kurtarma, merge, builder, çok segment/tombstone,
paged metadata, eşzamanlı sorgu, compression, soğuk cache ve yeni SQLite
karşılaştırması ölçülmedi. `.idx` ve `.fdb` içinde mevcut JSON kopyaları halen
vardır; bu deney bütün motor belleğini veya diskini binary'ye dönüştürmez.

## Tekrar üretim

```powershell
cargo build --release --example measure_engine_fold --features experimental-rkyv-payload
# Yeni isimle freeze et; eski executable/log dosyalarını ezme.
tools/measure-engine-payload.ps1 -Prefix <yeni-prefix> -Executable output/fdb-engine-binary-20261011.exe
python tools/summarize-engine-payload.py docs/benchmarks/<yeni-prefix>
```

[Executable SHA-256](benchmarks/engine-binary-20261011-executable.json):
`DDD77081BF325FDB88C1CADCABB9FFA5DD1B4BD7C1FEFB05E52C9CEF3B82ECB6`.
[Test kaydı](benchmarks/engine-binary-20261011-tests.log).
