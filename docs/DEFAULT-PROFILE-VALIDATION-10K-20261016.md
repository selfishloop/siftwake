# Ölçülmüş birleşimin varsayılana taşınması — 2026-10-16

Kabul edilen akışlı yazma, ortak çalışma bütçesi, sayfalı sözlük/metadata
ve doğrulanmış arama birleşimi normal derlemede `bounded-engine` adıyla
etkindir. `experimental-builder-budget` aynı uygulamaya bağlanan uyumluluk
alias'ıdır. Yeni deneysel uygulama, bağımlılık, kalıcı format, protokol
veya dış proje entegrasyonu eklenmedi.

Önceki çalışan durum `42116f769afe21c098f46731f29d28a6b6a21c5b`
yerel commit'inde sabitlendi. Bu taşıma ayrı commit'tedir; push/release
yayını yapılmadı. Varsayılan profilin taşınması bütün iş yüklerindeki
hız/RAM hedeflerinin veya üretim sertifikasyonunun tamamlandığı anlamına
gelmez. Kalan süre artışları aşağıda ve OPEN-WORK içinde açıktır.

## Tam olarak ne taşındı?

Eski `--no-default-features --features experimental-builder-budget`
birleşiminin etkin bileşen kümesiyle yeni normal derlemenin kümesi
eşleştirildi. Rust kaynaklarında yalnız cfg anahtarı yeni ada taşındı;
doc/error açıklamaları dışında algoritma gövdeleri normalize edildiğinde
checkpoint ile aynıdır. 0.1 fixture ve payload/WAL/manifest/index biçimi
değişmedi. [Özellik ve kaynak eşdeğerliği](benchmarks/stabilize-feature-proof-20261016.json).

Bağımsız rkyv, WAL-delta, shared-documents/shared-delta-text ve WAL sync
kapatma deneyleri varsayılanda kapalıdır. Diagnostic sayaçlar da kapalıdır.
SQLite engine'e eklenmedi; ayrı comparator'ın FDB tip bağımlılığı
`default-features = false` ile açıkça izole edildi.

`commit_streamed(iterator)` artık özel deneysel derleme seçeneği gerektirmez.
`commit(Vec<Mutation>)` mevcut limitlerini ve delta davranışını korur;
256 KiB streamed entry sınırına otomatik yönlendirilmez. Normal JSON Lines
commit isteği Vec API'sini kullanmaya devam eder. Strict `Database::open`
yerine deferred/record-read API'si otomatik seçilmedi.
[Güncel kullanım ve sınırlar](DEFAULT-PROFILE.md).

## Ölçüm koşulları

Aynı mevcut `data/rkyv-corpus-20261009/json.payload-bench` ilk10k:
264.842.858 B gövde,2.057–51.197 B/pasaj. Yeni corpus üretimi/indirme yok.
Yazma gerektiğinde yalnız sahiplik işaretli sabit FDB/SQLite slotları
sıfırlandı; veritabanı kopyası veya tarihli yeni DB klasörü tutulmadı.
Loglar docs/benchmarks içindedir. Normal release/Windows/Ryzen7840HS,
diagnostic timing/allocation kapalı, senkron WAL, batch500/20 transaction.
Sıcak OS cache, eviction/affinity/power izolasyonu yok; yalnız kısa-blok
sayfa kontrolü CPU mask4 kullanır. Timed koşular sırasında derleme/test yok.

Kontrol, checkpoint'in açık feature'lı derlemesi; aday, özel feature
bayrağı olmadan normal derlemedir. Her ingest sonrası ayrı süreçte10k
tam belge/version ve dört query total/ilk50 tam sayfa oracle'ı doğrulandı.
Bu doğrulama yazma süresine dahil değildir. Ingest içindeki40 sınır get
kontrolü ile input CRC/JSON çözme ve yazma sırasındaki bakım dahildir.
Son bakım flush/merge drain'dir. Source CRC, UTF-8/schema/key/version ve
kaynak yetkilendirme kontrolleri kaldırılmadı.

## Yazma/bakım: iki API ayrı

| API / seri, süreç medyanı | Önceki birleşim | Yeni varsayılan | Yorum |
|---|---:|---:|---|
| Vec API,2 dengeli çift, ingest |25,750 s |25,034 s |2/2 düşük |
| Vec API, son bakım |6,119 s |6,332 s |2/2 yüksek; kapatılmadı |
| Vec API, toplam |31,869 s |31,365 s |-%1,58;2/2 düşük |
| Streamed API, ilk4 dengeli çift, ingest |23,512 s |24,684 s |3/4 yüksek |
| Streamed API, ilk4 çift, son bakım |0,863 s |0,792 s |3/4 düşük |
| Streamed API, ilk4 çift, toplam |24,311 s |25,498 s |+%4,88;1/4 düşük |
| Streamed API, ayrı2 dengeli çift tekrar, toplam |24,079 s |23,870 s |-%0,87;2/2 düşük |

İlk +%4,88 saklanmadı; bu yüzden aynı exe A/A ve değiştirilmemiş iki
exe ile ayrı tekrar yapıldı. A/A2 çiftte iki etiket de aynı SHA'lı yeni
varsayılan: toplam26,863→26,238s (-%2,33), CPU-%3,27; iki çiftte de
fark var. A/A'nın oranı A/B'den çıkarılmadı. Ayrı tekrarda ilk artış
sürmedi. CPU/süre yönü kararsız olduğu için taşımaya yeni bir hız
kazancı veya kesin sıfır-regresyon garantisi atfedilmiyor.

Önceki gram algoritması adımının %11,49 yazma+bakım kazancı tarihli
kanıttır; bu tabloların kontrolü o optimizasyonu zaten içerir. Buradaki
farklar o algoritmanın yeni bir A/B ölçümü değildir. Büyük/küçük corpus
ve farklı oturum medyanları birbirine oranlanmaz.
[Vec API](benchmarks/stabilize-owned-write-10k-20261016-summary.json),
[ilk streamed seri](benchmarks/stabilize-streamed-write-10k-20261016-summary.json),
[aynı exe kontrolü](benchmarks/stabilize-aa-write-10k-20261016-summary.json),
[ayrı tekrar](benchmarks/stabilize-repeat-write-10k-20261016-summary.json).

## Bellek ve disk

| Proses sayacı / kapsam | Önceki birleşim | Yeni varsayılan |
|---|---:|---:|
| Streamed ilk4 çift, OS lifetime peak private commit |9,156 MiB |9,252 MiB |
| Streamed ilk4 çift, OS lifetime peak working set |13,104 MiB |13,139 MiB |
| Streamed tekrar2 çift, lifetime peak private commit |9,273 MiB |9,182 MiB |
| Streamed tekrar2 çift, lifetime peak working set |13,211 MiB |13,043 MiB |
| Vec API2 çift, lifetime peak private commit |79,178 MiB |79,111 MiB |
| Strict open+5s boşta,2 süreç private |1,893 MiB |1,889 MiB |
| Strict open+5s boşta,2 süreç working set |5,586 MiB |5,594 MiB |

İlk streamed seride private commit +98 KiB/working set +36 KiB; tekrarda
ikisi düşük. Sıfır bellek artışı iddiası yoktur; yaklaşık9MiB sınıfındaki
streamed sonuç korunur. Vec API'nin caller batch/delta maliyeti bu
sonuçla kapanmaz. Private commit fiziksel RAM değildir; working set
paylaşılan image sayfalarını da içerir. Lifetime sayaçları Windows OS
high-water mark'ıdır ve input/open/index/bakım toplam proses ömrüne aittir.
Faz sayaçları20ms örnek/100ms poll; faz sınırları ve kısa faz tepeleri
kaçabilir. Boşta sayaçlar query/oracle çalıştırmaz. Query peak, kernel
handle ve toplam sistem/file-cache RAM'i bu sonuçların yerine geçmez.

16 streamed ingestte122 .fdb/.idx/.ready dosyasının SHA256'ları birebir
aynıdır; önceki gram adımının122 dosyasıyla da aynı kalmıştır.
Retained core1.521.041.630 B. Vec API kendi dört ingestinde aynı
1.514.639.487 B üretir; flush/generation düzeni iki API arasında farklıdır.
Disk küçülmedi; eski nesiller hâlâ tutulur. Okuma/CLI kontrolleri sonrası
kaynak, indeks ve hazırlanmış cache hash'leri korundu.

## Arama, get ve açılış

Dört dengeli süreç çifti, aynı mevcut38-case seti (yaygın/dar/iki
karakter/AND/Unicode/olumsuz/ileri sayfa), her primary query80 ve her
sayfa20 örnek/variant. Bütün sonuçlar total/tam belge/version/sequence,
aynı256 get anahtarı ve missing-key durumlarıyla doğrulandı. Geniş
matrix24 düşük/14 yüksek pooled p95; yükselen hiçbir durum dört çiftin
tamamında yavaş değildir. Bu,14 artışı otomatik kapatmaz.

Yoğun sayfalar ayrıca4 süreç çifti×8 kısa blok×25 turla800 örnek/sayfa/
variant:5 düşük/3 yüksek p95. Artışlar the /0/200 için +%0,26,
ishmael /130/1 için +%1,29, quixote /57/50 için +%1,30. Sonuncu3/4 çiftte yüksek; bu üçü de
4/4 artış vermez. Blok medyanı ve pooled p95 ayrı istatistiklerdir.
Bu yeni kısa-oturum/cache düzeni önceki kesintisiz oturumun bütün
yerleşim/ortam etkilerini açıklamaz. Kaynak kontrolleri ısınmada korunur;
ilk kaynak doğrulaması warm-query p95'ine dahil değildir.
[38 durum](benchmarks/stabilize-read-matrix-10k-20261016-summary.json),
[yoğun sayfalar](benchmarks/stabilize-read-pages-10k-20261016-summary.json).

| Açılış/ilk kullanım,4 yeni süreç medyanı | Kontrol | Yeni varsayılan |
|---|---:|---:|
| Normal strict open, tam kaynak doğrulaması |490,592 ms |480,998 ms |
| Açıkça seçilen record-read open |176,097 ms |171,439 ms |
| Record-read ilk get9999 |0,135 ms |0,125 ms |
| Record-read ilk search, pending kaynakları kontrol eder |271,466 ms |257,393 ms |
| Aynı query tekrar search |2,435 ms |2,329 ms |

Fresh process sıcak OS-cache demektir; gerçek cold-cache veya p95 açılış
kapısı değildir. Record-read ilk get yalnız gerekli kaydı doğrular;
ilk search5 pending kaynağın tamamını doğrular. Normal open değişmedi ve
yaklaşık481ms maliyetini ödemeye devam eder; SQLite'ın düşük açılış
maliyetine ulaşıldığı iddia edilmez.
[Koşullar/ham açılışlar](benchmarks/stabilize-open-10k-20261016-design.json).

## SQLite referansı

İlk streamed serinin0 ve3 çiftlerinden sonra aynı ilk10k ile iki yeni
SQLite ingest/verify yapıldı: ingest34,866s, son bakım1,289s,
toplam36,155s medyan; peak private8,398MiB/working set12,637MiB,
disk583.860.224 B. FULL WAL/cache2MiB/mmap0/tempFILE; FTS optimize ve
checkpoint(TRUNCATE), FDB bakım algoritmasıyla aynı iş değildir.
Bu iki referans FDB'nin her çiftine eşleştirilmiş hız kapısı değildir.
Geniş FDB matrix'inden sonra ayrıca dört-query/get tek SQLite referansı
vardır; tüm38 sorgu için aynı tekrarlı SQL performans kapısı değildir.

SQLite10k payload doğrulamaları, integrity_check ve FTS external-content
integrity geçti. Validator transaction rollback edildi ve SQLite DB
SHA'sı korundu. Validator3.53.1/timed comparator3.53.2 farkı ölçüm kapsamı
olarak korunur. FDB'ye SQLite runtime bağımlılığı eklenmedi.
[SQLite veri ve yazma referansı](benchmarks/stabilize-streamed-write-10k-20261016-summary.json),
[integrity kanıtı](benchmarks/stabilize-sqlite-integrity-20261016.jsonl).

## Doğrulama ve karar

`cargo test --all-targets`: default151, reference/no-default92,
all-features170 geçti;4/2/6 ignored entry bağımsız atlanan işlevler değil,
üst testlerin çağırdığı subprocess helper'lardır. Cut, corruption,
process-kill/merge-kill, atomik guard, mixed normal/streamed writer,
shared-budget deadlock,0.1/JSON uyumu ve Unicode/çakışma kontrolleri dahil.
Default/all-features `cargo clippy --all-targets -- -D warnings` ve
`cargo fmt --check` geçti. Eski feature alias'ıyla ve ayrı comparator'la
check de geçti. Son normal worker/reader/memory `.text` bölümleri ölçülen
adaylarla aynı. `fdb.exe` varsayılan release olarak derlendi; mevcut
10k üzerinde stdio stats/get/search doğrulandı, yazma yapılmadı.

Kaynak/özellik eşdeğerliği, hata/kurtarma testleri, tam sonuç ve disk
eşitliği, düşük-RAM seviyesinin korunması üzerinden bu belirli birleşim
varsayılanda kalıcılaştırıldı. İlk yazma farkı,14 geniş-query artışı ve
üç küçük sayfa artışı kanıtsız kapatılmadı. Genel hız hedefi, Vec/query
RAM'i, disk/nesil temizliği ve concurrent/gerçek-cold latency kapıları
ayrı açık işlerdir. [İş takibi](OPEN-WORK.md),
[tüm kaynak/veri/test/CLI kanıtı](benchmarks/stabilize-final-proof-20261016.json).
