# Okunabilir snapshot / eşzamanlı yazma — 10k, 26 Ekim 2026

26 Ekim son kullanıcı kararı: Adım2 **kabul edildi**, o adımın kapısı kapatıldı.
Ölçülen throughput ve iki arama artışı açık izleme maddesi olarak korunur.
Adım3 artımlı maske/küçük commit/dirty açılış çalışması başladı; aşağıdaki
KALDI sonuçları önceki ölçümün tarihsel kanıtıdır. Reclaim varsayılan kapalı.


**Adım 2: KALDI. Adım 3 başlatılmadı.** Snapshot prototipi çalışıyor;
bu, bütün kabul kapılarının geçtiği anlamına gelmiyor. Revizyon geri kazanımı
ve önceki reddedilen query pruning deneyi varsayılan **kapalı** kaldı.
Kalıcı format, kaynak/WAL/manifest/payload codec'i ve SQLite ayarları değişmedi.

## Kapılar

| Kapı | Sonuç | Karar |
|---|---|---|
| 4 okuyucu + yazıcı p95 / aynı 4 okuyucu, yazıcı yok | Dengeli v2 koşularında38/38; v3 ek tekli38/38,100'lü36/38 | Tekrarda iki sorgu KALDI |
| Aynı yük / tek okuyuculu boşta taban | Tekli9/38,100'lü10/38 sınır dışında | Bu tabanda KALDI |
| Writer throughput ≥%80 | İki çift birlikte tekli%75,96;100'lü%78,68. Her grupta yalnız1/2 çift ≥%80 | KALDI |
| Tam normal merge + tamamlanan revision rewrite, okuyucular aktifken | Her FDB yük koşusunda doğrulandı; ayrı iptal testi de var | GEÇTİ, gözlenen |
| Her okuma tek commit modeline ait; güncelleme/silme/ekleme atomik | Bütün ölçüm oracle'ları ve kontrollü4-reader testleri | GEÇTİ, gözlenen |
| Snapshot pinleri; son okuyucuyla dosya temizliği | Segment + aktif WAL, writer kapandıktan sonra okuma, rewrite iptali, eşzamanlı release | GEÇTİ, gözlenen |
| Process-kill / eski okuyucu | v2 ve güncel v3:7/7 pencere, her birinde reopen/oracle;24Ekim okuyucusu | GEÇTİ, gözlenen |

≤100ms mutlak kapı uygulanmadı. Önceki adımın ±%5 pooled p95 kapısını
kullanıcı yalnız o adım için kapattı; burada %80 ve yeni okuma kapıları
değiştirilmedi. “Boşta” için iki taban da gösterildi; daha yavaş4R tabanı
seçilerek tek okuyucuya göre kalan artışlar gizlenmedi. Toplam karar iki
yorumda da **KALDI**.

## Tasarım ve API

[Ön tasarım](SNAPSHOT-DESIGN-20261026.md) prototipten önce yazıldı.
`Database::snapshot_reader()` isteğe bağlı kanalı açar; `SnapshotReader`
okuyucu iş parçacıklarına klonlanır. `acquire()` hazır Arc görünümünü kısa
RwLock altında alır. Commit/WAL fsync, mask hazırlama, merge/rewrite ve sorgu
bu pointer kilidini tutmaz. `Snapshot::sequence/get/search/search_into`
sabit bir görünüm kullanır. Başarılı commit/flush/install sınırlarında yeni
görünüm hazırlanıp yayımlanır; hazırlanmamış/yarım belge yayımlanmaz.

Run/indeks/kaynak handle'ları paylaşılır; kaynak ve eski WAL pinlenir.
Son okuyucu lease'leri bıraktıktan sonra güncel doğrulanmış manifest ve pin
listesi üzerinden emekli dosyalar temizlenir. Root publish ve GC ayrı mutex
ile sıralanır. Read admission bu mutex'e girmez. Son-reader GC beklemesi ve
I/O'su sorgu latency hesabına **dahildir**. Explicit `release()` temizliğin
hatasını da verir; normal Drop aynı işlemi yapar.

Akışlı commit delta0 bırakır: görünüm payload kopyalamaz. Maskeler paged
anahtar cursor'ıyla önceden hazırlanır; cache64KiB/görünüm. Hazırlıktaki
geçici dense maskeler O(toplam segment kaydı/8 byte), descriptor'lar da
segment sayısı kadardır. Owned/Vec delta bir yayın için bir kez kopyalanır;
dört okuyucu aynı Arc'ı paylaşır. Owned yolun bu turda yeni performans koşusu
yok; yalnız doğruluk testi var. Önceki owned RAM sonucu bu yolla çözülmüş
sayılmadı. Uzun snapshot'lar eski dosya/görünümleri tutar; sınırsız snapshot
ömründe disk/RAM üst sınırı kanıtlanmadı.

## Deney düzeni

Mevcut `json.payload-bench` içindeki aynı ilk10.000 belge, aynı önceki corpus
offset/case metadata'sı. Yeni corpus hazırlanmadı/indirilmedi. Yalnız
`data/benchmark-work/fdb` ve `.../sqlite` sahiplik kontrolüyle yeniden kuruldu;
DB kopyası saklanmadı. Corpus ve0.1 fixture SHA-256'ları tasarım kayıtlarında
korundu. Gereksinim notu repo içindeki belirtilen konumda yok; önceki gibi
`D:/Vibeprojects/frunner/docs/content-index/CODEX-GEREKSINIM.md` salt okunur
okundu. O nottaki50k/100k, derived/20ms, yeni arama sözleşmesi, ilk commit ve
C ABI talimatları bu turun doğrudan10k/strict/synced talimatlarının yerine
uygulanmadı; dış proje değiştirilmedi.

Her kol bakımı bitmiş aynı10k seed ile başlar; ilk1100 mevcut belge100'lük
akışlı commit'lerle güncellenir. Gövde donörü ve boyutu sabit, benzersiz
revision token ve version değişir. Bu, bütün38 sorgunun beklenen ID kümelerini
sabit tutup snapshot/version oracle'ını kesin yapar. **Rastgele yeni gövde
dağılımı, insert/delete churn'ünün performans tekrarı değildir.** Güncelleme/
silme/ekleme tutarlılığı ayrıca kontrollü testte ve kill workload'larında
kontrol edilir; önceki10/30tur disk kapısı yeniden ölçülmüş sayılmaz.

FDB'de her grup için iki dengeli sıra: yük→kontrol, kontrol→yük. SQLite aynı
şekilde ayrı iki referans çifti. Kontrolde okuyucu yok, yayın kanalı açık;
karşılaştırma publisher'ı kapatarak kolaylaştırılmadı. İlk çiftte kontrol
aynı commit sayısını kullanır; ters çiftin kontrolü önceki yük sayısını kullanır,
yeni yük okuyucular bitene kadar sürer. İkinci çiftte sayılar farklı olabilir;
ham tabloda iki sayı da vardır. Sabit bakım bedelinin amortismanı nedeniyle
bu fark önemlidir; tek çiftin%81 sonucu kararlı geçiş sayılmadı.

Yükte4 okuyucu, her biri38 sorguyu döngüde çalıştırır. FDB her fazda20
örnek/sorgu/çift, pooled40; SQLite maliyetli tam matrix için4/çift, pooled8.
SQLite8 örneğin nearest-rank p95'i maksimumdur; referansın kuyruğu için
yüksek kesinlik iddiası yok. Sayfalama, Türkçe katlama ve AND/alt-dize
sözleşmesi aynı; case34 bu adımın38-case tablosuna **dahildir**.

OS cache sıcak, tahliye yapılmadı; yeni segmentin ilk process-cache erişimi
ölçüme dahildir. Ayrı explicit query warmup yok; background/CPU/power izolasyonu
yok. Diğer build/test işleri query ölçümleriyle birlikte çalıştırılmadı.
SQLite: native rusqlite, FTS5trigram/detailnone, FULL/WAL,2MiB/connection cache,
mmap0,tempFILE;4 ayrı read-only bağlantı/BEGIN snapshot. Motor bağımlılığı
değil, izole karşılaştırma aracıdır.

Okuyucu her sorguda snapshot alır. SearchPage sequence, bütün sonuç ID'leri,
total ve dönen tam payload/version modelle karşılaştırılır. Yeni gözlenen
commit'in bütün1/100 kaydı ayrıca okunur; eski revision token yokluğu kontrol
edilir. Model gelecekteki sequence'i commit'ten önce kaydeder; okuyucu
sequence'ine göre yalnız≤sequence satırları seçilir. Oracle ledger kilidi
engine kilidi değildir, ayrı ham sayaçtır.

Latency=`admission + search + release`; oracle zamanı bu toplama eklenmez.
Oracle sırasında snapshot tutulur; onun CPU/I/O'su yükü ve pin ömrünü etkiler.
Tam “snapshot elde tutuldu, oracle dahil” süre de her ham satırdadır. Bu nedenle
oracle'sız Frunner servisinin kesin throughput'u gibi sunulmaz. Akışlı olan
yazma API'sidir; arama mevcut tam SearchPage'i döndürür. `search_into` API'sinin
ayrı allocation avantajı bu turda ölçülmedi.

8. commit'te yazar kalan normal merge'leri tamamlar, ardından gerçek revision
rewrite'ını bitirir. Bu sırada okuyucular durmaz. Her job'un öncesi/sonrası
segment kimlikleriyle rewrite, outputsuz retirement'tan ayrılır. Bu bakım
aralığı throughput'a dahildir; liveness teşhis taramaları da içindedir. Writer
bakım aralığında yeni commit göndermez; **kesintisiz yazı varken rewrite'ın
iptal olmadan ilerlediği iddia edilmez**. Sürekli yazının rewrite'ı iptal edip
bakımı boşta zamana ertelemesi Adım3 için bilinen sınır olarak korunur.

## Sonuç tabloları ve ham kanıt

[Her38 sorgu için p50/p95/max, iki boşta tabanı, mutlak farklar, bütün
geçti/kaldı satırları ve throughput tablosu](benchmarks/snapshot-10k-20261026-tables.md).
[Makinece okunabilir özet ve ham log SHA'ları](benchmarks/snapshot-10k-20261026-summary.json).
Tablolar `tools/report-snapshots.py` ile yalnız ham JSONL'den üretilir.

Dengeli kabul deneyi **v2** executable'larıyla yapıldı. Her çiftin sayıları:

| Motor / grup / çift | Yük/kontrol commit | ops/s kontrol → yük | Oran |
|---|---:|---:|---:|
| fdb/one/r0 | 57/57 | 9.313 → 6.572 | 70.57% |
| fdb/one/r1 | 66/57 | 9.373 → 7.622 | 81.32% |
| fdb/100/r0 | 18/18 | 274.921 → 208.094 | 75.69% |
| fdb/100/r1 | 20/18 | 272.734 → 222.529 | 81.59% |
| sqlite/one/r0 | 2897/2897 | 140.951 → 97.254 | 69.00% |
| sqlite/one/r1 | 2897/2897 | 140.852 → 98.378 | 69.84% |
| sqlite/100/r0 | 34/34 | 187.998 → 118.503 | 63.03% |
| sqlite/100/r1 | 34/34 | 183.036 → 116.972 | 63.91% |

Pooled throughput, toplam mutasyon/toplam writer-wall oranlarının bölümüdür; iki yüzdenin basit ortalaması değildir. Bakım/liveness aralığı dahildir. SQLite da bu yükte%80'i tutmadı; bu FDB kapısına istisna oluşturmaz.

FDB v2 process peak private41,89–48,50MiB, peak WS44,94–51,86MiB. Bunlar oracle/model ve4 reader dahil OS proses sayaçlarıdır; motor içi tahmin veya5sn temiz boşta RAM ölçümü değildir.

Her FDB yükte en az bir normal merge ve gerçek rewrite tamamlandı; her4 thread'in bakım aralığıyla örtüşen sorgu kaydı var. Ayrıntılar ham `maintenance.details` içindedir. Bu, önceki10/30tur disk/bakım koşusunun tekrarı değildir.

İlk naif kalibrasyon (tek sıra, kabul A/B'si değil):32 commit'te publisher kapalı/açık ortalama68,866/102,739ms;43 yayında919,718ms hazırlık. [Ham pilot](benchmarks/snapshot-10k-20261026-pilot-fdb-streamed-snapshot-pilot.jsonl) korunur. Sonra snapshot hazırlığında per-key metadata lock yerine ileri paged cursor kullanıldı. Koşullar/yerleşim farklı olduğundan bu sayılardan izole hızlanma yüzdesi çıkarılmadı.

### Son bırakma sayaç yarışı ve v3 ek ölçüm

`Arc::try_unwrap` başarısız olup Arc bırakılırken başka okuyucu da bırakırsa, son GC raporsuz Err dalında çalışabiliyordu. Dışarıdan ölçülen release wall ve ana latency bunu içeriyordu; v2 alt bekleme/unlink sayaçları bu kenar durumda eksik kalabilir. Güncel kod atomik `Arc::into_inner` kullanır; eşzamanlı iki son release için temizliğin tam bir kez raporlandığı test edildi.

Bu **v3** için iki ek760-sorguluk yük ölçümü ve yedi kill/reopen tekrarı yapıldı. Writer-only kontrol tekrarlanmadı; bu ek koşular yeni dengeli throughput kabulü sayılmaz. v2'nin başarısız sonuçları silinmedi veya v3'le tek havuz gibi birleştirilmedi.

[v3 tüm38 sorgu, mutlak farklar ve beklemeler](benchmarks/snapshot-waits-10k-20261026-tables.md) / [ham özet](benchmarks/snapshot-waits-10k-20261026-summary.json).

| v3 grup | Writer bekleyen sorgu | En uzun tek bekleme ms | Admission sayı / max ms | Release max ms | Release temizlenen dosya |
|---|---:|---:|---:|---:|---:|
| one | 260/760 | 1.3247 | 0 / 0.000000 | 11.4278 | 243 |
| 100 | 215/760 | 0.2842 | 0 / 0.000000 | 14.8926 | 96 |

v3 tekli yükte4R tabanı38/38;100'lüde **case22 `quixote whale`7,6778→15,6912ms (+8,0134ms;2,044×)** ve **case32 `the /5000/50`3,6280→8,0226ms (+4,3946ms;2,211×)** sınırı aştı. Bunlar<1ms istisnasında değildir. Tek okuyucu tabanında tekli14/38,100'lü13/38 aşım var. Dolayısıyla arama kapısı da kararlı geçiş diye kapatılamaz. Admission'da0 bekleme gözlenmesi, bütün sorgunun kilitsiz veya tüm OS beklemelerinin0 olduğu anlamına gelmez.

v3 process peak private44,50/47,00MiB; temiz boşta15MiB kapısı ölçülmedi. Uzun snapshot/disk üst sınırı ve sürekli yazmada rewrite ilerlemesi açık.

## Bekleme ölçümünün kapsamı

Admission pointer RwLock'u; search'te paged metadata, term-page ve block-check
cache mutex'leri; release'te aynı cache destructor'ları ve root/GC mutex'i
ayrı izlenir. “Writer bekleyen sorgu”, bu noktalardan en az birinde writer
sahipliği contention anında gözlenen sorgudur. Bir sorgu yalnız bir kez
sayılır; en uzun tek bekleme ve sorgu içi toplam ayrıca ham özettedir.
Writer'ın OS scheduler/disk kuyruğu yüzünden dolaylı etkisi bu sayaçlarla
tam sınıflanmaz. SQLite busy-handler retry zamanı ayrı referanstır; onun
hangi iç mutex sahibinden geldiği iddia edilmez.

## Kalan en pahalı iş

İlk çiftte bakım aralığı tekli1295→2172ms,100'lü1148→1949ms oldu.
Yayın hazırlığı tekli572→1113ms,100'lü207→242ms; bunlar koşu toplamıdır.
Pointer yayımı toplamı tekli kontrol/yük0,0062/0,0073ms;100'lü0,0023/0,0022ms.
**Bunlar exclusive aşamalar değildir:** yayın hazırlığı bakımın içinde de
olabilir; yüzdeleri toplayıp tek bir kök neden sonucu çıkarılmaz.

Asıl kapanmayan kapı writer throughput'udur. Bir sonraki dar iş, bakım/worker
drain zamanını ayrıştırmak ve her yayında baştan yapılan görünürlük hazırlığını
artımlı yapma imkânını ölçmektir. Kontrol sayısını, bakım aralığını veya kötü
çifti dışarı çıkararak bu kapı geçirilmedi. FDB'nin tek belge commit maliyeti
SQLite'tan hâlâ yüksektir; owned/Vec RAM'i bu açığı kapatmak için kullanılmadı.

Bilgi amaçlı: `zx/the zx` iki karakterli, olumsuz geniş taramadır. Snapshot
pointer kilidini küçültmek metin tarama maliyetini çözmez. Mevcut okuma bütçesi
içinde ardışık canlı payload pencerelerini birleştirmek/yerleşimi iyileştirmek
ayrı adaydır; CRC/kimlik doğrulaması korunmalı. Bu tur bu optimizasyonu yapmadı;
önceki1.112→2.013 read parçalanma bulgusu ve case34 OPEN-WORK'te açık kaldı.

## Doğrulama ve kalan sınırlar

Güncel v3: `cargo test --all-targets` **216 geçti,0 başarısız,12 ignored**;
ignored girdiler benchmark/subprocess yardımcılarıdır. Bunların9'u snapshot'a
özel testtir: owned update/delete epoch, kontrollü4-reader+writer,
segment/WAL son pin temizliği, eşzamanlı son release raporu, writer kapalıyken
okuma/DB kilidi, bozuk root altında GC reddi, reddedilen stream'in görünmezliği,
stream ortasında eski/yeni bütünlük ve rewrite cancellation/pin etkileşimi.
İlk lint denemesi yeni testte iki stil uyarısıyla durdu; düzeltildi, ilk log
korundu. Güncel fmt/clippy ve no-default/all-features lib check; native SQLite
fmt/clippy/content_churn testi geçti.
[Komutlar/exit kodları](benchmarks/snapshot-10k-20261026-validation-v3.json),
[tam test logu](benchmarks/snapshot-10k-20261026-validation-v3-test.log).

v2 ve v3'te ayrı ayrı7 process-kill penceresi: retirement install öncesi/sonrası,
rewrite cancellation sonrası iterator, normal iterator/flush/merge ve cache
onarımı. İlk üçü açık bariyer; diğerlerinde canlı geçici dosya/yayımlanmamış
kaynak gözlenip süreç askıya alındı ve öldürüldü. Her yeniden açılışta tam10k
payload/version ve38-case oracle geçti; pending old/new marker kontrolü geçti.
Cache onarımı kaynak/ready hash'lerini değiştirmedi. Cache kill'i `open` içinde,
snapshot kanalı kurulmadan önce olur; o pencere snapshot'lı sorgu diye sunulmaz.
Diğer probe'larda başlangıç snapshot'ı pinli tutuldu.
[v3 kurtarma kanıtı](benchmarks/snapshot-kill-v3-10k-20261026-summary.json),
[v2 önceki kanıt](benchmarks/snapshot-kill-10k-20261026-summary.json).
Güç kesintisi iddiası yok.

24 Ekim frozen okuyucusu v2 ve v3'ün ürettiği kaynakları tam oracle ile okudu;
0.1 JSON fixture'ı güncel testte okunmaya devam etti, fixture/corpus SHA'ları
korundu. Kill probe'ları son sabit slotun model ledger'ını ilerletir: ilgili
`state.json` ölçüm anının arşiv DB/model kopyası değildir. Ham timing JSONL'ler
korunur, yeni DB kopyası tutulmaz.
[Byte/hash ve komut denetimi](benchmarks/snapshot-10k-20261026-audit.json).


Genel RAM/dirty-open, küçük commit, kalan arama artışları, uzun pin ömrü,
kalıcı disk/geçici tepe ve gerçek-soğuk yük [açık işlerde](OPEN-WORK.md).
Adım1'in opt-in reclaim politikası/eşiği ve mevcut optimizasyonları değişmedi;
o10/30tur kazanımları bu kodda yeniden ölçülmüş sayılmaz. Bu çalışmanın başarısız throughput kapısı nedeniyle
Adım3 ve C ABI'ye geçilmedi.
