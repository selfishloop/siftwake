# Aynı sonuç sözleşmesi ve maliyet profili — 08.10.2026

Bu adım motorun kalıcı biçimini değiştirmez. Native SQLite/FDB karşılaştırması
tam sayfa içeriğiyle doğrulanır, anahtarla okuma eklenir ve ayrı FDB teşhis
binary'sinde wall-time bileşenleri ölçülür. Sonuç: **bu corpus'ta sorguların
çoğunda FDB hızlıdır; asıl açık yazma/bakım ve proses tepe RAM'indedir.**
SQLite'tan düşük RAM ve bütün iş yüklerinde daha yüksek hız hedefi tamamlanmadı.

## Karşılaştırma sözleşmesi

Windows release, native Rust harness; yeni DB dizinlerinde üç bağımsız çift:
FDB/SQLite, SQLite/FDB, FDB/SQLite. 100.000 kayıt, 256 UTF-8 bayt tekrarlı Türkçe
metin, on basamaklı ID/başlık, shared/contents scope/collection, day/source
alanları, 500'lük batch. `bench_support/corpus.rs` iki tarafta aynıdır.

İlk yazmalar Missing/unique davranışındadır. FDB kalıcı WAL sync; SQLite
3.53.2 WAL + synchronous FULL, 2048 KiB requested cache, mmap kapalı,
temp_store FILE. SQLite şeması UNIQUE key, source/range B-tree indeksleri ve
external-content FTS5 trigram indeksi + insert/update/delete trigger'larıdır.
SQLite yalnız ayrı benchmark paketinin bağımlılığıdır.

İki motor aynı mantıksal sorgu sözleşmesini gerçekleştirir: exact total,
scope/collection filtresi, ID sırasıyla en fazla 50 tam çözümlenmiş
StoredDocument. `bench_support/verify.rs` bağımsız corpus oracle'ı, her cevapta
**bütün belge, version ve sırayı** denetler. Oracle en fazla bir sayfayı
materialize eder; N kayıtlık referans ID listesi oluşturmaz. Kontroller timed
sorgu aralığının dışındadır. Her sorgu bir untimed warmup sonrası ölçülür:
dar metin/exact/range/ikinci sayfa 30; son sayfa/geniş metin üç tekrar.
100 deterministik farklı anahtarla okuma da bir warmup + üç tekrar kullanır.

Her motor 50 güncelleme/50 silme, count ve reopen kontrolü yapar. Geniş metin
ölçümü bu değişikliklerden sonra aynı benchmark prosesinde çalışır ve count
99.900'dür. SQLite ayrıca ayrı full DB/FTS integrity check yapar.

**Eşdeğer olmayan sınırlar:** Fiziksel indeksler aynı değildir. FDB dinamik
alan/link/guard doğrulamaları, kayıt CRC/identity kontrolleri ve kaynak replay'i
yapar; SQLite harness genel FDB API'sinin tamamını uygulamaz. FDB normal
açılışı tüm kaynakları doğrular; SQLite open full integrity scan değildir.
Literal `rapor-0000050000` ve `teknoloji` sorguları karşılaştırılır; arbitrary
LIKE wildcard/çok terimli sorgu eşdeğerliği burada kanıtlanmadı. Power-loss
davranışı SQLite tarafında bu harness ile test edilmedi. Kalıcılık ayarlarını
eşlemek her recovery garantisinin eşit olduğunu kanıtlamaz.

Yerel OS/file cache ingest ve warmup ile ısınır; cold-cache değildir. Genel
random workload, büyük/çeşitli metin, concurrent client ve milyon kayıt ölçeği
bu turda ölçülmedi. FDB experimental-streaming-open açık; diagnostic-timing
ve allocation sayaçları karşılaştırma binary'sinde kapalıdır.

## Üç run medyanı

Sorgu rakamları her run p50'sinin medyanıdır; native API ve payload çözümü
dahil, IPC yoktur. Ingest corpus üretimini içerir. SQLite batch içi timer
payload hazırlığını dışarıda bıraktığı için batch p50 üzerinden üstünlük
iddiası yapılmaz; toplam ingest ve son bakım esastır.

| Ölçüm | FDB | SQLite |
| --- | ---: | ---: |
| Ingest | 11,252 s | 7,921 s |
| Son bakım | 9,417 s | 0,307 s |
| Ingest + son bakım | 20,668 s | 8,229 s |
| Dar metin, tek hit | 3,7403 ms | 19,4693 ms |
| source=synthetic, ilk 50 + total | 0,2509 ms | 15,2352 ms |
| Aynı filtre, offset 50 | 0,2504 ms | 13,6870 ms |
| source=synthetic, day 2..5 | 0,5079 ms | 2,6869 ms |
| Aynı exact filtre, offset 99950 | 1,8044 ms | 26,7287 ms |
| Geniş metin teknoloji, değişiklik sonrası | 273,6802 ms | 454,8290 ms |
| 100 farklı key ile tam belge okuma | 2,3413 ms | 1,2916 ms |
| Proses tepe working set | 88,1602 MiB | 8,9492 MiB |
| Örneklenen private committed tepe | 85,0117 MiB | 4,8711 MiB |

FDB yazma+bakım yaklaşık 2,51 kat yavaş; proses working-set tepesi yaklaşık
9,85 kat yüksek. Alan filtreleri ve bu iki metin sorgusunda daha hızlı;
100-key okumada daha yavaş. Bu sayılar configured comparator'a aittir;
SQLite'ın her şemada/ayar altında performans üst sınırı değildir.

Bellek penceresi **bütün benchmark prosesi**: open, ingest, maintenance,
reopen, sorgular, update/delete; SQLite'ın ayrıca final integrity işi dahil.
OS PeakWorkingSet ve 20 ms istenen private sampling kullanılır. Kısa private
tepeler kaçabilir; gerçek örnekleme aralığı Windows scheduling ile uzayabilir.
Monitör ve sistem genelindeki OS file cache dahil değildir. Harness ve çözülen
çıktı belgeleri proses sayacına dahildir; boşta bellek ölçümü değildir.

FDB dizin toplamı 459,82 MiB, SQLite 102,23 MiB çıktı. FDB geçmiş, artık aktif
olmayan run/source/cache dosyalarını henüz GC etmediği için bu sayı aktif
indeks veya canlı kayıt başına alan maliyeti değildir. Disk farkını yalnız
kayıt biçimine bağlamak yanlıştır. FDB aktif kaynak/cache boyutu raw stats'tadır.

Reopen medyanı FDB 761,85 ms, SQLite 3,18 ms; yukarıdaki farklı integrity
sözleşmeleri nedeniyle aynı işi yapan açılış karşılaştırması sayılmaz.
SQLite ayrı integrity-check zamanı raw result içinde tutulur.

## Ayrı FDB teşhis koşusu

`diagnostic-timing` ile aynı 100 bin kayıt/256 bayt corpus'u yeni bir dizinde
çalıştırıldı. Bu binary gerçek hız/RAM karşılaştırmasına dahil edilmedi.
Timer, Mutex ve sayaç maliyeti vardır; ölçümler **wall span** süreleridir,
CPU sampling veya allocation attribution değildir. Nested span ve background
worker süreleri örtüşür; toplanamaz. Snapshot sırasında bitmemiş worker span'ı
bir sonraki pencereye geçebilir. Özellikle final maintenance'taki run span
toplamının pencere elapsed süresinden büyük olması bu sebeple mümkündür.

Profilde 200 commit'in toplam span süresi 11,205 s:

| Bileşen | Profil wall span toplamı | Yorum |
| --- | ---: | --- |
| Commit foreground maintenance | 5,617 s | Flush/bakım commit yolunu bekletiyor |
| Delta/index uygulaması | 4,894 s | RAM indeks güncelleme işi belirgin |
| Validate + guard/version | 0,272 s | Bu corpus'ta ana maliyet değil |
| Delta size için JSON üretimi | 0,118 s | Gereksiz iş var, fakat tek başına ana sorun değil |
| Frame JSON + CRC, worker dahil | 0,420 s | Commit dışı source frame'lerini de içerir |
| WAL sync, worker dahil | 0,221 s | İşlemci/disk kesin attribution değil |

Foreground maintenance ve delta uygulaması commit'e ait ayrı bölümlerdir;
diğer satırlar worker/nested işleri içerebildiği için commit süresi yüzdesi
olarak toplanmaz. Durability'yi gevşetmek bu kanıta göre ilk çözüm değildir.

Ingest penceresinde Builder.add 176.063, final bakımda 163.937 çağrı: toplam
**340.000** belge işleme. 100 bin yeni kayıt için flush/merge zinciri belgeleri
birden fazla kez işler. Builder.add aggregate span'ı iki pencerede toplam
13,693 s; concurrent işler nedeniyle toplam elapsed ile eş tutulmaz.
Bu, yazma hattı/index üretimini önce hedeflemek için doğrudan kanıttır.

Geniş metin: dört arama (warmup dahil) search.total 1.223,589 ms;
filter/visibility/text 1.222,099 ms, son sayfa hydration 1,486 ms.
399.600 payload çözümü yapıldı. İç içe decode/CRC/identity toplamı 788,621 ms,
bunun içindeki JSON deserialize 587,599 ms; JSON bu instrumented search span'ın
yaklaşık %48'idir. Positional read aggregate 108,673 ms; OS cache sıcaktır ve
bu span yalnız metin payload I/O'sunu kapsamaz. Bu query penceresi öncesindeki
update/reopen sayaçları da raw satırdadır; yukarıdaki attribution yalnız search
ve text prefix'li sayaçlardan alınır.

Dar metin: 31 aramada toplam 125,442 ms; 47.156 positional read span'ı
106,277 ms, 3.472 text payload decode 8,722 ms. Bu koşuda dar sorguda JSON'dan
çok posting/metadata erişimi öne çıkıyor. Positional read süreleri metadata ve
posting okumalarını ayırmaz; daha kesin ayrım sonraki hedefli profile gerekir.

Kopyalama maliyeti ayrı bir CPU/byte counter'ıyla ölçülmedi. JSON süresinden
tek başına kaç bayt kopyalandığı veya binary formatın ne kadar kazandıracağı
çıkarılamaz. Önceki allocation profilleri transient RAM'in yüksek olduğunu
gösterir, fakat bu tur timing sayaçları heap/RSS ölçmez.

## Kanıta göre sonraki uygulama sırası

1. **Yazma hattı ve delta indeksleri:** tekrar tekrar indeks üretimini azaltmak;
   flush/merge builder'ını küçük bloklarla üretmek; delta + builder + worker
   için ortak allocation bütçesi. 340 bin builder çağrısını ve foreground
   beklemeyi azaltıp aynı durable batch sonucunu korumak ilk kabul kapısıdır.
2. **Dar sorguda posting/metadata erişimi:** positional read'leri türüne göre
   ayırmak; adayları bloklarla birleştirmek. Cache'i sınırsız büyütmek yok.
3. **Geniş metinde belge çözümleme:** binary payload veya text-only decoder'ı
   ayrı prototipte ölçmek. Kaynak biçimi değişirse sürüm ve backward-compat
   testleri gerekir; bu tur biçim değişmedi.
4. Aynı native sözleşmede hız/RAM tekrar ölçümü; sonrasında çeşitli uzun
   içerik ve büyük ölçek. Şu anda yeni bir motor mimarisi hızlı ilan edilmez.

Dolayısıyla önce tüm motoru binary biçime geçirmek yerine, yazma/index
tekrarını ve shared transient bütçeyi düzeltmek daha güçlü kanıta dayanır.

## Doğrulama ve tekrar

All-features 65 test, varsayılan 50 test fixture/corruption/process-kill kapsamıyla geçti;
shared oracle'ın sayfa/sürüm/update/delete testi de geçti. Native SQLite iki
testi geçti (oracle ve trigger/transaction rollback). Cargo fmt ve iki pakette
Clippy -D warnings kontrolleri geçti. SQLite root motor bağımlılığına eklenmedi.

```powershell
cargo build --release --features experimental-streaming-open --example measure_scale
cargo build --release --locked --manifest-path tools/native_sqlite/Cargo.toml
tools/measure-native.ps1 -Executable output/fdb-equivalent-native-v2-20261008.exe -DatabasePath data/new-fdb -Count 100000 -OutputPrefix output/new-fdb
tools/measure-native.ps1 -Executable output/sqlite-equivalent-native-v2-20261008.exe -DatabasePath data/new-sqlite -Count 100000 -OutputPrefix output/new-sqlite
cargo build --release --features experimental-streaming-open,diagnostic-timing --example measure_scale
```

Ölçümler sırayla çalıştırılır. Yeni DB dizinleri gereklidir; mevcut corpus'a
yazılmaz. Default CLI tekrar ayrı derlenir, teşhis feature'ı default değildir.

Ham veri: `docs/benchmarks/equivalent-native-20261008-{fdb,sqlite}-{0,1,2}.jsonl`
ve `.memory.json`; özet `equivalent-native-20261008.summary.json`.
Teşhis `docs/benchmarks/fdb-cost-profile-20261008.jsonl`.
`tools/summarize-native-pairs.py` eksik/başarısız/yanlış corpus koşularını reddeder.

SHA256:

- FDB uninstrumented: `D84D82E6FDC14C7A0BB95F309287AD90C5C3E2E59C5C4CE58953076D229FF22F`
- SQLite native: `28E9707EA778BBCE75E17E5030D605E2CCA51A3612FBAD145D9A17D33A69E181`
- FDB teşhis: `CABF7B235DEDAE1E072832271FEB011AEDFE4C069CE860459E09114A1702CB44`
- İş sonunda yeniden derlenen default CLI: `9BA65BAFE5CA1CAE0CC6FE0057EDC01CE96FD94EE1BE4F0479CB8CB45B914AE2`
