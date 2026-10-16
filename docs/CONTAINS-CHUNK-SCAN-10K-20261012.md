# Contains: gövde dönüşümünü eşleşmede durdurma

## Sonuç

Aynı oturumdaki iki proses koşusunun percentile medyanı; p95 milisaniye:

| İşlem | Eski FDB | Yeni FDB | SQLite | Eski FDB'ye göre gecikme azalması |
|---|---:|---:|---:|---:|
| Get | 0,0771 | 0,0774 | 0,2352 | Değişmedi |
| `the` (10.000 sonuç) | 709,40 | **370,04** | 1.539,00 | %47,8 |
| `quixote` (56 sonuç) | 10,36 | **8,02** | 22,33 | %22,6 |
| `he` (10.000 sonuç) | 725,86 | **374,11** | 1.499,09 | %48,5 |
| `the man` (9.932 sonuç) | 713,25 | **384,18** | 1.637,24 | %46,1 |

P50 değerleri:

| Sorgu | Eski FDB | Yeni FDB | SQLite |
|---|---:|---:|---:|
| `the` | 664,60 | 342,67 | 1.239,71 |
| `quixote` | 8,81 | 7,23 | 17,08 |
| `he` | 665,15 | 342,82 | 1.209,98 |
| `the man` | 663,14 | 356,28 | 1.310,85 |

İki eşleşmede de dört sorgunun p95'i yeni FDB lehine. Geniş sorgu kazancı
eşleşme0'da %48,7–51,7, eşleşme1'de %43,9–47,2; dar sorgu kazancı
%31,4 ve %14,8. Tek koşunun mutlak süresi seçilmedi. Örneğin `the` p95
eski611,14 / yeni313,55 ve eski807,65 / yeni426,52 ms; koşu farkı belirgin.
Yeni FDB bu SQLite düzeninde geniş sorguda p95 yaklaşık 4,0–4,3 kat,
dar sorguda 2,8 kat kısa. Diğer sorgular veya SQLite düzenleri için genellenmez.
Geniş sorgunun 370–384 ms olması hâlâ yüksek; 50k/100k veya 30 sorgu kabul
kapısının geçtiği iddia edilmez. Disk/RAM kazancı iddiası yok.

Açılış medyanı eski1.013,81 / yeni1.005,54 ms; bu değişiklik açılışı hedeflemez.
SQLite bağlantı/pragma9,93 ms, tam kaynak doğrulaması yapmadığı için eşdeğer
açılış karşılaştırması değildir. Get p95 eski/yeni farkı 0,0003 ms,
get yolu değişmedi ve hız kazancı iddiası yok.

Altı proses: 18.000 zamanlı get ve 720 zamanlı sorgunun tüm full belge,
versiyon, toplam ve ilk 50 sayfa oracle kontrolleri geçti (ısıtma hariç).
Tüm mevcut `.fdb/.ready/.idx` ve `sqlite.db` dosyalarının önce/sonra SHA256
ve boyutları aynı. Yeni corpus, yeni DB kopyası veya yeniden indeksleme yok.

## Değişiklik

1024 bayttan uzun gövdeler artık UTF-8 sınırlarında yaklaşık 1 KiB parçalarla
katlanır ve taranır. Başlıkta veya önceki parçalarda bulunan terimler tekrar
aranmaz; tüm AND terimleri bulunduğunda kalan gövde normalleştirilmez.
Parçalar arasında en uzun katlanmış terimin bayt uzunluğu eksi bir kadar
örtüşme tutulur, başlangıç geriye doğru UTF-8 sınırına yuvarlanır. Böylece
sınırı geçen terimler ve Unicode dönüşümündeki boyut değişimi kaçırılmaz.
Bir terim yoksa gövdenin sonuna kadar tarama devam eder.

Kısa gövdelerde önceki tam dönüşüm ve 256 KiB query-local tekrar cache'i
korunur. Uzun gövdeler için tam dönüşüm cache'i oluşturulmaz. Ek scratch
bir küçük parça/örtüşme String'i ve terim başına bir bool'dur; kalıcı veya
prosesler arası cache eklenmedi. Genel API'nin 4096 bayt sorgu sınırı geçerlidir.

CRC, OFF1 sınır/UTF-8/typed metadata kontrolleri, key/version doğrulaması hâlâ
payload'ın tamamına uygulanır; erken çıkış yalnız contains doğrulamasının
metin dönüşüm/tarama kısmındadır. Get yolu ve kalıcı format değişmedi.
Yeni feature yok: doğrulanmış metin tarama iyileştirmesi normal arama yolundadır.
Önceki flush/bitmap deneyleri default kapalıdır.

## Profil

Eski/yeni diagnostic-timing build'lerinde dört aynı sorgu, bir ölçümsüz ısıtma
ve üç ölçümlü tekrar. Kaynak oracle ve açılış profil snapshot'ından çıkarılır.
`query.payload_crc`, `query.payload_validation`, `query.verify_text`,
`query.fold.body` ve yeni `query.scan.body` ayrı sayaçlardır. `verify_text`
içindeki fold/scan süreleriyle, toplamın içindeki disk/payload süreleri iç içedir;
bu sayaçlar CPU zamanı değildir ve birlikte toplanmaz.

Örnek orta tekrar (`the`, teşhis build'i): toplam 540,27 → 283,00 ms;
gövde dönüşümü 259,39 → 9,39 ms; CRC 137,97 → 138,14 ms;
payload biçim doğrulaması 52,16 → 50,62 ms; disk read span toplamı
80,37 → 74,58 ms. Normal performans kararı aşağıdaki instrumentation'sız
eşleşmiş koşulara dayanır. Ham üç tekrar:
`benchmarks/contains-profile-control-10k-20261012.jsonl` ve
`benchmarks/contains-profile-chunk-scan-10k-20261012.jsonl`.

## Corpus ve ölçüm düzeni

Yeni corpus/DB yok. Aynı mevcut JSON corpus'un ilk 10.000 belgesi,
264.842.858 metin baytı, 2.057–51.197 bayt/belge; sekiz kitabın çeşitli,
kısmen örtüşen pasajları. Mevcut FDB 5 immutable segment, delta=0;
mevcut SQLite optimize/checkpoint edilmiş FTS5 trigram external content.
Motor feature'ları önceki gibi `experimental-merge-postings
experimental-delta-bitmaps`, normal ölçümlerde diagnostic feature yok.
Kontrol executable'ı değişiklik öncesi kopyalandı, eski/yeni aynı FDB'yi açar.

Sıra kontrol0 → yeni0 → SQLite0 → SQLite1 → yeni1 → kontrol1.
OS cache boşaltılmadı; dosya hash'leri, corpus oracle okumaları ve her query/key
için bir ölçümsüz ısıtma sonrası sıcak cache. SQLite 2 MiB sayfa cache,
mmap kapalı, temp FILE, read-only. Her proses 30 tekrar × dört sorgu ve
100 dağıtılmış anahtar × 30 get; p50/p95 nearest-rank, rapor iki prosesin
percentile değerlerinin medyanıdır. SQL hazırlama, count + ilk 50 sıralı
full JSON payload decode/read transaction süreye dahildir. FDB Query
oluşturma, kaynak/payload kontrolleri ve full hit get süreye dahildir.
Oracle eşitlik kontrolleri süre dışındadır; FDB sequence kontrolü wrapper'dadır.
Scope shared, collection texts, offset0/limit50, kesin toplam ve ID sırası.

Bu ölçümün sınırları: dört sorgu, uzun İngilizce corpus, boş delta ve sıcak cache.
Soğuk cache, 30 kullanıcı tipi sorgu kapısı, 50k/100k, RAM/working set,
yazarken arama, ekleme/silme görünürlüğü, çok derin sayfa, çok uzun/bulunmayan
terimlerin ve birbirinin aynısı uzun gövdelerin performansı ölçülmedi.
Bulunmayan/uzun terimler doğruluk testlerinde
kontrol edildi; erken durma avantajı olmayan sorgular için hız kazancı iddiası yok.

## Doğrulama ve tekrar

Default `cargo test --all-targets`: **67 geçti**. Flush+bitmap birleşimi:
**75 geçti**. Sınır testleri Türkçe, Unicode büyüme/küçülme, çok baytlı karakter,
başlık/gövde AND, negatif sorgu, 3 KiB terim ve scratch tekrar kullanımını
tam fold oracle'ıyla karşılaştırır. Mevcut kesilme/bozulma/kurtarma testleri de
bu test suite'lerinde geçti. Kalıcı depolama kodu ve 0.1 fixture değiştirilmedi.
`cargo fmt --check`, default ve all-feature
`cargo clippy --all-targets -- -D warnings` geçti.

Normal release build sonrası `tools/measure-content-reads.ps1 -CompareChunkScan`.
Kontrol `output/content-reads-control-20261012.exe` gereklidir. Script mevcut
slotları kullanır, reset/ingest/maintenance çağırmaz. Ham kayıt/hash prefix'i
`benchmarks/contains-chunk-scan-10k-20261012`.
Özet: `tools/summarize-content-reads.py --prefix contains-chunk-scan-10k-20261012
--runs 2 --engines control fdb sqlite`.
