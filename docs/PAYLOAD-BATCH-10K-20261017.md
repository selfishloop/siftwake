# Aday payload okumalarını gruplama deneyi — 2026-10-17

Sonuç: bu prototip reddedildi; varsayılan motora alınmadı. Bilinen adayları
gruplamak 38 senaryonun 19'unda okuma çağrılarını azaltıyor. Normal A/B'de
holmes p95 -%9,18, tipik blok -%2,73; buna karşılık the/200 p95 ilk seride
+%10,38, yoğun tekrarda +%6,02. Genel hız kabulü geçmedi. Aynı-program
kontrolü de oynaktır; artışların kod payı sıfır diye kapatılmadı.

Motor ve test kaynağı 6eb3a89'a byte-eşit döndürüldü. Denenen kod,
[yeniden uygulanabilir patch](benchmarks/payload-batch-20261017-prototype.patch)
ve ölçüm kayıtlarında saklandı. Yeni feature, kalıcı format veya bağımlılık
eklenmedi. Mevcut belgeler/DB/SQLite/0.1 fixture değişmedi.

## Veri ve sınırlar

Aynı mevcut ilk10.000 belge; 264.842.858 B text, 2.057–51.197 B pasajlar,
8 kitap. FDB sabit slot349.805.698 B/18 dosya, sequence20/delta0/5 Run.
Yeni corpus, DB reset/repack/kopyası veya başka disk yok. SQLite slotu ve
0.1 fixture hash'leri de izlenir. Kaynak/WAL/manifest/cache/payload biçimi,
feature ve bağımlılık değişmez. Karşılaştırma tabanı6eb3a89'dur.

Windows11/Ryzen7 7840HS, Rust1.99/MSVC release thin-LTO/cgu1, CPU affinity4,
normal priority/Balanced güç planı; frekans/SMT/arka plan izolasyonu yok.
Sıcak OS cache; eviction veya gerçek-cold ölçümü yok. Normal strict open
tam kaynak/index kontrolü yapar, pending0; açılış ve corpus oracle'ı query
timing dışında. CRC/schema/UTF-8/key/version kontrolleri korunur.

## Önce yerleşim ölçümü

Mevcut SEG05 cache directory/posting'leri read-only okunur ve tüm indeks
CRC'leri denetlenir. Bu veri kümesindeki beş Run disjointtir; 10k key'in
tekilliği ve bağımsız engine profilindeki38 aday sayısı karşılaştırılır.
Bu analiz kaynak doğrulamasının yerine geçmez; normal engine testlerinde
kaynak ayrıca tam doğrulanır. Offline planlama hız veya RAM sonucu değildir.

| Sorgu | Aday / mevcut read | Adaylar arası medyan boşluk | İdeal128KiB read | Ek byte |
|---|---:|---:|---:|---:|
| quixote |56/56|2.201.252 B|56|0|
| holmes |1285/1285|125.631 B|1105|24.417|
| darcy |2129/2129|58.497 B|1692|40.020|
| watson |1954/1954|69.697 B|1547|35.706|

İdeal plan128KiB pencere, en fazla4KiB gap ve pencere başına en fazla%5
ek byte kullanır. Gerçek prototip64 aday/posting-block sınırı da koyduğu
için ideal minimuma her zaman ulaşmaz. quixote'ta bu planın faydası yok;
önceki dar-seed read-ahead denemesinin56→56 sonucuyla tutarlıdır.
[Adresler,38 sorgu ve farklı bütçe tahminleri](benchmarks/payload-layout-10k-20261017.json).

## Denenen algoritma

Görünürlük ve posting kesişiminden geçen gerçek adaylar64 slotluk stack
kuyruğunda toplanır. Her slotun sayısal metadata'sı okunur; dosyada yakın
adaylar128KiB/4KiB gap/%5 waste koşullarıyla birleştirilir. Payload tamponu
sorgu ömürlüdür, reserve_exact kullanır. Büyük belge önceki exact reader'a
döner. Geniş seed mevcut256KiB yoğun yolunu korur; seed64 altındaysa bu
deneye girmez. rkyv benchmark overlay ve JSON yolu da önceki okuyucudadır.

Okuma aralığındaki seçilmeyen payload'lar parse edilmez; her gerçek adayın
CRC, schema/UTF-8, key/version ve contains doğrulaması ayrı yapılır. Prefix
ve tam-ID yollarında sıralama, total, union/group etiketleri ve görünürlük
korunur. Yeni kalıcı önbellek veya tam belge resident tablosu yoktur.

## Gerçek motorda iş sayıları

İki dengeli diagnostic süreç çifti, her sürüm/senaryoda6 sıcak örnek.
Aşağıdaki sayılar bütün6 örnekte aynıdır; stage süreleri iç içedir ve
timer/mutex maliyeti içerir, normal sorgu p95'i yerine kullanılmaz.

| Sorgu | Read kontrol→aday | Okunan byte kontrol→aday | Ek byte |
|---|---:|---:|---:|
| quixote |56→56|1.945.325→1.945.325|0|
| holmes |1285→1106|35.906.386→35.930.798|24.412|
| darcy |2129→1700|67.132.043→67.165.534|33.491|
| watson |1954→1553|61.078.835→61.114.511|35.676|

38 senaryonun19'unda read azalır. Normal quixote ve mevcut yoğun zx/the-zx
yolu değişmez. Read API çağrıları fiziksel disk I/O sayısı değildir. Bütün
senaryolarda payload aday/CRC/schema/UTF-8/identity, body fold/scan,
posting intersection, görünürlük, kabul edilen ID ve hydration iş sayıları
korunur; yalnız read/read-window/disk.read_at sayıları değişir.
Bu, okuma çağrısının azaldığını kanıtlar; tek başına gerçek hız kanıtı değildir.
[38-case diagnostic profil](benchmarks/payload-batch-profile-10k-20261017-summary.json).

## Normal hız ve ayrı bellek ölçümü

İlk normal A/B:4 süreç çifti×4 blok×5 tur; primary80, page32 örnek/variant.
Tanılama ve ek thread sayaçları kapalı; bütün38 sorgunun tam belge/toplam/
sıra/sürüm eşitliği bağımsız oracle ile zamanlama dışında denetlendi.
Pooled p95'te34 düşük/4 yüksek. holmes36,4020→33,0594ms (-%9,18),4/4 çift;
tipik eşlenmiş blok -%2,73 ve13/16 hızlı. darcy -%3,53, tipik -%2,24,
14/16 hızlı; watson -%3,14, tipik -%2,89,15/16 hızlı. Bu sayıların tamamı
tek seri;16 blok16 bağımsız süreç değildir.

Önemli karşı kanıt:the0/1 p95+%22,55 vethe0/200+%10,38; ikisi4/4 çiftte
yüksek. Bu sorguların verification_terms listesi boştur; yeni batch yolu
çalışmaz. İzlenen iş sayıları aynı olsa da genel loop gövdesi/stack ve
üretilen program yerleşimi değişmiştir. Kaynağın bu dalına girmemek,
kodun hiç performans etkisi olmadığı kanıtı değildir. Salt zamanlama,
cache, frekans veya kod üretimi payları birbirinden ayrılmadı. Aynı kontrol
programıyla A/A ve aynı sekiz sayfada daha yoğun A/B kontrolü tamamlandı.
Kalan artışlar ölçüm dalgalanması diye kapatılmadı.

Oracle'sız ayrı query/get yükü:4 dengeli süreç çifti,24 sorgu/771 get.
OS proses ömrü peak private commit medyanı9.084.928→9.132.032B
(8,664→8,709MiB;+46KiB/%0,52); peak working set13.144.064→13.217.792B
(12,535→12,605MiB;+72KiB/%0,56). Private3/4,WS4/4 çiftte yüksek;
sıfır RAM artışı iddiası yok. Bu sayaçlar faza özel heap, toplam fiziksel
RAM veya sistem/file cache toplamı değildir.128KiB okuma tamponu bütçesi
bu proses sayaçlarına bir üst sınır koymaz. Disk byte'ları değişmedi.

[Normal38-case](benchmarks/payload-batch-ab-10k-20261017-summary.json),
[ayrı bellek](benchmarks/payload-batch-memory-10k-20261017-summary.json).

Yoğun sayfa A/B:4 süreç çifti×8 strict blok×25 tur=800 örnek/sayfa/variant.
3 düşük/5 yüksek p95. the 0/1 -%1,12 (2/4 çift düşük), ilk +%22,55 tekrar
etmez. the 0/200 7,1694→7,6012ms (+%6,02;2/4 çift düşük), tipik blok +%2,78,
14/32 hızlı. the9950/50+%7,04 fakat tipik-%1,81; quixote28/10+%4,14
fakat tipik-%1,15. Pooled tail ile tipik süre aynı ölçü değildir; küçük
örnekli ilk sonuçların bütün çiftlerde tekrarlanan regresyon olduğu
iddiası sürdürülmez. the0/200'ün p95 ve tipik süre artışı yine kabul
kapısını açık bırakır.32 blok32 bağımsız süreç değildir.
[Bütün sekiz sayfa](benchmarks/payload-batch-pages-10k-20261017-table.md).

Aynı SHA'lı kontrol programıyla A/A, aynı 4×8×25 düzeninde tamamlandı:
1 düşük/7 yüksek p95. the/200 p95 +%1,69 (2/4 çift düşük), tipik blok
-%3,74; quixote 28/10 p95 +%4,47, tipik -%0,02. Bütün sekiz sayfanın
tipik farkları -%5,00…-%0,02, p95 -%1,06…+%5,42 arasında. Bu değerler
güven aralığı, A/B'den çıkarılacak düzeltme katsayısı veya bütün artışların
ortamdan geldiği kanıtı değildir. Pooled tail ve tipik süre farklı yönlerde
değişebilir. Bu nedenle the/200'ün artışı kanıtsız kapatılıp terfi yapılmadı.
[A/A ham tasarım ve sonuç](benchmarks/payload-batch-pages-aa-10k-20261017-summary.json).

## Ne öğrendik, sonraki sınır

quixote adayları arasında medyan 2,2MB boşluk vardır; küçük read-ahead ile
birleştirme mümkün değildir. Daha yoğun kelimelerde gerçek adayları
bilmek kör read-ahead'den daha az ek veriyle istek azaltır. Fakat kontrol
edilen payload byte'ları, CRC/schema/identity ve metin tarama/normalleştirme
iş sayıları değişmez; %14–21 daha az read çağrısı aynı oranda toplam hız
anlamına gelmez. API read sayısı fiziksel disk transfer sayısı değildir.

Sonraki böyle bir adayda kuyruk/planlama ayrı yolda tutulmalı ve doğrulama
gerekmeyen mevcut hızlı döngüye yeni branch/stack yükü eklenmemelidir.
Bu bir sonraki hipotezdir; bu turda çözüm veya kök neden diye doğrulanmadı.
Sadece read sayısına bakıp başarı ilan edilmez; aynı 38-case, yoğun sayfa
ve ayrı bellek kapısı tekrar gerekir. Yeni corpus veya farklı DB şart değil.

## Test ve yeniden üretim

Prototipte iki hedefli test geçti: yoğun/seyrek adaylar exact okuyucuyla
aynı sonucu verir; Unicode/AND/negatif sorgu, sıralama/slot sınırları,
128KiB tampon ve overread bütçesi, seçilen CRC/identity/version ve kesilme
kontrolleri doğrulandı. Seçilmeyen gap byte'ı parse edilmez. Bu testlerin
kodu arşiv patch'indedir; final motorunda kullanılmayan helper bırakılmadı.
[Test kaydı](benchmarks/payload-batch-new-tests-20261017.log).

Normal ve diagnostic kontrol/aday programları aynı harness ile
`tools/build-read-comparison.py --base 6eb3a89 --name payload-batch-20261017`
ile donduruldu; [program SHA'ları](benchmarks/payload-batch-20261017-build-manifest.log).
Normal 38-case koşusu 4 çift/4 blok/5 tur, yoğun A/B ve A/A 4 çift/8 blok/
25 tur; ayrı profil 2 çift, bellek 4 çift. Tekrar çalıştırmak yeni corpus
veya DB kopyası gerektirmez; araçlar mevcut raporları ezmeyi reddeder.

Geri döndürülmüş motorda suite ve örnek araç testleri birlikte:

| Profil | Geçen | Başarısız | Ignored |
|---|---:|---:|---:|
| Varsayılan |180|0|7|
| No-default referans |107|0|3|
| All-features |199|0|9|

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` ve ayrıca
all-features Clippy geçti. Varsayılan normal release CLI ve iki okuma
aracı yeniden derlendi. Gerçek CLI stats10.000 belge/sequence20; the,
holmes, watson, darcy, quixote ve zx toplam/sayfa/sürüm kontrolleri geçti.
Mevcut kesilme, bozulma, süreç sonlandırma ve0.1 fixture testleri bu suite
içindedir; ignored testler çalışmış sayılmaz, gerçek güç kesintisi ölçülmedi.

Beş ölçüm fazının önce/sonra21 dosyası ve final snapshot aynı:18 FDB dosyası
349.805.698B, SQLite ve iki0.1 fixture. Corpus'un tamamı2.740.862.279B,
SHA-256 `11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B`
ile akışlı olarak yeniden kontrol edildi. Yeni veri dizini yok. Arşiv patch'i
final kaynak üzerine `git apply --check` ile tekrar uygulanabilir; kontrol
sırasında uygulanmadı. Engine ve window_tests kaynakları6eb3a89'a byte-eşit;
Cargo dosyaları satır sonları dışında aynı. [Final kanıt](benchmarks/payload-batch-final-proof-20261017.json),
[denetim aracı](../tools/prove-payload-batch.py).

## Açık işler

Genel p95/A/A sapması, sonuç hazırlamanın kalan maliyeti, yazma/bakım,
normal Vec/query/caller RAM'i, açılış, disk sıkıştırma ve eşzamanlı/gerçek
soğuk yük açık. Bu tur yazma/bakım veya yeni SQLite hız sonucu iddiası değildir.
