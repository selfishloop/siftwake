# FDB mimarisi — 0.3.1

2026-10-18 aynı executable/süreçte off0/off1/on kontrolü tamamlandı.
Normal 38-case 120 örnek/koşulda p95 plasebo da kararsız; genel geçiş yok.
İki normal query-history düzeninde uzun negatif off0'a karşı 4/4 süreçte
p95 artışı verdi; payload read/CRC ve ek heap işi yoktur. Quixote ikinci
read/CRC azalması korunur. Ayrı requested-heap profilinde canlı +2.464B,
52→66 sonuç Vec kapasitesi ×176B ile eşleşti; OS private/WS farkını tek
başına açıklamaz. CPU/wall kontrolünde büyük bekleme gözlendi; process
AboveNormal öncelik bunu bütünüyle çözmedi. Runtime/API/feature/format
kaynakları başlangıca döndü. Ortak boş/negatif yol korunmalı, pozitif
sonuç kapasitesi ayrı düzeltilip ölçülmeli; henüz uygulanmış çözüm değil.
[Kanıtlar ve sınırlar](REUSE-CONTROLS-10K-20261018.md).

2026-10-18 checked-page prototipinde doğrulanmış binary contains slice'ları
global Key sırasıyla en fazla50 kayıt/2MiB encoded byte tutan query-local
private Box alanında denenmiştir. Dönen50 kaydın ikinci read/CRC'si kalkar;
schema/UTF-8 decode sırasında tekrar kontrol edilir. Source/key/version
doğrulaması ve fresh read CRC'si sürer. Offset0/limit<=50/uzun terimli
sorgulara dar yol, oversized/bütçe/deep sayfaya eski fallback vardır.
Const CAPTURE=false eski pair-slot/ordinal/exact allocation yolunu koruyan
ayrı kod üretir. Dar quixote p95-%11,11 (4/4 süreç) fakat geniş15 düşük/
23 yüksek p95, query private+64KiB/WS+170KiB: varsayılan geçiş reddedildi;
bu cache runtime tasarımına eklenmeyecek. Kod/beş yeni test patch'tedir.
Format/feature/dependency,0.1 fixture ve sabit10k verisi değişmez.
[Ölçüm ve karar](CHECKED-PAGE-10K-20261018.md).

2026-10-18 query-only CRC/UTF-8 interleave deneyi reddedildi.8KiB geçerli
UTF-8 parçaları sırayla IEEE CRC durumuna ekleniyordu; tam schema ve
identity korunuyordu. Normal4-case kontrolünde zx4/4 ve holmes3/4 süreçte
daha yüksek p95 verdi. segment/payload kaynakları cb2c19e'a byte-eşit döndü;
unsafe içeren private helper/testler yalnız arşiv patch'indedir. Yeni
format/feature/bağımlılık yok. [Ölçüm ve sınırlar](VALIDATION-CHUNK-10K-20261018.md).

2026-10-17 ikinci payload gruplama prototipi de reddedildi. Kuyruk ve
posting/kesişim döngüsü ayrı inline edilmeyen helper'a alındı; iki erken
dispatch hariç eski fonksiyonlar byte-eşitti. Üç sorguluk400 örnek/variant
kontrolünde the/200 artışı tekrarlanmadı, fakat holmes p95 kazancı da
tekrarlanmadı. Runtime ve testler f6d18dd'a döndü; yeni format/feature/
bağımlılık yok. Genel case-index seçicisi benchmark aracı olarak kaldı;
motor sorgu sözcüğüne göre özel yol seçmez. [Kanıt ve sınırlar](PAYLOAD-ISOLATED-10K-20261017.md).

2026-10-17 bilinen aday payload'larını 64 slot/128KiB, en fazla4KiB gap ve
%5 ek veri bütçesiyle gruplama deneyi reddedildi. Okuma çağrıları azalırken
CRC/schema/UTF-8/key/version ve tarama işi korundu; normal kelimelerde dar
kazanç fakat hızlı sayfalarda açıklanmamış p95 artışı vardı. Genel posting
döngüsüne eklenen kuyruk/branch varsayılanda tutulmadı; segment ve testleri
6eb3a89'a byte-eşit döndü. Mevcut geniş-seed read-ahead ve hydration yolu
aynıdır. Patch, yerleşim analizi, normal A/B, aynı-SHA A/A ve RAM sınırları
[deney raporunda](PAYLOAD-BATCH-10K-20261017.md). Bu tur format değişmez.

2026-10-17 grouped page hydration: seçilen sıralı slotların sayısal
metadata'sı ve key karşılaştırması aynı query-local doğrulanmış yapraktan
okunur. Her decode öncesi yeniden sahipli Key kurma ve metadata lookup
tekrarı kaldırılır. Payload okuma penceresi/CRC/schema/UTF-8/version ve
source lease değişmez; büyük belge fallback'i sürer. Yeni cache/format/
feature/bağımlılık yok; mevcut leaf sınırı 256 KiB ve cursor sorgu ömürlü.
Aynı 10k'da 200 belgeli sayfa normal p95 -%11,04, 4/4 süreç ve16/16 blok;
query/get peak private/WS medyanı artmadı. Genel p95 ve payload I/O açık.
[Ölçümler, testler ve sınırlar](SEARCH-HYDRATION-10K-20261017.md).

2026-10-17 bounded-engine contains: yalnız1–2 küçük ASCII harfinden oluşan
ve Unicode/Türkçe fold'dan üretilemeyen harfleri kullanan sorgu parçaları,
özgün metindeki bütün ASCII case yazımlarıyla kesin aranır. `c/g/i/k/o/s/u`
harfleri veya başka terimler varsa önceki canonical fold/chunk yolu sürer.
Tüm Unicode scalar'ların ASCII katkıları ve hiçbir scalar'ın silinmediği
test edilir; bu özellikler yokluk kanıtını da geçerli kılar. Payload
CRC/schema/UTF-8/key/version önceden doğrulanır; yeni cache/heap tamponu,
format, feature veya bağımlılık yok. No-default reference aynı kalır.
Geniş arama, ayrı profil ve bellek sınırları
[aynı-veri arama raporundadır](SEARCH-STAGES-10K-20261017.md).

2026-10-17 eski nesil temizliği: strict açılış ve başarılı yayın sonrası,
güncel ready yeniden doğrulanıp eski ready'ler, ardından artık referanssız
standart kaynak/cache dosyaları kaldırılır. DB kökü kilit alınmadan önce
canonicalize edilir. Aktif WAL, canlı Run, Weak ile izlenen retired okuyucu
ve Job.output_id korunur. Root'tan büyük/eşit orphan ID bir sonraki yayına
kadar saklanır; allocator ID tekrar kullanmaz. Public `reclaim_obsolete`
önce full integrity yapar; otomatik hatalar yazma sonucunu zehirlemez.
Windows publication write-through rename, Unix directory sync kullanır;
kalıcı biçimler aynı. Süreç sonlandırma güç kesintisi testi değildir.
Mevcut10k'da105 dosya/622,9MB kaldırıldı,973→350MB; canlı17 core dosya
byte-eşit, eski/yeni okuyucu ve10k oracle geçti. Yazma+bakım2 çiftte+%1,34,
open serileri tutarsız; genel performans/RAM kapısı kapanmaz.
[Davranış, güvenlik ve ölçüm](RECLAIM-10K-20261017.md).

2026-10-17 kullanıcı onayıyla SEG05 flag1 varsayılan native Run yazıcısıdır.
Flush/delta posting reuse, merge ve eksik/bozuk cache yeniden indekslemesi,
doğrulanmış FDBLOG04 entry adreslerini doğrudan dictionary'ye taşır. Önce
kopyalı SEG04 üretip dönüştürmez. Reuse sırasında tam belge nesnesi kurulmaz;
tam yeniden indeksleme terim çıkarmak için tek belgeyi çözer. Kayıt/posting
spool'ları checksum'lı ve akışlıdır; nihai directory ikilidir. Tam kaynak CRC
geçmeden builder yayımlanmaz. Sağlam eski cache'ler otomatik yazılmaz;
Legacy kaynaklar ve no-default temel profil SEG04 yolunu korur. WAL/manifest/
kaynak/payload biçimleri aynı. [Güncel geçiş ve sınırlar](DEFAULT-SEG05-10K-20261017.md).

Önceki prototip ve ölçüm geçmişi:
2026-10-17 isteğe bağlı SEG05 cache prototipi: typed binary directory ve
FDBLOG04 source entry referansları; yeni kaynak/WAL/manifest/payload biçimi
yok. Normal builder/onarım/merge SEG04 kalır. CRC/schema/identity ve source
lease sürer; kaynak referansında5/42B entry/frame aralıkları mevcut bounded
read-ahead'e katılır. Tam dictionary taraması sürer, resident adres/belge
tablosu eklenmez. Normal4 çiftte open260→170ms; canlı idx353,7→81,9MB,
private/WS artışı yok.38-case32/5/1 ve yoğun8 sayfa5/3: genel/default geçiş
kapısı açık.133 sabit-slot dosyası deney sonunda eski okuyucuyla byte-eşit
SEG04'e döndü;0.1 fixture aynı. [Wire/API/kurtarma](SEG05-CACHE.md),
[ölçüm ve açık sınırlar](COMPACT-INDEX-10K-20261017.md).

2026-10-17 indeks CRC okuması: `segment::checksum` soğuk/ayrı fonksiyonda
tam prefix'i en fazla1MiB exact-read tamponuyla doğrular. Küçük indeks için
tampon prefix boyutuyla sınırlıdır; EOF bozuk indeks sayılır ve kaynak
rebuild yolu korunur. Header/footer/binding, tam CRC, JSON directory/slot/
sıra ve kaynak doğrulaması aynı. Kaynak/WAL `format::prefix_crc` yolu
değişmedi; yeni format/feature/bağımlılık yok. İki normal dört-çift seride
ilk kullanım -%4,20/-%6,18,8/8 hızlı. Büyük sözlük COW/reuse denemesi yoğun
aramada%5–10 artışla reddedildi; üretimde yok. Geniş arama, Vec RAM, disk,
concurrent/cold işleri açık. [Son kanıt/sınırlar](INDEX-OPEN-10K-20261017.md).

2026-10-17 normal strict açılış: yayınlanmış FDBLOG04 Run kaynaklarında
ayrı prefix CRC ve sahipli belge replay'i yerine tek sınırlı doğrulama geçişi.
Frame CRC'sinden birleştirilen kaynak CRC'si ready checksum'ıyla eşleşir;
frame/schema/UTF-8/key/link/version/sınır kontrolleri ve source lease korunur.
Kaynak tamamen doğrulanmadan cache kabul edilmez. Sağlam cache replay'i
atlatır; eksik/bozuk cache eski builder yoluyla yeniden oluşturulur. Yeni Run,
WAL ve JSON/0.1 yolları aynı; yeni format/feature/bağımlılık yok. Dört normal
release çiftinde430→270ms strict open, pending0; bütün arama/RAM/disk
kapıları kapanmış sayılmaz. [Ölçüm ve doğrulama](STRICT-OPEN-10K-20261017.md).

2026-10-16 varsayılan profil: `bounded-engine`, önceki ölçülmüş
builder-budget birleşiminin aynısını varsayılan derlemeye taşır.
Kaynak cfg'leri yeni ada bağlı; `experimental-builder-budget` yalnız
uyumluluk alias'ıdır. Bağımsız rkyv/WAL-delta/shared-documents/sync-control
deneyleri ve diagnostic sayaçlar açılmaz. Normal commit ve strict open
semantiği korunur; akışlı API artık özel derleme bayrağı gerektirmez.
Yeni kalıcı biçim veya bağımlılık yoktur. Önceki tarihli raporlardaki
default-kapalı durumu tarihsel kanıttır; güncel kullanım/sınırlar
[DEFAULT-PROFILE](DEFAULT-PROFILE.md) içindedir.
[Geçiş doğrulaması ve kalan performans sınırları](DEFAULT-PROFILE-VALIDATION-10K-20261016.md).

2026-10-16 kısa bloklu ölçüm: iki worker oracle'ı resident tutar; DB
her blokta tek süreçte açılır/kaynak doğrulanır/ısınır,25 turdan sonra
kapanıp kilit bırakılır. Kilit veya motor API'si değişmedi. Dört süreç
çifti×8 blok, aynı CPU, diagnostic kapalı. İki farklı-sürüm serisinde8/8
yeni p95 düşük; same-exe kontrolünde the tek-sonuç +%6,44/4 çift yüksek.
Bu yeni benchmark/codegen/cache düzeni önceki kesintisiz oturumun tüm
nedenlerini kapatmaz; yeni motor optimizasyonu/default terfisi yok.
[Kanıt/sınırlar](INTERLEAVED-SEARCH-10K-20261016.md).

2026-10-16 arama teşhisi: yalnız benchmark araçlarına profile-pages,
çağıran-thread çevrim sayacı ve isteğe bağlı CPU affinity eklendi.
Sekiz sayfada1.920 instrumented doğrulamada tüm iş sayıları aynı, body
gram çıkarımı0. Normal aynı-CPU A/A'da ishmael +%4,23 p95; A/B +%3,45,
quixote +%9,64. Ortam kararsızlığı kanıtlı; kod üretimi/yerleşim etkisi
ayrılmadı ve genel/default kapısı açık. Yeni format/bağımlılık/feature yok,
motorun arama davranışı aynı. [Kanıt/sınırlar](SEARCH-ATTRIBUTION-10K-20261016.md).

2026-10-16 body gram üretimi: builder-budget içinde BodyCache'in en az1024B
folded gövdesinde üç21-bit scalar geçici u64 anahtarına paketlenir.1024 giriş/
8KiB stack tablosu tam anahtar eşitliğini kanıtlar; çakışma ağaç probe'una
döner. Tekrarsız set BTreeSet<u64>, çıktı aynı BTreeSet<String>. Whitespace
pencereyi keser. Query grams kaynağı aynı; çıkarıcı inline(never), yalnız write
body yolunda. Yeni kalıcı biçim/feature yok. Son dört paired10k yazma+bakım
31,908→28,241s (-%11,49),4/4 hızlı;122 core dosya sekiz koşuda aynı.
Private commit +88KiB/WS+28KiB medyan; global RAM/hız kapısı geçmedi/default
kapalı. Read matrix33/5,800 örnekli sayfalar1/7; ishmael tek-sonuç +%4,87,
4/4 yüksek. Bu artışlar kapatılmadı; diğer açık işler raporda.
[Ölçüm/uyumluluk/sınırlar](GRAM-BODY-WRITES-10K-20261016.md).

2026-10-16 builder-budget contains: payload CRC/UTF-8/schema/key/version
kontrolünden sonra ASCII parçanın özgün body'deki lowercase/Titlecase/UPPERCASE
yazımı pozitif kanıt olabilir; eksiklik/mixed-case/Unicode eski fold yolundadır.
Body ilk64KiB/UTF-8 sınırı,128-byte stack yazım tamponu, run/query'de8 denemede
en az6 başarı yoksa spekülasyonu durdurma. Boyutlar semantik limit değildir;
ek büyük cache/feature/format yok.56 adayda815→0 body fold/scan, read/CRC56
kalır. Aynı10k/4 pair/800 örnekte dar sayfalar%4–15 düşük, dördü4/4 hızlı;
geniş matrix21/17, genel/default kapısı geçmedi. Diğer açık işler korunur.
[Ölçüm/kanıt/sınırlar](RAW-TEXT-PROOF-10K-20261016.md).

2026-10-15 ölçüm kontrolü: dar posting'lerde aynı256KiB adaptif pencereyi
açma denemesi hedefte payload request56→56, window hit0→0; geniş38-case
9 düşük/29 yüksek. Runtime bütünüyle geri alındı; önceki source lease kalır.
Yeni format/feature/bağımlılık yok. Test runner `-Candidate`/çift `-Pairs`
parametreleriyle aynı executable A/A ve dengeli varsayılan4 çift sağlar;
her sıra/hash/örnek sayısı raporlanır. Özet pooled p95 yanında çift farkı ve
medyanını verir, eski üç-çift logları okur. A/A dalgalanması motor regresyonu
diye sunulmaz; önceki açık işleri kapatmaz.
[Ölçüm ve reddedilme kanıtı](BENCHMARK-CONTROL-10K-20261015.md).

2026-10-15 source lease: builder-budget içinde Windows/plain-path Run dosyası
CRC/schema doğrulamasından önce FILE_SHARE_READ ile açılır. Aynı handle
Arc<DiskIndex> ömrünce kalır; mevcut writer açılışı reddettirir ve son sahip
kapanana kadar write/truncate/delete/rename engellenir. Aktif WAL/Legacy,
reparse/junction ve Windows dışı yollar önceki metadata guard'ını kullanır.
Covering source path guard'ı yalnız bu OS güvencesi varsa atlanır; index
handle guard ve source authorization/ilk tam doğrulama sürer. Yeni format,
feature, bağımlılık veya unsafe yok. Aynı10k/üç çift/600 örnek the /0/1 p95
0,8123→0,2459ms,3/3 hızlı. Geniş matrix14/38 düşük; genel kapı geçmedi,
diğer artışlar açık/default kapalı. [Ölçüm/test/sınırlar](SOURCE-LEASE-10K-20261015.md).

2026-10-15 küçük sayfa teşhisi: diagnostic-timing içinde covering guard,
source path metadata ve index handle metadata span'leri ayrıldı; normal
build'de sayaç yok, mevcut kontroller aynı. Aynı10k/3 proses ×3 warm örnekte
limit1 toplam0,3466ms/guard0,2778ms; fold/scan0. Posting full-prefix ve
GetFileAttributesExW varyantları yoğun sayfa kapısını geçmedi ve kaldırıldı;
Windows dependency/feature eklenmedi, değişmeyen Unicode-aralık normalizer'ı
korunur. Harness --pages-only ile200 örnek/proses ve loglarda limit sağlar.
Bu teşhis aşamasında sahiplik tasarımı açıktı; sonraki uygulama yukarıda.
[Deneyler ve kanıt](PAGE-GUARD-10K-20261015.md).

2026-10-15 builder-budget query normalizer'ı değişmeyen Unicode işaretlerinde
kopyalama aralığını korur; yalnız Türkçe/Unicode fold sonucu gerçekten değişen
scalar'da aralığı böler. ASCII lowercase tüm aralıkta, byte sonucu öncekiyle
aynı. Ek heap buffer/cache yok;32-bayt prefix ve iki-tampon chunk yolu,
CRC/schema/identity/source izinleri, default ve kalıcı format aynı. Aynı10k/üç
çift matrix35 düşük/3 yüksek; zx/the zx/quixote whale3/3 hızlı. İndeksleme
private9,13→9,18MiB, bakım+0,94MiB/WS+0,375MiB ve tek-sonuç sayfasında
0/3 hızlanma nedeniyle genel RAM/hız kapısı geçmedi; default'a geçiş reddedildi.
[Kanıt ve sınırlar](CHANGED-RUNS-10K-20261015.md).

2026-10-15 doğrudan overlap tamponuna normalleştirme ve tek residual parça
varyantı denendi; aynı10k üçer pair32/6 ve21/17. İki parçalı arama gerilemesi
iki denemede0/3 hızlı kaldı; iki runtime değişikliği geri alındı. fold_into ve
iki-tampon chunk döngüsü önceki halindedir. Yeni format/feature/heap cache yok.
[Reddedilen deney ve sınırlar](CHUNK-COPY-REJECTED-10K-20261015.md).

2026-10-14 builder-budget sorgu normalizer'ında ASCII prefix araması dört
8-bayt word'ün OR'u ile32 baytta bir kontrol edilir; mixed blok ve tail mevcut
8-bayt exact-position yolunu kullanır. Güvenli array loads; Türkçe/Unicode fold
sonucu, buffer/cache yapısı, CRC/identity/source guards ve kalıcı format aynı.
Default8-bayt algoritması korunur. Aynı10k/üç pair matrix33/5; zx/the zx3/3
hızlı. Tek RAM/write teyidi indexing9,14MiB/WS13,34MiB,bakım+0,66MiB ve
toplam+%3,41 nedeniyle genel gate geçmedi.
[Kanıt ve sınırlar](WIDE-FOLD-10K-20261014.md).

2026-10-14 builder-budget profilinde bütün canlı kayıtları kaplayan gramdan
iki Unicode karakterlik alt-parça kanıtı da türetilir. Parse sırasında en fazla
64 gram/16 KiB owned kapasite/run; source authorize/fingerprint guards korunur.
Scope ayrılır, kanıt yoksa mevcut payload taraması sürer: izole kısa metin
gram üretmeyebilir, özetten yokluk kanıtı çıkarılmaz. Reversed directory order
geçerlidir ve kısa-parça taramasına döner. Format/default aynı. Aynı 10k/üç
paired `he`132→1,81ms; indexing private9,09MiB/disk aynı/WS+0,30MiB.
Genel matrix15/23 nedeniyle default kapısı geçmedi.
[Kanıt ve sınırlar](SHORT-COVERAGE-10K-20261014.md).

2026-10-14 builder-budget profilinde complete trigram posting'i kesin contains
kanıtı olarak kullanılabilir. Yalnız source kontrolünden sonra authorize edilir;
source/index length+mtime bağları tekrar kontrol edilir, bilgi yoksa payload
doğrulaması sürer. İki karakter/uzun parça için payload taraması aynı, mixed
AND'de yalnız kesin parçalar çıkarılır. Dönen belge CRC/schema/key/version
hydrate edilir. Kaynak/.idx/payload biçimi değişmez, yeni text/result cache yok.
Genel count/prefix yolunu kullanır; offset5000/9950 ölçümünde3/3 hızlı.
Bu tüm aramalar veya ilk pending-source latency için hız garantisi değildir.
[Ölçüm/korunan kontroller/sınırlar](COVERED-TEXT-10K-20261014.md).

2026-10-14 düşük RAM profilinde binary-query record cursor ve borrowed key
karşılaştırması eklendi. Genel tüm-gram Bloom/fence preflight ve mevcut1MiB
directory cache içinde64KiB hot posting konumu/960KiB page payı kullanılır.
Payload CRC/validation/key/version kontrolleri sürer; source/payload formatı
ve default seçimi aynı. RAM'in bu10k profilde kabulü genel heap garantisi veya
hız kapısının kapanması değildir. [Ölçüm/sınırlar](BOUNDED-SPEED-10K-20261014.md).

2026-10-14 opt-in `experimental-builder-budget` altında `commit_streamed`:
tek native staging frame/synced WAL, index-only bitmap delta, işlem sonunda
source/manifest yayını ve resident delta0. Ortak16MiB working allowance,
14MiB intake+builder ve2MiB earmarked cache/scratch; RSS ölçümü değildir.
Posting directory spool ve mevcut IDX byte aralıklarında sparse/page-cache
term dictionary;64KiB/run genel Bloom, AND early-out. Format aynı/default kapalı.
Ölçüm indexing111→9,13MiB; toplam27,24→31,54s ve aktif-data arama gerilemesi
nedeniyle default kapısı reddedildi. [Kanıt/sınırlar](BOUNDED-MEMORY-10K-20261014.md).

2026-10-13 existing experimental-wal-delta encode_located ortak tek-frame
encode kullanır; ikinci payload→frame kopyası kaldırılır. Pending flush
öncesi limit kontrolü, sonrası encode; row offsets/checksum aynı. RAM'de
tam belge yerine WAL referansı ölçüldü fakat delta search hız kapısı geçmedi;
default kapalı. [RAM kazanımı/hız kaybı](COPY-REDUCTION-10K-20261013.md).

2026-10-13 delta gram dictionary scope→gram→posting olarak RAM'de gruplanır;
scope bir kez sahiplenilir, mevcut gram update borrowed lookup kullanır.
Kalıcı Term::Gram sırası/codeci ve document/folded body temsili aynı.
[Üç write/RAM pair ve unflushed delta arama ölçümü](DELTA-DICTIONARY-10K-20261013.md).

2026-10-13 commit frame limit validation pending flush öncesinde korunur;
non-WAL-delta encode pending flush sonrası, başarılı append/sync sonrası
encoded Vec drop delta apply öncesinde yapılır. Aynı codec/durability/source
yolu; yeni feature/format yok. [Indexing private124→112MiB,3/3 pair hızlı](RAM-LIFETIME-10K-20261013.md).

2026-10-13 ASCII prefix araması güvenli8-byte load ile, query token fold'u
aynı sözleşmeli fold_into ile hızlandırıldı. Experimental payload window,
tamamlanan pencerenin en az3/4'ü kullanılmazsa sorgunun kalanında exact read'e
döner; broad seed yarı-live eşiği korunur. CRC/schema/source guards ve
kalıcı biçimler aynı. [11 önceki case düşüşü,36/2 genel matris](REGRESSION-READS-10K-20261013.md).

2026-10-13 experimental-grouped-read altında query-local256 KiB native payload
window: broad seed ve fiziksel adjacency ile etkinleşir; record CRC/schema /
identity ve VFY block/source guard'ları korunur. Default compile dışı, yeni
feature/format yok. [Geniş query%20–22 kazanç fakat11 matrix regresyonu](WINDOW-READS-10K-20261013.md).

2026-10-13 OFF1 string64+ byte için pinned simdutf80.1.5 safe basic API;
kısa/invalid girdide std UTF-8 ile aynı validity ve detailed error korunur.
CRC/offset/typed grammar/doc limits/key/version aynı. Frame reuse/pre-plan
prototipleri regresyon nedeniyle geri alındı; source replay allocation aynı.
[İlk search208→173 ms, warm query%18–27 ve strict/memory sınırları](UTF8-READS-10K-20261013.md).

2026-10-13 ortak IEEE CRC:128+ byte girdide pinned crc32fast1.5.0 runtime CPU
dispatch, küçük başlıklarda mevcut table yolu. Raw state complement sözleşmesi,
crc_combine ve bütün byte/format/checksum değerleri aynı. Strict/deferred,
WAL/source/index/cache kontrolleri korunur; opt-in API kararı değişmez.
[İlk search381→229 ms, strict962→582 ms ve dört warm-query ölçümü](CRC-READS-10K-20261013.md).

2026-10-13 deferred native source doğrulaması: TXN2 frame'ları ortak framing /
whole-source CRC kontrolünden geçer; OFF1 borrowed görünümünde owned belge
üretmeden validate_doc ile aynı key/text/field/link/scope/size limitleri ve
version kontrol edilir. Strict ve experimental-streaming-open replay aynı;
source verified işareti ancak tam kaynak/count/sequence kontrolünden sonra.
[Tekrarlı ilk arama ölçümü](BORROWED-SOURCE-10K-20261013.md).

2026-10-13 opt-in `open_record_reads` + explicit `prepare_record_reads`: REF01
slot→source offset/len/CRC tablosu source end/CRC/sequence veSEG04 root'a bağlı;
SVFY actual source block CRC'si manifest root'a bağlı. Positive get gerçek
source kaydını kontrol eder; negative/tombstone get, search vewrite tam source
kontrolünü korur. Default yollar, source/WAL/SEG04 biçimi ve0.1 aynı.
[İlk get107→0,20 ms; warm-search regresyonları ve sınırlar](SOURCE-RECORD-READS-10K-20261013.md).

2026-10-12 VFY01 derived block checksum cache: explicit `prepare_open_checks`
tam integrity sonrası küçük CRC table yazar; `open_deferred` header/directory
ve touched payload/posting block'larını256 KiB CRC ile kontrol eder. Table CRC,
SEG04 prefix root ve binding şart; cache yok/bozuksa full fallback. Source
kontrolleri aynı; default strict veSEG04/OFF1 değişmedi. [298→60 ms open,
first-use maliyeti ve compatibility testleri](BLOCK-OPEN-10K-20261012.md).

2026-10-12 opt-in `open_deferred`: index CRC/grammar ve active WAL açılışta,
immutable kaynak doğrulaması segmentin ilk kullanımında. Get lookup öncesi,
search ve publication/write öncesi source kontrolleri şart. `check_integrity`
bütün kaynakları ve WAL'i yeniden kontrol eder; stats pending kaynaklarda
provisional. Default strict aynı; format değişmedi. [319 ms open, ilk-use
maliyeti ve güvence sınırları](DEFERRED-OPEN-10K-20261012.md).

2026-10-12 normal immutable open: replay sırasında doğrulanan frame CRC'leri
header/footer ile birleştirilerek manifestteki tam kaynak CRC kontrol edilir.
Ayrı source prefix taraması kaldırıldı; bütünlük karşılaştırması publication'dan
önce biter. Legacy unknown ve streaming-open önceki taramayı korur.
[Beş çift:946→747 ms; bozulma/kill testleri](OPEN-SOURCE-10K-20261012.md).

2026-10-12 aday seçiciliği profili: dar/negatif sorgularda false-positive payload
ve metin normalleştirme maliyeti baskın. Karışık Unicode girdinin ASCII run'ları
topluca dönüştürülür; bütün Unicode scalar eşitliği, CRC ve kesin toplam korunur.
Hedef dört sorguda üç çiftte kazanç; 34/38 p95 düşüş, dört artış raporlandı.
[Profil, SQLite dahil tam matris ve sınırlar](SELECTIVITY-READS-10K-20261012.md).

2026-10-12 geniş okuma matrisi: mevcut 10k DB, 30 query girişi + 8 sayfa,
beşer proses, exact full-document oracle. SQLite comparator p95'inde tüm
senaryolarda FDB hızlı; fakat az/hiç sonuçlu bazı sorgular hâlâ pahalı.
Motor/format değişikliği yok; negatif scan kontrolü küçük koşu farkları gösterdi.
[38 senaryo, get boyutları, p99 sınırları ve kaynak hash'leri](READ-MATRIX-10K-20261012.md).

2026-10-12 normal contains yolu uzun gövdeyi UTF-8 sınırlarında 1 KiB
parçalarla katlar; terim örtüşmesini korur ve tüm AND terimlerini bulunca durur.
Tam payload CRC/biçim/kimlik doğrulaması ve kesin toplam değişmedi.
[Aynı 10k DB: geniş p95 %46–48 düşüş, test ve ölçüm sınırları](CONTAINS-CHUNK-SCAN-10K-20261012.md).

2026-10-12 mevcut 10k DB üzerinde read-only ölçüm: sıcak cache, tam toplam
ve ilk 50 full belge, dar contains p95 FDB10,63 / SQLite19,70 ms.
Geniş sorgular FDB'de ~771–773 ms; delta boş, feature default kararı değişmedi.
[Kapsam, koşu değişkenliği ve hash doğrulaması](CONTENT-READS-10K-20261012.md).

2026-10-12 flush RAM delta dictionary'sini yeniden kullanır; mevcut bitmap
yoluyla birlikte 10k gövde fold/grams geçişi (önce20k), sıfır builder terim üretimi.
Format ve source doğrulaması korunur; default kapalı.
[Aynı belgelerde iki SQLite eşleşmesi](DELTA-FLUSH-10K-20261012.md).

2026-10-12 bağımsız merge posting reuse ölçümü: builder terim üretimi
20k → 10k; 10k belgede son bakım medyanı 24,85 → 4,55 s.
Kaynak/payload doğrulaması sürer, format aynı; default kapalı.
[Corpus, SQLite ve doğrulama sınırları](MERGE-REUSE-10K-20261012.md).

2026-10-12: default TXN2 output tamponu toplam payload uzunluğu ve footer
için önceden ayrılır; frame baytları ve sync davranışı aynı kalır.
[10k tekrar ölçümü ve opt-in bitmap değerlendirmesi](CONTENT-OPTIMIZATION-10K-20261012.md).

## Güncel kalıcı payload: FDBLOG04 / FDBSEG04

2026-10-11 kullanıcı kararıyla yeni veritabanları default olarak native OFF1
ikili belge payload'ı yazar. `FDBLOG04` içindeki `TXN2` binary Batch change
listesini, `FDBSEG04` / `FDBEND04` ise ikili belge payload'larını sürümlendirir.
Manifest ve index directory formatı aynı kalır. rkyv runtime/feature gerekmez;
genel API fields/protokol JSON biçimini kullanabilir. WAL, source/checkpoint,
flush ve merge yeni codec'i kullanır. Contains adayında CRC/grammar/identity
doğrulandıktan sonra title/text ödünç alınır; tüm belge yalnız hidrasyonda çözülür.
Default frame output'u ve query payload tamponu tekrar kopyalamayı azaltır;
normalleştirme ve CRC/UTF-8 tarama maliyeti sürer.

Ek payload-bench dosyası yoktur, ancak `.fdb` ve `.idx` içindeki mevcut payload
kopyaları halen vardır. RAM yerleşimi, directory JSON, source/cache ayrımı ve
index köken doğrulaması korunur. Eski parser/0.1 fixture korunmuştur; eski
veri migration'ı artık ürün gereksinimi değildir.
[Format ayrıntıları ve testler](NATIVE-BINARY-VALIDATION-20261011.md).

[Kalıcı sürümün genel ölçümü](NATIVE-GENERAL-PERFORMANCE-20261011.md): aynı
fold/query/maintenance koduyla default JSON kontrolü, iki 100k corpus ve üçer
eşleşmiş process çifti. Uzun contains 27/27 hızlı, açılış ve uzun belge get
üç çiftte hızlı; kısa yazma/küçük sorgular karışık. Kaynak/index kopyalama düzeni
iki varyantta da korunur; source-offset yerleşimi henüz ölçülmedi. RAM kazancı
gösterilmedi, kısa corpus WS arttı; disk kısa %10,42 / uzun %1,75 küçüldü.

[Yeni SQLite karşılaştırması](NATIVE-SQLITE-GENERAL-20261011.md): native ikili
FDB uzun yaygın/iki karakter/AND sayfalarında21/21 eşleşmiş karşılaştırmada hızlı;
dar sorgu karışık. Kısa yazma+bakımda SQLite üç çiftte hızlı, RAM/diskte küçük.
Bu SQL/schema/cache sözleşmesi genel motor eşdeğerliği değildir; source-offset
düzeni bu ölçümde de değişmedi.

Aşağıdaki JSON/SEG03 ve deneysel performans bölümleri önceki sürümleri ve
ölçüm geçmişini açıklar. Kalıcı payload/codec için bu bölüm güncel sözleşmedir.

## Deneysel commit sync kontrolü

`experimental-wal-sync-control` altında `Database::set_wal_sync(false)` yalnız
commit append sonundaki sync_all'ı atlar. WAL byte/CRC/framing, atomik batch,
checkpoint/source/manifest kalıcılık yolu değişmez. Her yeni/reopened DB true
başlar; CLI opt-in sunmaz. Tekrar true yapmak pending WAL byte'larını sync eder,
hata motoru poisoned yapar. Sistem/elektrik kesintisinde sync-off onayları
kaybolabilir; process-kill testi OS cache'ini ayakta tuttuğundan bunu kanıtlamaz.
[Ayrı commit ve bakım dahil ölçüm](WAL-SYNC-VALIDATION.md).

## Posting oluşturucu kotası

`bounded-engine` (eski `experimental-builder-budget` alias'ı) posting spill tahmini tamponuna oluşturucu başına
1 MiB tercih/256 KiB minimum ve ortak 2 MiB havuz uygular. I/O lease önce değil,
posting lease sonrasında alınır. Geçici spill merge fan-in en fazla32 okuyucudur;
ara dosyalar aynı geçici codec ile yazılır ve Drop temizliği sahipliği izler.
Kota toplam heap/RSS sınırı değildir. Varsayılan dahil spill beklenen uzunluğu
açılış/EOF'ta kontrol edilir; CRC ve strict ID sırası 4 KiB blok I/O'da korunur.
[Ölçüm ve hata testleri](BUILDER-BUDGET-VALIDATION.md).

## Resident belge paylaşımı ve delta sorgu anahtarları

`experimental-shared-documents` belge anahtarını docs/indeks/shadow arasında
tek Arc<Key> ile paylaşır. Docs standart HashMap, shadow sıralı BTreeSet'tir;
indeks mevcut Arc'ı alır. Hash tablosunun iterasyon sırası slot sırası veya
public key sırası sayılmaz. Query sıralama ve flush shadow key sırasını kullanır.
Flush docs hash kapasitesini de bırakır. İlk B-tree + paylaşımlı anahtar deneyi
unique okuma hızını bozduğundan HashMap lookup yoluna geçildi.
Row title/version/serialized_len/links ile gövde ve alan kümesini taşır. En fazla
4096 byte gövde ve 16 skaler alan içeren küçük kümeler CRC bucket ve tam byte
eşitliğiyle paylaşılır. Gövde ve alan cache'lerinin her biri 128 KiB tahmini
admission bütçesi ve 256 entry sınırı kullanır. Alan canonical JSON byte'ları
hash çakışmasında değer eşitliğini ve sayı gösterimini korur. Büyük/karmaşık
veya cache sınırına takılan değerler kendi String/BTreeMap'lerini tutar.
Bu bütçeler allocator ek yükünü ve toplam proses belleğini kapsamaz.

Get/search API'si caller-owned StoredDocument döndürür; ortak gövde/alanlar
kopyalanır, WAL I/O veya JSON decode gerekmez. Caller'ın dönen belgeyi
değiştirmesi engine verisini etkilemez. Recovery normal checksum/schema
doğrulaması sonrasında aynı RAM yerleşimini kurar. Başarılı flush cache'i
bırakır. Kalıcı codec değişmez. WAL-delta aynı anda açıksa WAL yolu önceliklidir;
all-features testi resident paylaşım yolunu kapsamıyor, seçilmiş feature testi
ayrıca gereklidir. Resident text sayacı paylaşılan gövdeleri bir kez sayar;
alan ağaçları/B-tree/Arc kapasitesi ve toplam proses RAM'i değildir.

Delta sorgu adayları sonuç başına yeni Key ve üç String allocation üretmez;
indeksin Arc<Key>'lerini taşır. Ara matched-ID Vec kaldırıldı. Compact slot
sırası key sırası olamayacağı için Arc referansları key değerine göre sıralanır.
Disk/delta merge cursor yalnız ilerlediği anahtarın owned kopyasını üretir;
sayfa payload'ları mevcut hydration sözleşmesiyle döner. Sonuç sayısı, key
sırası, visibility ve offset davranışı korunur. Bu sorgu değişikliği default
ve deneysel indeks yollarında uygulanır; belge paylaşımı default kapalıdır.
[Anahtar paylaşımı doğrulaması](SHARED-KEYS-VALIDATION.md).

## Flush/merge posting yeniden kullanımı

Bu bileşen güncel `bounded-engine` varsayılanının parçasıdır. Aşağıdaki
tasarım ayrıntıları ve bağlantılı ölçümler ilk bileşen deneyinin kapsamını korur.

`experimental-merge-postings` bağımsızdır; write-buffer-budget/streaming-metadata
yollarını otomatik açmaz. Worker önce input kaynak prefix checksum ve immutable
length'i, SEG03/SEG04 bütün dosya checksum/footer'ını doğrular. Legacy kaynak WAL append'i
nedeniyle checkpoint prefix'inden uzun olabilir; yalnız published prefix bağlanır.
MergeKeys en yeni kayıtları seçer; her input için u32 old→new slot map kurulur.
Tombstone/eski sürüm u32::MAX ile dışarıda kalır, output tombstone slotları korunur.

Kaynak checkpoint mevcut codec ile tamamlanır. Bu kaynak akışı Builder'a
payload/record olarak eklenir; terms/grams üretimi yapılmaz. Input term dictionary
iterator'ları heap ile, her eş term'in posting'leri 8 KiB/input penceresiyle
birleştirilir. Source ID sırası, bounds, monotonic remap ve duplicate output ID
kontrol edilir. Boş kalan term atılır. SEG03/SEG04 directory/footer aynı biçimde yazılır.
Sonra normal open_source bütün authoritative kaynak doğrulamasını tekrar yapar;
worker manifest yayınlamaz. Bu ek kaynak doğrulama geçişi hız ölçümüne dahildir;
native TXN2 binary payload veya legacy JSON mevcut codec ile okunur.

Map 4 byte/input fiziksel kayıt, root/term dictionaries ve scratch ortak I/O
reservation kotası dışında kalır; toplam heap/RSS bütçesi değildir. Flush indexleri
mevcut delta dictionary'sinden oluşturulur: scope/collection/exact/numeric/gram/link
listeleri sorted checkpoint slot'larına remap edilir. Tombstone slot'ları korunur;
RAM slot sırası disk kayıt sırası sayılmaz. Bir term'in remapped ID listesi dışında
toplu posting kopyası yoktur. Source replay ve normal open_source doğrulaması
sürer; metin/terim çıkarımı yeniden yapılmaz. Güncel bounded-engine varsayılanında açıktır.
[Önceki merge doğrulaması](MERGE-POSTINGS-VALIDATION.md).

## Deneysel WAL konumlu delta

`experimental-wal-delta` delta değerinde StoredDocument yerine WAL byte konumu,
length/CRC/version tutar. Commit encoder aynı kalıcı JSON/framing byte'larını
üretir; metadata bakım/flush sonrasında yeni active log başlangıcına bağlanır.
Recovery normal frame/source doğrulamalarını korur ve borrowed raw JSON
aralıklarından konum bulur. Tam WAL frame ve Batch geçici olarak hâlâ decode
edilir. Get/search/flush tek belgeyi okuyup CRC/key/version kontrol eder.
Guard/accounting tam belge okumaz. Hatalı append/index uygulaması motoru
poisoned yapar. Kalıcı format değişmez; default kapalıdır.

Resident full document sayısı bu feature'da sıfırdır; folded text/index,
shadow/record keys ve builder allocation'ları RAM'dedir. Toplam heap/RSS bütçesi
değildir. RAM azalması unflushed delta okuma hızını korumadı; bu yol default'a
alınmaz. Karar/ayrı query kapısı: [WAL delta doğrulaması](WAL-DELTA-VALIDATION.md).

## Akışlı metadata

Güncel bounded-engine varsayılanında açıktır; eski component adı ve
tasarım/ölçüm tarihçesi aşağıda korunur.

`experimental-streaming-metadata` streaming-open/paged-metadata içerir.
SEG03 checksum/bounds kontrolünden sonra kayıt JSON'u tek entry scratch ile
çözülür; her kayıt sıralama/offset/len/version açısından doğrulanır. PackedRecord
Vec, bütün ID string pool'u ve group dizini üretilmez. Token/posting sözlüğü
hâlâ RAM'dedir; Root fence/deletion bitmap N ile büyümeyi sürdürür.

Mevcut MET04 header/footer/global CRC/binding/root count doğrulanır. Ardından
SEG03 kayıt akışıyla bir leaf ve en fazla 32 leaf fence üzerinden bütün cache
kayıtları karşılaştırılır: numeric metadata ve allocation üretmeyen key byte
karşılaştırması, leaf/root identity, count, contiguity ve sayfa CRC korunur.
Deletion bitmap kayıt akışıyla büyür; cache header'daki count kadar peşinen
allocation yapılmaz. Cache ancak bütün SEG03 directory parse/validation ve
bütün slot eşitliği bittikten sonra kullanılır. .fdb replay/checksum, bu kontrolün
ardından da normal open_source içinde koşar; sağlam cache bozuk kaynağı gizlemez.

Eksik/bozuk sidecar create-new temporary içinde 64-record leaf / 32-fence
directory ile yeniden üretilir. Header count finish'te patch edilir; root JSON
16 MiB bounded sink ile yazılır, prefix/footer CRC ve sync/rename tamamlanır.
Sonra SEG03 bağımsız olarak tekrar parse edilir ve üretilen her slot karşılaştırılır.
İlk üretim geçişi de token/posting dictionary parse eder; bunlar doğrulama sonrası
bırakılır ve ikinci geçişte tekrar kurulur. Bu ek parse maliyeti ölçüm kapsamındadır.
MET04 byte biçimi değişmez. Sidecar builder'ın SEG03 rename'ından önce ortaya
çıkabilir; türetilmiş cache'tir, kaynak/manifest publication yetkisi yoktur.

DiskIndex.open bu feature'da paged sonuç verir; enable_paging ikinci kez byte
saymaz veya aynı tabloyu yeniden üretmez. Eski resident reference'i gerektiren
codec/fault-injection testleri açık resident mode kullanır; yeni streaming mode
ayrı cache cut/corruption/byte eşitliği ve tam DB testleriyle doğrulanır.

Record leaf, JSON scratch, directory fence, root/deletion ve token alanları ortak
I/O reservation kotası dışında kalır. Sabit toplam RAM garantisi değildir.
Ölçüm: [metadata akışı doğrulaması](STREAMING-METADATA-VALIDATION.md).

## Delta posting yerleşimi

Compact ve bitmap bileşenleri güncel bounded-engine varsayılanında açıktır.

`experimental-compact-delta-postings` posting içindeki Arc<Key> yerine u32
slot numarası tutar. Bir key→slot B-tree ve Option<Slot> Vec'i anahtar ile
folded metni çözer. Silme bütün eski posting'lerden ID'yi çıkardıktan sonra
slot'u boşaltır; boş numaralar yeniden kullanılır, update sayısıyla slot listesi
büyümez. Query sonuçları slot sırasıyla değil anahtar sırasıyla yeniden sıralanır.
Bu ek delta sort maliyeti ve slot Vec kapasitesi ölçüm kapsamındadır.

Kapasite commit WAL append'den önce konservatif batch üst sınırıyla kontrol
edilir; açılışta index kurulum hataları Result ile döner. Kalıcı WAL sonrası
beklenmeyen index hatası motoru poisoned yapar. u32 ID'ler yalnız RAM'e aittir;
source/cache formatı veya eski fixture değişmez. Kaynak bozulması denetimi sürer.

`experimental-delta-bitmaps` compact feature'ını içerir; liste dört ID'ye kadar
inline array, sonra sorted Vec, yeterli yoğunlukta u64 bitmap taşır. En az 64
ID ve gerekli bitmap byte sayısı <= ID sayısı*2 olduğunda bitmap'e geçer.
Uzak bir ID, bitmap'i aşırı büyütmek yerine sparse temsile döndürür. Bitmap
32 ID altına indiğinde sparse olur; sparse <=4 ID olduğunda inline olur.
Duplicate insert ve olmayan remove cardinality değiştirmez. Sparse insert/remove
lineer kaydırma yapabilir; density conversion, bitmap iteration ve retained Vec
capacity genel RSS kotası değildir. Bir slot dört milyardan fazla delta key'i
kapsamaz; sınır preflight ile hata verir, wrap edilmez.

Ölçüm: [delta posting doğrulaması](DELTA-POSTINGS-VALIDATION.md).

## Deneysel ortak delta metni

`experimental-shared-delta-text` bağımsız RAM yerleşimi deneyidir. Delta
indeksinin doğrulama metni birleşik title+newline+body String yerine ayrı title
ve Arc<str> body taşır. 256 KiB tahmini body-cache'e alınan aynı kaynak metni
aynı katlanmış gövdeyi paylaşır. Cache'e sığmayan gövde bağımsız Arc taşır;
çeşitli veya büyük içeriklerde tekrar paylaşımı garanti edilmez. Cache admission
sınırı tahminidir; belge referansları ve allocator kapasitesi toplam kotası değildir.

Sorgular Unicode whitespace ile terimlere ayrılır; hiçbir terim başlık/gövde
arasındaki newline'ı geçemez. Contains bu nedenle title veya body'de ayrı
çalışır. Trigram posting ve sıralama/gölgeleme düzeni değişmez. Builder da
aynı BodyCache tipini kullanır, paylaşım mantığı kalıcı payload'ı değiştirmez.
`resident_text_bytes` feature açıkken ortak folded body allocation'larını bir
kez sayar, cache'te kalan folded gövdeleri de kapsar; kaynak belge title/text
baytları ayrıca sayılır. Raw cache anahtarları, posting/B-tree/Arc yönetimi,
allocator kapasitesi ve proses belleği bu sayaç değildir.

Ölçüm: [ortak delta metni doğrulaması](SHARED-DELTA-TEXT-VALIDATION.md).

## Checkpoint yazma bütçesi

Bu bileşen güncel bounded-engine varsayılanına dahildir.

`experimental-write-buffer-budget` write-pipeline içerir. Yalnız unpublished
run checkpoint'inde değişiklikler doğrudan checksummed BufWriter üzerinden
JSON frame'e akar; büyük Vec<Change>/encoded frame tamponları oluşturulmaz.
Payload/footer sonra header length/CRC yazılıp sync edilir; kaynak formatı
aynıdır. WAL atomik encode yolunu korur. 8 MiB frame hedefi ve 64 MiB cap
korunur; büyük/composite alanlarda preflight boy sayımı kaydı ayrı frame'e alır.

Kaynak output buffer'ları (8..64 KiB) ve builder payload+spool buffer'ları
(16 KiB) ortak proses çapında 256 KiB byte reservation paylaşır. Lease'ler
allocation öncesi alınır, buffer'lardan sonra bırakılır; source/builder lease'leri
birlikte tutulmaz. Delta, caller/WAL, posting/spill/body/token, packed metadata,
page/output ve tekil belge bu kota dışındadır. Tam heap/RSS bütçesi değildir.
Detaylar: [yazma bütçesi doğrulaması](WRITE-BUFFER-BUDGET-VALIDATION.md).

## Disk metadata yolu

Bu bileşenler güncel bounded-engine varsayılanına dahildir; maintenance-slots
da birleşim içinde açıktır. Bağımsız feature adları karşılaştırma amacıyla korunur.

`experimental-write-pipeline` streaming-open/paged-metadata yolunu içerir.
Merge-policy, record-spool, maintenance-slots ve merge-slot artık bağımsız
`experimental-write-*` feature'larıdır. Güncel umbrella merge-policy, record-spool
ve merge-slot içerir; bakım kilitleri yalnız maintenance-slots ile açılır. Aynı koddan
tek bileşen çıkararak ölçüm: [bileşen doğrulaması](WRITE-COMPONENTS-VALIDATION.md).
Otomatik bakım yalnız dört benzer boyutta contiguous run seçer (max/min <= 2).
Açık start_compaction/compact kalan 2..4 contiguous run'ı da seçebilir; eski
100 bin key, 64 MiB source ve max/min <= 4 sınırları korunur. Adaylar arasında
keys/(run_count-1) maliyeti en düşük grup seçilir; büyük eski run'ı her küçük
flush'ta tekrar yazmak azaltılır. Sıra/görünürlük ve manifest yayın protokolü
değişmez. Otomatik bakımın beklemesi daha fazla run ve sorgu işi yaratabilir.

Bu feature'da Builder kayıt metadata'sı Vec<Record> yerine uzunluk + CRC
kontrollü geçici spool'a yazılır; RAM'de count ve son Key kalır. Finish spool'u
tek entry scratch (en fazla 256 KiB) ile okuyup aynı SEG03 JSON records dizisini
üretir. Spool yalnız geçici cache üretimidir, authoritative veri değildir;
normal dönüşte/drop'ta temizlenir, process-kill kalıntıları otomatik GC edilmez.
SEG03 footer/global CRC ve bağımsız DiskIndex.open kontrolü yine uygulanır.

`experimental-write-maintenance-slots` ayrıca açıksa write_run, kaynak
yazımı/checksum bitimine kadar proses çapında tek kaynak
çalışma slotunu tutar. Bu slot bırakıldıktan sonra doğrulama, cache builder
ve metadata paging için ayrı tek builder slotu alınır. Bir kaynak yazıcısı
ve bir builder örtüşebilir; iki kaynak yazıcısı veya iki builder örtüşemez.
Foreground flush ve background merge aynı iki sınırı paylaşır. İki kilit
birlikte tutulmaz; bunlar genel bir byte/RSS bütçesi değildir.
RAM delta, caller batch, output, token sözlüğü, cache rebuild yapan başka open
ve source frame/packed table allocation'ları ayrıca kalır. Merge, seçilmiş
run/slot'un payload'ını direct-slot ile okur; CRC/key/version doğrulanır ve
anahtarı yeniden arama işi kaldırılır. Kalıcı FDBLOG01/03, MAN03 ve SEG03
formatı değişmez. Ölçüm: [yazma hattı doğrulaması](WRITE-PIPELINE-VALIDATION.md).

`diagnostic-timing` isteğe bağlı geliştirme sayacıdır. Commit validation/guard,
frame encode, foreground maintenance, delta-size JSON, WAL write/sync, delta
index uygulaması; builder, segment open, positional read ve sorgu candidate/page
aşamalarının wall sürelerini toplar. İç içe span'lar ve worker'lar örtüşür;
toplamları CPU veya proses elapsed süresi değildir. Aktif span ancak bittiğinde
sayaca girer; worker tamamlanmadan alınan snapshot pencere sınırını aşabilir.
Sayaç/timer/Mutex normal build'de derlenmez. Profiling ayrı binary ile yapılır;
karşılaştırma binary'sinde feature kapalıdır. Ayrı atomik allocation sayacı
örnekleri RAM attribution için korunur; timing sayaçları allocation/RSS ölçmez.

`experimental-paged-metadata` gruplu/direct-slot feature'larını içerir. Yalnız
bu build'de kaynak doğrulaması tamamlandıktan sonra her run'ın PackedRecord,
ID String ve grup tablosu bırakılır. Default build mevcut resident yolu korur.
Kaynak/manifest biçimleri değişmez; SEG03 payload/posting cache'i de korunur.
Yeni `run-*.records04.idx` / `generation-*.records04.idx` yalnız türetilmiş
metadata cache'idir. SEG03 silindiğinde/yenilendiğinde kaynak replay ve cache
yeniden yapımı geçerlidir; hiçbir cache bozuk `.fdb` kaynağını kabul ettirmez.

Sidecar başlığı FDBMET04, SEG03 uzunluğu/footer CRC'si, sequence ve slot count
taşır. 64 slot'luk yaprakların anahtar bileşenleri ilk anahtara göre byte-prefix
sıkıştırılır; bağımsız u32 entry offset tablosuyla slot'a doğrudan erişilir.
Prefix UTF-8 karakter sınırında olmak zorunda değildir; birleşmiş anahtar UTF-8
olarak doğrulanır. Anahtar sırası scope/collection/id UTF-8 leksikografik sırasıdır;
uzunluk prefix'leri sıralama anahtarı olarak kullanılmaz.

32 yaprağa bir disk dizin sayfası, yani 2048 slota bir RAM root fence vardır.
Dizinler aynı binary kayıt kodlamasını kullanır; descriptor offset/length/slot
alanları yaprak sayfasını tanımlar. Root dizini JSON'dur ve 16 MiB ile sınırlıdır.
Her sayfanın CRC32'si, FDBMEND4 footer'da bütün cache ve footer CRC32'si vardır.
Açılışta slot/dizin sayıları, kesintisiz yerleşim, tüm kayıt/key/version alanları
bağımsız doğrulanmış SEG03 diziniyle karşılaştırılır. Bozuk/kayıp/eski sidecar
aynı SEG03'ten create-new temporary + sync + rename ile yeniden kurulur. Rename
öncesi eski türetilmiş sidecar kaldırılabilir; kesilme kaynağı kaybettirmez.
Normal cache yapımının process-kill kalıntıları otomatik GC edilmez.

Foreground ve merge worker aynı proses çapındaki 256 KiB raw-page cache'ini
kullanır. Mutex altında sayfa decode edilip owned sonuç döner; pinned sayfa
referansları bütçenin dışında tutulmaz. Yeni okuma buffer'ı ayrılmadan eviction
yapılır. Dizin/anahtar decode sonuçları, yönetim girişleri, çıktı belgeleri,
root fence ve transient build/replay bu raw-page bütçesine dahil değildir.
Tombstone bitmap'i yaklaşık slot başına bir bit RAM kullanır; posting döngüsü
her aday için metadata sayfası okumaz. Root ve bitmap hâlâ kayıt sayısıyla büyür.

Key/find/version/get/merge iterasyonu I/O hatalarını Result üzerinden taşır.
Key binary karşılaştırması geçici String üretmeden sıkıştırılmış byte parçalarını
karşılaştırır. Run'lar arasındaki görünürlük için Database başına en fazla
64 KiB slot payload / 256 girişlik ek cache kullanılır; yönetim Vec'i ayrıca
yer kaplar. Açılışta veya ilk sorguda hesaplanır, run listesi değiştiğinde temizlenir.
RAM delta gölgelemesi her sorguda ayrıca uygulanır. Cache'e sığmayan görünürlük
normal fallible key taramasına döner.

Metin doğrulaması için sorguya ait bir cursor son metadata yaprağını yeniden
kullanır. Ek owned yaprak buffer'ı en fazla 256 KiB, payload penceresi en fazla
64 KiB'dir; bu iki buffer ortak raw-page cache kotasına dahil değildir. Yaprak
değişirken eski metadata buffer'ı yeni buffer ayrılmadan bırakılır. Yakın payload
aralıkları aynı yaprakta 4096 bayt gap ve 64 KiB pencere sınırıyla okunur. Seçilen
her payload kendi CRC ve sıkıştırılmış anahtar/sürüm identity kontrolünden geçer;
seçilmeyen aradaki payload parse edilmez. 64 KiB'den büyük tekil belge ayrı okunur.
Bu buffer sınırları çıktı belgesi veya büyük tekil allocation sınırı değildir.

Bu feature'da `search` gruplu slot yolunu seçer. Derin offset'lerde görünür/sıralı
ID listeleri, rakip run'ın sıradaki anahtarına kadar ikili aramayla atlanır.
Atlama payload/total/sequence sırasını değiştirmez. Büyük offset'lerin tam u32
listeleri hâlâ RAM'de tutulur. Açılış count'u aynı görünürlük maskeleriyle hesaplanır;
source/cache doğrulaması atlanmaz. `disk_index_bytes` sidecar'ı da içerir.

Bu aşama açılış/build JSON dizinlerini, token sözlüğünü veya WAL delta'sını
akışa dönüştürmez; sabit toplam bellek garantisi ve SQLite'tan düşük RAM iddiası
yoktur. Ölçümler ve varsayılan yapılmama gerekçesi
[doğrulama belgesinde](PAGED-METADATA-VALIDATION.md).

Motor bağımsızdır; SDB/Fundops yalnız veri modeli referansıdır. Hazır DB veya
arama motoru kullanılmaz. Serde/serde_json veri serileştirmesine hizmet eder.
Bir klasörde OS dosya kilidini tutan tek Database sahibi vardır.

## Yazma ve kurtarma

Anahtar (scope, collection, id) üçlüsüdür. Batch'teki bütün kayıtlar ve
guard'lar önceki sürümlerle doğrulanır; aynı anahtar iki kez değiştirilemez.
Geçerli işlem bir sequence alır, günlükte tek frame olarak yazılır ve
sync_all sonrası RAM delta/index güncellenir. Başarılı yanıt kalıcı yazma
sonrasındadır. I/O hatası motoru poisoned yapar; yeniden açıp sürümü sorgulayın.

0.3 kaynak başlığı FDBLOG03; eski FDBLOG01 de okunur. Frame biçimi:
TXN1, little-endian u32 payload uzunluğu, u64 sequence, payload CRC32,
header CRC32, UTF-8 JSON batch, FDBDONE1. Payload en fazla 64 MiB.
CRC bozulma tespitidir, kriptografik doğrulama değildir.

Aktif WAL generation-<16hex>.fdb önce boş checkpoint taşır, ardından normal
işlemler gelir. Son eksik frame son tam sınırda kesilir; tam frame bozulması
açılışı reddeder. Haricen sonradan kısaltılmış kabul edilmiş bir işlem ile
cevap verilmemiş yarım yazma ayırt edilemez. Kaynakların dışarıdan değiştirilmesi
desteklenmez. Yanıt kaybında işlem kalıcı olabilir; kör retry yerine guard kullanın.

## Çoklu kaynak ve yayımlama

flush delta anahtarlarını sıralı, değişmez run-<16hex>.fdb kaynağına yazar.
Put özgün kayıt sürümünü, delete kalıcı silme işaretini taşır. Yaklaşık 8 MiB
checkpoint batch'leri aynı sequence ile çerçevelenir. Eski kaynaklar değişmez.
Kaynak sync edilir, türetilmiş cache hazırlanır, yeni boş aktif WAL sync edilir.
Son olarak .ready manifest yayımlanır; ardından RAM delta boşaltılır.

Manifest biçimi FDBMAN03, u64 yayımlama nesli, u32 JSON uzunluğu, JSON, u32 CRC32.
CRC başlık ve payload'ın tamamını kapsar; nesil dosya adıyla eşleşmelidir.
JSON version:3, eski→yeni sıralı kaynak listesi ve aktif WAL tanımı taşır.
Kaynakta kind/id/end/checksum/sequence/keys; aktifte id/end/sequence vardır.
En fazla 16 MiB manifest kabul edilir. .pending create-new + sync ardından
yeni .ready adına rename edilir. Unix parent directory de sync edilir.
Windows elektrik kesintisinde dizin metadata kalıcılığı ayrıca doğrulanmadı.

En büyük yayımlanmış nesil seçilir. En yeni marker veya referans verilen kaynak
bozuksa eski nesle düşülmez; aksi son kabul edilen yazmalar kaybolabilir.
Yayımlanmamış dosyalar okunmaz, ID tahsisinde hesaba katılır. Marker'lar kayıpsa
mevcut kaynaklardan boş DB kurulmaz. Eski dosyalar otomatik silinmez; tarihsel
arşivler güncel backup değildir. Backup için motoru kapatıp klasörü bütünüyle kopyalayın.

Her referans kaynak uzunluk/CRC ve frame replay ile doğrulanır. Immutable run
tam dosyadır; eski kaynak yalnız checkpoint prefix'i olabilir. Prefix doğrulaması
kaynakta kesme yapmaz. Kaynak bozulması sağlam .idx ile gizlenmez.

## Arka plan birleştirme

Bir worker sıralı, art arda gelen benzer büyüklükte 2–4 segmenti işler.
Toplam giriş 100.000 anahtar ve 64 MiB kaynakla sınırlıdır; en büyük/küçük
anahtar sayısı oranı en fazla 4'tür. Birleştirme tüm DB'yi yeniden yazmaz.
Sıralı heap ile her anahtarda en yeni kaynak seçilir; tombstone korunur.
Worker yalnız yeni kaynak/cache üretir, manifest yayımlamaz. Bu sırada sahip
normal yazmaları aktif WAL/delta'da kabul eder ve yeni segment ekleyebilir.
Sahip iş bitince seçili pencerenin aynı kaldığını doğrular ve onu tek run ile
değiştiren manifest yayımlar; sonradan eklenen segment ve WAL korunur.

compact: flush + en fazla bir iş başlatma; sonucu beklemez.
start_compaction: uygun bir grup varsa iş başlatır; iş zaten varsa false.
poll_compaction: biten işi beklemeden yayımlar.
wait_for_compaction: mevcut işi bekler ve yayımlar.
Commit/flush da biten işi poll eder. Boşta duran motor sonuç yayımlamaz.
Drop worker'ı join eder, sonucu yayımlamaz; kullanılmayan dosya kalabilir.
Flush ve manifest yayımlama maliyeti yazma sahibindedir. Büyük segmentler
iş bütçesini aştığında tekrar birleştirilmez; eski sürümler/tombstone disk maliyeti kalır.

## Türetilmiş disk indeksi

FDBSEG03 başlığı sequence, kaynak bitişi ve kaynak CRC'sini taşır. Sıralı
JSON kayıt payload'ları, sıralı u32 ID posting'leri ve JSON dizin tablosu vardır.
Kayıt dizini key/offset/length/version/checksum/deleted alanlarını taşır.
Tombstone payload taşımaz. Posting dizini term/offset/count tutar. FDBEND03
footer dizin sınırlarını, tüm cache CRC'sini ve footer CRC'sini doğrular.
Dizin segment başına 256 MiB sınırındadır; yerleşim sınırları ayrıca doğrulanır.

Posting derlemesi yaklaşık 4 MiB tahmini bütçede grupları geçici dosyalara döker.
Her grubun header/term/IDs checksum'u vardır. Sıralı heap ve akışla birleştirme
posting'lerin tamamını RAM'de tutmayı önler. Bütçe toplam bellek limiti değildir:
kayıt/token dizinleri, tek kaydın trigram kümesi, payload, buffer ve allocator
kapasitesi ayrıca yer kaplar. Geçici dosyalar normal çıkışta temizlenir;
zorla sonlandırma kalıntıları otomatik GC ile temizlenmez.

Anahtar/konum tablosu ve token sözlüğü RAM'dedir. İçerik ve posting'ler güvenli
positional I/O ile okunur; mmap/unsafe kullanılmaz. Kayıt okuması ayrıca CRC kontrol eder.
Kayıp/bozuk/eski .idx sağlam kaynak replay'iyle yeniden üretilir; cache kaynak değildir.

0.3.1'de disk dizini açıldıktan sonra RAM'de paketlenir: scope/collection
çiftleri segmentte ortak tutulur, ID'ler tek UTF-8 havuzundadır. Sabit kayıt
metadata'sı ayrı kompakt vektördedir. Kayıt başına üç ayrı String tutulmaz.
Diskteki SEG03 dizin biçimi değişmez; eski cache'ler yeniden yazılmadan okunur.
Dizinin JSON açılışı ve builder geçici dizini hâlâ ek tepe bellek maliyeti taşır.

## Sorgular ve bellek

RAM delta: BTreeMap kayıtlar, scope/collection posting'leri, scalar eşitlik,
i64 range, Türkçe katlanmış Unicode trigramları ve ters bağlantı indeksi.
0.3.1 posting'leri tam anahtar kopyaları yerine aynı Arc<Key> referansını
paylaşır; güncelleme/silmede eski posting referansları kaldırılır.
Diskte aynı token türleri u32 listelerdir. AND filtreleri küçük adaydan yürür.
Metin adayları payload okunup alt-dize karşılaştırmasıyla doğrulanır; kısa
terimler de aday metninde kontrol edilir. Nested JSON saklanır fakat nested sorgu yoktur.

Delta ve yeni segment anahtarları eski sonuçları gölgeler. Her run'ın eşleşme
u32 listesi geçici olarak tutulur; ID başına 4 bayt artı kapasite maliyeti vardır.
0.3.1'de gizlenen ID'ler sorgu başında bit maskesine işaretlenir; her aday
için bütün yeni segmentlere tekrar anahtar araması yapılmaz. Kesişmeyen
segment anahtar aralıkları atlanır. Maske run başına yaklaşık N/8 bayttır.
Güncelleme yoğun, kesişen run'larda maske hazırlamak hâlâ anahtar taraması gerektirir.
Sonuçlar heap ile key sırasında birleşir, yalnız sayfadaki payload'lar get edilir.
Offset büyükse önceki eşleşmeler de dolaşılır; snapshot pagination/ilgi puanı yoktur.

Varsayılan delta eşiği 10.000 değişmiş anahtar veya 16 MiB serileştirilmiş kayıt.
Sonraki geçerli commit öncesi flush edilir; guard reddi bakım başlatmaz.
Tek batch eşiği aşabilir. resident_text_bytes yalnız delta'nın özgün+katlanmış
metin baytlarıdır; RAM/RSS değildir. source_bytes/disk_index_bytes yalnız
manifestin referans verdiği kaynak/cache'i içerir; toplam klasör boyutu değildir.

Sorgu terimleri Unicode whitespace ile ayrılır. 0.3.1 yeni indekslerde whitespace
içeren trigram üretmez; hiçbir sorgu terimi bunları istemez. Eski cache'lerde
bulunan ek trigramlar zararsızdır. Metin doğrulaması başlık ve içerikte ayrı
çalışır; terimler whitespace içermediğinden başlık+newline+içerik davranışını korur.
Sorgu içinde aynı içerik için folded metin tekrar kullanılır. Özgün+folded
metin bütçesi yaklaşık 256 KiB'dır; BTreeMap/allocator ek maliyeti vardır.
Çeşitli veya uzun içerikler bütçeyi aştığında normal doğrulama yapılır.
Delta indeksinde ve segment builder'da da içerik için folded metin/trigram
kümeleri tekrar kullanılır. Her önbelleğin yaklaşık 256 KiB tahmini bütçesi
vardır; metinler ve trigram metadata'sı hesaba katılır. Bütçeyi aşan içerik
önbelleğe alınmadan işlenir. Başlık trigramları içerik kümesiyle sıralı union
ile birleşir; aynı kaydın aynı token'a iki kez eklenmesi önlenir. Link'ler ayrıca
tekilleştirilir. Bunlar toplam proses bellek limiti değildir.
CRC32 IEEE hesabı slicing-by-8 kullanır; checksum ve kalıcı format değişmez.

## Uyumluluk ve kapsam

### Bellek geliştirmesi: aşama 1

Kalıcı/cache biçimleri değişmeden sınırlı sayfa yolu eklendi. `offset+limit <= 4096`
için her run tüm eşleşmelerin ID vektörünü oluşturmaz; en fazla `offset+limit` ID'lik max heap
en küçük slot'ları tutar ve sayfa ID'leri sıralanır. Driver grupları collection
veya tek scalar değer posting'lerinden oluşur; kaynak indeksinin yapısı gereği
bir grubun posting'leri birbirinden ayrık kayıt kümeleridir. Posting başına
sıralı blok taraması korunur; ilk aday heap'in maksimumunu aştıktan sonra o
posting'in sonraki ID'leri sayfa seçiminde işlenmez, fakat sayım/filtreleme sürer.

Metinsiz bir sorguda her filtre grubunda tüm canlı kayıtları kapsayan bir posting
varsa count için görünür canlı kayıt sayısı kullanılır. Maske popcount'unda
tombstone slot'ları sayılmaz. Metin doğrulaması bu kestirmeyi kullanmaz. Maskeler
delta ve daha yeni run anahtarlarını aynı şekilde gölgeler. İlk sayfa için
source/cache açılış doğrulaması ve dönen payload CRC kontrolü korunur.

`offset+limit > 4096` yolunda tüm ID listesi ve sort/dedup hâlâ vardır; geçici ID belleği
kayıt sayısıyla büyür. Maskeler, token sözlüğü, kaynak replay'i ve builder da
genel bir RAM bütçesine bağlanmadı. Dolayısıyla bu prototip sayfalı indeks
veya SQLite düzeyinde proses belleği hedefinin tamamlanması değildir.

ID havuzu, dizin doğrulaması sırasında hesaplanan toplam UTF-8 ID uzunluğuyla
tek seferde ayrılır. String'in geometrik büyümesinden kalan capacity fazlası ve
havuz büyütme kopyaları kaldırılır. Kayıt sayısıyla büyüyen tablonun kendisi
henüz diske taşınmamıştır.

`Database::index_memory_layout()` seçilmiş Vec/String capacity miktarlarını
döndürür. BTreeMap düğümleri, allocator metadata, transient işler ve RSS dahil
değildir. Allocation profilleri yalnız opt-in örnek binary'lerdedir; normal
motorun allocator'ına sayaç veya atomic işlem eklenmez.

Native SQLite ayrı `tools/native_sqlite` Cargo paketindedir; FDB manifesti ve
root lockfile'ında SQLite bağımlılığı bulunmaz. Ölçümler ve sınırlar için
[aşama 1 doğrulaması](MEMORY-STAGE1-VALIDATION.md).

`experimental-direct-slot` Cargo feature'ı ile açılan `search_direct_slots`,
görünürlük maskesiyle seçilmiş run/slot'u doğrudan kullanır. RAM delta sonucu
`docs` içinden alınır. Disk okumasında slot bounds, tombstone, payload CRC ve
metadata ile key/version identity kontrolleri geçerlidir. Varsayılan `search`
ve `get(key)` anahtar üzerinden okuma yolunu korur. Hız kabulü tamamlanmadan
deneysel yol varsayılan yapılmaz. Bu adım kayıt tablosunu diske taşımaz;
sayfalı metadata için gerekli doğrudan slot yolunu sağlar.

`experimental-grouped-read` feature'ı direct-slot feature'ını içerir ve
`search_grouped_slots` metodunu açar. `offset+limit <= 4096` için önce küresel
sayfanın görünür run/slot seçimleri toplanır. Seçimler run'a göre ayrılır; yakın
payload aralıkları en fazla 64 KiB bir pencerede okunur. Aradaki en büyük boşluk
4096 bayttır. Her seçilmiş belge kendi CRC ve key/version kontrolünden geçer;
aradaki seçilmemiş belgeler parse edilmez. Tek scratch Vec bütün run'larda
yeniden kullanılır. Büyük belgeler tekil direct-slot okumasına döner. Sonuçlar
kaydedilmiş sayfa pozisyonlarıyla eski küresel sıraya alınır. Derin offset veya
toplama taşması direct-slot yolunu kullanır; ilk koşulsuz prototip bu yolda
ölçülen gerileme nedeniyle sınırlanmıştır. Default sorgu yolu değişmez.

64 KiB sınırı batch okuma penceresidir; çıktı belgeleri, seçim listeleri,
maskeler veya bütün proses belleğinin sınırı değildir. Bu mekanizma payload
okumalarını birleştirir; RAM'deki kayıt/ID tablosunu veya SEG03 biçimini değiştirmez.

0.1/0.2'nin FDBLOG01 + 28 bayt FDBREADY marker'ı okunur. Eski checkpoint
bir legacy run olur; WAL kuyruğu delta'da replay edilir. İlk yeni flush 0.3
manifest yayımlar. Eski kaynak baytları korunur; fixture değiştirilmez.
SEG02 cache yeniden kurulur. Eski motorlar MAN03/LOG03 okuyamaz; yükseltme
sonrasında eski marker'a dönmek güncel veriyi kaybettirebilir.

Scope sorgu sınırıdır; kullanıcı yetkilendirmesi host sorumluluğundadır.

`experimental-streaming-open`, paged-metadata feature'ını içerir. SEG03 dizini
64 KiB giriş penceresiyle kayıt başına çözülüp paketlenir; bütün JSON blob'u ve
bütün owned Record nesneleri aynı anda tutulmaz. Kayıt scratch'i 256 KiB ile
sınırlıdır; büyük exact alan token'ları eski SEG03 directory sınırına kadar
okunur. Token sözlüğü hâlâ RAM'dedir ve eski bulk BTreeMap yerleşimi korunur.
Paketlenmiş kayıt/ID tabloları bir run açılırken geçici olarak hâlâ ayrılır ve
sidecar doğrulamasından sonra bırakılır. Açılış belleği sabit bütçeli değildir.

Değişmez `.fdb` checkpoint frame'inin CRC/footer'ı önce 64 KiB bloklarla tamamen
doğrulanır, ardından JSON changes kayıt kayıt okunur. Batch schema, key/version,
manifest identity ve source CRC kontrolleri korunur. JSON giriş buffer'ı 64 KiB;
tek Change scratch'i mevcut 64 MiB frame sınırına kadar büyüyebilir. Bu limit
toplam proses RAM garantisi değildir. Hata durumunda geçici derived builder
yayımlanmaz. WAL replay'i atomik Batch çözümlemesini kullanmaya devam eder;
serileştirilmiş payload, Batch uygulanmadan önce bırakılır.

Bütün build'lerde frame encode tek output Vec'e serileştirir; SEG03 builder
directory JSON'unu doğrudan bounded BufWriter'a yazar. Kalıcı/cache byte formatı
değişmez; kaynak sürümü ve 0.1 fixture'ı korunur. Builder'ın kayıt/token tabloları,
caller batch'i, WAL DOM'u ve büyük query çıktıları genel bellek bütçesine henüz
bağlanmamıştır. Ölçümler: [akışlı açılış doğrulaması](STREAMING-MEMORY-VALIDATION.md).
SDB için hacim+tam referans+parent+ad, Fundops için açık doğal anahtar örnekleri
adapters modülündedir. Dosya içerikleri çıkarılmış UTF-8 bölüm kayıtları olarak
metadata kayıtlarına links ile bağlanabilir. Extractor, canlı USN takibi,
HTTP servisi, SQL, MVCC, dağıtık replikasyon ve otomatik GC bu sürümde yoktur.

## İzole rkyv payload deneyi

`experimental-rkyv-payload` default kapalıdır. `rkyv_payload` modülü belgeyi
archive olarak kodlar; fields için kendi düz postorder değer grafını kullanır.
Contains doğrulaması güvenli bytecheck/graph kontrolünden sonra yalnız archived
title/text/key/version'ı okur, sonuç sayfasını tam belgeye dönüştürür. CRC ile
archive doğrulama maliyeti benchmark'ta ayrı ölçülür.

Default `Database`, WAL, manifest, SEG03 veya protokol rkyv arşiv codec'ini çağırmaz. Benchmark'ın
`PLOADB01` taşıyıcısı atılabilir export'tur, kalıcı `.fdb`/`.ready` kaynağı veya
`.idx` cache'i değildir. 0.1 fixture korunur. Üretimde codec kullanımı ayrı format
sürümü ve uyumluluk testi gerektirir. Ölçüm ve karar:
[rkyv payload doğrulaması](RKYV-PAYLOAD-VALIDATION.md).

Aynı deneysel modüldeki `TextMatcher`, eski fold, ASCII hızlandırılmış fold
ve iki tekrar kullanılan String tamponuyla fold seçeneklerini karşılaştırır.
Normalizasyon ve alt dize tarama süreleri ayrı profillenir. Bu payload deneyinde
motor sorgu yolu değiştirilmedi; normalleştirilmiş corpus/cache saklanmadı.
[Normalleştirme ölçümü](FOLD-VALIDATION-20261011.md).

## Disk sorgusunda tekrar kullanılan normalleştirme tamponları

Payload deneyinin ardından default disk contains doğrulamasındaki iki döngü
ortak `QueryTextScratch` kullanır. Paylaşılan `index::fold_into`, ASCII için
toplu lowercase ve UTF-8 için aynı Türkçe/Unicode eşlemelerini uygular. Başlık
ve gövde tamponları sorgu içinde tekrar kullanılır. Başlık bütün terimleri
karşılıyorsa gövdeye gerek kalmaz. Eski 256 KiB gövde cache'i korunur; başlık
cache'lenmez. Her scratch sonraki adayda 128 KiB üzeri kapasitesini bırakır;
büyük tek belge geçici olarak daha fazla alan ister. Bu bir proses RAM sınırı
değildir. Sorgu sonunda scratch ve cache bırakılır.

Yazma/gram üretimi/terim hazırlamasındaki `index::fold` ve delta yerleşimi aynı
kalır. JSON payload ve kaynak doğrulaması korunur; rkyv motor tarafından
çağrılmaz. [Gerçek API, sayfalama ve bellek ölçümü](ENGINE-FOLD-VALIDATION-20261011.md).

## Bağımsız offset payload deneyi

`experimental-rkyv-payload` mevcut deney altyapısında `binary` codec'ini de
açar; yeni feature eklenmedi. `rkyv_payload::binary` rkyv kodlamasını kullanmaz.
`OFF1` başlığı sürüm, beş UTF-8 metin başlangıcı, fields/links başlangıçları ve
son offset'i taşır. Fields null/bool/tamsayı/float/string/list/map etiketleriyle,
links uzunluklu UTF-8 anahtar parçalarıyla saklanır. Unsigned sayı etiketi JSON
u64 değerlerini yuvarlamadan korur; metinler sıkıştırılmaz.

`checked` tüm offset'leri, UTF-8'i, metadata gramerini, sıralı map anahtarlarını,
sonlu float'ları ve 128 derinlik sınırını kontrol eder. Metinler byte tamponundan
ödünç alınır; metadata doğrulamasında alan ağacı oluşturulmaz. Tam belge yalnız
ilk 50 sonuç için oluşturulur. Benchmark taşıyıcısı her okumada CRC ve
key/version kimliğini ayrıca doğrular; JSON da aynı taşıyıcıyı ve reusable fold'u
kullanır. Açılış tüm kayıtları doğrular. Dosya ayrı `binary.payload-bench`
çıktısıdır; `.fdb`, `.ready`, `.idx`, WAL/manifest/protokol ve default motor
yolları değişmez. [Ölçüm ve kabul kararı](BINARY-PAYLOAD-VALIDATION-20261011.md).

## Gerçek API üzerinde geçici payload bağlama

Aynı feature altında yalnız açık `Database::attach_payload_benchmark` çağrısı,
tek immutable segment ve boş delta koşulunda JSON veya binary benchmark
export'unu bağlayabilir. Normal `Database::open` dosyayı keşfetmez/bağlamaz.
Bağlama tüm 100k kaydı mevcut indeksin tam belgeleriyle karşılaştırır; kaynak
`.fdb` doğrulaması daha önce normal açılışta yapılır. Corpus'a özel ordinal
key/version sözleşmesi nedeniyle bu genel bir motor payload API'si değildir.
Paged metadata/tombstone/çok segment bu bağlama deneyinde desteklenmez.

Posting/adayı üretme, görünürlük maskeleri, filtreler, sıralama, toplam ve
sayfalama normal `Database::search` içindedir. Contains doğrulamasında borrowed
title/text ve aynı `QueryTextScratch` kullanılır; sonuç hidrasyonu aynı bağlı
codec'ten tam belge üretir. JSON kontrolü ve binary aynı positional I/O,
tekrar kullanılan byte tamponu ve her okumada CRC/identity kontrolünü kullanır.
Metadata/UTF-8 doğrulaması binary için de her adayda sürer. Mutex payload
tamponunu korur; eşzamanlı sorgu throughput'u bu deneyin kapsamı değildir.

Original SEG03 ve JSON kaynak dosyaları korunur. Geçici payload ek dosyadır;
disk büyümesi ve tam bağlama süresi gerçek maliyet olarak ölçülür, kaynak/cache
yerine geçmediği için kalıcı binary format tasarımı veya disk tasarrufu sonucu
çıkarılamaz. Motor stats bu ek export'u içermez; benchmark ayrıca
`overlay_bytes`, `attach_ms` ve `total_ready_ms` kaydeder. Bozuk overlay ve tam
belge uyuşmazlığı reddedilir, normal reopen otomatik olarak JSON yolunu kullanır.

[Gerçek motor sonuçları ve reddedilen geçici overlay kapısı](ENGINE-BINARY-VALIDATION-20261011.md).
