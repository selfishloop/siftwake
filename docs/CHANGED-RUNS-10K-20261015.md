# Değişmeyen Unicode işaretlerinde normalleştirme aralığını koruma

Önceki [chunk kopyalama denemelerinin](CHUNK-COPY-REJECTED-10K-20261015.md)
ardından normalleştirmenin içindeki tekrar azaltıldı. Aynı mevcut 10k/üç
çiftte 38 arama senaryosunun 35'inde p95 düşük, üçünde yüksek. Uzun olumsuz
tarama ve iki parçalı sorgulardaki kazanç tekrarlandı. Dar kazanç mevcut
`experimental-builder-budget` içinde korunur; **genel RAM/hız kapısı geçmedi,
default'a geçiş reddedildi.** Yeni feature, kütüphane veya kalıcı format yok.

## Değişiklik

Eski karışık-metin döngüsü, normalleştirmede değişmeyen kıvrımlı tırnak veya
uzun çizgi gibi her non-ASCII karakterde ASCII aralığını bölüp kopyalıyor,
o küçük aralığı lowercase ediyor ve Unicode karakteri ayrıca ekliyordu.

Yeni yol Unicode karakteri yine çözüp mevcut Türkçe eşleme/lowercase kuralıyla
karşılaştırır. Karakterin byte'ları değişmiyorsa aynı kopyalama aralığı sürer.
Yalnız gerçekten değişen veya birden fazla scalar'a açılan karakterde aralık
sonlandırılır. ASCII lowercase aralığın tamamına uygulanır; UTF-8 byte'ları
korunur. Tam ASCII yolu ve önceki 32-bayt prefix taraması aynı.

Ek metin cache'i, heap tamponu veya kelimeye özel ayrım yok. Uzun metnin
1024-bayt parçaları ve iki scratch tamponu aynı; reddedilen doğrudan-window
prototipleri yeniden eklenmedi. Payload/source CRC, checked schema, key/version,
visibility ve covering izinleri korunur. Default profil önceki karışık-metin
döngüsünü kullanır. İndeks normalleştirmesi/kalıcı byte'lar değişmez.

## Aynı 10k / üç çift arama

`changed-runs-matrix-10k-20261015`: değişiklikten hemen önce dondurulan
kontrol ve yeni aday, ikisi release/experimental-builder-budget/record-read;
diagnostic kapalı. Kaynak mevcut
`data/rkyv-corpus-20261009/json.payload-bench` dosyasının aynı ilk 10.000
belgesi, toplam 264.842.858 B gövde. Kaynak header'ındaki 100k sayısı koşu
büyüklüğü değildir. Yeni corpus hazırlanmadı/indirilmedi. Sabit FDB slotu
yeniden kullanıldı; SQLite bu adımda yeniden ölçülmedi.

30 temel sorgu + 8 sayfa; temel senaryoda 20 sample/proses (60 pooled),
sayfada 5 sample/proses (15 pooled), nearest-rank p95. İkinci çift ters sırada;
query/get sırası seed ile karıştırılır. Sıcak OS cache, manuel eviction yok.
Tam total, sıralama, belge ve version oracle'ı geçti. Oracle resident olduğu
için bu prosesin RAM'i motorun proses RAM'i olarak sunulmaz.
[Bütün sonuçlar ve her çift](benchmarks/changed-runs-matrix-10k-20261015-summary.json).

| Query / offset / limit | Kontrol p95 | Aday p95 | Değişim | Hızlı çift |
|---|---:|---:|---:|---:|
| zx /0 /50 |309,4394 ms |222,0487 ms |-%28,24 |3/3 |
| the zx /0 /50 |307,4579 ms |235,8283 ms |-%23,30 |3/3 |
| quixote whale /0 /50 |5,8261 ms |5,0007 ms |-%14,17 |3/3 |
| holmes elementary /0 /50 |56,2300 ms |44,9247 ms |-%20,11 |3/3 |
| whale harpoon /0 /50 |90,8706 ms |72,1021 ms |-%20,65 |3/3 |
| presumption /0 /50 |111,3639 ms |92,6527 ms |-%16,80 |3/3 |
| quixote /0 /50 |5,0116 ms |4,2886 ms |-%14,43 |2/3 |
| geologist /0 /50 |23,7121 ms |20,9521 ms |-%11,64 |3/3 |
| he /0 /50 |2,0401 ms |1,9974 ms |-%2,09 |3/3 |
| the /0 /1 |0,8735 ms |1,4578 ms |+%66,89 |0/3 |
| quixote /57 /50 |3,5182 ms |3,5675 ms |+%1,40 |2/3 |
| 300 karakterli olumsuz parça /0 /50 |0,0492 ms |0,0528 ms |+%7,32 |1/3 |

**35/38 düşük, 3 yüksek.** Özellikle tek sonuçlu `the` sayfasındaki
0,5843 ms artış üç çiftte de var; dalgalanma diye kapatılmadı. Diğer iki artış
0,0493 ms ve 0,0036 ms. Önceki kullanıcı dalgalanma kabulü bu yeni sonuçlara
uygulanmadı. Covering ile gövde taramayan `he`/`the` veya indeksin elediği
uzun olumsuz parça değişimleri, gövde normalizer kazancı olarak yorumlanmaz.
Bu adımda gerilemenin kök nedeni belirlenmedi.

Rastgele get pooled p95, 0–4/4–16/16–32/32+ KiB gruplarında sırasıyla
0,0243→0,0232 /0,0301→0,0288 /0,0381→0,0365 /0,0466→0,0441 ms;
missing key 0,0009→0,0009 ms. Get kodu değişmedi; bu farklardan get
optimizasyonu iddia edilmez. Açılış ve ilk source kontrolü bu matrisin
paired performans kapısı değildir.

## Normalleştirme, tarama ve doğrulama ayrı

`changed-runs-profile-20261015`: ayrıca diagnostic-timing açık üç çift
proses, ikinci çift ters sıra. Önceki wide-fold diagnostic executable'ının
SHA256'sı önceki artifact kaydıyla kontrol edildi. Aynı ilk10k/sabit DB,
record-read, sıcak OS cache/eviction yok. Query başına üç warm iteration;
önce proses içi medyan, sonra üç prosesin medyanı. Bunlar instrumented
**wall süreleridir**, CPU attribution veya üstteki normal API p95 değildir.
İç içe span'ler toplanmaz; sayaç maliyeti vardır.
[Sayaçlar ve her proses](benchmarks/changed-runs-profile-20261015-summary.json).

`the zx` için:

| Span | Kontrol medyanı | Aday medyanı |
|---|---:|---:|
| Gövde normalleştirme |130,950 ms |89,429 ms |
| Substring tarama |20,866 ms |20,570 ms |
| Payload okuma |51,438 ms |49,321 ms |
| Payload CRC |27,305 ms |27,715 ms |
| Checked payload doğrulama |27,488 ms |26,856 ms |
| Metin doğrulama toplamı (iç içe) |193,888 ms |155,016 ms |
| Filter/visibility/text toplamı (iç içe) |308,680 ms |268,856 ms |

Normalleştirme üç çiftte de düşük: proses medyanları136,938/129,298/130,950
→88,509/89,429/92,632ms. Çağrı sayısı263.627, aday10.000, istenen
payload267.849.329 B, okunan287.040.309 B, read request1104; iki varyantın
bütün warm iteration'larında aynı. Kazanç daha az belge okumaktan veya
CRC/validation atlamaktan gelmiyor. Normalleştirme hâlâ en büyük tek span;
okuma/CRC/schema/tarama ve parça döngüsü maliyetleri kalıyor.

`quixote`: aynı56 aday/1.945.325 B/815 gövde parçası; normalleştirme
0,537→0,351ms,3/3 düşük; scan0,065→0,066ms. `he` iki varyantta da
candidate payload/fold/scan0, covered250; sonuç hit'leri normal hydrate edilir.
Her profilde ilk arama pending source5→0 yaptı ve10k belgeyi kontrol etti.
İlk kullanımın başlangıç cache durumu ayrıca sabitlenmedi; açılış kazancı
iddia edilmez.

## Üç çift yazma / bakım / RAM

`changed-runs-checkpoints-10k-20261015`: her varyant üç ayrı ingest ve üç
idle prosesi, ikinci çift ters sıra. Aynı ilk 10k, streamed input, synced WAL,
20 adet 500-belgelik işlem; son bakım ayrı. Private/working set 20 ms
örneklenir, phase 100 ms okunur. Sıcak OS cache/eviction yok. Corpus
CRC/JSON decode, commit/index ve 40 belge kontrolü ingest kapsamındadır;
book/oracle resident değildir. Yalnız araca ait sabit FDB slotu sıfırlandı.
Her ingest sonrası bütün 10k belge/version ve dört query kontrolü geçti;
önceki reader her aday DB'yi de okudu. Son slot10k/seq20/delta0/resident0.
[Ham çiftler ve medyanlar](benchmarks/changed-runs-checkpoints-10k-20261015-summary.json).

| Üç prosesin medyanı | Kontrol | Aday |
|---|---:|---:|
| İndeksleme private tepe |9,133 MiB |9,176 MiB |
| Bakım private tepe |7,535 MiB |8,477 MiB |
| OS peak working set |12,836 MiB |13,211 MiB |
| Idle private |1,648 MiB |1,648 MiB |
| Ingest |31,352 s |31,069 s |
| Son bakım |0,990 s |0,965 s |
| Toplam |32,342 s |32,034 s |
| Retained .fdb/.idx/.ready |1.521.041.630 B |1.521.041.630 B |

İndeksleme private +45.056 B (~0,043 MiB), bakım +987.136 B (~0,941 MiB),
WS +393.216 B (0,375 MiB), idle aynı. İndeksleme/bakım private ve WS her
üç çiftte yüksek: **RAM'de sıfır gerileme yok.** İndeksleme9 MiB düzeyi
korundu; bakım artışı ve query tepe belleği ayrı açık. Private bytes fiziksel
RAM toplamı değildir; örnekleme kısa pikleri kaçırabilir. OS dosya cache'i bu
proses sayaçlarının tamamına dahil değildir.

Toplam süre2/3 çiftte düşük, medyan -%0,95; ilk çift +%0,51. Bakım1/3
çiftte düşük. Son çiftin toplamı32,551→27,105s iken ilk ikisi32s civarında;
ortam varyansı tam kontrol edilmedi. Değişiklik query normalizer'ındadır;
bu sayılardan genel yazma/bakım hız kazanımı veya RAM artışının nedeni
çıkarılmaz. Genel yazma/bakım hedefi kapanmadı.

Disk aynı: canlı source/index/WAL+ready621.589.411 B;
retained .fdb/.idx/.ready1.521.041.630 B; SQLite583.860.224 B.
Altı ingestten sonra arama snapshot'ındaki124 FDB/cache/SQLite dosyasının
byte sayısı ve SHA256'sı aynı. Corpus2.740.862.279 B/SHA256 da aynı:
`11f826fce3f4ae790feb9fc6df9b1169f52c2fd699e89fc4a8a4b77aaa41731b`.
[Hash teyidi](benchmarks/changed-runs-post-ingest-hashes-20261015.json).

## Doğrulama ve kalan kapsam

Odak testleri: bütün Unicode scalar'larının bağımsız fold ile eşitliği,
scratch reuse, 8/32-bayt prefix hizalama/sınırları ve uzun term/AND/UTF-8
chunk sınırları. Ek test, değişmeyen Unicode/combining işaretlerin arasında
ASCII büyük harfleri, Türkçe eşlemeleri ve çok-scalar lowercase dönüşümlerini
birlikte kontrol eder. `changed-runs-unit-20261015.log`:5 geçti.

Builder-budget/default/all-features all-targets testleri geçti. Son küçük
Clippy düzeltmesinden sonra all-features all-targets suite tekrar geçti.
Kesilme, tek-bayt bozulması, valid cache ile bozuk kaynak, source değişimi,
process kill/merge kill, mixed writer, update/delete/reopen ve eski JSON
fixture kontrolleri başarılı. Parent testlerin çağırdığı subprocess helper'lar
test listesinde ignored işaretlidir; kurtarma parent'ları geçti.0.1 fixture
değişmedi. `cargo fmt --check`, default ve all-features
`cargo clippy --all-targets -- -D warnings` ve `git diff --check` geçti.

Loglar `changed-runs-tests-{feature,default,all}-20261015.log`,
`changed-runs-tests-all-final-20261015.log`,
`changed-runs-clippy-all-final-20261015.log`,
`changed-runs-clippy-default-20261015.log`,
`changed-runs-fmt-final-20261015.log`. İlk Clippy logundaki sliced-string
uyarısı ara `remaining` binding'i ile düzeltildi; uyarı bastırılmadı.
Ölçülen frozen adaylarla son reader/memory executable'larının `.text`,
`.data`, `.pdata`, `.reloc` bölümleri byte düzeyinde aynı. `.rdata`/dosya
hash'leri farklıdır; bütün executable'lar birebir aynı diye sunulmaz.
Performans ölçümlerinin kullandığı artifact hash'leri kendi matrix/checkpoint
kayıtlarında, son artifact ve bölüm eşitliği
[burada](benchmarks/changed-runs-final-artifacts-20261015.json).

Diagnostic profil sonrası normal release reader yeniden build edildi;
reader ve memory example diagnostic kapalıdır. Son38-case total/tam sayfa/
version kontrolü geçti (`changed-runs-matrix-check-final-20261015.jsonl`).
`src/segment.rs` önceki SHA256 ile aynı; payload/source/index/WAL/manifest
formatları değişmedi. Final doğrulamalar sonrası124 DB/cache/SQLite dosyasının
hash'i tekrar aynı bulundu. Bu son doğruluk kontrolü yeni performans ölçümü
yerine sunulmaz.

[OPEN-WORK](OPEN-WORK.md): üç yeni p95 artışı, kalan normalleştirme/tarama,
yazma/bakım, normal/default API ve genel heap/query tepe, disk/eski nesiller,
eşzamanlı ve soğuk yük açık. 1000 aktif-belge performansı burada tekrar
ölçülmedi. Dış proje entegrasyonu yok;0.1 fixture korunur.
