# Blok 3a sınırlı tanı — 3 Kasım 2026

**Sonradan çalışma alanı onarımı tamamlandı:** izinli16 kayıt iki motorda
GET ile doğrulanıp silindi; reopen/50k oracle/41-case/kill/eski okuyucu geçti,
slotlar ready. [Onarım raporu](BLOCK3A-REPAIR-20261103.md).
Aşağıdaki ölçüm sonuçları ve bloke durum **onarım öncesi tarihsel kayıttır**;
yeni ölçüm veya harness düzeltmesi yapılmadı. Kullanıcı kararıyla3a KISMEN,
tanı yeterli sinyal verdi; özgün performans kapısı ÖLÇÜLMEDİ.

**YARIM RAPOR — ölçüm adaptörü hatası nedeniyle DURULDU.** A/A tamamlanmadı,
`maintain_idle()` çağrılmadı, L1 yok. Run–gap ilişkisi ve gürültü yayılımı
**bilinmiyor**. Payload birleştirme için bu tur **ÖNERİ-YOK**; optimizasyon
veya varsayılan değişikliği yapılmadı. Yeni tur/gizli tekrar yok.

**Önemli çalışma alanı durumu:** iki motorda `keybench-numeric` koleksiyonundaki
16 tanı kaydı temizlenmedi. Ana 50k'nın başlangıç oracle'ı geçti; hata sonrası
yeniden açılış/ana oracle yapılmadı. Her iki sabit slot
`blocked_partial_diagnostic` olarak işaretlidir. Kopyalanan model dosyalarının
sequence'i güncel DB sequence'i diye kullanılamaz. Yeni ölçüme başlanmamalı;
doğrulanmış havuz temizliği ve model sequence eşitlemesi ayrı devam onayı bekler.

## Önceden sabitlenen sınır ve gerçekleşen kapsam

[Çalıştırmadan önce gösterilen plan](benchmarks/block3a-20261103-plan.md):
ilk DB açılışından son doğrulama/temizliğe kadar **1200 s**; derleme/test ayrıca
**1800 s** üst sınır. Derleme/test 801,754 s; DB bloğu **131,430 s** sürdü
(yerel saat yaklaşık 21:55:32–21:57:43). Süre aşımı yok; hata üzerine duruldu.
Bu süreler belge inceleme/rapor yazma süresi değildir.

| Ölçüm | Plan üst sınırı | Gerçekleşen |
|---|---:|---|
| Gözlem hücresi | 4 × 2 motor | Yalnız L0-A: FDB tamam; SQLite taramalar tamam, gap başarısız |
| Normal zx/the zx | 960 çağrı | 240; her motor/sorgu N60 |
| Ayrı tarama warmup/profil | 16 / 32 | 4 / 8 |
| Tekli update/gap | 512 işlem | FDB 64 işlem, 60 normal + 4 profilli gap; SQLite gap kaydı 0 |
| Profilli gap | 32 (normalden ayrı) | FDB 4; SQLite ilk committen sonraki acquire hata verdi |
| Sınırlı okuyucu yükü | 2048 sorgu | Motor başına 4 × 64 = 256; toplam 512 |
| Ana-belge oracle | En çok 300k GET | İki başlangıç geçişi: 100k ana GET, ikisi de geçti |
| Mevcut bakım çağrısı | **1** | **0** — A/A tamamlanmadan bakım yapılmadı |
| Temizlik / kill-reopen / eski okuyucu | Planlı | Bu DB koşusunda yapılmadı |

Oracle GET ve eski-marker sorguları zamanlı örnek sayısına katılmadı; bunların
da üst sınırı planda yazılıdır. Tamamlanan sonuçlar ayıklanmadı; eksik SQLite
hücresi tamamlanmış çift veya pooled A/A sonucu yapılmadı.

## Ortam ve sözleşme

Mevcut sabit `data/benchmark-work/fdb` ve `sqlite` slotları; ana 50k + ayrı
koleksiyonda 16 adet mevcut 2–4 KiB donör. Kaynak SHA256
`881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12`.
Yeni corpus, DB kopyası, reset veya yeniden ingest yok; `private/` okunmadı.
Sadece model JSON'ları/loglar kopyalandı. Her hücrede aynı key/gövde/marker
dizisi planlandı; sequence doğal olarak ilerler. Akışlı WAL katmanı ölçümde
açık, reclaim kapalı; try/seri K, A/D kapalı. Engine varsayılanları aynı.

Taramalar 1 çağıran thread, melez200 N50; iç işçiler mevcut dört izinle sınırlı.
Bu 4 bağımsız okuyucunun normal41 kabul koşusu değildir. SQLite mevcut
FTS5 trigram, WAL/FULL, cache 2 MiB, mmap0, temp FILE; aynı key sırası ve
substring-AND oracle. Her normal tarama öncesi aynı sorguya bir warmup; uygulama
cache'i/OS cache sıcak ve kontrolsüz. OS-cache düşürülmedi. Yüzdelik
nearest-rank `ceil(p*n)`; N60 p95 = sıralı 57. örnek, max değildir.
Profil açık örnekler primary p95'e katılmadı. Süreç CPU'su bütün query
işçilerini kapsar; kısa çağrılarda GetProcessTimes kuantizasyonu vardır.

## 1. Önce A/A: tamamlanmadı

| Motor / hücre | zx p95 ms | the zx p95 ms | Gap p95 ms | Immutable run |
|---|---:|---:|---:|---|
| FDB L0-A | 128,776 | 46,877 | 16,831 | 52 → 52; ID/byte listesi aynı |
| SQLite L0-A | 590,505 | 382,552 | ÖLÇÜLMEDİ | SQLite run metriği yok |
| FDB L0-A′ | ÖLÇÜLMEDİ | ÖLÇÜLMEDİ | ÖLÇÜLMEDİ | Başlamadı |
| SQLite L0-A′ | ÖLÇÜLMEDİ | ÖLÇÜLMEDİ | ÖLÇÜLMEDİ | Başlamadı |

**A/A farkı, min/medyan/max çift yayılımı ve gürültü zarfı hesaplanamaz.**
Eski K 12→32 ms / 44→7→52 run gözlemi bu eksik veriyle neden-sonuç ilişkisine
çevrilmedi. Yeni L0 tek kolu eski oturumdan bir hızlanma sayılmaz. SQLite
satırları yalnız tek-kol referansıdır; dengeli çift/üstünlük iddiası yok.

| Motor / sorgu | Normal n | p50 ms | p95 ms | Max ms | Toplam CPU ms / çağrı ortalaması | Dönen |
|---|---:|---:|---:|---:|---|---:|
| FDB zx | 60 | 122,111 | 128,776 | 133,108 | 19781,250 / 329,688 | 28 |
| FDB the zx | 60 | 44,181 | 46,877 | 48,092 | 6765,625 / 112,760 | 13 |
| SQLite zx | 60 | 566,541 | 590,505 | 656,157 | 32812,500 / 546,875 | 28 |
| SQLite the zx | 60 | 305,912 | 382,552 | 390,754 | 18421,875 / 307,031 | 13 |

Bu korpusta ikisi de sıfır sonuçlu değildir; N50 dolmadığı için gerçek toplamı
bulana kadar tüketilir. Eski kitap 10k/38-case tabanıyla birebir oran kurulmaz.

## 2. Tamamlanan FDB gap ve sınırlı profil

| FDB normal N60 | p50 ms | p95 ms | Max ms |
|---|---:|---:|---:|
| Commit | 1,966 | 2,624 | 4,906 |
| Commit dönüşü → ilk sonuç | 6,189 | 16,831 | 19,214 |
| Acquire | 0,0004 | 0,0005 | 0,0006 |
| İlk marker sorgusu | 6,187 | 16,828 | 19,212 |
| Aradaki query hazırlığı / sayaç başlangıcı | 0,0021 | 0,0027 | 0,0045 |

**Yük kapsamı kusuru:** her okuyucu en çok 64 çağrı yaptığı için yük erken
bitti. Normal 60 gap örneğinin 46'sında aktif okuyucu 0, 5'inde 1, 4'ünde 2,
5'inde 3; **hiçbirinde 4 değil**. Profilli ordinal0'da 4 okuyucu vardı.
Bu karışık örnekler 4R+W kabul p95'i olarak kullanılamaz. Yeni örnek ekleyerek
düzeltilmedi. Bu, sonraki tanı için ayrıca ele alınması gereken harness sınırıdır.

| Profil ordinal / sequence | Aktif R | Gap ms | Posting hazırlığı ms / run çağrısı | Term-cache acquire ms | Gerçek contended ms | Read+CRC+decode ms |
|---|---:|---:|---|---:|---:|---:|
| 0 / 529903 | 4 | 17,816 | 17,395 / 52 | 11,176 | 11,089 | 2,970 |
| 16 / 529919 | 0 | 5,722 | 5,589 / 52 | 0,038 | 0 | 3,341 |
| 32 / 529935 | 0 | 4,232 | 4,130 / 52 | 0,031 | 0 | 2,435 |
| 48 / 529951 | 0 | 6,260 | 6,132 / 52 | 0,036 | 0 | 3,744 |

Nested kalemler toplam süreye yeniden eklenmez. İlk örnekte bekleme ölçüldü;
hangi threadin ne kadar kilit tuttuğu/OS-cache/scheduler/antivirüs nedeni
**bilinmiyor**. Dört profilde 52 owner'a dokunuldu; run sayısı hiç değişmediği
için run sayısının marjinal ms maliyeti çıkarılamaz. Tek ağır örnekten tüm
gap dağılımının kök nedeni ilan edilmez.

| Ayrı tarama profili | Wall ms (2 örnek) | Canlı aday / payload read | İstenen payload MB | Window hit | Komşu ≤42 B / gözlenen range |
|---|---|---|---:|---:|---|
| zx | 153,942 / 153,768 | 50000 / 50000 | 171,044 | 0 | 49282 / 50000 |
| the zx | 52,306 / 50,802 | 12081 / 12081 | 72,743 | 0 | 2913 / 12081 |

İlk zx profilinde işçilerde biriken candidate-verify span toplamı409,039 ms;
bunun içindeki read199,746, CRC27,831, schema60,865, text66,003 ms'dir.
Toplam wall153,942 ms'den büyük olması paralel/nested span'ler nedeniyledir;
bu sayılar wall yüzdesi veya kazanç üst sınırı diye sunulmaz. Normal/profilli
wall farkı görünür; saf profilleme maliyeti için eşlenmiş A/A yoktur.

Histogram per-verifier/per-file erişim sırasıdır; her cursor'un ilk aralığında
öncül bulunmaz, worker sınırındaki komşuluk sayılmaz. Tüm dosyanın global
yerleşim histogramı değildir. API read/byte sayıları fiziksel disk veya OS-cache
miss değildir. Kaynak/indeks CRC/kimlik/schema kontrolü kaldırılmadı.
Bu melez50k yolundaki 50000 read ile eski10k exact yolundaki1112/2013 sayıları
aynı yöntemmiş gibi kıyaslanmaz. Pencere önerisi için gereken A/A + L1 kanıtı yok.

## 3. Durma nedeni ve korunan çalışma alanı

[SQLite stderr](benchmarks/block3a-20261103-sqlite.stderr):
`cannot start a transaction within a transaction`.

Ölçüm helper'ı `pinned = main.acquire()` ile eski snapshot tutuyor; ilk update
sonrasında yine `main.acquire()` çağırıyor. FDB'de bu kullanılabilir; SQLite
adaptöründe her view aynı client'ın `Rc<Connection>`'ını paylaşır ve acquire
`BEGIN` yapar. Açık eski transaction üstüne ikinci `BEGIN` hatası böyle oluştu.
Bu **benim ölçüm adaptörü hatam**, FDB/SQLite motor doğruluk hatası kanıtı değil.
Hata scope içindeki sınırlı okuyucular bitince process exit1 olarak geldi.
Driver bunu kaydetti, FDB sürecini durdurdu; kasıtlı test kill/reopen tanığı
yerine sayılmadı. Her iki PID'nin kapandığı doğrulandı.

FDB son ölçülen sequence529966, delta16; SQLite son yayımlanan setup tanığı
1026309. SQLite hata yolu ilk commitin sonrasında olduğu için bir update
uygulanmış olabilir; post-failure sequence yeniden açılarak doğrulanmadı.
16 anahtar `shared/keybench-numeric/0000000000..0000000015`.
Temizlik, bakım, reopen veya model sequence düzeltmesi yapılmadı.
[Slot kaydı](benchmarks/active-benchmark-slots.json) bunu açıkça bloke eder.

Önerilen **harness düzeltmesi** yalnız ayrı onayla: eski snapshot için ayrı
reader/client; sabit örnek üst sınırını artırmadan 4R örtüşmesini sağlayan
adım eşleştirmesi ve ön-test. Sonra mevcut havuz GET/oracle ile doğrulanıp
yalnız bu16 anahtar silinir, ana model sequence'i eşitlenir. Bu bir payload
optimizasyonu veya otomatik yeni ölçüm onayı değildir; bu tur uygulanmadı.

## 4. Kapı ve doğrulama tablosu

| Kapı / kanıt | Sonuç |
|---|---|
| Süre ve örnek üst sınırı | GEÇTİ; 131,430/1200 s, ek tur yok |
| Başlangıç 50k ana GET/version oracle | GEÇTİ, iki motor |
| Tamamlanan tarama/GET/eski-marker/eski snapshot oracle | GEÇTİ; yalnız gerçekleşen çağrılar |
| A/A gürültü yayılımı | TAMAMLANMADI |
| Tek bakım / run–gap önce-sonra | YAPILMADI; run sayısının etkisi bilinmiyor |
| Harness uçtan uca | **KALDI**, SQLite nested transaction; yük örtüşmesi de yetersiz |
| Bu DB'de kill/reopen ve eski okuyucu | YAPILMADI; fixture testleri bunun yerine sayılmadı |
| Özgün 3a: zx/the zx ≥%30, diğer36 ±%5 + A/A | ÖLÇÜLMEDİ; eşik değişmedi |
| Kaynak corpus / 0.1 fixture / frozen source ve ikili korunumu | SHA audit GEÇTİ; motor+fixture78 dosya değişmedi |
| Root test / fmt / clippy | GEÇTİ, kapsam aşağıda |

Full default230, full diagnostic234 test geçti; son sayaç düzenlemesinden sonra
odaklı3 test, ayrıca26 example testi geçti. Tekrarlanan testler toplanarak
benzersiz test sayısı yapılmadı. 15 ignored process/helper test, ilgili normal
üst testlerin yerine geçti diye sayılmaz. Root fmt check ve hem normal hem
diagnostic all-targets clippy `-D warnings` geçti. SQLite adapter clippy
`fdb/bounded-engine` ile geçti; normal comparator release build'indeki önceden
var olan `posting_pages::Pages::empty` dead-code uyarısı ayrıca build logunda.
CRC/bozulma/kill/uyumluluk fixture testleri geçti; gerçek50k son-reopen eksiktir.

Bounded owner muhasebesi her istek/thread için64 owner + taşan0; birleşimde de
aynı sınır. Ölçülen en çok52 owner; belge metni/key geçmişi eklenmedi. Adjacency
sayaçları sabit alanlıdır. Model/oracle/log belleği motor-only RAM diye sunulmaz;
bu tur ingest/idle D veya RAM kapısı yeniden ölçülmedi.

**Son karar: payload birleştirme ÖNERİ-YOK (kanıt eksik); motor optimizasyonu,
varsayılan veya format değişikliği yok.** Yalnız onaylı tanı sayaçları eklendi.
Özgün3a KISMEN kalır. Blok2 eksikleri, 3b, varsayılan kararlar, 100'lü/500drain,
2,6sn uç, WAL handle/File::open, reclaim varsayılanı, soğuk cache, uzun snapshot,
100k ve C ABI'ye geçilmedi. Devam için önce bu ölçüm hatası ve kirli havuzun
doğrulanmış temizliği konusunda kullanıcı kararı gerekir.

Ham kanıt: [sonuç/plan/SHA](benchmarks/block3a-20261103-result.json),
[hesaplanan bütün tablolar](benchmarks/block3a-20261103-analysis.json),
[FDB ham örnekler](benchmarks/block3a-20261103-fdb.jsonl),
[SQLite ham örnekler](benchmarks/block3a-20261103-sqlite.jsonl),
[FDB profil TSV](benchmarks/block3a-20261103-fdb-profiles.tsv),
[SQLite profil TSV](benchmarks/block3a-20261103-sqlite-profiles.tsv),
[süreç/çıkış nedeni](benchmarks/block3a-20261103-lifecycle.jsonl).
