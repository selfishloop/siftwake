# Terim cache kilidi — Ek-TC son sınırlı deney tasarımı

3 Kasım adlandırma düzeltmesi: bu, 2a görünürlük takip deneyi Ek-TC'dir.
Brifteki 2a–2c'nin tamamının veya 3a kapısının kapanışı değildir.
[Brife göre durum](BRIEF-BLOCK-STATUS-20261103.md). Tarihsel kapılar/sonuçlar aynı.

2 Kasım 2026. **Yalnız kod incelemesi ve tasarım; uygulama ve yeni ölçüm yok.**
Bu cümle ilk tasarım turunun durumudur. Sonraki kullanıcı onayıyla A uygulamasına
başlandı ve yalnız kapasite kontrolü D eklendi; aşağıdaki ilk tasarım tarihsel
olarak korunur. Güncel sabit ölçüm sırası:

|Blok|1|2|3|4|
|---|---|---|---|---|
|1|K|A|SQLite|D|
|2|A|D|K|SQLite|
|3|D|SQLite|A|K|
|4|SQLite|K|D|A|

K kabul edilmiş try/seri ve 960 KiB sayfa; A yalnız tek decoder ile kilit dışı
yükleme; D K ile aynı yol ve **4 MiB sayfa**, descriptor yine64 KiB. Her kol her
pozisyonda bir kez,12 yönlü ardışık çiftin her biri bir kez. Normal41 ve havuz
görünürlük fazları ayrı, aynı matris. Ortak bounded eviction/owner sayaçları ve
tanı ölçüm altyapısı üç FDB kolunda aynı. A/D tanı anahtarları opt-in; K default
kalır. Sonuçta varsayılan seçimi kullanıcıya ait. B/C uygulanmaz ve yeni tur yok.
[VISIBILITY-PHASE tanısı](VISIBILITY-PHASE-50K-20261102.md) kullanıcı tarafından
kabul edildi. Aşağıdaki adaylardan yalnız biri, kullanıcı onayından sonra tek
deney bloğunda değerlendirilebilir. Sonuca göre yeni aday/ayar turu açılmaz.

## Koddan doğrulanan kilit kapsamı

Kaynaklar: [term_pages.rs](../src/segment/term_pages.rs),
[TrackedMutex](../src/access.rs), [snapshot yayımı](../src/engine/snapshot.rs),
[Run sahipliği](../src/engine.rs), [çalışma bütçesi](../src/write_budget.rs),
[posting hazırlığı](../src/segment/first.rs),
[SEG05 decoder](../src/segment/compact.rs).

|Yol|Kilit ve yaptığı iş|
|---|---|
|`cache()` / `query_cache_lock()`|Süreçte tek `OnceLock<TrackedMutex<Cache>>`. Bütün DB/segment owner'ları aynı mutex'i kullanır. Query span edinme süresini, genel `lock.cache_wait` yalnız contended beklemeyi ölçer; sayaçlar örtüşebilir.|
|`Pages::get()`|Bloom reddi kilitten önce. Descriptor lookup `(owner, Term)` altında kilit alır; hit'te LRU stamp günceller ve `Option<Posting>` kopyalar. Miss sonrası fence bulur, `with_page` çağırır; sonra `remember` için yeniden kilit alır. Fence dışında reddedilen miss descriptor cache'e yazılmaz.|
|`with_page()` hit|Tek guard altında `VecDeque` içinde `(owner,page)` arar, entry'yi çıkarır, callback'i çalıştırır ve başa koyar. Yalnız lookup değil callback de guard içindedir.|
|`with_page()` miss|**Aynı guard** altında tahmini boyuta göre eski sayfaları atar, `decode()` ile dosyayı okur, CRC ve decode/kimlik/sıra kontrollerini yapar; callback, gerçek boyuta göre tekrar tahliye ve cache'e ekleme de aynı guard içindedir.|
|`Cache::remember()`|64 KiB descriptor sınırında en eski stamp'i bütün lookup map'ini gezerek bulur. Negatif `None` sonucu da cache'lenebilir. Bu dolaşım da guard içindedir; ayrı maliyeti ölçülmedi.|
|`Pages::drop()`|Aynı mutex ile yalnız kendi owner'ının sayfalarını/descriptor'larını kaldırır.|
|`Pages::cursor().next()`|Bakım/term cursor'ı `decode()` çağırır; bu sayfa cache'ini kullanmaz. Dolayısıyla bütün decode trafiği `with_page` istatistiği değildir.|

`decode()` fence checksum'u ile okunan baytları karşılaştırır; SEG05 veya eski
JSON decoder'ını çalıştırır; count, ilk/son terim kimliği ve sıkı terim sırasını
kontrol eder. Öneriler bu kontrolleri veya kaynak/indeks strict doğrulamasını
atlamaz. Hit yolu mevcut davranış gibi daha önce doğrulanmış volatile içeriği
kullanır; dışarıdan dosya değişikliğini her hit'te yeniden CRC ile tespit ettiği
iddia edilmez.

Önceki ölçümde numeric/Frunner term-cache edinimi 11,501/13,329 ms; page
read/CRC/decode 8,780/9,454 ms. İkisi posting hazırlığı altındadır; toplam wall'a
tekrar eklenmez. Mutex sahibi thread/owner ölçülmedi. Daha kısa kritik bölgenin
toplam gecikmeyi ne kadar düşüreceği henüz **bilinmiyor**.

## Commit cache'i neden soğutuyor gibi görünüyor?

**Her commit cache'i sıfırlar sonucunu kod desteklemiyor.**

- Page anahtarı `(Pages.owner, page-index)`; descriptor anahtarı `(Pages.owner,
  Term)`. `owner`, `Pages::new()` içinde süreçte artan sayıdan üretilir. Commit
  sequence, mtime veya manifest generation bu cache anahtarında yoktur.
- `Run` içindeki `Arc<DiskIndex>`, `snapshot_view()` içinde `runs.clone()` ile
  paylaşılır. Yalnız snapshot yayımı, var olan indeksin owner'ını değiştirmez.
- Yeni açılmış/yapılmış indeks yeni `Pages` ve owner alır. Flush/merge çıktısı
  bu nedenle kendi soğuk namespace'inden başlar; değişmeyen run'lar aynı owner'ı
  kullanır. Aynı dosyayı yeniden açmak da yeni owner üretir. Bu, güvenli izolasyon
  mekanizmasıdır; owner'ı commit sequence veya yalnız dosya yoluyla değiştirmeyiz.
- Resident page sınırı **960 KiB**, descriptor sınırı **64 KiB**; toplam sayılan
  resident içerik **1 MiB**. Bütün owner'lar/okuyucular bu kapasiteyi paylaşır.
  Page LRU ve descriptor stamp tahliyesi kodda var. Son indeks referansı düşünce
  `Drop` yalnız o owner'ın girdilerini temizler; eski snapshot referansı owner'ın
  ve dosyasının ömrünü korur.
- 415–448 lookup miss / 14–15 hit, **descriptor cache miss/hit** sayısıdır;
  terimin indekste yokluğu veya sorgu yanlış-pozitif oranı değildir. 306–331 page
  miss ayrı sayaçtır. Bloom/fence reddi ve cache dışı cursor decode'u da ayrıdır.

**Bilinmiyor:** önceki commit-sonrası miss'lerin yeni owner / kapasite tahliyesi /
o terime ilk erişim / cache'e sığmayan giriş dağılımı. Önce/sonra owner bazlı
cache durumu ve eviction nedeni kaydı yok. Çok run/okuyucu paylaşımı tahliyeye
izin verir; bunun gözlenen miss'lerin baskın nedeni olduğu ölçülmedi. Salt
14/448 oranından “commit cache'i soğutuyor” veya “cache büyütülmeli” sonucu çıkmaz.

Mevcut muhasebenin sınırı: `Cache.bytes` decoded Vec ve string kapasitelerini,
lookup hesabı muhafazakâr map/node payını sayar; allocator, VecDeque overhead'i,
decode raw/scratch ve cursor'lar bu 1 MiB sayacıyla tam proses belleği olmaz.
Sayfa hedefi 16 KiB/128 terimdir; tek uzun scalar bu bayt hedefini aşabilir.
Mevcut `len*2 + next_power_of_two(count)*sizeof(...)` **tahliye tahminidir**,
kanıtlanmış bütün-allocation üst sınırı değildir. `WORKING_POOL` 16 MiB içinde
2 MiB cache/scratch baseline ayırır; bunun tamamı boş veya bu cache'e ait değildir
(ayrı paged-metadata cache'i de 256 KiB). Bu baseline'a dayanıp dört decoder'a
bedava buffer ayrılamaz.

## Üç tasarım adayı

Ortak koşullar: aynı owner kimliği; readonly dosya handle'ı ve aynı fence;
strict CRC/count/identity/order; exact/serial ve capped sonucu/anahtar sırası;
snapshot sequence/pin, cancel ve global worker/bellek sınırları korunur.
Yeni arka plan thread'i, cache boyutu artırımı veya kalıcı format yok.

### A — Kısa metadata kilidi, read/decode kilit dışında (önerilen tek aday)

1. Metadata kilidinde page hit'i kontrol et; hit callback'i kısa kapsamda aynen
   çalışsın. Miss'te guard'ı bırak.
2. **Tek global cache-miss decode slotu** al; cache kilidini tutarak slot/bütçe
   bekleme. Slot alındıktan sonra cache'i yeniden kontrol et; arada yüklenmişse
   ikinci decode yapma. Slot kilidi ile cache kilidi için tek yönlü sıra olsun;
   hit/Drop slot beklemesin. Yeni worker izni tüketilmez/yeni thread açılmaz.
3. Byte rezervasyonu kesinleşince read/CRC/decode'u cache mutex'i dışında yap.
   Callback bu yerel immutable sonuç üzerinde çalışsın. Kısa metadata kilidinde
   doğrulanmış sonucu yayımla. Slot/reservation bütün error/cancel/unwind
   yollarında RAII ile bırakılır. Aynı anda en fazla bir yeni page decode edilir.

**Beklenen etki, ölçülmemiş:** miss okuması sürerken başka sayfalardaki hit'ler ve
lookup'lar global cache mutex'inde I/O'yu beklemez. Farklı sayfalardaki miss'ler
decode slotunda hâlâ sıraya girer. Dolayısıyla ölçülen 13 ms'nin tamamını
kaldıracağı veya 4R yükünü hızlandıracağı söylenemez. Slot beklemesi ayrıca
ölçülür; cache beklemesini yalnız başka sayaç altına taşımak başarı değildir.

**Bellek / üst sınır:** resident+in-flight içerik için mevcut 960 KiB page
allowance'ını paylaş; 64 KiB lookup allowance'ını büyütme. Raw buffer, decoded
Vec/string kapasitesi ve geçişte aynı anda yaşayan içerik rezervasyonda sayılır;
resident sayfaları gerekirse decode öncesi tahliye et. SEG05 için raw uzunluğu,
count ve string uzunluklarının sınırlarına dayanan checked üst sınır çıkarılmalı;
mevcut tahliye tahmini yeterli sayılmaz. Metadata/slot overhead'i de ayrılan
paydan düşülür. Sayacı ancak allocation'dan sonra kontrol etmek kabul edilmez.
Sığmayan/legacy oversized sayfa eski tek-decode seri fallback'te kalır; dört
paralel uncached kopya oluşturulmaz, eski okuma desteği reddedilmez. Bu mevcut
legacy fallback toplam proses için 1 MiB hard-cap değildir; ayrı ölçülür.

**Risk / tutarlılık:** kilit dışında decoded buffer'ın ömrü ve cache'e geçişi,
error'da yarım entry, double-check yarışı, slot/cache kilit sırası. Çağrı boyunca
`&Pages`/snapshot aynı owner'ı pinler; `Drop` o Pages canlıyken olamaz. Cache
dışına uzun ömürlü `Arc<Vec<...>>` verilmez; eviction sonrası gizli pinned cache
birikimi yaratılmaz. Diğer kaynak/metadata kilitleriyle ters sıra test edilir.

**Cancel / boşta RAM:** cache helper bugün token almaz; yeni slot beklemesi
mevcut cancel zincirine bağlanmalı (caller token'ı ulaştırma, bounded condvar
wait ile check; sleep/spin ve sonsuz uncancellable bekleme yok). Tek OS read
ortasındaki iptal hâlâ cooperative; anlık I/O kesme garantisi yok. İptal edilen
çağrı sonuç döndürmez; doğrulama tamamlanmamış entry yayımlanmaz. Geçici buffer
iş bitince bırakılır, resident limit aynı; allocator nedeniyle OS idle private'ın
artmayacağı varsayılmaz, ölçülür. Baseline/WORKERS limitleri artırılmaz.

### B — Kilit dışında yükleme + `(owner,page)` başına tek-uçuş

Metadata'da `Loading / Ready / Failed` state ve bekleyenler; aynı sayfanın bir
loader'ı olur, farklı sayfalar yalnız **rezerve byte ve slot sınırı** içinde
eşzamanlı yüklenebilir. Bekleyenler condvar'da metadata guard'ını bırakır;
CRC/kimlik/sıra geçmeden Ready olmaz.

- **Risk:** başarısız/iptal edilmiş loader'ın bekleyenleri uyandırmaması,
  owner'lar arasında yanlış Ready paylaşımı, cancel edilmiş bekleyenin diğer
  okuyucuların yüklemesini bozması, yüklenirken eviction ve son referans yarışı.
  Her terminal yolda notify+RAII; cancellation waiter-local; son waiter/loader
  sonrası state temizliği gerekir. Cache error'ı başarılı/negatif hit yapılmaz.
- **Bellek:** Loading state/waiter listesi de sınırlanır; sınırsız map/negative
  cache yok. Toplam resident+in-flight aynı 960 KiB, lookup aynı 64 KiB; slot
  sayısı en fazla 4 ve yeterli byte yoksa daha az. Her sayfa/loader ayrı
  kapasiteyle çarpılmaz; oversized eski seri fallback. Sabit entry/header payı
  cache bütçesinden düşülür. Yeni WORKERS izni/thread/baseline yok.
- **Snapshot/cancel:** owner pin loader ömrünü kapsar; bir bekleyenin cancel'ı
  başka snapshot'ın sonucunu değiştirmez. Beklemeler iptal kontrolü yapar; read
  kesilemez sınırı aynıdır. Boşta Loading/Failed/waiter state sıfırlanır; retained
  map kapasitesi de muhasebede kalır ve ölçülür.
- **Beklenti:** aynı-page miss çoğaltmasını önler, farklı miss'lere paralellik
  verir. Önceki sayaçlar eşzamanlı aynı-page miss sayısını göstermiyor;
  kazanç ve gerekli paralellik **bilinmiyor**. A'dan daha geniş doğruluk yüzeyi.

### C — Sabit 4 parçalı cache kilidi

Owner+page hash'i sayfa shard'ını seçer; descriptor için owner+term hash'i.
Bir lookup sırasında iki shard guard'ı birlikte tutulmaz. Page miss read/decode
şimdilik kendi shard guard'ı içinde kalabilir; diğer shard hit'leri ilerler.

- **Risk:** descriptor ve sayfanın farklı shard'a gitmesi, Drop'ta çoklu kilit
  sırası, owner izolasyonu, shard'lar arası kapasite kayması ve boşa düşen yer.
  Drop shard'ları tek tek alır; cross-shard callback/ikili guard yasak. Her
  shard aynı CRC/kimlik/sıra doğrulamasını korur.
- **Bellek:** toplam 960 KiB page +64 KiB lookup sabit; 4'e bölünür, her shard'a
  1 MiB verilmez. Dört in-flight decode'un raw/scratch toplamı ayrıca rezerveli
  ve aynı toplam sınırın içindedir; yalnız shard kilidi yeterli muhasebe değildir.
  Mutex/map overhead'i bütçeden düşer. Skew nedeniyle bir shard boşken diğeri
  tahliye yapabilir; daha çok miss/CPU üretme riski vardır.
- **Snapshot/cancel/idle:** key yine owner'a bağlı, pin semantiği aynı. Kendi
  shard'ında read bekleyen iptalin cooperative sınırı sürer; yeni iptalsiz
  bekleme eklenmez. Boşta dört header'ın maliyeti vardır; OS RAM etkisi ölçülür.
- **Beklenti:** hangi sayfalarda/shard'larda çekişildiği bilinmediği için 4'e
  bölünmenin hızlanması garanti değildir. Mevcut decode'u guard altında tutar;
  kök kritik bölgeyi A kadar doğrudan küçültmez.

**Öneri:** yalnız A. Tek decoder sınırı miss paralelliğini büyütmeden hit'leri
I/O kilidinden ayırır; bekleme yer değiştirmesi ve tahliye bedeli görünür kalır.
B/C alternatif açıklamadır, A başarısız olursa bu blokta denenmeyecektir.
Byte-bound/legacy fallback güvenle sağlanamıyorsa A da uygulanmaz ve sonuç
“uygulanamadı” diye yazılarak Ek-TC kapatılır. Şu anda onay bekleyen tasarımdır.

## Tek kanıt bloğu — yalnız uygulama onayından sonra

**Veri/yerleşim:** 881756DA… cap200k kaynağının ilk 50k'sı, aynı fixed FDB/SQLite
slotları; kaynağa/private'a dokunma, reset/reingest/yeni DB kopyası yok.
Mevcut kabul edilmiş try/seri kontrolü A, yalnız seçilmiş cache değişikliği B;
aynı kaynak/build ayarları, frozen ikili SHA'ları. Eski bloklayan hibrit rollback
cache kontrolü yerine kullanılmaz. Yerleşim kollar arasında ilerler; aynı fiziksel
durum veya eski 15 ms ile birebir kıyas iddiası kurulmaz.

**Dört dengeli çift:** A / SQLite / B, B / SQLite / A; iki tekrar. SQLite FTS5
trigram ve mevcut ayarlar değişmez. Kontrol ve aday aynı oturumda karşılaştırılır;
eski profilli 30 ms tek başına before kabul edilmez.

İki ayrı yük birbirine karıştırılmaz:

1. **Normal 41 N60:** önceki ESTIMATOR/HYBRID-DEFAULT düzeni aynen. 1R 60 tur,
   4R 15 tur/aktör; idle ve loaded her sorguda en az 60 gerçek örnek. Yüklü
   writer en az 16384 tekli commit, tüm okuyucular 15 aktif turu bitirene kadar
   sürer; mevcut 262144 emniyet sınırı aynı. Her sorguda oracle, zaman sınırı
   dışında. Normal kapı kolunda ayrıntılı cache profili kapalı. p50/p95/max ve
   ham örnekler; p95 nearest-rank `ceil(.95*N)`, kırpma/eleme/bootstrap yok.
2. **Görünürlük:** aynı 257-slot Frunner havuzu/ayrı collection, aynı donör,
   gövde ve 40–300 B yollar + tek 1000 B uç, 4096+1 işlem, aynı 4R beşli havuz
   yükü. Bu da aynı dört A/SQLite/B dengeli sırayla ölçülür. Numeric kollar bu
   son deneyde yeniden açılmaz; eski numeric veri yalnız tarihsel kalır.
   Commit→history→acquire→prepare→ilk marker sorgusu→release sınırları aynı;
   gap işlem başına `visibility - commit`, bağımsız p95'ler çıkarılmaz.
   Profil kapalı wall ana sonuç; mevcut request-local profil ile tanı örnekleri
   ayrı etiketlenir ve normal wall dağılımına karıştırılmaz. Profil kontrol/adayda
   aynı açık/kapalı düzenle alınır; otomatik ek tekrar/iyileştirme turu yok.

Cache neden sayaçları yalnız bu sabit tanı kapsamı içinde: owner yaratma/drop,
commit sequence ve run değişimi, owner/page/term hit-miss, capacity/oversized/drop
eviction nedeni, lock hold/wait, decode-slot wait, read/CRC/decode, tekrar kontrol
hit'i ve bytes resident/in-flight/peak. Descriptor miss ve indekste yokluk ayrı
raporlanır. Sınırsız term/path log'u tutulmaz; bounded sayaç ve sequence örnekleri
profil/model/history belleğinden ayrılır. Bu sayaçların eklenmesi de uygulama
onayına bağlıdır. Bilinmeyen OS-cache/scheduler/lock-owner nedenleri tahminle
doldurulmaz. Mevcut lock span'ları dış wall'a tekrar eklenmez.

**Yan maliyetler:** aynı ölçüm bloğunda reader/writer ops/s, parent+child CPU,
commit p95/p99/p99,9/max ve >100 ms sayısı; aynı 4R+writer prosesinde peak private
ve çalışan decode sayısı/byte tepesi. Boşta motor-only <=15 MiB kapısı aynı;
bu okuma deneyi ingest <=60 MiB kapısını yeniden ölçmüş sayılmaz. Yeni ingest
yapılmaz. Önceki RAM kapısı tarihsel GEÇTİ kalır. Sıcak/kontrolsüz OS cache;
gerçek soğuk cache veya eviction yok. 41 içindeki zx/ve/if/id/case34 kötü sonuçlar
dahil kalır; ayrı zx iyileştirme/ek tarama kampanyası açılmaz.

## Eşikler, başarı ölçütü ve durma

|Kayıt / karar|Aynen korunacak kural|
|---|---|
|Normal 41 okuma kapısı|Her çift/her sorgu için idle p95 <=30 ms ise loaded p95 <=30 ms; aksi halde loaded p95 <=2*idle. Kontrol/aday/SQLite geçti-kaldı adetleri ayrı; pooled sonuç tek kötü sorguyu örtmez.|
|Tekli commit ve görünürlük|Mevcut p95 <=20 ms, p99 <=50 ms, max <=100 ms; görünürlük gap'i ayrıca raporlanır. Eski B KALDI kabul notu yeni ölçümü otomatik geçirmez.|
|Yazma throughput|Okuyucusuz kontrole göre >=%75 ve SQLite pooled korunumu üstü; mutlak ops/s de yazılır, SQLite >%100 kontrolü tavan sayılmaz.|
|Motor-only idle|<=15 MiB. Ingest gate bu tur yeniden çalıştırılmaz. RAM muhasebesi veya benchmark süreç toplamı motor-only diye sunulmaz.|
|Geçmiş kabul notları|Normal41 ve sıfır-toleranslı CPU/geniş tarama/loadedRAM KALDI kayıtları aynen durur; bu tur eşiği revize edilmez, yan maliyetler saklanmaz.|

**Net iyileşme için önerilen karar yöntemi:** dört eşleşmiş çiftin gap p50/p95/max
ve yayımlarını yan yana göster; aynı oturumun kontrolüne göre Frunner gap
p95'inde tutarlı düşüş ve yeni slot beklemesi dahil ilk-query süresinde düşüş
aranır. Kilit sayacının küçülüp toplam gap/CPU'nun değişmemesi kazanım değildir.
Yeni yüzde/ms performans eşiği icat edilmez. Karışık yönlü/örtüşen sonuçta
“net iyileşme kanıtlanmadı” yazılır; en iyi çift seçilmez. Kapıların geçmesi
ve ölçülmüş hızlanma iki ayrı sütundur. Kazanç varsa bile varsayılan yapmak
için sonuç kullanıcıya sunulur; bu tasarım varsayılan değişikliği onayı değildir.

**Doğruluk zorunlu:** aynı owner/same-page eşzamanlı miss; farklı owner'da aynı
page/term; cache baskısı ve oversized legacy; decode/read/CRC/sıra hatası; error,
cancel ve unwind'da slot/byte bırakılması; slot beklerken/okuma sonrasında cancel;
eski snapshot pin'i, yeni sequence ve güncelleme/silme; N'in iki yanı; normal
exact/capped oracle bit eşitliği; strict-open, process-kill/reopen ve eski okuyucu.
Kaynak bozukluğu sağlam cache ile kabul edilmez. Test/debug state-machine ve
kilit-sırası testleri; default/diagnostic test, fmt, clippy -D warnings tekrar
geçer. Kill'in gerçekten kapsadığı aşama belirtilir; güç kesintisi iddiası yok.

**DURMA KURALI:** oracle/CRC/pin/cancel/kill/eski okuyucu bozulursa hemen dur;
o kolun performansını kabul sonucu yapma. Doğruluk geçerse önceden sabitlenmiş
dört çift bloğu tamamla, iyi/kötü bütün örnekleri raporla; yeni ölçüm turu yok.
Herhangi bir performans kapısı kalırsa veya net iyileşme kanıtlanmazsa mevcut
varsayılan korunur, deney reddedilir ve **Ek-TC kapanır**. A'dan B/C'ye geçme,
eşik/örnek düzenini değiştirme, yeni veriyle sonuç arama. Başarılı olsa da bu son
sınırlı deneyden sonra dur ve kullanıcıya raporla; otomatik sonraki iş yok.

## Bu tur kapanışı

Yalnız bu belge ve OPEN-WORK güncellendi; engine/benchmark kodu, DB ve corpus
değiştirilmedi. Yeni test/benchmark çalıştırılmadı; önceki testler yeni adayın
doğrulaması diye sunulmadı. Uygulama için kullanıcı kararı bekleniyor.

100'lük commit, C/500 drain, 2,6 sn uç, WAL handle/File::open, zx düzeltmesi,
reclaim varsayılanı, C ABI yok. Diğer yazma/bakım, geniş tarama/case34,
RAM/disk/eşzamanlı yük ve signed-zero açık kayıtları korunur.
