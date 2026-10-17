# İkili sözlük ve kaynak referansı — aynı10k

Bu belge isteğe bağlı prototipin tarihsel ölçümüdür. Sonraki kullanıcı kararı
ve doğrudan varsayılan yazıcı uygulaması [yeni geçiş raporundadır](DEFAULT-SEG05-10K-20261017.md).

İki hedefte ölçülmüş kazanç var: JSON directory yerine sürümlü ikili
directory, ardından payload kopyası yerine değişmez FDBLOG04 adresleri.
Son normal derlemede strict open260,075→170,024ms (-%34,62;4/4 hızlı).
Canlı indeks+metadata353.688.186→81.904.473 B (-%76,84).
Kaynak doğrulaması, payload kontrolleri ve0.1 kaynak uyumluluğu korunur.

Bu **isteğe bağlı SEG05 prototipidir**; normal yazıcı/onarım/merge SEG04
üretmeye devam eder. Arama kapısının tamamı geçmedi. Deney sonunda sabit
slot eski okuyucuyla tekrar SEG04'e oluşturuldu; başlangıçtaki133 FDB
slot dosyasının tamamı byte/hash olarak aynı. Kaynaklar, ready, WAL ve
SQLite değişmedi. Yeni corpus veya veritabanı kopyası üretilmedi.

## Düzen ve sınırlar

- Corpus: mevcut `data/rkyv-corpus-20261009/json.payload-bench`, aynı ilk10.000
  kayıt;264.842.858 body byte,2057–51.197 B/passage, sekiz kitaptan uzun metin.
  Tam dosya2.740.862.279 B, SHA256
  `11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B`.
- Yalnız sabit `data/benchmark-work/fdb` indeksleri dönüştürüldü. Source/ready
  ve SQLite hash'leri her koşuldan sonra aynı; okuma sırasında idx hash'leri de
  aynı. Corpus hazırlama aracı çalıştırılmadı. Repack süresi açılışa gizlenmedi.
- Aynı executable içinde JSON/ikili koşulları; dört dengeli süreç çifti,
  sıralar JSON/ref,ref/JSON,JSON/ref,ref/JSON. Ayrıştırma deneyi üç koşullu
  dört tur; her ikili karşılaştırmada ilk/ikinci sıra dengeli.
- Yeni proses, sıcak OS cache; tahliye/gerçek-cold yok. Windows affinity mask4
  başlatmadan sonra uygulanır; prosesin ilk talimatından itibaren sabitleme
  iddiası yok. Diagnostic derlemeler normal sürelerden ayrı tutuldu.
- Strict kaynak+indeks CRC/schema/UTF-8/key/link/version/bounds doğrulaması,
  pending0. Veri ve arama anlamı değişmez. Varsayılan profil/feature/dependency,
  kaynak/manifest/payload/protokol biçimleri aynı.

[Wire biçimi ve API](SEG05-CACHE.md),
[normal açılış tasarımı](benchmarks/compact-cache-window-open-10k-20261017-design.json),
[son koruma ve dosya kanıtı](benchmarks/compact-cache-final-proof-20261017.json).

## İki değişikliğin ayrı etkisi

İlk normal dört-tur deneyinde payload kopyasını koruyan ikili sözlük,
open265,709→236,544ms (-%10,98;4/4), ilk-kullanım270,406→240,982ms
(-%10,88;4/4) verdi. Bu satır sonraki executable/oturumla birleştirilmez.
Kopyalı ikili biçim ayrı38-case performans kapısından geçirilmedi; son
geniş sorgu kapısı ikili+referans birleşimi içindir.

| Canlı disk bileşeni | JSON/SEG04 | İkili+kopya | İkili+referans |
|---|---:|---:|---:|
| Ana idx |353.242.581 B |349.308.197 B |81.458.868 B |
| MET04 sayfaları |445.605 B |445.605 B |445.605 B |
| İndeks toplamı |353.688.186 B |349.753.802 B |81.904.473 B |
| Directory |8.507.592 B |4.573.208 B |4.573.208 B |
| Payload kopyası |267.849.329 B |267.849.329 B |0 B |
| Posting verisi |76.885.360 B |76.885.360 B |76.885.360 B |

Payload267.849.329 B ve sözlük3.934.384 B olmak üzere toplam271.783.713 B
çıkarıldı. Tutulan eski nesiller dahil core dosyalar deney sırasında
1.521.041.630→1.249.257.917 B olur; eski nesiller silinmedi. Bu toplam hâlâ
önceki SQLite584MB referansından büyüktür. Son durumda SEG04 byte'ları geri
geldiğinden bu disk kazancı **etkin deneme koşuluna** aittir.

[İlk ayrı-koşul ölçümü](benchmarks/compact-cache-open-10k-20261017-summary.json).

## 75ms hazırlığın yeni maliyeti

Ayrı instrumented dört-tur teşhis; span'ler iç içedir, toplanmaz ve normal
derleme performansı yerine geçmez. Kaynak kontrol algoritması aynı kaldı.

| Instrumented aşama medyanı | JSON | İkili+kopya | İkili+referans |
|---|---:|---:|---:|
| Directory parse |73,053ms |40,751ms |36,766ms |
| Tam indeks CRC |87,652ms |99,344ms |19,805ms |
| Toplam indeks açılışı |167,632ms |144,459ms |60,375ms |
| Tam kaynak kontrolü |110,439ms |117,202ms |108,440ms |

İkili+kopya aynı büyük payload alanını korur; bu profilde CRC'si yükseldi.
Referansın asıl ek kazancı taranan indeksin küçülmesidir. Directory hâlâ
tamamen doğrulanır; hazırlık maliyeti sıfırlanmadı. Kaynak268MB kontrolü de
devam ettiği için SQLite normal open seviyesine ulaşılmış sayılmaz.

[Teşhis ve her koşu](benchmarks/compact-cache-profile-10k-20261017-summary.json).

## Kaynak yerleşimindeki okuma artışı düzeltildi

İlk referans adayında kaynak entry başlıkları eski bitişiklik varsayımını
bozdu. Aynı `zx` sorgusunda10.000 aday,10.000 CRC ve10.000 yapı kontrolü
korunurken read request1104→10000, window hit8896→0 oldu. İlk normal
38-case22 düşük/16 yüksek p95 verdi; bu aday genel kapıyı geçmedi.

Son aday FDBLOG04'ün5 B entry ve42 B frame-geçiş aralığını mevcut256KiB
read-ahead hesabına katar. Üç komşu istek ve düşük isabette kapanma kuralı
aynıdır; sözcüklere özel durum yoktur. Son profilde tekrar1104 request/
8896 hit; aday/CRC/validation yine10.000. Okunan byte287.040.309→286.500.112.
Bu teşhis kontrollere dokunmadan kaybolan toplu okumayı geri kazandırır;
tek başına her sorguda hız kazancı kanıtı değildir.

[İlk matris](benchmarks/compact-cache-matrix-10k-20261017-summary.json),
[üç sürümün okuma/doğrulama sayaçları](benchmarks/compact-cache-query-io-proof-20261017.json).

## Son normal derleme: açılış ve ayrı proses belleği

| Ölçüm, dört çift medyanı | JSON | Son ikili+referans |
|---|---:|---:|
| Strict open |260,075ms |170,024ms |
| Open+ilk get9999+ilk quixote |264,784ms |173,884ms |
| Query/get peak private commit |8,891MiB |8,795MiB |
| Query/get peak working set |13,301MiB |13,260MiB |

Open ve toplam ilk kullanım4/4 düşük. Private/WS de4/4 düşük. Bellek ayrı
oracle'sız prosesin24 search/771 get ömür boyu OS tepesidir; corpus/expected
belgeler yüklenmez. Private commit fiziksel RAM değildir; WS shared image
sayfalarını içerir. Bu ölçüm normal Vec ingest, repack, concurrent/delta,
kernel/file-cache veya toplam sistem RAM garantisi değildir.

[Son açılış/bellek](benchmarks/compact-cache-window-open-10k-20261017-summary.json).

## Son arama/get kapısı

Aynı30 ana sorgu+8 sayfa: yaygın/dar kelime, iki karakter, AND, Unicode,
olumsuz/uzun ve metnin sonundaki eşleşmeler. Her ana sorguda80, sayfalarda20
pooled örnek; ek yoğun sayfalama testinde sayfa başına800 örnek. Tam toplam,
sayfa içeriği, fields/links/version ve sequence20 bağımsız oracle ile kontrol
edilir. Rastgele256 get dört boyut grubu ve üç olmayan key ile sınanır.
Oracle resident belleği bu sürelerde bulunur; RAM ölçümü yukarıda ayrıdır.

Geniş matris **32 düşük/5 yüksek/1 eşit** p95. Artışlar:

| Sorgu offset/limit | JSON p95 | Referans p95 | Fark | Düşük çift |
|---|---:|---:|---:|---:|
|`he` 0/50 |1,2408ms |1,3054ms |+%5,21 |2/4 |
|`the zx` 0/50 |184,9589ms |194,1960ms |+%4,99 |1/4 |
|`whale harpoon` 0/50 |57,6083ms |58,5767ms |+%1,68 |1/4 |
|`the` 9950/50 |1,1952ms |1,3853ms |+%15,91 |0/4 |
|`ishmael` 130/1 |6,6383ms |7,5482ms |+%13,71 |2/4 |

Yoğun8 sayfada5 düşük/3 yüksek: the0/1 +%0,08, the5000/50 +%0,04,
boş quixote57/50 +%2,25. Geniş ve yoğun oturum sonuçları aynı dağılım
değildir. Artışlar dalgalanma diye kapatılmadı; bu tur ayrı A/A yapılmadı.
Get p95 boyut gruplarında +%2,13/-%0,35/+%2,02/+%1,52; artanların mutlak
farkı0,5–0,7µs, olmayan key-%12,5. Bunlar da sıfır gerileme garantisi değildir.

[Geniş matris](benchmarks/compact-cache-window-matrix-10k-20261017-summary.json),
[yoğun sayfalar](benchmarks/compact-cache-window-dense-10k-20261017-summary.json).

## Doğrulama ve karar

Varsayılan tam suite161, reference94, son all-features tam suite180 test;
fmt ve default/all-features Clippy `-D warnings` geçti. Kesilme ve tek-byte
bozulması, CRC-valid geçersiz flag/count/adres, sağlam cache+bozuk kaynak,
cache yayınından önce/unlink arası/rename sonrası process kill, üç biçim
arasında dönüş ve silme/merge doğrulandı. Strict paged ve opt-in deferred/
record-read resident metadata yolları da yeni biçimi okur. Normal CLI gerçek
SEG05'te stats/get/search yaptı. Önceki dondurulmuş okuyucu SEG05'i kaynaktan
SEG04'e tekrar kurup38-case içerik/versiyon kontrolünü geçti.

[Test özeti](benchmarks/compact-cache-final-test-summary-20261017.json),
[eski okuyucu](benchmarks/compact-cache-old-reader-compat-20261017.jsonl),
[CLI](benchmarks/compact-cache-window-cli-20261017.jsonl),
[byte/fixture/corpus kanıtı](benchmarks/compact-cache-final-proof-20261017.json).

**Açılış/disk yönü ölçülmüş olarak yararlı; genel kabul kapısı açık.** Yeni
cache otomatik/default yazıcıya alınmadı. Sonraki kalıcı geçiş, source-adresini
normal streamed builder'a bağlayıp tekrar encode/kopyayı yazma yolundan da
çıkarmalı; ardından yazma/bakım, kalan query/get farkları ve belleği birlikte
ölçmeli. Bu tur SQLite performansı, normal Vec RAM'i, eşzamanlı bakım yükü,
gerçek cold cache, güç kesintisi ve büyük ölçek yeniden ölçülmedi.
[Açık işler](OPEN-WORK.md).
