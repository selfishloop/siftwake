# İçerik indeksi yönü: ilk 10 bin belge bellek ölçümü

Kullanıcı kararıyla geliştirme performans testleri **10 bin belgeyle sınırlı**.
50k/100k koşusu durduruldu, dosyaları korundu; bu rapora dahil edilmedi.
İki yeni benchmark binary'si 10k dışında sayıyı veritabanını açmadan reddeder.

Bu çalışma mevcut default FDB'nin başlangıç ölçümüdür. `derived` profil,
mmap sözlük/posting yerleşimi, tek kopya metin veya DLL henüz uygulanmadı.
Frunner/SDB/Fundops değişikliği yapılmadı; motor kalıcı formatı değişmedi.

## Sonuç

| Ölçüm | FDB native binary | SQLite |
|---|---:|---:|
| Boşta private, açılış sonrası 5s, üç proses medyanı | 21.406 MiB | 1.312 MiB |
| Boşta working set, aynı nokta | 23.641 MiB | 6.020 MiB |
| İndeksleme + bakım örneklenen tepe private | 184.746 MiB | 8.465 MiB |
| İndeksleme + bakım örneklenen tepe working set | 179.500 MiB | 13.992 MiB |
| Son bakım örneklenen tepe private | 151.719 MiB | 8.383 MiB |
| 10k ingest, tek koşu | 74.454 s | 49.367 s |
| Son bakım, tek koşu | 17.066 s | 1.684 s |
| Ingest + son bakım | 91.520 s | 51.052 s |
| Normal open, üç proses medyanı, sözleşmeler farklı | 1048.894 ms | 9.489 ms |
| Güncel snapshot fiziksel disk | 621143804 byte | 583860224 byte |
| Eski nesiller dahil retained fiziksel disk | 1513745887 byte | 583860224 byte |

FDB canlı disk SQLite'ın yaklaşık1.064katı; retained disk yaklaşık2.593katı.
FDB `.ready` manifestinin işaret ettiği güncel source/index/active dosyaları,
ölçümde kaydedilmiş gerçek dosya boyutlarıyla toplandı. Bu manifestin CRC ve
uzunluğu doğrulandı; motorun sayaçları disk boyutu olarak kullanılmadı.
Canlı snapshot267900709byte source/active +353242581byte index ve manifesttir.
Silinmemiş eski nesiller gerçek disk maliyeti olarak ayrıca gösterilir.
SQLite ingest sonrası kapanmış ana dosyası esas alınır; idle sırasında
ek32KiB SHM oluşur. Geçici disk tepe kullanımı ölçülmedi.

**Boşta ve indeksleme belleği ayrı pencerelerdir.** İndeksleme belleği yeni
prosesle measured API ingest ve final bakım sırasında örneklenir; açılış,
5s idle veya başka sorgu yükünün bütün-proses pikiyle değiştirilmez. WS/private
OS proses değerleridir; motor heap'i veya ortak RAM bütçesi değildir.

## Erken kontrol kapıları

Frunner belgesinin resmi kabulü50k/100k ister. Aşağıdaki sayısal sınırlar,
kullanıcı kararıyla **10k geliştirme kontrolü** olarak uygulanmıştır; 50k
kabulünü geçtiğimiz anlamına gelmez. İyi sonuç elde edilmeden büyük koşu yok.

| Erken 10k kontrolü | FDB sonucu |
|---|---|
| Boşta private ≤15MiB | KALDI: 21.406MiB |
| İndeksleme tepe private ≤60MiB | KALDI: 184.746MiB |
| Snapshot disk ≤metnin %40'ı | KALDI: canlı disk metnin yaklaşık%234.5'i |
| Disk ≤SQLite'ın 1.2katı | Canlı snapshot GEÇTİ; retained footprint KALDI |
| Açılış ≤500ms | KALDI; mevcut FDB tam kaynak doğrular |
| Final bakım ≤SQLite'ın 2katı | KALDI: yaklaşık10.13kat, tek koşu |
| Dar/AND/Türkçe sorgu p95 | ÖLÇÜLMEDİ |
| Tek belge görünme gecikmesi ≤20ms | ÖLÇÜLMEDİ |
| Bakım sürerken arama/yazma bloklanmaması | ÖLÇÜLMEDİ |

İndeksleme ve bakım birer koşudur: tekrarlanabilir hız veya tail latency
iddiası yapılmaz. Idle üç bağımsız prosestir; FDB private örnekleri
22568960 /22446080 /22446080byte; SQLite1376256 /1384448 /1372160byte.
Mevcut default FDB'nin bu boyutta bile bellek/bakım sınırlarını aşması
geliştirme önceliğini gösterir. Bunun hangi allocator/veri yapısı tarafından
kaç byte oluşturulduğu bu testte profillenmedi; JSON'a tek başına atfedilmez.

## Corpus, konfigürasyon ve doğruluk

- Önceki çeşitli uzun corpus'un ilk10000belgesi; Gutenberg kitap pencereleri,
  2057–51197UTF-8byte gövde, toplam264842858byte metin. Benzersiz pasajlar,
  bağımsız10000kitap veya Türkçe uzun corpus değil. Key/title/fields/links aynı.
  Sequence20, her500belgeli batch'in sürümü aynı iki motorda.
- Her iki araç aynı `json.payload-bench` dosyasını65,536byte BufReader ve
  yeniden kullanılan bir kayıt tamponuyla okur. Kitaplar, corpus planı,
  bütün belgeler ve sorgu oracle'ları RAM'e yüklenmez. Batch500; corpus CRC,
  JSON çözme, anahtar/version kontrolü, motor serialization ve örneklenen
  tam belge doğrulamaları ingest zamanına dahildir. FDB500Put bellekte tutar;
  SQLite tek transaction'a belgeleri tek tek ekler. Bu API/buffering farkı
  gerçek pencerenin parçasıdır, eşit ek bellek maliyeti olduğu iddia edilmez.
- FDB default release, OFF1/LOG04/SEG04, sync açık; delta10000key/16MiB.
  Flush, worker bekleme ve tüm başlatılabilir compaction'lar ölçülür; sonunda
  sıfır resident/delta,10000belge,5disk segmenti. Son bakım tek segment
  garantisi vermez. Diagnostic/experimental feature kapalı.
- SQLite ayrı Rust paketi, bundled3.53.2/rusqlite0.40.2; WAL/FULL,
  2MiB istenmiş page cache, mmap0,tempFILE. UNIQUE(scope,collection,id),
  orijinal JSONpayload + katlanmış title/newline/text, external-content
  FTS5trigram/detail=none/columnsize=0 ve insert trigger. Final optimize ve
  WAL checkpoint dahil. Dinamik alan/link/guard indekslerinin tümü eşdeğer değil.
- İki fresh ingest prosesi; ardından her motor için üç fresh idle prosesi,
  dönüşümlü sıra. FDB/SQLite ingest sırası tek koşu olduğu için sıra etkisi
  kontrol edilmiş sayılmaz. Idle önce count/stats doğrular, sonra5s sorgusuz
  bekler; observer ilk `idle_ready` noktasında private/WS alır. Sorgu yok.
  SQLite count kontrolü birkaç index page'i okuyabilir; tamamen dokunulmamış
  bağlantı idle'ı değildir. FDB zaten tam kaynak doğrulaması yapar.
- 20ms hedef örnekleme;100ms log phase gözlemi. Pencere sınırlarında bu
  gecikme olabilir, kısa private pikler kaçabilir. Sampled WS peak ile ayrı
  whole-process `PeakWorkingSet64` birbirine karıştırılmaz. OS file cache ve
  PowerShell observer bellek değerleri dahil değil; cache tahliyesi yapılmadı.
  Derleme/test/hash/profil ana ölçümle eşzamanlı çalıştırılmadı.
- 8 proses exit0; bütün10000giriş kayıtlarının header/payload CRC'si doğrulandı.
  Her batch ilk/son belge API'den tam içerik/version bakımından kontrol edildi:
  toplam80tam belge kontrolü. Idle count'ları ve sequence/resident stat'ları
  doğrulandı. SQLite+FTS tam integrity ayrı pencerede geçti. FDB idle açılışları
  authoritative kaynağı tamamen doğruladı. Yeni storage/recovery kodu yok.

Makine önceki karşılaştırmayla aynı Ryzen7 7840HS8/16, Windows11Pro10.0.26200,
yaklaşık15.3GiB RAM, rustc1.99.0; release thinLTO/codegen-units1.
[Ham özet](benchmarks/content-memory-10k-20261011-summary.json),
[binary SHA-256](benchmarks/content-memory-10k-20261011-executables.json),
[SQLite integrity](benchmarks/content-memory-10k-20261011-integrity.jsonl).

Root default55test, benchmark CRC1test ve ayrı SQLite paket4test geçti.
Root ve SQLite `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings`
geçti; 0.1fixture ve `tests/engine.rs` değişmedi. İki frozenexe SHA-256'sı
ölçüm sonunda eşleşti; 100k isteğinin hata verip DB oluşturmaması ayrıca
kontrol edildi. [Root test](benchmarks/content-memory-10k-20261011-default-tests.log),
[probe test](benchmarks/content-memory-10k-20261011-probe-tests.log),
[SQLite test](benchmarks/content-memory-10k-20261011-sqlite-tests.log),
[root clippy](benchmarks/content-memory-10k-20261011-clippy.log),
[SQLite clippy](benchmarks/content-memory-10k-20261011-sqlite-clippy.log).

## Sonraki çalışma sınırı

10k üzerinde kalınır. Önce builder/delta/worker ve açılış sonrası metadata
maliyetini ayrı profil ile belirle; `derived` içerik indeksi için disk sözlüğü,
sınırlı builder ve tek kopya payload seçeneğini aynı10k corpus'ta ölç.
İki aday depolama seçeneği olan metinsiz aday indeksi ve katlanmış/sıkıştırılmış
tek metin kopyasının hızı bu baseline'dan çıkarılamaz. Yeni kalıcı snapshot
ayrı format/sürüm ve bozulma/kesilme/kill testleri gerektirir.
Ardından gerçek30dar/AND/Türkçe sorgu seti ve güncelleme görünme gecikmesi gelir;
bu belleğe odaklı aşamada arama hızı sonucu yoktur.

Tekrarlama: `tools/measure-content-memory.ps1 -Prefix yeni-10k-ad`, ardından
`tools/summarize-content-memory.py docs/benchmarks/yeni-10k-ad`. FreshDB/prefix
zorunlu. Script iki frozen10kexe'yi kullanır; büyük sayı otomatik çalıştırılmaz.
