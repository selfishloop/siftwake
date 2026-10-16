# WAL tamponu yaşam süresi: indeksleme bellek tepesi

Kabul edilen dar değişiklik: yeni commit'in kodlanmış WAL tamponunu önceki
delta'nın flush işlemiyle üst üste tutmamak ve WAL append/sync tamamlanınca
tamponu delta indekslemesinden önce bırakmak. Üç çiftin tamamında indeksleme
özel bellek tepesi düşük ve yazma + bakım hızlıdır. Bakım RAM işi kapanmadı.

## Üç eşleştirilmiş koşu

| Ölçüm | Kontrol medyan | Aday medyan |
|---|---:|---:|
| Yazma | 34,885s | 32,950s |
| Son bakım | 1,862s | 1,745s |
| Yazma + son bakım | 36,705s | 34,695s |
| İndeksleme observed private peak | 124,11MiB | 111,52MiB |
| Bakım observed private peak | 101,06MiB | 103,66MiB |
| Tüm ingest prosesinin OS peak working set'i | 128,50MiB | 115,41MiB |
| Boşta private | 21,11MiB | 20,71MiB |
| Tutulan DB dosyaları | 1.513.744.522B | 1.513.744.522B |

İndeksleme private medyanı%10,14, bütün ingest prosesinin OS peak working
set medyanı%10,18 düşük. Toplam süre medyanı%5,48 kısa. Bakım private medyanı
%2,57 yüksek; bakımda bellek kazanımı iddiası yok. Boşta fark küçük, temel
delta/directory belleği değişmedi.

| Çift / sıra | Kontrol toplam s | Aday toplam s | Toplam değişim | İndeksleme private değişimi |
|---|---:|---:|---:|---:|
| 0: kontrol → aday | 39,697 | 34,209 | -%13,83 | -%10,14 |
| 1: aday → kontrol | 35,394 | 34,695 | -%1,97 | -%10,65 |
| 2: kontrol → aday | 36,705 | 35,569 | -%3,09 | -%8,14 |

Koşullarındaki farklar korunur; en iyi çift seçilmez. Bu sonuç her corpus /
batch boyutunda hızlanma garantisi değildir. SQL bu tur yeniden ölçülmedi;
[önceki aynı10k karşılaştırmada](WRITE-MAINTENANCE-10K-20261013.md) SQLite
indexing private8,33MiB idi. FDB'nin~112MiB seviyesiyle RAM farkı hâlâ büyüktür;
farklı oturumlardaki sürelerden güncel SQLite üstünlük oranı çıkarılmaz.

## Kod ve güvence

Commit'in frame sınırı önce doğrulanır. Non-WAL-delta yolunda pending flush
gerekiyorsa `binary_batch::encoded_len` sınır/schema hesaplaması yapılır,
gerçek `format::encode` flush sonrası çalışır. Pending flush yokken önceki
encode yolu korunur. Batch mutation guard/doc validation daha önce aynı
sırada yapılır. WAL-delta feature'ının location accounting yolu korunur.

Append başarılı olduktan sonra owned encoded byte Vec bırakılır; batch
changes ve gerekirse WAL location'ları delta apply için kalır. WAL sync,
acknowledgement, sequence ve hata/poison davranışı değişmez. Transaction
codec, source framing, manifest, index directory ve protocol aynı kalır.
Yeni feature/bağımlılık/format yok;0.1 fixture korunur.

Son state iki tarafta10k belge,sequence20,generation31,delta0,5 run;
source267.900.709B,index353.242.581B,WAL45B. İndeks yeniden üretilmediği
okuma testleri dışında yalnız sahiplik işaretli sabit FDB benchmark slotu
ingest için sıfırlandı. SQLite slotu bu tur değiştirilmedi.

## Reddedilen önceki deneme

Mevcut akışlı checkpoint yazıcısını feature dışına çıkarıp source batch
kopyalarını kaldırma denemesi üç çift ölçüldü. Medyan toplam39,539→41,815s
(+%5,75), indexing private124,47→122,14MiB (küçük düşüş), OS peak working
set128,24→128,50MiB. Eşleşmiş toplam farkları-%16,34/+%18,60/-%0,29;
tekrarlanabilir hız kazancı ve belirgin RAM kazancı yoktu.

Bu runtime değişiklikleri geri alındı. Mevcut checkpoint yazıcısı eskisi gibi
experimental-write-buffer-budget altında kaldı; source batch yazımı aynı.
Feature genişletilmedi. Bu deneyin tek bayt bozulması kontrolü eklenmiş testi
korundu. Deney toplam diskinde31B ready metadata farkı vardı; kabul edilen
WAL yaşam süresi denemesinde fark0B. Log prefix'leri ayrıdır:
`ram-stream-10k-20261013` (reddedildi), `ram-lifetime-10k-20261013` (kabul).

## Corpus, doğrulama modu ve ölçüm sınırları

Aynı ilk10k belge `data/rkyv-corpus-20261009/json.payload-bench`,
264.842.858 text byte,2.057–51.197B/doc. Yeni corpus/download/prepare yok.
Sabit `data/benchmark-work/fdb` her ingest öncesinde yalnız benchmark sahibi
slot olarak sıfırlanır; DB kopyaları tutulmaz. Logs `docs/benchmarks`.

Release merge-postings + delta-bitmaps, grouped-read ve diagnostic kapalı;
iki taraf aynı özelliklerle. Aynı500 belge/transaction,20 synchronized WAL
commit. CRC-checked corpus read, JSON input decode, indeksleme, commit ve
40 batch edge full-document kontrolü ingest timer içinde; son flush/merge
ayrı. OS cache eviction yok, arka plan sistem yükü izole değil, cold cache
ve concurrent bakım ölçülmez.

Proses private/WS20ms örneklenir, phase100ms log poll ile belirlenir; kısa
peak/faz sınırları kaçabilir. OS peak WS bütün ingest prosesini kapsar.
MiB1048576B; bunlar engine allocation quota veya OS file-cache toplamı değildir.
Profiler allocator sayaçları ayrı tanılama koşusundadır; süreleri normal
benchmark gibi kullanılmaz, origin peak'leri aynı anda olmuş sayılıp toplanmaz.

Ayrı diagnostic allocator koşusunda total requested-live peak
118.493.430→104.711.593B (%11,63 düşük), frame_encode origin peak
22.204.381→13.854.378B görüldü. Caller/delta/source_batch/packed_metadata
origin peak'leri aynı kaldı. Origin, allocation oluşurken verilen etikettir;
sonradan taşınan belgenin güncel sahibini tek başına açıklamaz. Bu requested
byte değerleri OS private/WS değildir, allocator overhead/fragmentation içermez.
Loglar `ram-{write,lifetime}-origins-20261013-fdb-ingest-10000-0.jsonl`;
diagnostic sürelerinden normal hız kıyaslaması yapılmaz.

`tools/measure-stream-checkpoints.ps1 -Prefix ram-lifetime-10k-20261013
-Candidate output/ram-lifetime-candidate-20261013.exe` üç paired ingest/idle
koşu yapar. Kontrol `output/ram-write-control-20261013.exe`. Her koşu sonrası
10k full-document/version ve dört contains sorgusu total/ilk50 full-page
oracle ayrı prosesle doğrulandı. Her aday koşusu ayrıca önceki control exe
ile tam okundu. Eski reader compatibility üç koşuda da geçti.

`tools/summarize-stream-checkpoints.py docs/benchmarks/ram-lifetime-10k-20261013`
yalnız loglardan medyan ve her paired farkı çıkarır. Executable SHA256 ve
ham logs/summary aynı prefix altında korunur.

## Takip

Final default/all-features all-targets test paketleri, `cargo fmt --check`,
default/all-features `cargo clippy --all-targets -- -D warnings` geçti.
Kesilme, tek bayt bozulması, acknowledged commit ve merge süreç sonlandırma,
sağlam cache arkasında bozuk kaynak ve eski fixture testleri korundu.
Güncel release binary aynı DB'de38-case full-page/document/version oracle
kontrolünü geçti (`ram-lifetime-matrix-correctness.jsonl`, performance_not_measured).
Ek dört-query warm kontrol logu `ram-lifetime-matrix-check.jsonl` altında
korunur; paired search hız kapısı olarak kullanılmaz. Son release examples
merge+bitmap açık, grouped/diagnostic kapalı build olarak bırakıldı.
Loglar `ram-lifetime-{final-tests-default,tests-all-features,fmt,clippy,clippy-all-features}.log`.

İndeksleme tepesindeki bu dar adım ölçüldü. [OPEN-WORK](OPEN-WORK.md):
delta/body/posting ve bakım çalışma alanı RAM'i, eski nesil disk alanı,
eşzamanlı okuma/yazma gecikmesi ve cold cache açık. Arama artışları önceki
kullanıcı kabulüyle kapalı; bu tur yeni arama hız iddiası yok.
