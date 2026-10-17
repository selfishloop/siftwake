# Aynı350MB veride arama maliyetlerinin ayrılması — 2026-10-17

Sonuç: uygun kısa ASCII sorgularında gereksiz body fold kaldırıldı.
Normal strict blok karşılaştırmasında zx p95 **277,138→190,633ms (-%31,21)**,
the-zx **266,181→175,714ms (-%33,99)**; ikisi4/4 süreç çiftinde ve32/32
blokta hızlı. Tipik blok kazancı%30–31. Bütün38 senaryo tekrar ölçüldü:
28 düşük/10 yüksek p95. Genel p95 kapısı **kapanmadı**: aynı programla
uzun matriste38/38, kısa bloklarda36/38 artış görülebiliyor. Bu A/A
farkının kod değişikliğinden gelmesi mümkün değildir; A/B farklarının
tamamını da ortam sayıp kapatmıyoruz. DB aynı349.805.698 B; yeni cache yok.

## Kapsam ve koşullar

Aynı mevcut corpus'un ilk10.000 belgesi:264.842.858 B gövde,2.057–51.197 B
pasajlar,8 kitap. Corpus hazırlanmadı/indirilmedi. Aynı temizlenmiş
`data/benchmark-work/fdb`,349.805.698 B/18 dosya (boş LOCK dahil),5 Run,
delta0/sequence20 kullanıldı. DB reset, repack veya DB kopyası yapılmadı.
SQLite slotu yalnız değişmezlik kontrolünde; bu çalışmada yeni SQLite hız
karşılaştırması yok.

Windows11 Pro26200, Ryzen7 7840HS, Rust1.99/MSVC; release thin-LTO ve
codegen-units1. Normal derlemelerde diagnostic kapalı, profil ayrı derleme.
CPU affinity4, Balanced plan; frekans/SMT/arka plan/sıcaklık izolasyonu yok.
Sıcak OS cache; eviction/gerçek soğuk-cache testi yapılmadı.
[Ortam kaydı](benchmarks/search-stages-environment-20261017.json).

Sorgular:30 kelime/AND/iki-karakter/Unicode-fold/sonuçsuz arama ve8 sayfa;
256 get anahtarı ve bulunamayan get'ler. Tam toplam, sayfa sırası, tam belge
alanları/links ve sürümler bağımsız corpus oracle'ıyla karşılaştırılır.
Normal matrix'te her primary sorgu20, her sayfa5 örnek/proses; dört çiftte
80/20 pooled örnek. Yoğun sayfa testinde800 örnek/sayfa/variant vardır.
Oracle yükleme ve tam-belge karşılaştırmaları zamanlama dışındadır; heap/
cache durumunu etkileyebilir. Bu yüzden proses belleği oracle'sız ayrı
24 search/771 get probunda ölçülür.

Strict open, bütün kaynak/frame/index CRC, schema/UTF-8/key/version ve
directory/page kontrolleri sürer; sorgu zamanlamasından önce pending0.
Okuma sayaçları OS API çağrıları/okunan byte'lardır; fiziksel disk I/O sayısı
veya soğuk disk bant genişliği değildir.

## 1. Eski/yeni okuyucu aynı byte'larda

Önceki SEG05 prototip okuyucusu (`compact-cache-window-reads`) ve GC sonrası
güncel okuyucu (`reclaim-reads`) aynı18 dosyada dört dengeli çift çalıştı.
Yeni okuyucuda38-case20 düşük/18 yüksek p95 çıktı. Önceki yazıcı+okuyucu
karşılaştırmasındaki22 artış birebir tekrarlanmadı. Bu yeni koşu kaynak
yerleşimi/yazıcı farkını kaldırır; bütün zaman farklarını koda bağlamaz.

| Sorgu | Eski p95 ms | Güncel p95 ms | Fark | Güncel düşük çift |
|---|---:|---:|---:|---:|
| zx |228,04|232,54|+%1,97|2/4|
| whale harpoon |66,84|72,41|+%8,33|2/4|
| darcy |56,73|62,99|+%11,03|2/4|
| watson |51,30|54,65|+%6,55|3/4|
| presumption |109,00|85,81|-%21,27|3/4|
| harpoon |72,32|71,69|-%0,86|1/4|

Okuma yollarının eski/güncel kaynak karşılaştırmasında text doğrulayıcısı,
normalizer ve payload parser aynıydı. Buna rağmen program bütünü/derlenen
kod ve oturum etkileri aynı kabul edilmedi. Özellikle pooled p95'in tek
bir yavaş süreçten etkilenebileceği çift sonuçlarında görünür.
[38 sorgu/get ve dosya kanıtı](benchmarks/search-same-db-baseline-10k-20261017-summary.json).

## 2. Okuma, doğrulama, normalleştirme/tarama, sonuç hazırlama

`--profile-matrix` bütün38 senaryoyu ayrı diagnostic derlemede çalıştırır.
Normal p95 ile bu instrumented medyanlar birbirinin yerine kullanılmaz.
Span'ler iç içedir: `verify_text`, fold/scan'i kapsar; `disk.read_at` da
payload/posting/metadata okumaları içinde bulunabilir. Satırlar toplanarak
toplam süre/CPU payı çıkarılmaz. Çok sayıdaki timer/mutex çağrısının maliyeti
de profilin içindedir.

Aşağıdaki paired iki çift/variant başına6 profil örneği, kısa-yol fonksiyonu
ayrılmadan önceki ikinci adayındır. Ayrı aşamalar görülebilir:

| zx, instrumented medyan | Kontrol ms | Kısa ASCII adayı ms |
|---|---:|---:|
| Search toplamı |206,407|119,032|
| Payload okuma |39,432|38,882|
| Payload CRC |20,778|20,914|
| Payload yapı/UTF-8 doğrulama |20,895|20,401|
| Metin eşleşme kontrolü (fold/scan dahil) |118,046|31,409|
| Bunun içindeki body fold |69,260|0|
| Bunun içindeki folded body scan |15,737|0|
| Doğrudan kısa ASCII scan |0|29,369|
| Sayfa birleştirme/hydration |0,002|0,002|

`quixote` gibi dar pozitif sorguda body fold zaten0; kontrol toplamı1,914ms,
payload okuma0,298ms, CRC0,146ms, yapı kontrolü0,099ms ve sonuç hazırlama
0,924ms. `holmes`'ta okuma9,259ms, CRC2,699ms, yapı3,104ms, body fold0,554ms,
toplam22,333ms. Aynı düzeltmenin bütün sorguları eşit hızlandırması beklenmez.

İki kısa negatif dışında36 senaryonun bütün counter adları ve min/max çağrı
sayıları iki variantta aynıdır. Kısa negatiflerde yalnız şu sayaçlar değişir:

- Body fold ve folded scan263.627→0 çağrı/sorgu.
- Yeni raw-short scan0→10.000 çağrı/sorgu.
- Payload/CRC/schema/identity, posting/visibility ve sonuç sayıları korunur.

Bu, diğer36 senaryodaki süre farklarının hepsini açıklamaz; ek aday/okuma/
doğrulama işi üretildiği hipotezini bu profil kapsamı için eler.
[Tam38 profil, aşamalar/çağrı sayıları](benchmarks/search-short-final-profile-10k-20261017-summary.json),
[önceki dört proseslik profil](benchmarks/search-stages-baseline-profile-10k-20261017-summary.json).

## 3. Denemeler ve kabul sınırı

4KiB chunk deneyi geri alındı: dört çiftte zx p95 -%2,23 fakat the-zx
+%1,29; hedefte tutarlı kazanç yok. Normal generic chunk boyu1KiB kaldı.
[Reddedilen deneyin tüm38 sonucu](benchmarks/search-chunk4k-10k-20261017-summary.json).

Yeni algoritma yalnız1–2 küçük ASCII harfli bütün verification terimleri
içindir. Non-ASCII fold'dan çıkabilen `c/g/i/k/o/s/u` harflerini içerirse
normal canonical verifier'a döner. Uygun terimin bütün2^n ASCII yazımları
özgün UTF-8 metinde aranır; heap/body kopyası veya belge önbelleği eklenmez.
Kural belirli benchmark kelimelerini tanımaz. Tüm Unicode scalar'ların
ASCII katkıları ve fold sırasında hiçbir scalar'ın silinmediği test edilir.
Bu iki özellik, doğrudan aramada yokluk sonucunu da kesin yapar. Türkçe/
Unicode ve desteklenmeyen terimlerde önceki yol korunur.

İlk adayda hedef zx/the-zx -%27,85/-%29,43 (ikisi4/4 düşük), genel38-case
24 düşük/14 yüksek. Sadeleştirilmiş ikinci adayın uzun matrisi8 düşük/30
yüksek; hedefte -%20,25/-%19,40 (4/4). Ayrı stress serisinde hedefte
-%35,23/-%29,20 (2/2); yoğun8 sayfada7 düşük/1 yüksek p95.

Ancak aynı ikinci executable aynı CPU'da iki etiketle çalışınca **38/38
pooled p95 yüksek** oldu; zx bile+%17,36, bazı sayfalar+%34–38. İki tarafta
SHA aynı olduğundan bu A/A farkı kod değişikliği olamaz. Önceki A/B
farklarını buradan sayısal olarak çıkarıp "düzeltilmiş" hız hesaplanmaz;
koddaki payın tamamı da sıfır ilan edilmez. Uzun oturum genel kabul kanıtı
olarak yeterli değildir. Son sürümde kısa-yol tarayıcısı genel verifier'ın
içine yayılmayan ayrı fonksiyondadır; kısa strict bloklar kalibre ediliyor.

[İlk aday](benchmarks/search-short-ascii-10k-20261017-summary.json),
[ikinci aday, geniş](benchmarks/search-short-final-matrix-10k-20261017-summary.json),
[ayrı stress](benchmarks/search-short-final-stress-10k-20261017-summary.json),
[yoğun sayfa](benchmarks/search-short-final-dense-10k-20261017-summary.json),
[aynı executable negatif kontrolü](benchmarks/search-short-aa-10k-20261017-summary.json).

## Son normal aday: kısa strict bloklar

Genel verifier'a yayılmaması için kısa tarayıcı `inline(never)` ile ayrı
fonksiyonda tutulur. Bu özniteliğin tek başına katkısı ayrıştırılmadı;
aşağıdaki sonuç son değişikliğin bütünü içindir. Eski kontrol `bf879b6`
kütüphanesi, aday güncel kütüphane; ikisi aynı strict-open worker/harness,
aynı Rust/features/release ayarlarıyla derlendi. Build aracı yalnız kendi
segment kaynak değişikliğini geçici olarak değiştirip exact byte'larla
geri yükler; DB/corpus'a dokunmaz. Kalıcı format/bağımlılık değişmez.

Dört süreç çifti ×8 blok ×5 tur. Her blokta yalnız bir süreç DB kilidini
tutar; strict source/index doğrulaması ve bütün query/get ısınması zamanlama
dışındadır. İki oracle worker resident kalır; eşzamanlı DB sorgusu yoktur.
Her primary160, her sayfa64 pooled örnek/variant. Bu düzen uzun tek-proses
matrix ile aynı cache/heap/yaşam süresi düzeni değildir; ayrı sonuçlardır.

| Sorgu | Kontrol p50/p95 ms | Son aday p50/p95 ms | p95 fark | Tipik blok fark | Hızlı süreç/blok |
|---|---:|---:|---:|---:|---:|
| zx |210,144/277,138|143,544/190,633|-%31,21|-%31,38|4/4;32/32|
| the zx |209,168/266,181|145,423/175,714|-%33,99|-%30,14|4/4;32/32|

Diğer36 senaryonun tipik blok farkları yaklaşık-%4,71…+%2,38. Toplam28
düşük/10 yüksek p95; örneğin he+%3,83, transubstantiation+%6,40,
the/5000/50+%4,71. Artışlar kapanmadı. Yeni global SQLite/throughput veya
bütün sorgularda hız garantisi çıkarılmadı.

Negatif kontrolde aynı aday executable:2 düşük/36 yüksek p95; tipik blok
farkları-%2,41…+%2,74. Etiketlerin dördünde de yavaş görünen aynı-kod
senaryoları da var. Pooled p95 ile tipik blok medyanları farklı dağılımları
ölçer; A/A oranı A/B'den çıkarılmadı ve32 blok bağımsız32 proses sayılmadı.
Kısa hedefte32/32 aynı yön ve farklı önceki serilerdeki tekrar güçlü dar
kanıttır; genel tail-latency problemi çözülmüş değildir.

[Bütün38 sorgu tablosu](benchmarks/search-blocks-ab-10k-20261017-table.md),
[A/B ham örnek/özet/tasarım](benchmarks/search-blocks-ab-10k-20261017-summary.json),
[aynı SHA A/A kontrolü](benchmarks/search-blocks-aa-10k-20261017-summary.json).

Son adayın ayrı diagnostic profilinde de iki kısa sorgu dışında36
senaryonun bütün izlenen min/max iş sayıları aynı. zx toplam349,763→180,345ms,
payload okuma83,816→60,183ms, CRC32,379→30,368ms, yapı/UTF-8
31,964→30,000ms, verify-text186,017→47,798ms. Body fold109,726→0,
folded scan23,780→0; raw-short scan44,592ms. Profilde de ortam/oturum
etkisi vardır: okuma kodu ve iş sayıları değişmediği halde okuma süreleri
farklıdır. Bu nedenle okuma veya açılış için yeni algoritmik kazanç iddiası
yok; iç içe timer'lar toplanmaz ve profil farkı normal hız sonucu değildir.
[Son38-case profil](benchmarks/search-isolated-profile-10k-20261017-summary.json).

## Bellek ve açılış: önceki ikinci adayın kontrolü

Dört çift, oracle'sız24 search/771 get. OS lifetime peak private commit
9.216.000→9.199.616 B (-%0,178;3/4 düşük), working set13.809.664→13.826.048 B
(+%0,119;2/4 düşük). Yaklaşık16KiB farklar; sıfır RAM artışı veya genel RAM
kapısı iddiası yok. Sayaçlar bütün proses ömrüne aittir; faza özel/heap-only/
kernel-file-cache dahil toplam sistem RAM'i değildir.

Strict open medyan155,575→157,678ms (+%1,35;2/4 düşük); ilk kullanım
159,193→161,522ms (+%1,46). Yeni açılış iyileştirmesi gösterilmedi.
[Bellek](benchmarks/search-short-final-memory-10k-20261017-summary.json),
[strict open/ilk kullanım](benchmarks/search-short-final-open-10k-20261017-summary.json).

Son ayrı-fonksiyon adayında dört yeni çift: peak private commit
9.222.144→9.187.328 B (-%0,38;4/4 düşük), peak WS13.830.144→13.778.944 B
(-%0,37;3/4 düşük, bir çift+%0,12). Bu workload'un tepe medyanları artmadı;
genel RAM/sıfır-artış garantisi değildir. Normal Vec/input/worker/kernel
belleği bu oracle'sız query probuyla ölçülmez. [Son bellek](benchmarks/search-isolated-memory-10k-20261017-summary.json).

Aynı son seride strict open225,153→209,821ms (-%6,81;4/4), ilk kullanım
230,448→215,963ms (-%6,29;4/4). Önceki seride yön farklıydı; açılış kodu
değişmedi ve A/A oynaklığı biliniyor. Bu gözlem genel açılış işini kapatmaz.
[Son açılış kontrolü](benchmarks/search-isolated-open-10k-20261017-summary.json).

## Doğruluk ve açık işler

Default/no-default/all-features `cargo test --all-targets`:179/107/198
başarılı,7/3/9 ignore (subprocess helper'ları dahil). Bunlar bütün target
çalıştırmalarının toplamıdır; ortak example testleri tekrar sayılabilir.
Son fonksiyon yerleşimi değişikliğinden sonra short3, scratch4, window7
testleri ayrıca geçti. Fmt ve default/all/no-default Clippy kontrolleri geçti;
son yerleşimde default/all Clippy ayrıca tekrarlandı.

Yeni testler bütün non-ASCII scalar'lar, bütün destekli kısa ASCII case
yazımları, Unicode komşuları,1/4/8KiB sınırları, doğrudan/prefetched payload
bozulması ve kesilmede kısa pozitif/negatif sorguların hatayı saklamamasını
kontrol eder. Mevcut process kill/kurtarma ve0.1 uyumluluk testleri de tam
suite kapsamındadır. Kalıcı kaynak, WAL, manifest, payload ve cache biçimi,
bağımlılıklar/feature listesi ve0.1 fixture değiştirilmedi.

Son dosya kanıtında16 tam önce/sonra snapshot'ı aynı:18 DB dosyası
349.805.698 B, SQLite ve iki0.1 fixture dosyası başlangıçla hash-eşit.
Corpus'un bütün2.740.862.279 B dosyasının SHA256'sı da öncekiyle aynı;
yalnız ilk10k kullanıldı. Normal varsayılan CLI ile stats, iki kısa
negatif ve quixote pozitif sorgusu geçti. Yeniden açılan normal reader'ın
makine kodu ve debug dışı verisi ölçülen executable ile birebir aynı;
tam dosyada yalnız9 PDB/debug kimlik byte'ı farklı, ayrıca doğrulandı.
[Dosya/corpus/kod/iş kanıtı](benchmarks/search-stages-final-proof-20261017.json),
[normal CLI kontrolü](benchmarks/search-cli-proof-20261017.json).

Araçlar: `measure-search-stages.py` mevcut slotta matrix/dense/stress/
profil/bellek/açılış çalıştırır; `measure-interleaved-reads.py --full-matrix
--blocks 8 --repeats 5 --pairs 4` strict worker'ları sırayla çalıştırır.
`build-search-workers.py` eski/güncel worker'ı kaynak byte'larını koruyarak
dondurur; `prove-search-stages.py` zamanlama bittikten sonra ham raporları,
dosya/corpus hash'lerini ve izlenen iş sayılarını tekrar denetler. Hiçbiri
corpus hazırlayıcı veya DB reset/repack aracı çağırmaz. Normal engine open
kurtarması korunur; sağlıklı ölçümlerde bütün cache byte'ları da aynı kaldı.

Genel arama artışları, yazma/bakım ve kaynak kabul/CRC/spool tekrarları,
normal Vec/query/caller RAM'i, genel açılış ve disk sıkıştırma,
eşzamanlı gecikme/throughput ve gerçek soğuk yük açık kalır.
[İş listesi](OPEN-WORK.md).
