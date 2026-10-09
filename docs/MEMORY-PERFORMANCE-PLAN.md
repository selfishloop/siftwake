# SQLite'tan düşük RAM ve mevcut hızın korunması — tasarım planı

Tarih: 08.10.2026. Durum: önerilen tasarım ve kabul ölçütleri; uygulanmış veya
ölçülmüş yeni bir motor sürümü değildir. Bu çalışma bağımsız FDB motoruyla
sınırlıdır. SDB/Fundops entegrasyonu ve hazır DB/arama motoru kullanımı yoktur.

Uygulama başladı: native karşılaştırma, allocation/layout profili ve ilk sayfa
sorgu prototipi için [aşama 1 doğrulaması](MEMORY-STAGE1-VALIDATION.md).
Sayfalı metadata yolu [deneysel olarak uygulandı](PAGED-METADATA-VALIDATION.md).
Checkpoint/dizin açılışı ve serileştirme için
[sonraki doğrulama](STREAMING-MEMORY-VALIDATION.md) ayrı tutulur. Token sözlüğünün
diskte sayfalanması, ortak hard allocation bütçesi ve akışlı büyük veri API'si
henüz uygulanmadı. Aşağıdaki bütçeler kabul hedefidir, uygulanmış garanti değildir.

## Karar

Kayıt sayısıyla büyüyen anahtar, kayıt konumu ve token dizinlerini RAM'de tutan
tasarımdan, disk üzerinde sayfalı indeks ve **motor genelinde tek bellek bütçesi**
kullanan tasarıma geçmek gerekiyor. Daha küçük String/BTreeMap kullanmak tek
başına yeterli değildir. Hız kazancı, daha az allocation, daha az JSON çözme,
daha küçük indeks ve sıralı blok erişiminden gelmelidir.

Bütün iş yüklerinde daha az RAM ve daha fazla hız önceden garanti edilemez.
Özellikle küçük cache ile soğuk, rastgele erişim bunu zorlaştırır. Bu nedenle
aşağıdaki adımların her biri ölçüm kapısından geçer; tekrarlanabilir hız kaybı
veren değişiklik hedefi karşılamış kabul edilmez.

## Bugünkü kanıt ve bellek nedenleri

[0.3.1 doğrulaması](VALIDATION-0.3.1.md): bir milyon küçük kayıt, tekrarlı 256
bayt Türkçe içerik, 500 kayıtlık batch, sıcak yerel OS/file cache. FDB ingest
108,086 saniye, son bakım dahil 115,664 saniye; tepe working set 162,22 MiB.
Yeni açılmış CLI boşta 58,81 MiB, açılış tepe working set 86,67 MiB.
Karşılaştırılan Python + SQLite prosesinin ingest'i 137,404 saniye ve gözlenen
tepe working set'i 20,21 MiB idi. Bu değerler farklı ölçüm aşamalarını gösterir;
boşta bellek ile ingest tepesini birbirinin yerine kullanmayacağız.

Koddan doğrulanan büyüme kaynakları:

| Yapı | Mevcut maliyet | Tasarım değişikliği |
| --- | --- | --- |
| `DiskIndex.records`, `ids`, `postings` | Tüm kayıt konumları/ID'ler/token sözlüğü RAM'de | Diskte sayfalı tablo ve sözlük; ihtiyaç duyulan sayfalar cache'te |
| SEG03 açılışı ve builder | Bütün JSON dizini, çözülmüş kayıtlar ve paketlenmiş temsil geçici olarak üst üste gelebiliyor | İndeks sayfalarını akışla üretme/okuma |
| Disk sorgusu eşleşmeleri | Tüm sonuçlar `Vec<u32>`; bir milyon ID en az 3,81 MiB, kapasite ayrıca | Akışlı posting iterator'ları ve sayfa için sınırlı referanslar |
| Görünürlük/gölgeleme | Run başına N/8 maske; kesişen yeni run'ların anahtar taraması | Diskte türetilmiş görünürlük blokları ve küçük WAL/delta overlay'i |
| RAM delta | Doküman nesneleri, anahtarlar, ağaç düğümleri ve Arc posting referansları | WAL konumları, run/delta içinde u32 ID, bütçeli arena |
| Flush/merge | Kayıt/token dizinleri, JSON batch'leri, spill listeleri ve worker aynı anda bellek kullanıyor | Ortak rezervasyon, akışlı builder, sınırlı merge fan-in |
| Protokol/kurtarma | 96 MiB istek satırı, 64 MiB frame ve materialize edilen sonuçlar | Akışlı giriş, replay ve çıkış; büyük veriyi geçici diskte staging |

Bu tablo allocation profili değildir. Her yapının gerçek kapasite, allocator
ve geçici yaşam süresi maliyeti ilk uygulama adımında ayrıca ölçülecek.

## İlk bellek bütçesi

Bir milyon küçük kayıtlı mevcut corpus için başlangıç tasarım bütçesi:

| Alan | Birlikte izin verilen motor allocation bütçesi |
| --- | --- |
| Sayfa cache'i; metadata ve pin takibi dahil | 2 MiB |
| Aktif delta anahtar/posting arena'sı | 2 MiB |
| Flush + background merge + spill buffer'ları toplam | 2 MiB |
| Sorgu iterator, decode, metin doğrulama buffer'ları | 1 MiB |
| Segment tanımları ve diğer motor buffer'ları | 0,5 MiB |
| Toplam başlangıç bütçesi | 7,5 MiB |

Bu rakam **toplam proses RAM'i değildir**. Executable/runtime, thread stack'leri,
allocator boşlukları, giriş/çıkış ve JSON nesneleri de ölçülecek. Aynı küçük
corpus'ta ilk deney hedefi 12–16 MiB tepe working set'tir; henüz ölçülmüş değildir.
Başarı için asıl eşik, aynı harness ile ölçülen native SQLite prosesinden düşük
working set ve private committed bytes kullanımıdır. Native SQLite daha düşük
çıkarsa 12–16 MiB tek başına başarı sayılmaz; bütçe yeniden ayarlanır.

**Native ölçüm sonrası revizyon:** aynı küçük corpus'ta SQLite 3.53.2'nin
20 ms istenen örnekleme aralığıyla gözlenen proses tepe working set'i 9,48 MiB,
örneklenen private committed tepe miktarı 5,22 MiB oldu. 12–16 MiB artık kabul
hedefi değildir. İlk yeni proses deney hedefi 8–9 MiB working set ve native
SQLite'ın altında private bytes; motor allocation bütçesi de başlangıç için
2,5–3 MiB düzeyine çekilerek prototiplenmeli. Örneğin 1 MiB sayfa cache'i,
0,5 MiB delta, 0,5 MiB birlikte builder/worker, 0,25 MiB sorgu ve 0,25 MiB
diğer buffer'lar. Bunlar uygulanmış limit veya garanti değildir. Özellikle
caller batch'i ve materialize eden eski API bu bütçeye dahil edilmeden toplam
proses hedefi sağlanmış sayılamaz. Aynı kalıcılık seviyesinde native ingest de
68,845 saniye çıktı; 0.3.1'in Pythonlu SQLite'a göre hız farkı native motor
üstünlüğü olarak kullanılamaz.

Bütçe segment veya worker başına çoğalmaz. Cache pin'leri kotanın içindedir.
Worker bellek rezervasyonu alamadığında küçük blokla ilerler veya ön plana yer
açar; commit için ayrılmış payı tüketmez. Segment listesi ve merge heap'i de
sınırlanır: sürekli artan run sayısı RAM, dosya handle ve sorgu maliyetini
yeniden sınırsız hale getirmemelidir. Büyük run'lar da sınırlı buffer'la
birleştirilebilmeli; bugünkü 100.000 anahtar sınırı kalıcı çözüm değildir.

## Uygulama sırası

### 1. Karşılaştırma ve allocation profili

Önce aynı küçük corpus'ta ayrı bir native SQLite benchmark uygulaması kurulur.
SQLite yalnız benchmark tarafında bulunur; FDB bağımlılığı olmaz. İki motorun
corpus üreticisi, Unicode katlama/alt-dize anlamı, batch, guard/atomiklik kapsamı,
indeksleri, count + 50 kayıt dönüşü ve kalıcılık seviyesi açıkça eşleştirilir.
SQLite'ta WAL/FULL ve mevcut 2 MiB cache ayarının yanında makul cache seçenekleri
de raporlanır. Python runtime farkı performans iddiasına dönüşmez.

FDB'de açılış, commit, flush, worker merge, sorgu ve çıktı oluşturma ayrı izlenir.
Profiling ek yüküyle alınan süreler release karşılaştırmasına karıştırılmaz.
Whole-process PeakWorkingSet, örneklenen private bytes, allocation tepe miktarı,
okunan/yazılan baytlar, cache hit/miss ve allocation sayısı birlikte raporlanır.
Geniş metin sorgusu ve recovery ayrı proseslerde de ölçülür; ingest tepesine
dahil olmayan işler açıkça belirtilir.

### 2. Akışlı sorgu ve metin doğrulama prototipi

Tüm eşleşmeleri vektöre toplamak yerine sıralı posting blokları üzerinde AND,
range union ve segment merge iterator'ları yürür. Tam `total` sayımı korunur.
Yalnız istenen sayfanın kayıt referansları tutulur; payload bu sayfa için okunur.
Offset davranışı korunur; ek bir cursor seçeneği olsa bile eski son-sayfa sorgusu
testten çıkarılmaz. Aynı run/slot biliniyorken payload için yeniden bütün
segmentlerde anahtar araması yapılmaz.

Geniş ve yoğun listelerde skip bilgisi/blok sayımları kullanılır. Tam görünürlük
ve filtre kapsaması ispatlanamıyorsa kestirme count uygulanmaz. Gölgeleme için
her adayda yeni run'lara rastgele lookup yapmak hızı geri götürür; bu yaklaşım
kullanılmaz. Manifest/source kimliklerine bağlı görünürlük blokları disk cache'i
olarak hazırlanır, küçük aktif delta ayrıca uygulanır. Değişmeyen run çiftlerinin
işi tekrar edilmez. Bu yapı snapshot, tombstone ve güncelleme testlerinden geçer;
her flush'ta bütün DB'yi yeniden taratan bir çözüm kabul edilmez.

Metin doğrulama için türetilmiş cache'te önceden katlanmış başlık/gövde blokları
denenir. Önce kaynak doğrulanır; özgün payload döndürme ve alt-dize anlamı korunur.
Arama adayları blok sırasıyla okunur, küçük read-ahead kullanılır. Bir milyon
adayda bir milyon JSON nesnesi ve tekrar Unicode katlama yapılması azaltılır.
Tekrarsız corpus'ta da fayda aranır. Sıkıştırma ve içerik paylaşımı zorunlu değildir;
CPU maliyeti kazancı aşarsa ilgili codec kullanılmaz. Hash ile paylaşım seçilirse
eşitlik gerçek baytlarla doğrulanır; hash eşitliği tek başına içerik eşitliği olmaz.

### 3. Sayfalı, ikili türetilmiş indeks

Yeni, sürümlü bir SEG04 cache prototipinde şu yapılar diske alınır:

- Sıralı anahtar blokları; ortak scope/collection sözlüğü ve blok içi prefix
  kodlama. Anahtar sıralaması mevcut UTF-8/Türkçe davranışı değiştirmez.
- Sabit boyutlu kayıt konum/sürüm/CRC tablosu; slot ile doğrudan sayfa erişimi.
- Sayfalı token sözlüğü; küçük üst seviye yönlendirme ve gerektiğinde alt sayfa.
- Bloklara ayrılmış posting'ler; delta-varint veya bit packing, skip/count
  metadata'sı. Yoğun ardışık ID'lerde aralık gösterimi deneysel seçenek olur.

Başlangıç sayfa boyutu 4 KiB, blok okuma 16–64 KiB arasında benchmark ile seçilir.
2 MiB global cache içinde sıcak yönlendirme sayfalarına öncelik verilir; büyük
taramanın küçük rastgele sorguların cache'ini boşaltması engellenir. Posting
decoder'ları buffer'ları tekrar kullanır. Token/kayıt dizininin tamamı veya
sınırsız Bloom filtreleri RAM'e yüklenmez.

Hız mekanizması: birkaç anahtar konumlama sayfası, slot ile payload erişimi,
daha az disk baytı, sıralı posting okuma ve daha az allocation. Özellikle
soğuk rastgele get için ek I/O riski prototipte ölçülür. Sadece RAM azaldı diye
sayfalı tasarım bütün sorgulara uygulanmış sayılmaz.

### 4. Bütçeli yazma ve bakım

Delta posting'leri anahtar/Arc yerine yerel u32 ID taşır. Anahtar bir kez,
bütçeli arena'da tutulur; doküman gövdesinin WAL konumu kullanılarak bütün
gövdenin RAM'de tutulması azaltılır. Eski posting'in kaldırılması ve read-your-write
korunur. Arena eşikleri toplam ayrılmış kapasiteye göre belirlenir.

Flush/merge kayıtları ve indeks sayfalarını akışla yazar; `Vec<Record>` ve bütün
JSON dizini kurmaz. Spill merge en fazla örneğin 8 reader'lı turlar kullanır;
reader buffer'ları ve geçici terim metadata'sı ortak bütçeden gelir. Run sayısını
kontrol eden boyut katmanları ve ardışık run birleştirme, büyük run'ların da küçük
buffer'larla işlenmesine izin verir. Yazma büyütmesi ve worker I/O'su ayrıca ölçülür.

Kaynak yazılırken cache üretimi birlikte yapılabilir. Fakat fiziksel kaynak
doğrulaması kaldırılmaz: yazılmış kaynak, beklenen uzunluk/checksum ve frame
sözleşmesine göre yayımlama öncesinde doğrulanır. Açılışta da kaynak doğrulanır.
Amaç aynı JSON'u tekrar nesne ağacına dönüştürmek ve geçici kopyaları azaltmaktır;
`sync_all`, guard kontrolleri veya kabul edilmiş işlem garantisi azaltılmaz.
Sadece delta eşiğini düşürmek planın çözümü değildir; bunun ek flush/sync ve
segment maliyeti vardır, toplam ingest + bütün bakım birlikte değerlendirilir.

### 5. Açılış ve büyük isteklerin tepe belleği

LEGACY LOG01/LOG03 replay için bütün 64 MiB frame'i `Vec<u8>` ve `Batch` olarak
tutmak yerine checksum/eksik frame doğrulaması ile akışlı çözme ayrılır. Tam
frame doğrulanmadan kısmi transaction görünür olmaz; gerekirse iki disk geçişi
veya geçici staging kullanılır. Kaynak kötü, cache sağlam senaryosu yine reddedilir.

Mevcut Rust API `Vec<Mutation>` alıyor ve `SearchPage` bütün sayfa payload'larını
döndürüyor. CLI ise büyük satır + deserialize edilmiş istek + `Value` sonucu
oluşturabiliyor. Dolayısıyla mevcut bütün API çağrıları için 16 MiB proses
garantisi verilemez: tek izin verilen istek bundan büyük olabilir.

Büyük içerik/batch için ek akışlı transaction ve sonuç writer API'si tasarlanır;
guard'lar tamamen kontrol edilir, staging disk üzerinde tutulabilir ve transaction
tek atomik birim olarak kalır. JSON Lines tarafında ek akışlı yol/sürümlü sözleşme
gerekebilir. Eski materialize eden API sessizce parçalanmaz veya kapsamı daraltılmaz.
Küçük corpus hedefi ile büyük veri için bounded-streaming hedefi ayrı raporlanır.
Caller'ın zaten ayırdığı belleği motor belleğinden çıkartarak proses zaferi ilan edilmez.

### 6. Uyumluluk ve kabul

İlk indeks değişikliği `.idx` biçimiyle sınırlanır; `.fdb`/`.ready` kaynak
biçimi değiştirilmez. SEG04 açık sürüm taşır; SEG03 okunması veya doğrulanmış
kaynaktan yeniden üretimi test edilir. Eski motorun yeni cache'i yeniden
üretebilmesi ayrıca denenir. LOG01 fixture'ları değiştirilmez. Yeni kalıcı
kaynak biçimine ihtiyaç çıkarsa ayrı sürüm/migration ve uyumluluk tasarımı gerekir.

Her ilgili adımda `cargo test --all-targets`, `cargo fmt --check` ve
`cargo clippy --all-targets -- -D warnings` çalışır. Storage/replay değişiklikleri
için kesilme, bozulma, cache kaybı, sağlam cache + bozuk kaynak, yayınlama sırasında
gerçek process kill, guard reddi, atomiklik, tombstone ve eski kaynak uyumluluğu
yeniden doğrulanır. Mevcut benchmark klasörleri ve kullanıcı verisi korunur.

## Performans kabul matrisi

İlk kapı mevcut bir milyonluk corpus'tur. Ardından en az 100 bin / 1 milyon /
10 milyon kayıtla ölçek; tekrarsız 256 bayt içerik, uzun ve farklı metinler,
yüksek token çeşitliliği, birden fazla scope/collection, çakışan ID aralıkları ve
güncelleme/silme yoğun corpus eklenir. Uzun içerik testinin batch büyüklüğü iki
motor için eşit ve mevcut sözleşme limitlerine uygun seçilir.

| Alan | Kabul ölçütü |
| --- | --- |
| Proses RAM | Aynı iş yükünde native SQLite'ın altında WS ve private bytes; açılış, sorgu, ingest, bakım ve replay ayrı raporlanır |
| Ölçek | Kayıt/token sayısıyla RAM'de doğrusal tablo büyümesi olmaz; segment/manifest üst maliyeti de ölçülür |
| Yazma | Ingest + beklenen bütün bakım dahil; 0.3.1'e göre tekrarlanabilir gerileme yok, SQLite avantajı korunur |
| Sorgu | Dar başlık, source, range, geniş alt-dize, kısa Unicode terim, random get ve son sayfa; count + aynı payload sonucu |
| Gecikme | Medyan yanında p95 ve worst-case; foreground + bakım çakışması ayrıca |
| Cache | Sıcak ve kontrollü soğuk test ayrı; proses restart OS cache'i boşaltmış sayılmaz |
| İçerik | Tekrarlı corpus kazancı tekrarsız/uzun corpus'a genellenmez |
| Doğruluk | Aynı sequence, sıralama, guard, Unicode/substring, görünürlük ve kabul edilmiş işlem kurtarma sonucu |

Ölçümler sırayla, diğer benchmark çalışmıyorken, tercihen en az beş bağımsız
tekrarla yapılır. Deney sırası değiştirilir; varyans ve cache hazırlığı kaydedilir.
0.3.1'in bugünkü 108,086 saniye ingest, 9,332 ms metadata, 4,086 ms range ve
4,887 saniye geniş metin değerleri referanstır; tek koşunun rastgele daha iyi
çıkması kabul değildir. Açılış ölçümü tam kaynak doğrulaması kapsamını ayrıca
belirtir; SQLite normal open ile FDB tam replay'i eş kapsamlı gibi sunulmaz.

OS file cache'i iki motor için ayrıca açıklanır. mmap, working-set trim veya
daha büyük OS cache kullanımı RAM maliyetini yok etmiş gibi raporlanmaz. Disk
boyutu/I/O azalması destekleyici ölçümdür; bütünlük veya sorgu anlamı değişikliği
performans iyileştirmesi sayılmaz.

## Önerilen ilk teslimat

Native comparator + allocation profili, ardından sınırlı bir sayfalı indeks ve
akışlı sorgu prototipi. Bu prototip aynı corpus'ta **RAM ve hız koşullarını birlikte**
geçerse delta/builder/replay bütçelendirmesi tamamlanır. Geçemezse cache boyutu,
blok yerleşimi ve count/visibility yöntemi yeniden ölçülür; yavaşlama ile düşük
RAM hedefi tamamlanmış ilan edilmez.

SQLite'tan öğrenilen ilkeler motor bağımlılığı değildir: [page cache bütçesi](https://www.sqlite.org/pragma.html#pragma_cache_size),
[bellek havuzları ve allocation maliyeti](https://www.sqlite.org/malloc.html),
[FTS5 sayfalı ve sıkıştırılmış posting yerleşimi](https://www.sqlite.org/fts5.html#fts_index_b_trees).
SQLite `cache_size` bütün prosesin RAM limiti değildir; aynı ayrımı FDB için de koruruz.
