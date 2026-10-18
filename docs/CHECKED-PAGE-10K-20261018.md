# Contains sonrası doğrulanmış payload tekrarını azaltma — 2026-10-18

Sonuç: varsayılan geçiş reddedildi. Doğrulanmış byte'ları tekrar kullanmak
quixote ilk sayfasında tekrarlanabilir kazanç verdi; bütün sorgu setinde
gerilemesizlik ve RAM artmaması sağlanmadı. Son izolasyonlu prototipin kodu
ve beş yeni testi [patch olarak](benchmarks/checked-page-20261018-prototype.patch)
saklanır; runtime97f6ef7'a byte-eşit geri döndü. Disk/kaynak/
format değişmez. Bu karar, ikinci okumanın faydasız olduğu anlamına gelmez;
genel kullanıma uygun bir uygulama henüz kanıtlanmadı.

## Hipotez ve tek prototip

Önceki mevcut-sürüm profilinde quixote için56 aday CRC/yapı/UTF-8/identity
kontrolü ve50 sonuç decode/CRC; holmes için1285+50 çağrı vardı. zx için
10000+0, the/200 için0+200: ikinci payload okuması yalnız doğrulanan ve
sonuçta dönen pozitif belgelerde hedeflenebilir. Negatif sorguya kazanç
vaat edilmez. [Önceki ölçüm](VALIDATION-CHUNK-10K-20261018.md).

Yeni CheckedPage bir sorguya aittir; offset0/limit<=50, grouped hydration
ve en az bir3 Unicode scalar'dan uzun sorgu parçasında kullanılır. Karar
sözcük adına göre verilmez. Diğer sayfalar ve no-default reference eski
yolu kullanır. Binary contains CRC, schema/UTF-8 ve key/version kontrolü
sonrası olumlu eşleşmenin aynı byte slice'ını private Box<[u8]>'a kopyalar.
Yeni dosya okumasında geçerli sayılacak bir checksum muafiyeti üretilmez.

Bütün Run'lar ortak alana yazar; Run başına belge önbelleği yoktur. Global
Key sırasıyla en fazla50 kayıt ve toplam2MiB encoded payload byte'ı tutulur;
bütçe veya boyut aşıldığında sonuç normal okuma/decode yolundan tamamlanır.
Tutulan kaydın Run/slot kimliği sonuç seçimiyle eşleşmelidir. Sayısal slot
sırası farklı Run'lar arasında anahtar sırası yerine kullanılmaz. Delta,
visibility, tombstone, total, union, filtre ve global sıralama aynı kalır.

Saklanan sonuç ikinci dosya okuması ve CRC olmadan, aynı immutable owned
byte'lardan çözülür. Schema/UTF-8 bu çözümlemede tekrar kontrol edilir;
bu tekrarın da kaldırıldığı iddia edilmez. Seçilmeyen kopyalar ve bütün alan
sorgu bitince düşer.2MiB yalnız retained encoded byte sınırıdır; en fazla50
Key/map girdisi, mevcut query scratch, dönen owned belgeler ve allocator
maliyeti ayrıca vardır. Bu sayı toplam proses RAM garantisi değildir.
Sınır sorgu başınadır; eşzamanlı sorgular arasında ortak2MiB kota değildir.
Concurrent tepe bellek ve gecikme bu ölçümün kapsamı dışındadır.

Kalıcı kaynak/cache/WAL/manifest/payload/protokol biçimi, dependency ve
feature değişmez.0.1 fixture korunur. Eski JSON kaynağı normal decode'a
gider. rkyv benchmark overlay bu capture callback'ini kullanmaz.

## Koşullar ve karar sırası

Aynı mevcut ilk10.000 belge/8 kitap,264.842.858B text,2.057–51.197B pasaj.
Corpus hazırlama/indirme yok. Sabit FDB18 dosya/349.805.698B, sequence20,
delta0/5 Run; SQLite583.860.224B. DB reset/repack/kopya/başka disk yok.
DB+SQLite+iki0.1 fixture toplam21 dosyada önce/sonra SHA-256 izlenir.

Normal dar kontrol: mevcut38-case oracle'dan0=quixote,11=holmes,20=zx,
31=the/200;4 süreç çifti×4 dönüşümlü strict blok×25 tur=400 örnek/
sorgu/variant. Her blokta normal strict source/index open ve ısınma süre
dışında; full document/field/link/version/total/sıra oracle'ı süre dışında.
OS cache sıcak; eviction/gerçek-cold/concurrent yok. Diagnostic timer ve
ek thread sayaçları normal ölçümde kapalı.16 blok16 bağımsız süreç değildir.
Pooled p95, süreç çiftleri ve tipik blok medyanları ayrı yorumlanır.
Windows/MSVC, Ryzen7 7840HS, Rust1.99.0/LLVM23.1.1; release thin-LTO,
codegen-units1. Affinity mask4 (tek mantıksal işlemci), Balanced güç planı;
frekans, SMT ve arka plan süreçleri izole edilmedi.

Dar kazanç tekrarlanırsa bütün38-case, ayrı oracle'sız query/get process
peak private commit/working set ve diagnostic sayımlarına geçilir. Bunlar
fazın canlı Rust heap'i veya fiziksel toplam RAM diye sunulmaz. Kod/yerleşim,
frekans/SMT/arka plan payları tamamen ayrılmaz. Normal sonuçla diagnostic
timer maliyetli süreler karıştırılmaz; nested span'ler toplanmaz.

Kontrol, SHA doğrulanmış önceki immutable okuyucudur; cb2c19e ile97f6ef7
library/dependency/harness kaynakları aynıdır. Yeni normal+profile build,
kaynak ve executable SHA'ları
[manifestte](benchmarks/checked-page-20261018-build-manifest.json) saklanır.

İlk dondurulmuş adayda capture kullanılmayan sayfanın hydration Vec'i
önceki exact capacity yerine büyüyerek doluyordu. Kaynak incelemesinde bu
gereksiz fark bulundu; final adayda reserve_exact ile önceki ayırma davranışı
korunur. İlk program/loglar değiştirilmez; ilk patch ayrıca arşivlenir.
Final karar, düzeltilmiş programın yeniden ölçümüne dayanmalıdır. Kaynak
düzeltmesi immutable executable ile süren ölçümü değiştirmez; ölçüm sırasında
derleme/test çalıştırılmaz.

Exact reserve düzeltmeli ikinci400-örnek serisinde quixote p95-%18,70,
tipik blok-%16,57;4/4 süreç ve16/16 blok hızlıdır. holmes p95-%11,25,
tipik-%2,20;3/4 süreç ve14/16 blok hızlı. zx-%1,43/the200-%8,94 farkları
capture kazancı sayılmaz. [İkinci dar seri](benchmarks/checked-page-final-focus-10k-20261018-summary.json).
Ardından kalan off-page kopyaların fallback/delta belge hazırlığından önce
düşmesi sağlandı. Son release programı bu kaynakla ayrıca dondurulur;
dar iki pozitif sorgu400 örnek/variant, geniş38-case ve4-çift bellek/profil
ayrı ölçülür. İlk iki sürümün dosya ve sonuçları son release sonucu yerine
sunulmaz. Son manifest: [checked-page-release](benchmarks/checked-page-release-20261018-build-manifest.json).

## Ön doğrulama

Üç yeni CheckedPage testi: global Key sırası/full nested field/link/version,
payload-byte/slot sınırı ve oversized fallback, owned byte bağımsızlığı ve
Run/slot kimliği. Yeni capture testi bozulma/kesilmenin admission öncesi
reddini denetler. Yeni engine testi üç farklı sıralı Run, delta güncelleme/
silme/yeni kayıt, iki collection, AND/negatif/covered/boş sorgu, exact/range/
link filtreleri ve küçük/büyük/deep sayfalarda direct-slot ile full equality
ve get/version doğrulaması yapar. Bunlar küçük unit fixture'lardır; corpus
ve sabit ölçüm veritabanı üretmez/değiştirmez. Bu tur7 checked_* seçili test
ve ayrıca7 önceki window testi geçti; all-features/all-targets Clippy temiz.

## İlk dondurulmuş aday (final karar değildir)

| Sorgu, offset/limit | Kontrol p95 ms | Aday p95 ms | Fark | Tipik blok fark | Hızlı çift/blok |
|---|---:|---:|---:|---:|---:|
| quixote,0/50 |4,7670|3,6528|-%23,37|-%16,69|4/4;16/16|
| holmes,0/50 |41,3366|38,7489|-%6,26|-%2,46|2/4;13/16|
| zx,0/50 |227,0751|209,5231|-%7,73|-%1,76|3/4;13/16|
| the,0/200 |7,3587|6,4469|-%12,39|-%4,71|2/4;11/16|

Her satır400 örnek/variant. Capture kullanmayan zx/the/200'deki fark
capture algoritmasına yazılmaz. holmes süreç p95 kazancı2/4'tür; quixote
dar hedefte4/4 ve16/16 tutarlıdır. [Ham çift/bloklar](benchmarks/checked-page-focus-10k-20261018-summary.json).
İlk iki oracle'sız bellek çiftinde private commit medyanı9.197.568→
9.318.400B (+120.832B/118KiB,%1,31), working set13.510.656→13.606.912B
(+96.256B/94KiB,%0,71); ikisi de2/2 çiftte artmıştır. Bellek aynı kaldı
iddiası yok; bunlar final düzeltilmiş sürümün4-çift sonucu değildir.
[İlk bellek kontrolü](benchmarks/checked-page-initial-memory-10k-20261018-summary.json).

## Açık işler

Yazma/bakım, genel arama sapmaları, normal Vec/query/caller RAM'i, açılış,
disk sıkıştırma, eşzamanlı/gerçek-cold yük açık. Dar prototip bunları kapatmaz.

## Release adayının geniş kontrolü ve izolasyon düzeltmesi

Erken off-page drop içeren release adayı dar400-örnek sorgularda quixote
p95-%14,00/tipik-%16,24 (4/4 süreç,16/16 blok), holmes-%4,29/tipik-%1,16
(3/4 süreç,9/16 blok) verdi. Geniş38-case4×4×5 tur kontrolünde34 p95
düşük/4 yüksek: he+%10,01, the5000/50+%4,16, ishmael130/1+%4,12,
quixote57/50+%15,95. Primary sorgu80, sayfa sorgusu32 örnek/variant.
Tam oracle geçti; son konsol tablosunda Türkçe karakter basarken Python
cp1252 hatası oluştu. Summary ve21-file before/after hash karşılaştırması
hata öncesi tamamlandı. Veriler yeniden çalıştırılmadı; sonraki alt süreçler
explicit -X utf8 ile çalıştırılır.

Yedi sorguluk400-örnek tekrarda he+%6,23/tipik+%3,58 (3/4 çift yüksek),
the/200+%9,47/tipik+%6,14 (3/4 çift ve14/16 blok yüksek). quixote0/50
-%13,85/tipik-%14,90 (3/4 süreç,16/16 blok hızlı); sonuçsuz quixote57/50
+%4,72/tipik+%2,52. Bu artışlar dalgalanma diye kapatılmadı. Release adayı
bu haliyle varsayılana alınamaz. [Geniş matris](benchmarks/checked-page-release-matrix-10k-20261018-summary.json),
[yoğun tekrar](benchmarks/checked-page-release-regressions-10k-20261018-summary.json).

Dört oracle'sız query/get bellek çiftinde private medyan9.132.032→9.250.816B
(+118.784B/116KiB,%1,30), working set13.203.456→13.375.488B
(+172.032B/168KiB,%1,30). İkisi de4/4 süreçte artar; RAM gerilemesizliği
kanıtlanmadı. [Bellek](benchmarks/checked-page-release-memory-10k-20261018-summary.json).

Altı diagnostic örnek/variantta quixote disk.read_at131→81,
payload.decode_crc_identity50→0, reuse_checked0→50; candidate CRC56→56.
holmes1514→1467 read API çağrısı,50→0 ikinci decode/CRC,1285→1285
candidate CRC verir. quixote sonuç hazırlama instrumented medyan1,397→
0,518ms; nested span/diagnostic maliyeti nedeniyle normal p95 yerine
geçmez. API sayısı fiziksel disk transferi değildir. zx ve the200 iş sayıları
aynıdır. [Profil](benchmarks/checked-page-release-profile-10k-20261018-summary.json).

Kod incelemesinde capture kullanmayan sayfanın selected Vec girdisinin de
(run,slot)'tan(run,slot,position)'a büyüdüğü görüldü. Bunun gecikmedeki payı
tek başına kanıtlanmış değildir. Tek bir izolasyon düzeltmesi: compile-time
CAPTURE=false yolu eski pair slot/ordinal position/exact capacity biçimini
korur; yalnız true yolu ayrı fallback-position listesi taşır. İlk sayfa
kararı wrapper'da verilir; source/validation/query gövdesi yine bir kez
çalışır. Önceki release kaynak SHA'sı korunan engine yedeğiyle eşleştirildi.
Yeni7 checked_* test geçti. Sonuç, izolasyonlu ayrı executable ile ölçülmelidir;
önceki programın hızlı/yavaş sonuçları yeni koda mal edilmez.


## Son izolasyonlu prototip ve karar

Normal8-case×400 örnek/variant kontrolde8/8 p95 düşük: quixote
4,2165→3,7480ms (-%11,11), holmes43,3872→37,5214ms (-%13,52),
ikisi4/4 süreç ve15/16 blok hızlı. Tipik blok farkları-%17,92/-%3,82.
he-%9,36 ve the200-%9,31, ikisi4/4 süreç hızlı. Önceki dar artışlar
bu tekrarda sürmedi; bütün çalışma bağlamlarında çözüldüğü iddia edilmez.
[400 örnekli sekiz sorgu](benchmarks/checked-page-isolated-focus-10k-20261018-summary.json),
[son immutable build](benchmarks/checked-page-isolated-20261018-build-manifest.json).

Son geniş38-case kontrolü15 p95 düşük/23 yüksek verdi. quixote p95
4,7491→4,4562ms (-%6,17;3/4 süreç), tipik-%10,29/13 blok hızlı.
holmes p95-%9,28 olsa da1/4 süreç hızlı ve tipik fark+%0,05'tir.
zx+%15,14/tipik+%4,23; the200+%10,83/tipik-%0,85. Uzun negatif
zxzx… sorgusu bütün4 süreçte yüksek p95 verir. Diğer artışların kod,
yerleşim, cache/geçmiş ve ortam payları ayrılmadı; bütün farklar tek bir
CPU nedeni veya dalgalanma diye sunulmaz. Dar sekiz sorgu sonucu, geniş
23 artışı geçersiz kılmaz. Geniş kontrolün bütünü aşağıdadır.

| Sorgu | Offset/limit | n/variant | Kontrol p95 ms | Aday p95 ms | Fark | Hızlı çift | Tipik blok fark |
|---|---:|---:|---:|---:|---:|---:|---:|
|quixote|0/50|80|4,7491|4,4562|-6.17%|3/4|-10.29%|
|zeuglodon|0/50|80|4,6204|5,3026|+14.76%|2/4|-7.88%|
|beagle|0/50|80|32,2108|31,7912|-1.30%|3/4|+1.03%|
|mesopotamia|0/50|80|5,4767|5,2335|-4.44%|3/4|-2.16%|
|transubstantiation|0/50|80|86,0136|93,7470|+8.99%|2/4|+7.85%|
|harpoon|0/50|80|105,2145|120,4078|+14.44%|2/4|+2.30%|
|ishmael|0/50|80|13,5432|12,5104|-7.63%|2/4|-6.50%|
|invertebrata|0/50|80|19,6700|24,6993|+25.57%|1/4|-4.31%|
|darcy|0/50|80|80,0705|94,2130|+17.66%|1/4|-1.33%|
|elizabeth|0/50|80|75,8890|117,8719|+55.32%|1/4|+0.78%|
|watson|0/50|80|75,8406|79,8673|+5.31%|2/4|-2.03%|
|holmes|0/50|80|58,5008|53,0727|-9.28%|1/4|+0.05%|
|frankenstein|0/50|80|27,2026|30,6781|+12.78%|1/4|-1.52%|
|galapagos|0/50|80|19,1057|22,5769|+18.17%|2/4|-5.63%|
|fossil|0/50|80|55,6067|58,5591|+5.31%|2/4|+1.23%|
|geologist|0/50|80|33,5267|32,7723|-2.25%|2/4|-2.96%|
|the|0/50|80|1,7836|2,2583|+26.61%|1/4|+3.82%|
|man|0/50|80|3,2148|2,6637|-17.14%|1/4|-8.61%|
|he|0/50|80|1,7760|1,7253|-2.85%|1/4|+0.79%|
|zzxqnever|0/50|80|0,0588|0,0612|+4.08%|1/4|-5.69%|
|zx|0/50|80|221,3039|254,8130|+15.14%|1/4|+4.23%|
|the zx|0/50|80|246,0917|312,5520|+27.01%|2/4|+1.64%|
|quixote whale|0/50|80|5,8420|4,7608|-18.51%|2/4|-12.21%|
|holmes elementary|0/50|80|61,8638|73,0390|+18.06%|1/4|-1.42%|
|fossil geologist|0/50|80|19,4672|23,2705|+19.54%|1/4|-4.02%|
|whale harpoon|0/50|80|127,2205|102,6905|-19.28%|2/4|-0.25%|
|QUİXOTE|0/50|80|7,7730|4,6967|-39.58%|2/4|-22.29%|
|İSHMAEL|0/50|80|11,7767|11,6072|-1.44%|3/4|-7.36%|
|zxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzxzx|0/50|80|0,0527|0,0595|+12.90%|0/4|+9.26%|
|presumption|0/50|80|124,8884|161,0084|+28.92%|1/4|+0.88%|
|the|0/1|32|0,4038|0,3497|-13.40%|3/4|-5.13%|
|the|0/200|32|7,9029|8,7588|+10.83%|2/4|-0.85%|
|the|5000/50|32|3,0593|3,7323|+22.00%|2/4|-7.46%|
|the|9950/50|32|2,8111|2,0535|-26.95%|3/4|+1.18%|
|ishmael|130/1|32|9,9767|11,0303|+10.56%|1/4|+4.39%|
|ishmael|60/200|32|28,2621|30,4662|+7.80%|2/4|+4.22%|
|quixote|28/10|32|3,4339|3,0722|-10.53%|3/4|-4.21%|
|quixote|57/50|32|4,2924|4,4336|+3.29%|2/4|+6.21%|

[Ham geniş sonuç/çift/bloklar](benchmarks/checked-page-isolated-matrix-10k-20261018-summary.json).
Bu tablo önceki release adayının34/4 tablosu değildir; adaylar ve oturumlar
birleştirilmez. P95 örneklemin kuyruğunu, blok medyanı tipik örneği anlatır.
Bu ikisi ters yöndeyken birini seçip diğerini yok saymayız.

Dört oracle'sız query/get proses çiftinde private medyan9.121.792→9.187.328B
(8,699→8,762MiB, +65.536B/64KiB/%0,72); working set13.205.504→13.379.584B
(12,594→12,760MiB, +174.080B/170KiB/%1,32). İkisi de4/4 süreçte arttı.
Bunlar toplam proses ömrü tepe sayaçlarıdır; canlı Rust heap'i veya fiziksel
RAM toplamı değildir. İki oracle worker'ın belleği bu sayılara dahil değildir;
ayrı, oracle yüklemeyen prosesler ölçülür. 2MiB byte bütçesi, measured peak farkının
64KiB olduğu gerekçesiyle64KiB'lik cache diye sunulamaz. Yazma/normal Vec
RAM'i veya concurrent yük bu ölçümde yoktur.
[Bellek ham verileri](benchmarks/checked-page-isolated-memory-10k-20261018-summary.json).

Son2-pair diagnostic profilde6 örnek/variantın her birinde quixote131→81
read API,56→56 candidate CRC,50→0 ikinci decode/CRC ve0→50 checked reuse;
holmes1514→1467 read,1285→1285 candidate CRC,50→0 ikinci decode/CRC.
quixote sonuç hazırlama instrumented medyan1,295→0,551ms, holmes1,289→
0,520ms. Schema/UTF-8 çözümlemede tekrar kontrol edilir. Negatif zx ve covered the200
capture kullanmaz. Bu somut iş azalması, bütün38 sorgunun daha hızlı olduğu
anlamına gelmez. [Son profil](benchmarks/checked-page-isolated-profile-10k-20261018-summary.json).

Sınırlı prototip hedefi ölçüldü; sorgu sınırı/bütçe/sürüm/sıra/CRC korunarak
ikinci okumanın kaldırılabildiği kanıtlandı. Genel hız ve RAM geçiş kapısı
geçilmediği için varsayılan değiştirilmeyecek. Daha fazla boyut/eşik ayar
turu yapılmadı. Yazma/bakım, genel arama/RAM, normal Vec, açılış, sıkıştırma
ve concurrent/gerçek-cold yük işleri açık kalır.

On iki ölçüm fazının önce/sonra snapshot'ları birbirine eşit;21 dosyanın
tamamı başlangıçla byte/SHA-eşittir.18 dosyalık FDB349.805.698B,
SQLite583.860.224B ve iki0.1 fixture değişmedi. Corpus'un tamamı
2.740.862.279B ve önceki SHA-256 ile akışlı yeniden eşleşti. Data altında
yalnız benchmark-work ve rkyv-corpus-20261009 vardır. Yeni corpus/DB
reset/repack/kopya/başka disk kullanılmadı. [Veri kanıtı](benchmarks/checked-page-data-proof-20261018.json).

## Final doğrulama ve geri dönüş

Son izolasyonlu prototipte cargo test --all-targets185 geçti/7 ignored;
no-default-features107 geçti/3 ignored; all-features204 geçti/9 ignored.
Beş yeni test bu sayılardadır. Eski JSON fixture, cut/byte corruption ve
process-kill wrapper testleri geçti; bu, güç kesintisi testi değildir.
Prototipte fmt --check ve default/all-features all-targets Clippy -D warnings
temiz. [Komut/sonuçlar](benchmarks/checked-page-prototype-checks-20261018.json).

Archive ters check sonrası engine.rs, segment.rs, window_tests.rs ve
tests/engine.rs doğrudan97f6ef7 Git byte'larıyla geri yüklendi; helper dosyası
kaldırıldı. Arşivin ileri git apply --check kontrolü de geçti. Tüm runtime,
test/0.1 fixture ve dependency diff'i boştur. Yeni feature/API/format yoktur.
[Kaynak SHA ve geri dönüş](benchmarks/checked-page-rollback-20261018.json).

Geri dönülen motorda7 window/corruption testi, fmt --check ve default/
all-features all-targets Clippy yeniden geçti. Bu tur geri dönülen motorda
tam180/107/199 suite yeniden çalıştırılmadı;185/107/204 sonuçları arşivdeki
prototipe aittir. Normal default release CLI ve iki okuyucu yeniden derlendi.
Yeni reader executable SHA'ları eski immutable kontrolle bit-eşit değildir;
yeniden derlenen kaynak97f6ef7'a byte-eşittir. Ölçümler dondurulmuş A/B
programlarına aittir; yeni executable SHA'ları onların yerine yazılmadı.

Gerçek normal CLI'de10.000 belge/sequence20 ve quixote56/50, holmes1222/50,
zx0/0, the10000/200 total/hit, global sıra ve version kontrolü geçti. CLI
öncesi/sonrası21 dosya da önceki snapshot'larla SHA-eşittir. Bu CLI kontrolü
yeni hız veya RAM ölçümü değildir. [Final kanıt ve aktif binary SHA'ları](benchmarks/checked-page-final-proof-20261018.json).
