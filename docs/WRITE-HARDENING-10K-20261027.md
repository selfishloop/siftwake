# Yazma sağlamlaştırması —Blok1 /27 Ekim2026

Son kullanıcı kararı: 1b tekli ölçütü ≥%75 ve SQLite pooled üstü olarak
değiştirildi; aşağıdaki %77,22 yeni kapıya göre GEÇTİ. Bu raporun %80 KALDI
tablosu tarihsel kanıttır. Yeni 30tur/100'lük grup/sequence profil durumları
[güncel kapı sayfasında](WRITE-HARDENING-GATES-10K-20261027.md) tutulur.

**Blok1 KALDI; kullanıcının durma kuralı uygulandı.** Sonraki blok/C ABI/entegrasyon yok.
Küçük WAL ve revizyon reclaim varsayılan **KAPALI** kalır.

| Kapı | Şimdiki kanıt | Karar |
|---|---|---|
| 1a: normal yükte ≥3 uzun koşu, commit≤50ms | Dört dengeli768 koşu;0/3072 aşım;max26,366ms | GEÇTİ, yalnız bu kapsam |
| Ek uzun4R yükünde commit uçları | Üç4096 koşuda1/12288 aşım;max419,587ms | KALDI /uç gecikme tüm yüklerde çözülmedi |
| 1b: tekli ve100'lük pooled≥%80,≥3/4 çift | Tekli%77,85/%76,94/%76,86; gözlenen3 çift pooled%77,22;0 geçen | KALDI;≥3/4 artık mümkün değil |
| 1c: gerçek10k/30tur, sürekli yazı ve2sn idle, disk≤1,3× | Araç/idle API hazır; başarısız1b sonrası ölçülmedi | ÖLÇÜLMEDİ |
| 1d:30tur tombstone ve kill scratch≤1MB | Küçük30tur ve10k kill sonrası scratch0byte geçti; gerçek30tur tombstone kapısı ölçülmedi | KISMİ; TAM KAPI KAPANMADI |
| Oracle/kill/eski okuyucu |232 test,7/7 nihai10k kill/reopen, eski exe ve dört prefix kill sınırı geçti | GEÇTİ, ölçülen kapsam |

Bu belge başarısız blok sonucudur.100'lük throughput ve gerçek30tur koşusu
başlatılmadı; dört tekli çiftin ilk ikisi başarısız olduğunda≥3/4 kapısı
matematiksel olarak imkânsızlaştı. Durdurma kararı işlenirken üçüncü FDB çifti
tamamlandı; SQLite üçüncü yük kolu kontrollü iptal edildi. Bu kısmi kol ölçüm
başarısı veya motor çökmesi sayılmaz; iptal kaydı ve yarım log korundu.
Dördüncü çift veya100'lük grup için pooled sonuç uydurulmadı.
Tüm ham sonuçlar/kötü örnekler
[türetilen tablolarda](benchmarks/write-hardening-10k-20261027-tables.md)
ve `write-hardening-*` JSONL/process/stderr kanıtlarında korunur.

## Teşhis:513. işlem

Normal eski motorun bu oturumdaki dört768 koşusunda sıçrama438–469ms oldu.
Ayrı diagnostic build'de446,804ms ölçüldü; diagnostic sayaç overhead'i nedeniyle
normal build ile doğrudan A/B değildir. Aşağıdaki ayrım aynı diagnostic commit'in
ayrık kapsamlarıdır; iç içe sayaçlar ikinci kez toplanmaz.

| Kapsam | ms | Toplam payı |
|---|---:|---:|
| Light kayıtların indeks hazırlığı |268,484|60,09%|
| Flush kaynak/indeks yazımı ve açılması |113,027|25,30%|
| Flush yeni aktif WAL/manifest |2,901|0,65%|
| Flush içindeki kalan işler (yayım/GC dahil) |12,295|2,75%|
| İlk worker drain |0,0054|0,0012%|
| Flush sonrası worker drain |0,0003|0,0001%|
| Yeni transaction kaynak/indeks/yayımı |38,730|8,67%|
| Bu kapsamların dışındaki commit işi |11,363|2,54%|

İlgili fsync çağrıları ayrı sayaçlarla ölçüldü: source checkpoint10,348ms,
index4,008ms, active WAL1,552ms, manifest1,079ms, staged transaction WAL0,437ms.
Toplam17,425ms bu üst kapsamların **içindedir**, üzerine eklenmez. Worker drain
ana neden çıkmadı; ağırlık foreground indeks üretimi/segment hazırlığıdır.
Kaynak: `write-hardening-profile-10k-20261027-baseline-fdb-streamed-small-boundary.jsonl`.

## Değişiklik

[Tasarım](WRITE-HARDENING-DESIGN-20261027.md):256 anahtar veya8MiB fiziksel
WAL'da tutarlı bir kesit worker'a aktarılır. Belge gövdeleri yerine checked WAL
konumları/anahtarlar/volatile proof Arc'ları paylaşılır. Kesit hazırlanırken tail
hem mevcut hem yeni standart WAL'a sync edilir; manifest yalnız hazır kaynak ve
tam tail'i birlikte yayımlar. Son okuyucu bırakana kadar eski WAL pinlenir.
Ölçülen normal fsync maliyetinde geçici çift-WAL bedeli vardır; gizlenmez.

Opt-in küçük yol128 işlem/8MiB çağrılara kadar proof üretip inverted index'i
erteler. Hard backpressure2048 anahtar/64MiB WAL ve Options delta sınırlarıdır.
Worker yetişmezse fallback hâlâ bekleyebilir; gerçek zaman garantisi verilmez.
Owned/Vec yolun büyük input RAM maliyeti bu değişiklikle çözülmüş sayılmaz.

`maintain_idle()` tek çağrıyla pending flush/merge/reclaim işlerini drain eder,
aynı standart manifest'i yeni ID'de yayımlar ve sahipsiz future dosyaların GC'sini
yeniden dener. Süresi2sn ile sınırlı değildir; planlanan30tur aracı2sn gerçek idle
ve çağrının ek senkron süresini ayrı kaydeder. Başarısız kapı saklanmaz.
Küçük opt-in commit'ler bitmiş worker'ı poll eder; arka plan ilerlemesi sağlanır.

Tombstone kaldırma yalnız daha eski hiçbir immutable source aynı anahtarı
taşımıyorsa mümkündür. WAL'daki güncelleme/silme bu kanıtın yerine geçmez.
Strict open tüm kaynak/WAL doğrulamasından sonra ve exclusive DB lock altında
yalnız tam engine input/build/spill isimlerini temizler; reparse/link/dizin ve
gerçek .fdb/.idx/.ready dosyaları bu allowlist'e girmez. Geçerli bir snapshot
aynı lock ömrünü uzattığı için ikinci bir opener canlı scratch temizleyemez.

Kalıcı codec/magic/manifest/WAL biçimi ve0.1 fixture değişmedi. Yeni DB/arama
motoru bağımlılığı/experimental feature eklenmedi. Elektrik kesintisi iddiası yok.

## 1a: dört dengeli tekrar

Sıralar kontrol→aday→SQLite /SQLite→aday→kontrol /aday→kontrol→SQLite /
SQLite→kontrol→aday. Her kol aynı corpus'un ilk10k'sından temiz slotta başlayıp
bakım sonrası768 adet%50 update/%25 delete/%25 insert tekli streamed commit
uygular. Her işlemin yeni/eski marker görünürlüğü, sonunda bütün payload/version
modeli ve38-case sorgu oracle'ı doğrulanır.

| Tekrar | Eski commit p95 /max ms | Aday p95 /max ms | SQLite p95 /max ms |
|---|---:|---:|---:|
|0|3,720 /468,881|4,081 /23,631|13,047 /77,783|
|1|2,688 /438,320|3,690 /25,822|14,028 /79,150|
|2|2,437 /443,850|2,631 /26,366|13,413 /75,114|
|3|1,973 /456,784|2,514 /25,593|12,800 /77,782|

Eski motorda4/3072, adayda0/3072, SQLite'ta23/3072 commit50ms üstünde.
Adayda görünürlük max26,675ms. Normal p95 bazı çiftlerde yükseldi: kuyruk
uçları düzeldi diye tüm tekli işlemlerin hızlandığı söylenmez. Adayın gözlenen
OS lifetime peak private14,68–15,75MiB; kontrol14,02–14,39MiB. Bunlar harness/model
dahil proses sayaçlarıdır, temiz idle veya motor rezervasyonu değildir. Sayaç
son emit anına kadardır; Drop'taki son worker drain'inin son milisaniyeleri için
ayrı parent sampler yoktur.60MB üst sınır garantisi bu tablodan çıkarılmaz.

## 1b düzeni ve sınırlar

Plan dört çift/işlem grubuydu; tamamlanan her çiftin okuma açık/kapalı kollarında eşit işlem sayısı:
FDB4096 tekli veya128×100; SQLite512 tekli veya64×100. SQLite daha kısa sayıyla
bilgi amaçlı referanstır; FDB ile toplam süre/iş adedi eşitmiş gibi sunulmaz.
Kol sıraları C/L,L/C,L/C,C/L, motor sıraları dengelenir. Başlangıç bakımı iki
kolda da drain edilir.4 okuyucu yazıcı bitene kadar sorgu döngüsünü sürdürür.
FDB minimum2 tur/okuyucu, SQLite1; tam38 sorgu kullanılır. Oran eşit işlem
sayılarında toplam kontrol süresi/toplam yük süresidir; ortalama oran değildir.

Model oracle engine timer'ının dışında çalışır ama aynı CPU'yu kullanır. Okuma
tablosunda yalnız writer_active=true başlayan sorgular ayrı seçilir; her sorgunun
örnek sayısı, mutlak ms, wait ve merge/rewrite coverage'ı ham kanıtta saklanır.
OS cache sıcak; cache tahliyesi, CPU/background/power izolasyonu yapılmadı.
SQLite FTS5trigram/detailnone,FULL WAL,2MiB cache,mmap0,tempFILE ayarındadır.

### Ölçülen sonuç ve engel

| Tekli çift | Kontrol /4R yük ops/s | Oran | Yükte commit max ms |
|---|---:|---:|---:|
|FDB0|340,05 /264,74|77,85%|44,834|
|FDB1|352,78 /271,43|76,94%|29,191|
|FDB2|341,86 /262,74|76,86%|419,587|
|SQLite0|113,58 /77,70|68,41%|137,420|
|SQLite1|114,27 /82,82|72,48%|133,503|

FDB gözlenen3 çift toplam kontrol süresi35,638s /yük46,152s →%77,22.
SQLite gözlenen2 çift%70,38. SQLite'ın altında kayıp görülmesi kendi%80 kapısını
geçmiş sayılmaz. SQLite'da yazı kısa bittiği için bazı sorgular writer_active
örneği üretmedi; ilgili hücreler ÖLÇÜLMEDİ olarak gösterilir.

| FDB çift | Commit çağrıları kontrol /yük ms | Bakım arası kontrol /yük ms | Diğer harness kontrol /yük ms |
|---|---:|---:|---:|
|0|11644,131 /14216,216|283,238 /1111,394|117,842 /144,330|
|1|11206,353 /13852,182|288,977 /1092,827|115,370 /145,530|
|2|11537,501 /14329,477|328,040 /1110,765|116,110 /149,511|

Bu üç kapsam yazıcı wall toplamını ayrık olarak oluşturur. Model/oracle CPU'su
eşzamanlı sorgu tarafında da çalışır; wall sürelerinden CPU yüzdesi çıkarılmaz.
İlk çiftin ek3,427s'sinin0,828s'si bakım aralığında,2,572s'si commit çağrılarında,
0,026s'si diğer harness işindedir. Bakımın yükte yaklaşık1,1s /kontrolde0,3s
olduğu tekrarlandı. Altındaki pacing/CRC/I/O/kilit payları bu normal build'de
ayrı ölçülmedi; bu farkın tamamına pacing denmez.

İlk çiftte snapshot hazırlığı771,810→1073,930ms, eski görünüm bırakma
147,578→556,218ms. Bu sayaçlar commit kapsamının içindedir; yukarıya yeniden
eklenmez. Her iki kolda4116 yayın/61084 mask key taraması vardır; iş miktarı
eşleşir. Yalnız maskeyi düzeltmek bütün kaybı açıklamaz.

Üçüncü yükte915. commit,sequence946:419,587ms. Aynı koşuda max snapshot
hazırlığı408,446ms; toplam WAL reader hazırlığı kontrol151,845→yük587,413ms.
Bu toplu sayaçlarda sequence başına eşleştirme olmadığından **408ms ile o tek
commit'in aynı yayın olduğu kanıtlanmadı**. Yeni WAL için foreground pathname
open/reader hazırlığı güçlü bir sonraki profil adayıdır; kesin tek neden diye
sunulmaz. Yeni handle'ı worker'da hazırlayıp install'de yalnız devralmak ve
bakım aralığı farkını alt aşamalara ayırmak sonraki dar çalışma önerisidir.

Önceki4R sorgu sınırı bu tekrarın pooled'unda38 sorgunun2'sinde aşılır:
case6 `ishmael`,case22 `quixote whale`. Her çiftin ayrı kötü sonuçları ve bütün
38-case mutlak ms/fark/örnek sayıları ham tablolardadır; case34 izlenmeye devam
eder. Okumadaki en uzun gözlenen writer-owned/admission/cache/release beklemesi
7,361ms'dir; tüm search wall süresi diye sunulmaz. Explicit aralıkta her çiftte
bir rewrite ve4 okuyucunun çakışması görüldü; interlude normal_merge sayacı0
oldu. Otomatik merge'ler bu sayaca dahil değildir; bu sayaçtan tam normal merge
coverage'ı iddia edilmez.

Uzun4R koşularında OS lifetime peak private58,23–60,50MiB: harness/model/okuyucu
oracle'ı dahildir.60MB/MiB genel sınırına geçti denmez; normal768 tepesinden
farklı bir yüktür. Arama, sorgu RAM'i ve bakım RAM'i açık kalır.

## Doğrulama ve açık iş

Son v6 üretim kaynağında232 test geçti,14 helper ignored; fmt/clippy, no-default/
all-features ve native SQLite kontrolleri geçti. Kanıt:
`snapshot-10k-20261026-validation-write-hardening-20261027-v6*`.
Yeni prefix için build,
publish öncesi/sonrası ve ack process-kill sınırları hedefli testte geçti; eski
snapshot ve tail adreslerinin bağımsızlığı/son pin sonrası dosya temizliği test
edildi. Küçük30tur tombstone testi gerçek10k/30tur disk sonucunun yerine geçmez.

`write-hardening-kill-v6-10k-20261027-summary.json`: retire-before/after,
rewrite sırasında yarım input, iterator, flush, merge ve cache repair olmak
üzere7/7 yeni10k process-kill/reopen+tam payload/matrix/eski-yeni marker oracle
geçti.24Ekim frozen eski okuyucu yeni motorun verisini okudu. Kesme/tek-byte
bozulma ve0.1 fixture koruması test paketinde tekrar geçti. Corpus/fixture SHA
koruması değişmedi. Cache repair kill/reopen authoritative source hash'lerini
değiştirmedi. Son input/build artığı0byte; gerçek30tur metadata büyümesi
ölçülmediği için1d tamamlandı denmez.

Performans executable'ı v4, nihai correctness/kill executable'ı v6'dır. Son
incelemede strict-open scratch cleanup recovery flush/worker başlangıcından
önceye taşındı: dolu delta açılışta flush edilip yeni worker başlatırsa temizleyici
onun canlı build dosyalarıyla karşılaşmamalı. Regresyon testi eklendi; commit/
query yolunda bu arada yeni optimizasyon yapılmadı. v6 için yeni performans
sonucu varmış gibi sunulmadı.

Ek test helper'ı iki100-entry retirement fixture'ını mevcut segment yoluyla
kurup WAL opt-in'ini hedef kill öncesi geri açar. Aksi halde yeni128-entry yolu
aynı aktif anahtarları günceller, sahte biçimde immutable dead-source beklenir.
Otomatik reclaim test kurulurken kapalı, hedef işler explicit planlanmıştır.
Tarihsel `cancel_iterator` etiketi fast WAL yolunun worker'ı iptal ettiğini
kanıtlamaz; yarım input ve prior rewrite ile kill'i kanıtlar. Gerçek cancellation
ve pin etkileşimi ilgili unit testlerinde tekrar geçti. Güç kesintisi iddiası yok.

Nihai kill sonrası tek `maintain_idle()` çağrısı ayrıca aynı10k modelinde
kontrol edildi: eski snapshot yaşarken önceki bütün source path'leri yerinde
kaldı ve10k eski snapshot payload/version oracle'ı geçti. Son okuyucu bırakınca
referanssız `.fdb` artığı **0byte**, input/build artığı **0byte** oldu. Sonraki
38-case/full-payload oracle ve eski frozen okuyucu tekrar geçti.
Retained533,714MB→pin yaşarken590,195MB→pin bırakınca510,334MB; bu geçmiş
churn/kill setup'larıyla kirlenmiş doğruluk slotudur,30tur/1,3× disk benchmark'ı
değildir. Genel pin ömrü/geçici disk kapısı kapatılmadı.

case34, geniş zx/the zx, parçalı yerleşim1112→2013 read, sorgu RAM'i, uzun
snapshot pinleri/geçici disk tepesi ve gerçek-soğuk yük OPEN-WORK'te açık kalır.
Bu blok tamamlanmadan sonraki blok veya C ABI'ye geçilmez.
