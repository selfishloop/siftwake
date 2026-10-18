# Çağırana ait sonuç metni tamponları — 18 Ekim 2026

Sonraki kullanıcı kararıyla bu V3 kodu motora alındı:
[güncel durum ve API kullanımı](RESULT-BUFFERS-ACTIVE-20261018.md).
Aşağıdaki reddetme/geri dönüş kaydı geçişten önceki değerlendirmedir;
ölçüm rakamları ve eski ham kanıtlar değiştirilmedi.

**Karar: genel geçiş reddedildi.** Dar tekrar kazancı ölçüldü; genel hız/RAM
kapısı geçilmedi. Prototip kod/test yaması arşivlendi; normal motorun kaynakları
başlangıçtaki f89b827 koduna döndü. `search_into` normal kaynaklarda yoktur.
Geri dönüş ve veri değişmezliği kontrolleri tamamlandı.

## Davranış ve bellek sözleşmesi

Arşiv prototipte `bounded-engine` altında `Database::search_into(query, &mut page)`
önceki çağıran sonucunu tüketir. Boş sonuç tamponu, limit=1, doğrudan okuma ve
mevcut checked-payload capture sorgularında normal aramayı kullanır. Diğer
gruplu sonuçlarda yalnız önceki **metin String'leri** taşınır; eski key/title,
fields ve links yeni sorgu filtrelenmeden önce bırakılır. En fazla yeni limit
(motor sınırı 1000) kadar eski metin alınır. Sayfa dışı metinler çağrı sonunda
bırakılır. Bu bir genel proses RAM bütçesi değildir.

Bir metin yalnız kapasitesi yeni metnin byte uzunluğuna tam uyarsa yeniden
kullanılır. Diğer durumda eski tampon bırakılıp normal sahipli decode yapılır.
Metin her seferinde temizlenir ve **yeniden okunmuş, doğrulanmış baytlarla**
doldurulur. İçerik cache'i, önceki key/version'a güvenme veya sorgular arası
doğrulama istisnası yoktur. Key/title/fields/links güncel payload'dan oluşturulur.
Yeni String'lerde eskiden gelen fazladan metin kapasitesi tutulmaz.

Başarılı çağrı total/sequence/hits'i değiştirir. Hata eski sonucu da tüketir ve
page'i total=0/sequence=0/hits=[] durumunda bırakır. `search` imzası ve sahipli
sonuç davranışı korunur. Yeni feature, bağımlılık veya kalıcı format yoktur;
64 KiB okuma penceresi ve kaynak/CRC/schema/UTF-8/key/version kontrolleri aynıdır.
Başka projeye, protokole veya servise entegrasyon yapılmadı.

## Tahsis ve doldurmayı ayıran dar ölçüm

Aynı corpus'un ilk 200 belgesindeki 5.383.655 metin byte'ı resident kaynak olarak
kullanıldı; yeni metin üretilmedi. Release, affinity=4, beş ısınma +60 ölçüm;
sıraları ters çevrilen 200-String blokları. Vektörün dış tahsisi, sonuç eşitlik
kontrolü ve bırakma süreleri saat dışında. Bu DB araması veya RAM ölçümü değildir.

| Medyan, 200 metin | Süre |
|---|---:|
| Yeni String tahsisi + doldurma (`to_owned`) | 2,484 ms |
| Yalnız kapasite ayırma | 0,585 ms |
| Yeni ayrılmış kapasiteyi ilk doldurma | 1,876 ms |
| Hazır String'i temizleyip tekrar doldurma | 0,366 ms |

Kapasite ayırma süresi saf allocator CPU maliyeti değildir; ilk doldurma ilk
erişim/page/cache etkilerini de içerir. Bu sayılar saf memcpy payını izole etmez
ve ayrı medyanlar toplanarak arama süresi çıkarılmaz. Kopyalama ortadan kalkmadı.
[Ham ölçüm](benchmarks/buffers-20261018-text.jsonl),
[özet](benchmarks/buffers-20261018-text-summary.json).

## Normal release sonuçları

Kontrol ve aday **aynı frozen executable** içinde farklı çağrı yollarıdır.
İki yol da önceki sonucun bırakılmasını ve Query hazırlığını süreye dahil eder.
Bu, eski `search` dönüşünü ölçüp sonucun bırakılmasını saat dışında tutan
raporlarla aynı dağılım değildir. Her sorgudan sonra total, sıra, tüm alanlar,
links ve version bağımsız corpus oracle'ıyla saat dışında karşılaştırılır.

| V3 son sürüm / yük | Örnek/yol/sorgu | P95 farkı | Tipik blok farkı | Hızlı çift/blok |
|---|---:|---:|---:|---:|
| Yalnız the0/200 tekrarı | 400 | **-%38,78** | **-%34,56** | **4/4;16/16** |
| Dört değişen sayfa, the0/200 | 400 | -%10,18 | -%4,06 | 4/4;12/16 |
| Dört değişen sayfa, ishmael60/200 | 400 | -%5,77 | -%1,75 | 4/4;10/16 |
| Dört değişen sayfa, the0/1 | 400 | **+%10,68** | -%2,84 | 0/4;11/16 |
| Dört değişen sayfa, boş quixote57/50 | 400 | +%0,17 | -%0,73 | 2/4;8/16 |
| Her seferinde boş tampon, the0/200 | 200 | +%0,92 | -%3,73 | 2/4;4/8 |

Saf tekrarda the200 p95 **6,6529→4,0728 ms**, p50 **5,0604→3,20655 ms**.
Bu, tamponu dolu çağıran için kazançtır; ilk kullanım için kazanç değildir.
Boş tampon kontrolünün dört case'i iki düşük/iki yüksek p95 verdi; düzenli
bir ilk-kullanım kazancı veya tüm sorgulara hızlanma iddiası yoktur.

Geniş 38-case, 80 örnek/yol/sorgu: 33 düşük/5 yüksek p95. the200 genel p95
yaklaşık değişmedi (+%0,01); tipik blok -%2,78 ve 7/8 blok hızlı. Geniş
dağılımın p95'i ile dar tekrarın p95'i birbirinin yerine kullanılamaz.
Kelime sorgularının çoğu zaten normal capture yoluna düşer; onların düşük
p95'i metin tekrar kullanımına atfedilmez. the0/1 geniş p95 +%21,53 (2/4 çift
hızlı), uzun negatif +%6,16 ve absent +%37,61 artışları açık tutulur. Uç
sürelerin kaynağı bütünüyle ayrılmadı; genel kapı kapanmadı.

[Saf tekrar](benchmarks/buffers-v3-same-10k-20261018-summary.json),
[değişen sayfalar](benchmarks/buffers-v3-focus-10k-20261018-summary.json),
[boş tampon](benchmarks/buffers-v3-fresh-10k-20261018-summary.json),
[38 sorgu](benchmarks/buffers-v3-matrix-10k-20261018-summary.json).

### Normal API'nin eski okuyucuyla kontrolü

Önceki 7cc6eb5 frozen normal okuyucuya karşı yeni prototip derlemesinin normal
`search` yolu da ayrıca ölçüldü. Tarihsel worker'da aynı 38 sorgu, dört çift,
iki blok, on tekrar: 80 örnek/case. Bu yük, yukarıdaki dedicated worker'dan
farklı olarak get'leri de sorgular arasına ekler ve sonuç bırakmayı arama
saatinin dışında tutar. Yüzdeler iki düzen arasında çıkarılmaz/toplanmaz.

İlk geniş koşu 2 düşük/36 yüksek p95 verdi. Son aday bloğunun toplam süresi
8,671 saniyelik kontrolden sonra 11,563 saniyeye çıktı. Bu tek başına sebebi
kanıtlamaz. Aynı frozen executable'lar, altı seçilmiş sorgu ve thread
CPU/cycle gözlemleriyle 400 örnek/case tekrarlandı: beş düşük/bir yüksek.
Watson ve uzun negatifin önceki büyük artışları bu dar yükte tekrarlanmadı.

the200 artışı ise kaldı: **5,2082→5,7293 ms p95 (+%10,01)**, yalnız 1/4 çift
hızlı; tipik blok +%0,76 ve 8/16 blok hızlı. Cycle p50 +%2,15/p95 +%6,14;
thread CPU zamanı 0/15,625ms adımlıdır. Cycle sabit frekanslı bir saat değildir;
bu sayaçlar iş/OS beklemesi/cache/frekans paylarını tam ayırmaz. Artışı yalnız
beklemeye bağlayacak kanıt yoktur; dalgalanma denilerek kapatılmaz. Tekrarlanan büyük
sonuç sayfasındaki kazanç, mevcut normal yolun bu kabul sorununu telafi etmiş
sayılmaz. Bu nedenle kod normal motora alınmayacak.

[38-case normal kontrol](benchmarks/buffers-default-10k-20261018-summary.json),
[400 örnekli thread kontrolü](benchmarks/buffers-default-repeat-10k-20261018-summary.json).

## Bellek ve reddedilen geniş varyantlar

### Ayrı diagnostic kontroller

Bir tam içerik kontrollü tekrar dizisinde diagnostic-memory, ikinci the200
çağrısında tahsis/realloc sayısını **5690→5491** ölçtü: 200 String tahsisi
kalktı, tek String-pool vektörü eklendi. Sonuç text capacity her iki yolda
5.383.655 B, result capacity 209; canlı requested-heap farkı 0. İkinci çağrının
ek requested peak'i 114.497→82.513 B. Bunlar Rust'un istediği heap byte'larıdır;
OS RAM/allocator bookkeeping/stack/cache değildir ve süreleri geçersizdir.

200→50→200→1→boş geçişi tam oracle'la geçti. Son boş sayfada text/result
capacity=0; bu, bütün proses heap'i sıfır oldu demek değildir. Ortak index/mask
cache ısınması son adımda iki yolda da +468.240 canlı requested byte bıraktı.
Adayın o boş sorgudaki transient requested peak'i +32.120 B yüksekti: önceki
tek metin 32.272 B + String-pool24 B - normal yolda tutulan eski result176 B.
Bu eşleşme gereksiz buffer ömrünü gösterir; OS private farkının tamamının
nedeni diye sunulmaz ve deney sonunda yeni bir eşik ayarı turu yapılmadı.

Ayrı diagnostic-timing tekrarında **200 text_reused**, her iki yolda aynı
**115 sonuç okuması, 200 CRC, 200 schema/UTF-8 ve 200 identity** kontrolü
görüldü. Kaynak doğrulaması korunur. Tek instrumented adımın text span'i
1,381→0,341ms idi; nested span'ler ve ayrı süreçler nedeniyle normal p95 veya
CPU yüzdesi değildir. Reuse helper'ında bazı metadata alt-span'leri bulunmasa
da fields/links decode'u yapılır; bunların sıfır maliyetli olduğu çıkarılmaz.

[Requested heap ve kapasite](benchmarks/buffers-v3-20261018-retention-summary.json),
[timing çağrı sayıları](benchmarks/buffers-v3-20261018-timing-summary.json).

### OS proses belleği

Ayrı, oracle'sız normal proseslerde aynı 24 arama/771 get, dört dengeli çift.
Strict açılış dahil proses ömrü OS yüksek su işaretleridir; sorgu anlık RAM'i,
toplam sistem RAM'i veya Rust requested-heap sayacı değildir.

| V3, dört proses medyanı | Normal | İsteğe bağlı reuse | Fark |
|---|---:|---:|---:|
| Peak private commit | 9.201.664 B | 9.230.336 B | +28 KiB (3/4 çift yüksek) |
| Peak working set | 13.303.808 B | 13.352.960 B | +48 KiB (4/4 çift yüksek) |

Bu, genel RAM kazancı veya sıfır gerileme kanıtı değildir. Sonuç metninde
eski kapasite fazlası tutulmaması OS allocator/stack/çalışma setinin aynı
olduğunu garanti etmez. [Ham bellek sonuçları](benchmarks/buffers-v3-memory-10k-20261018-summary.json).

İlk varyant eski StoredDocument'ları ve esnek text kapasitesini taşıyordu:
aynı sayfada -%38,07 p95 fakat karışık tek sonuç +%34,93 ve private medyan
+1.048.576 B; bu hali korunmadı. İkinci varyant exact kapasite/capture fallback
ile private farkını +239.616 B'ye indirdi, fakat eski metadata'yı da taşıyordu;
o da daraltıldı. V3 havuzda yalnız String taşır. Farklı zaman/executable ve
birden fazla değişiklik nedeniyle bütün OS farkı tek nedene nicel olarak
atfedilmez. Önceki ham sonuçlar ve iki source patch arşivlenmiştir.

## Koşullar ve ölçülmeyen sınırlar

Aynı mevcut `data/rkyv-corpus-20261009/json.payload-bench` dosyasının ilk 10.000
belgesi: sekiz kitap, 264.842.858 metin byte'ı, belge başına 2.057–51.197 byte.
Yalnız `data/benchmark-work/fdb` tekrar kullanıldı. Yeni corpus/DB kopyası,
reset, repack, hazırlama veya başka diske taşıma yapılmadı. SQLite bu tur
yalnız değişmezlik kontrolündedir; yeni SQLite hız karşılaştırması yoktur.

Affinity mask 4; sıcak OS cache, eviction yok. Her blokta tam kaynak/indeks
kontrolü ve oracle ısınması saat dışında. İki resident oracle worker ve tek DB
sahibi; aynı anda arama yapan iki DB prosesi yoktur. Dedicated sonuç tamponu
worker'ında query'ler arasında get yoktur; bellek probunda belirtilen 771 get
vardır. Derleme/ağır testler hız koşularıyla birlikte çalıştırılmadı. Tam makine
izolasyonu, gerçek-soğuk disk, concurrency veya SQL karşılaştırması iddiası yok.

## Son kontrol ve açık işler

Prototip default tam takım: **191 geçti/7 ignored**, hata yok. Bu takım
kesilme, tek byte bozulması, process-kill/kurtarma ve 0.1 fixture kontrollerini
de içerir. Referans engine17; all-features hedefli payload1/grouped5/engine1/
fixture1 geçti. Fmt-check ve default/all-features/no-default-features all-targets
Clippy `-D warnings` geçti. Diğer profillerin tam test takımları koşulmadı.
[Prototip komutları ve logları](benchmarks/buffers-v3-20261018-checks.json).

Altı engine/payload/segment/test dosyası f89b827 Git blob'larına byte-eşit
geri yazıldı; Cargo ve fixture değişmedi. Geri dönüşte engine18, payload2,
grouped5, 0.1 fixture1 ve bağımsız aktarım örneği1 geçti. Fmt-check ve üç
profil all-targets Clippy tekrar geçti. Geri dönüşte tam storage takımı tekrar
koşulmadı; byte-eşit kaynak, hedefli testler ve yeni normal release derlemesi
kontrol edildi. Normal reader'ın38-case tam oracle'ı ve CLI10k/seq20/the200
sıra-version kontrolü geçti. Bu yeni derlemeye hız/RAM kazancı atfedilmez;
önceki binary ile byte-eşit olduğu iddia edilmez.
[Geri dönüş kontrolleri](benchmarks/buffers-20261018-restored-checks.json),
[kaynak geri dönüş kanıtı](benchmarks/buffers-v3-20261018-restoration.json).

[Tam prototip kod/test yaması](benchmarks/buffers-v3-20261018-prototype.patch)
f89b827 tabanlıdır; yeni worker da patch'in içindedir. Patch mevcut geri
dönmüş kaynakta forward-check, prototip kaynakta reverse-check geçti.
[Worker'ın inceleme kopyası](benchmarks/buffers-v3-20261018-worker.rs) derleme
hedefi değildir. Aktif benchmark araçları frozen prototip executable'larını
kullanabilir; normal engine'de bu API/helper bulunmaz. İlk iki varyantın
patch'leri ve immutable executable hash manifestleri de aynı arşivdedir.

İlk/son **21 dosya SHA-eşit**: 18 FDB dosyası349.805.698 B, SQLite583.860.224 B
ve iki0.1 kaynak fixture'ı aynı. Corpus'un tamamı2.740.862.279 B ve
SHA-256 `11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B`
ile önceki corpus'a eşit; bütün hız koşularından sonra hash alındı. `data`
altında yalnız mevcut `benchmark-work` ve `rkyv-corpus-20261009` dizinleri var.
Host: Ryzen7 7840HS,8 çekirdek/16 logical; rustc1.99.0, LLVM23.1.1,
x86_64-pc-windows-msvc. [Final veri/CLI/derleme kanıtı](benchmarks/buffers-20261018-proof.json).

Genel karar: tekrar edilen büyük sonuç sayfasında dar, isteğe bağlı kazanç
vardır. Deneyin genel geçişi reddedildi; kod ölçüm yamasında saklanır.
Tek sonuç p95'i, Harpoon p95/genel arama farkları, normal Vec ingest RAM'i,
yazma/bakım, açılış/disk sıkıştırma ve eşzamanlı/gerçek-soğuk yük kapanmadı.
[Açık işler](OPEN-WORK.md).
