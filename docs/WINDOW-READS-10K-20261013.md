# Sorguya özel sınırlı payload okuma penceresi

Sonraki inceleme: [11 artış için normalleştirme ve adaptive window](REGRESSION-READS-10K-20261013.md).
Aynı özelliklerle yeni matris36 düşüş/2 artış; önceki11'in tamamında pooled
p95 düşük. Bu belgedeki V1/V2 sayıları tarihsel koşular olarak korunur.
Pencere hâlâ default kapalıdır.

**Genel regresyon kapısı geçmedi.** Pencere mevcut
`experimental-grouped-read` altında,default kapalı olarak korunur. Yeni feature
eklenmedi; default scratch layout ve exact individual-read yolu korunur.
V1/V2 performance koşuları feature gating öncesi prototiplerdir; final gated
build ayrı38-case full-oracle correctness kontrolünden geçirilmiştir.

İlk birleşik prototip38-case p95'inde27 düşüş/11 artış gösterdi. V1 ham
sonuçları `window-reads` / `window-matrix` prefix'lerinde korunur. V2 dar
exact read kodunu ayrı tutar ve DiskIndex layout değişikliğini kaldırır;
final değerlendirme `window-split-reads` / `window-split-matrix` üzerinden
yapılır. Prototip sonuçları seçilip birleştirilmez.

## Kapsam ve güvence

Native OFF1 contains adaylarında küçük read_at isteklerini azaltan query-local
pencere eklendi. En küçük posting seed'i segment live kayıtlarının en az
yarısını kapsıyorsa ve en az64 live kayıt varsa aday olabilir. Ayrıca üç
fiziksel olarak bitişik requested record görülmeden read-ahead başlamaz.
Dar veya seyrek seed'ler exact individual read yolunda kalır. Query kelimesi
adına özel koşul yok; eşik metadata yoğunluğu ve fiziksel offsetlerdir.

Pencere en fazla256 KiB; aynı reusable payload Vec'i kullanır, ek global RAM
cache veya belge kopyası yok. V2 dosyanın checksum'lı prefix sınırında durur;
posting/directory bölgesinden de küçük overread olabilir.256 KiB'den büyük tek belge
önceki individual read kapasitesini kullanır;256 KiB tüm query RAM sınırı
değildir. Query bittiğinde pencere bırakılır; eşzamanlı query'ler paylaşmaz.

Her kullanılan record slice CRC, OFF1 offsets/UTF-8/typed metadata/key/version
kontrollerinden geçer. Cached window tüm record'ları doğrulanmış saymaz.
Read_at mevcut VFY touched-block kontrolünden geçer; hata sonrası incomplete
pencere usable olmaz. Source guard bütün `.fdb` kaynaklarını search öncesi
kontrol eder. External file mutation after verification desteklenmez; bu
query-local cache bu eski immutable sözleşmeyi genişletmez. Legacy JSON ve
active rkyv overlay individual yolları korunur.

Normal ve offset/prefix query loops aynı scratch politikasını kullanır;
toplam/order/paging/filter/shadow/tombstone algoritmaları aynı. DiskIndex
layout'u V2'de aynı; prefix bound query-local metadata read ile bir kez alınır.
Kaynak/WAL/index/cache biçimi, feature ve
API/protokol değişmedi.0.1 fixture korunur. Get/hydration ayrı yolları korur.

## Profil

Ön diagnostic geniş AND gözleminde9932 payload adayına karşı1127 read_at
request ve8805 window hit;267.550.090 requested byte karşı286.914.971 read
byte (~%7,24 overread). Bütün record'ların CRC/schema kontrolü sürer. Önce
toplam disk.read_at9992, sonra1187 (postings/hydration da dahil). Bu sayaçlar
helper request sayısıdır, fiziksel disk I/O veya guaranteed syscall sayısı değil.
Read phase108,77→77,06 ms, query210,97→172,48 ms tek gözlem; nested wall-time
span'lar toplanmaz ve paired performance gate yerine geçmez.

Loglar `benchmarks/window-profile-{before,after}.jsonl`; profil erken
prototiptendir, dar seed'in exact read cache şartı sonradan netleştirildi.

## Ölçüm sözleşmesi

Mevcut aynı ilk10k belge `data/rkyv-corpus-20261009/json.payload-bench`:
264.842.858 text byte,2.057–51.197 byte/doc, sekiz Gutenberg kitabının
benzersiz/örtüşen pasajları. Sabit mevcut FDB10k,5 source,sequence20,delta0.
Reset/rebuild/prepare/new corpus/DB/cache yok. Control önceki safe UTF-8
sürümü; iki taraf release merge-postings+delta-bitmaps, diagnostic kapalı.
OS cache sıcak, eviction yok; farklı CPU/cold-cache/concurrent ölçülmez.

`tools/measure-borrowed-source.ps1 -Prefix docs/benchmarks/window-reads-10k-20261013
-Control output/window-read-control-20261013.exe -CheckMatrix -MeasureMemory`:
beş paired first-use open/get9999/quixote/repeat; pending5→5→0 iki tarafta.
Üçer warm process: engine başına9k timed get ve query başına90 sample,
the/quixote/he/the man full total/page/document/version bağımsız corpus oracle.
Nearest-rank pooled p95/p99,query p99 bu örnek sayısında maksimumdur.
Oracle/warmup API timer dışı; read/CRC/UTF-8/schema/normalization timer içinde.

Hedef10 ms poll first-use ve tüm warm process'te Windows private committed
ve OS peak working set'i örnekler. Warm process **oracle hazırlığı, warmup ve
get/query** dahil tüm benchmark prosesidir; yalnız motor idle RAM'i değildir.
Kısa peak'ler kaçabilir; kesin memory gate veya allocation quota sayılmaz.

`tools/measure-window-matrix.ps1`: üç paired process/variant,seed0/1/2,
aynı38 query/page; her primary20 ve page5 timed örnek/process, full total/page/
StoredDocument version oracle. SQLite mevcut readonly2 MiB cache/mmap0,
JSON full decode ve FTS5 trigram comparator'dan dört-query **tek referans
process** ayrıca alınır; repeatable SQL performance veya equal-engine gate
sayılmaz. Tüm FDB/cache ve SQLite DB byte/SHA256 önce/sonra eşit olmalı.

## Ölçülmüş sonuç ve karar

V2 üç paired warm process,90 sample/query/variant:

| İşlem | Önce p95 ms | Pencere p95 ms |
|---|---:|---:|
| get | 0,0472 | 0,0409 |
| the | 188,4897 | 146,5813 |
| quixote | 5,7965 | 5,3582 |
| he | 184,0055 | 146,3527 |
| the man | 203,0641 | 157,4680 |

Üç geniş query per-process p95 toplam9/9 pair'de hızlı; pooled%20–22 düşük.
Get3/3 hızlı. Quixote pooled p95%7,56 düşük fakat per-process1/3 artış
(4,0770→4,2225 ms). V1 warm sayılarıyla sonuç seçilip birleştirilmez.
V2 ilk quixote search170,88→177,28 ms (%3,75 artış); open+get+search237,55
→245,32 ms. İlk use'ta hızlanma iddiası yok; bu dar seed pencere kullanmaz.

V2 üçer matrix process'te38 case'in **27'sinde pooled p95 düşük,11'inde
yüksek**. Önemli artışlar: geologist28,19→30,11 ms (%6,78,3/3 pair yavaş),
quixote offset57 page3,23→3,66 ms (%13,32,3/3 yavaş),300-byte negative
0,1111→0,1435 ms (%29,16 fakat +0,0324 ms,2/3 yavaş). Diğer sekiz artış
yaklaşık%0,27–1,98. Bütün full total/page/document/version oracle'ları geçti.
Sayı ve sıralama doğru olmak performance regression kapısını geçmek değildir.

Bu yüzden pencere **default'a alınmadı**; mevcut grouped-read experiment'ine
alındı. Default bütün window fields/helper code'larını compile dışına alır;
normal core exact read algoritması ve DiskIndex layout'u aynı kalır. Dar
query regresyonlarının nedeni kesinleştirilmedi; overread/eligibility veya
codegen/allocator etkileri sonraki scoped incelemedir.11 case işi kapanmadı.

V2 tüm warm benchmark process'i (oracle/get/query dahil) observed private
peak median41.611.264→41.299.968 B,OS peak WS44.761.088→44.630.016 B.
Bu yaklaşık39,7 /42,6 MiB benchmark prosesidir; idle motor RAM'i veya kesin
peak/quota değildir. Kısa peak'ler kaçabilir, fark küçüktür; genel RAM kazanımı
ilan edilmez. Query pencere kapasite sınırı süreç RAM sınırı değildir.

SQLite dört-query tek referans process p95:the1128,25,quixote17,08,
he1120,08,AND1209,33 ms. V2'nin146,58/5,36/146,35/157,47 ms değerleri bu
specific readonly2 MiB cache,mmap0,JSON+trigram ayarlarında düşük; SQLite'a
karşı repeatable/general engine üstünlüğü ilan edilmez. SQL source/payload/
CRC/open işi FDB ile eşit değildir; reference ilk matrix turunun oturumundadır.

Tüm FDB/cache ve SQLite DB byte/SHA256 değerleri aynı. Ek DB/cache/corpus
yok; DB disk artışı0 B. Default/prototype artifact dosyaları DB alanı değildir.
Prototip summary'leri `benchmarks/window-split-{reads,matrix}-10k-20261013-summary.json`;
SQL reference`window-matrix-10k-20261013-sqlite-reference.jsonl`.

## Test ve açık işler

Final default all-targets suite,grouped/all-features targeted window tests,
fmt ve default/all-features Clippy -D warnings geçti. Gating öncesi full
all-features suite de geçti. Final default ve experimental grouped build
38-case full-oracle doğruluk kontrolü ayrı yapılır. Loglar`window-final-*`.

Default/all-features all-targets testleri,fmt ve all-targets Clippy -D warnings
geçti. Yeni window testleri dense/sparse/backward/duplicate slot order,
cross-window boundary,large-document individual path,prefetched bad record,
unopened VFY bad block ve file cut'ı kapsar. Mevcut authoritative source
corruption/process-kill/0.1 compatibility testleri de geçti. Unit fixture'lar
bounded ve auto-cleaned; benchmark corpus üretimi değildir.

[OPEN-WORK](OPEN-WORK.md): güncel SQLite write/maintenance,geniş RAM/disk
ve concurrent bakım yükü açık. İlk arama/get/open bu optimizasyonun hedefi
değildir; dar-query ve get artışları genel kapıda ayrıca değerlendirilir.
