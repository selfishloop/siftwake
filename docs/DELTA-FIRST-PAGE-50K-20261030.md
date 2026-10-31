# Capped first-page delta — 30 Ekim 2026

Adım 1 kullanıcı tarafından kabul edildi. Yazmasız maske sonuçları için neden atfı kanıtlanmadı; yeni maske kabul koşusu yapılmadı. Bu rapor yalnız ayrı delta düzeltmesini ölçer. A gerçek eşzamanlı yük kapısı ayrıca değerlendirilecek.

## Değişiklik ve doğruluk

`first_delta` açık `Option<usize>` cap alır. First-page offset0 çağrısı Some(limit), experimental exact None gönderir. Exact/serial delta yolu değişmez. Streamed primary, owned docs boşken light BTreeMap sırasıyla ilk N eşleşmede durur. Owned/mixed yol mevcut delta sonuçlarını tam hesaplar, public Key sırasıyla sıralar, sonra keser; owned slot sırası kullanılmaz. Cancellable shadow BTreeSet yolunda aynı scalar semantiği korunarak N eşleşmede durulur; son token kontrolü kalır. Full shadow/mask, revision/pin/sequence, kaynak/indeks CRC ve biçimler değişmez. Global min-heap artık kullanmadığı delta kuyruğunu hesaplamaz.

Yeni oracle testi source/delta N sınırları, reverse ingest sırası, update/delete, cap dışında eski source revizyonlarının görünmemesi, eski snapshot/sequence ve cancel durumlarını exact referansla karşılaştırır. Signed-zero testine hem normal hem cancellable first-page eklendi: owned/light mevcut farkı korunur, birleştirilmedi. Seçilen WAL payload tek bayt bozulunca normal/cancellable capped yol CRC hatası vermelidir; ayrı test eklendi. Önceden var olan tam test paketi ayrıca çalıştırılır.

## Yöntem ve sınırlar

881756DA… cap200k corpus ilk50k; source/private/ değişmedi/okunmadı. Sabit FDB slotu100k mask-only olduğu için yalnız bu owner slot reset ile50k kuruldu; DB kopyası ve yeni corpus yok. Önce500 tekli update; otomatik flush nedeniyle ölçüm açılışında244 aktif delta anahtarı kaldı. Bu500 pending-delta dirty-open testi değildir. Aynı DB/modelde kontrol/aday/aday/kontrol, önce0/3/39/40 1R+4R, sonra41-case 1R+4R. Warm OS cache, eviction yok; her sorgu payload/model oracle. N50 hybrid budget200. Her kolda5 round/reader; pooled 1R10,4R40 örnek/case/kol. Esas kontrol kabul edilmiş maske + eski uncapped delta ikilisi; aday yalnız delta algoritması farklı. CPU ayrı ölçülmedi. Negatif sorgular tam delta tarar; erken bitiş kazancı beklenmez. Kötü p95 değerleri de tabloda.

[Ham tasarım, program SHA/exit/lifecycle](benchmarks/delta-cap50-20261030-design.json) · [Bütün örneklerden sonuç özeti](benchmarks/delta-cap50-20261030-summary.json). Process RAM model/oracle/katalog içerir; motor-only D sayacı yerine sunulmaz. SQLite referansı A koşusunda aynı corpus/FTS5 ayarlarıyla tekrar ölçülecek; bu delta-only mikro A/B SQLite ölçümü değildir.

## Önce dört sorgu

|Okuyucu / case / sorgu|Kontrol p95 ms|Aday p95 ms|Fark ms|
|---|---:|---:|---:|
|1R /0 /olduğu|4.638|3.469|-1.169|
|1R /3 /arasında|6.221|3.992|-2.228|
|1R /39 /qzxkvwnever|0.467|0.410|-0.057|
|1R /40 /transubstantiationz|1.886|1.853|-0.033|
|4R /0 /olduğu|5.040|4.096|-0.944|
|4R /3 /arasında|7.810|5.441|-2.369|
|4R /39 /qzxkvwnever|0.280|0.339|+0.059|
|4R /40 /transubstantiationz|3.425|2.662|-0.763|
## Ardından41 sorgu

|Okuyucu / case / sorgu|Kontrol p95 ms|Aday p95 ms|Fark ms|
|---|---:|---:|---:|
|1R /0 /olduğu|5.997|5.075|-0.922|
|1R /1 /büyük|5.012|4.238|-0.774|
|1R /2 /ayrıca|5.463|4.366|-1.097|
|1R /3 /arasında|8.129|6.160|-1.970|
|1R /4 /birlikte|4.994|4.767|-0.227|
|1R /5 /sanat|5.218|6.195|+0.977|
|1R /6 /görenler|10.229|10.393|+0.164|
|1R /7 /millî|3.328|3.316|-0.012|
|1R /8 /ikinci|4.828|5.056|+0.228|
|1R /9 /toplam|4.855|4.866|+0.011|
|1R /10 /isimli|9.467|9.578|+0.111|
|1R /11 /öldürür|31.727|29.805|-1.922|
|1R /12 /ölümü|6.714|6.803|+0.088|
|1R /13 /akarsu|17.996|17.588|-0.408|
|1R /14 /demokratik|6.187|6.760|+0.574|
|1R /15 /çalışmalarıyla|81.849|81.960|+0.111|
|1R /16 /üreticisi|33.385|33.652|+0.267|
|1R /17 /joseph|2.641|3.700|+1.059|
|1R /18 /leyla|49.925|50.442|+0.517|
|1R /19 /protest|8.947|9.151|+0.204|
|1R /20 /furlong|10.336|10.533|+0.197|
|1R /21 /korkaklığa|36.038|35.647|-0.391|
|1R /22 /ormanların|123.764|121.727|-2.038|
|1R /23 /yanıklarda|128.085|129.757|+1.672|
|1R /24 /külliyatı|112.920|111.167|-1.753|
|1R /25 /timmy|18.211|19.191|+0.980|
|1R /26 /yüklük|24.492|26.074|+1.582|
|1R /27 /ilçesine köyleri|7.285|9.131|+1.846|
|1R /28 /geldiği buldu|40.200|40.136|-0.065|
|1R /29 /chicago studio|9.862|9.935|+0.073|
|1R /30 /kısım dişli|140.330|137.210|-3.120|
|1R /31 /güzeli zafer|42.943|41.565|-1.378|
|1R /32 /İnternetten hazinem|81.212|81.632|+0.421|
|1R /33 /oyuncudur atlanta|60.812|72.691|+11.879|
|1R /34 /kendisine çıkacağını|108.473|112.585|+4.112|
|1R /35 /ve|1.206|1.954|+0.748|
|1R /36 /if|2.480|2.627|+0.147|
|1R /37 /id|1.800|2.132|+0.333|
|1R /38 /zx|135.789|138.834|+3.045|
|1R /39 /qzxkvwnever|0.881|0.993|+0.113|
|1R /40 /transubstantiationz|3.366|4.206|+0.840|
|4R /0 /olduğu|7.686|6.739|-0.947|
|4R /1 /büyük|6.454|6.197|-0.257|
|4R /2 /ayrıca|7.103|7.766|+0.662|
|4R /3 /arasında|10.835|8.859|-1.976|
|4R /4 /birlikte|7.504|7.485|-0.019|
|4R /5 /sanat|7.579|9.444|+1.865|
|4R /6 /görenler|13.782|14.651|+0.870|
|4R /7 /millî|5.864|6.126|+0.262|
|4R /8 /ikinci|7.430|7.421|-0.010|
|4R /9 /toplam|6.777|7.548|+0.771|
|4R /10 /isimli|13.193|13.526|+0.333|
|4R /11 /öldürür|169.052|159.958|-9.095|
|4R /12 /ölümü|9.114|8.204|-0.910|
|4R /13 /akarsu|237.902|133.569|-104.333|
|4R /14 /demokratik|8.580|9.540|+0.960|
|4R /15 /çalışmalarıyla|379.348|354.466|-24.883|
|4R /16 /üreticisi|371.378|175.639|-195.738|
|4R /17 /joseph|5.438|5.621|+0.183|
|4R /18 /leyla|204.333|248.844|+44.511|
|4R /19 /protest|11.777|12.155|+0.378|
|4R /20 /furlong|11.458|11.234|-0.224|
|4R /21 /korkaklığa|40.639|41.031|+0.392|
|4R /22 /ormanların|394.176|383.887|-10.289|
|4R /23 /yanıklarda|438.670|415.510|-23.160|
|4R /24 /külliyatı|351.958|400.794|+48.836|
|4R /25 /timmy|112.443|232.403|+119.960|
|4R /26 /yüklük|129.405|131.867|+2.462|
|4R /27 /ilçesine köyleri|10.104|9.732|-0.372|
|4R /28 /geldiği buldu|202.380|160.149|-42.230|
|4R /29 /chicago studio|10.553|11.791|+1.237|
|4R /30 /kısım dişli|445.353|426.885|-18.467|
|4R /31 /güzeli zafer|230.693|157.363|-73.330|
|4R /32 /İnternetten hazinem|200.098|263.340|+63.241|
|4R /33 /oyuncudur atlanta|422.359|255.197|-167.162|
|4R /34 /kendisine çıkacağını|307.593|481.721|+174.128|
|4R /35 /ve|3.796|3.609|-0.187|
|4R /36 /if|5.998|5.878|-0.120|
|4R /37 /id|5.177|4.223|-0.954|
|4R /38 /zx|685.572|697.448|+11.876|
|4R /39 /qzxkvwnever|1.114|1.256|+0.141|
|4R /40 /transubstantiationz|4.208|4.777|+0.569|

## Kapı ve kalan işler

Delta oracle GEÇTİ;249 default test ve6 diagnostic first testi, fmt,default/diagnostic clippy GEÇTİ.3kill/reopen ve50000GET+41exact eski okuyucu GEÇTİ. [Test](benchmarks/delta-cap-20261030-v2-all-tests.log), [CRC dahil diagnostic test](benchmarks/delta-cap-20261030-diag-tests.log), [kurtarma](benchmarks/delta-cap50-20261030-recovery.json). İlk clippy denemesi testte Active::path kullanımı yüzünden derlenmedi; test manifest::path ile düzeltildi, başarılı v2 kayıtları esas. Motor hot-path aynı dondurulmuş ikiliyle ölçüldü; son ek CRC testi release davranışını değiştirmez. **Gerçek A4R+tekli kapısı daha sonra2/41 sorguda KALDI; duruldu.** [Tam A raporu](CONCURRENT-DELTA-1-50K-20261030.md). Yeni performans eşiği eklenmedi; 4R+yazıcı A için idle≤30ms ise loaded≤30ms, diğerleri≤2×idle; throughput≥%75 ve SQLite pooled üstü, uzun tekli20/50/100ms p95/p99/p99,9. A başarısızsa D/B/C/E ve WAL düzeltmesine geçilmez. RAM D, motor dışı profil/model/oracle/history hariç ölçülecek. B/C/E,2,6sn uç ve500 drain, Frunner anahtarı, timmy cache/mutex açık. Format/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k yok.
