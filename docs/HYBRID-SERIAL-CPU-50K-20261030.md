# Seri fallback CPU ve ek tarama —30 Ekim2026

Ana kabul41-case düzeni değiştirilmedi;bu ölçümler ayrıca. the zx yalnız metadata oracle ile42.ekcase,kapı dışında;eski ishmael dışarıda. Auxsource corpus CRC ile ilk50k okunup sadece query bit41 üretildi;corpus dosyası değiştirilmedi. Same live source/WAL/manifest SHA read-onlyaux+CPU boyunca eşit. 1R/4R aux ABBA,CPU ABBA. Program SHA/loglar tasarımda.

## Parent+child process CPU blokları

Her query için5blok;fast200/slow5 query/blok. GetProcessTimes parent+temporaryqueryworkers toplamı,oracle veGET doğrulaması blok dışında;helper sampler CPU dahil. Sonuç/droplama/loop overhead dahil. CPU ms/query=ΣblokCPU/Σquery;tekqueryCPU yüzdeliği değildir. Yeni eşik/tolerans eklenmedi;bütün sayısal artışlar da görünür.

|Case/sorgu|Before CPU ms/query|After|Fark ms|
|---|---:|---:|---:|
|0:olduğu|3.765625|3.796875|+0.031250|
|1:büyük|3.906250|3.898438|-0.007812|
|2:ayrıca|4.179688|4.070312|-0.109375|
|3:arasında|4.523438|4.515625|-0.007812|
|4:birlikte|4.429688|4.328125|-0.101562|
|5:sanat|5.367188|5.437500|+0.070312|
|6:görenler|9.492188|9.671875|+0.179688|
|7:millî|3.367188|3.164062|-0.203125|
|8:ikinci|5.085938|5.031250|-0.054688|
|9:toplam|5.078125|5.140625|+0.062500|
|10:isimli|8.421875|8.554688|+0.132812|
|11:öldürür|27.187500|28.750000|+1.562500|
|12:ölümü|5.000000|5.000000|+0.000000|
|13:akarsu|9.687500|9.687500|+0.000000|
|14:demokratik|3.125000|3.125000|+0.000000|
|15:çalışmalarıyla|150.000000|150.312500|+0.312500|
|16:üreticisi|43.125000|42.500000|-0.625000|
|17:joseph|1.250000|1.875000|+0.625000|
|18:leyla|79.687500|77.187500|-2.500000|
|19:protest|5.625000|5.937500|+0.312500|
|20:furlong|6.250000|5.937500|-0.312500|
|21:korkaklığa|25.312500|25.312500|+0.000000|
|22:ormanların|248.437500|242.812500|-5.625000|
|23:yanıklarda|243.750000|235.937500|-7.812500|
|24:külliyatı|193.750000|194.687500|+0.937500|
|25:timmy|12.812500|12.812500|+0.000000|
|26:yüklük|23.750000|20.625000|-3.125000|
|27:ilçesine köyleri|5.312500|5.625000|+0.312500|
|28:geldiği buldu|49.687500|50.937500|+1.250000|
|29:chicago studio|5.625000|5.937500|+0.312500|
|30:kısım dişli|257.812500|268.125000|+10.312500|
|31:güzeli zafer|28.125000|29.687500|+1.562500|
|32:İnternetten hazinem|77.187500|84.062500|+6.875000|
|33:oyuncudur atlanta|57.812500|60.312500|+2.500000|
|34:kendisine çıkacağını|181.250000|184.375000|+3.125000|
|35:ve|1.195312|1.234375|+0.039062|
|36:if|1.968750|1.929688|-0.039062|
|37:id|1.460938|1.515625|+0.054688|
|38:zx|293.437500|285.625000|-7.812500|
|39:qzxkvwnever|0.515625|0.500000|-0.015625|
|40:transubstantiationz|1.234375|1.257812|+0.023438|
|41:the zx|100.625000|98.125000|-2.500000|

## Negatif/kısa ek taramalar:1R ve4R p95

|Okuyucu|Case|Before p95/N|After p95/N|
|---|---|---|---|
|1R|kendisine çıkacağını|90.381/20|89.867/20|
|1R|ve|1.339/20|1.086/20|
|1R|if|2.018/20|1.876/20|
|1R|id|1.856/20|1.342/20|
|1R|zx|115.486/20|109.586/20|
|1R|the zx|42.829/20|40.168/20|
|4R|kendisine çıkacağını|731.749/80|234.851/80|
|4R|ve|2.580/80|2.501/80|
|4R|if|4.208/80|3.701/80|
|4R|id|3.158/80|2.703/80|
|4R|zx|894.895/80|994.314/80|
|4R|the zx|356.235/80|258.403/80|

## Loaded request CPU,1R davranış ve tektry kanıtı

Diagnostic ikili ayrı;profil overheadi nedeniyle kabul p95 yerine kullanılamaz. GetThreadTimes parent+child nicemlenebilir;processCPUblokları bunu tamamlar. Her fallback requestte try≤1,backgroundworker0,grant0:doğrulandı.1Rbefore/after bütün41 case fallback0:doğrulandı.

|Faz/case|Role|N|Wall p95|Mean CPU|CPU p95|Fallback adedi|Max request thread|Condvar p95|
|---|---|---:|---:|---:|---:|---:|---:|---:|
|snapshot_idle_single/0|before|24|3.328|0.000|0.000|0|1|0.000|
|snapshot_idle_single/0|after|24|3.218|2.604|15.625|0|1|0.000|
|snapshot_idle_single/1|before|24|2.648|1.302|15.625|0|1|0.000|
|snapshot_idle_single/1|after|24|3.019|1.302|15.625|0|1|0.000|
|snapshot_idle_single/2|before|24|3.056|1.302|15.625|0|1|0.000|
|snapshot_idle_single/2|after|24|3.224|1.953|15.625|0|1|0.000|
|snapshot_idle_single/3|before|24|4.101|5.208|15.625|0|1|0.000|
|snapshot_idle_single/3|after|24|3.877|3.255|15.625|0|1|0.000|
|snapshot_idle_single/4|before|24|3.323|1.953|15.625|0|1|0.000|
|snapshot_idle_single/4|after|24|3.707|3.255|15.625|0|1|0.000|
|snapshot_idle_single/5|before|24|4.143|3.255|15.625|0|1|0.000|
|snapshot_idle_single/5|after|24|3.934|1.953|15.625|0|1|0.000|
|snapshot_idle_single/6|before|24|8.498|4.557|15.625|0|1|0.000|
|snapshot_idle_single/6|after|24|8.506|7.161|15.625|0|1|0.000|
|snapshot_idle_single/7|before|24|2.572|1.953|15.625|0|1|0.000|
|snapshot_idle_single/7|after|24|2.716|2.604|15.625|0|1|0.000|
|snapshot_idle_single/8|before|24|3.115|4.557|15.625|0|1|0.000|
|snapshot_idle_single/8|after|24|3.246|2.604|15.625|0|1|0.000|
|snapshot_idle_single/9|before|24|3.152|3.255|15.625|0|1|0.000|
|snapshot_idle_single/9|after|24|3.358|0.000|0.000|0|1|0.000|
|snapshot_idle_single/10|before|24|7.421|7.812|15.625|0|1|0.000|
|snapshot_idle_single/10|after|24|7.585|5.859|15.625|0|1|0.000|
|snapshot_idle_single/11|before|24|24.564|36.458|62.500|0|4|0.000|
|snapshot_idle_single/11|after|24|24.797|42.318|62.500|0|4|0.000|
|snapshot_idle_single/12|before|24|4.081|3.255|15.625|0|1|0.000|
|snapshot_idle_single/12|after|24|4.354|2.604|15.625|0|1|0.000|
|snapshot_idle_single/13|before|24|15.780|12.370|15.625|0|3|0.000|
|snapshot_idle_single/13|after|24|16.088|14.974|15.625|0|3|0.000|
|snapshot_idle_single/14|before|24|5.634|4.557|15.625|0|1|0.000|
|snapshot_idle_single/14|after|24|5.249|4.557|15.625|0|1|0.000|
|snapshot_idle_single/15|before|24|70.605|152.995|187.500|0|4|0.000|
|snapshot_idle_single/15|after|24|69.798|160.156|187.500|0|4|0.000|
|snapshot_idle_single/16|before|24|31.366|44.922|62.500|0|4|0.000|
|snapshot_idle_single/16|after|24|30.930|47.526|62.500|0|4|0.000|
|snapshot_idle_single/17|before|24|3.275|3.255|15.625|0|1|0.000|
|snapshot_idle_single/17|after|24|3.017|1.953|15.625|0|1|0.000|
|snapshot_idle_single/18|before|24|42.859|80.078|109.375|0|4|0.000|
|snapshot_idle_single/18|after|24|42.513|80.729|93.750|0|4|0.000|
|snapshot_idle_single/19|before|24|7.300|7.812|15.625|0|1|0.000|
|snapshot_idle_single/19|after|24|7.626|5.208|15.625|0|1|0.000|
|snapshot_idle_single/20|before|24|8.845|3.906|15.625|0|1|0.000|
|snapshot_idle_single/20|after|24|8.587|5.859|15.625|0|1|0.000|
|snapshot_idle_single/21|before|24|31.723|26.693|31.250|0|1|0.000|
|snapshot_idle_single/21|after|24|31.176|27.344|31.250|0|1|0.000|
|snapshot_idle_single/22|before|24|107.663|254.557|281.250|0|4|0.000|
|snapshot_idle_single/22|after|24|106.312|245.443|265.625|0|4|0.000|
|snapshot_idle_single/23|before|24|109.895|248.698|296.875|0|4|0.000|
|snapshot_idle_single/23|after|24|111.191|260.417|296.875|0|4|0.000|
|snapshot_idle_single/24|before|24|98.180|201.823|234.375|0|3|0.000|
|snapshot_idle_single/24|after|24|98.824|197.266|218.750|0|3|0.000|
|snapshot_idle_single/25|before|24|16.872|15.625|31.250|0|1|0.000|
|snapshot_idle_single/25|after|24|15.904|12.370|15.625|0|1|0.000|
|snapshot_idle_single/26|before|24|21.379|25.391|46.875|0|4|0.000|
|snapshot_idle_single/26|after|24|20.976|29.297|46.875|0|4|0.000|
|snapshot_idle_single/27|before|24|6.777|5.859|15.625|0|1|0.000|
|snapshot_idle_single/27|after|24|6.876|5.859|15.625|0|1|0.000|
|snapshot_idle_single/28|before|24|37.561|55.339|78.125|0|4|0.000|
|snapshot_idle_single/28|after|24|33.651|54.688|78.125|0|4|0.000|
|snapshot_idle_single/29|before|24|8.854|10.417|15.625|0|1|0.000|
|snapshot_idle_single/29|after|24|9.121|7.161|15.625|0|1|0.000|
|snapshot_idle_single/30|before|24|129.948|262.370|296.875|0|4|0.000|
|snapshot_idle_single/30|after|24|122.780|272.786|312.500|0|4|0.000|
|snapshot_idle_single/31|before|24|35.752|29.948|46.875|0|1|0.000|
|snapshot_idle_single/31|after|24|35.527|27.995|31.250|0|1|0.000|
|snapshot_idle_single/32|before|24|66.465|83.333|93.750|0|2|0.000|
|snapshot_idle_single/32|after|24|66.231|82.031|109.375|0|2|0.000|
|snapshot_idle_single/33|before|24|55.425|59.896|78.125|0|2|0.000|
|snapshot_idle_single/33|after|24|51.860|59.896|78.125|0|2|0.000|
|snapshot_idle_single/34|before|24|96.047|184.245|203.125|0|3|0.000|
|snapshot_idle_single/34|after|24|97.152|188.151|203.125|0|3|0.000|
|snapshot_idle_single/35|before|24|1.426|0.000|0.000|0|1|0.000|
|snapshot_idle_single/35|after|24|1.486|0.651|0.000|0|1|0.000|
|snapshot_idle_single/36|before|24|2.490|0.651|0.000|0|1|0.000|
|snapshot_idle_single/36|after|24|2.534|1.953|15.625|0|1|0.000|
|snapshot_idle_single/37|before|24|1.763|1.302|15.625|0|1|0.000|
|snapshot_idle_single/37|after|24|1.774|1.953|15.625|0|1|0.000|
|snapshot_idle_single/38|before|24|144.201|358.724|406.250|0|4|0.000|
|snapshot_idle_single/38|after|24|143.028|368.490|406.250|0|4|0.000|
|snapshot_idle_single/39|before|24|0.971|0.000|0.000|0|1|0.000|
|snapshot_idle_single/39|after|24|0.836|0.000|0.000|0|1|0.000|
|snapshot_idle_single/40|before|24|3.346|2.604|15.625|0|1|0.000|
|snapshot_idle_single/40|after|24|3.101|3.255|15.625|0|1|0.000|
|snapshot_idle_four/0|before|24|4.983|3.255|15.625|0|1|0.000|
|snapshot_idle_four/0|after|24|4.646|2.604|15.625|0|1|0.000|
|snapshot_idle_four/1|before|24|3.395|3.906|15.625|0|1|0.000|
|snapshot_idle_four/1|after|24|3.262|3.906|15.625|0|1|0.000|
|snapshot_idle_four/2|before|24|4.020|5.208|15.625|0|1|0.000|
|snapshot_idle_four/2|after|24|4.096|3.906|15.625|0|1|0.000|
|snapshot_idle_four/3|before|24|5.235|3.906|15.625|0|1|0.000|
|snapshot_idle_four/3|after|24|5.296|6.510|15.625|0|1|0.000|
|snapshot_idle_four/4|before|24|4.382|2.604|15.625|0|1|0.000|
|snapshot_idle_four/4|after|24|4.185|2.604|15.625|0|1|0.000|
|snapshot_idle_four/5|before|24|4.365|3.906|15.625|0|1|0.000|
|snapshot_idle_four/5|after|24|4.534|5.208|15.625|0|1|0.000|
|snapshot_idle_four/6|before|24|9.689|9.115|15.625|0|1|0.000|
|snapshot_idle_four/6|after|24|9.390|8.464|15.625|0|1|0.000|
|snapshot_idle_four/7|before|24|3.602|3.255|15.625|0|1|0.000|
|snapshot_idle_four/7|after|24|3.727|3.906|15.625|0|1|0.000|
|snapshot_idle_four/8|before|24|3.958|3.255|15.625|0|1|0.000|
|snapshot_idle_four/8|after|24|3.948|2.604|15.625|0|1|0.000|
|snapshot_idle_four/9|before|24|5.098|2.604|15.625|0|1|0.000|
|snapshot_idle_four/9|after|24|4.910|2.604|15.625|0|1|0.000|
|snapshot_idle_four/10|before|24|9.288|9.766|15.625|0|1|0.000|
|snapshot_idle_four/10|after|24|8.906|7.812|15.625|0|1|0.000|
|snapshot_idle_four/11|before|24|138.647|38.411|62.500|0|4|104.811|
|snapshot_idle_four/11|after|24|32.678|35.807|62.500|9|4|0.000|
|snapshot_idle_four/12|before|24|5.395|5.208|15.625|0|1|0.000|
|snapshot_idle_four/12|after|24|4.931|5.208|15.625|0|1|0.000|
|snapshot_idle_four/13|before|24|105.601|14.323|15.625|0|3|89.377|
|snapshot_idle_four/13|after|24|16.960|15.625|15.625|13|3|0.000|
|snapshot_idle_four/14|before|24|6.464|11.068|15.625|0|1|0.000|
|snapshot_idle_four/14|after|24|6.149|5.859|15.625|0|1|0.000|
|snapshot_idle_four/15|before|24|377.912|171.224|203.125|0|4|202.770|
|snapshot_idle_four/15|after|24|117.098|85.286|203.125|18|4|0.000|
|snapshot_idle_four/16|before|24|111.123|52.083|62.500|0|4|79.126|
|snapshot_idle_four/16|after|24|51.752|39.062|62.500|17|4|0.000|
|snapshot_idle_four/17|before|24|3.495|3.906|15.625|0|1|0.000|
|snapshot_idle_four/17|after|24|4.383|3.255|15.625|0|1|0.000|
|snapshot_idle_four/18|before|24|269.493|90.495|109.375|0|4|208.686|
|snapshot_idle_four/18|after|24|62.114|62.500|93.750|14|4|0.000|
|snapshot_idle_four/19|before|24|8.333|6.510|15.625|0|1|0.000|
|snapshot_idle_four/19|after|24|8.650|9.115|15.625|0|1|0.000|
|snapshot_idle_four/20|before|24|9.044|10.417|15.625|0|1|0.000|
|snapshot_idle_four/20|after|24|9.614|7.161|15.625|0|1|0.000|
|snapshot_idle_four/21|before|24|34.733|29.948|46.875|0|1|0.000|
|snapshot_idle_four/21|after|24|34.530|29.948|31.250|0|1|0.000|
|snapshot_idle_four/22|before|24|464.923|276.693|312.500|0|4|151.612|
|snapshot_idle_four/22|after|24|286.437|170.573|296.875|15|4|0.000|
|snapshot_idle_four/23|before|24|411.399|271.484|296.875|0|4|295.101|
|snapshot_idle_four/23|after|24|327.295|286.458|312.500|11|4|0.000|
|snapshot_idle_four/24|before|24|248.336|220.052|250.000|0|3|103.851|
|snapshot_idle_four/24|after|24|250.931|225.911|250.000|8|3|0.000|
|snapshot_idle_four/25|before|24|96.839|17.578|31.250|0|1|78.700|
|snapshot_idle_four/25|after|24|19.707|17.578|31.250|13|1|0.000|
|snapshot_idle_four/26|before|24|122.584|37.760|46.875|0|4|99.873|
|snapshot_idle_four/26|after|24|34.316|28.646|46.875|9|4|0.000|
|snapshot_idle_four/27|before|24|8.160|5.859|15.625|0|1|0.000|
|snapshot_idle_four/27|after|24|8.568|7.812|15.625|0|1|0.000|
|snapshot_idle_four/28|before|24|90.784|63.151|93.750|0|4|40.032|
|snapshot_idle_four/28|after|24|39.472|44.922|78.125|13|4|0.000|
|snapshot_idle_four/29|before|24|9.883|7.161|15.625|0|1|0.000|
|snapshot_idle_four/29|after|24|9.730|10.417|15.625|0|1|0.000|
|snapshot_idle_four/30|before|24|315.400|281.250|312.500|0|4|173.451|
|snapshot_idle_four/30|after|24|311.309|229.167|328.125|10|4|0.000|
|snapshot_idle_four/31|before|24|191.230|34.505|46.875|0|1|155.821|
|snapshot_idle_four/31|after|24|39.067|35.156|46.875|11|1|0.000|
|snapshot_idle_four/32|before|24|204.235|93.099|109.375|0|2|102.363|
|snapshot_idle_four/32|after|24|102.590|95.052|109.375|14|2|0.000|
|snapshot_idle_four/33|before|24|163.603|68.359|78.125|0|2|107.396|
|snapshot_idle_four/33|after|24|76.206|68.359|78.125|10|2|0.000|
|snapshot_idle_four/34|before|24|312.229|199.219|218.750|0|3|224.134|
|snapshot_idle_four/34|after|24|228.221|211.589|234.375|11|3|0.000|
|snapshot_idle_four/35|before|24|2.293|3.255|15.625|0|1|0.000|
|snapshot_idle_four/35|after|24|2.494|2.604|15.625|0|1|0.000|
|snapshot_idle_four/36|before|24|3.667|3.906|15.625|0|1|0.000|
|snapshot_idle_four/36|after|24|4.284|3.906|15.625|0|1|0.000|
|snapshot_idle_four/37|before|24|2.736|1.302|15.625|0|1|0.000|
|snapshot_idle_four/37|after|24|3.135|2.604|15.625|0|1|0.000|
|snapshot_idle_four/38|before|24|876.342|421.875|531.250|0|4|150.218|
|snapshot_idle_four/38|after|24|723.199|451.823|562.500|7|4|0.000|
|snapshot_idle_four/39|before|24|0.877|0.651|0.000|0|1|0.000|
|snapshot_idle_four/39|after|24|0.851|1.302|15.625|0|1|0.000|
|snapshot_idle_four/40|before|24|3.406|3.255|15.625|0|1|0.000|
|snapshot_idle_four/40|after|24|3.792|1.302|15.625|0|1|0.000|
|snapshot_load/0|before|117|15.284|7.345|15.625|0|1|0.000|
|snapshot_load/0|after|157|10.504|6.170|15.625|0|1|0.000|
|snapshot_load/1|before|117|9.654|5.743|15.625|0|1|0.000|
|snapshot_load/1|after|157|12.549|7.265|15.625|0|1|0.000|
|snapshot_load/2|before|118|11.248|6.091|15.625|0|1|0.000|
|snapshot_load/2|after|157|9.772|6.469|15.625|0|1|0.000|
|snapshot_load/3|before|118|11.811|10.196|15.625|0|1|0.000|
|snapshot_load/3|after|158|14.088|7.615|15.625|0|1|0.000|
|snapshot_load/4|before|118|11.524|7.812|15.625|0|1|0.000|
|snapshot_load/4|after|158|10.736|9.098|15.625|0|1|0.000|
|snapshot_load/5|before|118|17.249|8.739|15.625|0|1|0.000|
|snapshot_load/5|after|158|17.289|9.395|15.625|0|1|0.000|
|snapshot_load/6|before|118|26.450|15.625|31.250|0|1|0.000|
|snapshot_load/6|after|158|26.410|15.922|31.250|0|1|0.000|
|snapshot_load/7|before|118|9.199|5.694|15.625|0|1|0.000|
|snapshot_load/7|after|158|11.036|6.329|15.625|0|1|0.000|
|snapshot_load/8|before|118|13.218|9.004|15.625|0|1|0.000|
|snapshot_load/8|after|158|13.687|8.999|15.625|0|1|0.000|
|snapshot_load/9|before|119|14.466|8.141|15.625|0|1|0.000|
|snapshot_load/9|after|158|15.566|8.505|15.625|0|1|0.000|
|snapshot_load/10|before|119|21.877|13.918|31.250|0|1|0.000|
|snapshot_load/10|after|158|30.010|12.955|31.250|0|1|0.000|
|snapshot_load/11|before|119|309.276|43.592|62.500|0|4|268.554|
|snapshot_load/11|after|158|58.928|33.327|62.500|95|3|0.000|
|snapshot_load/12|before|118|13.988|8.475|15.625|0|1|0.000|
|snapshot_load/12|after|158|15.267|8.703|15.625|0|1|0.000|
|snapshot_load/13|before|118|132.196|17.346|31.250|0|3|113.611|
|snapshot_load/13|after|158|26.560|18.394|31.250|11|3|0.000|
|snapshot_load/14|before|118|17.972|9.534|15.625|0|1|0.000|
|snapshot_load/14|after|157|14.283|8.758|15.625|0|1|0.000|
|snapshot_load/15|before|118|301.967|183.792|218.750|0|4|163.332|
|snapshot_load/15|after|157|211.492|117.138|203.125|86|4|0.000|
|snapshot_load/16|before|120|189.254|52.995|78.125|0|3|138.231|
|snapshot_load/16|after|157|70.028|43.292|62.500|85|3|0.000|
|snapshot_load/17|before|120|7.366|5.078|15.625|0|1|0.000|
|snapshot_load/17|after|158|9.606|5.637|15.625|0|1|0.000|
|snapshot_load/18|before|120|239.346|96.094|125.000|0|4|159.984|
|snapshot_load/18|after|158|115.236|80.993|125.000|65|4|0.000|
|snapshot_load/19|before|120|15.296|10.417|15.625|0|1|0.000|
|snapshot_load/19|after|158|14.747|9.988|15.625|0|1|0.000|
|snapshot_load/20|before|120|14.407|10.286|15.625|0|1|0.000|
|snapshot_load/20|after|158|15.893|10.483|15.625|0|1|0.000|
|snapshot_load/21|before|120|52.031|36.198|46.875|0|1|0.000|
|snapshot_load/21|after|158|53.505|34.612|46.875|0|1|0.000|
|snapshot_load/22|before|119|517.628|292.148|328.125|0|4|204.999|
|snapshot_load/22|after|158|386.587|203.619|343.750|84|4|0.000|
|snapshot_load/23|before|118|490.434|299.788|343.750|0|4|189.248|
|snapshot_load/23|after|158|430.759|307.555|343.750|65|4|0.000|
|snapshot_load/24|before|118|392.536|239.804|281.250|0|3|159.459|
|snapshot_load/24|after|157|313.542|245.223|281.250|52|3|0.000|
|snapshot_load/25|before|119|199.347|19.695|31.250|0|1|180.060|
|snapshot_load/25|after|157|34.683|19.805|31.250|43|1|0.000|
|snapshot_load/26|before|119|180.044|32.694|62.500|0|3|154.500|
|snapshot_load/26|after|158|43.469|29.272|62.500|65|3|0.000|
|snapshot_load/27|before|119|20.100|9.716|15.625|0|1|0.000|
|snapshot_load/27|after|158|17.614|9.988|15.625|0|1|0.000|
|snapshot_load/28|before|119|302.458|63.419|93.750|0|4|220.492|
|snapshot_load/28|after|159|75.055|49.135|78.125|81|4|0.000|
|snapshot_load/29|before|119|17.442|11.555|15.625|0|1|0.000|
|snapshot_load/29|after|159|19.228|12.087|15.625|0|1|0.000|
|snapshot_load/30|before|119|430.093|316.833|359.375|0|4|159.701|
|snapshot_load/30|after|159|393.610|235.947|359.375|81|4|0.000|
|snapshot_load/31|before|119|268.805|41.623|46.875|0|1|217.796|
|snapshot_load/31|after|158|63.717|40.843|46.875|69|1|0.000|
|snapshot_load/32|before|120|263.509|101.693|125.000|0|2|173.424|
|snapshot_load/32|after|158|140.345|103.936|125.000|84|2|0.000|
|snapshot_load/33|before|120|188.145|77.865|93.750|0|2|111.596|
|snapshot_load/33|after|157|105.964|78.722|93.750|77|2|0.000|
|snapshot_load/34|before|121|359.197|224.174|265.625|0|3|146.625|
|snapshot_load/34|after|157|327.719|233.977|265.625|73|3|0.000|
|snapshot_load/35|before|118|3.259|2.913|15.625|0|1|0.000|
|snapshot_load/35|after|158|3.582|2.868|15.625|0|1|0.000|
|snapshot_load/36|before|118|8.634|3.840|15.625|0|1|0.000|
|snapshot_load/36|after|158|5.737|3.560|15.625|0|1|0.000|
|snapshot_load/37|before|118|4.303|2.119|15.625|0|1|0.000|
|snapshot_load/37|after|160|4.207|3.320|15.625|0|1|0.000|
|snapshot_load/38|before|118|874.198|472.458|578.125|0|4|227.597|
|snapshot_load/38|after|160|993.959|522.559|640.625|52|4|0.000|
|snapshot_load/39|before|116|2.452|1.751|15.625|0|1|0.000|
|snapshot_load/39|after|157|3.884|1.393|15.625|0|1|0.000|
|snapshot_load/40|before|116|11.646|4.445|15.625|0|1|0.000|
|snapshot_load/40|after|157|12.269|6.170|15.625|0|1|0.000|

Kapanış: KALDI. Ek 4R zx p95 894,895→994,314 ms; profilli loaded zx parent+child ortalama CPU 472,458→522,559 ms arttı. İzin beklemesinin kalkması tek başına kabul değildir. 1R kod yolu/fallback sıfır kanıtı, bütün süre ve CPU değerlerinin aynı olacağı anlamına gelmez; sayısal farklar yukarıda korunur. [Bütün kapılar ve duruş](HYBRID-SERIAL-50K-20261030.md).

[Bütün CPU blokları/auxhamörnekleri](benchmarks/hybrid-serial-extra50-20261030-summary.json) · [Her sequence/thread ve kaynak sayaçları](benchmarks/hybrid-serial-extra50-20261030-requests.json) · [SHA/exit/readonlysource ve bütün loglar](benchmarks/hybrid-serial-extra50-20261030-design.json). Profil4Rroller DBdeilerleyen yazıları kullanır;fiziksel yerleşim aynı diye sunulmaz. Kötü sonuçlar çıkarılmadı;the zx gate dışında bilgi olarak kalır.
