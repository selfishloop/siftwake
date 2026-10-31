# 4R + 1li yazıcı — 30 Ekim 2026

**Kapı: KALDI — sonraki maddelere geçilmedi.** Son SHA/test/kapsam ve kontrol/aday model eşitliği [audit'te](benchmarks/delta-cap50-20261030-final-audit.json). Adım 1 kabulü [doğrudan kullanıcı kararı](benchmarks/mask-stage1-20261030-user-acceptance.json) olarak ayrıca kaydedildi; eski ham kararlar değiştirilmedi.

881756DA… corpus ilk50k; akışlı API, hybrid200/N50, strict kaynak/indeks CRC, aynı FTS5 trigram SQLite ayarları. Sabit owner slotlar dışında DB yok. Her kol fresh50k aynı planla kuruldu; normal kabul sırası FDB kontrol → SQLite kontrol → SQLite4R → FDB4R. Tek dengeli çift; çoklu çift iddiası yok. Warm OS cache, eviction yok. Her idle/yüklü sorguda bağımsız commit-sequence/payload/model oracle. Oracle search zamanının dışında, ama CPU/I/O yükü olarak koşuda mevcut. Yeni p95 sınırı eklenmedi: idle≤30ms ise loaded≤30ms, diğerleri≤2×idle. Throughput≥%75 ve SQLite pooled üstü. Uzun tekli p95/p99/p99,9≤20/50/100ms; max bilgi.

[Ham kollar/SHA/lifecycle](benchmarks/delta-load50-20261030-b1-design.json) · [Tam kapı](benchmarks/delta-load50-20261030-b1-gate.json) · [>500ms bütün olaylar ve bekleme](benchmarks/delta-load50-20261030-b1-slow-waits.json).

## Yazma ve kapılar

|Motor|Kontrol ops/s|4R ops/s|Oran|4R p95 / p99 / p99,9 ms|Max ms|>100ms|
|---|---:|---:|---:|---|---:|---:|
|fdb|326.23|298.19|91.40%|4.900 /16.018 /23.260|48.481|0 (0.00000%)|
|sqlite|280.49|199.42|71.10%|11.008 /24.115 /133.342|391.051|40 (0.24414%)|

## 41 sorgu — her iki motorun bütün sonuçları

|Case/sorgu|FDB idle / loaded p95 ms|Sınır|Karar|SQLite idle / loaded ms|
|---|---|---:|---|---|
|0: olduğu|5.338 /11.004|30.000|GEÇTİ|664.965 /863.149|
|1: büyük|4.085 /12.931|30.000|GEÇTİ|611.094 /812.936|
|2: ayrıca|5.102 /11.397|30.000|GEÇTİ|718.383 /943.353|
|3: arasında|5.904 /17.016|30.000|GEÇTİ|982.255 /1225.231|
|4: birlikte|5.198 /15.675|30.000|GEÇTİ|627.426 /817.040|
|5: sanat|5.593 /13.993|30.000|GEÇTİ|580.910 /712.063|
|6: görenler|12.096 /25.705|30.000|GEÇTİ|624.619 /785.841|
|7: millî|5.343 /9.905|30.000|GEÇTİ|177.353 /251.210|
|8: ikinci|6.491 /11.408|30.000|GEÇTİ|692.344 /882.993|
|9: toplam|5.246 /16.980|30.000|GEÇTİ|524.418 /664.941|
|10: isimli|10.527 /28.046|30.000|GEÇTİ|689.335 /880.647|
|11: öldürür|191.602 /373.651|383.203|GEÇTİ|301.694 /417.061|
|12: ölümü|5.690 /16.444|30.000|GEÇTİ|578.698 /698.977|
|13: akarsu|241.724 /51.103|483.448|GEÇTİ|222.429 /286.115|
|14: demokratik|6.878 /13.595|30.000|GEÇTİ|120.996 /164.666|
|15: çalışmalarıyla|359.491 /430.969|718.981|GEÇTİ|469.018 /594.992|
|16: üreticisi|79.984 /288.698|159.968|KALDI|357.538 /448.073|
|17: joseph|3.914 /8.072|30.000|GEÇTİ|76.107 /106.394|
|18: leyla|208.108 /404.957|416.216|GEÇTİ|443.179 /547.159|
|19: protest|10.783 /17.962|30.000|GEÇTİ|176.218 /240.064|
|20: furlong|10.835 /18.619|30.000|GEÇTİ|15.776 /37.274|
|21: korkaklığa|38.134 /50.045|76.268|GEÇTİ|50.977 /60.095|
|22: ormanların|633.113 /516.466|1266.226|GEÇTİ|583.241 /681.152|
|23: yanıklarda|536.404 /500.581|1072.809|GEÇTİ|444.297 /640.304|
|24: külliyatı|337.607 /475.579|675.214|GEÇTİ|369.878 /454.622|
|25: timmy|86.669 /199.126|173.339|KALDI|31.487 /71.442|
|26: yüklük|210.248 /139.159|420.497|GEÇTİ|303.562 /372.486|
|27: ilçesine köyleri|8.754 /16.214|30.000|GEÇTİ|158.783 /192.474|
|28: geldiği buldu|456.910 /221.756|913.819|GEÇTİ|384.320 /509.031|
|29: chicago studio|11.432 /17.274|30.000|GEÇTİ|19.704 /42.630|
|30: kısım dişli|424.528 /589.208|849.057|GEÇTİ|503.286 /646.364|
|31: güzeli zafer|267.220 /282.166|534.439|GEÇTİ|60.248 /119.338|
|32: İnternetten hazinem|358.239 /366.811|716.479|GEÇTİ|197.391 /237.932|
|33: oyuncudur atlanta|548.860 /284.010|1097.720|GEÇTİ|111.043 /179.170|
|34: kendisine çıkacağını|281.884 /483.577|563.767|GEÇTİ|350.482 /424.313|
|35: ve|2.617 /4.787|30.000|GEÇTİ|934.996 /1248.608|
|36: if|5.620 /8.043|30.000|GEÇTİ|1192.902 /1501.673|
|37: id|3.520 /5.322|30.000|GEÇTİ|1070.691 /1338.895|
|38: zx|1035.089 /923.780|2070.178|GEÇTİ|1114.245 /1364.211|
|39: qzxkvwnever|1.625 /3.568|30.000|GEÇTİ|0.894 /4.697|
|40: transubstantiationz|4.351 /12.216|30.000|GEÇTİ|6.950 /15.220|

## Maske etkisi: aynı capped ikilide gerçek yük

Ana adaydan sonra ayrı maske-reference kolunda yalnız replacement reuse kapalıdır (full fallback), delta capped aynı. Kabul/SQLite throughput havuzuna katılmaz. Bu ilave tek yönlü karşılaştırma keşif niteliğindedir; dengeli tekrarlı neden kanıtı diye sunulmaz. Yeni yazmasız maske koşusu yapılmadı.

|Case|Full maske loaded p95 ms|Artımlı maske loaded p95 ms|
|---|---:|---:|
|0|12.968|11.004|
|1|10.149|12.931|
|2|11.929|11.397|
|3|24.215|17.016|
|4|14.974|15.675|
|5|17.691|13.993|
|6|35.465|25.705|
|7|13.876|9.905|
|8|16.633|11.408|
|9|17.137|16.980|
|10|37.125|28.046|
|11|398.047|373.651|
|12|13.401|16.444|
|13|111.817|51.103|
|14|14.695|13.595|
|15|447.212|430.969|
|16|345.868|288.698|
|17|8.411|8.072|
|18|395.581|404.957|
|19|26.885|17.962|
|20|27.822|18.619|
|21|54.010|50.045|
|22|510.058|516.466|
|23|555.456|500.581|
|24|490.303|475.579|
|25|155.134|199.126|
|26|256.553|139.159|
|27|28.404|16.214|
|28|259.130|221.756|
|29|33.205|17.274|
|30|540.489|589.208|
|31|186.959|282.166|
|32|252.898|366.811|
|33|250.168|284.010|
|34|438.475|483.577|
|35|4.572|4.787|
|36|7.929|8.043|
|37|5.735|5.322|
|38|925.538|923.780|
|39|3.648|3.568|
|40|15.133|12.216|

## Sınırlar ve açık işler

Process RAM; benchmark/model/oracle/history/katalog ve varsa trace içerir; motor-only D veya≤60/15MiB kapısı diye sunulmaz. Bütün normal/profile >500ms olaylar ham JSON listesinde. Eski2,6sn olayın nedeni bu farklı koşuyla geriye dönük kanıtlanmaz: BİLİNMİYOR. Antivirüs iddiası yok. Maske/Delta oracle, seçili WAL CRC,249 test/fmt/clippy ve3kill/reopen/eski okuyucu kanıtı [delta raporunda](DELTA-FIRST-PAGE-50K-20261030.md). Bu yükteki son modelin recovery ayrıca kaydedilecek.

A kapısı KALDI ise100lü veya D/B/C/E/WAL düzeltmesine geçilmez. Motor RAM,Frunner anahtarı,500 worker drain,timmy41-vs4/cache/mutex; ayrıca arama regresyonları/case34, disk/bakım, signed-zero ve bilinmeyen eski uçlar açık. Reclaim default kapalı. Biçim/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k yok.

## Kontrol ve yüklü commit kuyrukları — 16384 işlem/kol

|Motor/kol|p95 / p99 / p99,9 ms|Max ms|>100ms adet/oran|Process peak private / WS MiB|
|---|---|---:|---|---|
|fdb/pair0-control|4.446 /15.415 /28.769|216.825|2 /0.01221%|50.27 /54.99|
|sqlite/pair0-control|7.547 /21.170 /94.422|227.952|13 /0.07935%|42.73 /47.56|
|sqlite/pair0-loaded|11.008 /24.115 /133.342|391.051|40 /0.24414%|98.80 /102.31|
|fdb/pair0-loaded|4.900 /16.018 /23.260|48.481|0 /0.00000%|119.02 /121.64|
|fdb/mask-reference|5.352 /16.917 /46.728|235.171|3 /0.01831%|122.67 /125.44|

Bu RAM sayıları yalnız benchmark prosesidir; D ölçümü YAPILMADI. Profil/model/oracle/history dışarıda değildir. D için≤60/15MiB sonucu çıkarılmaz.

## Bekleme, neden ve kurtarma

Normal FDB kolunda2516 writer-active okuma; admission contention0, maksimum admission wait0ms. Yazıcıya bağlı cache/release beklemesi18okumada, en uzun2,1575ms. Bu sayaçlar bütün hibrit işçi/permit/posting mutex beklemelerini kapsamaz (`internal_wait_measured=false`); sıfır sayaçtan hiç cache çekişmesi yok sonucu çıkarılamaz. Üreticisi/timmy p95 örneklerinde raporlanan cache wait0;288,7/199,1ms nedenini kanıtlamaz. CPU,posting,I/O,scheduler ayrımı bu kolda yok: **neden BİLİNMİYOR**. Durma kuralıyla ek optimizasyon/profil başlatılmadı.

Aynı capped ikilinin tam maske kolu:282,16ops/s; artımlı maske298,19ops/s. Tam/artımlı maske toplam wall2742,56/1838,75ms; full rebuild21/1. Ancak timmy loaded p95 full155,13 / artımlı199,13ms; üreticisi345,87/288,70ms. Bu ilave tek sıra keşif karşılaştırmasında iki yönlü sonuçlar var; okuyucu için genel gerilememe/tek neden iddiası kurulmaz, kapı gevşetilmez.

Son16384lük modelde3 strict-open witness sonrası process kill + strict reopen + eski okuyucuda50000GET/41exact oracle GEÇTİ. Kaynak/canlı manifest SHA korundu. [Kurtarma kanıtı](benchmarks/delta-load50-20261030-b1-recovery.json). Tam testte WAL before/after ack,flush/merge/cache kill/kesilme/CRC ve snapshot pin testleri ayrıca geçti; güç kesintisi iddiası yok.

Tüm normal/ref benchmark alt süreçleri exit0; sürücü STOP_failed kapı kararıyla normal sonlandı.100lü devam sürücüsü STOP_B1_GATE_FAILED yazıp yeni worker başlatmadı. Açıklanamayan süreç kapanması yok. WMI yetkisizliğinde mevcut session/exit fallback sınırı geçerli.

**Durum:** A tekli sorgu39/41 GEÇTİ,2/41 KALDI; yazma throughput/tail GEÇTİ.100lü/profil, D(motor-only RAM),B(Frunner anahtarı),C(500 drain),E(timmy41-vs4 ve posting cache/mutex),WAL handle düzeltmesi BAŞLAMADI. Sonraki öneri yalnız bu iki sorguyu aynı4R+yazıcı yükünde first.delta_filter/posting/payload/permit-cache/CPU aşamalarına ayırmak; uygulama için kullanıcı kararı beklenir.
