# FDB 50k sıcak açılış — hafif maske tanısı ve WPR karşılaştırması

Kapsam 7 Kasım 2026, **12:48:33–13:13:33 UTC**. Aynı Cap200k ilk 50k
FDB, corpus kimliği 881756DA…, sequence **529967**, generation **8185**,
**52 run**, 16 delta anahtarı. Önceki ölçümün 159 dosyalık kopyası
yeniden kullanıldı. Her maske bloğundan önce **467.167.854 B** normal cached
okuma ile ısıtıldı. Fiziksel residency ölçülmedi.

## Hafif maske ölçümü

Ağır `diagnostic-timing` kapalı. Ana ölçümde paged açılış görünürlük
döngüsünün çevresinde iki saat okuması, var olan aralık kontrolünü geçen
çiftler için bir sayaç var. Döngü çok küçük olan görünür belge sayımını da
içeriyor. Önceki ayrıntılı tanıda bu sayım p50 0,043 ms idi, bu turda
ayrıca zamanlanmadı. Çift başına saat okumaları yalnız ayrı ayrıntı kolunda.
İki ikili default release olarak geçici derleme kopyasında derlendi.

| WPR kapalı sıcak kol | N | Strict-open p50 / p95 (ms) | Maske döngüsü p50 / p95 (ms) |
| --- | ---: | ---: | ---: |
| Sayaçsız kontrol | 10 | **1.251,08 / 1.276,46** | — |
| Hafif sayaç | 20 | **1.280,50 / 1.317,08** | **764,05 / 801,74** |
| Çift zamanlama | 10 | 1.269,67 / 1.421,79 | 757,90 / 806,44 |

**Maske payı %59,97**, 20 hafif örneğin süre toplamları üzerinden.
Önceki ağır profil 1.213,39 ms ve %69,67 vermişti. Yeni hafif kol,
10 bloktaki iki hafif açılış ortalamasının kontrolle eşleştirilmesinde
medyan **+36,37 ms** fark verdi (−0,64…+58,26 ms). Bu fark ölçülen
toplam duvar süresidir. Sayaç maliyeti ve koşu değişkenliği ayrı
ölçülmedi. 764,05 ms hafif sayaçlı ölçümdür. Sayaçsız açılıştaki kesin
maske süresi bilinmiyor.

10 dengeli blokta kontrol/hafif/çift/hafif ve ters sıra. 40 yeni proses
başlatıldı, ikişer PID Windows tarafından daha sonra yeniden kullanıldı.
Prosesler sırayla beklenip kapatıldı ve zaman pencereleri çakışmadı.
p95 en yakın üst sıra, N=10’da maksimum. Ölçüm **13:02:11,793–13:03:14 UTC**.

## Run çifti dağılımı

**697/1.326 (%52,56)** çift var olan min/max anahtar aralığı kontrolünü
geçti. Kalan **629 çift** zaten eleniyor. Aralık örtüşmesi gerçek ortak
anahtar olduğunu göstermiyor. Çift kimlikleri ve slot sayıları her
ayrıntı örneğinde aynı. 6.970 çağrıda `hide_keys` p50 **0,407 ms**,
p95 **3,885 ms**, tekil max **13,707 ms**. Her açılışta ölçülen örtüşen
çağrıların toplamı p50 **757,54 ms**, p95 **805,95 ms**. Süreler wall
time, I/O ve CPU payları ölçülmedi.

| Eski run slotu | Yeni run slotu | Çift | Çağrı p50 / p95 (ms) | Çift süresi payı |
| ---: | ---: | ---: | ---: | ---: |
| 0–255 | 1.024–8.191 | 144 | 1,875 / 2,078 | %36,08 |
| 1.024–8.191 | 1.024–8.191 | 66 | 3,891 / 4,271 | %34,19 |
| 256–1.023 | 1.024–8.191 | 78 | 2,063 / 2,266 | %21,66 |
| Diğer boyutlar | Diğer boyutlar | 409 | Ham tabloda | %8,07 |

**Yeni run’ı 1.024–8.191 slot olan 288 çift, çift süresinin %91,93’ü.**
`hide_keys` örtüşme kontrolünden sonra yeni run’ın slotları üzerinde
yürüyüp her anahtarı eski run’da arıyor. En yüksek medyanlı çift
`1ca8 → 1f10`, 2.306 → 2.306 slot, **4,040 / 4,505 ms** p50/p95.
`1f5d → 1faa` aynı boyutta **3,960 / 6,186 ms**.
En pahalı 10 çift toplam sürenin yalnız **%5,28**’i.
[hide_keys kodu](../src/segment.rs#L1688) ·
[697 çiftin kimliği, boyutu ve dağılımı](benchmarks/strict-open-followup-50k-20261107/analysis.json)

## WPR etkisi

**Geçersiz.** İlk kullanılmayan pencere 13:01:13 UTC’de kayıt olmadan
kapandı. Maske serisi bitince pencere yeniden açıldı. Yönetici betiğinin
AST kontrolü geçti. Aynı profilsiz ikili ve PID **8796** ile ilk sıra
kapalı→açık tamamlanarak 5+5 süre ve sorgu oracle’ı üretildi. WPR
başlatma ve durdurma, kaydedilen açılış saatlerinin dışında kaldı.
WPR start exit 0, sinyal **13:05:37,588 UTC**, stop isteği
**13:05:46,943 UTC**. Stop **0x800705DE**, Windows açıklaması
“event log file is full”. Kalan **System Collector.etl 50.331.648 B**,
yani profilin **48 MiB** sınırı. Canlı dosya boyutu örneklemesi bu dolumu
stop öncesi yakalayamadı. Birleşik ETL oluşmadı, olay kaybı ölçülemedi.
Son `wpr -status` sonucu **WPR is not recording**. Ters sıra başlamadı.

| İlk sıra, geçersiz ham veri | N | Strict-open p50 / p95 (ms) | Open-start → ilk sonuç p50 / p95 (ms) |
| --- | ---: | ---: | ---: |
| Kapalı | 5 | 1.641,06 / 1.804,57 | 1.764,09 / 1.927,59 |
| Açık | 5 | 1.703,13 / 1.784,45 | 1.832,93 / 1.908,01 |

Aynı tekrar numarasındaki ham farklar **+83,18, +170,64, +62,07,
+58,70, −138,15 ms**. Tek sıra ve dolmuş iz nedeniyle **WPR maliyeti
bilinmiyor**. WPR kapalı kolun 1.641 ms medyanı ayrı prosesli maske
kontrolünün 1.251 ms medyanından yüksek. Bu oturum farkının nedeni
ölçülmedi. Bütün 10 sonuçta oracle ve sonda slot/kopya/model hash’i
geçti. 1 sıcak kayıt başlatıldı, **0 geçerli dengeli blok**, 0 purge.
Kalan ETL saklandı. Önceki ETW kanıtları dahil kapanışta **115.139.146 B
ETL**, yaklaşık **293,4 MB** toplam. Bu sıcak izde System Collector
sınırı görüldü. Önceki cold denemede dolan collector’ın kimliği hâlâ
bilinmiyor.
[AST doğrulanmış yönetici betiği](benchmarks/strict-open-followup-50k-20261107/run-warm-wpr.ps1) ·
[WPR olay kaydı](benchmarks/strict-open-followup-50k-20261107/wpr-admin.json) ·
[WPR ham 10 örnek](benchmarks/strict-open-followup-50k-20261107/wpr-samples.jsonl) ·
[WPR son doğrulama](benchmarks/strict-open-followup-50k-20261107/wpr-validation.json)

## Seçenekler

Koşullu brüt tahminler başlangıç maliyetini mevcut ölçümden alıyor.

| Seçenek | Tahmini kazanç | Risk ve koşul | Dosya biçimi etkisi |
| --- | --- | --- | --- |
| Run başına sınırlı, sıralı anahtar önbelleği | Yeni run’ı büyük çiftlerin yaklaşık 0,70 s’lik işinin %25–50’sini kaldırabilirse **~175–350 ms** brüt | Geçici RAM ve ilk doldurma. Slot/anahtar kimliği, sıralama ve tombstone semantiği korunmalı. Decode ile eski run lookup payı ayrı ölçülmedi. | Bellek içi tasarımda yok |
| Daha dar aralık veya sayfa bazlı örtüşme ön elemesi | Süre ağırlıklı işin %10’u güvenli elenebilirse **~76 ms** brüt | Min/max 629 çifti zaten atlıyor. Gerçek ortak anahtar oranı bilinmiyor. Ek kontrol maliyeti ve yanlış negatif riski ölçülmeli. | Mevcut metadata ile yok. Yeni özet yazılırsa türetilmiş indeks/cache sürümü gerekir. |
| Açılışta tembel veya artımlı maske | **~764 ms** brüt tavan. Ertelenen iş ilk sonuçta kalırsa uçtan uca kazanç azalabilir. | Tam görünür sayı ve ilk sonuç semantiği korunmalı. Artımlı yeniden kullanım sıra, shadow, tombstone ve snapshot pin kanıtı ister. | Bellek içi yeniden kullanımda yok. Kalıcı özet eklenirse ayrı cache kimliği gerekir. |
| Yazma/yayım anında türetilmiş maske saklama | **~764 ms** brüt tavan eksi okuma ve bağlama doğrulaması. Ölçülen 50 run için salt bitmap yükü **6.272 B**, kalan iki run ve başlıklar hariç. | Yayım maliyeti yazmaya taşınır. Kuşak, run sırası, source kimliği, CRC ve WAL/shadow bağlamı doğrulanmalı. Atomik yayım ve geçersiz cache fallback’i gerekir. | Kaynak/WAL/manifest değişmeden sürümlü türetilmiş sidecar mümkün. Kalıcı biçime bağlamak ayrı onay ister. |

## Doğruluk ve kapsam

40/40 maske ve kontrol örneği case0 sequence 529967, 50 hit, Verified ve
CRC32 725765487 oracle’ını geçti. Her örnekten sonra kopyanın dosya
envanteri, boyutu, mtime ve öznitelikleri aynı. Sonda canlı slot ve kopya
159/159 SHA-256, model SHA eşit. Açılış doğrulaması ve strict CRC korundu.
Üretim motoru, varsayılan, eşik ve kapı değişmedi. Yeni purge ve cold iz
çözümleme yok. Blok 2b açık. Maske önerilerinden hiçbiri uygulanmadı.

Önceki raporda **569.256.286 B** olarak kaydedilen
`C:\Users\Serkan\AppData\Local\Temp\fdb-strict-open-20261107-123212`
klasörü bu turda da bırakıldı. İçindeki mevcut kopya yeniden kullanıldı
ve ek geçici derleme çıktıları aynı klasöre yazıldı. Temizlik kullanıcıya
bırakıldı.
[Ham 40 ölçüm](benchmarks/strict-open-followup-50k-20261107/mask-samples.jsonl) ·
[Maske doğrulaması](benchmarks/strict-open-followup-50k-20261107/mask-validation.json) ·
[Önceki aşama raporu](STRICT-OPEN-PHASES-50K-20261107.md)


Rapor kapanışı **13:07:29 UTC**. Geçici klasörün bu tur sonundaki boyutu **579007388 B**.
