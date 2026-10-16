# İki karakterli olumlu aramada mevcut indeks kanıtı

Önceki [covered-text adımının](COVERED-TEXT-10K-20261014.md) devamı.
Yeni feature, kalıcı format, belge corpus'u veya dış proje entegrasyonu yok.
Kod mevcut `experimental-builder-budget` içindedir; default kapalıdır.

## Genel kural ve sınırları

Bir scope'taki üç karakterlik gram posting'inin count'u run'ın bütün canlı
kayıt sayısına eşitse, o gram her canlı belgede vardır. İçindeki iki Unicode
karakterlik alt parça da bu belgelerde kesin vardır. Örneğin `the` bütün canlı
belgelerde bulunuyorsa `th` ve `he` için tekrar payload taramak gerekmez.
Kural gram adlarından bağımsızdır; benchmark kelimelerine özel tablo yoktur.

Mevcut directory okunurken en fazla 64 tam-kapsama gramı ve 16 KiB owned
buffer/string kapasitesi/run tutulur. Bu sınır allocator/proses RAM sınırı
değildir. Liste sınırlı olduğundan kanıt bulunmaması eşleşme yok demek değildir.
Scope karşılaştırılır; Unicode scalar uzunluğu normalleştirmeden sonra sayılır.
İzole iki karakterlik kelimeler de gram üretmez: böyle bir belge mevcut payload
kontrolünden geçer. Bir karakterlik veya üçten uzun sorgu için bu kısa parça
kuralı uygulanmaz. AND'de sadece kanıtlanan parçalar içerik kontrolünden çıkar.
Collection/field/link filtreleri ve silinmiş/örtülmüş kayıt maskesi korunur.

Özet records alanı önce okunan directory'den türetilir. Postings alanı önce
gelen geçerli eski JSON directory de okunur; canlı count henüz bilinmediğinden
kısa parça kanıtı oluşturulmaz, mevcut taramaya dönülür. SEG03/JSON payload ve
rkyv benchmark overlay'i mevcut doğrulamayı kullanır. Dosya biçimi değişmez.

Source kontrolü tamamlanmadan kanıt kullanılmaz. Önceki source/index
length+mtime guard'ı, CRC/source kontrolleri ve sonuç sayfasının payload
checksum/schema/key/version doğrulaması korunur. Fingerprint timestamp'i
koruyan dış değişime karşı hash/snapshot garantisi değildir; motorun exclusive
DB ownership modeli ve yeniden açılıştaki kaynak doğrulaması geçerlidir.

## 10k arama: üç eşleştirilmiş proses

`short-proof-matrix-10k-20261014`: kontrol önceki son covered-text executable,
aday yeni kısa parça kanıtı. İkisi release/experimental-builder-budget,
diagnostic kapalı, record-read açılış. Aynı mevcut10k/sabit FDB slotu,
30 temel arama ve8 sayfa. İkinci çift ters sıra. Temel case20 sample/proses
(60 pooled), sayfa5 sample/proses (15 pooled), nearest-rank p95. Sıcak OS cache,
eviction yok, sorgular seed ile karıştırılır. Tam total/sıralama/belge/version
oracle'ı geçti. Oracle belgelerini tutan bu proseslerden motor RAM'i çıkarılmaz.
FDB/cache ve SQLite DB hash'leri aynı. SQLite bu adımda yeniden ölçülmedi.
Ham sonuçlar ve çalıştırılan executable hash'leri aynı prefix altındadır;
[arama özeti](benchmarks/short-proof-matrix-10k-20261014-summary.json).

| Query / offset / limit | Önce p95 | Sonra p95 | Hızlı çift |
|---|---:|---:|---:|
| he /0 /50 |132,2703ms |1,8072ms |3/3 |
| the /0 /50 |2,0827ms |2,0308ms |1/3 |
| man /0 /50 |2,5394ms |2,5357ms |1/3 |
| quixote /0 /50 |4,7388ms |4,6370ms |1/3 |
| zx /0 /50 |290,3257ms |295,4119ms |0/3 |
| the zx /0 /50 |292,4698ms |311,7970ms |1/3 |
| the /0 /1 |0,9760ms |1,5892ms |1/3 |
| the /5000 /50 |2,5213ms |2,3935ms |1/3 |
| the /9950 /50 |2,2772ms |2,0315ms |2/3 |
| quixote /28 /10 |3,4440ms |4,5661ms |1/3 |

`he` p95 %98,63 düşük; üç çiftte de hızlı. Diğer iki karakterlik olumlu
parçalar için genel kural unit/oracle testleriyle doğrulandı, ayrı performance
sayısı üretilmedi. **15/38 p95 düşük,23 yüksek:** genel sıfır-regresyon kapısı
geçmedi. Harpoon+%4,83, darcy+%3,62, presumption+%2,78 ve zx+%1,75 bütün
üç çiftte yavaş. Tek sayfalarda p95 daha büyük artışlar da var; eski dalgalanma
kabulü bu yeni sonuçlara uygulanmaz. `he` kazancı bütün aramaların hızlandığı
anlamına gelmez. İki karakterlik olumsuz/kanıtsız parça ve uzun parçaların
payload okuma/normalleştirme/tarama maliyeti açık kalır.

Record-read açılış medyanı bu üç proseste kontrol 160,15ms / aday 158,72ms;
üç örnek, p95 açılış kapısı değildir. Pending source kontrolü search'e taşınır;
bu süreler tam source kontrolüyle birlikte açılış süresi gibi sunulmaz.

## İçerik okuma, doğrulama, normalleştirme ve tarama

`short-proof-profile-20261014.jsonl`: aynı mevcut 10k, diagnostic-timing açık
tek proses. Manüel OS cache eviction yapılmadı; başlangıç cache durumu kontrol
edilmedi. Aşağıdaki iteration1 örnekleri query warmup sonrasıdır, paired p95
kapısı değildir. İç içe span'ler toplanmaz. Sayaçlar sadece aday contains
kontrolünü kapsar; sayfa hydration ve source kontrolü ayrıca vardır.

| Query /offset0 /limit50 | he | quixote | the zx |
|---|---:|---:|---:|
| İstenen payload byte |0 |1.945.325 |267.849.329 |
| Gerçek payload read byte |0 |1.945.325 |287.040.309 |
| Payload read request |0 |56 |1104 |
| Payload okuma span |0 |0,398ms |54,548ms |
| Payload CRC span |0 |0,187ms |29,099ms |
| Payload validation span |0 |0,414ms |29,024ms |
| Gövde normalleştirme span |0 |0,578ms |136,879ms |
| Gövde substring tarama span |0 |0,068ms |23,245ms |
| Filter/visibility/text toplam span |0,410ms |2,152ms |329,591ms |

`he` için candidate payload okuma/CRC/normalleştirme/tarama kalktı; 250
prefix adayına indeks kanıtı uygulandı. Dönen 50 belge normal CRC/schema/
identity kontrolüyle hydrate edildi (ayrı span 0,641ms). `the zx`te `the`
kanıtlanır, `zx` kanıtlanmaz: bütün 10k adayın içerik kontrolü sürer. Gruplu
okumada actual read ile requested byte aynı değildir; overread gizlenmez.
Bu örnekte kalan büyük pay normalleştirmededir. Uzun quixote için gram
kesişimi sadece 56 adayı bırakır, fakat gerçek substring hâlâ doğrulanır.

Bu prosesin ilk `the` araması pending source5→0 ve 10k kaynak belge kontrolü
yaptı: source.verify_total544,26ms, API546,88ms; open415,54ms. Kaynak
kontrolü korunur. Başlangıç cache durumu kontrol edilmediğinden bu tek örnek
önceki ilk-arama ölçümleriyle hız/yavaşlama kanıtı gibi karşılaştırılmaz.
Warm `he`1,81ms sonucu ilk kaynak kullanımına veya soğuk cache'e genellenmez.

## Yazma, bakım, RAM ve doğrulama

`short-proof-memory-10k-20261014`: aynı mevcut ilk 10.000 belge, üç paired
proses, ikinci çift ters sıra; iki varyant release/experimental-builder-budget
ve streamed input. 20 synced 500-belge işleminde input CRC/JSON decode,
commit/index ve 40 belge kontrolü ingest süresinde; son merge ayrıca ölçülür.
Her koşunun dışında bütün 10k belge/version ve dört query doğrulandı; önceki
executable da adayın verisini okudu. Yalnız sabit FDB slotu sıfırlandı, corpus
yeniden hazırlanmadı. Son slot 10k/seq20/delta0 olarak bırakıldı. Sıcak OS cache,
manuel eviction yok. Private/WS 20ms örneklenir, phase 100ms poll edilir.
[Yazma/RAM özeti](benchmarks/short-proof-memory-10k-20261014-summary.json).

| Medyan ölçüm | Önce | Sonra |
|---|---:|---:|
| Ingest |31,773s |31,273s |
| Son bakım |1,099s |0,985s |
| Her koşunun ingest+bakım toplamı |32,872s |32,247s |
| İndeksleme private tepe |9,086MiB |9,090MiB |
| Bakım private tepe |8,113MiB |7,836MiB |
| OS peak working set |12,789MiB |13,090MiB |
| Idle private |1,688MiB |1,645MiB |
| Retained disk |1.521.041.630 B |1.521.041.630 B |

Toplam medyan %1,90 düşük; üç çiftte de düşük (%0,95–2,17). Bu dar adımın
süreleri düşük olsa da genel yazma/bakım hedefi kapanmaz. İndeksleme private
medyanı +4.096 B, bakım -290.816 B, idle -45.056 B. OS peak working set
+315.392 B (~0,30 MiB); üç çiftte de daha yüksek. Private bytes toplam fiziksel
RAM değildir; bu ölçüm query tepesini içermez. İlk çiftte indexing +0,16 MiB
görüldü, üç çiftin medyanı +4 KiB oldu; ikisi birbirinin yerine sunulmaz.
Disk bütün altı koşuda aynı. Kaynak/index/ready biçimleri değişmedi. Canlı
source/index/WAL 621.588.895 B, son ready 516 B ile 621.589.411 B; source
sayacı 45 B WAL'i zaten içerir, tekrar eklenmez. Retained sayı `.fdb/.idx/.ready`
dosyalarını kapsar; eski nesiller hâlâ tutulur. SQLite DB 583.860.224 B;
aynı veride disk farkı kapanmadı. Altı ingest sonrasında da FDB/cache ve SQLite
dosyalarının hash'leri arama ölçümündeki snapshot ile birebir aynı:
`short-proof-final-artifacts-20261014.json`.
SQLite RAM/yazma bu adımda yeniden ölçülmedi; önceki sayılarla yeni paired
SQLite karşılaştırması iddia edilmez. 1000 aktif belge performansı bu adımda
tekrar ölçülmedi; update/delete/reopen correctness testi eklendi.

Yeni testler: scoped iki karakterlik Unicode kanıtı, counts/live/tombstone,
özetin entry/byte sınırı, reversed directory order fallback, izole kısa metin,
AND/collection/fields/sıra/offset/update/delete/reopen oracle karşılaştırması;
kaynak izni yokken taramaya dönüş ve warm search sonrası kaynak veya
seçilmeyen indeks payload'ı değişince reddetme. Sağlam cache bozuk `.fdb`
kaynağını gizlemez. Builder-budget, default ve all-features all-targets test
suites geçti; kesilme/tek bayt bozulması/ACK öncesi ve sonrası process kill/
merge kill/mixed-writer kontrolleri başarılı. Default ve all-features
`cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` geçti.
0.1 fixture'a dokunulmadı. Loglar `short-proof-tests-{feature,default,all}-20261014.log`,
`short-proof-clippy-{default-final,all}-20261014.log`, `short-proof-fmt-final-20261014.log`.
Diagnostic build sonrası normal release examples yeniden build edildi;
son normal artifact 38 case doğruluk kontrolünü tekrar geçti:
`short-proof-matrix-check-final-20261014.jsonl`. Bu son check performans ölçümü
değildir; ölçülen frozen executable hash'leri matrix/memory metadata'dadır.

Dar kısa-parça kazancı opt-in kodda korunur. **Genel hız kapısı başarısızdır:**
23/38 p95 yüksek ve WS biraz yüksek. Default feature seçimi aynı kaldı.

[OPEN-WORK](OPEN-WORK.md): yazma/bakım, diğer p95 artışları, RAM'in normal API /
genel heap ve query tepe kapsamı, disk/eski nesiller, eşzamanlı ve soğuk yük açık.
