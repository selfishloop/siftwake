# Uzun gövde indekslemesinde daha az ağaç araması — 2026-10-16

Sonraki [arama teşhisi](SEARCH-ATTRIBUTION-10K-20261016.md) ek query işi
bulmadı; aynı program/aynı CPU negatif kontrolü de ishmael için +%4,23
p95 verdi. Aşağıdaki tarihli ham sonuçlar korunur; bütün farkın koddan
veya yalnız gürültüden geldiği ilan edilmedi. Kod/yerleşim katkısı açık.

Son sürümde aynı mevcut 10 bin belge için yazma+son bakım medyanı
31,908→28,241 saniye: süre %11,49 daha kısa. Dört dengeli çiftin tamamında
kazanç var; çift farkları -%11,93/-10,81/-11,88/-11,17. CPU süresi de
dört çiftte düşük. Kalıcı kaynak ve indeks byte'ları sekiz koşuda birebir aynı.

Bu dar yazma optimizasyonu mevcut `experimental-builder-budget` içinde
tutulur; default kapalıdır. Son bakımın kendisi kapsamlı optimize edilmiş
sayılmaz. RAM artışı, diğer arama artışları ve genel/default kapısı aşağıdaki
sınırlarla açık kalır.

## Son sürümün eşleştirilmiş yazma sonucu

Normal release, diagnostic timing/allocation kapalı, dört ayrı proses çifti.
Sıra A/B, B/A, A/B, B/A; aynı kontrol ve aday SHA'sı her tekrarda korunur.

| İşlem, dört koşunun medyanı | Kontrol | Son aday | Süre farkı | Daha kısa çift |
|---|---:|---:|---:|---:|
| 10k yazma |30,859 s |27,290 s |-%11,57 |4/4 |
| Son bakım |0,988 s |0,940 s |-%4,84 |3/4 |
| Yazma + son bakım |31,908 s |28,241 s |-%11,49 |4/4 |
| Tüm proses CPU süresi |29,781 s |26,250 s |-%11,86 |4/4 |

Yazma süresi input CRC/JSON çözme, indeks oluşturma, senkron WAL commit ve
40 sınır belge get kontrolünü içerir. Yazma sırasında çalışan/bitmesi beklenen
flush/merge de bu süreye dahildir. Son bakım yalnız ingest sonrasındaki
flush/merge drain'dir. Yeni gövde çıkarımı merge'de yeniden çalıştırılmaz;
son bakımdaki küçük fark bağımsız bir merge algoritması kazancı ilan edilmez.

Medyanların toplamı ile toplam süre medyanı farklı hesaplamalardır; tabloda
toplam her prosesin yazma+bakım toplamından ayrıca hesaplandı. Çiftlerin
toplam süre oranlarının medyanı -%11,52; bu da medyanların oranıyla aynı
istatistik değildir. Dört çift anlamlılık testi veya her iş yüküne garanti
oluşturmaz.
[Ham tekrarlar, bellek ve çift oranları](benchmarks/gram-body-write-10k-20261016-summary.json),
[sıra ve koşullar](benchmarks/gram-body-write-10k-20261016-design.json).

## Ne değişti?

Önceki algoritma her geçerli üç Unicode karakterlik pencereyi
`BTreeSet<[char;3]>` içinde arıyordu; eşsiz pencereleri sonra String'e
çevirip mevcut sıralı terim kümesini oluşturuyordu.

Uzun gövdede yeni algoritma üç scalar'ı kayıpsız 63 bitlik geçici anahtarda
tutar. 21 bit/scalar birbirine karışmaz; `u64::MAX` gerçek anahtar olamaz.
1024 girişli, toplam 8 KiB stack tablosu yakın zamanda görülen tam anahtarı
tutar. Hash çakışması eşitlik kanıtı sayılmaz: tam anahtar aynı değilse ağaç
araması yapılır. Bu yüzden çakışma hiçbir terimi düşüremez. Tekrarsız anahtarlar
`BTreeSet<u64>` içinde tutulur; sonuç yine aynı `BTreeSet<String>` olur.
Whitespace bir kez denetlenip pencereyi keser; önceki üçlü-whitespace
semantiği korunur.

Son bağlama yalnız `BodyCache::get` içindeki en az 1024 byte'lık folded gövde
içindir. Küçük gövdeler ve sorgular eski `grams()` fonksiyonunu kullanır;
bu fonksiyonun kaynak gövdesi kontrolle birebir aynıdır. Yeni çıkarıcı
`inline(never)` ile ayrı tutulur. 8 KiB tablo çağrı sonunda biter; Index'e,
sorgu önbelleğine veya diske saklanmaz. Bu boyut toplam proses RAM sınırı değildir.

Payload codec, WAL/manifest/dizin biçimi, CRC/schema/kimlik kontrolleri,
transaction boyutu ve kalıcılık modu değişmedi. Yeni feature veya bağımlılık
yok; packed anahtar kalıcı dosya biçimi değildir. 0.1 fixture korunur.

## Teşhis ve önceki varyant

İlk mevcut-kod diagnostic profilinde `body.grams` 10,57 s,
`body.fold` 1,19 s sürdü. Ortak `grams()` girişine bağlanan ilk varyantta
çıkarım 6,36 s idi. Bu iç içe wall span'ler CPU payı gibi toplanmaz ve normal
release kazancı hesabında kullanılmaz.

O profilde 121.428.064 geçerli pencerenin 74.463.502'si (%61,32) ağaç
araması olmadan elendi; ağaç probe sayısı 46.964.562, toplam eşsiz terim
19.070.767 idi. Çıkarıcının fonksiyon gövdesi son sürümde aynı, bağlama
noktası ve inlining sınırı değişiktir. Bu profil zamanları son düzenlemenin
yeniden ölçülmüş diagnostic süreleri diye sunulmaz; son performans tablosu
ayrı dört normal çiftin sonucudur.

İlk varyant yazma+bakımda %10,27/4/4 kazandı fakat read matrix 13 düşük/25
yüksek p95 verdi. Artışların hepsinin nedeni belirlenmedi; yalnız QUİXOTE
her dört çiftte yükseldi. Ortak query fonksiyonuna ek yol tutmak yerine son
sürüm yalnız body üretimine bağlandı. İlk varyantın sayıları son sürümle
karıştırılmaz veya farklı zamanlı iki medyan doğrudan birbirine oranlanmaz.
[Kontrol profili](benchmarks/gram-key-profile-control-20261016-fdb-ingest-10000-0.jsonl),
[ilk aday profili](benchmarks/gram-key-profile-candidate-20261016-fdb-ingest-10000-0.jsonl),
[ilk varyant yazma](benchmarks/gram-key-write-10k-20261016-summary.json),
[ilk varyant read matrix](benchmarks/gram-key-read-matrix-10k-20261016-summary.json).

## RAM ve disk

Proses ömrü boyunca Windows `GetProcessMemoryInfo` tepe sayaçları, proses
sonlandıktan sonra elde tutulan handle ile okundu; kısa tepe için örnekleme
boşluğu yoktur. Bunlar opening/input/indexing/maintenance toplam proses
ömrüdür; yalnız indeksleme heap'i veya sistem RAM'i değildir.

| Sayaç, dört proses medyanı | Kontrol | Son aday | Fark / çift durumu |
|---|---:|---:|---:|
| OS tepe private commit |9,021 MiB |9,107 MiB |+88 KiB; 2 düşük/2 yüksek |
| OS tepe working set |12,986 MiB |13,014 MiB |+28 KiB; 1 düşük/3 yüksek |
| Örnekli indeksleme private |8,947 MiB |9,107 MiB |+164 KiB; 1 düşük/3 yüksek |
| Örnekli bakım private |8,383 MiB |8,314 MiB |-70 KiB; 2 düşük/2 yüksek |

Private commit fiziksel RAM değildir; working set shared image sayfalarını
da içerir. Faz sayaçları 20 ms örnek/100 ms faz poll ile gözlenmiştir;
faz sınırları ve kısa tepeler kaçabilir. Sıfır RAM artışı şartı kanıtlanmadı.
Normal/default API'nin owned batch/genel heap sorunu, query tepe belleği ve
eşzamanlı yük bu sayaçlarla kapanmaz.

Sekiz yazma koşusunda tutulan .fdb/.idx/.ready 1.521.041.630 B ve aynı
122 dosyanın tüm SHA256'ları birebir aynı. Canlı kaynak/index/WAL+ready
621.589.411 B; eski nesil birikimi değişmedi. Disk kazancı yoktur.

## Son sürümün arama kontrolü

Aynı son DB, dört dengeli çift, diagnostic kapalı normal reader, mevcut
30 query +8 sayfa. Primary20/page5 örnek/proses; her sonuç tam belge,
key/version/sequence/total oracle'ıyla ve256 get anahtarıyla denetlendi.
Sıcak OS cache/warmup, eviction yok; ilk source doğrulaması zaman dışıdır.
Kaynak doğrulaması/checksum/record-read cache modu iki executable'da aynıdır.

Son matris33 düşük/5 yüksek pooled p95 verdi. Beş artışın hiçbiri dört
çiftin tamamında yükselmedi; bu onları otomatik dalgalanma diye kapatmaz.
Sorgu `grams()` kaynak gövdesi kontrolle aynıdır ve yeni çıkarıcı arama
yolunda çağrılmaz. Bu kaynak ayrımı her p95 farkının nedenini açıklamaz;
derleme yerleşimi ve ortam etkileri tamamen ayrıştırılmadı. Arama hızlanması
bu yazma değişikliğinin kanıtlanmış CPU kazancı diye sunulmaz. Eski açık
aramalar ve yeni beş pooled artış genel kapı kapsamında izlenir.
[Bütün38 durum/ham örnekler/çift farkları](benchmarks/gram-body-read-matrix-10k-20261016-summary.json),
[sıra tasarımı](benchmarks/gram-body-read-matrix-10k-20261016-design.json).

Geniş matristeki beş artış (sayfalar yalnız5 örnek/proses):

| Sorgu / offset / limit | Pooled p95 farkı | Daha hızlı çift |
|---|---:|---:|
| holmes /0/50 |+%0,49 |2/4 |
| İSHMAEL /0/50 |+%2,38 |2/4 |
| the /5000/50 |+%3,41 |1/4 |
| ishmael /60/200 |+%5,19 |1/4 |
| quixote /28/10 |+%13,29 |1/4 |

Bu nedenle ayrıca200 örnek/proses, toplam800 örnek/sayfa ile dört dengeli
çift çalıştırıldı. Yoğun test1 düşük/7 yüksek pooled p95 verdi:

| Sorgu / offset / limit | Kontrol→aday p95 ms | Fark | Daha hızlı çift |
|---|---:|---:|---:|
| the /0/1 |0,34→0,35 |+%2,59 |2/4 |
| the /0/200 |7,91→8,30 |+%4,98 |2/4 |
| the /5000/50 |2,95→2,97 |+%0,76 |2/4 |
| the /9950/50 |2,59→2,55 |-%1,48 |2/4 |
| ishmael /130/1 |8,98→9,42 |+%4,87 |0/4 |
| ishmael /60/200 |17,84→18,64 |+%4,46 |1/4 |
| quixote /28/10 |2,88→2,99 |+%4,03 |1/4 |
| quixote /57/50 |2,55→2,60 |+%2,34 |1/4 |

Seyrek testteki quixote %13,29 farkı yoğun testte aynı büyüklükte değildir;
fakat artış yönü sürer. Ishmael tek-sonuç sayfası dört çiftte de yavaştır.
Bu artışlar ölçüm dalgalanması diye kapatılmadı. Sorgu fonksiyonunun kaynak
eşitliği, derleme/yerleşim/ortam etkisinin tümünü ayrıştırmaz ve gözlenen
gecikmeyi ortadan kaldırmaz. Yazma kazancı vardır; genel hız/RAM kapısı
geçilmedi. Default'a geçiş reddedildi; dar yazma yolu mevcut deneysel
profilde kalır, sayfa artışları sonraki iş olarak açıktır.
[Yoğun sayfalar/ham örnekler/çift farkları](benchmarks/gram-body-read-pages-10k-20261016-summary.json).

## SQLite referansı

Aynı oturumun önceki varyant ölçümünde SQLite iki ayrı kez aynı ilk 10k ile
yeniden ölçüldü: yazma 41,522 s, son bakım 1,358 s, toplam 42,881 s; OS
private commit 8,408 MiB, working set 13,238 MiB. Son body bağlamasında SQLite
yeniden koşturulmadı; aynı comparator SHA'sı ve kayıtlar referans olarak korunur.
Bu iki koşu son dört FDB çiftine eşleştirilmiş veya aynı zaman aralığında
ölçülmüş değildir. Bu sayıların oranı bir SQLite/FDB kabul kapısı sayılmaz.

SQLite docs JSON + folded text + FTS5 trigram/LIKE düzeni, cache 2 MiB,
mmap 0/temp FILE, WAL + synchronous FULL. Son bakım FTS optimize + WAL
checkpoint(TRUNCATE); FDB bakım algoritmasıyla birebir aynı iş değildir.
Her referansta 10k payload kayıpsız doğrulandı. Son DB'de ayrıca dört query
total/ilk50 tam sayfa, PRAGMA integrity_check ve FTS external-content
integrity geçti; kontrol transaction'ı rollback edildi ve DB SHA aynı kaldı.
Validator SQLite 3.53.1, timed comparator SQLite 3.53.2; validator süresi
performans sonucuna dahil değildir. FDB engine'e SQLite bağımlılığı eklenmedi.
[SQL veri/koşu referansları](benchmarks/gram-key-write-10k-20261016-summary.json),
[SQLite integrity](benchmarks/gram-key-sqlite-integrity-20261016.jsonl),
[SQLite query doğrulaması](benchmarks/gram-key-sqlite-queries-20261016.jsonl).

## Ölçüm kapsamı ve araç düzeltmesi

Aynı mevcut `data/rkyv-corpus-20261009/json.payload-bench` ilk 10.000 kayıt;
264.842.858 B gövde, 2.057–51.197 B/pasaj. Yeni corpus/download/prepare yok.
Yalnız sahiplik işaretli sabit `data/benchmark-work/fdb` ve `sqlite` slotları
yazma ölçümü gerektiğinde sıfırlandı. DB kopyası tutulmadı. Son sürüm sekiz
FDB ingest; SQLite slotu son sürümde reset edilmedi. WAL senkron, 20 transaction,
500 kayıt/transaction. OS cache; eviction/affinity/power izolasyonu yok.

İlk varyantın son çiftinde faz monitor'ü çoklu satırı array fazı olarak
işleyip hata verdi; o eksik deneme dışlandı. Son çift iki tarafıyla yeni
prefix altında tekrarlandı, ilk üç geçerli çift korundu; birleşik sıra dengeli.
Eski hata çoklu faz satırıyla yeniden üretildi. Monitor şimdi son satırdaki
string fazı seçer; tamamlanmamış JSON'da son geçerli fazı korur. Son sürümün
sekiz ingestinin tamamı düzeltilmiş aynı araçla çalıştı.
[Poll hatası/düzeltme kontrolü](benchmarks/gram-key-phase-poll-reproduction-20261016.json).

## Son doğrulama

Son sürümde all-targets testleri: default88, builder-budget147,
all-features166 geçti; sırasıyla2/4/6 subprocess helper ignore'dur. Paketleme/
hash çakışması/Unicode-whitespace/NUL/max-scalar testleri bağımsız pencere
oracle'ıyla, body bağlaması da BodyCache üzerinden kontrol edildi. Kesilme,
tek byte bozulması, process kill/merge kill,0.1 fixture ve eski okuyucu
uyumluluk testleri dahildir. Hiçbir authoritative kaynak kontrolü kaldırılmadı.

Default/all-features/diagnostic-only `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check` geçti. Son diagnostic kapalı memory/reader `.text`
bölümleri ölçülen adaylarla birebir aynı; tüm executable SHA'ları PE
metadata nedeniyle aynı olmak zorunda değildir. Son38-case tam belge/
version/sequence kontrolü geçti.0.1 fixture Git durumu boş ve corpus
SHA256'sı önceki snapshot'la aynı.122 core dosya ilk varyantın kontrolüyle
de aynı; engine/segment/covering/Cargo kaynak hash'leri korunur.
[Son kaynak, test, executable ve dosya kanıtı](benchmarks/gram-body-final-proof-20261016.json),
[son38-case kontrolü](benchmarks/gram-body-final-read-check-20261016.jsonl).

## Açık işler

Yazma CPU maliyetinde dar ve tekrar eden kazanç vardır. Son bakım/merge
tamponları ve payload'ın tekrar çözülüp kodlanması ayrı sonraki adaylardır;
bu değişiklik onları çözmez. Kalan arama artışları, query/normal API RAM'i,
disk/eski nesil temizliği, concurrent ve gerçek cold yük açıktır.
[İş takibi](OPEN-WORK.md).
