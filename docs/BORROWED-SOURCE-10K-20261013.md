# İlk arama: kaynağı belge kopyalamadan doğrulama

## Değişiklik ve güvence

`open_deferred` / `open_record_reads` sonrasındaki ilk arama, kullanılmamış
immutable kaynakların tamamını doğrulamaya devam eder. FDBLOG04/TXN2 için
`verify_deferred_source`, yalnız kontrol amacıyla bütün title/text/fields
değerlerini owned StoredDocument nesnelerine çözmek yerine OFF1 borrowed
görünümünü kullanır. Delete anahtarları önceki küçük owned yolda kalır.

Frame header/payload/footer CRC, tam kaynak CRC, checkpoint/sequence/sınır
kuralları ortak replay kodunda korunur. UTF-8, bütün metadata grammar/depth,
anahtar kontrol karakterleri/uzunlukları, text/field/link limitleri, field
isimleri, link scope ve sürüm limitleri kontrol edilir. Manifest record count
ve sequence karşılaştırması aynı kalır; bütün kontrol bitmeden kaynak
verified olarak işaretlenmez. Negative get ve write guard da bu yolu kullanır.

Strict açılış ve experimental-streaming-open önceki replay yolunu kullanır.
Pozitif record-get, arama posting seçimi, normalleştirme, sayfa toplamı ve
hydration algoritması değiştirilmedi. Yeni feature veya kalıcı/cache formatı
yok; `.fdb/.ready/.idx/.rmp/.svfy/.vfy` yeniden üretilmez.0.1 fixture korunur.

## Profil

Diagnostic build tek önce/sonra gözleminde ilk quixote search501,01→310,42 ms;
source verification454,61→278,83 ms. Önce batch decode177,77 ms ve document
validation38,45 ms; sonra borrowed batch validation54,17 ms. Frame CRC140,31
→136,60 ms, read+allocate85,18→76,31 ms. Nested span'lar toplanmaz; bunlar
wall-time, CPU attribution veya bağımsız tekrarlı performans kanıtı değildir.
Profil dosyaları: `benchmarks/first-search-profile-{before,after}.jsonl`.

## Tekrarlı ölçüm sözleşmesi

Mevcut aynı ilk10.000 belge: `data/rkyv-corpus-20261009/json.payload-bench`,
264.842.858 text byte, belge başına2.057–51.197 byte, sekiz Gutenberg kitabının
benzersiz/örtüşen pasajları. Yeni corpus/DB/reset/prepare yok. Tek mevcut FDB
slotu `data/benchmark-work/fdb`;10k live doc,5 immutable source,sequence20,
delta0. İki executable release, merge-postings +delta-bitmaps, diagnostic
kapalı; kontrol aynı opt-in record-read API'nin değişiklik öncesi sürümü.

Runner `tools/measure-borrowed-source.ps1`: beş dönüşümlü fresh-process çiftinde
open→get9999→ilk quixote search→repeat search; get sonrası iki tarafta pending5,
search sonrası0. İlk sorgu toplam56/ilk50 belge, sürüm20; repeat full hits eşit.
Bu ilk-use probe tam bağımsız corpus oracle'ı değildir. Üçer warm read process
ise the/quixote/he/the man için kesin toplam ve sıralı50 full StoredDocument
version oracle'ına eşitliği, rastgele dağıtılmış100 get anahtarını doğrular.
Motor başına9k timed get ve query başına90 timed örnek; nearest-rank pooled
p95/p99, p99 bu örnek sayısında maksimumdur. Oracle work timer dışındadır.

OS cache sıcak, eviction yapılmaz. İlk use'ta kaynak CRC/grammar kontrolü timer
içindedir; warm sorgularda kaynak verified durumdadır. Explicit full integrity
ayrı ölçülür. Tüm mevcut DB/cache dosyalarının byte/SHA256 önce/sonra karşılaşır.
SQLite,38-query matrisi, RAM, cold cache ve concurrent load bu turda ölçülmez.
SQLite~9 ms connection açılışıyla bu tam doğrulama işi eşit sayılmaz.

## Sonuç ve karar

Beş dönüşümlü fresh-process çiftinin medyanları; iki tarafta aynı record-read
API, aynı source cache ve get sonrası pending5:

| Ölçüm | Owned validation kontrol ms | Borrowed validation ms |
|---|---:|---:|
| Açılış | 73,3577 | 71,8444 |
| İlk get9999 | 0,1919 | 0,1982 |
| İlk quixote search | **528,7872** | **336,1228** |
| Open+get+ilk search | **602,3599** | **406,9386** |
| İlk search sonrası repeat | 5,6606 | 5,9977 |

İlk search medyanı **%36,44**, tam open+get+ilk search medyanı **%32,44**
azaldı. İlk search eşleşmelerinin5/5'inde hızlı; kontrol528,79/540,80/537,48/
501,54/481,11 ms, aday332,78/336,95/339,37/336,12/305,53 ms. Fark,
önceki deferred-get / record-get raporundaki4/5 source karşılaştırması değildir;
bu turda her iki tarafta da ilk search beş kaynağı tamamen kontrol eder.
İlk get~0,2 ms kalır; bu küçük farklardan open/get kazancı iddia edilmez.

Üçer warm process, motor başına9.000 timed get ve her query90 örnek; toplam
18.000 get +720 timed search'ın full document/version oracle'ı geçti:

| Warm işlem | Kontrol p95 ms | Borrowed p95 ms | Kontrol p99 ms | Borrowed p99 ms |
|---|---:|---:|---:|---:|
| get | 0,0824 | 0,0800 | 0,1215 | 0,1184 |
| the | 374,9198 | 374,3085 | 414,4632 | 385,7871 |
| quixote | 8,4950 | 8,6106 | 9,4369 | 8,7542 |
| he | 378,4118 | 376,6114 | 402,7385 | 388,9147 |
| the man | 387,1806 | 387,9969 | 440,6259 | 402,4533 |

Warm quixote pooled p95 **%1,36 /0,1156 ms arttı**; AND **%0,21 /0,8163 ms**
arttı. Quixote per-process p95 8,1725/8,7318/8,2445 →8,5557/8,7316/8,2205;
AND386,1873/384,2329/416,3114 →387,3963/396,9224/360,9817 ms. Bu artışları
gizlemiyoruz; genel sıfır-regresyon veya warm arama hızlanması ilan etmiyoruz.
Önceki REF01 warm regresyon işinin tamamı da bu dört query'yle kapatılmaz.
Timed first-use repeat örnek sayısı5'tir; warm tablo onun yerine90 örnek kullanır.

Explicit full integrity711,20 ms tek örnektir; strict replay değişmediği için
bu fark hızlandırma kanıtı değildir. Bütün mevcut DB/cache dosyalarının byte ve
SHA256 değerleri aynı; **ek disk verisi0 B**, yeni metadata/cache/corpus yok.
RAM/working set ölçülmedi; daha az belge kopyası RAM kazanımı olarak sunulmaz.
Sonuçlar `benchmarks/borrowed-source-10k-20261013-summary.json`, tüm paired
ham JSONL'ler ve before/after hash raporlarıyla yeniden incelenebilir.

Karar: mevcut opt-in deferred/record-read native doğrulamada bu dar değişiklik
korunur; ilk kullanım kazancı tekrarlanmıştır. Record-read API default'a
alınmaz, strict açılış değişmez. Warm regresyon kapısı hâlâ açık; SQLite genel
karşılaştırması veya bütün aramaların hızlı olduğu iddiası yok.

## Doğrulama

Default ve all-features `cargo test --all-targets` geçti; native binary9/9
testleri iki yapıda da source/cache cuts, single-byte corruption ve deferred
read/search/write guard'larını kapsar. Mevcut process-kill/cache publication ve
0.1 compatibility testleri geçti. Yeni borrowed/owned document-limit eşitliği
ve borrowed frame her kesilme/her tek byte bozulması testleri default lib'de
geçti. All-features'ta streaming yolu korunur; borrowed-limit eşitliği testi
yine çalışır. Son ek CRC'leri sağlam fakat invalid version/key/field/link ve
manifest count/sequence testi default'ta ayrıca geçti. `cargo fmt --check`,
default/all-features `cargo clippy --all-targets -- -D warnings` geçti.

Loglar `benchmarks/borrow-verify-{tests-default,tests-all-features,
final-semantic-test,fmt,clippy,clippy-all-features}.log` altında. Bu metadata/
frame testleri kısa, geçici ve otomatik temizlenen unit fixture'lardır;
benchmark corpus üretimi veya benchmark DB sıfırlaması değildir.

## Açık işler

[OPEN-WORK](OPEN-WORK.md): güncel SQLite write/maintenance karşılaştırması,
38-query regresyonları, RAM/disk ve concurrent bakım yükü açık. Bu çalışma
source taramasını kaldırmaz; daha ucuz doğrular. CRC ve frame read/allocate
sonraki ilk-use darboğazlarıdır. Default opt-in record-read kararı değişmez.
