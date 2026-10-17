# Seçilen belgelerin metadata tekrarları — 2026-10-17

Seçilen belgeleri hazırlarken metadata tekrarları azaltıldı. Aynı mevcut
10k veride 200 belgeli sayfa p95 6,496→5,779 ms (-%11,04), tipik blok
kazancı %12,75; 4/4 süreç çifti ve16/16 blok hızlı. Payload doğrulaması
korunur; metadata çağrısı714→514. Ayrı query/get proses bellek tepesi
medyanları artmadı ve DB349.805.698 B kaldı. Dar kazanç ölçüldü; genel
p95 ve normal kelimelerin payload I/O maliyeti kapanmadı.

## Sabit veri ve karşılaştırma

Mevcut corpus'un aynı ilk10.000 kaydı:264.842.858 B gövde,2.057–51.197 B,
8 kitap. Corpus üretme/indirme, DB reset/repack/kopyası yok. Sabit FDB
slotu349.805.698 B/18 dosya; SQLite583.860.224 B. Önce/sonra SHA-256 ile
FDB/cache/SQLite/0.1 fixture değişmezliği denetlenir. Windows11/Ryzen7
7840HS, Rust1.99/MSVC, release thin-LTO/cgu1; affinity4, normal priority,
Balanced güç planı; güç/frekans/SMT/arka plan izolasyonu yok.

Sıcak OS cache, eviction yok. Normal strict open/source/index doğrulaması
zamanlama dışında, pending0. Payload CRC/schema/UTF-8/key/version sürer.
30 primary +8 page sorgusu; aynı bağımsız corpus oracle'ı tam belge/alan/
link/sürüm/toplam/sıra eşitliğini zamanlama dışında denetler. Oracle bellek
ölçümlerinde yüklenmez. Karşılaştırma tabanı b804de8; bütün değişen runtime
kodu segment.rs içindedir. Formatlar, feature'lar ve bağımlılıklar değişmez.

## Ölçüm kontrolünde öğrenilenler

Önceki aynı-SHA kısa blok A/A'da36/38 pooled p95 yüksekti. Yeni aynı-SHA
kontrolde29 düşük/9 yüksek; tek yönlü etiket farkı tekrar etmedi. Yeni
kontrol4 süreç çifti×4 blok×5 turdur: primary80, page32 örnek/variant.
Tipik eşlenmiş blok medyanları-%5,45…+%5,61; p95-%17,80…+%25,97.
Bu aralıklar güven aralığı ya da sonraki sonuçlardan çıkarılacak bir
düzeltme katsayısı değildir. Tek A/A koşusu küçük farkları güvenilir kılmaz.

Bu kontrol benchmark-only thread gözlemlerini açar: duvar saati bracket'ı
dışında GetThreadTimes ve QueryThreadCycleTime. Tek sayaç okuması çoğu
blokta yaklaşık0,6–0,8us; ham1000 örnek/blok saklanır. CPU süresi bu
makinede15,625ms adımlı, dar sorgularda0 olabilir; duvar süresinden
çıkarılıp I/O/bekleme maliyeti yapılmaz. Çevrim sayısı sabit frekanslı saat
değildir ve sayaç maliyetini de içerir. Birçok A/A p95 farkı çevrimlerde de
görünür; salt thread'in zamanlanmamasını kök neden ilan etmek için kanıt
yok. Frekans, cache ve diğer süreç etkileri birbirinden ayrılmadı.

Normal hız karşılaştırmaları bu ek thread sayaçlarını kapatır; aynı
harness ve ölçüm modu iki tarafta da aynıdır. Diagnostic span süreleri
ayrı derlemede, iç içe ve sayaç maliyeti dahil ölçülür; toplanmaz veya
normal sorgu gecikmesi diye sunulmaz.

[A/A ham sonuç ve tasarım](benchmarks/search-observed-aa-10k-20261017-summary.json).

## Dar değişiklik

read_selected_batch seçilen sıralı slotlar için aynı metadata yaprağını
query-local cursor ile yeniden kullanır. Payload boyutu/adresi/CRC/version
sayısal olarak okunur; anahtar karşılaştırması doğrulanmış yapraktan
yapılır. Her decode öncesi metadata'yı tekrar okuyup sahipli Key üretmek
gerekmez. Dönen belgelerin alanları/links/metni yine tamamen çözülür.

Payload okuma/pencere algoritması, aday seçimi, contains normalleştirmesi,
source lease ve bütün belge CRC/schema/identity kontrolleri korunur.
Yeni resident tablo veya belge cache'i yok; cursor yaprağı küçük ve sorgu
ömrüyle sınırlıdır (yaprak başına mevcut256KiB sınırı). Büyük belge
fallback'i önceki okuyucudur.

## Normal A/B ve aynı-kod tekrarları

Geniş matrix4 süreç çifti×4 strict blok×5 tur: primary80/page32 örnek.
23 düşük/15 yüksek pooled p95; genel p95 kapısı kapanmadı. the/50 ve he/50
tipik blok farkları-%12,67/-%11,85; bunların p95'i-%17,50/-%16,86.
quixote/50 tipik-%4,13; holmes tipik+%0,41. Dolayısıyla sıradan kelimelerin
payload doğrulaması/I/O maliyeti çözülmüş değildir.

Yoğun8 sayfa:4 süreç çifti×4 blok×25 tur,400 örnek/sayfa/variant.

| Sorgu, offset/limit | Kontrol p95 ms | Aday p95 ms | p95 fark | Tipik blok fark | Hızlı çift/blok |
|---|---:|---:|---:|---:|---:|
| the,0/200 |6,4961|5,7789|-%11,04|-%12,75|4/4;16/16|
| the,9950/50 |1,8363|1,8585|+%1,21|+%1,27|1/4;6/16|
| ishmael,60/200 |15,5741|14,9887|-%3,76|-%4,03|3/4;13/16|
| quixote,28/10 |2,3572|2,2134|-%6,10|-%1,54|2/4;10/16|

Geniş matrix'te the/200 de p95-%18,48, tipik-%10,19;3/4 süreç ve13/16
blok hızlıydı. Hedefte iki ayrı A/B dizisinde kazanç var. Yoğun aynı-mod
A/A, aynı aday SHA ile the/200 p95+%6,66, tipik+%0,21 ve8/16 hızlı blok
verdi. Bu oranlar A/B'den çıkarılmadı.16 blok16 bağımsız proses değildir.
Dar the/200 kazancı tekrarlanır; bütün sorguların p95'i veya gerilemelerin
tamamı çözülmüş sayılmaz. the/9950 artışı düşük örnekli geniş matrix'te
+%32,38'di; yoğun tekrarda+%1,21, A/A'da+%4,31 oldu. Kod payının sıfır
olduğu kanıtlanmadı ve bu başlık kapatılmadı.

Mevcut offset+limit>4096 sayfaları önceki direct-slot fallback'ini kullanır;
bu değişiklik o yola girmez. the/5000 ve the/9950'nin izlenen bütün çağrı
sayıları kontrolle aynıdır. Süre farkları yeni metadata algoritmasının o
sayfalarda çalıştığı anlamına gelmez; program yerleşimi/ortam payı ayrılmadı.

[38-case tablo](benchmarks/search-hydration-ab-10k-20261017-table.md),
[yoğun8 sayfa](benchmarks/search-hydration-dense-10k-20261017-table.md),
[aynı-mod A/A](benchmarks/search-hydration-dense-aa-10k-20261017-table.md).

## Ayrı diagnostic profil: yapılan iş

İki çift, her variantta6 örnek/senaryo. İç içe span'ler toplanmaz; ek timer/
mutex maliyeti dahildir. the/200 metadata çağrısı714→514, metadata span
0,459→0,069ms; page merge/hydration5,959→5,403ms. Dönen200 belgenin
CRC/decode/identity işlemi200→200 kalır; ödünç anahtar karşılaştırması
ayrı query.record_identity sayacında200 çağrı olarak görünür.

quixote metadata251→201, aday payload okuması56→56 ve1.945.325 B aynı.
holmes metadata1481→1431; aday okuması1285→1285 ve35.906.386 B aynı.
Bu turda payload I/O azaldı iddiası yok. Body fold/scan, aday/sonuç sayısı,
payload CRC/yapı doğrulaması ve kaynak kontrolleri korunur. Page hazırlığı
the/50'de0,790→0,651ms, he/50'de0,722→0,551ms; bu instrumented değerler
normal hız tablosunun yerine geçmez.
[Bütün38 profil](benchmarks/search-hydration-profile-10k-20261017-summary.json).

## Ayrı proses bellek ölçümü

Dört çift, oracle yüklemeden24 query/771 get; OS lifetime peak private
commit medyanı9.213.952→9.183.232 B (-30KiB;4/4 düşük). Peak working set
13.807.616→13.801.472 B (-6KiB;2 düşük/1 eşit/1+4KiB). Medyan tepeler
artmadı; bunlar küçük farklardır ve genel RAM kapısı değildir. Açılış ve
sonuç tamponları dahildir; faza özel heap veya kernel/file cache dahil
toplam sistem RAM'i ölçülmez. Normal Vec ingest RAM'i bu probun dışında.
FDB18 dosya349.805.698 B olarak hash-eşit kaldı.
[Ham OS sayaçları](benchmarks/search-hydration-memory-10k-20261017-summary.json).

## SQLite: aynı veride güncel arama karşılaştırması

38/38 sorguda pooled p95 FDB'de düşük; her sorgunun dört süreç çiftinin
tamamında da FDB hızlı. Bu karşılaştırma yeni aday/SQLite arasındadır;
önceki FDB sürümüne göre geniş matrix'teki23/15 sonucuyla karıştırılmaz.

| Sorgu, offset/limit | FDB p95 ms | SQLite p95 ms | SQLite/FDB |
|---|---:|---:|---:|
| quixote,0/50 |3,6089|18,0867|5,01|
| holmes,0/50 |35,7900|214,8940|6,00|
| zx,0/50 |176,9441|1644,3235|9,29|
| the,0/200 |5,2877|1376,6946|260,36|

the gibi tam kapsamı kanıtlanan sorgularda FDB önceki indeks kanıtını
kullanır; SQLite karşılaştırıcısı LIKE üzerinden toplamı hesaplar. Büyük
oranlar eşit CPU işi yapıldığını veya bütün DB işlemlerinin bu kadar
hızlandığını göstermez. SQLite the/tek sonuç p95'inde3.282ms gibi yüksek
kuyruk da var; bunun kesin ortam nedeni ölçülmedi. Bütün sonuçlar ve ham
örnekler saklanır, yalnız iyi sorgular seçilmez.

256 anahtarın dört boyut grubunda get p95 FDB0,0275–0,0443ms,
SQLite0,0623–0,1588ms; missing get0,0008/0,0120ms. Bunlar sıcak-cache
çağrı süreleri, fiziksel disk gecikmesi veya protokol uçtan uca süresi değil.
Disk FDB349.805.698 B, SQLite583.860.224 B; ikisi de read koşusu öncesi/
sonrası hash-eşit. Disk farkı önceki format/GC işlerinin de sonucudur;
bu metadata düzeltmesi diski küçültmedi.

[Bütün38 sorgu tablosu](benchmarks/search-hydration-sqlite-10k-20261017-table.md),
[ham örnekler/paired sonuçlar ve tasarım](benchmarks/search-hydration-sqlite-10k-20261017-summary.json).

Mevcut SQLite slotu read-only açılır; bundled SQLite3.53.2 FTS5 trigram
tablosunda folded LIKE, count ve aynı sıralı sayfa/full JSON belge çözümü
ölçülür. Sayfa cache'i2 MiB, mmap0, temp_store=FILE, prepared statement
cache64. İki motor da aynı38 sorgu ve256 get anahtarını, aynı tam belge/
sürüm oracle'ını kullanır. Dört dengeli fresh-process çift; her primary
20, her page5 örnek/proses, pooled80/20. Thread gözlemleri ve diagnostic
span'ler bu hız koşusunda kapalıdır. CLI JSON yanıt serileştirmesi bu
ölçümde yoktur; tam sonuç belgelerinin hazırlanması dahildir.

FDB strict open tam kaynak/index kontrolü yapar, SQLite açılışında tam
integrity taraması yoktur; arama bracket'ı ikisinin de açılışını dışlar.
SQLite karşılaştırıcısı FDB'nin bütün field/link/guard yeteneklerini
uygulamaz. Sonuç bu yapılandırma ve query seti içindir; bütün SQLite
kullanımlarına ya da diğer depolama motorlarına genellenmez.

Kota kesintisinden önce tamamlanmış çift0/1 logları byte'ları korunarak
yeniden kullanıldı. Yarım çift2 SQLite denemesi ayrı interrupted loguna
saklandı ve istatistiğe katılmadı; çift2/3 aynı program SHA'ları, affinity
ve DB/fixture SHA'larıyla sürdürüldü. Ara nedeniyle makine koşulları aynı
kabul edilmez; eşlenmiş sonuçlar da raporlanır. Yeni DB veya corpus yok.

Ayrı oracle'sız24 arama/771 get bellek karşılaştırması, dört dengeli yeni
çift: lifetime peak private commit medyanı FDB9.203.712 B (8,78MiB),
SQLite13.451.264 B (12,83MiB), FDB-%31,58 ve4/4 düşük. Peak working set
FDB13.142.016 B (12,53MiB), SQLite17.512.448 B (16,70MiB), FDB-%24,96
ve4/4 düşük. Bu yeni SQLite karşılaştırma serisi, önceki eski/yeni FDB
bellek serisinden ayrıdır; ortam ve proses açılışı tepeleri etkiler.
Sayaçlar bütün proses ömrünün OS high-water değerleridir; private commit
fiziksel RAM değildir, kernel/file cache dahil toplam sistem RAM'i değildir.
Normal Vec ingest, yazma/bakım ve eşzamanlı yük ölçülmedi.
[SQLite karşılaştırmalı ham bellek sayaçları](benchmarks/search-hydration-sqlite-memory-10k-20261017-summary.json).

## Kod ve kurtarma doğrulaması

Yeni paged batch testi iki metadata yaprağı boyunca tam metin/nested fields/
links eşitliğini, CRC-geçerli yanlış key/version'ı, tek bayt bozulmasını ve
kesilmeyi sınar. Mevcut grouped fallback/gap/büyük belge/deleted/boş batch
testleri de geçti. Tam target yürütmeleri: default180 geçti/7 ignored,
no-default107/3, all-features199/9; başarısız0. Bunlar paylaşılan example
testlerini de içeren yürütme toplamları, benzersiz mantıksal test sayısı
değildir. Mevcut process-kill/recovery ve eski JSON/0.1 testleri dahildir;
güç kesintisi testi değildir. Fixture'a yazılmaz.

cargo fmt --check, default ve all-features cargo clippy --all-targets --
-D warnings geçti. Python araçları py_compile ile kontrol edildi.
Normal CLI/read worker yeniden diagnostic kapalı derlendi. Karşılaştırma
aracı yalnız görevde değişen segment.rs'yi geçici değiştirip exact byte'larla
geri yükler; DB'yi açmaz. Hash'li frozen normal/profile okuyucuları ve build
logları saklanır. Kaynak/WAL/manifest/payload/cache biçimi, bağımlılık ve
feature seçimi değişmez; normal default bounded-engine korunur.
Yeniden derlenmiş normal okuyucuyla ölçümdeki frozen okuyucunun executable
kodu ve debug dışı verisi byte-eşittir; yalnız9 PDB/debug kimlik byte'ı
farklıdır. Tam executable SHA eşitliği iddia edilmez.
[Build manifesti](benchmarks/search-hydration-20261017-build-manifest.log),
[default testler](benchmarks/search-hydration-default-tests-20261017.log),
[tüm özellikler](benchmarks/search-hydration-all-tests-20261017.log),
[Clippy](benchmarks/search-hydration-clippy-20261017.log).

Son bağımsız kanıt aracı8 önce/sonra snapshot çiftini ve mevcut dosyaları
karşılaştırdı: FDB18 dosya349.805.698 B aynı, SQLite ve0.1 fixture aynı.
2.740.862.279 B corpus'un tamamı1MiB parçalarla SHA-256 doğrulandı:
11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B.
Hiçbir corpus/DB kopyası oluşturulmadı. Bütün38 diagnostic senaryoda
metadata ve borrowed identity dışındaki izlenen bütün çağrı sayıları
min/max eşit; grouped path'te her dönen belge için metadata bir azalır,
identity karşılaştırması ayrı sayaçta görünür. Direct-slot fallback aynı.
Normal default CLI stats10k/sequence20, the/200, quixote/50 ve zx/0 doğru;
programdan JSON yanıtları ve sürümler kontrol edildi, yazma yapılmadı.
[Dosya, iş sayısı, derleme ve CLI kanıtı](benchmarks/search-hydration-final-proof-20261017.json).

## Açık sınırlar

Genel p95, normal kelimelerdeki payload I/O, yazma/bakım, normal Vec RAM'i,
açılış, disk sıkıştırma ve concurrent/gerçek-cold yük hâlâ açık. Bu çalışma
yazma veya bakım süresinde kazanç iddiası değildir.
