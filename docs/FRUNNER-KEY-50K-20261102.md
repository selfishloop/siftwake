# B: Frunner anahtarı, ayrı havuz — 2 Kasım 2026

**Kapı: KALDI. Tamamlanan kol 16/16.** Yeni try/seri varsayılan; policy override yok. Bu çalışma motor optimizasyonu veya Frunner entegrasyonu değildir.

Frunner commit kapısı GEÇTİ; görünürlük p95 20.148–26.749ms>sınır20ms: KALDI. Havuz okuma Frunner blokları 3/5, 3/5, 4/5, 3/5. Anahtar sırası/atomik mtime/1000B uç doğruluğu geçti; sonraki işe geçilmedi.

## Kapsam ve yöntem

Ölçümden önce kullanıcıya [plan ve eşikler](FRUNNER-KEY-DESIGN-50K-20261102.md) gösterildi; kullanıcı ayrı collection havuzunu seçti. Aynı881756DA ilk50k içinden256 gerçek donör +1 uzun-yol slotu. Kaynak/private/ değiştirilmedi; private/ okunmadı. Yeni corpus, DB kopyası, tüm50k yeniden ingest veya yeniden anahtarlama yok. Numeric `keybench-numeric`, Frunner `keybench-frunner`; ikisi16UTF8B collection adı. Mevcut50k `texts` korunur. Hazırlık ve cleanup süreleri loglarda ayrı; yalnız bu iki collection anahtarları yazılır. Her kol başında257 kayıt, yeni mtime her zaman başa; güncelleme aynı anahtarda, mtime değişikliği tek committe delete+insert.

ID10B numeric veya `{:016x}|path` ters-u64-mtime: yol40/80/160/300B, anahtar57/97/177/317B; bir1000B yol/1017B ID ucu. UTF8 bayt uzunluklarıdır. Body2–4/4–16/16–64/64–200KB; aynı donörler her kolda. Gövdede oracle etiketi için en fazla199936B kaynak öneki +etiket; disk corpus değişmez.

Her tip×body×path hücresi64 örnek: tip başına1024; kol4096 normal+1 uç commit. P50/p95/p99/max nearest-rank; trim/bootstrap yok. N64te p95=max değildir, p99=max olabilir. Commit zamanı native API içidir; key/document hazırlığı ayrıca. Görünürlük commit başlangıcından yeni snapshotta ilk marker sorgusunun dönüşüne kadar; tam oracle sonra.

Havuz5 kontrol sorgusu: ortak oracle etiketi yalnız sıralama için, olduğu/üreticisi/olduğu birlikte gerçek gövde, negatif qzxkvwkeypoolabsent. 1R idle60,4R15tur/aktör; loaded yalnız writer_active örnekleri, her sorgu≥60. Dört okuyucu aynı beşli sıra ile başlar; eski41-sorgu yükünün zamanlaması kopyalanmadı, kullanıcı seçimiyle küçük havuz kontrolüdür. Marker sorgusundan genel içerik arama üstünlüğü çıkarılmaz. Oracle her snapshot sequence için bağımsız history modelidir; zamanı engine_ms dışında. Engine_ms=acquire+search+release. Dört blok numeric/Frunner ve FDB/SQLite ters sıralı; sıcak/kontrolsüz OS cache, eviction yok. Fiziksel yerleşim/sequence değişir; yalnız anahtar byte sayısına kesin neden atfı yapılmaz. Numeric artan ID sonunda, Frunner ters-mtime başında yeni kayıt verir: ilk50 aynı belgeler olmak zorunda değildir, dönüş payload maliyeti de değişebilir. Bu okuma kontrolü salt anahtar karşılaştırma CPU deneyi değildir. SQLite aynı FTS5 trigram/detailnone/columnsize0, WAL/FULL/cache2MiB/mmap0/tempFILE; yeni bağımsız benchmark adaptörü, engine bağımlılığı eklenmedi.

## Değişmeyen kapılar

FDB kısa commit ve görünürlük: her işlem tipinde p95≤20,p99≤50,max≤100ms; uçN1de max≤100ms. Havuz okuma: kendi idle≤30 ise loaded≤30ms, aksi halde≤2×idle. B kararının esas yolu FDB Frunner; numeric kontrol ve SQLite ayrıca gösterilir. SQLite referanstır, FDB max şartı uygulanmaz. Numeric→Frunner farkına yeni yüzde kapısı konmadı. CPU/model/oracle/history içeren process private/WS motor-only D sayılmaz; bu tur D ingest/RAM kabulü yeniden ölçülmedi. Önceki normal41 KALDI kullanıcı kabul kaydı bu yeni kapıyı otomatik geçirmez. Commit/s hazırlık, görünürlük ve her işlemdeki tam oracle dahil yazıcı döngüsüdür; HYBRID-DEFAULT uzun seri ops/s ile birebir kıyaslanmaz. Yalnız native API commit kuyruk değerleri ayrı sütundadır. Kontrollü yol dizgileri kullanıldı; gerçek filesystem erişimi/Win32 bileşen geçerliliği ölçülmedi.

|Blok|Motor|Anahtar|Yazma kapısı|Okuma geçti/5|Commit/s|Mutation/s|Process peak private MiB|
|---|---|---|---|---:|---:|---:|---:|
|0|fdb|numeric|GEÇTİ|3|120.971|151.235|111.660|
|0|sqlite|numeric|REFERANS|5|61.956|77.456|120.590|
|0|sqlite|frunner|REFERANS|5|59.943|74.939|118.148|
|0|fdb|frunner|KALDI|3|74.085|92.620|121.398|
|1|fdb|frunner|KALDI|3|56.017|70.031|122.184|
|1|sqlite|frunner|REFERANS|5|100.052|125.084|129.160|
|1|sqlite|numeric|REFERANS|5|101.708|127.153|127.020|
|1|fdb|numeric|GEÇTİ|4|79.807|99.774|121.812|
|2|fdb|numeric|GEÇTİ|3|64.241|80.313|125.113|
|2|sqlite|numeric|REFERANS|5|101.788|127.254|119.691|
|2|sqlite|frunner|REFERANS|4|98.696|123.388|129.109|
|2|fdb|frunner|KALDI|4|51.051|63.823|131.438|
|3|fdb|frunner|KALDI|3|43.905|54.889|134.840|
|3|sqlite|frunner|REFERANS|5|74.155|92.708|124.453|
|3|sqlite|numeric|REFERANS|5|74.868|93.599|130.613|
|3|fdb|numeric|KALDI|3|30.219|37.779|133.164|

## Ön sıradaki payload farkı, ölçüm sınırı

Her FDB kolunun son snapshotında marker ilk50 dönen metin byte toplamı; bütün sorguların byte toplamı veya saf CPU profili değildir.

|Blok|Numeric ilk50 text B|Frunner ilk50 text B|
|---|---:|---:|
|0|144412|4247315|
|1|144412|4247315|
|2|144412|4247315|
|3|144412|4247315|

Bu donör/işlem sırası Frunner başında daha büyük belgeler bırakır. Anahtar byte maliyeti ile farklı sonuç/payload maliyeti bu okumada izole edilmedi; bu fark görünürlük/commit kaçırmalarının kanıtlanmış kök nedeni olarak sunulmaz.

## Commit/görünürlük, bütün tipler ve çiftler

|Blok/motor/anahtar|Tip/N|Commit p50/p95/p99/max ms|Görünürlük p50/p95/p99/max ms|Hazırlık p95 ms|FDB kuralı|
|---|---|---|---|---:|---|
|0/fdb/numeric|update/1024|2.619/5.043/14.659/31.546|5.138/18.932/23.608/33.967|0.041|GEÇTİ|
|0/fdb/numeric|delete/1024|1.886/2.823/4.005/18.800|2.555/16.983/21.422/27.689|0.002|GEÇTİ|
|0/fdb/numeric|insert/1024|2.577/4.786/8.689/18.762|3.914/17.737/20.974/24.511|0.050|GEÇTİ|
|0/fdb/numeric|mtime_delete_insert/1024|2.600/4.856/14.214/20.399|4.159/18.876/24.642/36.877|0.019|GEÇTİ|
|0/fdb/numeric|edge1000_mtime/1|—/—/—/2.515|—/—/—/18.392|0.015|GEÇTİ|
|0/sqlite/numeric|update/1024|5.154/24.688/34.134/79.575|11.384/32.061/42.571/83.472|0.074|REFERANS|
|0/sqlite/numeric|delete/1024|4.019/15.953/26.299/361.283|8.161/19.616/28.690/362.791|0.002|REFERANS|
|0/sqlite/numeric|insert/1024|4.130/16.861/23.454/82.782|10.453/23.486/29.968/87.846|0.049|REFERANS|
|0/sqlite/numeric|mtime_delete_insert/1024|7.153/31.140/49.078/362.110|13.063/37.200/56.256/367.636|0.018|REFERANS|
|0/sqlite/numeric|edge1000_mtime/1|—/—/—/2.885|—/—/—/9.099|0.023|REFERANS|
|0/sqlite/frunner|update/1024|5.423/24.991/43.901/370.001|11.459/32.382/50.037/375.267|0.057|REFERANS|
|0/sqlite/frunner|delete/1024|4.039/15.489/22.663/344.385|8.264/19.565/25.249/347.720|0.005|REFERANS|
|0/sqlite/frunner|insert/1024|4.170/17.239/24.849/82.867|10.429/23.956/30.317/87.204|0.049|REFERANS|
|0/sqlite/frunner|mtime_delete_insert/1024|7.149/31.734/43.667/127.533|13.233/38.101/49.667/132.517|0.032|REFERANS|
|0/sqlite/frunner|edge1000_mtime/1|—/—/—/10.029|—/—/—/16.187|0.025|REFERANS|
|0/fdb/frunner|update/1024|2.731/4.781/13.975/21.073|7.857/22.298/26.704/42.523|0.045|KALDI|
|0/fdb/frunner|delete/1024|1.937/2.798/4.148/20.523|3.438/21.336/25.483/32.357|0.003|KALDI|
|0/fdb/frunner|insert/1024|2.626/4.471/6.642/30.577|5.476/22.423/27.418/39.238|0.046|KALDI|
|0/fdb/frunner|mtime_delete_insert/1024|2.683/4.479/9.140/23.642|6.846/22.900/28.317/40.914|0.034|KALDI|
|0/fdb/frunner|edge1000_mtime/1|—/—/—/1.968|—/—/—/20.815|0.020|GEÇTİ|
|1/fdb/frunner|update/1024|2.652/4.939/15.747/21.266|11.717/26.749/31.974/43.738|0.044|KALDI|
|1/fdb/frunner|delete/1024|1.976/2.818/3.434/19.045|9.114/23.613/27.440/36.959|0.003|KALDI|
|1/fdb/frunner|insert/1024|2.592/4.443/7.349/22.334|10.668/24.889/32.554/42.709|0.046|KALDI|
|1/fdb/frunner|mtime_delete_insert/1024|2.667/4.426/10.016/23.482|11.793/25.743/30.518/38.201|0.030|KALDI|
|1/fdb/frunner|edge1000_mtime/1|—/—/—/2.263|—/—/—/18.284|0.020|GEÇTİ|
|1/sqlite/frunner|update/1024|3.198/14.473/21.978/48.277|6.854/19.032/26.560/50.546|0.036|REFERANS|
|1/sqlite/frunner|delete/1024|2.603/9.955/17.347/43.408|5.013/12.628/19.456/46.727|0.003|REFERANS|
|1/sqlite/frunner|insert/1024|2.660/11.115/22.881/73.734|6.253/15.342/26.032/76.682|0.036|REFERANS|
|1/sqlite/frunner|mtime_delete_insert/1024|4.242/19.043/32.296/191.315|7.842/23.165/36.242/193.994|0.017|REFERANS|
|1/sqlite/frunner|edge1000_mtime/1|—/—/—/4.292|—/—/—/7.676|0.021|REFERANS|
|1/sqlite/numeric|update/1024|3.176/14.976/23.836/199.040|6.814/19.451/28.438/201.889|0.039|REFERANS|
|1/sqlite/numeric|delete/1024|2.591/9.735/16.032/80.419|4.997/12.075/18.287/85.633|0.002|REFERANS|
|1/sqlite/numeric|insert/1024|2.605/9.739/16.455/36.341|6.204/13.943/21.562/41.370|0.036|REFERANS|
|1/sqlite/numeric|mtime_delete_insert/1024|4.185/20.012/38.375/200.341|7.834/24.100/45.647/202.992|0.013|REFERANS|
|1/sqlite/numeric|edge1000_mtime/1|—/—/—/2.061|—/—/—/5.236|0.014|REFERANS|
|1/fdb/numeric|update/1024|1.981/3.674/10.883/17.554|8.346/17.586/21.677/27.964|0.031|GEÇTİ|
|1/fdb/numeric|delete/1024|1.517/2.231/2.935/13.514|7.330/15.985/18.859/26.281|0.001|GEÇTİ|
|1/fdb/numeric|insert/1024|1.954/3.489/4.495/14.219|8.029/16.899/21.609/29.230|0.037|GEÇTİ|
|1/fdb/numeric|mtime_delete_insert/1024|1.984/3.341/4.526/15.043|8.502/18.053/21.966/26.180|0.012|GEÇTİ|
|1/fdb/numeric|edge1000_mtime/1|—/—/—/1.581|—/—/—/11.373|0.007|GEÇTİ|
|2/fdb/numeric|update/1024|1.955/3.605/11.836/17.775|10.167/19.621/24.241/28.994|0.029|GEÇTİ|
|2/fdb/numeric|delete/1024|1.517/2.160/3.351/17.134|9.173/18.265/21.195/30.272|0.001|GEÇTİ|
|2/fdb/numeric|insert/1024|1.945/3.253/4.308/14.976|9.558/19.422/23.193/30.500|0.037|GEÇTİ|
|2/fdb/numeric|mtime_delete_insert/1024|1.978/3.422/5.110/17.190|10.694/19.634/23.348/44.505|0.013|GEÇTİ|
|2/fdb/numeric|edge1000_mtime/1|—/—/—/1.877|—/—/—/19.635|0.007|GEÇTİ|
|2/sqlite/numeric|update/1024|3.179/15.274/22.999/68.654|6.938/19.549/26.617/72.058|0.032|REFERANS|
|2/sqlite/numeric|delete/1024|2.640/9.550/14.026/215.673|5.284/12.073/16.880/216.708|0.002|REFERANS|
|2/sqlite/numeric|insert/1024|2.676/9.888/15.522/45.796|6.490/14.105/19.503/50.682|0.037|REFERANS|
|2/sqlite/numeric|mtime_delete_insert/1024|4.357/18.386/25.208/205.775|8.130/22.645/29.258/208.554|0.011|REFERANS|
|2/sqlite/numeric|edge1000_mtime/1|—/—/—/2.104|—/—/—/6.579|0.016|REFERANS|
|2/sqlite/frunner|update/1024|3.211/14.571/21.980/194.491|6.952/18.858/26.050/197.500|0.041|REFERANS|
|2/sqlite/frunner|delete/1024|2.635/11.467/36.875/73.303|5.109/14.140/41.037/74.501|0.003|REFERANS|
|2/sqlite/frunner|insert/1024|2.672/10.198/18.779/65.868|6.385/13.835/24.445/70.316|0.044|REFERANS|
|2/sqlite/frunner|mtime_delete_insert/1024|4.335/18.565/32.100/76.338|8.001/22.626/35.390/79.374|0.017|REFERANS|
|2/sqlite/frunner|edge1000_mtime/1|—/—/—/2.580|—/—/—/6.287|0.020|REFERANS|
|2/fdb/frunner|update/1024|2.055/3.551/11.329/19.130|12.409/21.573/26.362/36.499|0.036|KALDI|
|2/fdb/frunner|delete/1024|1.571/2.253/3.433/31.613|11.668/20.148/24.953/46.923|0.003|KALDI|
|2/fdb/frunner|insert/1024|2.027/3.406/4.745/16.455|11.950/21.257/26.164/32.189|0.036|KALDI|
|2/fdb/frunner|mtime_delete_insert/1024|2.027/3.351/5.202/18.047|12.870/21.981/28.306/36.495|0.019|KALDI|
|2/fdb/frunner|edge1000_mtime/1|—/—/—/2.052|—/—/—/22.280|0.024|GEÇTİ|
|3/fdb/frunner|update/1024|2.038/3.546/12.421/19.038|13.869/22.705/27.578/46.832|0.039|KALDI|
|3/fdb/frunner|delete/1024|1.567/2.220/3.063/19.004|13.218/21.780/26.360/36.575|0.002|KALDI|
|3/fdb/frunner|insert/1024|2.031/3.175/4.288/17.919|13.760/23.072/28.123/33.346|0.040|KALDI|
|3/fdb/frunner|mtime_delete_insert/1024|2.021/3.282/4.745/18.117|13.612/24.017/29.210/40.428|0.022|KALDI|
|3/fdb/frunner|edge1000_mtime/1|—/—/—/1.675|—/—/—/30.269|0.014|GEÇTİ|
|3/sqlite/frunner|update/1024|4.356/20.343/30.666/306.953|9.400/26.192/36.464/311.414|0.045|REFERANS|
|3/sqlite/frunner|delete/1024|3.429/12.250/16.720/71.897|6.779/15.666/19.604/75.525|0.004|REFERANS|
|3/sqlite/frunner|insert/1024|3.640/13.411/19.310/38.930|8.691/19.100/24.309/43.681|0.039|REFERANS|
|3/sqlite/frunner|mtime_delete_insert/1024|6.125/25.384/35.634/294.300|11.060/31.096/40.744/297.983|0.029|REFERANS|
|3/sqlite/frunner|edge1000_mtime/1|—/—/—/2.468|—/—/—/7.857|0.022|REFERANS|
|3/sqlite/numeric|update/1024|4.501/19.447/27.401/74.921|9.535/25.251/33.509/79.527|0.052|REFERANS|
|3/sqlite/numeric|delete/1024|3.523/13.068/20.898/91.201|6.833/16.585/24.375/92.934|0.002|REFERANS|
|3/sqlite/numeric|insert/1024|3.591/13.528/20.425/286.121|8.685/19.383/26.019/289.353|0.042|REFERANS|
|3/sqlite/numeric|mtime_delete_insert/1024|5.828/25.432/35.850/280.500|10.927/30.620/41.464/284.250|0.017|REFERANS|
|3/sqlite/numeric|edge1000_mtime/1|—/—/—/5.957|—/—/—/10.544|0.018|REFERANS|
|3/fdb/numeric|update/1024|2.356/3.960/12.747/19.394|20.021/31.349/37.468/51.968|0.036|KALDI|
|3/fdb/numeric|delete/1024|1.786/2.419/3.659/18.969|18.559/29.738/35.318/48.210|0.002|KALDI|
|3/fdb/numeric|insert/1024|2.351/3.701/4.825/19.063|19.865/31.932/38.502/40.923|0.043|KALDI|
|3/fdb/numeric|mtime_delete_insert/1024|2.383/3.741/5.353/19.919|20.084/31.973/38.014/46.663|0.015|KALDI|
|3/fdb/numeric|edge1000_mtime/1|—/—/—/2.344|—/—/—/28.662|0.011|GEÇTİ|

## Havuz okuma:1R,4R idle ve yüklü

|Blok/motor/anahtar|Sorgu|1R N:p50/p95/max|4R idle N:p50/p95/max|Loaded N:p50/p95/max|Sınır ms|Karar|
|---|---|---|---|---|---:|---|
|0/fdb/numeric|fdbkeypoolmark|60:0.976/1.266/4.061|60:2.513/3.420/5.544|1168:20.730/42.008/48.759|30.000|KALDI|
|0/fdb/numeric|olduğu|60:1.559/1.981/2.122|60:3.115/3.609/3.810|1168:12.953/23.601/33.479|30.000|GEÇTİ|
|0/fdb/numeric|üreticisi|60:24.912/28.335/48.548|60:30.520/32.186/33.395|1164:37.552/43.232/78.304|64.373|GEÇTİ|
|0/fdb/numeric|olduğu birlikte|60:3.552/4.653/5.471|60:5.343/6.150/6.619|1164:20.828/32.294/45.165|30.000|KALDI|
|0/fdb/numeric|qzxkvwkeypoolabsent|60:0.142/0.205/1.626|60:0.148/0.222/0.888|1164:10.325/15.915/25.139|30.000|GEÇTİ|
|0/sqlite/numeric|fdbkeypoolmark|60:28.899/34.358/34.950|60:51.186/72.786/90.179|154:62.572/76.245/83.875|145.572|GEÇTİ|
|0/sqlite/numeric|olduğu|60:357.635/399.110/3882.627|60:681.445/818.716/844.161|154:689.178/730.586/765.171|1637.431|GEÇTİ|
|0/sqlite/numeric|üreticisi|60:233.063/266.093/370.167|60:389.653/411.201/416.168|150:390.114/408.641/439.656|822.401|GEÇTİ|
|0/sqlite/numeric|olduğu birlikte|60:326.068/353.797/363.235|60:581.443/783.212/851.119|150:598.456/644.004/706.894|1566.423|GEÇTİ|
|0/sqlite/numeric|qzxkvwkeypoolabsent|60:0.505/0.689/1.095|60:0.981/1.652/2.313|150:3.788/5.568/7.490|30.000|GEÇTİ|
|0/sqlite/frunner|fdbkeypoolmark|60:29.079/32.135/39.614|60:54.313/61.492/61.917|156:63.310/75.962/79.031|122.983|GEÇTİ|
|0/sqlite/frunner|olduğu|60:361.225/417.528/454.922|60:708.466/770.755/794.592|156:690.033/769.509/797.954|1541.511|GEÇTİ|
|0/sqlite/frunner|üreticisi|60:234.055/272.448/300.277|60:400.494/417.818/427.882|154:396.860/435.780/470.223|835.637|GEÇTİ|
|0/sqlite/frunner|olduğu birlikte|60:329.966/387.280/422.798|60:605.379/662.677/681.111|154:594.521/672.638/720.349|1325.353|GEÇTİ|
|0/sqlite/frunner|qzxkvwkeypoolabsent|60:0.564/0.753/0.809|60:0.934/1.498/1.774|152:3.992/7.663/9.828|30.000|GEÇTİ|
|0/fdb/frunner|fdbkeypoolmark|60:2.361/3.334/5.610|60:5.432/6.562/6.732|1798:21.335/39.261/54.426|30.000|KALDI|
|0/fdb/frunner|olduğu|60:1.664/2.212/2.377|60:3.416/4.103/4.210|1798:13.415/23.540/34.879|30.000|GEÇTİ|
|0/fdb/frunner|üreticisi|60:27.648/33.081/46.079|60:32.550/34.582/35.272|1798:39.764/45.182/63.808|69.164|GEÇTİ|
|0/fdb/frunner|olduğu birlikte|60:5.082/6.240/7.110|60:7.468/8.651/9.054|1794:22.634/32.686/48.442|30.000|KALDI|
|0/fdb/frunner|qzxkvwkeypoolabsent|60:1.821/2.544/2.589|60:2.441/4.044/4.191|1794:13.510/18.643/24.336|30.000|GEÇTİ|
|1/fdb/frunner|fdbkeypoolmark|60:3.054/4.117/4.305|60:6.849/7.457/7.624|2210:23.768/39.719/51.394|30.000|KALDI|
|1/fdb/frunner|olduğu|60:2.542/3.375/17.057|60:5.025/5.706/7.607|2210:14.764/24.545/34.347|30.000|GEÇTİ|
|1/fdb/frunner|üreticisi|60:27.440/31.013/45.932|60:33.870/35.700/36.482|2210:41.421/46.487/62.654|71.400|GEÇTİ|
|1/fdb/frunner|olduğu birlikte|60:5.627/6.886/8.782|60:8.750/10.411/10.755|2210:24.933/34.398/43.740|30.000|KALDI|
|1/fdb/frunner|qzxkvwkeypoolabsent|60:3.573/5.110/7.500|60:5.180/6.554/7.537|2210:15.458/20.922/30.058|30.000|GEÇTİ|
|1/sqlite/frunner|fdbkeypoolmark|60:29.593/37.811/39.908|60:31.802/54.411/56.104|160:39.587/52.120/97.237|108.822|GEÇTİ|
|1/sqlite/frunner|olduğu|60:369.211/427.266/4660.402|60:415.747/672.310/703.164|160:392.987/421.358/443.824|1344.620|GEÇTİ|
|1/sqlite/frunner|üreticisi|60:234.868/282.462/387.194|60:241.873/386.865/387.787|156:231.690/248.201/257.902|773.730|GEÇTİ|
|1/sqlite/frunner|olduğu birlikte|60:335.848/393.950/421.451|60:365.030/564.368/585.505|156:345.086/376.368/380.715|1128.736|GEÇTİ|
|1/sqlite/frunner|qzxkvwkeypoolabsent|60:0.511/0.765/0.995|60:0.640/1.277/1.938|156:6.066/26.467/35.878|30.000|GEÇTİ|
|1/sqlite/numeric|fdbkeypoolmark|60:22.127/23.715/29.409|60:30.517/34.501/36.500|156:38.671/54.275/91.793|69.002|GEÇTİ|
|1/sqlite/numeric|olduğu|60:266.045/281.840/294.726|60:364.360/486.444/500.404|156:395.223/429.955/443.924|972.888|GEÇTİ|
|1/sqlite/numeric|üreticisi|60:173.008/177.780/207.119|60:215.451/260.972/273.370|152:233.542/264.243/292.293|521.943|GEÇTİ|
|1/sqlite/numeric|olduğu birlikte|60:242.628/257.113/307.306|60:316.795/358.414/372.825|152:347.062/376.371/383.038|716.828|GEÇTİ|
|1/sqlite/numeric|qzxkvwkeypoolabsent|60:0.445/0.537/0.871|60:0.725/1.325/1.558|152:6.487/22.074/29.695|30.000|GEÇTİ|
|1/fdb/numeric|fdbkeypoolmark|60:2.739/4.438/8.086|60:4.623/5.634/6.688|2236:17.157/30.400/38.270|30.000|KALDI|
|1/fdb/numeric|olduğu|60:2.394/3.815/6.558|60:3.593/4.303/4.423|2236:10.230/16.839/21.445|30.000|GEÇTİ|
|1/fdb/numeric|üreticisi|60:20.591/25.316/30.425|60:21.570/22.076/22.198|2236:27.337/29.934/32.488|30.000|GEÇTİ|
|1/fdb/numeric|olduğu birlikte|60:4.648/6.526/9.832|60:6.122/6.734/7.185|2236:17.073/23.772/29.156|30.000|GEÇTİ|
|1/fdb/numeric|qzxkvwkeypoolabsent|60:3.184/5.012/5.323|60:4.376/4.897/5.319|2232:10.530/13.656/15.770|30.000|GEÇTİ|
|2/fdb/numeric|fdbkeypoolmark|60:3.221/4.757/6.555|60:5.318/5.796/5.940|2620:18.724/31.434/37.132|30.000|KALDI|
|2/fdb/numeric|olduğu|60:2.841/3.384/7.756|60:4.195/4.438/4.586|2620:10.955/17.373/20.915|30.000|GEÇTİ|
|2/fdb/numeric|üreticisi|60:21.135/22.709/43.418|60:21.924/22.548/22.823|2620:28.100/30.830/34.594|30.000|KALDI|
|2/fdb/numeric|olduğu birlikte|60:5.368/6.281/11.423|60:7.251/8.370/8.513|2620:18.514/25.031/29.607|30.000|GEÇTİ|
|2/fdb/numeric|qzxkvwkeypoolabsent|60:3.750/5.852/20.035|60:5.280/6.028/6.215|2616:11.648/15.014/18.470|30.000|GEÇTİ|
|2/sqlite/numeric|fdbkeypoolmark|60:21.900/22.939/23.767|60:30.076/31.341/31.695|144:40.895/50.113/78.177|62.683|GEÇTİ|
|2/sqlite/numeric|olduğu|60:266.973/271.305/278.094|60:367.259/399.096/408.462|144:446.437/479.396/496.690|798.191|GEÇTİ|
|2/sqlite/numeric|üreticisi|60:173.236/175.926/178.547|60:215.985/226.784/228.445|144:250.747/256.874/277.764|453.568|GEÇTİ|
|2/sqlite/numeric|olduğu birlikte|60:243.041/247.683/252.029|60:330.119/340.913/342.382|143:385.796/405.714/456.814|681.827|GEÇTİ|
|2/sqlite/numeric|qzxkvwkeypoolabsent|60:0.461/0.527/0.652|60:0.660/1.342/1.660|140:2.947/13.155/29.688|30.000|GEÇTİ|
|2/sqlite/frunner|fdbkeypoolmark|60:21.858/22.628/23.778|60:28.836/31.716/32.299|160:39.287/48.097/60.820|63.431|GEÇTİ|
|2/sqlite/frunner|olduğu|60:266.128/272.505/287.023|60:356.997/360.977/365.834|160:398.795/426.883/449.246|721.955|GEÇTİ|
|2/sqlite/frunner|üreticisi|60:172.174/177.513/183.100|60:214.172/218.594/221.984|156:232.932/275.611/289.601|437.187|GEÇTİ|
|2/sqlite/frunner|olduğu birlikte|60:241.897/248.444/266.311|60:315.912/334.731/342.577|156:348.011/362.596/371.186|669.462|GEÇTİ|
|2/sqlite/frunner|qzxkvwkeypoolabsent|60:0.447/0.528/0.796|60:0.614/1.056/1.293|156:6.794/30.969/37.209|30.000|KALDI|
|2/fdb/frunner|fdbkeypoolmark|60:3.698/5.756/8.600|60:5.814/6.156/6.190|3180:18.564/29.539/35.944|30.000|GEÇTİ|
|2/fdb/frunner|olduğu|60:3.096/3.843/4.315|60:4.416/4.799/4.914|3180:11.074/17.255/22.634|30.000|GEÇTİ|
|2/fdb/frunner|üreticisi|60:21.622/23.780/35.147|60:22.641/23.577/24.060|3180:28.793/31.728/37.116|30.000|KALDI|
|2/fdb/frunner|olduğu birlikte|60:6.239/8.423/10.822|60:8.692/10.411/10.531|3176:19.240/24.892/31.884|30.000|GEÇTİ|
|2/fdb/frunner|qzxkvwkeypoolabsent|60:4.350/5.575/21.681|60:6.185/6.969/7.379|3176:12.612/16.415/22.372|30.000|GEÇTİ|
|3/fdb/frunner|fdbkeypoolmark|60:6.069/9.287/15.561|60:9.136/12.683/12.814|3560:19.552/31.197/39.201|30.000|KALDI|
|3/fdb/frunner|olduğu|60:3.576/4.813/7.566|60:5.232/7.807/8.114|3560:11.613/17.978/27.170|30.000|GEÇTİ|
|3/fdb/frunner|üreticisi|60:22.319/26.170/41.606|60:23.984/29.754/32.462|3560:29.314/33.067/47.165|30.000|KALDI|
|3/fdb/frunner|olduğu birlikte|60:7.431/8.552/15.031|60:10.449/17.966/18.401|3556:20.342/26.407/36.581|30.000|GEÇTİ|
|3/fdb/frunner|qzxkvwkeypoolabsent|60:5.260/6.670/26.073|60:7.296/9.524/10.540|3556:13.178/17.645/26.259|30.000|GEÇTİ|
|3/sqlite/frunner|fdbkeypoolmark|60:21.976/22.977/23.519|60:35.888/39.509/40.052|166:48.770/58.618/69.724|79.017|GEÇTİ|
|3/sqlite/frunner|olduğu|60:267.589/288.418/293.800|60:441.338/527.541/537.205|166:502.687/573.679/629.034|1055.081|GEÇTİ|
|3/sqlite/frunner|üreticisi|60:173.525/178.334/195.516|60:272.237/291.086/300.770|166:297.929/331.457/340.609|582.173|GEÇTİ|
|3/sqlite/frunner|olduğu birlikte|60:244.375/250.364/265.916|60:397.035/405.344/406.379|165:449.548/503.644/587.152|810.689|GEÇTİ|
|3/sqlite/frunner|qzxkvwkeypoolabsent|60:0.410/0.479/0.586|60:0.692/1.127/1.248|162:3.182/5.420/8.369|30.000|GEÇTİ|
|3/sqlite/numeric|fdbkeypoolmark|60:22.557/24.486/31.471|60:38.358/44.538/49.058|158:50.790/60.444/67.162|89.075|GEÇTİ|
|3/sqlite/numeric|olduğu|60:281.841/295.604/352.478|60:455.674/544.524/552.732|158:551.923/591.624/610.342|1089.049|GEÇTİ|
|3/sqlite/numeric|üreticisi|60:181.052/193.968/212.884|60:279.097/312.507/312.911|154:315.344/342.677/350.819|625.015|GEÇTİ|
|3/sqlite/numeric|olduğu birlikte|60:256.313/278.178/297.619|60:411.192/467.942/477.024|154:482.379/522.603/539.479|935.884|GEÇTİ|
|3/sqlite/numeric|qzxkvwkeypoolabsent|60:0.493/0.731/0.845|60:0.890/1.478/1.959|154:3.333/5.763/8.679|30.000|GEÇTİ|
|3/fdb/numeric|fdbkeypoolmark|60:6.960/8.919/13.608|60:12.504/14.120/14.687|3984:28.519/44.727/56.267|30.000|KALDI|
|3/fdb/numeric|olduğu|60:3.883/4.535/14.248|60:7.113/8.082/8.358|3984:15.465/23.159/29.774|30.000|GEÇTİ|
|3/fdb/numeric|üreticisi|60:23.272/25.389/29.941|60:31.258/32.653/33.113|3984:37.335/41.692/48.472|65.307|GEÇTİ|
|3/fdb/numeric|olduğu birlikte|60:8.358/11.631/17.462|60:14.061/14.962/15.325|3984:26.470/35.049/44.723|30.000|KALDI|
|3/fdb/numeric|qzxkvwkeypoolabsent|60:6.018/9.573/10.847|60:9.598/10.609/10.882|3984:18.270/23.866/29.340|30.000|GEÇTİ|

## Numeric → Frunner, eşleşmiş yazma farkları

|Blok/motor|Tip|Commit p95 B→F ms|Görünürlük p95 B→F ms|Commit p95 Δ%|
|---|---|---|---|---:|
|0/fdb|update|5.043→4.781|18.932→22.298|-5.19|
|0/fdb|delete|2.823→2.798|16.983→21.336|-0.86|
|0/fdb|insert|4.786→4.471|17.737→22.423|-6.58|
|0/fdb|mtime_delete_insert|4.856→4.479|18.876→22.900|-7.76|
|0/sqlite|update|24.688→24.991|32.061→32.382|+1.23|
|0/sqlite|delete|15.953→15.489|19.616→19.565|-2.91|
|0/sqlite|insert|16.861→17.239|23.486→23.956|+2.24|
|0/sqlite|mtime_delete_insert|31.140→31.734|37.200→38.101|+1.91|
|1/fdb|update|3.674→4.939|17.586→26.749|+34.41|
|1/fdb|delete|2.231→2.818|15.985→23.613|+26.33|
|1/fdb|insert|3.489→4.443|16.899→24.889|+27.34|
|1/fdb|mtime_delete_insert|3.341→4.426|18.053→25.743|+32.47|
|1/sqlite|update|14.976→14.473|19.451→19.032|-3.35|
|1/sqlite|delete|9.735→9.955|12.075→12.628|+2.27|
|1/sqlite|insert|9.739→11.115|13.943→15.342|+14.12|
|1/sqlite|mtime_delete_insert|20.012→19.043|24.100→23.165|-4.84|
|2/fdb|update|3.605→3.551|19.621→21.573|-1.50|
|2/fdb|delete|2.160→2.253|18.265→20.148|+4.32|
|2/fdb|insert|3.253→3.406|19.422→21.257|+4.69|
|2/fdb|mtime_delete_insert|3.422→3.351|19.634→21.981|-2.07|
|2/sqlite|update|15.274→14.571|19.549→18.858|-4.60|
|2/sqlite|delete|9.550→11.467|12.073→14.140|+20.08|
|2/sqlite|insert|9.888→10.198|14.105→13.835|+3.14|
|2/sqlite|mtime_delete_insert|18.386→18.565|22.645→22.626|+0.97|
|3/fdb|update|3.960→3.546|31.349→22.705|-10.44|
|3/fdb|delete|2.419→2.220|29.738→21.780|-8.21|
|3/fdb|insert|3.701→3.175|31.932→23.072|-14.19|
|3/fdb|mtime_delete_insert|3.741→3.282|31.973→24.017|-12.27|
|3/sqlite|update|19.447→20.343|25.251→26.192|+4.61|
|3/sqlite|delete|13.068→12.250|16.585→15.666|-6.26|
|3/sqlite|insert|13.528→13.411|19.383→19.100|-0.87|
|3/sqlite|mtime_delete_insert|25.432→25.384|30.620→31.096|-0.19|

## Kanıt ve sınırlar

Her değişiklikte eski marker kaybolması, yeni marker/GET ve version; snapshot tek sequence; yeni insert/mtime first-key; uzun-yol kısa eski snapshot; bütün ilk50 key sırası oracle ile sınandı. Frozen eski okuyucu her FDB havuzunu tam257GET ve ilk50 ile okur. Kill tanıkları pool full oracle sonrası; in-flight transaction/güç kesintisi iddiası yok. Tam test paketi ayrı kesilme/bozulma/kill senaryolarını da içerir.

[Ham kol/süreç/varsayılan/SHA](benchmarks/frunner-key50-20261102-design.json) · [Özet](benchmarks/frunner-key50-20261102-summary.json) · [Body×path hücrelerinin hepsi](benchmarks/frunner-key50-20261102-cells.tsv) · [Okuma örnekleri](benchmarks/frunner-key50-20261102-read-samples.json) · [Süreç olayları](benchmarks/frunner-key50-20261102-lifecycle.jsonl).

100lü,C500drain,2,6sn,WAL handle/File::open,zx iyileştirmesi,format/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k yok. Reclaim default kapalı; geniş tarama, signed-zero, yazma/bakım/disk/sorguRAM açık. [OPEN-WORK](OPEN-WORK.md).

## Havuz temizliği ve kalan alan

İki havuzun canlı kaydı0; ana koleksiyon50000. FDB canlı-run kaynak bytes (WAL dahil) 186768163→255349925; .idx bytes 140057305→167065177. Tombstone/ölü revizyon kalabilir: logical delete, başlangıç fiziksel alanına dönüldü iddiası değildir. Reclaim default kapalı kaldı. Başlangıç toplam retained disk sayılmadığı için toplam retained büyüme oranı verilmez. Son retained FDB422418358B; SQLite477908992B. Bu B turunda yeni disk kapısı eklenmedi.

Doğrulama GEÇTİ: 256 default +161 diagnostic test, SQLite adaptörü3 test; fmt/root ve diagnostic clippy -D warnings. Native en son ölçüm etiketleri eklenince ilgili2 test/clippy ve iki release ikilisi tekrar doğrulandı. Kaynak/0.1fixture SHA aynı, engine accepted-default ile bit aynı. Ana50k FDB tamGET/41oracle ve SQLite payload digest korundu.9 witnessedkill/reopen,8 FDB pool kolunda frozen eski okuyucu257GET+ilk50 sırası geçti. Normal35 child exit0. CPU/model/oracle hariç bir motor RAM sonucu çıkarılmaz.

İki hazırlık hatası ölçümden önce yakalandı: en uzun donör ilk63 içinde olunca seçicide duplicate; ardından model yolu str/Path uyuşmazlığı. İkisi de DB worker açılmadan durdu; düzeltildi, eski loglar silinmedi. Bu kayıtlar tamamlanmış ölçüm veya engine oracle hatası olarak sayılmadı.

[İlk hazırlık logu](benchmarks/frunner-key50-20261102-driver.log) · [İkinci hazırlık logu](benchmarks/frunner-key50-20261102-driver-v2.log) · [Asıl ölçüm](benchmarks/frunner-key50-20261102-driver-v3.log) · [Doğrulama/oracle/CRC/kill/eski okuyucu ve ana50k koruma denetimi](benchmarks/frunner-key50-20261102-audit.json).

**Sonuç KALDI.** Performans kapısı KALDI; sonraki işe geçilmedi.
