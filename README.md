# FDB

[Contains sonrası aynı doğrulanmış byte'ları kullanma](docs/CHECKED-PAGE-10K-20261018.md):
dar prototipte quixote okuma131→81, ikinci CRC/decode50→0; candidate56
kontrolü korunur. Son400-örnek/variant kontrolde p95-%11,11/tipik-%17,92,
4/4 süreç hızlı. Geniş38-case15 düşük/23 yüksek p95 ve query/get private
medyan+64KiB/WS+170KiB nedeniyle varsayılan geçiş reddedildi. Kod/testler
patch'te saklanır; kaynak/format/corpus/DB aynı. Yazma/bakım, genel arama,
RAM/açılış/sıkıştırma ve concurrent/soğuk yük açık kalır.

[CRC/UTF-8 parçalamasını ölçme](docs/VALIDATION-CHUNK-10K-20261018.md)
reddedildi; varsayılan motor aynı. Aynı10k/400 örnek kontrolünde holmes
p95+%1,80 (3/4 çift yüksek), zx+%0,32 (4/4 çift yüksek); tipik blok da
yavaş. Normalde bu yola girmeyen the/200 farkı algoritma kazancı sayılmadı.
CRC/schema/UTF-8/identity kontrolleri korunur; yeni38-case/RAM koşusu yok.
Yazma/bakım, genel arama, RAM/açılış/disk ve eşzamanlı/soğuk yük açık.

[Gruplamayı hızlı sorgu yolundan ayırma kontrolü](docs/PAYLOAD-ISOLATED-10K-20261017.md)
de reddedildi; motor önceki sürümde. Aynı10k/üç sorgu/400 örnek kontrolünde
darcy p95 -%2,74, holmes +%0,98, the/200 -%2,98. the/200 gerilemesi bu
koşuda tekrarlanmadı; holmes kazancı ise süreçlerde2/4 kaldı. Gruplama için
yeni ayar turu yapılmayacak; genel arama, yazma/bakım, RAM/açılış/disk ve
eşzamanlı/soğuk yük işleri açık. Yeni38-case veya RAM sonucu iddiası yok.

[Aday payload okumalarını gruplama deneyi](docs/PAYLOAD-BATCH-10K-20261017.md)
reddedildi; varsayılan motor aynı. Aynı 10k veride holmes read 1285→1106,
darcy 2129→1700, watson 1954→1553; ek byte %0,1'in altında. holmes p95
-%9,18 olsa da the/200 ilk +%10,38, yoğun tekrarda +%6,02. A/A oynaklığı
bu artışları kapatmak için yeterli değil. Query peak private medyanı +46KiB,
disk aynı; kod arşiv patch'inde. Genel arama, yazma/bakım, Vec RAM, açılış,
sıkıştırma ve eşzamanlı/soğuk yük açık kalır.

[Seçilen belgelerde metadata tekrarını azaltma](docs/SEARCH-HYDRATION-10K-20261017.md):
aynı mevcut 10k / 350 MB veride 200 belgeli sayfa p95 6,496→5,779 ms
(-%11,04), 4/4 süreç ve 16/16 blok hızlı; tipik kazanç %12,75.
İzlenen metadata çağrıları 714→514, belge CRC/schema/identity korunur.
Ayrı query/get proses bellek tepesi medyanları artmadı; DB byte'ları aynı.
Geniş 38-case 23 düşük/15 yüksek; A/A p95 sapması ve normal kelimelerin
payload I/O maliyeti açık. Yazma/bakım, Vec RAM, açılış, sıkıştırma ve
eşzamanlı/soğuk yük bu dar sonuçla kapanmaz.
Güncel aynı-veri SQLite okuma karşılaştırmasında 38/38 p95 ve her sorguda
4/4 çift FDB düşük. Ayrı oracle'sız query/get peak private 8,78/12,83 MiB,
working set 12,53/16,70 MiB; disk 350/584 MB. Bunlar okuma yükü sonuçlarıdır;
normal Vec ingest RAM'i, yazma/bakım ve açılış bu tur yeniden ölçülmedi.

[Aynı350MB veride arama maliyetleri](docs/SEARCH-STAGES-10K-20261017.md):
Unicode kanıtlı kısa ASCII yolunda body fold kaldırıldı; zx/the-zx normal
strict blok p95 -%31,21/-%33,99, ikisi4/4 süreç ve32/32 blok hızlı.
Bütün38 tekrar:28 düşük/10 yüksek; A/A p95 kararsız, genel kapı açık.
Son query peak private/WS medyanı artmadı;18 DB dosyası349.805.698 B aynı.
CRC/schema/UTF-8/key/version,0.1 fixture ve formatlar korunur. Yazma/bakım,
genel RAM/açılış, sıkıştırma ve concurrent/cold işleri açık kalır.

[Eski nesillerin otomatik temizliği](docs/RECLAIM-10K-20261017.md):
mevcut10k'da105 eski dosya kaldırıldı; toplam973→350MB (-%64,04), canlı
kaynak/indeks/ready byte'ları aynı. Yeni yazmalarda da350MB; yeni corpus yok.
Güncel WAL, açık okuyucu ve worker çıktısı korunur. Kesilme/bozulma/kill
testleri ve eski okuyucuyla yeniden açılış geçti. İki yazma çiftinde toplam
medyan+%1,34, bakım+%2,82; açılış serileri değişken. Arama22/38, RAM ve
genel yazma/bakım/açılış, eşzamanlı/soğuk yük başlıkları açık kalır.

[SEG05 varsayılan yazıcı geçişi](docs/DEFAULT-SEG05-10K-20261017.md):
kullanıcı kararıyla yeni native indeksler ikili sözlük ve kaynak referanslarıyla
yazılır. Commit/flush, birleştirme ve cache onarımı belgeyi indekse kopyalamaz.
Okunabilir eski indeksler desteklenir; kaynak/WAL/manifest ve0.1 fixture aynı.
Dört normal çiftte yazma+bakım28,559→25,744s (-%9,86), strict open
259,443→169,180ms (-%34,79), canlı idx353,7→81,9MB, retained core
1,521GB→972,7MB. Yeni38-case16 düşük/22 yüksek; akışlı write private
+%1,42, query private düşük. Arama ve genel RAM kapıları açık kalır.

Önceki prototipin tarihsel sonucu:
[İkili sözlük ve kaynak referansı deneyi](docs/COMPACT-INDEX-10K-20261017.md):
aynı10k/4 dengeli çiftte strict open260→170ms (-%34,62;4/4), canlı indeks
353,7→81,9MB;267,8MB belge kopyası çıktı. Query private/WS artmadı.
Son geniş38-case32 düşük/5 yüksek/1 eşit, yoğun8 sayfa5/3; genel kapı açık.
Yeni SEG05 isteğe bağlı; normal yazıcı SEG04, fixture/kaynak/protokol aynı.
Sabit slot eski okuyucuyla SEG04'e döndürüldü,133 dosyanın byte'ı başlangıçla
aynı. Yazma/bakım, normal Vec RAM'i, eski nesiller ve concurrent/cold açık.

[İndeks CRC okuması ve ilk kullanım ölçümü](docs/INDEX-OPEN-10K-20261017.md):
indekse özel en fazla1MiB exact-read checksum yolu; kaynak/WAL ve sözlük
algoritmaları aynı. İki ayrı dört-çift seride strict open+ilk get+search
-%4,20/-%6,18 (8/8 hızlı); yalnız open kazancı%1–8 arasında değişti.
Yoğun8 sayfanın p95'i düşük, ayrı query/get tepesinde artış yok. Sözlük
prototipi arama artışı nedeniyle geri alındı; genel kapılar hâlâ açık.
Son instrumented profilde CRC83,8→78,3ms, toplam indeks156→158ms;
indeks hazırlığı çözülmüş sayılmaz.

[Tam doğrulamalı açılış iyileştirmesi](docs/STRICT-OPEN-10K-20261017.md):
aynı mevcut10k/dört dengeli çiftte normal strict open430→270ms (%37 düşük),
açılış+ilk get+arama424→282ms. Kaynak CRC/schema/version kontrolleri tam,
bekleyen doğrulama0; sağlam indeks varsa sahipli belge replay'i gerekmiyor.
Genel arama gerilemesi, normal Vec RAM'i, disk ve concurrent/cold işleri açık.

[Varsayılan FDB / SQLite güncel10k karşılaştırması](docs/SQLITE-DEFAULT-COMPARISON-10K-20261016.md):
akışlı yazma+bakım23,60/34,51s,38/38 aramada p95 FDB düşük (her biri4/4 çift).
Query/get peak private9,19/12,74MiB; normal Vec78,91/8,40MiB,
strict açılış434/1,53ms ve disk1,52GB/584MB. Aynı mevcut corpus/sabit iki
slot; motor değişmedi. FTS5/LIKE, sıcak-cache ve faz sınırları rapordadır.

Güncel varsayılan profil **`bounded-engine`**: normal derleme ölçülmüş
sayfalı indeks/arama ve akışlı yazma birleşimini içerir. Özel deneysel
feature seçmeden `Database::commit_streamed` kullanılabilir; mevcut
`commit(Vec<Mutation>)` limitleri ve davranışı korunur. Yaklaşık9MiB
akışlı ölçüm normal Vec API'sinin toplam RAM garantisi değildir.
[Kullanım, sınırlar ve eski feature adları](docs/DEFAULT-PROFILE.md).
[Varsayılana taşıma ölçümleri ve açık sınırlar](docs/DEFAULT-PROFILE-VALIDATION-10K-20261016.md):
151/92/170 test, format/hash eşitliği ve gerçek CLI kontrolü geçti.
Alt tarihli raporlardaki default seçimi o ölçümün tarihsel durumudur.

## Önceki ölçümlerin günlüğü

[Kısa bloklu arama kontrolü](docs/INTERLEAVED-SEARCH-10K-20261016.md):
aynı mevcut10k/dört süreç çifti×8 blok×25 tur; iki farklı-sürüm serisinde
8/8 sayfada yeni p95 düşük. Ishmael tek-sonuç -%6,51 / -%3,05,
quixote sayfası -%4,39 / -%2,45; tipik blok farkları daha küçük.
Aynı-kod kontrolünde tek-sonuç the +%6,44/4 çift yüksek olabilir.
Bu kısa oturum düzeninde gerileme tekrarlanmadı; motor bu tur değişmedi,
önceki kesintisiz oturum/yerleşim payı ve genel/default kapısı hâlâ açık.

[Yazma sonrası arama farkının teşhisi](docs/SEARCH-ATTRIBUTION-10K-20261016.md):
sekiz sayfada aday/read/CRC/doğrulama sayıları birebir aynı; yazma gram
çıkarıcısı sorguda0 çağrı. Aynı program/aynı CPU negatif kontrolünde
ishmael p95 +%4,23; farklı-sürüm yeni testte +%3,45, quixote +%9,64.
Ortam oynaklığı kanıtlandı; kalan kod/yerleşim payı ayrılmadı, farklar
kapatılmadı. Motor araması değişmedi; normal build ve default seçimi korunur.

[Uzun gövde indekslemesinde yazma kazancı](docs/GRAM-BODY-WRITES-10K-20261016.md):
aynı mevcut10k/dört dengeli çiftte yazma+bakım31,908→28,241s (-%11,49),
4/4 hızlı; CPU -%11,86. Geçici8KiB tam-anahtar tablosu ve63-bit scalar
anahtarlar yalnız BodyCache'te, query grams aynı; yeni format/feature yok.
Sekiz koşuda122 source/index dosyası aynı. Private commit +88KiB/WS+28KiB
medyan; sıfır RAM artışı kapısı geçmedi. Son read matrix33 düşük/5 yüksek,
800 örnekli sayfalar1 düşük/7 yüksek; ishmael tek-sonuç +%4,87/4 çift yüksek.
kalan arama, bakım/payload tekrarları, RAM/disk ve concurrent/cold açık.

[Ham metinden sınırlı eşleşme kanıtı](docs/RAW-TEXT-PROOF-10K-20261016.md):
aynı10k/dört dengeli çift/800 örnekli sayfada quixote p95 %10–15,
ishmael %4–7 düşük; dördü4/4 hızlı. Geniş matriste quixote whale %23
düşük/4/4; genel21 düşük/17 yüksek, diğer artışlar açık.56 adayın815 body
fold/scan'i0 olur;56 read/CRC/yapı kontrolü korunur.64KiB/başarı oranı
sınırı düşük isabetli ham taramayı durdurur; yeni büyük cache/format/feature
yok, builder-budget/default seçimi aynı. Genel RAM/hız kapısı geçmedi;
yazma/bakım, RAM/disk ve concurrent/cold açık. Koşullar/sınırlar raporda.

[Aynı sürüm kontrolü ve dar okuma denemesi](docs/BENCHMARK-CONTROL-10K-20261015.md):
aynı executable ile üç çiftte8/8 sayfanın pooled p95'i yükseldi; bu kod
regresyonu değildir. Runner artık ilk/ikinci sırayı eşit kullanan dört çift
çalıştırır, ham örnek/p95'i denetler ve çift bazındaki farkları da gösterir.
Dengeli800-örnek A/A'da aynı kod3 düşük/5 yüksek; sıfır fark garantisi yok.
Dar seed'e pencere açma denemesi38-case9/29 ve hedefte56→56 read sonucu
ile reddedildi/geri alındı. Source lease korunur; açık artışlar kapatılmaz.
Bu adım yeni engine hız/RAM/yazma kazancı iddiası değildir.

[Değişmez kaynak ömrüyle küçük sayfa maliyeti](docs/SOURCE-LEASE-10K-20261015.md):
aynı10k/üç çift/600 örnekli testte tek-sonuç p95 0,8123→0,2459ms
(-%69,73),3/3 hızlı; ileri sayfalarda -%18–20. Windows builder-budget
profilinde doğrulanacak kaynak aynı read-only handle ile tutulur;
yazma/silme/rename engellenir. Beş source path denetimi kaldırıldı, index
denetimi ve ilk10k kaynak doğrulaması sürer. Geniş38-case14 düşük/24 yüksek;
genel kapı geçmedi/default kapalı. Yazma/bakım, RAM/disk ve diğer açık
işlerin ölçüm/sınırları raporda;0.1 fixture/format/bağımlılıklar aynı.

[Küçük sayfanın dosya denetimi maliyeti](docs/PAGE-GUARD-10K-20261015.md):
aynı10k üzerinde600 örnekli sayfa ve38-case üçer çift ölçüm. Posting öneki
ve Windows metadata prototipleri hedefte güvenilir kazanç sağlamadı; ikisi
geri alındı, ek Windows bağımlılığı kaldırıldı. Ayrı warm instrumented profilde
tek-sonuç sorgusu0,347ms, source/index guard0,278ms (~%80); gövde fold/scan0.
Önceki+%66,89 artışın nedeni kapatılmadı. Önceki normalizer korunur; bu turda
yeni RAM/write ölçümü veya DB reset yoktu. Dosya sahipliği üzerinden sonraki
ölçülmüş adım yukarıdaki raporda; bu eski teşhis ayrı korunur.

[Normalleştirmede değişmeyen Unicode aralıkları](docs/CHANGED-RUNS-10K-20261015.md):
aynı mevcut10k/üç çiftte38 senaryonun35'inde p95 düşük,3'ünde yüksek.
`zx`309→222ms, `the zx`307→236ms, `quixote whale`5,83→5,00ms;
üçü de3/3 hızlı. Yeni cache/format/feature yok; mevcut builder-budget içinde.
Üç çift RAM medyanı indexing private9,13→9,18MiB, bakım7,54→8,48MiB,
WS12,84→13,21MiB; disk aynı. Tek-sonuç sayfası0/3 hızlı. Genel RAM/hız
kapısı geçmedi; default'a geçiş reddedildi, kalan işler açık.

[Chunk kopyasını azaltma — reddedildi](docs/CHUNK-COPY-REJECTED-10K-20261015.md):
aynı mevcut10k üzerinde iki prototip, her biri üç paired proses. Doğrudan
overlap fold32 düşük/6 yüksek; yalnız tek residual varyant21/17. İki parçalı
quixote whale her iki batch'te0/3 hızlı. İki runtime değişikliği geri alındı;
önceki wide-fold yolu/default/format korunur. Yeni corpus veya DB yazması yok.

[ASCII aralıklarını daha geniş kontrol etme](docs/WIDE-FOLD-10K-20261014.md):
aynı 10k/üç çiftte `zx` p95 336→309ms, `the zx`337→317ms, ikisi3/3 hızlı;
genel matrix33 düşük/5 yüksek. Mevcut builder-budget içinde32-bayt güvenli
kontrol; Unicode/kalıcı format/default aynı. Tek RAM teyidi indexing private
9,14MiB/WS13,34MiB; bakım+0,66MiB ve toplam32,36→33,47s. Disk aynı.
Genel hız/RAM kapısı geçmedi; kalan arama ve yazma/bakım işleri açık.

[İki karakterli olumlu aramada indeks kanıtı](docs/SHORT-COVERAGE-10K-20261014.md):
mevcut gramın bütün canlı kayıtları kaplamasından genel alt-parça kanıtı,
64 entry/16 KiB sınırı/run; kalıcı format ve default seçimi aynı. Aynı 10k,
üç çiftte `he` p95 132,27→1,81ms (%98,63 düşük,3/3 hızlı). İndeksleme private
9,09MiB, disk aynı; WS +0,30MiB. Yazma+bakım 32,87→32,25s (3/3 düşük).
Genel matrix 15 düşük/23 yüksek; uzun ve olumsuz tarama/diğer p95 işleri açık.

[İndeks kanıtıyla payload tekrarını azaltma](docs/COVERED-TEXT-10K-20261014.md):
genel üç karakterli parçalar için source kontrolünden sonra covering yolu.
Son üç paired10k koşuda `the`145→2,30ms, `man`147→2,63ms; ileri sayfalar
146–151→2,94–3,28ms. Son tek RAM teyidi private9,19MiB/WS13,32MiB; disk
aynı. Bütün sorgular hızlanmadı (matrix15/23, aktif-data17/1); iki karakterli
ve uzun parça taraması/yazma-bakım açık, default feature seçimi aynı.

[Düşük RAM profilinde hız — ilk adım](docs/BOUNDED-SPEED-10K-20261014.md):
RAM önceliği bu10k iş yükü için kullanıcı tarafından kabul edildi. Yeni üç çiftte
indexing private9,07→9,09MiB, working set12,93→13,00MiB; disk aynı, bakım
private+0,46MiB. Geniş arama36/38, aktif-data15/18 p95 düşük; iki ileri sayfa
ve iki dar sorgu state'i geriledi.
Uzun payload'ı yeniden okuma ve genel hız hedefi açık; default seçimi aynı.

[Giriş/delta/bakım bütçesi ve sayfalı sözlük](docs/BOUNDED-MEMORY-10K-20261014.md):
opt-in `commit_streamed` yolunda aynı10k/üç paired koşu indexing111→9,13MiB,
bakım100→7,94MiB,idle21,5→1,74MiB. SQLite yeni referans indexing8,40MiB.
Normal yola göre toplam27,24→31,54s yavaş;38-case17/21 ve1000 belge write-after
arama0/18 hız kapısı geçmedi. Default'a alınması reddedildi; feature kapalı,
normal API korunur. Kalıcı codec ve0.1 kaynak aynı, yeni corpus yok.

[Tam belge kopyasını azaltma deneyi](docs/COPY-REDUCTION-10K-20261013.md):
WAL referansı indexing111→84MiB,bakım102→76MiB yaptı; delta aramada17/18
p95 arttı. Hız şartını geçmedi, existing WAL-delta default kapalı kaldı.
Tek-frame encode sadeleştirildi; normal yol ve kalıcı codec aynı.

[Mevcut bütçeli yol ölçümü](docs/BUDGET-MEASURE-10K-20261013.md): bakım
private99,86→96,07MiB, toplam yazma+bakım35,17→38,59s; üç çiftte yavaş.
Kombinasyon reddedildi, default kapalı. Pool sayaçları toplam RAM sınırı değil;
delta/body/caller/metadata belleği hâlâ kapsam dışında.

[Delta RAM sözlüğü](docs/DELTA-DICTIONARY-10K-20261013.md): scope/parça
kopyaları azaltıldı. Aynı10k/üç çiftte toplam42,995→36,803s,3/3 hızlı;
indexing private111,75→110,36MiB, bakım100,54→97,58MiB. Bellek kazancı küçük.
1000 belge delta araması17/18 p95 düşük; tek~14µs artış ve sınırlar raporda.

[İndeksleme bellek tepesi](docs/RAM-LIFETIME-10K-20261013.md): WAL tamponunun
yaşam süresi kısaltıldı. Aynı10k/üç çiftte indexing private124→112MiB,
toplam yazma+bakım36,70→34,70s; üç çiftte hızlı ve RAM düşük. Bakım RAM'i
düşmedi; genel RAM/disk/eşzamanlı yük işleri açık. Akışlı source deneyi reddedildi.

[Güncel yazma/bakım karşılaştırması](docs/WRITE-MAINTENANCE-10K-20261013.md):
aynı ilk10k belgede üç çift; FDB medyan37,73s / SQLite46,41s, FDB2/3 çift
hızlı. Bakım SQLite hızlı; FDB RAM/disk farkı sürüyor. Yeni corpus yok.

[11 arama artışının yeni ölçümü](docs/REGRESSION-READS-10K-20261013.md): aynı
özelliklerle üç paired process'te önceki11 senaryonun tümünde pooled p95
düşük (%4–18); sekizi3/3 hızlı. Genel matris36 düşüş/2 sayfa artışı.
Normalleştirme hızlandırıldı, seyrek read-ahead kullanımına göre durduruluyor.
Pencere default kapalı; ham sonuçlar raporda korunur.
Varsayılan yol ayrı ölçüldü:35 düşüş/3 artış; geologist%6 ve boş quixote
sayfası%18 hızlı. Kalan artışlar2026-10-13 kullanıcı kararıyla ölçüm
dalgalanması kabul edilerek kapatıldı; ek istatistiksel kanıt iddiası yok.

[Payload okuma penceresi deneyi](docs/WINDOW-READS-10K-20261013.md): geniş warm
sorgularda%20–22 kazanç, yaklaşık10k→1,1k payload read request; ancak38-case
matriste27 p95 düşüş/11 artış, genel kapı geçmedi. Mevcut grouped-read
experiment'inde default kapalı; normal exact read yolu aynı. Aynı corpus ve
DB hash'leri korundu; SQLite dört-query tek referans ve RAM sınırları raporda.

[Safe UTF-8 hızlandırması](docs/UTF8-READS-10K-20261013.md): aynı10k DB,
ilk search208→173 ms (%16,46), dört warm query p95%18–27 düşük,12/12 paired
karşılaştırma hızlı. Gözlenen first-use proses WS~30 MiB; DB hash'leri aynı.
Tampon prototipleri reddedildi/geri alındı. Strict açılış sonuçları karışık;
genel açılış/RAM/yazma-bakım kapıları açık.

[IEEE checksum hızlandırması](docs/CRC-READS-10K-20261013.md): aynı10k DB,
beş çiftte ilk search381→229 ms (%39,94), tam strict open962→582 ms (%39,51).
Dört warm query p95 üçer eşleşmenin tamamında düşük (%26–41). CRC değerleri /
formatlar aynı;38-case içerik kontrolü geçti, DB/cache hash'leri aynı.
RAM ve güncel SQLite yazma/bakım ölçümü açık.

[İlk aramada borrowed kaynak doğrulaması](docs/BORROWED-SOURCE-10K-20261013.md):
kontrol amaçlı bütün belgeleri kopyalamak kaldırıldı; kaynak CRC/grammar/limit
kontrolleri korunur. Aynı DB'de beş çift: ilk search529→336 ms (%36,44),
open+get+search602→407 ms (%32,44). Warm p95 iki sorguda küçük artış gösterdi;
record-read API opt-in, strict açılış aynı, DB/cache dosyaları değişmedi.

[Kaynak record-get prototipi](docs/SOURCE-RECORD-READS-10K-20261013.md): ilk
get107→0,20 ms, open+get183→74 ms; gerçek `.fdb` kaydı block/record CRC ile
okunur. Warm search p95 %1–4 arttı, genel regresyon kapısı geçmedi; opt-in.
Ek REF/SVFY metadata164.480 B, eski dosyalar aynı, RAM ölçülmedi.

[İndeks blok kontrolü](docs/BLOCK-OPEN-10K-20261012.md): isteğe bağlı VFY01
cache ile deferred open298→60 ms, open+ilk get383→150 ms. Ek cache5.580 B;
kaynak/index byte'ları aynı. İlk get/search doğrulama maliyeti korunur; sıcak
query sonuçları karışık, default strict aynı. SQLite9 ms hedefine ulaşılmadı.

[Opt-in ertelenmiş açılış](docs/DEFERRED-OPEN-10K-20261012.md): beş çiftte
strict752 /deferred319 ms; ilk get92 /ilk search360 ms doğrulama maliyeti
dahildir. `open_deferred` + `check_integrity`, default strict korunur; SQLite9 ms
hedefine ulaşılmadı. Tam kaynak doğrulaması veri kullanımından önce yapılır.

[10k açılış iyileştirmesi](docs/OPEN-SOURCE-10K-20261012.md): ayrı source CRC
tarama geçişi kaldırıldı; beş çiftte medyan946→747 ms (~%21). Tam kaynak
checksum/record kontrolleri korunur; DB hash'leri aynı. Streaming yolu değişmedi.

[Açık işler ve sonraki genel ölçüm](docs/OPEN-WORK.md): açılış, güncel yazma/bakım,
arama regresyonları, RAM/disk ve bakım sırasında okuma/yazma takibi.

[10k aday profili ve dönüşüm optimizasyonu](docs/SELECTIVITY-READS-10K-20261012.md):
beagle/AND/uzun negatif/zx her üç çiftte hızlı; pooled p95 %16–22 azaldı.
38 senaryonun34'ünde p95 düşük, dört artış raporlandı; genel sıfır-regresyon
iddiası yok. Aynı DB/corpus, SQLite dahil; tüm dosya hash'leri aynı, RAM ölçülmedi.

[Genişletilmiş 10k okuma matrisi](docs/READ-MATRIX-10K-20261012.md): 30 query
girişi + 8 sayfa, beşer proses, rastgele get/4 boyut grubu ve negatif kayıtlar.
Bu SQLite düzeninde tüm senaryoların p95'i FDB lehine; fakat 46 sonuçlu
beagle61,27 ms, 65 sonuçlu AND110,73 ms, negatif kısa sorgu~711 ms.
Az sonuç hızlı arama garantisi değil; genel4×/production kabul iddiası yok.
DB hash'leri aynı, yeni corpus/DB yok; negatif scan regresyonu kesinleştirilmedi.

[10k contains tarama iyileştirmesi](docs/CONTAINS-CHUNK-SCAN-10K-20261012.md):
tam CRC/biçim doğrulaması ve kesin toplam korunarak uzun gövde dönüşümü
eşleşmede durur. Aynı oturumda geniş sorgu p95 ~709–726 → 370–384 ms
(%46–48 düşüş); SQLite~1.499–1.637 ms. Dar sorgu10,36 → 8,02 ms,
SQLite22,33 ms. Get aynı, mevcut DB hash'leri aynı; RAM yeniden ölçülmedi.

[Mevcut 10k veride okuma/contains](docs/CONTENT-READS-10K-20261012.md): üçer
proses, sıcak cache; dar sorgu p95 FDB10,63 / SQLite19,70 ms, tek belge
0,074 / 0,173 ms. Geniş sorgu p95 FDB~771–773 / SQLite~1.343–1.421 ms;
FDB hızlı ama geniş sorgular hâlâ pahalı. Yeni corpus/DB veya RAM ölçümü yok.

[10k delta/flush optimizasyonu](docs/DELTA-FLUSH-10K-20261012.md): flush RAM
indeksini yeniden kullanır; mevcut bitmap ile iki eşleşmede toplam medyan
62,87 → 29,75 s (%52,68). SQLite34,44 s; bu kurulumda iki eşleşmede FDB hızlı.
Gövde taraması20k → 10k, builder terim üretimi sıfır. Feature'lar default kapalı;
o yazma ölçümünde arama gecikmesi ölçülmedi; yukarıdaki ayrı okuma raporu
mevcut DB'leri kullanır. Disk aynı, idle private0,49 MiB arttı.

[10k bağımsız merge optimizasyonu](docs/MERGE-REUSE-10K-20261012.md): aynı
belgeler ve yeniden kullanılan DB alanıyla iki eşleşmede yazma+bakım
112,03 → 89,04 s (%20,52), bakım 24,85 → 4,55 s (%81,71) azaldı.
Builder terim çıkarımı 20k → 10k; SQLite toplamı 52,86 s, hâlâ daha hızlı.
Feature default kapalı; arama p95 ölçülmedi. Sonraki darboğaz delta/flush inşası.

Test verisi politikası: mevcut corpus'un aynı ilk **10.000 belgesi** kullanılır;
yeni corpus üretilmez, testler başka diske taşınmaz. Aktif ölçümler yalnız
`data/benchmark-work/fdb` ve `data/benchmark-work/sqlite` alanlarını yeniden
kullanır; koşu başına veritabanı kopyası saklanmaz. Okuma/arama için mevcut
indeks kullanılır. [Temizlik ve çalışma düzeni](docs/BENCHMARK-STORAGE.md).

[10k hedefli optimizasyon ve tekrar ölçümü](docs/CONTENT-OPTIMIZATION-10K-20261012.md):
default çerçeve tamponu önceden ayrılır; private tepe yaklaşık %4,4 azaldı,
belirgin hız kazancı yok. Mevcut opt-in bitmap yoluyla yazma+bakım %15,3,
tepe private %26,2 azaldı; bitmap default kapalı, bakım/idle hedefleri geçilmedi.

[İçerik indeksi için ilk 10k bellek ölçümü](docs/CONTENT-MEMORY-10K-20261011.md):
geliştirme testleri kullanıcı kararıyla10bin belgeye sınırlı. Gerçek çeşitli
uzun metin, streamed input, ayrı idle/indexing pencereleri. FDB boşta private
21,41MiB, indeksleme tepesi184,75MiB; SQLite1,31 /8,46MiB. Bunlar default
motor başlangıç değerleridir; `derived` profil henüz uygulanmadı. Bu ölçekte
iyi sonuç elde edilmeden büyük koşuya dönülmeyecek.

Yeni veritabanlarının belge payload'ı artık doğrudan **ikili OFF1** formatıyla
yazılır: kaynak `FDBLOG04/TXN2`, türetilmiş indeks `FDBSEG04`. WAL, flush ve
merge aynı codec'i kullanır; contains adayları tam belge çözmeden doğrulanır.
Ek benchmark payload dosyası gerekmez. Manifest/directory/protokol JSON'u ve
source/index içindeki mevcut payload kopyalama düzeni korunur. RAM/disk
optimizasyonu ayrı aşamadır.
[Kalıcı format ve doğrulama](docs/NATIVE-BINARY-VALIDATION-20261011.md).

[Yeni genel performans testi](docs/NATIVE-GENERAL-PERFORMANCE-20261011.md):
iki 100k corpus, 12 process, default feature'lar, aynı koddan JSON kontrolü.
Uzun contains 27/27 eşleşmiş sayfa/tur hızlı; yaygın %25–45, iki karakter
%27–44, AND %15–21, dar %6–28; açılış %29–35, 100 uzun belge get %26–50.
Uzun disk 6,293→6,183 GB; working set ≈130 MiB kaldı. Kısa yazma+bakım
medyanı 17,51→15,98 s, fakat bir çift %23,53 yavaş: tekrarlanabilir yazma
kazancı yok. Kısa RAM arttı; küçük exact/range/get sonuçları karışık.
Tek-kopya/source-offset yerleşimi henüz ölçülmedi.

[Yeni ikili FDB / SQLite genel karşılaştırması](docs/NATIVE-SQLITE-GENERAL-20261011.md):
iki 100k corpus, 12 yeni process, 978 doğru search. Uzun yaygın sorgular
5,51–7,85, iki karakter 5,10–6,92, AND 3,31–5,22 kat süre oranıyla FDB lehine;
dar sorgu karışık. Kısa yazma+bakım FDB15,47 / SQLite8,46s: SQLite üç çiftte
hızlı. Uzun tepe WS FDB130,24 / SQLite31,61MiB; disk6,183 /5,807GB.
Sonuç kullanılan SQL/schema/cache düzenine aittir; açılış doğrulamaları farklıdır.

Aşağıdaki önceki performans raporları JSON motoru ve geçici payload deneylerinin
tarihsel sonuçlarıdır; yeni kalıcı format için hız/RAM/disk sonucu sayılmaz.

Güncel öncelik: **okuma, yazma ve içerir aramasında sonuç getirme hızı**.
Hız kazanırken disk ve RAM'de gerileme olmaması hedeflenir. [Yeni hız karşılaştırması ve uygulama sırası](docs/SPEED-REASSESSMENT-20261009.md).

[İkili payload'ın gerçek motor ölçümü](docs/ENGINE-BINARY-VALIDATION-20261011.md):
100k uzun pasaj, 9 proses, 279 gerçek `Database::search`; bütün tam sayfa
oracle'ları eşleşti. Native JSON'a göre yaygın kelime %22–27, iki karakter
%25–27, AND %14–19 hızlı; dar ilk sayfa bir turda %3 yavaş. Tepe working
set ≈130 MiB kaldı, sampled private medyanı 113,55→132,25 MiB yükseldi.
Kaynak/SEG03 korunup ek export bağlandığı için disk 6,29→8,98 GB ve hazır
olma 19,64→34,46 s oldu. Bu geçici motor deneyi default için **reddedildi**;
kalıcı format değiştirilmedi. Codec-only kapısı ayrı kapsamdır.

[Kendi ikili offset payload prototipi](docs/BINARY-PAYLOAD-VALIDATION-20261011.md):
aynı 100k uzun pasaj, aynı reusable fold ve üç native proses çifti. Payload
contains kapısı geçti: yaygın kelime %28–31, iki karakter %24–29, AND %25–36,
dar kelime %3–18 kısa sürede tamamlandı. Disk yaklaşık %1,97, proses working
set medyanı 16,51→12,41 MiB azaldı. Default kapalı, kalıcı motor formatına
uygulanmadı; bu RAM değerleri gerçek FDB/SQLite motor ölçümü değildir.

[Yeni uzun metin SQLite karşılaştırması](docs/LONG-SQLITE-COMPARISON-20261011.md):
aynı 100k gerçek pasaj, üç yeni native proses çifti, tam toplam + sıralı 50 tam
belge. Bu SQLite SQL/cache/LOB düzeninde FDB yaygın kelimede 4,3–6,2,
iki karakterde 3,9–5,1, AND'de 3,2–3,9 kat kısa sürede tamamladı; dar sorguda
kazanan koşuya göre değişti. Working set medyanı FDB 130,22 / SQLite 28,38 MiB;
disk FDB 6,29 / SQLite 5,81 GB. Açılış doğrulaması ve alan/link indeks
kapsamı farklıdır; genel SQLite üstünlüğü iddiası değildir. Yazma ölçülmedi.

[Gerçek motor sorgusunda ASCII ve tampon paylaşımı](docs/ENGINE-FOLD-VALIDATION-20261011.md):
100k gerçek uzun metin, aynı `Database::search`, üç eşleşmiş proses çifti.
Ölçülen dokuz sayfanın hepsi her çiftte hızlandı: yaygın kelime %10–18,
dar kelime %11–16, iki karakter %27–33, AND %26–38. Working set medyanı
130,25→129,61 MiB; dosya formatı ve boyutları değişmedi. Optimizasyon default
disk sorgu yolunda. Açılış kazancı gösterilmedi; soğuk cache, çok segment/delta
ve SQLite üstünlüğü bu ölçümün kapsamında değildir.

[rkyv payload prototipi ve kabul ölçümü](docs/RKYV-PAYLOAD-VALIDATION.md):
`experimental-rkyv-payload` bağımsız codec/benchmark ve açık geçici motor bağlama yolunu açar, default
kapalıdır. Bu codec deneyi mevcut JSON kaynak ve motor yollarını değiştirmedi; uzun metin corpus'unda
hız, disk ve toplam proses working set birlikte değerlendirildi. Disk/RAM
iyileşti, tekrarlanabilir contains hız kapısı geçmedi: deney **reddedildi**.

Normalleştirme ve alt dize taramasını ayıran sonraki deney:
[ASCII yolu ve tekrar kullanılan tampon ölçümü](docs/FOLD-VALIDATION-20261011.md).
Üç çiftte uzun sorgular JSON'da %38–42, rkyv'de %45–49 hızlandı. Bu karşılaştırma
yalnız payload benchmark'ındadır. Ardından yukarıdaki gerçek motor ölçümünde
normalleştirme optimizasyonu disk sorgu yoluna taşındı; JSON codec korundu.

[WAL commit senkronlaması ölçüldü](docs/WAL-SYNC-VALIDATION.md): tek kayıtlık
işlemlerde büyük fark,100k/batch500 bakım dahil toplamda küçük fark çıktı.
`experimental-wal-sync-control` ile yalnız explicit Rust API çağrısı commit sync'i
kapatır; bütün build'ler varsayılan sync açık başlar. WAL yazımı devam eder.

En son: [posting oluşturucu bütçesi ve kesilme doğrulaması](docs/BUILDER-BUDGET-VALIDATION.md).
100k sıcak-cache üç çiftte tepe proses working set18.65→17.34MiB,
yazma+bakım10.91→10.81s. Küçük süre farkı kesin hız artışı değildir;
yeniden açılış/bazı sorgu kapıları ve SQLite hedefi tamamlanmadı. Bütçe default kapalıdır.

Son deney: `experimental-shared-documents` delta belgelerinde tekrarlı gövde ve
küçük skaler alan kümelerini RAM'de paylaşır. Delta sorguları eşleşen anahtarların
tam kopyaları yerine mevcut indeks anahtarlarını referansla tutar. Kalıcı format
değişmez. [Ölçüm, doğruluk ve kalan hedefler](docs/SHARED-DOCUMENTS-VALIDATION.md).
Sonraki anahtar yerleşimi aynı anahtarı docs/indeks/shadow arasında paylaşır;
belge lookup standart Rust HashMap kullanır, flush/sorgu sıralaması korunur.
[Anahtar paylaşımı ölçümleri](docs/SHARED-KEYS-VALIDATION.md).

## Motor ve kullanım

SDB ve Fundops verilerini, bağlantılı kayıtları ve dosyalardan çıkarılmış metinleri
saklamak ve aramak için Rust ile yazılan özel veritabanı motoru. **SQLite,
RocksDB, LMDB, sled, Tantivy veya başka bir veritabanı/arama motoru kullanılmaz.**
Depolama biçimi, işlem günlüğü, kurtarma, indeksler ve checkpoint mekanizması bu
repository'de uygulanır. `serde` ve `serde_json` yalnızca veri serileştirme içindir.

Bu sürüm çalıştırılabilir **0.3.1 çekirdeğidir**. Ürün kapsamı bağımsız DB motorudur;
SDB ve Fundops yalnız veri modeli ihtiyaçlarına referanstır. Rust API, yerel
JSON Lines süreci ve örnek istemci/aktarım yardımcıları hazırdır.

Güncel ölçüm ve hata testleri: [0.3.1 doğrulama sonuçları](docs/VALIDATION-0.3.1.md).
Bir milyon kayıt için ayrı benchmark: [SQLite karşılaştırması](docs/SQLITE-COMPARISON.md).
SQLite yalnız karşılaştırma aracında kullanılır; FDB motorunun bağımlılığı değildir.

## Önceki geliştirme aşamalarının teknik notları

Bu bölüm ilk prototiplerin kararlarını ve sınırlarını korur. Güncel varsayılan
bileşen seçimi [DEFAULT-PROFILE](docs/DEFAULT-PROFILE.md) içindedir; aşağıdaki
eski default-kapalı ifadeleri bugünkü derleme ayarı değildir.

Bellek planının geliştirme aşaması başladı: [aşama 1 doğrulaması](docs/MEMORY-STAGE1-VALIDATION.md)
ve [uygulama planı](docs/MEMORY-PERFORMANCE-PLAN.md). İlk sayfa sorguları artık
yalnız sınırlı sayıda eşleşme ID'si saklar; tam sonuç sayısı korunur. Anahtar/konum
tablosu hâlâ RAM'dedir; SQLite'tan düşük toplam proses belleği hedefi tamamlanmadı.

Sayfalı kayıt tablosu için [ilk prototip ve ölçüm](docs/PAGED-RECORDS-PROTOTYPE.md)
hazırdır. Bellek yerleşimi küçüldü; lookup hız kapısı henüz geçilmediği için
üretim motoruna alınmadı.

[Doğrudan kayıt konumundan okuma](docs/DIRECT-SLOT-VALIDATION.md) deneysel Cargo
feature'ı olarak hazır. Aynı proses karşılaştırmasında ilk/ikinci sayfada yaklaşık
%10 kazanç görüldü; tüm hız/bellek kapıları tamamlanana kadar default sorgu yolu korunur.

[Gruplu payload okuması](docs/GROUPED-READ-VALIDATION.md) deneysel olarak eklendi.
Yakın kayıtlar sınırlı bir okuma penceresinde alınır; ilk/ikinci sayfa ölçümünde
yaklaşık %31 kazanç görüldü. Kayıt tablosu hâlâ RAM'dedir.

`experimental-paged-metadata` ile kayıt/ID tablolarını diske taşıyan deneysel
motor yolu eklendi: [uygulama ve ölçüm](docs/PAGED-METADATA-VALIDATION.md).
Sıkıştırılmış kayıt sayfaları, iki seviyeli dizin ve ortak 256 KiB sayfa cache'i
kullanır. Default build kayıt tablosunu RAM'de tutmaya devam eder; deneysel
yolun tüm hız ve toplam proses RAM kapıları henüz tamamlanmadı.

`experimental-streaming-open` ayrıca checkpoint kaynaklarını ve SEG03 dizinlerini
kayıt bazında çözer. Açılış tepe belleği, sorgu/yazma ölçümleri ve kalan sınırlar:
[akışlı açılış doğrulaması](docs/STREAMING-MEMORY-VALIDATION.md). Bu yol da
deneyseldir; SQLite'tan düşük toplam proses belleği hedefi tamamlanmadı.

Karşılaştırmanın aynı sonuç sözleşmesini doğrulayan ve yazma/sorgu maliyetlerini
ayıran sonraki aşama: [iş yükü ve maliyet doğrulaması](docs/WORKLOAD-COST-VALIDATION.md).
`diagnostic-timing` yalnız ayrı teşhis derlemesinde süre sayaçlarını açar;
normal performans ölçümünde kapalı tutulur.

`experimental-write-pipeline`, flush/merge tekrarını azaltan ve builder kayıt
dizinini akışla üreten sonraki deneydir: [ilk yazma hattı ölçümü](docs/WRITE-PIPELINE-VALIDATION.md).
Bakım kilitleri hız maliyeti nedeniyle güncel umbrella'dan çıkarıldı; ayrıca
`experimental-write-maintenance-slots` ile açılabilir. Bu sınırlar delta dahil
toplam byte bütçesi değildir; deney normal build'de varsayılan olarak kapalıdır.
Parçalar ayrı Cargo feature'larıyla ölçülebilir: [bileşen karşılaştırması](docs/WRITE-COMPONENTS-VALIDATION.md).

`experimental-write-buffer-budget`, checkpoint kaynaklarını büyük batch/frame
Vec'leri olmadan akıtır ve kontrollü yazma tamponlarını ortak 256 KiB rezervasyona
alır: [uygulama ve ölçüm](docs/WRITE-BUFFER-BUDGET-VALIDATION.md). Delta dahil
toplam heap bütçesi henüz tamamlanmadı; bu feature da varsayılan olarak kapalıdır.

`experimental-shared-delta-text`, delta indeksinde aynı katlanmış içerikleri
paylaşır; başlıklar belgeye özgü kalır. Kontrollü body cache'e sığmayan metinler
kendi referans sayımlı içeriklerini taşır. Bu feature normal build'de kapalıdır.
[Ortak delta metni doğrulaması](docs/SHARED-DELTA-TEXT-VALIDATION.md).

`experimental-compact-delta-postings` delta eşleşmelerini u32 slot numaralarıyla
saklar. `experimental-delta-bitmaps` ayrıca küçük inline/sıralı listeler ile
bitmap arasında yoğunluğa göre geçer. Kalıcı format değişmez; normal build'de
kapalıdır. [Delta posting doğrulaması](docs/DELTA-POSTINGS-VALIDATION.md).

`experimental-streaming-metadata`, SEG03 kayıtlarını geçici tam tabloya almadan
MET04 sayfalarına akıtır veya mevcut sayfalarla karşılaştırır. Kalıcı biçim ve
kaynak doğrulaması korunur. [Metadata akışı doğrulaması](docs/STREAMING-METADATA-VALIDATION.md).
Açılış/sorgu belleği iyileşti; yazma dahil toplam tepe hedefi henüz çözülmedi.

## Neler yapıyor?

- `(scope, collection, id)` ile idempotent kayıt ekleme/güncelleme/silme.
- Toplu işlemin tamamını tek kalıcı çerçevede yazma; eksik işlem uygulanmaz.
- Sürüm denetimi: `missing` veya `version` ile eşzamanlı güncelleme çakışmasını yakalama.
- İndeksli alan eşitliği ve `i64` alanlarda dahilî alt/üst sınır filtreleri.
- Başlık ve içerikte Unicode trigram indeksiyle AND alt-dize araması.
- Türkçe `İ/ı/I`, `ş/s`, `ğ/g`, `ç/c`, `ö/o`, `ü/u` eşleştirmesi.
- Aynı scope içindeki kayıt bağlantıları ve ters bağlantı sorgusu.
- Toplam kayıt sayısı, offset/limit ile numaralı sayfalama için veri.
- Çökme sonrası günlükten yeniden açılma; checksum bozulmasında açık hata.
- Yalnız değişmiş kayıtları yeni segmente yazan flush ile etkin günlüğün küçültülmesi.
- Yazmalar sürerken sınırlı segment gruplarını arka planda birleştirme.
- Yaklaşık 4 MiB posting gruplarıyla geçici dosyalarda indeks yapımı.
- Checkpoint kayıtlarını ve u32 arama posting'lerini diskte saklama; içeriği gerektiğinde okuma.
- RAM'deki değişiklikler için ayarlanabilir otomatik checkpoint eşikleri.
- Kayıp/bozuk türetilmiş disk indeksini sağlam işlem günlüğünden yeniden kurma.
- İşletim sistemi dosya kilidiyle aynı klasör için tek motor sahibi.

## Çalıştırma

Rust 1.89 veya üzeri gerekir. Windows PowerShell:

```powershell
cargo test --all-targets
cargo build --release --bins --examples
cargo run --release --example linked_search -- data/example
```

`linked_search`, tamamen **örnek verilerle** bir fonu, raporu, dosya metadata'sını
ve çıkarılmış içeriği birbirine bağlar. Aramayı, tarih filtresini, checkpoint'i
ve yeniden açıldıktan sonraki sonucu doğrular. PDF açma/çıkarma işlemi yapmaz.

Tek isteği UTF-8 JSON dosyasından yürütmek:

```powershell
.\target\release\fdb.exe data/mydb examples/requests/commit.json
.\target\release\fdb.exe data/mydb examples/requests/search.json
```

Uzun yaşayan süreç:

```powershell
.\target\release\fdb.exe data/mydb --stdio
```

Her satır bir JSON isteği, her yanıt bir JSON satırıdır. Başarılı yanıt
`{"ok":true,"result":...}`, hatalı istek `{"ok":false,"error":"..."}` döndürür.
`commit`, `get`, `search`, `stats`, `flush`, `compact`, `poll_compaction`,
`wait_compaction` desteklenir. Ayrıntı:
[protokol](docs/PROTOCOL.md).

Python worker bağlantısını doğrulamak:

```powershell
python clients/demo.py target/release/fdb.exe data/python-demo
```

Rust'ta `Database::open`, `commit`, `get`, `search`, `compact` kullanılır.
Birden fazla thread için `Arc<Mutex<Database>>` ile paylaşılabilir. Ayrı süreçlerin
aynı klasörü eşzamanlı açması reddedilir; istemciler tek sahip sürece bağlanmalıdır.

## SDB veri modeli referansı

`adapters::file_entry`, hacim kimliği + dosya referansı + parent + ad ile hard
linkleri ve farklı hacimlerde aynı ID'leri birbirinden ayırır. Canlı bağlantıda
hacim harfi yerine SDB'nin `ScanOutput.serial` değeri, tam NTFS dosya referansı
ve USN cursor/journal kimliği kullanılmalıdır. Rename/move eski ad anahtarını
silip yeni anahtarı ekleyen atomik işlem olarak gönderilir. İçerik ayrı bir
`file_contents` kaydı olabilir; dosya kaydına `links` ile bağlanır.

Mevcut SDB TSV biçimi için salt okunur kaynak aktarımı:

```powershell
.\target\release\fdb-import-sdb.exe data/sdb-sample D:/Vibeprojects/sdb/data/c.tsv export-20261007-C shared 1000
```

Kaynak TSV'ye yazılmaz. TSV tam NTFS referansı, hacim seri numarası veya çözülmüş
dosya yolu taşımaz; aktarılmış kayıtlar `identity_kind=sdb_tsv_export_id` olarak
etiketlenir. Her bağımsız export için ayrı namespace kullanın. Bu aktarım canlı
USN takibi değildir. Her 500 kayıt ayrı atomik işlemdir; aktarım hatasında önceki
başarılı batch'ler kalır. Aynı namespace ve aynı kayıtlarla tekrar çalıştırma
aynı anahtarları günceller. Export'tan kalkmış kayıtlar otomatik silinmez.

## Fundops veri modeli referansı

`adapters::fundops_record` alanları kayıpsız JSON metadata olarak taşır,
`fund_code` bulunan raporları aynı scope'taki `funds` kaydına bağlar.
Fon kodu, rapor ID'si veya `(fon, gün, sağlayıcı)` doğal anahtarı açıkça verilir;
motor metinden kimlik tahmin etmez. Şimdiki bağlantı sözleşmesi:
[veri modeli ve mimari](docs/ARCHITECTURE.md).

## Disk segmentleri ve bellek

Yazmalar önce kalıcı işlem günlüğüne ve RAM delta'sına eklenir. `flush` yalnız
bu delta'yı `run-<hex>.fdb` kaynağına ve türetilmiş `.idx` indeksine yazar;
önceki segmentleri yeniden yazmaz. İçerikler ve u32 posting listeleri disktedir.
Anahtar/konum ve token sözlükleri RAM'de kalır. Yeni kayıtlar ve silme işaretleri
eski segmentlerdeki aynı anahtarı gölgeler; arama sonuçları anahtar sırasıyla birleşir.

Geliştirme prototipinde offset+limit <= 4096 için run başına en fazla offset+limit
kadar ID saklanır; ilk sayfada bu sayı limit kadardır.
Tam canlı kapsaması bilinen metinsiz filtrelerde count, tombstone'ları hariç tutan
görünürlük maskesinden hesaplanır. Büyük offset'ler hâlâ tam eşleşme listesini
kullanan önceki yolu izler; genel bir hard RAM sınırı yoktur.

Varsayılan 10.000 değişmiş anahtar veya 16 MiB serileştirilmiş delta eşiği,
bir sonraki geçerli yazma öncesinde flush tetikler. Tek batch bu eşiği aşabilir;
kesin RAM limiti değildir. Açılışta büyük WAL delta'sı da flush edilir.

```rust
let options = fdb::Options {
    max_delta_keys: 5_000,
    max_delta_bytes: 8 * 1024 * 1024,
};
let mut db = fdb::Database::open_with_options("data/mydb", options)?;
db.flush()?;
db.compact()?; // flush + en fazla bir arka plan birleştirme başlatır
db.wait_for_compaction()?; // başlatılmış işi bekler ve sonucunu yayımlar
```

**0.3'te `compact` arka plan işini beklemez.** Rust API'deki
`start_compaction()` delta'yı flush etmeden iş başlatır. `poll_compaction()`
biten işi beklemeden yayımlar; `wait_for_compaction()` yalnız mevcut işi bekler.
Tüm uygun grupları bitirmek için wait ardından start/wait döngüsü kullanılabilir.
JSON Lines'ta `compact` + `wait_compaction` aynı bakım akışını sağlar.

Bir iş art arda gelen, benzer boyutlu 2–4 segmenti seçer; toplam giriş en fazla
100.000 anahtar ve 64 MiB kaynak veridir. Dört veya daha çok segment olduğunda
otomatik birleştirme denenir. Büyük segmentler bu sınırın üstünde tekrar
birleştirilmez. Yazma sahibi yeni delta'yı işlerken worker kaynak/cache'i üretir;
sonucu yalnız sahip motor yeni `.ready` manifest ile yayımlar.
`Drop` çalışan işi bekler fakat sonucunu yayımlamaz; kullanılmayan dosyalar kalabilir.
Flush ve manifest yayımlama hâlâ eşzamanlı bakım maliyetidir.

İndeks yapımı posting girdilerini yaklaşık 4 MiB tahmini bütçede checksummed
geçici dosyalara döker ve sıralı akışla birleştirir. Bu bütçe toplam derleme RAM'i
olmaz: kayıt/token dizinleri, tek kaydın trigram kümesi, payload ve allocator
kapasitesi ayrıca yer kaplar. Arama geçici eşleşme ID listelerini tutar.

`stats` segment sayısını, çalışan işi, referans verilen kaynak ve cache baytlarını
bildirir. `resident_text_bytes`, yalnız delta'nın kaynak+katlanmış UTF-8 metin
baytlarıdır; proses RAM'i değildir. Gerçek working set ölçümü için
[ölçüm aracı](tools/measure-scale.ps1) ve [sonuçlar](docs/VALIDATION-0.3.1.md) var.
Arşiv ve yayımlanmamış dosyalar stats'a dahil değildir; toplam klasör diski ayrıca ölçülür.

0.1/0.2 kaynak/marker biçimi okunur; eski kaynak baytları yeniden yazılmaz.
İlk yeni flush 0.3 manifest yayımlar. **Eski motor sürümleri bu manifesti okuyamaz.**
Eski `.idx` biçimi yeniden üretilir. Kaynak `.fdb` bozuksa sağlam cache ile açılışa
izin verilmez. Güncel kalıcı biçim [mimari belgesinde](docs/ARCHITECTURE.md).

## Sınırlar

- Anahtar/konum ve posting sözlükleri RAM'dedir. Dizin JSON'u segment başına
  256 MiB ile sınırlıdır. Sentetik bir milyon kayıt ölçümü gerçek belge çeşitliliği
  veya kesin bir RAM üst sınırı garantisi değildir.
  Disk erişimi güvenli positional I/O kullanır; mmap kullanılmaz.
- Açılışta kaynak log ve türetilmiş segment checksum'ları tamamen doğrulanır.
  Güvenilir hızlı açılış için bu tarama henüz atlanmaz; büyük dosyalarda I/O maliyeti vardır.
- Geniş metin sorguları aday içerikleri tek tek okuyup katlar. Dar metadata/isim
  sorgularının sonuçları geniş içerik sorgularının hızını temsil etmez.
- Kimlik scope'u sorgu sınırıdır; **kimlik doğrulama/yetkilendirme değildir**.
  Güvenilen host, kullanıcının scope'unu oturumundan belirlemelidir.
- SQL, otomatik foreign-key kontrolü, SQL join, dağıtık replikasyon, MVCC,
  HTTP sunucusu, şifreleme ve otomatik yedek geri yükleme yoktur.
- PDF/DOCX/OCR çıkarıcısı yoktur. Çıkarılmış UTF-8 metin kabul edilir;
  ileride çıkarıcı sürümü, kaynak hash'i ve bölüm numarası metadata olarak taşınabilir.
- Para/fiyatların kayıpsız temsili için ondalık string veya ölçekli `i64` kullanın.
  Aralık indeksi yalnız `i64` değerleri içindir; JSON float fiyatları bu indekse girmez.
- Sıralama scope/collection/id anahtar sırasıdır; ilgi puanı veya tarih sırası değildir.
  Sayfalar arasında yazma olursa sonuç değişebilir; yanıttaki `sequence` ile değişim
  anlaşılır. Snapshot pagination yoktur.
- Bir kayıtta başlık+metin 2 MiB, toplam JSON 4 MiB; işlem 64 MiB ve 10.000 mutasyon;
  sayfa 1.000 sonuç ile sınırlıdır. Büyük içerikleri bölüm kayıtlarına ayırın.
- Checkpoint eski nesilleri otomatik silmez. Aktif dosya küçülür; eski nesiller
  arşiv olarak diskte kalır. Motor kapanmadan backup kopyalamayın.
- `sync_all` ve process-kill kurtarması doğrulanır. Windows'ta dizin metadata'sının
  elektrik kesintisindeki atomik kalıcılığı ayrıca doğrulanmamıştır; güç kesintisi
  garantisi iddia edilmez. I/O hatasında yazmanın sonucu belirsiz olabilir; motor
  yeniden açılıp kayıt sürümü kontrol edilmelidir.

Güncel indeks testleri aynı mevcut corpus'un ilk 10.000 belgesiyle yapılır.
Yeni sentetik corpus veya veritabanı klasörü üretilmez. Mevcut sabit FDB
slotunda tam sayfa/anahtar/sürüm doğrulaması için:

```powershell
cargo build --release --example measure_content_reads
.\target\release\examples\measure_content_reads.exe --record-reads --matrix 0 --matrix-check
```

Testler gerçek süreç sonlandırmasını, her olası son-işlem kesilme konumunu, tam
işlemde tek bayt bozulmasını, atomik guard reddini, kapsamları, Unicode aramayı,
çok çerçeveli checkpoint'i ve referans modele karşı güncelleme/silme dizisini kapsar.
Disk testleri bozuk/kayıp indeksin yeniden kurulmasını, base/delta gölgelemeyi,
blok sınırlarını, RAM metinlerinin checkpoint sonrası boşalmasını ve 0.1 uyumunu doğrular.

`experimental-wal-delta`, tam delta belgeleri yerine active WAL konumlarını
tutan deneydir. 100k yazma dahil proses RAM'i %38 düştü; fakat unflushed delta
get100 yaklaşık 6 kat yavaşladı. Hız kapısı geçmediği için default kapalıdır.
[WAL delta ölçümü ve karar](docs/WAL-DELTA-VALIDATION.md).

`experimental-merge-postings`, flush sırasında RAM delta listelerini, merge sırasında
mevcut SEG03/SEG04 listelerini yeniden kullanır. Metin/alan indeksini yeniden
üretmeden listeleri güncel kayıt ID'lerine taşır.
Kaynak/cache bütünlüğü korunur, kalıcı format değişmez; güncel bounded-engine
varsayılanının parçasıdır.
Feature bağımsızdır; başka yazma/açılış deneylerini otomatik açmaz.
[Merge posting doğrulaması](docs/MERGE-POSTINGS-VALIDATION.md).
