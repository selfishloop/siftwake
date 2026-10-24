# Merge posting tamponunu yeniden kullanma — 24 Ekim 2026

Bu takip, önceki [RAM gram posting](POSTING-MERGE-10K-20261024.md)
adımında açık kalan son bakım maliyetini inceler. Önceki hızlı intake fold,
gram sıralama ve RAM posting ekleme kazanımları korunur. Tek runtime adayı,
her immutable input için mevcut 8 KiB posting okuma tamponunu merge ömrü
boyunca yeniden kullanır. Yeni corpus, kalıcı format, feature, dependency,
WAL/durability/protokol veya dış proje entegrasyonu yoktur.

Normal release iki çift/API ölçümünde akışlı yazma+bakım
19,841→19,243 s (-%3,01), owned 14,903→14,525 s (-%2,54); iki API'de de
iki çift düşük. Bu toplam kazancı dar değişikliği tutmanın gerekçesidir.
Ayrı son bakım yalnız 1/2 çiftte düşük; bakım sorunu çözülmüş veya önceki
artışın nedeni kanıtlanmış değildir. Akışlı CPU yalnız 1/2 çiftte düşük;
RAM medyanları da artar. Geniş 38 durumda 12 düşük/26 yüksek p95;
100 örnekli seçilmiş yedi durumda QUİXOTE +%12,60, the/1 +%5,68 p95
kalır. Genel RAM/arama ve bakım kapıları açık kalır.

## Önce teşhis: owned son bakım sınırı

Tek diagnostic-timing owned koşusu aynı mevcut ilk 10.000 belge ve
264.842.858 özgün metin byte'ını kullanır. Ingest 9,032074 s, sonraki drain
2,640663 s; bütün proses thread'lerinin OS CPU toplamı 10,531250 s'dir.
Bu instrumented koşu normal release performans karşılaştırması değildir.

Drain başında 9 run ve 1.000 resident/delta belge vardır; çalışan compaction
yoktur. Ingest snapshot'ında merge worker/scheduled job bulunmaz. Son bakım
1.000 belgenin flush'ını ve toplam 10.000 input kaydını kapsayan beş merge'ü
tamamlar; sonunda 5 run, sıfır delta/resident ve 19.221.340 posting girdisi
vardır. Bu koşuda son bakımı ingest'ten taşan çalışan worker açıklamaz.
Önceki normal ölçümün owned +%11,97 bakım artışının kesin nedeni bu tek
profil ile açıklanmış değildir.

| Instrumented wall span | Süre, s |
|---|---:|
| Beş merge worker | 2,340996 |
| Merge join bekleme | 2,341875 |
| Posting stream döngüleri | 0,711700 |
| Kaynak frame encode/write, flush dahil 6 çağrı | 0,651954 |
| Merge kaynak/indeks doğrulaması | 0,112381 |
| Merge kimlik eşlemeleri | 0,005846 |

Builder ve source workspace bekleme span'leri toplam mikro-saniye
düzeyindedir. Worker, join, source, builder ve posting span'leri iç içe
veya thread'ler arasında örtüşür; toplanamaz, CPU yüzdesi sayılamaz.
Allocation, sıfırlama, allocator veya memcpy ayrı ölçülmedi. Posting
döngüsündeki terim başına 8 KiB cursor kurulumunu kaldırmak bir dar
hipotezdir; bu tablodan kesin kök neden çıkarılmaz.

Diagnostic span'leri tamamlandıkları snapshot'a girer. Yeni tek-seferlik
ingest/drain işareti span'ın wall süresini `before_drain_ns` ve
`after_drain_ns` olarak ayırır; `started_before_drain` başlangıç tarafını
sayar. Snapshot alınırken süren span bir sonraki snapshot'ta görünebilir.
Örnek/iş sayaçlarının `elapsed_ns` alanları bu span ayrımının parçası
değildir. Bu owned koşusunda worker span'lerinin before kısmı sıfırdır.
Normal build'de bu ek gözlem kodu derlenmez.

[Ham profil](benchmarks/maintenance-profile-before-20261024-owned.jsonl),
[OS CPU ve çıkış durumu](benchmarks/maintenance-profile-before-20261024-owned-process.json).

## Tek dar aday ve doğruluk sınırı

Önceki yol, her terim grubunda katılan input'lar için cursor Vec'i kurup
8 KiB tamponları sıfırlıyordu. Aday merge başında input başına bir cursor
kurar; terim değişiminde posting offset/count, consumed, available,
position, previous ve mapped_previous sıfırlanır. Önceki tampon byte'ları
yeni positional read yapılmadan erişilebilir değildir. Sözlük iterator'ı,
ID heap'i, eski→yeni eşleme, deleted sentinel, strict input/mapped sıra,
bounds/duplicate kontrolü ve çıktı codec'i aynıdır. Tamponlar sorguya veya
sonraki merge'e taşınmaz. Bu input başına 8 KiB eleman alanıdır; tüm merge
veya proses RAM sınırı değildir. Input'ların bir terimde birlikte
bulunmaması durumunda eski yola göre daha çok tampon aynı anda canlı
kalabilir; OS RAM sonucu ayrıca ölçülmelidir.

Test-only const generic referans aynı algoritmayı terim başına kurulumla
çalıştırır; normal executable yalnız yeniden kullanılan yolu çağırır.
Odak testler yarım 8 KiB pencere sonrası kısa/boş posting'i, 2.048 ID
sınırını geçen refill'i, silinmiş ID'leri, yanlış/eksik eşlemeyi, eski ve
mapped sıra hatalarını, duplicate destination'ı, truncated read'i, hata
sonrası reset'i ve CRC/byte eşitliğini kapsar.

## Aynı executable içinde salt okunur ön test

Mevcut sabit `data/benchmark-work/fdb` içindeki tamamlanmış beş indeks
doğrudan okunur; DB açılmaz, sıfırlanmaz veya kopyalanmaz. 10.000 eşsiz
key'den mevcut sıralı eşleme kurulur. Input checksum doğrulaması, key/map
hazırlığı ve sonuç eşitlik kontrolü zaman dışında; merge döngüsü, checksum
sink'i ve çıktı directory üretimi zaman içindedir. Gerçek dosya yazması,
source encode, flush, publication ve DB ingest/drain bu probe'a dahil
değildir. OS cache sıcaktır; eviction/güç/donanım izolasyonu yapılmadı.

| Aynı süreçte sırası değişen blok | Terim başına kurulum, ms | Merge başına kurulum, ms |
|---|---:|---:|
| 0, referans→aday | 658,1584 | 579,6042 |
| 1, aday→referans | 606,2099 | 580,3223 |
| 2, referans→aday | 602,8601 | 571,2054 |
| 3, aday→referans | 584,2917 | 571,0980 |
| Dört bloğun toplamı | 2.451,5201 | 2.302,2299 |

Toplam -%6,09; dört blokta da aday düşük. Her blokta 76.885.360 çıktı
posting byte'ının CRC'si ve 53.499 directory teriminin tamamı offset/count
ile eşittir. Eski kurulum 103.667 input-terim cursor'ı, aday 5 input
cursor'ı kurar; bunlar koddan sayılan kurulum adetleridir, ölçülmüş tahsis
veya bellek transfer sayısı değildir. Bu tek süreçli dar sonuç normal DB
bakım/ingest, genel CPU veya OS RAM kazancı ilan etmez.
[Ham ön test](benchmarks/maintenance-workspace-probe-20261024.log).

## Normal release yazma ve son bakım

Kontrol, önceki RAM posting ekleme adımının dondurulmuş normal programı;
aday yalnız merge cursor tekrar kullanımını ekleyen normal default build'dir.
Diagnostic özellikler kapalıdır. Her API için iki süreç çifti; sıra önce
kontrol/aday, sonra aday/kontrol, toplam sekiz süreçtir. Aynı mevcut
`data/rkyv-corpus-20261009/json.payload-bench` ilk 10.000 belge,
264.842.858 özgün metin byte'ı ve 20×500 synced WAL transaction kullanılır.
Input CRC/JSON ve 40 sınır get'i ingest süresindedir. Son bakım ingest
sonrasındaki flush/merge drain; worker ingest ile örtüşebilir. Her toplam
önce süreç ingest+drain'inden hesaplanır; ayrı faz medyanları toplanmaz.
CPU bütün proses thread'lerinin OS toplamıdır. Süreçler ardışık ve OS cache
sıcaktır; eviction, güç/donanım izolasyonu yoktur. Bu oturumun mutlak
süreleri önceki tarihli oturumla eşit hız koşulu sayılmaz; yüzdeler
birleştirilmez.

| Medyan, saniye | Kontrol | Aday | Fark | Düşük çift |
|---|---:|---:|---:|---:|
| Akışlı yazma | 19,0318 | 18,4699 | -%2,95 | 2/2 |
| Akışlı son bakım | 0,8094 | 0,7734 | -%4,45 | **1/2** |
| Akışlı yazma+bakım | 19,8412 | 19,2433 | **-%3,01** | **2/2** |
| Akışlı proses CPU | 17,8750 | 17,5625 | -%1,75 | **1/2** |
| Owned yazma | 11,0125 | 10,6611 | -%3,19 | 2/2 |
| Owned son bakım | 3,8909 | 3,8639 | -%0,69 | **1/2** |
| Owned yazma+bakım | 14,9033 | 14,5249 | **-%2,54** | **2/2** |
| Owned proses CPU | 13,4609 | 13,0313 | -%3,19 | 2/2 |

Akışlı toplam çiftleri -%4,68/-%1,29; owned -%0,51/-%4,51'dir. Son bakım
akışlı -%8,62/+%0,008, owned +%6,18/-%6,88 ile karma kalır. Akışlı CPU
çiftleri -%4,29/+%0,89'dur. İki API'de toplamın 2/2 düşük olması dar
adımı korumayı destekler; genel hız veya istatistiksel anlamlılık garantisi
vermez. Özellikle owned son bakımın yalnız -%0,69 medyanını kesin merge
kazancı saymak veya önceki +%11,97 artışı kapatmak doğru değildir.

SQLite bu tur yeniden yazılmadı veya süre ölçümüne katılmadı. Önceki
SQLite sonuçları bu yeni FDB süreleriyle güncel eşli kıyas oluşturmaz.
[Tasarım ve özet](benchmarks/maintenance-write-10k-20261024-summary.json),
[bütün süreç kayıtları](benchmarks/maintenance-write-10k-20261024-records.json).

## RAM, disk ve veri koruma

OS lifetime private commit fiziksel veya toplam sistem RAM'i değildir.
Working set paylaşılan image sayfalarını içerir. Faz örneklemesi kısa
tepeleri kaçırabilir; aşağıdaki sayaçlar süreç sonuna tutulan handle'ın
lifetime tepeleridir.

| OS tepe sayacı | Kontrol | Aday | Medyan farkı | Düşük çift |
|---|---:|---:|---:|---:|
| Akışlı private commit | 10.442.752 B | 10.444.800 B | **+2 KiB** | 1/2 |
| Akışlı working set | 14.493.696 B | 14.743.552 B | **+244 KiB** | 1/2 |
| Owned private commit | 83.941.376 B | 84.080.640 B | **+136 KiB** | 1/2 |
| Owned working set | 87.373.824 B | 87.685.120 B | **+304 KiB** | 1/2 |

Bütün sayaçlarda çift yönleri karmadır; dört medyan da artar. Cursor
kurulum sayısının azalması OS RAM azalması sayılmaz; genel RAM kapısı
geçmedi. Owned caller Vec/delta tepesi yaklaşık 80 MiB düzeyindedir.
Query/get OS bellek tepesi bu yazma serisiyle ölçülmez.

Sekiz süreç sonunda core disk 349.805.698 B, fark 0 B'dir. Aynı API içinde
kontrol/adayı source, index ve ready byte'ları eşittir. Her süreç sonunda
süre dışında tam 10.000 payload ve `the`, `quixote`, `he`, `the man`
contains sayfaları oracle'ı geçti. Yalnız sahipliği kontrol edilen sabit
`data/benchmark-work/fdb` alanı benchmark workspace yardımcısıyla
sıfırlandı; yeni DB kopyası veya corpus hazırlanmadı, başka diske taşınmadı.
Corpus ve 0.1 fixture SHA'ları normal seri öncesi/sonrası eşittir. Dinamik
geçici disk tepesi ölçülmedi.
[Korunan kaynaklar önce](benchmarks/maintenance-write-10k-20261024-protected-before.json),
[sonra](benchmarks/maintenance-write-10k-20261024-protected-after.json).

## Okuma: geniş kontrol ve seçilmiş yoğun tekrar

Aynı son 10k DB'de normal kontrol/aday için 38 durum, iki süreç çifti×iki
blok×beş tekrar çalıştı: durum başına 20 örnek/yol. CPU maskesi 4, blok
başlangıç sırası dengelidir. Her blokta strict kaynak doğrulaması ve
ısınma süre dışındadır. İki resident oracle worker vardır; DB sorguları
eşzamanlı değildir. Bu düzen sürekli açık tek süreç veya eşzamanlı yük
testi değildir. Tam payload/sıra/version oracle'ı geçti; önce/sonra izlenen
21 dosya SHA-eşittir. Pooled p95 12 durumda düşük, 26 durumda yüksek;
az örnekli bu set genel arama kabulünü geçirmez.
[Geniş 38 durum](benchmarks/maintenance-read-10k-20261024-summary.json).

Önceki açık sayfalar ve geniş kontroldeki büyük/tekrarlanan artışlardan
seçilmiş yedi durumda tek yoğun takip tamamlandı. Aynı program/veri/CPU
maskesiyle iki çift×iki blok×25 tekrar; durum başına 100 örnek/yol vardır.
Bu set sonradan seçilmiştir; tüm 38 durumun yeniden ölçümü veya ilk geniş
sonuçların bağımsız doğrulaması değildir. Pooled p95 beş düşük/iki yüksek;
tam oracle ve 21 dosyanın önce/sonra SHA eşitliği yine geçti.

| Sorgu / offset / limit | Kontrol p95, ms | Aday p95, ms | P95 farkı | Hızlı çift | Tipik blok farkı |
|---|---:|---:|---:|---:|---:|
| zx /0/50 | 176,6713 | 172,9290 | -%2,12 | 1/2 | **+%0,17** |
| QUİXOTE /0/50 | 2,6378 | 2,9702 | **+%12,60** | **0/2** | **+%2,20** |
| uzun zx /0/50 | 0,0478 | 0,0395 | -%17,36 | 2/2 | **+%1,53** |
| the /0/1 | 0,2111 | 0,2231 | **+%5,68** | 1/2 | **+%2,84** |
| the /0/200 | 5,9022 | 5,1161 | -%13,32 | 2/2 | -%7,94 |
| the /9950/50 | 1,9266 | 1,7118 | -%11,15 | 2/2 | **+%2,62** |
| quixote /28/10 | 2,3906 | 2,1466 | -%10,21 | 2/2 | **+%1,81** |

QUİXOTE p95 çiftleri +%14,25/+%16,76; iki çiftte de yüksek, tipik blok da
medyanda yavaştır. The/1 çiftleri +%10,90/-%2,49 ile karma olsa da pooled
0,0120 ms artışı ve tipik blok artışı açık kalır. Zx yalnız 1/2 çiftte
hızlıdır. Uzun negatif, son sayfa ve quixote28/10 p95'leri iki çiftte
düşük olsa da tipik blok farkları artar; son sayfanın dört blok medyanı
da yüksektir. Pooled tail ve blok medyanları ayrı dağılımlardır; yalnız
birini seçerek genel hız başarısı veya gerilemenin kapanması ilan edilmez.
The200 bu oturumda iki çiftte ve tipik blokta düşüktür; farklı önceki
kontrol/oturumların the200 veya diğer tarihsel regresyonlarını otomatik
kapatmaz.

Sorgu algoritması değişmedi fakat executable'ların kod/yerleşimi aynı
değildir. Ortam/oturum/codegen katkıları kesin ayrıştırılmadı; QUİXOTE ve
the/1 artışları ölçüm gürültüsü diye kapatılmaz. Bu native okuma CLI JSON
round-trip kuyruğunu, query/get OS bellek tepesini, genel açılışı veya
gerçek-soğuk/eşzamanlı yükü ölçmez. Genel arama kabulü geçmedi.
[Yoğun yedi durum](benchmarks/maintenance-read-focused-10k-20261024-summary.json).

## Doğrulama

Default `cargo test --all-targets`, `cargo fmt --check` ve default,
all-features, no-default `cargo clippy --all-targets -- -D warnings`
geçti. Tam suite gerçek süreç sonlandırma, transaction/source/cache
kesilmesi, byte bozulması, merge publication/recovery ve sağlam cache'in
bozuk authoritative kaynağı gizleyememesi kontrollerini içerir. Reuse
odak testleri all-features ve yalnız `experimental-merge-postings`
referans profilinde dörder testle geçti; corpus probe'u olağan suite'te
ignored kalır. Normal release CLI ile write/read example'ları ayrıca
derlenip donduruldu. Yeni format olmadığı için sürüm değiştirilmez;
0.1 kaynak fixture'ları korunur.
[Kontrol çıktısı](benchmarks/maintenance-checks-20261024.log).
[Normal program SHA'ları](benchmarks/maintenance-frozen-20261024.json).
Tamamlanan kanıtların dizini [makine özetindedir](benchmarks/maintenance-10k-20261024-proof.json);
bu dizin yeni test çıktısı değildir.

## Açık işler

Genel okuma ve RAM kapıları geçmedi. Ayrı son bakım iki API'de yalnız 1/2 çiftte düşük;
önceki owned artışının kesin nedeni kanıtlanmadı. Önceki arama
p95/CLI kuyrukları, owned caller RAM'i ve genel RAM, kaynak encode/yazma,
açılış, disk sıkıştırma/geçici alan, eşzamanlı ve gerçek-soğuk yük işleri
bu dar adımla kapanmaz. [Açık iş listesi](OPEN-WORK.md) korunur.
