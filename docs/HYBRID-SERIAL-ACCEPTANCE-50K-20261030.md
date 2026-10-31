# Hibrit seri devam önce/sonra kabul tablosu —30 Ekim2026

Tek frozen FDB ikilisinde before=blocking,after=non-blocking try+aynı seri frontier. Dört matched çiftte before/SQLite/after ve ters sıra eşit. Her FDB rolü4 kez;SQLite ortak4 kez. Ek üç16k okuyucusuz kontrol throughput paydası. Aynı sabitDBler reset edilmeden,aynı50k/881756DA…corpus,41 sorgu,1R12round/4R3round ve16384tekli writer+oracle. Fiziksel yerleşim/sequence yazı/bakımla ilerler;bit-aynı yerleşim değildir. Idle N12 olduğundan p95=max. Derleme/test bu kollarla paralel çalışmadı;warm OS cache,cold eviction yok. No new thresholds.

## Her çiftin sorgu kapısı

|Çift|Rol|Geçti /kaldı|Kalan case ID|
|---|---|---|---|
|0|before|38 /3|[20, 26, 28]|
|0|after|34 /7|[6, 10, 13, 22, 25, 29, 38]|
|0|sqlite|40 /1|[31]|
|1|before|37 /4|[11, 21, 25, 28]|
|1|after|36 /5|[6, 10, 22, 25, 38]|
|1|sqlite|40 /1|[29]|
|2|before|38 /3|[11, 15, 21]|
|2|after|38 /3|[18, 25, 26]|
|2|sqlite|38 /3|[20, 25, 40]|
|3|before|40 /1|[33]|
|3|after|36 /5|[15, 16, 18, 25, 26]|
|3|sqlite|39 /2|[29, 31]|

## Yazma throughput (pooled)

|Rol|Kontrol ops/s|4R ops/s|Korunum|
|---|---:|---:|---:|
|before|345.018|331.779|96.163%|
|after|345.920|297.286|85.940%|
|sqlite|257.034|212.668|82.739%|

## Yazma kuyrukları

|Çift/rol|p95 /p99 /p99,9 ms|Max ms|>100ms/N|Benchmark peak private MiB|
|---|---|---:|---|---:|
|-1/before|4.134/14.168/26.682|222.418|2/16384|76.844|
|-1/sqlite|8.040/22.242/195.063|586.456|21/16384|42.062|
|-1/after|3.988/14.428/25.199|172.865|2/16384|76.699|
|0/before|5.426/16.910/26.251|253.040|3/16384|146.734|
|0/sqlite|11.243/22.786/224.013|874.942|23/16384|92.828|
|0/after|5.582/16.749/30.576|266.877|4/16384|164.195|
|1/after|5.413/17.226/34.222|326.974|2/16384|163.676|
|1/sqlite|11.079/22.192/225.307|950.129|22/16384|98.340|
|1/before|3.688/9.976/19.822|277.755|3/16384|156.730|
|2/before|3.718/10.079/20.412|182.812|3/16384|156.004|
|2/sqlite|9.078/18.040/194.053|682.676|20/16384|99.551|
|2/after|4.089/10.812/21.325|166.663|2/16384|169.398|
|3/after|4.237/12.380/22.033|212.940|2/16384|166.973|
|3/sqlite|8.914/17.303/185.164|746.161|20/16384|100.812|
|3/before|4.131/10.769/21.796|143.942|1/16384|149.504|

## Pooled kuyruk ve SQLite oranları

Her oran aynı fazdaki SQLite yüzdeliğine bölünür; 1 altı FDB gecikmesinin daha düşük olduğunu gösterir. Loaded her rol için 65.536, control 16.384 commit. Örnekleri birleştirip yeniden yüzdelik hesaplandı; yüzdeliklerin ortalaması alınmadı.

|Faz/rol|p95 / p99 / p99,9 / max ms|SQLite oranı p95 / p99 / p99,9 / max|>100 ms / N|
|---|---|---|---|
|control/before|4.134/14.168/26.682/222.418|0.514/0.637/0.137/0.379|2/16384|
|control/sqlite|8.040/22.242/195.063/586.456|1.000/1.000/1.000/1.000|21/16384|
|control/after|3.988/14.428/25.199/172.865|0.496/0.649/0.129/0.295|2/16384|
|loaded/before|4.213/12.303/22.921/277.755|0.418/0.599/0.118/0.292|10/65536|
|loaded/sqlite|10.087/20.538/194.053/950.129|1.000/1.000/1.000/1.000|85/65536|
|loaded/after|4.741/14.873/27.978/326.974|0.470/0.724/0.144/0.344|10/65536|

## Idle yayılımı: eski rapor ve bu tur yan yana

|Case|Önceki rapor min/medyan/max ms|Bu tur before|Bu tur after|
|---|---|---|---|
|13:akarsu|86.779/120.180/382.041|106.123/170.541/389.215|17.563/19.965/22.652|
|16:üreticisi|50.693/227.175/397.118|105.801/180.062/650.603|35.023/54.791/75.380|
|25:timmy|110.581/231.064/435.112|64.174/142.316/257.281|18.844/21.740/24.112|

## Üreticisi/timmy/akarsu ham satırları

|Çift/rol|Case|1R idle p95|4R idle p95|4R loaded p95|Sınır|Karar|
|---|---|---:|---:|---:|---:|---|
|0/before|akarsu|18.043|163.307|49.648|326.614|GEÇTİ|
|0/before|üreticisi|35.867|650.603|266.762|1301.205|GEÇTİ|
|0/before|timmy|36.185|94.584|178.703|189.167|GEÇTİ|
|0/sqlite|akarsu|169.927|238.594|300.162|477.188|GEÇTİ|
|0/sqlite|üreticisi|249.858|354.874|509.518|709.748|GEÇTİ|
|0/sqlite|timmy|27.180|36.287|58.969|72.575|GEÇTİ|
|0/after|akarsu|19.208|22.652|47.562|30.000|KALDI|
|0/after|üreticisi|38.974|75.380|75.802|150.759|GEÇTİ|
|0/after|timmy|21.148|23.868|44.716|30.000|KALDI|
|1/after|akarsu|19.978|21.290|26.323|30.000|GEÇTİ|
|1/after|üreticisi|41.339|50.251|66.849|100.502|GEÇTİ|
|1/after|timmy|19.853|24.112|44.386|30.000|KALDI|
|1/sqlite|akarsu|179.019|227.212|306.729|454.425|GEÇTİ|
|1/sqlite|üreticisi|279.693|352.753|473.210|705.506|GEÇTİ|
|1/sqlite|timmy|24.460|31.516|41.083|63.032|GEÇTİ|
|1/before|akarsu|11.630|106.123|93.714|212.245|GEÇTİ|
|1/before|üreticisi|24.049|151.049|190.136|302.098|GEÇTİ|
|1/before|timmy|13.137|64.174|141.847|128.347|KALDI|
|2/before|akarsu|11.765|389.215|36.316|778.430|GEÇTİ|
|2/before|üreticisi|24.038|105.801|185.135|211.602|GEÇTİ|
|2/before|timmy|12.608|257.281|152.660|514.562|GEÇTİ|
|2/sqlite|akarsu|141.167|183.241|272.474|366.483|GEÇTİ|
|2/sqlite|üreticisi|200.945|495.126|414.832|990.252|GEÇTİ|
|2/sqlite|timmy|16.111|29.580|45.733|30.000|KALDI|
|2/after|akarsu|24.021|17.563|23.618|30.000|GEÇTİ|
|2/after|üreticisi|29.364|59.332|73.151|118.663|GEÇTİ|
|2/after|timmy|15.408|18.844|36.296|30.000|KALDI|
|3/after|akarsu|14.904|18.640|24.110|30.000|GEÇTİ|
|3/after|üreticisi|30.604|35.023|72.891|70.046|KALDI|
|3/after|timmy|15.560|19.613|34.051|30.000|KALDI|
|3/sqlite|akarsu|139.424|191.370|296.885|382.741|GEÇTİ|
|3/sqlite|üreticisi|209.728|312.514|379.563|625.027|GEÇTİ|
|3/sqlite|timmy|19.908|40.274|47.091|80.547|GEÇTİ|
|3/before|akarsu|15.104|177.775|143.606|355.550|GEÇTİ|
|3/before|üreticisi|29.572|209.076|158.488|418.151|GEÇTİ|
|3/before|timmy|15.707|190.048|149.940|380.096|GEÇTİ|

Bütün41 sorgu,35–38ve case34 dahil1R/idle/loadedbeforeafter [TSV](benchmarks/hybrid-serial50-20261030-cases.tsv);herörnek [JSON](benchmarks/hybrid-serial50-20261030-summary.json). [Frozen SHA/ham log/exit](benchmarks/hybrid-serial50-20261030-design.json). Benchmark process RAM model/oracle/history/records içerir ve query hızına göre kayıt sayısı değişebilir; motorun seri devam ek maliyeti diye çıkarılmaz. Bunun için ayrı model/profil hariç4R engineRAM kolu vardır. CPU parent+child blokları ve loaded requestCPU ayrıdır. the zx user kararıyla ana41/kapı dışında ayrı metadata oracle/tarama/CPUda. Genel kapı kararı bütün bu ölçümler sonrası verilir;kötü veri atılmadı.
