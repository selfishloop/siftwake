# Resident belge paylaşımı ve delta sorgu kopyaları — 2026-10-09

## Kapsam ve karar

`experimental-shared-documents` eklendi; motor bağımsızdır, SDB/Fundops'a
entegrasyon yapılmadı. Yeni DB/arama bağımlılığı yok. Kalıcı FDBLOG01/03,
MAN03, SEG03 ve MET04 byte biçimleri değişmedi; 0.1 fixture kaynakları korundu.
Belge paylaşımı henüz default değil. Delta sorgularındaki tam anahtar kopyaları
default dahil kaldırıldı. SQLite'tan daha hızlı ve daha düşük toplam proses
belleği hedefi **tamamlanmadı**; SQLite bu aşamada yeniden ölçülmedi.

## Yerleşim ve doğruluk

Row anahtarı docs tablosunda tutar, key'i payload içine yeniden koymaz.
Gövde String veya Arc<str>; alanlar BTreeMap veya Arc<BTreeMap> taşır.
Her cache için tahmini 128 KiB / 256 entry admission sınırı var. Gövde >4096
byte, alan >16 veya nested array/object içeriyorsa kendi değerlerini saklar.
CRC yalnız bucket seçimidir; tam içerik/canonical alan JSON eşitliği gerekir.
Küçük uygun alan miss'i canonical serialization scratch üretebilir. Cache
eviction yapmaz, flush'a kadar tutulur. Referans/allocator/B-tree yükü toplam
RAM kotasına alınmış değildir.

Get yalnız caller-owned belgeyi klonlar, WAL okuma/JSON decode yapmaz.
Commit payload'ı dayanıklı append sonrası paylaşır; hatalı apply motoru poison
eder. Recovery kaynak doğrulamasını korur; flush ardından cache serbest kalır.
Delta-byte watermarks serialized belge uzunluğuyla ölçülür. Sayısal metin sayacı
alan ağaçlarını kapsamaz ve proses belleği yerine sunulmaz.

İki unit testi 1000 tekrar/benzersiz kayıt, kontrollü hash çakışmaları,
admission sınırı, büyük/nested içerik, sürüm/key/links ve cache bırakıldıktan
sonra payload eşitliğini doğrular. Dönen belgenin text/fields/links'ini değiştirmek
resident kaydı etkilemez. Seçilmiş feature suite crash/cut/corruption/cache,
guard/update/delete/flush/merge ve 0.1 kaynak uyumluluğunu ayrıca doğrular.

## 10 bin unflushed delta sorgusu

`measure_delta_queries`: yeni ayrı DB, 20 × 500 commit, delta keys10001 /
bytes64MiB; 10000 belge, 0 segment, hiçbir flush. Baseline merge-postings /
shared-delta-text / delta-bitmaps; candidate buna shared-documents ve sorgu
anahtar referanslarını ekler. Diagnostic/WAL features kapalı. B/C,C/B,B/C.
Her corpus için üç çift; açılış ve yazmalar query süresine dahil değil.
10 untimed warmup; küçük sorgular200, son sayfa/get100, metin20 tekrar.
Native API, sıcak OS cache; benchmark sırasında build/test/profile çalışmadı.
Tam total/sıralı StoredDocument payload oracle kontrolleri timer dışındadır.

Repeated: aynı 256 UTF-8 byte gövde, day=id%30, source=synthetic, rapor başlığı.
Unique: her gövdeye ` unique-{id:010}` ve nested metadata eklenir; bu corpus
paylaşım cache'lerinin fallback'ini de sınar. Mixed: çift ID'ler unique,
tek ID'ler repeated. `text_bytes=256` temel gövde uzunluğudur; unique gövdeler
ek sonekle daha uzundur. Get100 ID'leri n*97+1, dar metin rapor-0000005000,
geniş metin teknoloji, exact source, range day2..5, son sayfa offset9950.

V2 yalnız belge paylaşımı bazı geniş query sürelerini korumadı; default'a
alma kapısı geçmedi. Kaynak incelemesi her eşleşmenin Key ve üç String'inin
kopyalandığını gösterdi. V3 indeks Arc<Key>'lerini tutar ve ara ID Vec'ini
kaldırır. Bu nedenle sonuç listesinin kendisi hâlâ N referans ve sort maliyeti
taşır; bütün query memory sabit bütçeli değildir.

V3 medyan p50 süreleri (ms; her satır 3 koşunun medyanı):

| Corpus / sorgu | Baseline | Candidate |
| --- | ---: | ---: |
| repeated ilk sayfa | 1.2950 | 0.2740 |
| repeated range | 0.2452 | 0.0868 |
| repeated son sayfa | 1.3181 | 0.2857 |
| repeated metin | 2.3347 | 0.7626 |
| repeated get100 | 0.0750 | 0.0716 |
| unique ilk sayfa | 1.3248 | 0.2789 |
| unique metin | 2.2779 | 0.7665 |
| unique get100 | 0.0965 | 0.0951 |
| mixed ilk sayfa | 1.3078 | 0.2898 |
| mixed metin | 1.8765 | 0.7472 |
| mixed get100 | 0.0890 | 0.0827 |

Dar sorgular yaklaşık0.004ms; mixed p50 0.0044→0.0045ms, bu küçük değişim
genel olarak her sorgunun hızlandığı iddiasını desteklemez. Geniş ilk sayfa
%78–79, metin %60–67 daha kısa. p95 ve diğer sayfalar ham summary'dedir.
Bu test toplam proses RAM ölçmez; `resident_text_bytes` RSS değildir.

Ham delta V2/V3: `docs/benchmarks/shared-documents-delta-v{2,3}-20261009-*`.
Özet üretimi: `python tools/summarize-delta-pairs.py
docs/benchmarks/shared-documents-delta-v3-20261009`.

## 100 bin tam native yazma / bakım

İlk V1 üç çift: medyan yazma+bakım 9.2792→9.1880s; tepe working set
33.8594→23.1328MiB (%31.68 azalma). Bütün süreç açılış/yazma/bakım/reopen/
sorgu/50 update/50 delete penceresinde ölçüldü; toplam sistem RAM ve OS-wide
file cache dahil değil. Private tepe 20ms hedefli örneklemedir, kesin high-water
değeri değildir. 100k, aynı 256 byte tekrarlı gövde, batch500, keys10000 /
bytes16MiB. Bu corpus paylaşım için uygundur; benzersiz büyük dosyalara aynı
RAM kazancı genellenemez. Sorgular çoğunlukla flushed immutable kayıtları kapsar.

V1 üç çiftte final ready + iki authoritative fdb + iki idx byte-identical:
`shared-documents-v1-20261009.byte-equality.json`. Her koşu exact total,
sıralı payload/sürüm, 50 update/delete, yeniden açılma bağımsız oracle'ından
geçti. V1/V2 native ham ölçümleri ayrı tutulur; V2 ayrıca delta anahtar
referanslarını içerir.

V2 final kod, yeniden B/C,C/B,B/C üç çift, aynı corpus ve pencere:

| Medyan | Baseline | Candidate |
| --- | ---: | ---: |
| Yazma | 5.8057s | 5.6815s |
| Son bakım | 3.4522s | 3.4182s |
| Yazma + bakım | 9.2652s | 9.0997s |
| Tepe working set | 34.6445MiB | 22.6055MiB |
| Örneklenen private tepe | 32.1953MiB | 19.0547MiB |

Sum her koşunun toplamının medyanıdır. Working set %34.75, sum %1.79 azaldı.
Yeni üç çift de oracle'dan geçti; final ready/two-fdb/two-idx byte-identical
(`shared-documents-v2-20261009.byte-equality.json`). Binaries SHA256:
`shared-documents-20261009.binaries.json`.

Flushed sorgu p50/p95 ms medyanları:

| Sorgu | Baseline | Candidate |
| --- | ---: | ---: |
| narrow | 2.9648 / 3.6876 | 2.7244 / 3.6816 |
| broad | 0.1907 / 0.3161 | 0.2894 / 0.3146 |
| range | 0.4628 / 0.6553 | 0.4141 / 0.6433 |
| second page | 0.1900 / 0.2603 | 0.1947 / 0.3140 |
| last page | 1.2172 / 1.4639 | 1.2580 / 1.3216 |
| broad text | 222.9618 / 228.8317 | 225.5371 / 227.5479 |
| get100 | 1.4313 / 1.5039 | 1.4646 / 1.4883 |

Kısa sorguların dağılımı değişken: broad p50 yükseldi, p95 aynı düzeyde;
second-page p95 de yükseldi. Bu veri her flushed sorguda hız korunmasının
kanıtı değildir. Delta V3 kazancı ayrı workload'da doğrulandı. Cold-cache,
bir milyon unique/büyük belge ve taze SQLite karşılaştırması ölçülmedi.

## Son doğrulama

Final kodda `cargo fmt --check` başarılı. `cargo test --all-targets` 53,
paylaşımlı resident yol için seçilmiş feature'larla 88, `--all-features` 91
test geçti. Default, seçilmiş ve all-feature
`cargo clippy --all-targets -- -D warnings` başarılı. Raw test logları
`docs/benchmarks/shared-documents-{default,selected,all-features}-tests-20261009.log`.
All-features WAL delta'yı tercih eder; paylaşım testini tek başına kanıtlamaz.
Selected test özellikle resident Row/cache yolunu çalıştırır. Kesilme,
bozulma, acknowledged process-kill, background merge, random model,
interleaved deep pagination ve 0.1 kaynak byte uyumluluğu bu testlerde geçti.
Normal CLI default `cargo build --release --bin fdb` ile yeniden üretildi:
SHA256 `5B6FE9C22407A17496F0C2DFD4D1340C352362D356334179144E4655A9611EC3`.

## Kalan hedef

Yeni, ayrı diagnostic allocator profili: toplam requested allocation tepe
14169896 byte (13.5135MiB); bu RSS değildir ve profiler süreleri native hız
kanıtı değildir. Bağımsız origin tepeleri delta_tables4.9481MiB,
delta_index2.8162MiB, builder3.0839MiB, packed_metadata1.3761MiB. Bu tepeler
aynı anda oluşmaz, toplanamaz. Origin ilk allocation sahibini gösterir;
değer başka yapıya taşınsa da origin değişmez. 100000 kayıt sonundaki en
yüksek sampled live dağılımında delta_tables5188304, delta_index2903666,
packed_metadata998940 byte. Ham: `shared-documents-origins-v2-20261009.jsonl`.
Kontrollü I/O reservation tepe131072/limit262144 byte; bu toplam heap değildir.
Dolayısıyla sonraki iş yalnız builder tamponunu küçültmek olmayacak:
docs/shadow/index arasındaki yinelenen anahtar yerleşimi de azaltılacak.

Ortak delta/builder/worker toplam heap bütçesi hâlâ yok. Kaynak/key/indeks
yönetimi, builder spill grupları ve metadata scratch tekrar incelenecek.
SQLite ile taze eşleşmiş ölçüm, cold-cache/çeşitli büyük corpus ve default
promotion kapıları tamamlanmadan asıl hedef bitmiş sayılmaz.
