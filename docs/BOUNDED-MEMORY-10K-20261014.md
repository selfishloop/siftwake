# Birlikte sınırlanan yazma/bakım ve sayfalı sözlük

Yeni yol mevcut `experimental-builder-budget` altında, default kapalıdır.
`Database::commit_streamed` çağrısı gerekir; normal `commit(Vec<Mutation>)`
ve protokol bu profile otomatik geçirilmedi. Kalıcı source/WAL/manifest/IDX
biçimleri değişmedi; 0.1 fixture korunur.

## Uygulama

- Fallible iterator bir belgeyi okur; 500 belgelik owned giriş Vec'i kurulmaz.
  Aynı 500 mutation tek atomik işlem ve tek sürümde kalır. Geçici native
  staging dosyası, küçük key/konum/CRC tablosu ve index-only bitmap delta'sı
  kullanılır. İndeksleme sırasında bütün belgelerin özgün/folded metinleri
  biriktirilmez; eski küçük BodyCache bütçesi korunur.
- Bütün input/guard/duplicate/limit kontrolleri WAL append'den önce tamamlanır.
  Normal TXN2 ile byte-eşit tek frame kopyalanıp sync edilir. CRC kontrollü
  staged kayıtlar sıralı source'a aktarılır; mevcut index/manifest yayımlama
  yolundan sonra başarı döner. Hata sonrası RAM state güvenilir değilse engine
  poisoned olur. Kesik WAL kuyruğu eski replay kuralıyla toparlanır.
- Her başarılı streamed commit sonunda resident delta boştur. Önceki merge
  intake öncesinde beklenir; büyük intake/delta/source/worker kopyaları birlikte
  birikmez. Bu seçim daha sık yayınlama ve yazma sırasında bakım beklemesi
  getirir; maliyeti toplam süreye dahildir.
- Ortak working allowance16MiB:14MiB streamed intake/index+builder için,
  2MiB sabit cache/scratch payı için ayrılır. Nested builder aynı allowance'ı
  kullanır; iki kere rezervasyon yapmaz. Ayrı bir maintenance builder8MiB
  alır. Bütçe, workspace kilidinden önce alınır. Cache mutex tutulurken bütçe
  beklenmez. `baseline_reserved_bytes` boş cache'te de ayrılmış kapasiteyi gösterir.
- Builder'ın record metadata'sına ek olarak posting dizini de checksummed
  geçici spool'dan JSON'a akıtılır; bütün terim Vec'i RAM'de biriktirilmez.
  Normal source/IDX bytes ve terim sırası korunur. Flush/merge mevcut gram /
  bitmap indeksini yeniden üretmeden kullanır.
- Disk terim sözlüğü mevcut IDX JSON dizininin byte aralıkları üzerinden
  okunur. Sparse first/last fences, en fazla128 terim/~16KiB normal sayfa,
  proses genelinde1MiB decoded-page cache kullanılır. Normal full BTreeMap
  bırakılır. Numeric range ve tüm terimleri dolaşan merge cursor'ları da
  sayfalı yolu kullanır. Açılış CRC/grammar/order/offset/count kontrolleri sürer.
- Her run'daki bütün terimler için genel64KiB Bloom filtresi oluşturulur.
  False-positive yalnız ek lookup üretir; sonuç doğrulamasını atlatmaz.
  AND'de zorunlu gram yoksa diğer sayfalar okunmadan0 döner. Sorgu sözcüklerine
  özel tablo/optimizasyon yoktur. FDB source guard'ı search'ten önce hâlâ çalışır.
- Staged index'in gram heap hesabı tüm sözlüğü her belgede taramak yerine
  gram ekleme/posting capacity değişiminde güncellenir. Diğer metadata ve
  anahtarların admission hesabı ayrıca yapılır.

## Sınırlar

Bu bir toplam heap/OS working-set limiti değildir. Pool sayaçları allocation
ölçümü değildir;16MiB rezervasyon,16MiB gerçek tahsis demek değildir. OS RAM
sonuçları ayrı proses örneklemesinden gelir. BTreeMap admission hesabı yaklaşık
byte tahminleri kullanır. Caller'ın önceden oluşturduğu Vec/tekil Document,
query output, allocator/stack/OS file cache ve bütün sparse-root kapasitesi
bu pool sayacıyla birebir ölçülmez.

Yeni API'nin encoded entry limiti256KiB, key allowance1MiB ve tahmini index
allowance4MiB'dır; max10k mutation ve mevcut64MiB frame sınırı da geçerlidir.
Bu profile sığmayan input WAL yazılmadan reddedilir. Normal API'nin mevcut
belge/işlem limitleri değiştirilmedi. Büyük legacy scalar terimler normal
sayfadan büyük, uncached tekil lookup isteyebilir; genel sabit-RAM iddiası yoktur.
Normal API ile kalmış WAL tail'ının açılış/replay belleği bu yazma admission'ına
otomatik girmez. Streamed yola geçişte mevcut delta önce flush edilir.

## İlk prototip ve düzeltmeler

`bounded-memory-10k-20261014` üç çift: normal indexing111,33MiB / streamed8,73MiB;
bakım99,94 /7,29MiB; idle21,48 /1,45MiB; OS peak WS115,46 /12,63MiB.
Toplam31,133 /38,147s; üç çiftte de yavaş. Retained disk1513,74 /1521,04MB.
Bunlar son düzeltmenin sonuçları değildir; eski loglar korunur.

İlk38-case matriste10 düşük/28 yüksek p95 vardı. Sparse lookup'un JSON sayfa
decode maliyeti ve gereksiz negative lookup'lar görüldü; Bloom/last fence,
AND early-out ve cache admission hesabı iyileştirildi. Yazmada gram bütçesi
taraması kaldırıldı. Tam suite paralel koşusunda cold cache reservation'ı
mutex altında beklediği için deadlock yakalandı; fixed baseline reservation
ile kaldırıldı. Takılan11 disk testi ve mixed normal/streamed writer testi
sonrasında geçti. İlk bozuk sürüm ready/default yapılmadı.

## Güncel doğrulama ve ölçüm

Son corrected artifact'ta exact-feature ve all-features all-targets testler,
all-features Clippy -D warnings geçti. Cut / single-byte corruption, duplicate /
fallible input / guard rejection, update/delete / field-range-link sorguları,
normal commit interop, süreç kill intake/WAL/ack ve mixed writers kontrol edildi.
Staged WAL mevcut encoder'la byte-eşit; posting spool JSON bytes uyumludur.
0.1, native source/recovery, bad-source/good-cache testleri tam suite'te korunur.

Mevcut corpus `data/rkyv-corpus-20261009/json.payload-bench` ilk10k:
264.842.858 text byte,2.057–51.197B/doc, sekiz kitabın çeşitli/örtüşen pasajları.
Yeni corpus/download/prepare yok. Yalnız iki sahiplik kontrollü sabit DB slotu
kullanılır; sonuç DB kopyaları tutulmaz.20 işlem×500 belge, synced WAL;
input CRC/JSON decode, commit ve40 edge full-document kontrolü ingest timer
içinde; kalan flush/merge ayrı, toplamda birlikte değerlendirilir.

Prefix `benchmarks/bounded-memory-corrected-10k-20261014`; üç dönüşümlü
normal/aday process çifti. Her koşu sonrası ayrı prosesle bütün10k belge /
version +4query total/ilk50 oracle; her aday eski normal reader ile ayrıca
doğrulanır. Idle ayrı strict-open+5s, search warmup yok.20ms private/WS,
100ms phase poll; kısa tepeler kaçabilir. Cache OS cache, eviction yok;
cold cache ve eşzamanlı bakım latency benchmark'ı değildir. Unit mixed-writer
testi performans/latency kanıtı sayılmaz.

## Corrected sürümün ölçülen sonuçları

**Bellek hedefinin bu10k iş yükündeki prototipi çalıştı; hızdan ödün vermeyen
genel çözüm kapısı geçmedi. Default'a alınması reddedildi.** Normal API ve
protokol mevcut yolu kullanır; düşük bellek değerleri explicit streamed API'ye aittir.

| Ölçüm / medyan | Normal FDB,3 paired proses | Streamed FDB,3 paired proses | SQLite,3 ayrı referans proses |
|---|---:|---:|---:|
| İndeksleme observed private peak | 111,13MiB | 9,13MiB | 8,40MiB |
| Son bakım observed private peak | 100,11MiB | 7,94MiB | 7,26MiB |
| Tüm ingest prosesinin OS peak working set'i | 114,73MiB | 12,92MiB | 12,59MiB |
| Boşta private | 21,48MiB | 1,74MiB | 1,23MiB |
| Yazma | 25,591s | 30,590s | 45,322s |
| Son bakım | 1,613s | 0,987s | 1,652s |
| Yazma + son bakım | 27,244s | 31,538s | 46,966s |
| Retained DB dosyaları | 1513,74MB | 1521,04MB | 583,86MB |

FDB pair'lerinde üç kez de indexing/maintenance/idle private ve OS peak WS
düştü; toplam süre üç kez de yükseldi. Toplam medyan yaklaşık%15,76 uzun;
paired süre artışları%20,60 /%15,38 /%14,74. Indexing private yaklaşık%91,78 düşük.
SQLite koşuları FDB çiftlerinden sonra ayrı grup halinde yapıldı; sürelerden
interleaved paired SQLite hız kazancı/garantisi çıkarılmaz. Aynı veri ve
kalıcılık ayarlarıyla gözlenen referans süreleri tabloda verilir. SQLite
field/range/link indeks kapsamı FDB'ye eşdeğer değildir.

Her metric'in medyanı ayrı alınır; ingest medyanı+maintenance medyanını toplayıp
combined medyanı diye kullanmayın. MiB=1048576B, diskMB=1000000B. WS/private,
rezervasyon veya resident_text_bytes sayacı değildir. SQLite cache2MiB,mmap0,
tempFILE,WAL/FULL, sürüm3.53.2; JSON payload+folded sütun+external FTS5 trigram
detail=none,columnsize=0.Üç fresh ingest+idle;10k full-document/version oracle
her koşuda ayrı çalıştı, son SQL full/FTS integrity geçti.

Son streamed state10k belge,sequence20,generation51,5run,delta0;
source267.900.709B,index353.688.186B,WAL45B. Index sayacı records04 sidecar'larını
içerir; ana IDX/source codec'i değişmedi. Normal index353.242.581B.
Canlı index/source/WAL toplamı **621.588.895B**;
retained+7.297.108B (%0,482), canlı+445.605B. Eski nesil temizliği yapılmadı.
Normal shadow/delta watermark path'i streamed metodu dışında değişmedi.
`source_bytes` WAL45B'ı zaten içerir; tekrar eklenmez. Normal canlı toplam
621.143.290B; publication marker ayrıca514B(normal)/516B(aday) ve en son
manifest dahil canlı toplam621.143.804B /621.589.411B'dır.

Working pool son snapshot: limit16MiB,baseline2MiB,in_use2MiB,peak16MiB;
baseline cache/scratch kapasitesi olarak ayrılır,16MiB allocation/RAM iddiası
değildir. Write pool256KiB/peak64KiB,posting pool2MiB/peak1MiB; nested reuse
izinleri gerçekten o boyutta allocation yapıldığı anlamına gelmez.
`IndexMemoryLayout` ayrıca sparse term roots+BLOOM capacity'sini ayrı adla raporlar.

## Son arama kapıları

`bounded-memory-corrected-matrix-10k-20261014`: aynı immutable10k DB'de3 paired
proses/variant,38 query/page; normal kontrol ve streamed-feature reader aynı
source/IDX'yi okudu. Record-read API iki tarafta aynı; metadata/liveness /
normalization/CRC/UTF-8/full50 hydration API timer içinde. Primary query
20 sample/process (60 pooled), sayfa5/process (15 pooled), nearest-rank p95.
Independent full-doc/page/version oracle geçti. Bütün FDB/cache ve SQLite
dosya SHA'ları önce/sonra aynıydı. **17 p95 düşük /21 yüksek.**

| Query / offset | Normal p95ms | Aday p95ms | Adayın hızlı olduğu çift |
|---|---:|---:|---:|
| zzxqnever /0 | 0,1481 | 0,9140 | 0/3 |
| quixote /57 | 2,9713 | 5,6716 | 0/3 |
| zeuglodon /0 | 2,6577 | 4,7531 | 0/3 |
| quixote whale /0 | 4,0747 | 7,2134 | 0/3 |

Bloom ve cache düzeltmeleri ilk10/28 sonucunu iyileştirdi; bu21 artış speed
gate'ini geçmez. Daha küçük pinned dictionary yerine page lookup CPU/I/O
maliyeti geldi. Her alt fazın kesin CPU ataması alınmadı; bu tablo combined
API süresidir. Generic tüm-gram membership preflight, packed page decoding
ve hot dictionary erişimi hâlâ optimizasyon adaylarıdır; uygulanmış sonuç değildir.

`bounded-memory-delta-20261014`: mevcut corpus ilk1000; reverse ingest40
delete/reinsert+reopen; aynı6 query×3 logical state,3 paired process/variant,
1warm+10sample/process (30 pooled/case), full total/ilk50 document/version oracle.
Normal kontrol128MiB/2000key watermark ile RAM delta'sında; streamed aday her
commit'te segmente yayımlıyor. State isimleri eski harness kimliğini korur;
adayda gerçek delta_keys0'dır. Bu fiziksel yerleşim farkı tasarımın maliyetidir.
Oracle1000 belgeleri RAM'de tutar; bu alt testte proses RAM ölçülmedi.

**0 p95 düşük /18 yüksek**; bu yeni sistematik gerilemeler önceki kullanıcı
dalgalanma kabulüyle kapatılmaz. İlk state'te `the`0,8500→21,6415ms,
`quixote`0,0406→0,7974ms,`he`0,6129→22,9671ms. Reopen sonrası da fark sürüyor.
Tam folded metin ve resident belgeyi kaldırmanın küçük aktif-data aramasında
bedeli var; immutable10k benchmark'ının sonucu bunu gizleyemez.

SQLite read reference tek proses/dört query/30 sample, Full page+version oracle;
FDB/SQL38-case paired karşılaştırması değildir. SQL reference timing'i ve
RAM ingest ölçümü ayrı proseslerdir. SQL sonuçları
`bounded-memory-sqlite-10k-20261014-summary.json` ve matrix reference logunda.

## Son durum

Motorun normal yoluna geçiş yapılmadı. Streamed profile code feature altında
korunur; default'a alma hız şartı nedeniyle reddedildi. Normal fixed FDB slotu
aynı ilk10k ile yeniden hazırlandı, full10k+4query oracle ve38-case correctness
kontrol edildi; SQLite slotu da10k olarak bırakıldı. Bu son correctness kontrolü
performance_not_measured'dır. Yeni corpus veya per-run DB kopyası yok.

Exact-feature ve all-features all-targets test/Clippy logları
`bounded-memory-corrected-{tests-feature,tests-all,clippy-all}.log`;
default/fmt/final normal-build logları `bounded-memory-final-*` ve
`bounded-memory-normal-build.log`. Cut/corruption/kill/compatibility kontrolleri
başarılıdır. Mevcut fixture kaynakları ve dış projeler değiştirilmedi.

[OPEN-WORK](OPEN-WORK.md): normal/default RAM modeli, yazma/bakım hızı,
yeni sayfalı arama farkları, disk/eski nesiller ve concurrent yük açık.
Önceki grouped sorgu dalgalanması kullanıcı kararıyla kapalıdır; yeni prototype
regresyonlarını otomatik kapatmaz.
