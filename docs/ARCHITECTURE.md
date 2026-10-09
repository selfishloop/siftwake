# FDB mimarisi — 0.3.1

## Deneysel commit sync kontrolü

`experimental-wal-sync-control` altında `Database::set_wal_sync(false)` yalnız
commit append sonundaki sync_all'ı atlar. WAL byte/CRC/framing, atomik batch,
checkpoint/source/manifest kalıcılık yolu değişmez. Her yeni/reopened DB true
başlar; CLI opt-in sunmaz. Tekrar true yapmak pending WAL byte'larını sync eder,
hata motoru poisoned yapar. Sistem/elektrik kesintisinde sync-off onayları
kaybolabilir; process-kill testi OS cache'ini ayakta tuttuğundan bunu kanıtlamaz.
[Ayrı commit ve bakım dahil ölçüm](WAL-SYNC-VALIDATION.md).

## Posting oluşturucu kotası

`experimental-builder-budget` posting spill tahmini tamponuna oluşturucu başına
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

## Deneysel merge posting yeniden kullanımı

`experimental-merge-postings` write-buffer-budget/streaming-metadata yollarını
kullanır. Worker önce input kaynak prefix checksum ve immutable length'i,
SEG03 bütün dosya checksum/footer'ını doğrular. Legacy kaynak WAL append'i
nedeniyle checkpoint prefix'inden uzun olabilir; yalnız published prefix bağlanır.
MergeKeys en yeni kayıtları seçer; her input için u32 old→new slot map kurulur.
Tombstone/eski sürüm u32::MAX ile dışarıda kalır, output tombstone slotları korunur.

Kaynak checkpoint mevcut codec ile tamamlanır. Bu kaynak akışı Builder'a
payload/record olarak eklenir; terms/grams üretimi yapılmaz. Input term dictionary
iterator'ları heap ile, her eş term'in posting'leri 8 KiB/input penceresiyle
birleştirilir. Source ID sırası, bounds, monotonic remap ve duplicate output ID
kontrol edilir. Boş kalan term atılır. SEG03 directory/footer aynı biçimde yazılır.
Sonra normal open_source bütün authoritative kaynak doğrulamasını tekrar yapar;
worker manifest yayınlamaz. Bu ek JSON doğrulama geçişi hız ölçümüne dahildir.

Map 4 byte/input fiziksel kayıt, root/term dictionaries ve scratch ortak I/O
reservation kotası dışında kalır; toplam heap/RSS bütçesi değildir. Flush indexleri
henüz yeniden üretir, yalnız merge'deki terms üretimi kaldırılmıştır. Varsayılan
kapalıdır. [Doğrulama ve ölçüm](MERGE-POSTINGS-VALIDATION.md).

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

## Deneysel akışlı metadata

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

## Deneysel delta posting yerleşimi

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

## Deneysel checkpoint yazma bütçesi

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

## Deneysel disk metadata yolu

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
