# SQLite / FDB bellek farkının kök nedeni

FDB'nin düşük RAM hedefi çözülmedi. Kök neden, yazma sırasında tam belgelerin,
katlanmış arama metinlerinin, RAM indeksinin ve bakım çalışma alanlarının
birlikte yaşaması; bunların ortak bir bellek sınırıyla yönetilmemesidir.
Boşta da segmentlerin terim dizinleri bellekte kalır. Binary payload bu çalışma
modelini değiştirmez. Son mesajdaki sıcak belge önbelleği önerisi bu nedenlerin
tamamını hedeflemiyordu; ölçülmüş çözüm veya uygulanacak ilk adım sayılmaz.

Bu çalışma motor kodunu değiştirmeden yapıldı. Aynı mevcut corpus'un ilk
10.000 kaydı CRC ve key/version kontrolüyle okundu. Mevcut sabit FDB/SQLite
veritabanları yalnız incelendi; corpus, veritabanı veya yeni ingest üretilmedi.
[Makine tarafından hesaplanan bulgular](benchmarks/root-cause-memory-10k-20261014.json).

## SQLite bu ölçümde ne yapıyor?

Comparator `cache_size=-2048`, `mmap_size=0`, `temp_store=FILE` kullanır;
WAL ve synchronous FULL açıktır. Mevcut DB'nin sayfa boyutu 4096 byte'tır.
Sayfa cache'i yaklaşık 2 MiB için ayarlanır; SQLite ihtiyaç halinde sayfa
ayırır ve kirli sayfaları işlem bitmeden de diske aktarabilir. Bu değer tüm
proses için sert bir RAM limiti değildir.
[SQLite cache_size / cache_spill açıklaması](https://sqlite.org/pragma.html#pragma_cache_size).

Comparator'ın kullandığı libsqlite3-sys 0.38.2 kaynak kodunda FTS5 pending
indeks eşiği `FTS5_DEFAULT_HASHSIZE = 1024*1024` olarak tanımlıdır. Bir sonraki
kayda başlanırken `nPendingData > nHashSize` ise hash diske flush edilir.
Bu yaklaşık indeks boyutu eşiğidir; tek kayıt aşabilir, tüm heap kotası değildir.
Yerel kaynak: `C:/Users/Serkan/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/libsqlite3-sys-0.38.2/sqlite3/sqlite3.c`,
245604 ve 257373–257386. Benchmark logundaki SQLite sürümü 3.53.2'dir.

FTS5 terimler ve eşleşmeleri disk segmentlerine/sayfalarına yerleştirir;
sözlük ve belge listelerinde prefix/delta/varint kodlama kullanır. Bellekte
bütün segmentlerin terimlerini ayrı String ağaçlarına açması gerekmez.
[FTS5 segment ve doclist biçimi](https://sqlite.org/fts5.html#fts5_data_structures).
Dolayısıyla küçük proses RAM'i veriyi veya indeksi tutmadığı anlamına gelmez;
esas saklama yeri disk, RAM ise çalışılan sayfalar ve geçici indeks verisidir.

Comparator 500 kaydı tek transaction içinde **tek tek** okuyup insert eder.
FDB aracı ise 500 owned Document'ı önce bir Vec'e doldurur. İşlem sayısı ve
kalıcılık ayarlarının aynı olması caller RAM kullanımını eşitlemez.
Kod: `tools/native_sqlite/src/bin/content_memory.rs:110` ve
`examples/measure_content_memory.rs:65`.

## FDB'deki eşik toplam RAM'i sınırlamıyor

`src/lib.rs:123` varsayılanı 16 MiB'dır. `src/engine.rs:1387` bu sayaçta
native payload'ın kodlanmış boyutunu toplar. Folded metin, posting'ler,
BTreeMap düğümleri, mevcut disk dizinleri, gelen batch ve bakım buffer'ları
bu sayıya dahil değildir.

`needs_flush` mevcut delta için kontrol edilir; gelen batch delta'ya
uygulanmadan önce eşik aşılmamışsa bütün batch kabul edilir. Flush ancak
sonraki commit'te veya açık flush çağrısında gelir. Bu mevcut sözleşmede
bir batch aşımıdır; 16 MiB fiziksel bellek garantisi değildir.

Aynı corpus'ta `payload::encoded_len` formülüyle hesaplanan ilk iki batch
27.043.900 B, yani 25,79 MiB payload taşır. Üçüncü batch öncesi flush edilir.
On bin kayıt boyunca commit içindeki dokuz flush'ın her biri 1000 belge
birikmişken tetiklenir; flush öncesi payload 25.948.677–27.211.658 B'dır.
Son 1000 belgenin payload'ı 27.273.538 B, yani 26,01 MiB'dır.

Üstelik 500 belgenin yalnız title+text içeriği batch başına **12,20–13,08 MiB**.
String kapasitesi, fields ve anahtarlar buna dahil değildir. Bu owned Vec
araca verilmişken bütün prosesin 8–10 MiB olması mümkün değildir. Küçük
proses hedefi için API/testin girdiyi biriktirme biçimi de ele alınmalıdır.
Bu caller farkı, motorun kendi kopyalarını ve dizin maliyetini açıklayıp
mazur göstermez; onlar ayrıca büyüktür.

## Aynı allocation snapshot'ındaki büyük paylar

Korunan tanılama logu
`benchmarks/ram-lifetime-origins-20261013-fdb-ingest-10000-0.jsonl`,
`records=10000` snapshot'ı aşağıdaki requested-live byte'ları içerir.
Bu koşu WAL tamponu yaşam süresi düzeltmesinden sonra, nested delta sözlüğü
değişikliğinden öncedir; bugünkü motorun yeni allocation profili değildir.
Merge-postings+delta-bitmaps açıktır, diğer yazma deneyleri kapalıdır.

| Allocation kökeni | Aynı snapshot'ta canlı MiB | Açıklama |
|---|---:|---|
| caller_documents | 27,345 | Girdide oluşturulup delta'ya taşınan tam belgeler ve örnek kontrol belgeleri |
| delta_index | 29,482 | Folded metinler, gram/posting/slot tabloları ve küçük body cache |
| packed_metadata | 12,514 | Açılmış disk segmentlerinin anahtar/terim/konum dizinleri ve ilgili tahsisler |
| delta_tables | 0,610 | Delta/shadow map tahsisleri |
| other + source_batch | 0,065 | Bu snapshot'taki kalan etiketler |
| Toplam requested-live | 70,016 | Aynı snapshot; farklı origin peak'lerinin toplamı değil |

`caller_documents` allocation'ın doğduğu yeri belirtir. Belge motorun docs
tablosuna taşınınca etiketi değişmez; 27,345 MiB'nin tamamı dışarıda bekleyen
caller batch diye sunulamaz. Buna karşılık sonraki batch hazırlanırken eski
delta belgeleri de yaşar; bu origin'in ayrı tepe değeri 41,065 MiB'dır.

Delta indeksi owned belgeden ayrı `format!("{title}\n{}", body.folded)` String'i
tutar (`src/index.rs:494`). Kaynak title/text `Database.docs` içinde kalır
(`src/engine.rs:227`). Böylece aynı içerik hem özgün hem katlanmış biçimde
yaşar. Küçük body cache bu çeşitli uzun corpus'u tekilleştirmez.

Bu allocator OS private/working set değildir; allocator bookkeeping,
fragmentation, stack ve sistem file cache'i içermez. Diagnostic realloc
replacement ayırırken eski allocation'ı da kısa süre yaşatır. Tanılama
süreleri normal performans sonucu olarak kullanılamaz; snapshot sayaçları
eşzamanlı worker nedeniyle yaklaşık gözlemdir. Bu tablo 110 MiB proses
tepesinin her byte'ını kesin sınıflandırdığımız anlamına gelmez.

## Boşta da disk sözlüğü RAM'de kalıyor

Bugünkü sabit FDB slotunun generation31 manifestinde beş canlı run vardır.
Footer/manifest CRC ve sınırları kontrol edilerek dizinleri okundu:

- Toplam **103.667 terim girdisi** RAM'e açılacak biçimdedir.
- Bunların **60.635'i Gram**; beş run toplamında yalnız **12.147 farklı Gram**
  vardır. Ortak gramlar segment başına ayrı dizin girdisiyle tutulur.
- Ayrıca 19.864 Exact, 19.824 Numeric ve 3334 Link girdisi vardır.
- `DiskIndex.postings` tüm terimlerin offset/count metadata'sını
  `BTreeMap<Term, Posting>` içinde tutar (`src/segment.rs:656`). Posting
  listelerinin kendileri disktedir; RAM'e tüm posting listeleri yükleniyor
  iddiası yapılmaz.
- Normal open önce JSON Directory'nin byte tamponunu ve owned dizinini açar,
  sonra packed kayıtlar/BTreeMap'e dönüştürür (`src/segment.rs:877`). Geçici
  ve kalıcı metadata yaşam süreleri bu aşamada örtüşebilir.

Yukarıdaki eski diagnostic koşuda bakım bitip delta boşaldığında da
packed_metadata origin'i **13.121 MiB requested-live** tutmaktadır. Bu normal
yoldaki pinned metadata'yı hedeflemeden 8–10 MiB proses hedefini yalnız belge
kopyasını azaltarak yakalamak mümkün değildir.

Karşılaştırmanın kapsam farkı da açık olmalıdır: FDB bütün scalar fields için
eşitlik, i64 için sayısal ve links için ters bağlantı indeksleri üretir.
SQLite comparator yalnız composite key ve FTS indeksi oluşturur; field/link
sorguları bu benchmark'ta karşılaştırılmaz. Bu ek indekslerin bedeli FDB
ölçümündedir. Aynı veri ve contains sonuçları, bütün indeks işlerinin eşdeğer
olduğu anlamına gelmez.

## Bakım neden ayrıca tepe yapıyor?

Normal flush `get` üzerinden delta belgesini clone eder; source batch'leri
yaklaşık 8 MiB biriktirip frame olarak kodlar. Eski delta, yeni caller batch,
source kopyası/encode tamponu ve yeni indeks dizini birlikte yaşayabilir.
Background merge aynı anda bir sonraki commit ile çalışabilir; ayrı küçük
builder kotası bütün bu allocation'ları kapsamaz.

Eski tanılama koşusunun ayrı origin peak'leri source_batch16,705 MiB ve
frame_encode13,213 MiB'dır. Bunlar snapshot tablosuna eklenerek toplam tepe
hesaplanmaz; aynı anda gerçekleştiği ölçülmedi. Bakımın güncel OS tepesini
hangi allocation/free anının oluşturduğu henüz tam zaman çizgisiyle ayrılmadı.

[Bütçeli kombinasyonun raporu](BUDGET-MEASURE-10K-20261013.md) mevcut pool'ların
delta, caller, folded metin ve bazı metadata tahsislerini kapsamadığını
gösterir. Pool açmak ortak toplam bellek sınırı oluşturmadı.

## WAL referansı deneyi neden çözüm olmadı?

[Ayrı üç çiftteki deney](COPY-REDUCTION-10K-20261013.md) tam belgeyi RAM'den
kaldırınca indexing private111,13→83,87 MiB yaptı. Folded metin, RAM posting'leri
ve disk sözlüğü kaldı. Üstelik döndürülecek her belge için WAL read+CRC+decode
eklendi; pozitif delta arama senaryolarının16/16'sı üç çiftte de yavaşladı.
Bu deney belge kopyasının önemli olduğunu destekler; bütün farkın tek nedeni
olduğunu veya referans tasarımının hız şartını geçtiğini göstermez.

## Bu bulguların belirlediği yön

Önce kapsanan bellek ve yaşam süreleri tanımlanmalıdır: caller batch, özgün
payload, folded delta metni/posting'ler, pinned disk dizinleri ve foreground /
background bakım. Yeni bütçe bunlardan yalnız bir output buffer'ını saymamalı.

İlk tasarım seçenekleri bounded giriş/transaction staging, delta'nın gerçek
metin+indeks büyüklüğünü kapsayan eşik ve segment terim dizinini küçük sayfalı
bir cache üzerinden okuma olmalıdır. Tam belgeyi ayrı String ağacı yerine
tek kompakt payload içinde kullanabilme bu sınıra dahil değerlendirilmelidir.
Eşik küçültmek flush/merge sıklığını artırabilir; bunun hızı ayrıca ölçülmelidir.
WAL referansı deneyi tam belgeyi her istekte diskten okumanın bu iş yükünde
arama maliyeti eklediğini gösterdi. Hız korunmuş sayılmadan bütünleştirilmez.
Sıcak belge cache'i tek başına kök nedeni çözmez.

Her değişiklik transaction atomicity, synced WAL, kaynak doğrulaması ve
version/update/delete davranışını korumalıdır. Yeni kalıcı format varsa ayrı
sürüm ve uyumluluk testi gerekir. Bu rapor uygulama veya performans kazancı
değildir; önceki bellek kök nedeni talebinin kanıtlı teşhisidir.

Yeni ingest/arama ölçümü yapılmadı; mevcut ilk10k, sıcak OS cache geçmiş
logları kullanıldı. Soğuk cache, eşzamanlı bakım yükü ve tam Windows heap
attribution hâlâ ölçülmedi. Güncel özel bellek ~110 MiB bandı ile eski SQLite
~8,3 MiB ölçümünden bu tur yeni paired oran üretilmez. SQLite disk incelemesi
Python SQLite3.53.1 read-only immutable bağlantısıdır; comparator timing'i
3.53.2 idi. Disk metadata incelemesi full source integrity kontrolü yerine geçmez.

[OPEN-WORK](OPEN-WORK.md): RAM çözümü, yazma/bakım iyileştirmesi, diskte eski
nesiller ve eşzamanlı yük açıktır. Önceki arama dalgalanmaları kullanıcı
kararıyla kapalıdır; WAL referansının yeni sistematik yavaşlaması reddedilmiş
deney olarak kalır. Motor, kaynaklar ve 0.1 fixture değiştirilmedi.

Bu rapor turunda `cargo fmt --check` ve
`cargo clippy --all-targets -- -D warnings` geçti. Kod değişmediği için yeni
test/ingest koşusu yapılmadı. Loglar
`benchmarks/root-cause-memory-20261014-{fmt,clippy}.log` altında tutulur.
