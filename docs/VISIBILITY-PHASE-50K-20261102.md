# Görünürlük aşama tanısı — 2 Kasım 2026

3 Kasım sınıflandırması: **Ek-VP**, brifteki 2a yazma görünürlüğünün takip
tanısıdır. Gerçek soğuk cache (2b), uzun snapshot (2c) veya özgün 3a kapısının
yerine geçmez. [Brife göre durum](BRIEF-BLOCK-STATUS-20261103.md).

**Yalnız tanı, kapı yok.** Tamamlanan kol 4/4. B kabul edildi; eski görünürlük KALDI kaydı/eşiği değişmedi. Bu tur motor/format/varsayılan optimizasyonu yapılmadı.

**Ölçülen yer: ilk marker sorgusunun posting/terim-sözlüğü hazırlığı.** Numeric/Frunner FDB gap ortalamaları 26.936/30.115ms; ilk sorgu gapin %99.972/%99.974'ü. Posting hazırlığı %98.58/%98.65; term-cache edinimi 11.501/13.329ms ve sayfa okuma/CRC/decode 8.780/9.454ms. Bu alt süreler nested toplamların üstüne tekrar eklenmedi.

Acquire 0.000359/0.000405ms; aynı commit sequence hemen edinildi. Query maskesi 0.024/0.027ms, delta 0.239/0.247ms. Snapshot hazırlığı 0.185/0.223ms ve maskesi 0.097/0.129ms commitin İÇİNDE ölçüldü. Bu koşuda commit sonrası yayımlanacak sürümü bekleme değil, edinilmiş sürüm üzerindeki ilk doğrulama aramasının çalışması baskın.

SQLite gap 5.742/5.689ms, ilk sorgu 5.572/5.519ms. Bu tanıdaki daha düşük uçtan uca sürenin farkı ilk-query aşamasında ölçüldü; geçmiş B bloklarının neden değiştiğini bu ölçüm tek başına kanıtlamaz.

**Önemli sınır:** eski Bdeki yaklaşık15ms bu tanıda aynı sayı olarak üretilmedi: profilsiz numeric B ilküç blok gerçek işlem-bazlı gap p95leri14,719–16,831ms; yeni tanıda numeric/Frunner gap ortalamaları27–30ms, p95leri45,799/48,629ms. Sayaçların maliyeti ile oturum/ilerlemiş yerleşim etkisi ayrı A/B ile ölçülmedi, payları **bilinmiyor**. Eski profilsiz blokların tam aşama dağılımı **bilinmiyor**; aşağıdaki tarihsel gapler yalnız ham işlem farklarıdır.

## Yöntem ve zaman sınırları

Aynı881756DA ilk50k taban ve önceki257-record ayrı havuz. Aynı256 donör/gövde/yol,4096 normal+1 uzun-yol transaction,4R beşli sorgu yükü,N60+. Tek seferlik bir dengeli blok: FDB numeric / SQLite numeric / SQLite Frunner / FDB Frunner. Reset/yeniDBkopyası/yeniden50kingest yok;private/ okunmadı. Ana50k ön/son oracle/digest ile korunur; havuzlar temizlenir. Sıcak/kontrolsüz OS cache, eviction yok.

Commit başlangıcından tek monotonic clock üzerindeki sınırlar: commit return → benchmark history yayımı → acquire → marker/Query hazırlığı → profiler init → ilk marker sorgusu → snapshot release. Ardışık sınır farkları ayrık ve toplamı visibility ile tam eşittir. Gap=visibility−commit; history kısmı motora ait değildir. Tam GET/eski-marker oracle daha sonra gelir ve visibility içinde değildir. Profiler sonuçlarını toplama/JSON/SQL EXPLAIN görünürlük zamanından SONRA yapılır ve ayrıca ölçülür.

FDB mevcut opt-in commit trace snapshot.prepare/mask ile ilk sorgunun request-local first.total/delta_filter/visibility_mask/posting/candidate sayaçları kullanıldı. Bunlar nested wall süreleridir; commit maskesi commit süresinin, delta/query maskesi ilk sorgunun alt kümesidir. Üst/alt süreler toplanıp yeni toplam üretilmez. Background okuyucular ayrı request context kullanır; yeni worker/izin politikası yok. SQLite yalnız writer probe için SQL hazırlama/step+payload kopyası/JSON decode alt zamanları; ilk plan EXPLAIN queryden sonra alındı.

Profil derlemesi ve sayaçlar zamanlamayı/4R kaynak takvimini etkileyebilir. Normal B p95leri bu tanıyla değiştirilmez; farklı seans/yerleşim etkisi ölçülmedi. Bu veri kabul kapısı veya önce/sonra kazanç deneyi değildir. Eski blokların neden farklı çıktığını bu tek tanıyla kesinleştirmiyoruz.

## Ayrık zamanlar — normal4096 işlem/kol ortalama ms

|Motor/anahtar|Commit|Commit→visibility gap|History|Acquire|Probe hazırlık|Profiler init|İlk sorgu|Release|
|---|---:|---:|---:|---:|---:|---:|---:|---:|
|fdb/numeric|2.846579|26.936318|0.001284|0.000359|0.002311|0.003284|26.928670|0.000411|
|sqlite/numeric|8.715702|5.741666|0.001715|0.141919|0.005167|0.000402|5.571985|0.020478|
|sqlite/frunner|8.677558|5.689019|0.001836|0.143148|0.005140|0.000390|5.519302|0.019204|
|fdb/frunner|2.904624|30.115378|0.001285|0.000405|0.002388|0.003487|30.107593|0.000220|

## Gap payları; aynı olayların toplamından yüzde

|Motor/anahtar|History %|Acquire %|Hazırlık %|Profiler init %|İlk sorgu %|Release %|
|---|---:|---:|---:|---:|---:|---:|
|fdb/numeric|0.005|0.001|0.009|0.012|99.972|0.002|
|sqlite/numeric|0.030|2.472|0.090|0.007|97.045|0.357|
|sqlite/frunner|0.032|2.516|0.090|0.007|97.017|0.338|
|fdb/frunner|0.004|0.001|0.008|0.012|99.974|0.001|

## FDB: ilk sorgu ve commit alt kümeleri

|Anahtar|İlk sorgu ort ms|Delta filtre ort ms|Delta /sorgu %|Delta /gap %|Query maskesi ort ms|Commit snapshot.prepare ort ms|Commit snapshot.mask ort ms|
|---|---:|---:|---:|---:|---:|---:|---:|
|numeric|26.928670|0.239170|0.888|0.888|0.023920|0.184759|0.097473|
|frunner|30.107593|0.246752|0.820|0.819|0.027153|0.222515|0.128783|

## FDB posting/sözlük hazırlığı altındaki mevcut sayaçlar

|Anahtar|Posting setup ort ms|Term-cache acquire ort ms|Contended cache-wait ort ms (nested)|Term-page decode/read/CRC ort ms|Read-at ort ms (nested)|Posting/gap %|Term-cache acquire/gap %|
|---|---:|---:|---:|---:|---:|---:|---:|
|numeric|26.554520|11.501314|11.384967|8.780212|1.832047|98.583|42.698|
|frunner|29.709789|13.328761|13.197052|9.453553|2.002256|98.653|44.259|

|Anahtar|Term-cache lock çağrı/probe|Contended cache lock/probe|Term page hit/probe|Term page miss/probe|Lookup hit/probe|Lookup miss/probe|İstenen read byte/probe|
|---|---:|---:|---:|---:|---:|---:|---:|
|numeric|1257.989|91.847|108.599|306.106|13.873|414.705|1277635.214|
|frunner|1357.551|112.618|116.427|331.195|14.687|447.621|1376099.589|

`query.term_cache_mutex_wait` bütün term-cache kilit edinme süresi; `lock.cache_wait` yalnız contended cache lockların bloklanan kısmı. Aynı bekleme iki sayaçta örtüşebilir; tekrar eklenmez. İkinci sayaç diğer cache mutexlerini de kapsayabilir; mutex sahibi okuyucu/yazıcı thread kimliği ölçülmedi. Term cache süreçte ortaktır; sayfa missinde read/CRC/decode aynı guard içindedir ([kod](../src/segment/term_pages.rs)). Bekleme ölçüldü; belirli bir thread, scheduler veya antivirüse neden atfedilmedi. `disk.read_at` decode içinde de bulunduğundan üstüne eklenmez. İstenen read byte gerçek OS cache miss/diskten okunan fiziksel byte değildir.

Delta filtre spanı birlikte gram filtre kontrolü, gerekirse WAL payload okuma/CRC, decode, fold/contains ve key dolaşımını kapsar ([kod](../src/engine/light.rs)). Bunların tek tek payları bu tur ölçülmedi: **bilinmiyor**. Wall süresinin saf CPU/I/O/OS-cache miss/scheduler/antivirüs dağılımı da **bilinmiyor**. Commit içindeki snapshot hazırlığını post-commit gap ile karıştırma.

## SQLite: ilk sorgu alt süreleri

|Anahtar|İlk sorgu ort ms|SQL kurma|Statement prepare|Step+payload copy|JSON decode|
|---|---:|---:|---:|---:|---:|
|numeric|5.571985|0.006089|0.002110|5.513568|0.039990|
|frunner|5.519302|0.006262|0.002102|5.459119|0.041807|

EXPLAIN (numeric, sequence736037; visibility dışında):

- MATERIALIZE selected
- SCAN search VIRTUAL TABLE INDEX 0:L0
- SEARCH d USING INTEGER PRIMARY KEY (rowid=?)
- USE TEMP B-TREE FOR ORDER BY
- SCAN s
- SEARCH d USING INTEGER PRIMARY KEY (rowid=?)

EXPLAIN (frunner, sequence740648; visibility dışında):

- MATERIALIZE selected
- SCAN search VIRTUAL TABLE INDEX 0:L0
- SEARCH d USING INTEGER PRIMARY KEY (rowid=?)
- USE TEMP B-TREE FOR ORDER BY
- SCAN s
- SEARCH d USING INTEGER PRIMARY KEY (rowid=?)

SQL step+sütun kopyası; VM çalışması, satır bulma/sıralama ve folded LIKE doğrulamasını tek tek ayırmaz. Referansın folded alanı/FTS trigrama yazıda hazırlanır ([kod](../tools/native_sqlite/src/bin/key_pool.rs)); sorgu planı yukarıda ölçüldü. Bu tanıdaki düşük query süresi ölçülmüş gözlemdir. SQLite için FDB counter alanlarının0 olması SQLite içinde hiç kilit/CRC maliyeti olmadığı anlamına gelmez; bu sayaçlar SQLite içi için uygulanmadı. Tarihsel B bloklarının ortam/cache/yerleşim farkı profillenmediğinden o blokların kesin nedeni **bilinmiyor**.

## Gap p95ine denk gelen aynı sequence olayları

|Motor/anahtar|Sequence/tip|Commit|Gap|History|Acquire|İlk sorgu|Release|Delta altı|Query maskesi altı|
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
|fdb/numeric|222857/delete|1.661900|45.798600|0.000700|0.000400|45.792900|0.000300|0.014400|0.026400|
|sqlite/numeric|739652/mtime_delete_insert|21.967400|8.250000|0.003900|0.102300|8.119700|0.019200|0.000000|0.000000|
|sqlite/frunner|744607/mtime_delete_insert|33.483800|8.288800|0.005700|0.089300|8.166300|0.021600|0.000000|0.000000|
|fdb/frunner|228224/delete|3.864800|48.629100|0.002400|0.000700|48.610800|0.000200|0.034500|0.041500|

## P50/p95/max; tüm normal işlem örnekleri

|Motor/anahtar|Metrik|N|p50/p95/max ms|
|---|---|---:|---|
|fdb/numeric|commit|4096|2.509600/4.369200/25.223000|
|fdb/numeric|visibility|4096|28.584300/49.099900/92.736400|
|fdb/numeric|gap|4096|25.868000/45.798600/90.127000|
|fdb/numeric|history|4096|0.000900/0.002200/0.296700|
|fdb/numeric|acquire|4096|0.000400/0.000600/0.002500|
|fdb/numeric|first_query|4096|25.862000/45.792900/90.119000|
|fdb/numeric|release|4096|0.000200/0.000400/0.273400|
|fdb/numeric|collect|4096|0.077700/0.116300/0.805100|
|sqlite/numeric|commit|4096|4.987400/24.881000/372.272000|
|sqlite/numeric|visibility|4096|10.930100/31.895000/377.330400|
|sqlite/numeric|gap|4096|5.915200/8.250000/20.093400|
|sqlite/numeric|history|4096|0.001300/0.004000/0.077300|
|sqlite/numeric|acquire|4096|0.136500/0.201600/2.494100|
|sqlite/numeric|first_query|4096|5.748700/8.051700/17.573100|
|sqlite/numeric|release|4096|0.019100/0.024800/0.877500|
|sqlite/numeric|collect|4096|0.006400/0.008400/0.292100|
|sqlite/frunner|commit|4096|4.896200/24.697000/366.760300|
|sqlite/frunner|visibility|4096|11.008900/31.415800/371.050500|
|sqlite/frunner|gap|4096|5.823100/8.288800/11.846000|
|sqlite/frunner|history|4096|0.001300/0.004100/0.252400|
|sqlite/frunner|acquire|4096|0.138200/0.201500/0.973100|
|sqlite/frunner|first_query|4096|5.656700/8.096400/11.737600|
|sqlite/frunner|release|4096|0.018300/0.024100/0.615900|
|sqlite/frunner|collect|4096|0.006300/0.008300/0.682200|
|fdb/frunner|commit|4096|2.584400/4.288000/28.949100|
|fdb/frunner|visibility|4096|32.844600/52.285400/89.615000|
|fdb/frunner|gap|4096|29.908800/48.629100/85.360500|
|fdb/frunner|history|4096|0.000900/0.002100/0.248000|
|fdb/frunner|acquire|4096|0.000400/0.000600/0.004300|
|fdb/frunner|first_query|4096|29.902500/48.610800/85.351300|
|fdb/frunner|release|4096|0.000200/0.000400/0.007200|
|fdb/frunner|collect|4096|0.079600/0.126900/0.657300|

## B hamlarından gerçek işlem-bazlı gap (profilsiz, tarihsel)

Her işlemde visibility−commit hesaplandı; bağımsız p95lerden çıkarma yapılmadı. Bu bloklarda aşama sayacı yoktu; yeni tanı onların tam nedeni sayılmaz.

|Blok/motor/anahtar|Gap ort/p50/p95/max ms|
|---|---|
|0/fdb/numeric|3.511070/1.088200/14.857700/23.148300|
|0/sqlite/numeric|5.549056/5.768200/7.970200/17.340600|
|0/sqlite/frunner|5.584711/5.774000/8.014800/13.860500|
|0/fdb/frunner|6.328658/2.800100/19.152500/30.011900|
|1/fdb/frunner|8.834226/8.209600/22.323500/40.497800|
|1/sqlite/frunner|3.289020/3.366100/4.724100/10.841500|
|1/sqlite/numeric|3.311104/3.374200/4.724200/11.752800|
|1/fdb/numeric|6.342894/6.063000/14.718900/23.026700|
|2/fdb/numeric|8.010849/7.774300/16.830500/27.315100|
|2/sqlite/numeric|3.374736/3.490100/4.844600/11.139700|
|2/sqlite/frunner|3.313035/3.386300/4.822400/11.540000|
|2/fdb/frunner|10.125856/10.068200/18.707800/30.311600|
|3/fdb/frunner|11.904190/11.491900/20.222700/38.541700|
|3/sqlite/frunner|4.482597/4.620700/6.487700/11.860000|
|3/sqlite/numeric|4.555839/4.737200/6.420100/10.119600|
|3/fdb/numeric|17.540689/17.253800/28.401800/43.440700|

## Doğrulama ve kapanış

256 default +161 diagnostic +3 SQLite test; fmt/default ve diagnostic clippy geçti.9 witnessedkill/reopen ve eski okuyucu oracle geçti. Ana50k GET/41case ve SQLite payload digest korundu. İki pool canlı kaydı0;motor kaynakları accepted-default ile SHA eşit. 16388 sample zaman sınırları monoton ve ayrık toplam denetimi geçti. Güç kesintisi/in-flight commit iddiası yok. Bu tanıdan sonra duruldu.

[Tasarım/sıra/SHA/süreç logları](benchmarks/visibility-phase50-20261102-design.json) · [Özet](benchmarks/visibility-phase50-20261102-summary.json) · [Tüm ölçülen örnekler](benchmarks/visibility-phase50-20261102-samples.tsv) · [JSON örnekler](benchmarks/visibility-phase50-20261102-samples.json) · [Olaylar](benchmarks/visibility-phase50-20261102-lifecycle.jsonl).

100lü,C500drain,2,6sn,WAL handle/File::open,zx,reclaim varsayılanı,C ABI yapılmadı. Geniş tarama/case34,signed-zero,yazma/bakım,disk/sorguRAM açık. [OPEN-WORK](OPEN-WORK.md).
