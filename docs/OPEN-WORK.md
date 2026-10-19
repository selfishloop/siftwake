# FDB açık işler

2026-10-20 RAM sıkıştırma ön testi tamamlandı/reddedildi:aynı ilk10k,
en fazla200 kaynak metin,4 read-only süreç. Bağımsız LZ4 block+UTF-8 alanı
-%34,17;50 metin yeni sonuç p95 medyan0,462→1,250ms,tampon tekrar
0,095→0,930ms.1/50/200 ve CRC-açık/kapalı bütün karşılaştırmalarda4/4
süreç yavaş. Bu yalnız metin allocation kapasitesi/geri-hazırlama zamanıdır;
proses RAM'i veya gerçek DB get/arama kaybı değildir. Ön kapı geçmediği
için motorun RAM yapısı değiştirilmedi;LZ4 bağımlılığı yalnız ayrı araçta.
5 test/root+probe fmt/Clippy geçti;21 DB/cache/0.1 dosyası ve corpus SHA-eşit.
DB açma/reset,yeni corpus/kalıcı codec/engine feature veya dış entegrasyon
yok. Normal Vec/caller/query RAM,son sayfa/genel p95,yazma/son merge,
açılış/disk sıkıştırma,eşzamanlı/gerçek-cold işler açık.
[Ön test ve kanıtlar](RAM-TEXT-LZ4-10K-20261020.md).

2026-10-20 owned RAM kök nedeni ölçüldü: sonraki500 input, önceki1k
özgün+folded delta ile üst üste; max aşama71,39MiB requested-live, bunun
41,13MiB'si input+özgün belge kökeni,28,91MiB'si RAM arama indeksi. Bu
OS/physical RAM değildir; o anda WAL frame0. Erken flush+hızlı intake fold
normal4 çiftte private80,10→66,75MiB/toplam15,726→15,110s,CPU-%3,91,
disk/core byte'ları aynı verdi. Ancak ilk1k watermark sonrası6 CLI sorgusunun
tamamı4/4 çiftte yavaş: +%19–516p95. Genel geçiş reddedildi, iki runtime
değişikliği geri alındı; normal motor ve önceki yazma kazanımı korundu.
Opt-in aşama gözlemleri/paired araçlar kaldı. Yeni corpus/DB kopyası yok,
boundary sonrası sabit FDB slotu eski normal10k'ya döndü. Normal Vec RAM'i
kapanmadı: yalnız flush zamanını değiştirmek yeterli değil; sıcak yeni
belge okumasını koruyan input/delta tasarımı gerekli. Son sayfa/genel arama
p95,yazma/son merge,query/caller RAM,açılış/disk sıkıştırma,eşzamanlı/cold
yük açık. [Rapor ve bütün örnekler](OWNED-MEMORY-10K-20261020.md).
Geri dönüş193 default/135 all-feature hedef/112 referans ve diagnostic example2
test,fmt/üç profil Clippy,10k/38-case/256-key oracle ve CLI ile doğrulandı;
core/corpus/0.1 fixture byte'ları aynı,SQLite read-only integrity başarılı.

2026-10-18–19 yazma/bakımın dar kimlik eşleme adımı uygulandı/ölçüldü:
19.221.340 posting girdisi10.000 Key aramasıyla eşleniyor. Normal akışlı
yazma+bakım27,277→22,334s (-%18,12;4/4 çift),owned23,274→17,246s
(-%25,90;2/2); CPU bütün çiftlerde düşük. Kaynak/indeks byte'ları API başına
aynı. Streamed son bakım +%0,96/2/4 hızlı: ayrı merge kazancı kanıtlanmadı.
Owned private+310KiB/WS+454KiB,ikisi2/2 yüksek ve açıklaması açık. Ayrı
query/get private -52KiB/WS -72KiB,3/4 düşük; owned RAM'i kapatmaz.
Geniş38'de11 düşük/27 yüksek p95; yoğun9/200'de8/1. Son sayfa
1,8581→1,9965ms (+%7,45) açık; genel arama/RAM kabulü geçmedi.
SQLite yazma kıyası ve iki çift38-case FTS5+LIKE okuma kıyası tamamlandı;
38/38 p95 FDB düşük,bu kendi okuyucumuza karşı artışları kapatmaz.
193 default/135 all-feature hedef/112 referans test;fmt/üç profil Clippy,
kesilme/bozulma/kill,full10k/38-case oracle ve CLI geçti.21 okuma dosyası,
corpus/0.1 fixture SHA-eşit;format/feature/protokol/entegrasyon değişmedi,
yeni corpus/DB kopyası yok. [Rapor ve ham kanıtlar](WRITE-STAGES-10K-20261018.md).
Metin parçalama/RAM posting CPU'su,son merge,normal Vec/caller RAM'i,kalan
arama p95'i,açılış/disk sıkıştırma ve eşzamanlı/gerçek-soğuk yük açık.

2026-10-18 kullanıcı "bu motora geç" kararıyla ölçülen V3'ü normal bounded-engine
profiline aldı. `search_into` etkin; önceki sayfayı tekrar veren çağıran uygun
metin tamponlarını kullanır. `search` uyumlu kalır. Geçiş doğrulandı: normal
paket 191 test/7 ignored; fmt ve üç profil Clippy geçti. Normal release'te
search38 + search_into38 tam belge/sıra/sürüm oracle kontrolü başarılı.
Sayaçlar ikinci the200 çağrısında 200 tamponun yeniden kullanıldığını ve aynı
okuma/CRC/schema/kimlik kontrollerini doğruladı.21 veri/fixture dosyası ve
bütün corpus SHA-eşit; yeni corpus/DB/reset/kopya/format/feature yok.
Önceki kabul reddi aşağıda tarihçedir; kapı geçildi denmiyor. Tek sonuç/normal
okuyucu p95 artışları, OS private+28KiB/WS+48KiB ve geniş sorgu farkları açık.
[Güncel geçiş kaydı](RESULT-BUFFERS-ACTIVE-20261018.md).

Yazma/bakımın ilk yeni adımı yukarıda ölçüldü. Normal Vec ingest/query/
caller RAM'i, Harpoon/genel arama p95'i, açılış/disk sıkıştırma ve eşzamanlı/
gerçek-soğuk yük kapanmadı.

## Geçiş öncesi kabul değerlendirmesi

2026-10-18 sonuç tamponu deneyinin genel geçişi reddedildi. Arşiv prototip
aynı the200 tekrarında p95 -%38,78 (4/4 çift,16/16 blok), değişen dört sayfada
-%10,18 verdi; tek sonuç p95 +%10,68 açık. Geniş38'de the200 p95 yaklaşık aynı
(+%0,01). Eski normal okuyucuya karşı 38-case kontrolünün ardından yoğun400
tekrarda normal API/the200 +%10,01 p95 kaldı (yalnız1/4 çift hızlı); tipik
blok +%0,76. Bekleme/dalgalanma denilerek kapatılmadı. Oracle'sız OS private
medyan +28KiB/WS+48KiB; RAM kazancı yok. Motor/test kaynakları f89b827'ye
döndü; yeni API/helper yalnız source patch'te. Prototip191/7ignored ve üç
profil Clippy geçti. Geri dönüşte engine18/payload2/grouped5/fixture1/aktarim1,
fmt/üç profil Clippy ve normal38-case full oracle geçti.21 veri/fixture dosyası
ve tüm corpus SHA-eşit; yeni corpus/DB/reset/kopya/format/feature yok.
[Güncel rapor](RESULT-BUFFERS-10K-20261018.md).

Sonraki bağımsız öncelik yazma/bakım aşamalarını ayırmak. Normal Vec ingest
RAM'i, tek sonuç/Harpoon/genel arama p95'i, açılış/disk sıkıştırma ve eşzamanlı/
gerçek-soğuk yük açık. Büyük sayfa tekrarındaki dar kazanç bu işleri kapatmaz.

## Önceki hydration profili

2026-10-18 the200 hydration profili tamamlandı; bu tur hız kazancı yok.
Diagnostic30 tekrar: read115/1,810ms, CRC200/0,577ms, schema200/0,586ms,
materialize200/3,631ms; içindeki text String tahsisi+kopyası2,907ms baskın,
fields0,557ms, key0,050ms, links0,042ms. Span'ler nested/instrumented;
normal p95/CPU yüzdesi değildir. Dönen metin5.383.655B; tahsis/memcpy/
ilk erişim payı ayrılmadı. Codec/CRC ayarı ana maliyeti açıklayan çözüm
diye seçilmedi. Sonraki hipotez caller-owned sonuç tamponu tekrar kullanımı;
ayrı API/ömür sözleşmesi ve ölçüm gerekir, henüz uygulanmadı.

İki tek-geçiş prototipi reddedildi: ilk400 örnek the200 p95+%11,58,
tipik+%11,33; eski tahsis sırasını koruyan ikincide p95-%0,42, yalnız8/16
blok hızlı. Ayrı sonuç penceresi64→128KiB çağrıyı115→47 azalttı fakat
normal400 örnekte the200 p95+%8,32/tipik+%16,38,1/4 çift hızlı; reddedildi.
Üçü de kod/test patch'inde arşivlenip geri alındı. Algoritma/source-check/
codec/format aynı, yalnız mevcut diagnostic feature'da sayaçlar kaldı.

The200 uçtan uca hedefi, Harpoon p95, kalan arama farkları, normal Vec/
query/caller RAM, yazma/bakım, genel açılış/disk sıkıştırma ve eşzamanlı/
gerçek-soğuk yük açık. Hız kabulü geçmediği için yeni38-case hız/OS RAM
koşusu yok; final normal38-case full oracle var. Default185/7 ignored,
referans engine17, all-features payload5/grouped5, fmt/üç profil Clippy
geçti; son restore kaynak checkpoint'e byte-eşit, grouped/fmt/default+
all-features Clippy tekrar geçti. Yeni corpus/DB/reset/kopya yok.
[Son hydration raporu](HYDRATION-10K-20261018.md).

## Önceki arama ve kapasite takibi

2026-10-18 sonraki dar takip tamamlandı: ölçülmüş2.464B ek canlı sonuç
kapasitesi giderildi. 38/38 sorguda kapasite artmadı; canlı heap farkı
tam olarak kapasite farkı×176B. Boş sonuçta capacity/live delta0; hemen
elenen negatiflerin tahsis/tepe sayıları aynı. Candidate metni tarayan
negatif ve pozitiflerde yeni4KiB scratch ile geçici peak yaklaşık12KiB
artabilir; genel RAM işi kapanmadı.

Profil beagle19.483→5.520, harpoon46.903→14.039 body fold/scan çağrısı;
38/38 candidate CRC/schema/accepted/payload request-byte/reuse sayıları
aynı. Normal80 örnek/case/variant:38 sorgu31 düşük/7 yüksek p95, beagle
-%20,33 (4/4 çift,8/8 blok). Aynı SHA'larla yoğun9-case200 örnekte beagle
-%3,49 (4/4,8/8), tipik -%4,24. Harpoon tipik -%2,33/8/8 hızlı fakat
yoğun p95 -%0,43/2/4: genel p95 kazancı kanıtlanmadı. The200 hydration
algoritması değişmedi, yoğun p95 -%0,51; büyük kazanım/kök neden işi açık.
Geniş man+%11,60/İSHMAEL+%10,51 yoğun yükte tekrarlanmadı; Unicode-fold
ve sayfa artık artışları açık tutulur. Önceki38/80 ve9/200 örnekler ayrı
dağılımlardır; «bütün regresyonlar çözüldü» denmez.

Oracle'sız4 çift/24search771get OS peak private8,75391→8,78906MiB (+36KiB),
WS12,71289→12,72266MiB (+10KiB); ikisi4/4 artış. Yazma/bakım, diğer sorgu
regresyonları, normal Vec/query/caller RAM, genel açılış, disk sıkıştırma,
eşzamanlı/gerçek-soğuk yük açık. Default185/7ignored, referans engine17+
scratch2, all-features engine1+scratch4, fmt ve üç profil Clippy geçti.
Normal bütün38+yoğun9 full oracle ve CLI kontrolleri geçti.21 dosya ve
tüm corpus SHA-eşit; yeni corpus/DB/reset/kopya/format/feature yok.
[Son arama/kapasite takibi](SEARCH-FOLLOWUP-10K-20261018.md).

## Varsayılan geçişi ve önceki ölçümler

2026-10-18 güncel karar: kullanıcı talebiyle ölçülen checked-page algoritması
varsayılan `bounded-engine` motoruna alındı. Aynı arşiv kodu/beş test;
feature/API/kalıcı format değişikliği yok. Kaynak ve candidate doğrulaması
korunur. Default185 test (7 ignored), referans engine17 ve all-features
hedefli5 test, fmt ve üç profil Clippy geçti. Normal release38 sorgu tam
oracle ve CLI kontrolü, diagnostic default reuse kanıtı geçti.

Son normal release A/B: aynı10k,38 sorgu,80 örnek/motor/sorgu, sıcak cache,
dört dengeli süreç çifti. P95:20 düşük/18 yüksek; quixote -%6,62
(3/4 çift, tipik blok -%18,40,8/8 hızlı), quixote-whale -%11,03 (4/4).
Beagle +%14,33 (4/4 yüksek), harpoon +%14,91, the/200 +%8,88 ve
ishmael/130/1 +%13,12 açık; he/zx ve uzun negatif tipik blok maliyeti de
kapanmadı. Ayrı oracle'sız24 search/771 get,4 çift: OS peak private
8,7090→8,7363MiB (+28KiB,3/4 artış); WS12,5859→12,6797MiB
(+96KiB,4/4 artış). Genel bütün-sorgu/OS RAM kapısı geçti denmiyor;
varsayılan geçiş kullanıcı kararıdır. Önceki tek-executable tekrar ve
bu iki-executable sonuç birbirine karıştırılmaz.

Öncelikli küçük takip: ölçülmüş2.464B fazla sonuç Vec kapasitesi ve ortak
boş/negatif yol. Bu geçişte kapasite veya eşik ayarı yapılmadı; yeni hız
iddiası bunlara atfedilmez. Beagle/harpoon/sayfa p95 payları, yazma/bakım,
normal Vec/query/caller RAM, genel açılış, disk sıkıştırma, eşzamanlı ve
gerçek-soğuk yük açık.21 dosya ve tüm corpus SHA-eşit; FDB349.805.698B,
SQLite583.860.224B, fixture/Cargo aynı. Yeni corpus/DB/reset/kopya yok.
[Varsayılan geçiş raporu](DEFAULT-CHECKED-PAGE-10K-20261018.md).

## Önceki ölçümler ve kararlar (varsayılan geçişinden önce)

2026-10-18 arka plan işleri kapatıldıktan sonraki tekrar tamamlandı.
Aynı SHA’lı program, aynı 10k, 38 sorgu /120 örnek/koşul: quixote p95
-%11,64/-%10,59 (iki kapalı kontrol), ikisinde 4/4 süreç ve 23/24 blok hızlı.
Yoğun dört sorgu /240 örnek: quixote -%13,68/-%14,96; 4/4 süreç, 24/24 blok.
Holmes genel p95 kazancı hâlâ tutarsız; zx’in eski büyük artışı bu koşuda
tekrarlanmadı. Uzun negatif yoğun p95 +0,9/+1,7µs (+%2,86/+%5,54), ikinci
kontrole göre 4/4 süreçte yüksek; açık. the/1 ve ishmael/130/1 artışları
dört-sorguluk yükte iki kontrole göre tutarlı tekrarlanmadı; geniş sorgu
setindeki artışlar kapatılmadı. Sıra/durum/yerleşim payları tam ayrılmadı.

Geniş koşuda blok CPU/wall medyanı %99,60; blok süreleri kararlı, tamamen izole sistem iddiası yok. Genel 38-case/OS RAM kabulü geçmedi;
aktif motor kaynakları aynı, prototip varsayılana alınmadı. Sonraki küçük
kod işi: 2.464 B sonuç kapasitesi fazlasını gidermek ve ortak boş/negatif
hızlı yolu korumak. Henüz uygulanmış çözüm değil. Yazma/bakım, diğer arama
regresyonları, normal Vec/query/caller RAM, açılış, sıkıştırma ve eşzamanlı/
gerçek-soğuk işler açık. fmt/Clippy ve tüm oracle kontrolleri geçti; 21 dosya
ve tüm corpus SHA-eşit, yeni corpus/DB/reset/kopya yok.
[Son tekrar](REUSE-REPEAT-10K-20261018.md).

2026-10-18 tek-program kontrolü tamamlandı; yeni çalışma yönü daraldı.
38-case /120 örnek/koşulda off1/off0 20 düşük/18 yüksek p95; on/off0
22/16, on/off1 19/19. Genel p95 kabulü yok. Quixote dar tipik blok kazancı
iki kapalı kontrol karşısında sürüyor; holmes genel kazancı kanıtlanmadı.
Uzun negatif karışık/tek-sorgu normal düzende off0'a göre 4/4 süreçte daha
yüksek p95; kapatılmadı. Payload read/CRC/retention sıfır, requested heap
sayaçları aynı: CRC/codec ayarı bu kalemi hedeflemez. Büyük wall/CPU
farkları bekleme payını kanıtladı; bütün eski artışları veya belirli bir
uygulamayı açıklayan kök neden olarak sunulmadı. AboveNormal da plasebo
oynaklığını gideremedi.

Pozitiflerde sorgu sonrası +2.464B canlı heap: sonuç Vec kapasitesi
52→66, 14×176B ile eşleşti. Bu, önceki +64KiB private/+170KiB WS'nin
tamamının açıklaması değildir; yeni oracle'sız OS RAM koşusu yok. Sonraki
kod işi: ortak boş/negatif yolu korumak, yeniden kullanım helper'ını gerçek
pozitif aşamasında tutmak ve sonuç vektörünün fazla kapasitesini düzeltmek.
Bu henüz uygulanmış/hızlanmış bir çözüm değil. 2MiB/50 eşik turu veya
format değişimi yok. Genel geçiş için plasebo kontrollü bütün 38 sorgu ve
ayrı proses RAM kapısı gerekiyor.

Prototip 190/111/209 test (7/3/9 ignored), fmt ve Clippy geçti; aktif
runtime/test/Cargo başlangıç byte'larına döndü. Geri dönen motorda 7 window
testi, fmt/default+all-features Clippy geçti. On fazda 21 dosya ve bütün
corpus SHA-eşit; yeni corpus/DB/reset/kopya yok. Yazma/bakım, diğer arama
regresyonları, normal Vec/query/caller RAM, genel açılış, disk sıkıştırma,
eşzamanlı/gerçek-cold yük açık. [Son rapor](REUSE-CONTROLS-10K-20261018.md).

2026-10-18 pozitif contains sonrası ikinci payload okuması ölçüldü. Son
izole prototipte quixote read API131→81, ikinci CRC/decode50→0, candidate
CRC56→56; holmes1514→1467 read/1285 candidate CRC korunur. Aynı mevcut10k,
400-örnek/variant quixote p95-%11,11/tipik-%17,92 (4/4 süreç,15/16 blok),
holmes-%13,52/tipik-%3,82 (4/4,15/16). Dar8-case8/8 düşük, önceki he/the200
artışları bu tekrarda yok. Buna rağmen geniş38-case15 düşük/23 yüksek p95;
quixote-%6,17, zx+%15,14, the200+%10,83. Ortam/yerleşim/kod payları tam
ayrılmadı; artışlar kapatılmaz. Query/get peak private medyan+64KiB,
WS+170KiB; ikisi4/4 süreçte arttı. Genel geçiş reddedildi, prototip/beş
test patch'te saklanır. Sonraki iş bu dar iş azalmasını genel sorgu ve bellek
regresyonu olmadan koruyan tasarım olmalı; henüz çözülmedi. Daha fazla2MiB/
50/eşik ayar turu yok. Yazma/bakım, genel arama, normal Vec/query/caller RAM,
açılış, disk sıkıştırma ve eşzamanlı/gerçek-cold yük açık kalır.
[Bütün sonuçlar ve sınırlar](CHECKED-PAGE-10K-20261018.md).
Final: prototip185/107/204 test, fmt ve default/all-features Clippy geçti.
Runtime ve test kaynakları97f6ef7'a byte-eşit döndü;7 window testi ve fmt/
Clippy yeniden geçti. Normal CLI10k/sequence20, dört sorgu total/hit/version
kontrolü ve21-file SHA eşitliği geçti;12 ölçüm snapshot'ı ve tüm corpus da
SHA-eşit. Yeni hız/RAM sonucu aktif, geri dönülen motora atfedilmez.
[Final doğrulama](benchmarks/checked-page-final-proof-20261018.json).

2026-10-18 CRC/UTF-8 parçalama hipotezi ölçüldü ve reddedildi. Baseline
12 diagnostic örnekte holmes CRC3,491ms/yapı4,215ms; zx26,849/26,569ms.
Normal4-case/400 örnek kontrolünde holmes p95+%1,80 (3/4 çift yüksek),
zx+%0,32 (4/4 yüksek), tipik blok+%0,79/+%1,77. Yeni helper'a girmeyen
the/200-%2,35 farkı algoritma kazancı sayılmaz; kod/yerleşim/ortam payları
tam ayrılmadı. Runtime cb2c19e'a byte-eşit döndü; private unsafe modül
kaldırıldı, kod/testler patch'te saklandı. Yeni38-case/RAM koşusu yok.
Gruplama ve CRC chunk ayar turları kapalı. Ayrı sonraki hipotez: eşleşen
belgenin sonuç hazırlamada tekrar okunması/doğrulanması; henüz uygulanmadı.
Yazma/bakım, genel arama, normal Vec/query RAM'i, açılış, sıkıştırma ve
eşzamanlı/gerçek-cold işler açık. [Son kanıt](VALIDATION-CHUNK-10K-20261018.md).
Üç prototip ve yedi geri dönüş testi, fmt ve default/all-features Clippy
geçti; normal release tekrar hazır.21 dosya ve corpus SHA-eşit. Tam-suite
bu tur yeniden koşulmadı. Pozitif tekrarda quixote56+50, holmes1285+50
CRC/decode çağrısı ölçüldü; negatif zx'te ikinci hydration işi yoktur.

2026-10-17 gruplamayı hızlı döngüden ayıran ikinci prototip de reddedildi.
Kaynakta eski döngüler korundu; aynı10k/4 çift/16 blok/400 örnek kontrolünde
darcy p95-%2,74 (3/4 çift), holmes+%0,98 (2/4 düşük), the/200-%2,98.
Tipik blok -%2,32/-%1,39/-%0,88; holmes'ta tekrarlanan p95 kazancı yok.
the/200 artışı bu koşuda tekrarlanmadı; kök nedeni kesin çözüldü sayılmaz.
Dar kapı geçmedi, yeni38-case/diagnostic/RAM koşusu yapılmadı. Runtime ve
window_tests f6d18dd'a byte-eşit döndü; kod/testler patch'te saklandı.
Gruplama ayar denemeleri burada kapandı; sonraki bağımsız arama işi kalan
byte doğrulama/tarama/hydration tekrarını ölçüp tek hedef seçmek olmalı.
Yazma/bakım, genel arama, normal Vec/query RAM'i, açılış, disk sıkıştırma,
eşzamanlı/gerçek-cold işler açık. [Son kontrol](PAYLOAD-ISOLATED-10K-20261017.md).
18 Ekim final kontrolü:180 default/all-targets test, fmt ve default/all-features
Clippy geçti;21 dosya ve tüm corpus SHA-eşit. Yeni RAM/38-case sonucu yok.

2026-10-17 payload adaylarını gruplama prototipi reddedildi/default'a alınmadı.
Aynı mevcut10k'da holmes read1285→1106, darcy2129→1700, watson1954→1553;
ek byte%0,1 altı, bütün38 senaryoda doğrulama ve tarama iş sayıları aynı.
Normal38-case34 düşük/4 yüksek; holmes p95-%9,18 (4/4 çift), tipik-%2,73.
the/200 ilk p95+%10,38, yoğun800 örnek/variantta+%6,02 ve tipik+%2,78.
A/A the/200 p95+%1,69/tipik-%3,74; kod/yerleşim/ortam payı ayrılmadı,
artışlar dalgalanma sayılıp kapatılmadı. Query/get peak private+46KiB,
WS+72KiB medyan; disk349.805.698 B/hash aynı. Runtime ve test kaynağı
6eb3a89'a döndü, prototip patch'te saklandı. Sonraki adayda planlama ayrı
yolda tutularak mevcut hızlı döngü korunmalı; bu henüz ölçülmüş çözüm değil.
Yazma/bakım, genel arama/RAM, normal Vec, açılış, sıkıştırma, eşzamanlı/
gerçek-cold işler açık. [Son deney ve kanıt](PAYLOAD-BATCH-10K-20261017.md).
Final default180/reference107/all199 test (örnekler dahil), fmt ve default/
all-features Clippy geçti;21 dosya snapshot'ı ve tüm corpus SHA-eşit.

2026-10-17 seçilen belgelerde metadata tekrarı azaltıldı: aynı 350 MB /10k
DB'de the/200 metadata çağrısı 714→514; CRC/schema/key/version korunur.
İki A/B dizisinde dar kazanç var; yoğun 400 örnek/sayfa/variantta p95
6,496→5,779 ms (-%11,04), 4/4 süreç ve16/16 blok hızlı, tipik -%12,75.
Ayrı query/get private medyanı -30 KiB, WS -6 KiB; genel RAM kapısı değil.
Geniş 38-case23 düşük/15 yüksek; p95 sapmasının kesin nedeni ayrılmadı.
Yeni A/A'da 29 düşük/9 yüksek, yoğun aynı-mod A/A tipik farklar küçük;
kontrol oranları A/B'den çıkarılmaz ve kalan artışlar kapatılmaz.
holmes payload okuması hâlâ1285 /35,9 MB; quixote56 /1,95 MB aynı.
Sıradaki arama hedefi normal kelimelerde gerçek payload okuma/aday maliyeti.
Yazma/bakım, normal Vec/query/caller RAM'i, açılış, disk sıkıştırma ve
eşzamanlı/gerçek-cold yük açık. Default180/reference107/all199 test geçti,
fmt/Clippy temiz; format/feature/bağımlılık ve0.1 fixture değişmedi.
[Bu adımın kanıtı ve ayrıntılar](SEARCH-HYDRATION-10K-20261017.md).
Güncel aynı-veri SQLite read-only karşılaştırması tamamlandı: 38/38 p95,
her sorguda4/4 çift FDB düşük. Ayrı query/get peak private FDB8,78 /
SQLite12,83 MiB, WS12,53/16,70 MiB; disk350/584 MB. Aynı mevcut8 kitap/
ilk10k, query/get full-payload oracle ve8 snapshot çiftinde hash eşitliği
geçti; corpus'un tamamı SHA-eşit. İlk iki hız çifti kota kesintisi öncesi,
son ikisi aynı binary/DB hash'leriyle sonra tamamlandı; koşullar raporda.
Bu sonuç önceki FDB'ye göre15 p95 artışını, payload okuma maliyetini,
normal Vec ingest RAM'ini, açılışı veya yazma/bakımı kapatmaz.

2026-10-17 dar arama kazancı tamamlandı: aynı350MB/10k DB'de uygun kısa
ASCII yolunda gövde fold263.627→0; CRC/schema/UTF-8/key/version aynı.
Son normal strict blok p95 zx277,138→190,633ms (-%31,21), the-zx
266,181→175,714ms (-%33,99); ikisi4/4 süreç çiftinde,32/32 blok hızlı.
Tipik blok kazancı%30–31. Tüm38-case28 düşük/10 yüksek; genel p95 kapısı
açık. Aynı SHA A/A uzun38/38, kısa36/38 yüksek tail; tipik blok farkları
-%2,41…+%2,74. Kalan artışlar dalgalanma sayılıp kapatılmadı.
Diğer36 sorgunun izlenen bütün iş sayıları aynı.4KiB chunk reddedildi.
Son query peak private/WS medyanı -%0,38/-%0,37; genel RAM kapısı değil.
Açılış gözlemi -%6,81, algoritmik kazanç sayılmadı.18 dosya349.805.698 B
hash-eşit, yeni corpus/reset/repack yok. Yazma/bakım, normal Vec/query/
caller RAM'i, genel açılış, sıkıştırma, concurrent/cold hâlâ açık.
[Sonuç, bütün38 tablo, A/A ve sınırlar](SEARCH-STAGES-10K-20261017.md).

Standart eski nesil temizliği tamamlandı: mevcut10k'da105 dosya,
622.859.277 B kaldırıldı; toplam972.664.975→349.805.698 B (-%64,04).
Canlı kaynak/cache/ready byte'ları ve10k/38-case oracle aynı; yeni
corpus/kopya yok. İki yeni ingestte de350MB, canlı17 core hash-eşit.
Yazma+bakım iki çiftte+%1,34 (1/2 düşük), bakım+%2,82; open ilk+%8,40,
tekrar-%2,86, aynı-program+%3,00. Performans etkileri gürültü diye
kapatılmadı. Default159/reference90/all177 tam test, ayrıca üç profilde
automatic publication/cleanup kill1'er test; fmt/Clippy temiz.
Arama22/38 artış, yazma/kabul/CRC ve spool tekrarları, normal Vec/query RAM,
açılış, eşzamanlı/gerçek-cold ve güç kesintisi hâlâ açık. Dynamic geçici
isimler ve root'tan ileri orphan'lar için genel alan geri kazanma sınırları
raporda. [Güncel temizlik raporu](RECLAIM-10K-20261017.md).

Kullanıcı kararı: SEG05 flag1 yeni native indekslerde varsayılan. Commit/
flush, merge ve cache onarımı gerçek kaynak entry adresini doğrudan yazar;
önce kopyalı SEG04 oluşturulmaz. Okunabilir eski cache'ler korunur, kaynak/
WAL/manifest/0.1 fixture değişmez. Default147/reference78/all165 test,
fmt ve üç profil Clippy temiz. Yeni4 çiftte yazma+bakım28,559→25,744s
(-%9,86), strict open259,443→169,180ms (-%34,79), canlı indeks353,7→
81,9MB, retained core1,521GB→972,7MB. Ayrı query private8,883→8,766MiB;
akışlı write private10,156→10,301MiB (+%1,42;4/4 yüksek).38-case16 düşük/
22 yüksek p95; arama ve genel RAM kapısı kapanmadı. Yeni Run'ın son
kabulündeki sahipli replay tekrarı, geçici JSON spool maliyeti, normal Vec
RAM'i, eski nesiller ve concurrent/
gerçek-cold açık. [Geçiş raporu](DEFAULT-SEG05-10K-20261017.md).

Önceki isteğe bağlı prototipin tarihsel sonucu:
Son SEG05 deneyi: JSON dictionary73→41ms (kopya korunurken instrumented),
source referansıyla267.849.329B payload kopyası çıkarıldı. Son normal4 çiftte
strict open260,075→170,024ms (-%34,62;4/4), ilk kullanım264,784→173,884ms.
Canlı indeks353.688.186→81.904.473B; ayrı query/get private8,891→8,795MiB,
WS13,301→13,260MiB,4/4 düşük. Kayıp read-ahead1104→10000 isteğe çıkmıştı;
5/42B kaynak başlıkları tanınınca1104/8896 hit geri geldi,10k CRC/schema aynı.
Geniş38-case32 düşük/5 yüksek/1 eşit, yoğun8 sayfa5/3; artışlar kapanmadı.
Normal yazıcı SEG04; isteğe bağlı prototype otomatik/default'a alınmadı.
Sonraki kalıcı adım source adresini streamed builder'a bağlamak ve yazma/
bakım/query/get/bellek kapılarını birlikte ölçmek. Vec RAM'i, eski nesiller,
kernel/file cache, concurrent/gerçek-cold açık. Yeni corpus/reset/DB kopyası
yok; eski okuyucu38-case kontrolüyle SEG04'e döndü,133 dosya byte-eşit.
[Son rapor](COMPACT-INDEX-10K-20261017.md).

Önceki indeks açılışı adımı: kaynak/WAL ve sözlük yolunu koruyan bağımsız
exact-read CRC helper'ı. İki ayrı dört dengeli çiftte strict open+ilk
get+search279,712→267,971ms (-%4,20),282,370→264,915ms (-%6,18);8/8 hızlı.
Yalnız open -%1,08/-%8,16; tek sabit yüzde iddiası yok. Yoğun8 sayfa p95
düşük; son adayda geniş38-case performansı yeniden ölçülmedi. Ayrı dört
query/get memory çiftinde private/WS tepe artışı yok; genel/Vec RAM kapısı
kapanmaz. İlk COW/sözlük adayı yoğun8/8 p95 +%5,17…10,08 ile geri alındı.
Metadata/sözlük maliyeti, yazma/bakım, önceki arama artışları, disk ve
eşzamanlı/gerçek-cold yük açık. Kaynak/indeks byte'ları korunur; yeni
corpus/reset/kopya yok. Son dört-çift instrumented profilde CRC83,831→
78,336ms, toplam indeks155,886→158,377ms; indeks toplamı hızlandı iddiası
yok. Son testler155/94/174, fmt/Clippy ve normal CLI geçti.
[Son rapor](INDEX-OPEN-10K-20261017.md).

Güncel dar açılış adımı: aynı mevcut10k/dört dengeli çiftte normal strict
open430,303→270,116ms (-%37,23;4/4), açılış+ilk get+search423,924→281,714ms
(-%33,55;4/4). Kaynak CRC/schema/key/link/version kontrolleri tam; pending0.
Sağlam cache varken kaynak belge nesneleri kurulmaz, tek doğrulama geçişi
kullanılır.38-case son kontrol23 düşük/15 yüksek p95; genel arama kapısı
kapanmadı. Kaynak/cache/SQLite hash'leri aynı; yeni corpus/reset/kopya yok.
İndeks CRC/directory hazırlığı, normal Vec RAM'i, yazma/bakım tekrarları,
disk ve concurrent/cold açık. Yoğun8 sayfa kontrolünde p95 +%1,13…6,22;
aynı-kod A/A'da7/8 yüksek, kod payı sıfır diye kapatılmadı. Son testler
154/93/173 ve fmt/Clippy geçti. Yeni sıcak teşhiste kaynak100ms/indeks162ms;
sonraki açılış hedefi indeks CRC/directory hazırlığıdır.
[Güncel rapor](STRICT-OPEN-10K-20261017.md).

Son SQLite karşılaştırması: aynı mevcut10k, normal default release,
dört dengeli akışlı/SQLite çifti ve dört38-case okuma çifti. Yazma+bakım
23,598/34,509s (-%31,62;4/4);38/38 p95 ve her durumda4/4 FDB hızlı.
Ayrı oracle'sız query/get peak private9,189/12,740MiB, WS12,824/16,695MiB.
Akışlı ingest private9,156/8,402MiB (+%8,97); normal Vec78,912MiB hâlâ
yüksek. Strict open433,686/1,535ms, disk1.521/584MB; Vec son bakım5,104/
1,646s. Bu karşılaştırma tamamlandı; önceki eski/yeni arama artışları,
mutlak olumsuz tarama gecikmesi, Vec/query RAM kapsamı, bakım tekrarları,
disk/eski nesil ve concurrent/gerçek-cold kapıları açık. Yeni corpus/DB
kopyası/engine değişikliği yok; tüm belge, query ve hash/integrity kontrolleri
geçti. [Güncel kanıt ve sınırlar](SQLITE-DEFAULT-COMPARISON-10K-20261016.md).

Kullanıcı isteği: sonraki performans sonuçlarında bu listeyi hatırlat ve
tamamlanan işi kanıt/rapor bağlantısıyla kapat. Zamanlanmış otomasyon değildir;
bu repo içindeki çalışma takibidir. Yeni corpus yok; aynı ilk10k belge ve sabit
iki DB slotu kullanılır. Büyük koşu için önce10k sonuçları iyileştirilir.

Güncel varsayılan `bounded-engine` olarak kalıcılaştırıldı: aynı ölçülmüş
birleşim, eski feature adı uyumluluk alias'ı, özel bayrak gerektirmeyen
streamed API; normal Vec/strict open/WAL sync/format aynı. Default151,
no-default92,all170 test ve Clippy/fmt geçti. Streamed tepe private9,252MiB
(ayrı tekrar9,182MiB), WS13,139/13,043MiB; Vec79,111MiB, boşta1,889MiB.
İlk write+bakım+%4,88, ayrı tekrar-%0,87; A/A-%2,33: artış kanıtsız
kapatılmadı. Geniş arama24/14, yoğun sayfa5/3; üç küçük sayfa artışı
%0,26…1,30.16 streamed koşu122 core hash'i aynı; önceki profile de eşit.
Bu dar taşıma tamamlandı; genel hız/Vec-query RAM/disk/concurrent/cold
kapıları açık kalır. [Son ölçüm ve karar](DEFAULT-PROFILE-VALIDATION-10K-20261016.md),
[normal kullanım ve sınırlar](DEFAULT-PROFILE.md).

Önceki kısa-blok kontrolü: iki farklı-sürüm serisinde8/8 sayfanın yeni p95'i
düşük; ishmael tek-sonuç -%6,51 / -%3,05, quixote /28/10 -%4,39 / -%2,45.
Aynı-kod kontrolünde the tek-sonuç +%6,44 ve4/4 yüksek olabilir. Bu düzende
gerileme tekrarlanmadı; yeni worker/codegen ve cache oturum ömrü nedeniyle
eski kesintisiz koşulların tüm payları veya genel kabul kapısı kapanmadı.
Motor değişmedi; %11,49 yazma+bakım önceki ölçümden korunur, RAM/disk ve
concurrent/cold performansı yeniden ölçülmedi.
[Son kontrol ve sınırlar](INTERLEAVED-SEARCH-10K-20261016.md).

Önceki teşhis: sekiz sayfada tüm query iş sayaçları birebir aynı ve yazma gram
çıkarımı0 çağrı. Aynı program/aynı CPU kontrolünde ishmael p95 +%4,23;
farklı-sürüm yeni aynı-CPU testinde +%3,45, quixote +%9,64. Ortam
kararsızlığı kanıtlandı; ilk farkın tamamı kod regresyonu veya tamamen
gürültü diye kapatılmadı. Kod/yerleşim katkısı ve genel kabul kapısı açık.
[Son teşhis ve araçlar](SEARCH-ATTRIBUTION-10K-20261016.md).

Önceki yazma adımının yoğun sayfa kontrolü:800 örnek/sayfa,1 düşük/7 yüksek
p95; ishmael /130/1 +%4,87 ve4/4 yüksek, quixote /28/10 +%4,03 ve3/4
yüksek. Bunlar dalgalanma diye kapatılmadı. %11,49 yazma+bakım kazancı
deneysel profilde tutulur; genel/default kabul kapısı geçmedi.
[Yoğun kontrol ve sınırlar](GRAM-BODY-WRITES-10K-20261016.md).

| İş | Durum / kapanış kanıtı |
|---|---|
| CRC ve payload/UTF-8 geçişlerini yakın parçalarla yürüt | Reddedildi. Aynı10k/400 örnekte holmes p95+%1,80/3 çift yüksek, zx+%0,32/4 çift yüksek; tipik blok da yavaş. Kontroller korunmuştu, runtime geri alındı; private unsafe helper yalnız arşivde. Yeni38-case/RAM sonucu yok; chunk ayar turu açılmayacak. Pozitif sonuçta aynı belgenin tekrar okunması ayrı, uygulanmamış hipotez. [Ölçüm/patch](VALIDATION-CHUNK-10K-20261018.md) |
| Gruplamayı hızlı sorgu yolundan ayır | İkinci prototip reddedildi; aynı10k/400 örnekte darcy -%2,74, holmes +%0,98/2 çift düşük, the/200 -%2,98. the/200 artışı bu koşuda yok; hedefte tekrarlanan p95 kazancı yok. Eski döngüler byte-eşit korunmuştu, runtime geri alındı. Yeni38-case/RAM koşusu yapılmadı. Gruplama için yeni ayar turu yok; genel arama ve diğer performans işleri açık. [Kanıt/patch](PAYLOAD-ISOLATED-10K-20261017.md) |
| Bilinen payload adaylarının yakın okumalarını birleştir | 2026-10-17 ilk prototip reddedildi; runtime 6eb3a89'a döndü. 19/38 senaryoda read azaldı, holmes 1285→1106 ve p95 -%9,18; the/200 ilk +%10,38, yoğun tekrarda +%6,02. A/A oynaklığı kod payını sıfırlamaz. Ayrı query/get private +46KiB/WS +72KiB medyan, disk aynı. Kuyruk/planlamayı hızlı döngüden ayırma henüz ölçülmüş çözüm değil; genel arama, yazma/bakım, Vec/genel RAM, açılış, sıkıştırma ve eşzamanlı/soğuk yük açık. [Patch ve kanıt](PAYLOAD-BATCH-10K-20261017.md) |
| Aynı temiz veride arama gerilemelerini ayır, en pahalı aşamayı azalt | Dar kısa-ASCII adımı tamam: body fold263.627→0; iki negatifte normal p95 -%31,21/-%33,99 (4/4 süreç,32/32 blok). Tüm38-case28/10; A/A36/38 tail artışı nedeniyle genel p95 kapısı açık. Diğer36 senaryoda izlenen iş sayıları aynı; dar/uzun kelimelerde payload okuma ve sonuç hazırlama hâlâ maliyetli. Query peak medyanları artmadı, disk349.805.698 B/18 dosya aynı. Yazma/bakım, Vec/genel RAM, açılış, sıkıştırma ve concurrent/cold kapanmadı. [Son ölçüm/kanıtlar](SEARCH-STAGES-10K-20261017.md) |
| JSON sözlük ve indeksteki267,8MB belge kopyası | SEG05 flag1 kullanıcı kararıyla native default yazıcıya taşındı: commit/flush, merge ve cache onarımı doğrudan kaynak adresi yazar, payload kopyası üretmez. Eski okunabilir SEG03/04 korunur; kaynak/WAL/manifest/fixture aynı. Varsayılan ve referans/all-feature testleri, kesilme/bozulma ve builder/repack kill kontrolleri geçti. Yeni4 çiftte write+bakım-%9,86 ve open-%34,79; canlı idx353,7→81,9MB, retained core1,521GB→972,7MB. Yeni38-case16/22; write private +%1,42/4 çift yüksek, query private düşük. Önceki yoğun sayfa artışları da açık.72 kaynak/ready ve prototip wire karşılaştırmasında122 core dosya hash-eşit. Yazma/kabul tekrarları, RAM/disk-retention ve concurrent/cold açık. [Güncel geçiş/ölçüm](DEFAULT-SEG05-10K-20261017.md), [ilk prototip](COMPACT-INDEX-10K-20261017.md) |
| İndeks açılışında checksum okuma ve hazırlığı azalt | Dar okuma adımı ölçüldü: indekse özel max1MiB exact reads; bütün indeks CRC'si ve diğer doğrulamalar tam, kaynak/WAL ve sözlük algoritmaları aynı. İki4-çift seride ilk kullanım -%4,20/-6,18;8/8 hızlı. Yalnız open kazancı oturumlara bağlı%1–8. Yoğun8 sayfanın p95'i düşük; genel arama kapısı değil. Ayrı query/get tepesinde artış yok; Vec ve concurrent RAM ölçülmedi. COW/reuse prototipi %5–10 arama artışıyla geri alındı. Hazırlık maliyeti ve genel açılış işi açık. [Son rapor](INDEX-OPEN-10K-20261017.md) |
| Tam doğrulamalı açılışta kaynak tarama ve belge hazırlığını azalt | Dar kaynak adımı ölçüldü:4 dengeli normal release çiftinde430,303→270,116ms (-%37,23;4/4), pending0; ilk kullanım toplamı423,924→281,714ms. Frame+source CRC, UTF-8/schema/key/link/version/sınırlar ve source lease sürer. Sağlam cache yoksa önceki builder yolu, WAL/0.1 kaynaklar aynı. Query/get peak private9,197→8,637MiB; bu Vec ingest veya toplam sistem RAM'i değildir. Son38-case23 düşük/15 yüksek; genel arama garantisi yok. İndeks CRC/directory maliyeti ve genel açılış işi açık; SQLite normal open seviyesine ulaşılmadı. [Rapor ve ham kanıt](STRICT-OPEN-10K-20261017.md) |
| Varsayılan FDB'yi SQLite ile aynı10k'da yeniden karşılaştır | Tamamlandı, kapsam sınırlı:4 dengeli streamed ve2 Vec yazma çifti;38-case ve256 get için4 dengeli okuma çifti;4 opening ve2 oracle'sız query/idle memory çifti. Streamed toplam23,598/34,509s;38/38 p95 ve bütün durumlarda4/4 FDB hızlı. Query private9,189/12,740MiB; normal Vec78,912/8,402MiB, strict open433,686/1,535ms ve retained disk2,61 kat. İlk iki SQLite write süresi yüksekliği ayrıştırılmadı; SQL prepared cache64, FTS5/LIKE/native3.53.2; validator3.53.1. Kaynak/fixture/corpus/hash/10k payload/FTS integrity aynı. Motor, storage, protokol değişmedi; eski/yeni arama regresyonları ve genel kapılar bu kıyasla kapanmaz. [Rapor](SQLITE-DEFAULT-COMPARISON-10K-20261016.md) |
| Kabul edilen birleşimi normal derlemeye taşı ve yerelde sabitle | Tamamlandı (dar kapsam): checkpoint42116f7; bounded-engine default, eski feature alias'ı ve no-default reference korunur. Runtime algoritmaları/bağımlılıklar/kalıcı biçim/protokol aynı; bağımsız rkyv/WAL-delta/shared-documents/sync-off deneyleri kapalı.151/92/170 test, Clippy/fmt, eski alias ve comparator check, varsayılan fdb.exe stats/get/search geçti.2 çift Vec,4 çift streamed+2 SQLite referansı,2 çift A/A ve2 çift tekrar,38-case/yoğun sayfa/açılış/idle ölçüldü. İlk streamed+%4,88 tekrar-%0,87; broad24 düşük/14 yüksek ve pages5/3. Kalan hız artışları kapatılmadı; genel hız/Vec-query RAM/disk/eşzamanlı/cold kapıları açık.122 dosya16 streamed ingestte ve önceki profille byte-eşit. Yeni corpus/DB kopyası yok; yalnız sabit slotlar sıfırlandı. [Sonuç/karar](DEFAULT-PROFILE-VALIDATION-10K-20261016.md) |
| Kısa bloklarla eski/yeni aramayı ve etiket etkisini sınama | Ölçüldü:4 süreç çifti×8 blok×25 tur,800 örnek/sayfa/variant/seri; ilk A/B ve ters etiketli BA8/8 yeni p95 düşük. Ishmael /130/1 -%6,51 / -%3,05 (4/4 ve3/4 hızlı), quixote /28/10 -%4,39 / -%2,45 (3/4 ve2/4). A/A6 düşük/2 yüksek, the /0/1 +%6,44/4 çift yüksek fakat blok medyanı-%1,01; tek p95 yönü kod nedeni ilan edilmez. Kilit/CRC/kaynak/format/default ve motor aynı; yeni corpus/reset/DB kopyası yok,139 dosya hash'i aynı. Bu kısa-oturum/codegen düzeninde gerileme tekrarlanmadı; kesintisiz oturumun yerleşim/ortam katkısı ve genel kabul kapısı açık. Yazma/bakım, query/normal API RAM'i, disk ve concurrent/cold işleri korunur.8 example testi/fmt/Clippy geçti. [Kanıt/sınırlar](INTERLEAVED-SEARCH-10K-20261016.md) |
| Yazma sonrası arama gerilemesinin nedenini ayır | İncelendi, tam kapanmadı. Normal dört-çift aynı-CPU A/B8 yüksek p95; A/A2 düşük/6 yüksek, aynı kodda ishmael +%4,23. İlk A/A son çift bütün sayfalarda+%44,79…54,91. Profilde sekiz sayfa/1.920 doğrulamanın aday/read/byte/CRC/doğrulama/ham-kanıt sayıları iki sürümde aynı; ek body fold/scan ve write gram çağrısı0. Normal yeniden derlemeler eski measured kontrol/aday `.text`'leriyle eşleşir; sorgu kaynakları aynı ama machine code/yerleşim aynı değil. Motor aramasına tahmine dayalı düzeltme yapılmadı; kalan quixote ve kod/yerleşim katkısı açık. %11,49 yazma+bakım kazancı önceki ölçümdür, yeniden ölçülmedi. Yeni RAM/disk kazancı veya concurrent/cold sonucu yok. [Kanıt/sınırlar](SEARCH-ATTRIBUTION-10K-20261016.md) |
| Uzun gövdede indeks parçası üretim CPU maliyetini azalt | Dar kazanç uygulandı/ölçüldü: son body-only sürüm, aynı10k/dört dengeli çift, yazma30,859→27,290s; yazma+bakım31,908→28,241s (-%11,49),4/4 hızlı, çift farkları-%10,81…11,93. CPU-%11,86/4/4. Son bakım0,988→0,940s,3/4 düşük; ayrı merge optimizasyonu sayılmaz. Geçici8KiB tablo/63-bit scalar keys yalnız BodyCache'te, query grams eski haliyle aynı. Yeni format/feature yok,122 core dosyanın byte'ı sekiz koşuda aynı. OS private commit9,021→9,107MiB (+88KiB),WS12,986→13,014MiB (+28KiB); sıfır RAM artışı şartı kanıtlanmadı. Read matrix33 düşük/5 yüksek; genel/default kapısı açık, feature kapalı. Payload tekrar çöz/kodla, merge tamponları, kalan arama artışları, query/normal API RAM'i, disk/eski nesil ve concurrent/cold açık. SQLite önceki adımın iki unpaired referansı, son FDB'yle paired gate değil. [Sonuç/kanıtlar](GRAM-BODY-WRITES-10K-20261016.md) |
| Dar aramada gereksiz body normalleştirmesini azalt | Dar kazanç uygulandı/ölçüldü: aynı10k/dört dengeli pair/800 örnekte quixote /28/10 -%10,41, /57/50 -%15,04; ishmael /130/1 -%7,28, /60/200 -%4,17; dördü4/4 hızlı. Geniş38-case21 düşük/17 yüksek, AND quixote whale -%23,35/4/4. ASCII raw pozitif kanıt56 adayda815→0 body fold/scan yapar;56 payload read/CRC/schema korunur.64KiB/8 denemede6 başarı sınırı düşük isabetli ek taramayı durdurur; yeni büyük cache/feature/format yok. Builder-budget içinde/default kapalı; genel kabul kapısı geçmedi.17 artış, yazma+bakım önceki+%3 gözlemi, warm query/normal API RAM'i, disk/eski nesil ve concurrent/cold açık. İlk kullanım proses belleği ve ölçüm sınırları raporda; indeksleme RAM'i/yazma yeniden ölçülmedi. [Sonuç/ham kanıtlar](RAW-TEXT-PROOF-10K-20261016.md) |
| Arama artışlarını aynı sürüm kontrolüyle değerlendir | Ölçüm aracı düzeltildi/doğrulandı; motor hız işi açık. Aynı SHA'lı reader üç çiftte8/8 pooled p95 yüksek (+%0,31–6,39); bunlar kod regresyonu olamaz. Dengeli dört çift/800 örnekte aynı kod3 düşük/5 yüksek (-%1,79…+%2,94); sıfır fark veya otomatik tolerans iddiası yok. Runner artık dengeli4 çift, açık candidate, sıra/hash tasarımı ve örnek doğrulaması; özet paired fark/medyanını da verir ve eski logları okur. Dar sorguya adaptif pencere prototipi hedefte56→56 read/0→0 window hit,38-case9 düşük/29 yüksek; reddedildi ve runtime geri alındı. Source lease korunur; önceki24 artış, yazma/bakım, RAM/disk ve concurrent/cold işler açık. Yeni corpus/DB reset/RAM/write/SQLite performans ölçümü yok. [Kanıt ve sınırlar](BENCHMARK-CONTROL-10K-20261015.md) |
| Tek-sonuç sayfasının guard maliyetini azalt | Dar adım uygulandı/ölçüldü: Windows/plain-path immutable source doğrulamadan önce read-share ile açılır, aynı handle index sahibi/merge worker ömrünce tutulur; write/truncate/delete/rename engellenir. WAL/Legacy/reparse/non-Windows eski guard'da; index guard ve ilk10k source doğrulaması kalır. Aynı10k/üç pair/600 örnekte the /0/1 p95 0,8123→0,2459ms (-%69,73), ileri5000/9950 -%18,27/-%20,34; üçü3/3 hızlı. Geniş38-case14 düşük/24 yüksek; the /0/1 burada da -%72,18/3/3. Üç yoğun-sayfa artışı, diğer matrix artışları ve önceki+%66,89 farkın tam nedensel açıklaması açık. Ayrı instrumented guard0,3663→0,0357ms, source path çağrı5→0/index5→5. Genel kapı geçmedi/default kapalı; RAM/write sonuçları son raporda. Query tepe/normal API, yazma-bakım, RAM/disk, uzun tarama ve concurrent/cold yük açık. [Son kanıt/sınırlar](SOURCE-LEASE-10K-20261015.md), [ilk teşhis/reddedilen deneyler](PAGE-GUARD-10K-20261015.md) |
| Değişmeyen Unicode işaretlerinde normalleştirme aralığını koru | Dar kazanç ölçüldü: mevcut10k/üç çift matrix35 düşük/3 yüksek; zx309→222ms, the zx307→236ms, quixote whale5,83→5,00ms, üçü3/3 hızlı. Query normalizer'ı aynı byte sonucunu daha az aralıkla kopyalar; ek buffer/cache/format/feature yok, builder-budget içinde. Üç çift indexing private9,13→9,18MiB, bakım7,54→8,48MiB, WS12,84→13,21MiB; disk/hash aynı. Toplam32,34→32,03s,2/3 hızlı; genel yazma kazancı değildir. Tek-sonuç the sayfası+%66,89/0,5843ms,0/3 hızlı; diğer iki p95 artışı da açık. Genel RAM/hız kapısı geçmedi/default'a geçiş reddedildi. Query tepe/normal API/genel heap, kalan normalleştirme-tarama, yazma/bakım, RAM/disk ve concurrent/cold yük açık. [Kanıt/sınırlar](CHANGED-RUNS-10K-20261015.md) |
| Metin parçalarının ikinci kopyasını azalt | İki deneme reddedildi ve runtime değişiklikleri geri alındı (2026-10-15). Aynı mevcut10k/üçer pair doğrudan fold32 düşük/6 yüksek; tek residual varyant21/17. quixote whale her iki batch'te0/3 hızlı (+%6,14/+%10,58). İlk the zx3/3 kazancı genel kapıya yetmedi. Prototype Unicode/chunk/reuse ve final default/feature/all-features all-targets, cut/byte/kill/interop tests, Clippy/fmt ve38-case check geçti. Normal release önceki kodla yeniden build edildi. DB yazması/RAM checkpoint yok; önceki RAM sayıları yeni ölçüm değil. Corpus/FDB/cache/SQL hash/format aynı. CPU normalleştirme/tarama, mevcut arama artışları, yazma/bakım, RAM/disk ve eşzamanlı yük açık. [Reddedilme kanıtı](CHUNK-COPY-REJECTED-10K-20261015.md) |
| Uzun/olumsuz taramada normalleştirmeyi hızlandır | İlk dar adım ölçüldü: builder-budget içinde güvenli32-bayt ASCII-prefix kontrolü; Unicode/fold/source/format aynı. Aynı10k/3 pair zx336→309ms/the zx337→317ms,3/3 hızlı; matrix33 düşük/5 yüksek. Default geçişi reddedildi, beş yeni artış açık. Tek RAM/write teyidi private9,14MiB/WS13,34MiB; bakım+0,66MiB, toplam32,36→33,47s; disk aynı. Tek instrumented the zx normalleştirme135,81ms/scan20,84ms; normalizer payı hâlâ büyük, ayrı paired normalizer oranı iddia edilmez. Query tepe ve1000 aktif performansı tekrar ölçülmedi. Tests/Clippy/fmt geçti. Kalan normalleştirme/tarama, chunk kopyaları, genel yazma/bakım/RAM/disk/eşzamanlı yük açık. [Kanıt/sınırlar](WIDE-FOLD-10K-20261014.md) |
| İki karakterli olumlu aramada genel indeks kanıtı | Dar adım ölçüldü: tam-live gramdan scoped alt-parça kanıtı, 64 entry/16 KiB/run; aynı 10k/3 pair he132,27→1,81ms,3/3 hızlı. Profilde candidate payload read0; source kontrolü/hit hydration korunur. Yazma+bakım32,87→32,25s,3/3 düşük; indexing private9,09MiB (+4KiB medyan), disk aynı, WS+0,30MiB. Genel matrix15 düşük/23 yüksek; kanıtsız/olumsuz iki karakter ve uzun parça taraması, diğer p95 artışları açık. Default kapısı başarısız. Default/feature/all-features tests, cut/byte/kill/interop, Clippy/fmt ve son38-case check geçti. Query tepe/1000 aktif belge performansı bu adımda ölçülmedi. [Kanıt/sınırlar](SHORT-COVERAGE-10K-20261014.md) |
| Kesin gram kanıtıyla payload tekrarını azalt | Dar adım ölçüldü: genel3 karakterli parçalar, early-out öncesi source izni/fingerprint guard ve mevcut posting-count. Son3 pair the145→2,30ms/man147→2,63ms; offset5000/9950 önceki büyük fark3/3 çözüldü. Geniş matrix15/38 p95 düşük,23 yüksek; aktif-data17/18 düşük,1 yüksek. Diğer uzun/2 karakterli tarama ve yeni p95 artışları açık. Son tek RAM teyidi private9,19MiB/WS13,32MiB; disk aynı. Önceki3 pair bakım+0,40MiB/yazma+bakım33,22→33,79s, hız işi kapanmadı. İlk source kontrolü228ms search'ün225ms'si; sıfırlanmadı. Default kapısı başarısız/feature aynı. [Kanıt/sınırlar](COVERED-TEXT-10K-20261014.md) |
| Düşük RAM profilinde hızı geri kazan | İlk adım geçmiş kanıtı: borrowed record cursor, genel tüm-gram preflight, mevcut1MiB içinde64KiB hot posting cache. İlk üç paired aktif-data15/18/geniş36/38 p95 düşük;10k indexing private9,07→9,09MiB, disk aynı/bakım+0,46MiB. Yazma+bakım40,69→40,16s (2/3 hızlı) bir hız kapısı değildi. O adımın the ileri offset5000/9950 farkı sonraki covered ölçümünde3/3 çözüldü; güncel23/38 p95 artışı ve iki karakter/uzun tarama işi üst satırdadır. Toplam hız hedefi/default geçiş açık. [İlk adım kanıtı](BOUNDED-SPEED-10K-20261014.md), [son durum](COVERED-TEXT-10K-20261014.md) |
| Giriş/delta/bakımı ortak bütçeye al, disk sözlüğünü sayfalardan oku | Uygulandı/ölçüldü; `experimental-builder-budget`+`commit_streamed`, default kapalı. Corrected3 çiftte indexing111,13→9,13MiB,bakım100,11→7,94MiB,idle21,48→1,74MiB; WS114,73→12,92MiB. SQL3 ayrı referans indexing8,40MiB. Toplam27,24→31,54s (3/3 yavaş), immutable matrix17/21 vewrite-after1000 belge0/18 p95: default hız kapısı reddedildi. Genel Bloom/AND/cache/incremental gram admission var; cold-cache/budget deadlock'u düzeltildi, tam suite/mixed writer/cut/kill/old-reader kontrolleri geçti. Normal API/global heap hedefi kapanmadı. [Kanıt/sınırlar](BOUNDED-MEMORY-10K-20261014.md) |
| SQLite / FDB RAM farkının kök nedenini önce teşhis et | Teşhis raporlandı; RAM çözümü açık. Eski tek allocation snapshot'ında belge27,35MiB+delta indeks29,48MiB+metadata12,51MiB; güncel live5 run103.667 terim girdisi.16MiB payload eşiği total RAM değil ve bu corpus'ta~26MiB'ye aşılır.500 owned girişin yalnız title/text'i12,20–13,08MiB; SQLite aynı transaction'ı tek belgeyle besliyor. Genel field/link indeks kapsamı da farklı. Yeni performans/uygulama iddiası yok. [Kanıt ve sınırlar](ROOT-CAUSE-MEMORY-10K-20261014.md) |
| Tam belge/folded metin çift kopyasını WAL referansıyla azalt | Ölçüldü, hız şartı nedeniyle reddedildi/default kapalı: indexing111→84MiB,bakım102→76MiB; toplam31,09→31,89s,2/3 yavaş. Delta arama1/18 p95 düşük,pozitif16 case3/3 yavaş. Tek-frame encode ve harness row/resident ayrımı düzeltildi; kalıcı codec aynı. Hızlı tek temsilli belge tasarımı açık. [Kanıt](COPY-REDUCTION-10K-20261013.md) |
| Mevcut builder/write bütçeli kombinasyonu ölç | Ölçüldü ve hızdan ödün vermeyen çözüm olarak reddedildi: üç çift, toplam35,17→38,59s (3/3 yavaş), bakım private99,86→96,07MiB.256KiB/2MiB pool'ları total RSS sınırı değil; peak reservation64KiB/1MiB, waits0. Default kapalı kaldı. Gerçek delta/body/caller/metadata bütçesi açık. [Kanıt](BUDGET-MEASURE-10K-20261013.md) |
| Delta RAM gram sözlüğündeki scope/kopya tekrarını azalt | Dar adım ölçüldü: nested scope sözlüğü,3/3 pair toplam hızlı ve indexing/bakım private düşük. Medyan toplam42,995→36,803s, indexing111,75→110,36MiB, bakım100,54→97,58MiB; disk aynı. Delta alt örnek arama17/18 p95 düşük,tek+14µs fark kayıtlı. Belge/body kopyaları hâlâ açık. [Kanıt](DELTA-DICTIONARY-10K-20261013.md) |
| Çok sayıda küçük payload okumasını birleştir | Adaptive kullanım kontrolü tamamlandı; presumption overread%22→%0,78. Yeni38-case36 düşük/2 yüksek; önceki11'in tümü düşük. Kalan artışlar2026-10-13 kullanıcı kararıyla ölçüm dalgalanması olarak kabul edildi. Grouped-read default kapalı kalır. [Kanıt ve kabul](REGRESSION-READS-10K-20261013.md) |
| Normalleştirmede ASCII aralığı ve query token maliyetini azalt | Güvenli8-byte prefix araması ve query fold_into eklendi; tüm Unicode scalar/hizalama/chunk testleri geçti. Aynı grouped features ile11 önceki case p95%4–18 düşük. Reusable finder deneyi17/21 sonucu ile reddedildi ve kaldırıldı. [Kanıt](REGRESSION-READS-10K-20261013.md) |
| OFF1 UTF-8 doğrulama maliyetini düşür | Dar adım ölçüldü: first208→173 ms,5/5 pair hızlı; dört warm query p95%18–27 düşük,12/12 pair hızlı. WS first-use~30 MiB; sampled proses ölçümü, geniş RAM kapısı değil. Strict ilk5 median%3,73 arttı, ek5 karışık; genel opening işi kapanmadı. [Kanıt](UTF8-READS-10K-20261013.md) |
| Kaynak frame tamponunu reuse/pre-plan ile değiştir | Reddedildi ve runtime değişiklikleri geri alındı. İlk deneme RAM+8 MiB; ikinci deneme ilk search hızlı fakat bütün warm p95 eşleşmeleri yavaş. [Kanıt](FRAME-PLAN-10K-20261013.md). Reuse kaldırıldı; bounded multi-frame testi ve proses bellek gözlem aracı korundu |
| Ortak IEEE checksum aritmetiğini hızlandır | Bu dar adım ölçüldü: ilk search381→229 ms, strict open962→582 ms;5/5 eşleşme hızlı. Dört warm query p95 toplam12/12 eşleşmede düşük. CRC/formatlar ve DB hash'leri aynı. [Kanıt](CRC-READS-10K-20261013.md). RAM/yazma ve38-query tekrarlı performans kapsamı açık |
| İlk deferred aramada kaynak belge kopyalama maliyetini kaldır | Bu dar adım ölçüldü: aynı record-read API ve5 pending source ile5/5 çiftte ilk search529→336 ms (%36,44); open+get+search602→407 ms. [Kanıt](BORROWED-SOURCE-10K-20261013.md). Bütün kaynak kontrolleri korunur; CRC/read ve genel açılış işi açık |
| Açılış CRC / source replay / index maliyetini ölç ve tekrarları azalt | Strict ilk adım946→747 ms. Opt-in deferred ikinci adım:752→319 ms, ilk get92 /ilk search360 ms; [kanıt](DEFERRED-OPEN-10K-20261012.md). VFY blok kontrolü298→60 ms open,383→150 ms open+get; [kanıt](BLOCK-OPEN-10K-20261012.md). Record-get ilk107→0,20 ms; [kanıt](SOURCE-RECORD-READS-10K-20261013.md). Warm search%1–4 arttı, opt-in/default değil. SQLite~9 ms seviyesine ulaşılmadı; kaynak search kökeni ve directory maliyeti açık |
| Güncel kodda yazma + flush/merge bakımını SQLite ile yeniden ölç | 2026-10-13 merge+bitmap profili karşılaştırıldı: FDB35,81+1,92=37,73s / SQLite44,95+1,46=46,41s; FDB2/3 çift hızlı, bakım SQLite hızlı. Bu tarihli sonuç korunur. Güncel source-lease builder-budget/streamed profili üç çiftte kontrol26,667+1,007 / yeni27,445+0,991s; toplam medyan27,609→28,436s (+%3,00),1/3 hızlı; bakım2/3 hızlı. Güncel profilin SQLite yazma eşleştirmesi bu adımda yapılmadı; genel yazma/bakım işi açık. [Güncel kontrol](SOURCE-LEASE-10K-20261015.md), [SQLite ile önceki profil](WRITE-MAINTENANCE-10K-20261013.md) |
| Bütün temel test başlıklarını birlikte raporla | Açık: yazma, bakım, get,38 arama/sayfa, açılış, RAM ve disk |
| Arama senaryolarındaki p95 artışını incele | Kapalı —2026-10-13 kullanıcı kararıyla kalan grouped2/default3 artış ölçüm dalgalanması kabul edildi. Ham36/2 ve35/3 sonuçlar korunur; ek istatistiksel kanıt veya bütün iş yüklerinde hız garantisi değildir. [Ölçüm ve kullanıcı kabulü](REGRESSION-READS-10K-20261013.md) |
| RAM: boşta ve indeksleme/bakım tepesini düşür | Bu10k streamed/sayfalı profilin düşük RAM sonucu2026-10-14 kullanıcı tarafından kabul edildi; öncelik bu seviyede hız. Son source-lease3 çift indexing private9,195→9,129MiB ve bakım8,457→8,145MiB, ikisi3/3 düşük; idle1,695→1,527MiB,2/3 düşük. OS peak WS13,000→13,027MiB,2/3 yüksek; sıfır RAM artışı iddiası yok. Bunlar arama/oracle olmayan ayrı proses sayaçları; query tepe belleği, kernel handle belleği ve toplam sistem RAM'i değildir. Normal/default API, genel heap, büyük entry, WAL replay ve concurrent kapsamı açık; düşük RAM kabulü default'a geçiş değildir. [Son üç çift](SOURCE-LEASE-10K-20261015.md), [önceki üç çift](CHANGED-RUNS-10K-20261015.md), [kök neden/uygulama](BOUNDED-MEMORY-10K-20261014.md) |
| Disk: source/index kopyaları ve tutulan eski nesiller | Standart eski nesil temizliği tamam:105 dosya/622.859.277 B, retained core972.664.975→349.805.698 B; source+WAL267.900.709 B, idx+MET81.904.473 B, ready516 B. Canlı byte/hash aynı,10k full-doc ve38-case oracle geçti; eski okuyucu da açar. İki streamed ingestte aynı350MB; current17 core hash-eşit. GC güvenliği, root/source/WAL bozulması, interrupted unlink, explicit/automatic kill, reader/worker pin ve retry testleri geçti. Genel sıkıştırma, dynamic temp/orphan sınırları, concurrent/cold ve genel RAM/hız kapıları açık. [Güncel ölçüm ve sınırlar](RECLAIM-10K-20261017.md) |
| Bakım sürerken okuma/yazma gecikmesini ölç | Açık; eşzamanlı yük ve bloklanma kapsamlı ölçülmedi |
| Soğuk cache ve farklı iş yüklerinde sonuçları doğrula | Açık; mevcut karşılaştırmalar sıcak OS cache, belirli SQLite ayarları |

Bağımsız Rust motoru; SDB/Fundops/Frunner entegrasyonu kapsam dışı.
0.1 fixture korunur; `.fdb/.ready` kaynak, `.idx` cache. Valid cache bozuk kaynağı
gizleyemez. Yeni format gerekirse sürüm/uyumluluk testleri şarttır.
