# Varsayılan FDB / SQLite — aynı 10k, 2026-10-16

Bu 10k koşulunda varsayılan FDB, akışlı yazma+bakımda %31,62 düşük süre
ve38/38 arama/sayfalama durumunda düşük p95 verdi. Her arama durumunda
dört çiftin dördü de FDB lehine. Oracle içermeyen query/get bellek testinde
private/working set FDB'de düşük. Normal strict açılış, disk ve normal Vec
yazma belleği ise açık eksiklerdir. Bu görev motor optimizasyonu değildir;
engine, bağımlılıkları, kalıcı formatları ve protokolü değiştirilmedi.

## Yazma ve bakım

Süreler süreç medyanıdır; toplam her süreçte ingest+maintenance olarak
hesaplanıp sonra medyan alınır. Tekil sütun medyanlarının toplamı değildir.

| Seri | FDB | SQLite | Kapsam |
|---|---:|---:|---|
| Akışlı ingest |22,750 s |33,434 s |4 dengeli çift |
| Akışlı son bakım |0,789 s |1,062 s |Farklı bakım algoritmaları |
| Akışlı toplam |23,598 s |34,509 s |-%31,62;4/4 hızlı |
| Akışlı proses CPU |21,641 s |33,422 s |-%35,25 |
| Vec ingest |22,911 s |48,215 s |2 dengeli çift |
| Vec son bakım |5,104 s |1,646 s |FDB bu API'de daha yavaş |
| Vec toplam |28,015 s |49,861 s |2/2 FDB hızlı; aşağıdaki oturum farkı geçerli |

Akışlı dört çiftte toplam farklar -%31,40/-%31,61/-%32,44/-%31,13.
İlk Vec serisindeki SQLite toplamları47,752/51,971s iken sonraki akışlı
referanslar34,366–34,693s aralığında. SQLite yazma kodu/input/ayarları
bu iki seride aynıdır; CPU da ilk seride yüksektir. Sebebi bu benchmark'ta
ayrıştırılmadı. Bu iki SQLite grubunu tek medyana yığmadık; Vec oranı
genel/kalıcı kazanç sayılmaz. Önceki oturum medyanlarıyla yeni bir motor
optimizasyonu iddia edilmez. Ana yazma karşılaştırması dört dengeli akışlı
çifttir. Vec bakım maliyeti ve tekrar kodlama/indeksleme işi açık kalır.

## Arama ve tek belge okuma

30 primary ve8 ek sayfanın tamamında pooled p95 FDB lehine; her durum
4/4 çiftte düşük. Aramalar total ve tam payload sayfasını içerir.

| Sorgu, offset0/limit50 | FDB p95 | SQLite p95 |
|---|---:|---:|
| `quixote`, dar |4,051 ms |15,434 ms |
| `the`, yaygın |1,494 ms |1087,192 ms |
| `he`, iki karakter |1,443 ms |1065,498 ms |
| `quixote whale`, AND |4,696 ms |14,213 ms |
| `zx`, olumsuz iki karakter |224,532 ms |1433,879 ms |
| `the zx`, olumsuz AND |206,421 ms |1629,431 ms |

İleri sayfalarda the /9950/50:2,009/1143,884ms, ishmael /60/200:
14,871/65,435ms. FDB'nin `zx` ve olumsuz AND taramaları hâlâ yaklaşık
200ms; SQLite'tan hızlı olması bu mutlak gecikme işini kapatmaz.
[Bütün38 durum ve çift yönleri](benchmarks/sqlite-default-10k-20261016-table.md).

Yaygın/iki karakterli sayım farkı, bu FTS5/LIKE yolunun aday metinlerini
kontrol etmesine karşı FDB'nin uygun durumda indeks kanıtı kullanmasıyla
uyumludur. Bu turda ayrı instrumented CPU atıf profili çalıştırılmadı;
SQL parse maliyeti veya yalnız normalleştirme maliyeti ayrıştırılmadı.
SQLite için alternatif MATCH/yardımcı indeks/özel kanıt planı denenmedi;
sonuç bütün SQLite sorgu planlarının üst sınırı değildir.

| Anahtarla tam belge get, p95 | FDB | SQLite | Örnek/motor |
|---|---:|---:|---:|
|2–4 KiB |0,0301 ms |0,0547 ms |1280 |
|4–16 KiB |0,0340 ms |0,0773 ms |5280 |
|16–32 KiB |0,0424 ms |0,1090 ms |6640 |
|32–50 KiB |0,0481 ms |0,1410 ms |7280 |
|Eksik anahtar |0,0009 ms |0,0104 ms |240 |

## Bellek, disk ve açılış

| Proses sayacı / kapsam | FDB | SQLite |
|---|---:|---:|
| Akışlı ingest lifetime peak private |9,156 MiB |8,402 MiB |
| Akışlı ingest lifetime peak working set |12,900 MiB |12,637 MiB |
| Vec ingest lifetime peak private |78,912 MiB |8,402 MiB |
| Vec ingest lifetime peak working set |82,506 MiB |12,646 MiB |
| Ayrı query/get lifetime peak private |9,189 MiB |12,740 MiB |
| Ayrı query/get lifetime peak working set |12,824 MiB |16,695 MiB |
| Strict open+5s idle private örneği |1,766 MiB |1,238 MiB |
| Strict open+5s idle working set örneği |5,516 MiB |5,375 MiB |
| Retained core disk, son akışlı layout |1.521.041.630 B |583.860.224 B |

Akışlı ingest private +%8,97/working set +%2,09; query/get proses tepesinde
FDB private yaklaşık%28 düşük. Bu farklı fazlar tek RAM iddiasında
birleştirilmez. Vec/JSON Lines yolunun caller/delta maliyeti hâlâ vardır;
akışlı9MiB sayısı bu yola ait değildir. Genel/aktif-delta query RAM kapısı
bu sekiz durumluk memory probe ile kapanmaz.

Disk FDB'de2,61 kat. FDB .fdb803.702.647 B + .idx717.327.822 B +
.ready11.161 B; eski nesiller dahildir. Yardımcı cache164.480 B eklenince
gerçek dosya envanteri1.521.206.110 B'dır. SQLite583.860.224 B. Corpus
her iki sayıya da dahil değil; geçici disk high-water mark ölçülmedi.

| Yeni süreç, sıcak OS cache;4 süreç medyanı | FDB strict | SQLite normal |
|---|---:|---:|
| Açılış |433,686 ms |1,535 ms |
| İlk get9999 |0,102 ms |0,229 ms |
| İlk dar search |4,628 ms |12,416 ms |
| Aynı search tekrar |2,380 ms |10,768 ms |
| Açılış+ilk get+ilk search alt süreleri toplamı |438,380 ms |14,159 ms |

FDB record-read seçeneği ayrıca167,520ms open +238,990ms ilk search,
toplam406,668ms medyan verdi. Normal strict open yerine otomatik seçilmedi.
Erteleme doğrulama maliyetini ortadan kaldırmaz. SQLite normal open tam
kaynak bütünlüğü taramaz; bu fark ölçümden çıkarılmadı veya eşit iş diye
sunulmadı. Açılış/ilk kullanım gecikmesi çözülmedi.

## Son doğrulama

Reader example4 ve native comparator4 helper testi geçti; prepared-cache
değişikliğinden sonra comparator test/Clippy/release build tekrar geçti.
Root `cargo fmt --check`, comparator fmt ve root/comparator
`cargo clippy --all-targets -- -D warnings` geçti. Motor kaynak/manifest/
lock dosyaları değişmediğinden bu tur yeni storage/kurtarma kodu yoktur.
Önceki commit'in kesilme/bozulma/process-kill testleri bu tur yeniden
çalıştırılmış gibi sunulmaz.

Her ingestte10k tam belge doğrulandı; ayrı okuma serilerinde tüm38 arama
sayfası ve version/sequence/get kontrolleri geçti. FDB core hash'leri her API'nin tekrarlı ingestlerinde
aynı. Okuma/ilk kullanım/bellek testleri boyunca kaynak/indeks/yardımcı
cache hash'leri aynı. Corpus'un2.740.862.279 B boyutu ve SHA256'sı,
data altındaki aynı iki klasör ve0.1 fixture korundu. SQLite tam integrity
ve external-content FTS integrity geçti; doğrulama transaction'ı rollback
edildi, ana DB hash'i korundu. Validator SQLite3.53.1, timed native3.53.2;
bu sürüm farkı açık bir kapsam sınırıdır.

[Makine/corpus kaydı](benchmarks/sqlite-default-10k-20261016-environment.json),
[tüm süreler/samples/çiftler](benchmarks/sqlite-default-10k-20261016-summary.json),
[kaynak/corpus/fixture/bütünlük/disk kanıtı](benchmarks/sqlite-default-10k-20261016-proof.json).

## Koşullar ve eşdeğerlik

- Başlangıç motor commit'i `8b90c11a5356ef427f4b83c21e82235765985962`.
  Normal release/default `bounded-engine`; diagnostic sayaçlar kapalı.
- Mevcut corpus'un aynı ilk 10.000 belgesi: 264.842.858 B gövde,
  2.057–51.197 B/pasaj, sekiz kitaptan farklı metinler. Yeni corpus,
  indirme, DB kopyası veya yeni DB klasörü yok. Yalnız sahiplik kontrollü
  `data/benchmark-work/fdb` ve `sqlite` yazma ölçümlerinde sıfırlandı.
- Windows, Ryzen 7 7840HS; sıcak OS cache, eviction/CPU affinity/power
  izolasyonu yok. Süreçler sırayla çalıştı; ölçüm sırasında derleme/test yok.
- Native SQLite 3.53.2, FTS5 trigram external-content indeks,
  `detail=none,columnsize=0`, FULL WAL, 2 MiB sayfa cache isteği, mmap kapalı,
  temp_store FILE. Reader en fazla 64 hazırlanmış SQL ifadesini yeniden
  kullanır; bu cache sayfa cache'inden ayrıdır ve proses belleğine dahildir.
- İki motorda da write batch 500, 20 atomik işlem, senkron WAL; input CRC,
  JSON çözme, normalleştirme/indeksleme ve 40 sınır get kontrolü yazmaya dahil.
  FDB'nin son bakımı flush/merge drain; SQLite'ınki FTS optimize/checkpoint.
  Bunlar aynı bakım algoritması değildir. Her ingest sonrasında 10k tam belge
  ve version ayrı süreçte doğrulandı; doğrulama yazma süresinin dışındadır.
- Akışlı API / SQLite dört dengeli çift; normal Vec API / SQLite iki dengeli
  çift. İki API aynı bellek sonucu gibi sunulmaz. Okuma testleri son akışlı
  ingest'ten kalan DB'yi yeniden kullanır; derived record cache hazırlığı
  ölçüm dışındadır.
- 38 aynı contains/AND/fold/pagination durumu, 256 aynı get anahtarı ve üç
  eksik anahtar; dört dengeli süreç çifti. Her primary query süreç başına20,
  her ek sayfa5 tekrar: pooled p95 primary80, page20 örnek. Query/get zamanına
  tam payload dönüşümü dahil; oracle hazırlığı, warmup ve tam eşitlik kontrolü
  dahil değil. Bütün sayfalarda total/sıra/tam belge/version doğrulanır.
  FDB sequence20 de korunur. Bunlar tam-kelime sorguları değildir.
  Süreler Rust API/native SQLite çağrılarıdır; JSON Lines, servis veya IPC
  taşıma maliyeti bu ölçüme dahil değildir.
- SQLite LIKE koşulları FTS5 üzerinden çalışır; her arama tek snapshot içinde
  count + seçilmiş tam JSON payload sayfasını getirir. Set içinde LIKE
  wildcard'ları yoktur. FDB'nin bütün dinamik field/link/guard yetenekleri
  veya SQLite'ın bütün kullanım biçimleriyle eşdeğerlik iddiası yoktur.
- Normal strict open / SQLite normal open dört dengeli yeni süreçte ölçülür.
  FDB tüm kaynakları doğrular; SQLite normal open tam integrity_check yapmaz.
  FDB record-read open ayrıca açıkça seçilerek ölçülür. Yeni süreç, gerçek
  cold-cache değildir. İlk get/search/warm tekrar ayrı sürelerdir; açılış
  toplamlarına süreç başlatma ve istatistik/count yazdırma dahil değildir.
- Ayrı query-memory sürecinde corpus/oracle yüklenmez: üç tur,24 arama ve771
  get. Sekiz durum yaygın/dar/iki karakter/AND/olumsuz/ileri/200-belgelik sayfa
  içerir. Bu yolda count/key/order/version kontrolü yapılır; tam payload
  eşitliği bağımsız matrix'te doğrulanır. Bellek izleme var, hız kapısı yok.
  İki dengeli çift ve ayrıca aramasız strict open+5s idle iki çift vardır.

## Sayaçların kapsamı

Windows OS lifetime peak private commit, fiziksel RAM veya toplam sistem
belleği değildir. Peak working set paylaşılan image sayfalarını da içerir;
OS dosya cache'i ve izleyen PowerShell süreci bu sayaçlarda yoktur.
Lifetime tepe opening/input/query/result/index/maintenance proses ömrünün
tamamına aittir. Query tepesinden idle sayısı çıkarılarak query allocation
iddia edilmez. Idle private/working set o andaki örnektir. Faz sayaçları
20 ms örnek/100 ms faz poll; kısa faz sınırları kaçabilir. OS high-water
mark'ı süreç handle'ı tutulup çıkış sonrası okunur, örnekleme aralarını kapsar.

Disk core sayacı .fdb/.idx/.ready veya SQLite DB/WAL dosyalarını kapsar;
FDB eski nesilleri içerir. Yardımcı .rmp/.svfy cache'leri ayrıca envanterde
vardır. Mantıksal okuma byte'ları fiziksel disk okuması veya yeni depolanan
veri olarak sunulmaz.

## Açık kalan kapılar

Bu seri bakım sonrası sabit belge setini ölçer. Güncelleme/silme churn'ü,
aktif delta üzerinde okuma, eşzamanlı okuyucu/yazıcı gecikmesi, gerçek cold I/O,
güç kesilmesi ve toplam OS/file-cache RAM'i ölçülmez. Önceki eski/yeni motor
arama regresyonları bu SQLite karşılaştırmasıyla otomatik kapanmaz.
Yazma/bakım tekrarları, normal Vec/query RAM'i, disk/eski nesil temizliği ve
eşzamanlı/cold yük [OPEN-WORK](OPEN-WORK.md) içinde takip edilir.

Runner: [measure-sqlite-default.ps1](../tools/measure-sqlite-default.ps1).
Özetleyici yalnız log okur: [summarize-sqlite-default.py](../tools/summarize-sqlite-default.py).
Koşul/corpus/hash kaydı: [environment](benchmarks/sqlite-default-10k-20261016-environment.json).
