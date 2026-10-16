# REF01: gerçek kaynak kaydından kontrollü get

## Dar kapsam ve API

`prepare_record_reads()` önce tam integrity kontrolü yapar, ardından aktif
native immutable kaynaklar için küçük adres/block checksum cache'leri üretir.
`open_record_reads()` opt-in olarak bu cache'leri kullanır. Normal `open` ve
`open_deferred` aynı davranışı korur. Yeni feature, WAL/source/SEG04 biçimi,
CLI/protokol veya dış proje entegrasyonu yok;0.1 fixture değiştirilmez.

Bu adım **pozitif get** için segment taramasını kaldırır. Döndürülen belge
indeks payload kopyasından değil, `.fdb` içindeki actual OFF1 kaydından gelir.
Adres tablosunun index/source binding'i, dokunulan source256 KiB block CRC'si,
belge CRC'si, OFF1 schema/UTF-8, anahtar ve sürüm doğrulanır.

Negatif get, tombstone get, search ve write/flush/merge mevcut tam source
kontrolünü korur. Bu cache kaynak içerik indeksinin doğruluğunu tek başına
kanıtlamaz; empty posting sonucu üzerinden unread source corruption'un
gizlenmesine izin vermemek için search guard'ı kaldırılmaz. Yeni kod bütün
arama yolunu source offset ile yürütmez. Per-record search / index derivation
kanıtı sonraki tasarım işidir; burada böyle bir kazanç iddia edilmez.

Başarılı positive get yalnız kullanılan kaynak blokları/kaydı doğrular;
unread başka bir source block'u bozuk olabilir. `pending_source_checks()`
bu kaynak için0 olmaz; full integrity tamamlandı anlamına gelmez. Negative
get/search/write o kaynakla ilgili tam kontrolü çalıştırınca bozulmayı reddeder.
Active WAL recovery ve resident/shadow görünürlüğü aynı kalır. Tam kontrol
tamamlandıktan sonraki dış file mutasyonu desteklenmez; explicit integrity
bütün kaynakları tekrar kontrol eder. Bu CRC cache'leri saldırgana karşı
kriptografik authentication değildir; önceki IEEE CRC güvencesi korunur.

## Yeni türetilmiş cache sürümü ve binding

| REF01 `.rmp` byte | İçerik |
|---|---|
| 0..8 | FDBREF01 magic |
| 8..16 | Source end u64 |
| 16..20 | Manifestteki source CRC u32 |
| 20..28 | Source sequence u64 |
| 28..32 | SEG04 footer prefix CRC u32 |
| 32..36 | Index slot/source key sayısı u32 |
| 36..son4 | Slot başına16 B: source payload offset u64, len u32, CRC u32 |
| son4 | Bütün önceki table byte'larının IEEE CRC'si |

Map loader64 MiB sınır, exact size, source end/CRC/seq/index root/key count ve
table CRC'yi kontrol eder. Her address source bounds içindedir; live slotun
len/CRC'si index metadata'sıyla eşit olmalıdır. Okurken actual source key ve
version index lookup key/version'a eşit olmak zorundadır. Map stale/kesik veya
bozuksa aynı `open_record_reads` tam source kontrolüne fallback yapar.

`.svfy` kaynak block tablosu, mevcut FDBVFY01 codec'ini kullanır; end source'un
tam byte uzunluğu, root manifestteki source checksum'dur. Block CRC'lerinin
birleşimi bu root'a eşit olmalıdır. `.vfy` index tablosu ayrı dosyada kalır;
var olan index cache'i overwrite edilmez. `.svfy` end'i index footer prefix'i
değil bütün source'dur. Kaynak ve index cache tipleri aynı filename'i paylaşmaz.

Hazırlama gerçek TXN2 frame/header/footer/payload/checkpoint/seq kontrollerini
yapar; actual source entry'lerinin key/version/deletion görünürlüğünü index
slotlarıyla eşler. Duplicate/missing key ve whole-source CRC mismatch reddedilir.
Tables create-new temporary + write/sync + rename ile yayımlanır. Source/index
immutable ve yayın descriptor'ları aynı kalır. Interrupted cache preparation
old map'i/source'u değiştirmez; cache yok/bozuksa full fallback mümkündür.
Eski okuyucular source/SEG04'ü bu cache'lerden bağımsız okuyabilir.

RAM yapısı: slot başına address Vec, block CRC/checked-state ve file handle'ları
eklenir; bu motor/process RAM kazancı değildir. Bütün belge içerikleri cache'e
kopyalanmaz. RAM/working set bu görevde ölçülmez.

## Testler

Native source-record testi actual source'daki unread farklı block corruption'u
başarılı unrelated get'in global integrity sonucu sayılmadığını gösterir;
bozuk record get'i, negatif get, boş search ve write reddedilir. REF/SVFY her
cut ve her single-byte corruption'da fallback kontrol edilir. CRC-valid ama
yanlış offset'te cached index document döndürülmez. Source/index hash byte'ları
aynı; version2 update, version1 eski kayıt ve tombstone görünürlüğü korunur.
Publication öncesi process kill eski map ve kaynak byte'larını korur.

Varsayılan/all-features all-targets, cut/byte/process kill ve0.1 fixture
testleri çalıştırılır; ek son native default ve all-features kontrolleri,
`cargo fmt --check` ve iki modda `cargo clippy --all-targets -- -D warnings`
geçmeden kabul sonucu yazılmaz.

## Ölçüm yöntemi

Aynı mevcut10k corpus/DB:5 segment,seq20,boş delta, ilk10.000 source kayıt;
264.842.858 B text,2.057–51.197 B/belge,8 Gutenberg eserinden benzersiz
kısmen örtüşen pasajlar. Yeni corpus/DB yok; sabit `data/benchmark-work/fdb`.
Yalnız active5 source için RMP/SVFY cache'i hazırlanır; eski nesillerin cache'i
ve DB kopyası oluşturulmaz. SQLite tekrar ölçülmez;~9 ms SQLite bağlantısıyla
aynı açılış/güvence hedefinin tamamlandığı iddia edilmez.

Baseline frozen VFY block-deferred exe; aday record-read exe. İkisi release
thin-LTO, experimental-merge-postings + experimental-delta-bitmaps, diagnostic
kapalı. Engine başına untimed open;5 first-use çiftinde sırayı ters çevirerek
open, get9999/version20, ilk quixote(total56,page50), repeat quixote ölçülür.
Source validation aynı iş aşamasında tamamlanmaz: aday get sonrası pending5,
kontrol get sonrası pending4; search her ikisinde pending0 yapar. Bu fark
ayrı olarak raporlanır; sadece get latency'den bütün workload kazancı çıkarılmaz.

Üç warm read çifti: süreç başına3.000 get ve4 query ×30 örnek, query/key
untimed warmup; full total/sıralı50 StoredDocument/version bağımsız corpus
oracle'ına eşit olmalı. Sıcak OS cache, eviction yok. Corpus oracle/hash/log
süre dışı; payload read/CRC/decode ve gerekli doğrulama timer içinde.
RAM, cold-cache, concurrent workload, write hızları, yeni38-query matrisi ve
50k/100k bu turda ölçülmez. Hazırlama ve explicit full integrity ayrıca
kaydedilir. Eski executable compatibility ve source/index/ready/VFY SHA256
önce/sonra kontrol edilir. Yeni cache byte maliyeti ayrıca gösterilir.

Runner `tools/measure-record-reads.ps1`; rapor varsa ezmez, corpus/reset üretmez.
Prefix `docs/benchmarks/record-reads-10k-20261013`. Sonuç aşağıdadır.

## Açık işler

[OPEN-WORK](OPEN-WORK.md): bütün temel testleri güncel kodla birleştirme,
SQLite write/maintenance,38-query regresyonları, RAM/disk ve bakım sırasında
okuma/yazma açık. Kaynak get adresleri bu prototipte ilerler; source search
kontrolü ve~60 ms directory başlangıcı hâlâ tasarım/performans işi olarak kalır.


## Ölçülmüş sonuç ve karar

Beş dönüşümlü fresh process çiftinin medyanları:

| Ölçüm | Önceki block-deferred | Kaynaktan record-get |
|---|---:|---:|
| Open ms | 76,4009 | 74,2519 |
| İlk get9999 API ms | **106,6348** | **0,1977** |
| Open+ilk get ms | **183,3302** | **74,4921** |
| İlk quixote search API ms | 456,3013 | 554,4203 |
| Open+get+ilk search ms | 643,9794 | 630,4028 |
| Get sonrası pending source | 4 | 5 |

İlk positive get her beş çiftte hızlı; medyanda~%99,81 API süre azalması.
Open+get medyanı~%59,37 azaldı. Open medyan farkı~%2,8; küçük farkın bütününü
bu değişikliğe bağlamıyoruz. Önceki rapordaki60 ms başka oturumun koşuludur;
bu oturumdaki76/74 ms aynı çiftlerin karşılaştırmasıdır. SQLite9 ms hedefi
veya bütün açılış işinin tamamlandığı iddiası yok.

İlk search API'si~%21,50 arttı: önceki get bir kaynağı tam doğrularken yeni
get yalnız actual document block'larını doğrular. Böylece search yeni yolda
beş tam kaynak yerine önceki yoldaki dört kaynak kontrolüyle kıyaslanır;
bu fark saklanmadı. Toplam open+get+search medyan farkı~%2,11 düşüş;
aynı gözlemlerde bu küçük farktan genel workload hız kazancı iddia edilmez.

### Sıcak okuma ve arama

Üç fresh süreç/motor; toplam18.000 get ve720 timed search'ın tam source
oracle karşılaştırmaları geçti. Motor başına9.000 get; her query90 örnek,
nearest-rank pooled percentile. Query p99 bu örnek sayısında en yüksek örnektir;
production tail kanıtı değildir.

| İşlem | Kontrol p95 ms | Record-get p95 ms | Kontrol p99 ms | Record-get p99 ms |
|---|---:|---:|---:|---:|
| get | 0.0858 | 0.0845 | 0.1334 | 0.1243 |
| the | 396.8509 | 401.9225 | 438.5827 | 431.1019 |
| quixote | 8.9068 | 9.1592 | 10.0879 | 10.3582 |
| he | 390.4302 | 407.7491 | 399.6150 | 416.1144 |
| the man | 405.7021 | 414.5430 | 431.7483 | 419.0702 |

Warm get p95~%1,5 düşük; anlamlı genel kazanç ilan etmiyoruz. Dört warm search
pooled p95 artışı: the~%1,28, quixote~%2,83, he~%4,44, AND~%2,18.
Quixote per-process p95 control8,8957 /9,1627 /8,9749, aday9,4194 /9,3895 /
8,4916; iki çiftte artış. He390,4302 /394,6960 /384,1207 →410,9257 /
406,2400 /381,5978; iki çiftte artış. **Genel sıfır-regresyon kapısı geçmedi.**
Farkların sebebi ayrıca profile edilmeden tamamı source-get'e bağlanmaz;
38-query regresyon işi bu dört query'yle kapatılmaz.

### Hazırlama / disk / testler

Explicit record-cache preparation1.713,07 ms (tek örnek, öncesindeki strict
open hariç): full integrity + source frame/entry mapping + actual source block
CRC table + cache write/sync. Explicit full integrity731,30 ms (tek fresh
record-read process, pending0). Bunlar yeni write/maintenance hız ölçümü değil.

Active5 kaynak için10 yeni küçük dosya toplam **164.480 B** (~160,6 KiB).
Mevcut index VFY5.580 B ayrıca önceden vardır. Bu metadata address/block CRC
 tutar; belge/payload kopyası yok. Source/index/ready/mevcut VFY byte uzunluk ve
SHA256 değerleri önce/sonra aynı; eski strict frozen executable yeni cache'ler
varken DB'yi açtı. Compatibility kontrolünde~948,99 ms açılış kaydedildi;
bu ayrı strict performans karşılaştırması değildir.0.1 fixture bytes korunur.

Default/all-features all-targets testleri geçti. Son ek native version/tombstone
ve wrong-address kontrolleri default'ta2/2 tekrar, all-features'ta9/9 suite içinde
geçti. Address-cache publication process kill ve mevcut VFY kill testleri geçti.
`cargo fmt --check`, default/all-features `cargo clippy --all-targets -- -D warnings`
geçti. RAM/working set bu turda ölçülmedi; address/block/file-handle metadata
bellek maliyeti vardır, RAM azalması iddia edilmez.

Karar: source record-get API opt-in prototip olarak korunur. Pozitif ilk get'te
hız kazancı ölçülmüş, genel warm-query regresyon kapısı geçmemiştir; normal
strict ve mevcut deferred açılış default davranışları değiştirilmedi. Sonraki
adım source-address cache'i varsayılanlaştırmak değildir; sıcak arama farklarını
profillemek ve38-query matrisi + güncel write/maintenance ölçümünü birlikte
kapsamaktır. Kaynak search doğrulaması/index köken kanıtı ve directory açılış
maliyeti hâlâ açık. [Açık işler](OPEN-WORK.md) unutulmadı.
