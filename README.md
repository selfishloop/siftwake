# FDB

Güncel öncelik: **okuma, yazma ve içerir aramasında sonuç getirme hızı**.
RAM azaltımı bu aşamanın hedefi değil. [Yeni hız karşılaştırması ve uygulama sırası](docs/SPEED-REASSESSMENT-20261009.md).

[WAL commit senkronlaması ölçüldü](docs/WAL-SYNC-VALIDATION.md): tek kayıtlık
işlemlerde büyük fark,100k/batch500 bakım dahil toplamda küçük fark çıktı.
`experimental-wal-sync-control` ile yalnız explicit Rust API çağrısı commit sync'i
kapatır; bütün build'ler varsayılan sync açık başlar. WAL yazımı devam eder.

En son: [posting oluşturucu bütçesi ve kesilme doğrulaması](docs/BUILDER-BUDGET-VALIDATION.md).
100k sıcak-cache üç çiftte tepe proses working set18.65→17.34MiB,
yazma+bakım10.91→10.81s. Küçük süre farkı kesin hız artışı değildir;
yeniden açılış/bazı sorgu kapıları ve SQLite hedefi tamamlanmadı. Bütçe default kapalıdır.

Son deney: `experimental-shared-documents` delta belgelerinde tekrarlı gövde ve
küçük skaler alan kümelerini RAM'de paylaşır. Delta sorguları eşleşen anahtarların
tam kopyaları yerine mevcut indeks anahtarlarını referansla tutar. Kalıcı format
değişmez. [Ölçüm, doğruluk ve kalan hedefler](docs/SHARED-DOCUMENTS-VALIDATION.md).
Sonraki anahtar yerleşimi aynı anahtarı docs/indeks/shadow arasında paylaşır;
belge lookup standart Rust HashMap kullanır, flush/sorgu sıralaması korunur.
[Anahtar paylaşımı ölçümleri](docs/SHARED-KEYS-VALIDATION.md).

SDB ve Fundops verilerini, bağlantılı kayıtları ve dosyalardan çıkarılmış metinleri
saklamak ve aramak için Rust ile yazılan özel veritabanı motoru. **SQLite,
RocksDB, LMDB, sled, Tantivy veya başka bir veritabanı/arama motoru kullanılmaz.**
Depolama biçimi, işlem günlüğü, kurtarma, indeksler ve checkpoint mekanizması bu
repository'de uygulanır. `serde` ve `serde_json` yalnızca veri serileştirme içindir.

Bu sürüm çalıştırılabilir **0.3.1 çekirdeğidir**. Ürün kapsamı bağımsız DB motorudur;
SDB ve Fundops yalnız veri modeli ihtiyaçlarına referanstır. Rust API, yerel
JSON Lines süreci ve örnek istemci/aktarım yardımcıları hazırdır.

Güncel ölçüm ve hata testleri: [0.3.1 doğrulama sonuçları](docs/VALIDATION-0.3.1.md).
Bir milyon kayıt için ayrı benchmark: [SQLite karşılaştırması](docs/SQLITE-COMPARISON.md).
SQLite yalnız karşılaştırma aracında kullanılır; FDB motorunun bağımlılığı değildir.

Bellek planının geliştirme aşaması başladı: [aşama 1 doğrulaması](docs/MEMORY-STAGE1-VALIDATION.md)
ve [uygulama planı](docs/MEMORY-PERFORMANCE-PLAN.md). İlk sayfa sorguları artık
yalnız sınırlı sayıda eşleşme ID'si saklar; tam sonuç sayısı korunur. Anahtar/konum
tablosu hâlâ RAM'dedir; SQLite'tan düşük toplam proses belleği hedefi tamamlanmadı.

Sayfalı kayıt tablosu için [ilk prototip ve ölçüm](docs/PAGED-RECORDS-PROTOTYPE.md)
hazırdır. Bellek yerleşimi küçüldü; lookup hız kapısı henüz geçilmediği için
üretim motoruna alınmadı.

[Doğrudan kayıt konumundan okuma](docs/DIRECT-SLOT-VALIDATION.md) deneysel Cargo
feature'ı olarak hazır. Aynı proses karşılaştırmasında ilk/ikinci sayfada yaklaşık
%10 kazanç görüldü; tüm hız/bellek kapıları tamamlanana kadar default sorgu yolu korunur.

[Gruplu payload okuması](docs/GROUPED-READ-VALIDATION.md) deneysel olarak eklendi.
Yakın kayıtlar sınırlı bir okuma penceresinde alınır; ilk/ikinci sayfa ölçümünde
yaklaşık %31 kazanç görüldü. Kayıt tablosu hâlâ RAM'dedir.

`experimental-paged-metadata` ile kayıt/ID tablolarını diske taşıyan deneysel
motor yolu eklendi: [uygulama ve ölçüm](docs/PAGED-METADATA-VALIDATION.md).
Sıkıştırılmış kayıt sayfaları, iki seviyeli dizin ve ortak 256 KiB sayfa cache'i
kullanır. Default build kayıt tablosunu RAM'de tutmaya devam eder; deneysel
yolun tüm hız ve toplam proses RAM kapıları henüz tamamlanmadı.

`experimental-streaming-open` ayrıca checkpoint kaynaklarını ve SEG03 dizinlerini
kayıt bazında çözer. Açılış tepe belleği, sorgu/yazma ölçümleri ve kalan sınırlar:
[akışlı açılış doğrulaması](docs/STREAMING-MEMORY-VALIDATION.md). Bu yol da
deneyseldir; SQLite'tan düşük toplam proses belleği hedefi tamamlanmadı.

Karşılaştırmanın aynı sonuç sözleşmesini doğrulayan ve yazma/sorgu maliyetlerini
ayıran sonraki aşama: [iş yükü ve maliyet doğrulaması](docs/WORKLOAD-COST-VALIDATION.md).
`diagnostic-timing` yalnız ayrı teşhis derlemesinde süre sayaçlarını açar;
normal performans ölçümünde kapalı tutulur.

`experimental-write-pipeline`, flush/merge tekrarını azaltan ve builder kayıt
dizinini akışla üreten sonraki deneydir: [ilk yazma hattı ölçümü](docs/WRITE-PIPELINE-VALIDATION.md).
Bakım kilitleri hız maliyeti nedeniyle güncel umbrella'dan çıkarıldı; ayrıca
`experimental-write-maintenance-slots` ile açılabilir. Bu sınırlar delta dahil
toplam byte bütçesi değildir; deney normal build'de varsayılan olarak kapalıdır.
Parçalar ayrı Cargo feature'larıyla ölçülebilir: [bileşen karşılaştırması](docs/WRITE-COMPONENTS-VALIDATION.md).

`experimental-write-buffer-budget`, checkpoint kaynaklarını büyük batch/frame
Vec'leri olmadan akıtır ve kontrollü yazma tamponlarını ortak 256 KiB rezervasyona
alır: [uygulama ve ölçüm](docs/WRITE-BUFFER-BUDGET-VALIDATION.md). Delta dahil
toplam heap bütçesi henüz tamamlanmadı; bu feature da varsayılan olarak kapalıdır.

`experimental-shared-delta-text`, delta indeksinde aynı katlanmış içerikleri
paylaşır; başlıklar belgeye özgü kalır. Kontrollü body cache'e sığmayan metinler
kendi referans sayımlı içeriklerini taşır. Bu feature normal build'de kapalıdır.
[Ortak delta metni doğrulaması](docs/SHARED-DELTA-TEXT-VALIDATION.md).

`experimental-compact-delta-postings` delta eşleşmelerini u32 slot numaralarıyla
saklar. `experimental-delta-bitmaps` ayrıca küçük inline/sıralı listeler ile
bitmap arasında yoğunluğa göre geçer. Kalıcı format değişmez; normal build'de
kapalıdır. [Delta posting doğrulaması](docs/DELTA-POSTINGS-VALIDATION.md).

`experimental-streaming-metadata`, SEG03 kayıtlarını geçici tam tabloya almadan
MET04 sayfalarına akıtır veya mevcut sayfalarla karşılaştırır. Kalıcı biçim ve
kaynak doğrulaması korunur. [Metadata akışı doğrulaması](docs/STREAMING-METADATA-VALIDATION.md).
Açılış/sorgu belleği iyileşti; yazma dahil toplam tepe hedefi henüz çözülmedi.

## Neler yapıyor?

- `(scope, collection, id)` ile idempotent kayıt ekleme/güncelleme/silme.
- Toplu işlemin tamamını tek kalıcı çerçevede yazma; eksik işlem uygulanmaz.
- Sürüm denetimi: `missing` veya `version` ile eşzamanlı güncelleme çakışmasını yakalama.
- İndeksli alan eşitliği ve `i64` alanlarda dahilî alt/üst sınır filtreleri.
- Başlık ve içerikte Unicode trigram indeksiyle AND alt-dize araması.
- Türkçe `İ/ı/I`, `ş/s`, `ğ/g`, `ç/c`, `ö/o`, `ü/u` eşleştirmesi.
- Aynı scope içindeki kayıt bağlantıları ve ters bağlantı sorgusu.
- Toplam kayıt sayısı, offset/limit ile numaralı sayfalama için veri.
- Çökme sonrası günlükten yeniden açılma; checksum bozulmasında açık hata.
- Yalnız değişmiş kayıtları yeni segmente yazan flush ile etkin günlüğün küçültülmesi.
- Yazmalar sürerken sınırlı segment gruplarını arka planda birleştirme.
- Yaklaşık 4 MiB posting gruplarıyla geçici dosyalarda indeks yapımı.
- Checkpoint kayıtlarını ve u32 arama posting'lerini diskte saklama; içeriği gerektiğinde okuma.
- RAM'deki değişiklikler için ayarlanabilir otomatik checkpoint eşikleri.
- Kayıp/bozuk türetilmiş disk indeksini sağlam işlem günlüğünden yeniden kurma.
- İşletim sistemi dosya kilidiyle aynı klasör için tek motor sahibi.

## Çalıştırma

Rust 1.89 veya üzeri gerekir. Windows PowerShell:

```powershell
cargo test --all-targets
cargo build --release --bins --examples
cargo run --release --example linked_search -- data/example
```

`linked_search`, tamamen **örnek verilerle** bir fonu, raporu, dosya metadata'sını
ve çıkarılmış içeriği birbirine bağlar. Aramayı, tarih filtresini, checkpoint'i
ve yeniden açıldıktan sonraki sonucu doğrular. PDF açma/çıkarma işlemi yapmaz.

Tek isteği UTF-8 JSON dosyasından yürütmek:

```powershell
.\target\release\fdb.exe data/mydb examples/requests/commit.json
.\target\release\fdb.exe data/mydb examples/requests/search.json
```

Uzun yaşayan süreç:

```powershell
.\target\release\fdb.exe data/mydb --stdio
```

Her satır bir JSON isteği, her yanıt bir JSON satırıdır. Başarılı yanıt
`{"ok":true,"result":...}`, hatalı istek `{"ok":false,"error":"..."}` döndürür.
`commit`, `get`, `search`, `stats`, `flush`, `compact`, `poll_compaction`,
`wait_compaction` desteklenir. Ayrıntı:
[protokol](docs/PROTOCOL.md).

Python worker bağlantısını doğrulamak:

```powershell
python clients/demo.py target/release/fdb.exe data/python-demo
```

Rust'ta `Database::open`, `commit`, `get`, `search`, `compact` kullanılır.
Birden fazla thread için `Arc<Mutex<Database>>` ile paylaşılabilir. Ayrı süreçlerin
aynı klasörü eşzamanlı açması reddedilir; istemciler tek sahip sürece bağlanmalıdır.

## SDB veri modeli referansı

`adapters::file_entry`, hacim kimliği + dosya referansı + parent + ad ile hard
linkleri ve farklı hacimlerde aynı ID'leri birbirinden ayırır. Canlı bağlantıda
hacim harfi yerine SDB'nin `ScanOutput.serial` değeri, tam NTFS dosya referansı
ve USN cursor/journal kimliği kullanılmalıdır. Rename/move eski ad anahtarını
silip yeni anahtarı ekleyen atomik işlem olarak gönderilir. İçerik ayrı bir
`file_contents` kaydı olabilir; dosya kaydına `links` ile bağlanır.

Mevcut SDB TSV biçimi için salt okunur kaynak aktarımı:

```powershell
.\target\release\fdb-import-sdb.exe data/sdb-sample D:/Vibeprojects/sdb/data/c.tsv export-20261007-C shared 1000
```

Kaynak TSV'ye yazılmaz. TSV tam NTFS referansı, hacim seri numarası veya çözülmüş
dosya yolu taşımaz; aktarılmış kayıtlar `identity_kind=sdb_tsv_export_id` olarak
etiketlenir. Her bağımsız export için ayrı namespace kullanın. Bu aktarım canlı
USN takibi değildir. Her 500 kayıt ayrı atomik işlemdir; aktarım hatasında önceki
başarılı batch'ler kalır. Aynı namespace ve aynı kayıtlarla tekrar çalıştırma
aynı anahtarları günceller. Export'tan kalkmış kayıtlar otomatik silinmez.

## Fundops veri modeli referansı

`adapters::fundops_record` alanları kayıpsız JSON metadata olarak taşır,
`fund_code` bulunan raporları aynı scope'taki `funds` kaydına bağlar.
Fon kodu, rapor ID'si veya `(fon, gün, sağlayıcı)` doğal anahtarı açıkça verilir;
motor metinden kimlik tahmin etmez. Şimdiki bağlantı sözleşmesi:
[veri modeli ve mimari](docs/ARCHITECTURE.md).

## Disk segmentleri ve bellek

Yazmalar önce kalıcı işlem günlüğüne ve RAM delta'sına eklenir. `flush` yalnız
bu delta'yı `run-<hex>.fdb` kaynağına ve türetilmiş `.idx` indeksine yazar;
önceki segmentleri yeniden yazmaz. İçerikler ve u32 posting listeleri disktedir.
Anahtar/konum ve token sözlükleri RAM'de kalır. Yeni kayıtlar ve silme işaretleri
eski segmentlerdeki aynı anahtarı gölgeler; arama sonuçları anahtar sırasıyla birleşir.

Geliştirme prototipinde offset+limit <= 4096 için run başına en fazla offset+limit
kadar ID saklanır; ilk sayfada bu sayı limit kadardır.
Tam canlı kapsaması bilinen metinsiz filtrelerde count, tombstone'ları hariç tutan
görünürlük maskesinden hesaplanır. Büyük offset'ler hâlâ tam eşleşme listesini
kullanan önceki yolu izler; genel bir hard RAM sınırı yoktur.

Varsayılan 10.000 değişmiş anahtar veya 16 MiB serileştirilmiş delta eşiği,
bir sonraki geçerli yazma öncesinde flush tetikler. Tek batch bu eşiği aşabilir;
kesin RAM limiti değildir. Açılışta büyük WAL delta'sı da flush edilir.

```rust
let options = fdb::Options {
    max_delta_keys: 5_000,
    max_delta_bytes: 8 * 1024 * 1024,
};
let mut db = fdb::Database::open_with_options("data/mydb", options)?;
db.flush()?;
db.compact()?; // flush + en fazla bir arka plan birleştirme başlatır
db.wait_for_compaction()?; // başlatılmış işi bekler ve sonucunu yayımlar
```

**0.3'te `compact` arka plan işini beklemez.** Rust API'deki
`start_compaction()` delta'yı flush etmeden iş başlatır. `poll_compaction()`
biten işi beklemeden yayımlar; `wait_for_compaction()` yalnız mevcut işi bekler.
Tüm uygun grupları bitirmek için wait ardından start/wait döngüsü kullanılabilir.
JSON Lines'ta `compact` + `wait_compaction` aynı bakım akışını sağlar.

Bir iş art arda gelen, benzer boyutlu 2–4 segmenti seçer; toplam giriş en fazla
100.000 anahtar ve 64 MiB kaynak veridir. Dört veya daha çok segment olduğunda
otomatik birleştirme denenir. Büyük segmentler bu sınırın üstünde tekrar
birleştirilmez. Yazma sahibi yeni delta'yı işlerken worker kaynak/cache'i üretir;
sonucu yalnız sahip motor yeni `.ready` manifest ile yayımlar.
`Drop` çalışan işi bekler fakat sonucunu yayımlamaz; kullanılmayan dosyalar kalabilir.
Flush ve manifest yayımlama hâlâ eşzamanlı bakım maliyetidir.

İndeks yapımı posting girdilerini yaklaşık 4 MiB tahmini bütçede checksummed
geçici dosyalara döker ve sıralı akışla birleştirir. Bu bütçe toplam derleme RAM'i
olmaz: kayıt/token dizinleri, tek kaydın trigram kümesi, payload ve allocator
kapasitesi ayrıca yer kaplar. Arama geçici eşleşme ID listelerini tutar.

`stats` segment sayısını, çalışan işi, referans verilen kaynak ve cache baytlarını
bildirir. `resident_text_bytes`, yalnız delta'nın kaynak+katlanmış UTF-8 metin
baytlarıdır; proses RAM'i değildir. Gerçek working set ölçümü için
[ölçüm aracı](tools/measure-scale.ps1) ve [sonuçlar](docs/VALIDATION-0.3.1.md) var.
Arşiv ve yayımlanmamış dosyalar stats'a dahil değildir; toplam klasör diski ayrıca ölçülür.

0.1/0.2 kaynak/marker biçimi okunur; eski kaynak baytları yeniden yazılmaz.
İlk yeni flush 0.3 manifest yayımlar. **Eski motor sürümleri bu manifesti okuyamaz.**
Eski `.idx` biçimi yeniden üretilir. Kaynak `.fdb` bozuksa sağlam cache ile açılışa
izin verilmez. Güncel kalıcı biçim [mimari belgesinde](docs/ARCHITECTURE.md).

## Sınırlar

- Anahtar/konum ve posting sözlükleri RAM'dedir. Dizin JSON'u segment başına
  256 MiB ile sınırlıdır. Sentetik bir milyon kayıt ölçümü gerçek belge çeşitliliği
  veya kesin bir RAM üst sınırı garantisi değildir.
  Disk erişimi güvenli positional I/O kullanır; mmap kullanılmaz.
- Açılışta kaynak log ve türetilmiş segment checksum'ları tamamen doğrulanır.
  Güvenilir hızlı açılış için bu tarama henüz atlanmaz; büyük dosyalarda I/O maliyeti vardır.
- Geniş metin sorguları aday içerikleri tek tek okuyup katlar. Dar metadata/isim
  sorgularının sonuçları geniş içerik sorgularının hızını temsil etmez.
- Kimlik scope'u sorgu sınırıdır; **kimlik doğrulama/yetkilendirme değildir**.
  Güvenilen host, kullanıcının scope'unu oturumundan belirlemelidir.
- SQL, otomatik foreign-key kontrolü, SQL join, dağıtık replikasyon, MVCC,
  HTTP sunucusu, şifreleme ve otomatik yedek geri yükleme yoktur.
- PDF/DOCX/OCR çıkarıcısı yoktur. Çıkarılmış UTF-8 metin kabul edilir;
  ileride çıkarıcı sürümü, kaynak hash'i ve bölüm numarası metadata olarak taşınabilir.
- Para/fiyatların kayıpsız temsili için ondalık string veya ölçekli `i64` kullanın.
  Aralık indeksi yalnız `i64` değerleri içindir; JSON float fiyatları bu indekse girmez.
- Sıralama scope/collection/id anahtar sırasıdır; ilgi puanı veya tarih sırası değildir.
  Sayfalar arasında yazma olursa sonuç değişebilir; yanıttaki `sequence` ile değişim
  anlaşılır. Snapshot pagination yoktur.
- Bir kayıtta başlık+metin 2 MiB, toplam JSON 4 MiB; işlem 64 MiB ve 10.000 mutasyon;
  sayfa 1.000 sonuç ile sınırlıdır. Büyük içerikleri bölüm kayıtlarına ayırın.
- Checkpoint eski nesilleri otomatik silmez. Aktif dosya küçülür; eski nesiller
  arşiv olarak diskte kalır. Motor kapanmadan backup kopyalamayın.
- `sync_all` ve process-kill kurtarması doğrulanır. Windows'ta dizin metadata'sının
  elektrik kesintisindeki atomik kalıcılığı ayrıca doğrulanmamıştır; güç kesintisi
  garantisi iddia edilmez. I/O hatasında yazmanın sonucu belirsiz olabilir; motor
  yeniden açılıp kayıt sürümü kontrol edilmelidir.

Sentetik ölçüm için boş bir hedef klasörle:

```powershell
cargo run --release --example measure -- data/benchmark
cargo run --release --example measure_disk -- data/content-benchmark 50000 1024
.\tools\measure-scale.ps1 -DatabasePath data/scale-new -Count 1000000 -TextBytes 256
```

Testler gerçek süreç sonlandırmasını, her olası son-işlem kesilme konumunu, tam
işlemde tek bayt bozulmasını, atomik guard reddini, kapsamları, Unicode aramayı,
çok çerçeveli checkpoint'i ve referans modele karşı güncelleme/silme dizisini kapsar.
Disk testleri bozuk/kayıp indeksin yeniden kurulmasını, base/delta gölgelemeyi,
blok sınırlarını, RAM metinlerinin checkpoint sonrası boşalmasını ve 0.1 uyumunu doğrular.

`experimental-wal-delta`, tam delta belgeleri yerine active WAL konumlarını
tutan deneydir. 100k yazma dahil proses RAM'i %38 düştü; fakat unflushed delta
get100 yaklaşık 6 kat yavaşladı. Hız kapısı geçmediği için default kapalıdır.
[WAL delta ölçümü ve karar](docs/WAL-DELTA-VALIDATION.md).

`experimental-merge-postings`, merge sırasında metin/alan indeksini yeniden
üretmek yerine mevcut SEG03 posting listelerini güncel kayıt ID'lerine taşır.
Kaynak/cache bütünlüğü korunur, kalıcı format değişmez; varsayılan kapalıdır.
[Merge posting doğrulaması](docs/MERGE-POSTINGS-VALIDATION.md).
