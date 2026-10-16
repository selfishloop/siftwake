# Ham metinden sınırlı eşleşme kanıtı — 2026-10-16

Dar aramada tekrar eden hız kazancı elde edildi. Aynı mevcut10k, dört dengeli
proses çifti ve800 örnek/sayfa: quixote /28/10 p95 2,10→1,88ms,
quixote /57/50 1,94→1,65ms; sırasıyla yaklaşık%10/%15 düşük ve ikisi4/4
hızlı. Kesin değerler aşağıdaki ham sonuç/özet dosyalarındadır. Bu değişiklik
mevcut `experimental-builder-budget` içinde tutulur; default kapalı kalır.

Genel38-case matrix21 düşük/17 yüksek p95 verdi. Bütün aramalar hızlandı
veya genel kabul kapısı geçildi iddiası yok. Yazma/bakım, query belleği,
disk ve concurrent/cold başlıkları kapanmaz.

## Koşullar

- Windows/release, `experimental-builder-budget`, `open_record_reads`.
- Aynı `data/rkyv-corpus-20261009/json.payload-bench` dosyasının ilk10.000
  belgesi;264.842.858B gövde metni, önceki çeşitli uzun metin corpus'u.
  Yeni corpus, DB kopyası, DB reset veya bakım yapılmadı.
- Sabit `data/benchmark-work/fdb`;10k belge/sequence20/generation51/beş run,
  delta0/resident0. SQLite sabit slotu da hash kontrolüne dahildir.
- Sıcak OS cache; her case/key için zaman dışı warmup; eviction, affinity
  veya power sabitleme yok. İlk kaynak doğrulaması warm sorgu zamanına dahil
  değildir; ayrı ilk-kullanım ölçümüne dahildir. Gerçek soğuk disk ölçülmedi.
- Normal, diagnostic kapalı iki executable: kontrol
  `output/raw-proof-control-20261016.exe`, aday
  `output/bounded-raw-proof-candidate-20261016.exe`.
  A/B,B/A,A/B,B/A; her senaryoda aynı query/offset/limit, tam belge/key/
  version/sequence kontrolü ve zaman dışı256 get anahtarı oracle ile denetlendi.
- Geniş matrix primary20/page5 örnek/proses (toplam80/20); ayrı sayfa testi
  200 örnek/proses (toplam800). Geniş matristeki sayfa kuyruğu seyrektir.
  Pooled p95 ve çiftlerin p95'i farklı dağılımlardır; hızlı çift sayısı bir
  anlamlılık testi değildir. Önceki A/A serisi bu koşunun tolerans bandı sayılmaz.

## Değişen yol

Payload zaten OFF1 ikili biçimde, borrowed view ile okunuyordu. Tam JSON
çözümü kaldırılmadı; bu yolda böyle bir işlem zaten yoktu. Disk okuma,
tam kayıt CRC, UTF-8/yapı ve key/version doğrulaması aynı sırada korunur.

Title önce mevcut kuralla normalleştirilir. Kalan bütün sorgu parçaları en az
üç ASCII byte ve normalleştirilmişse, özgün body içinde küçük harfli, ilk
harfi büyük veya tamamı büyük yazımı aranır. Bu ASCII yazımların fold sonucu
sorguyla aynıdır: yalnız pozitif eşleşme kanıtlanır. Karışık büyük/küçük harf,
Türkçe/Unicode dönüşümü veya bulunamama önceki tam doğrulamaya döner. AND
parçaları title/body arasında önceki semantikle bağımsız doğrulanır.
Sözcük, belge kimliği veya corpus adına bağlı motor kuralı yoktur.

Deneme body'nin ilk64KiB'siyle sınırlıdır, sınır UTF-8 scalar'ı bölmez.
128-byte stack tamponu yalnız yazım denemesini sınırlar; uzun sorgular veya
ilerideki eşleşmeler reddedilmez, önceki yola döner. Her run/query'de sekiz
denemelik blokta en az altı başarı yoksa ek ham tarama durur. Başarılı bloklar
yeniden sekiz deneme yapar; sonradan verim düşerse yine durur. Ek kalıcı/query
cache, full-body buffer, feature, bağımlılık veya dosya biçimi eklenmedi.

## Ölçülmüş hız

Yoğun sayfa testi; pooled p95 ve daha hızlı proses çifti:

| Sorgu / offset / limit | Kontrol→aday p95 ms | Değişim | Hızlı çift |
|---|---:|---:|---:|
| quixote /28/10 |2,10→1,88 |-%10,41 |4/4 |
| quixote /57/50 |1,94→1,65 |-%15,04 |4/4 |
| ishmael /130/1 |6,32→5,86 |-%7,28 |4/4 |
| ishmael /60/200 |12,12→11,62 |-%4,17 |4/4 |

Yoğun testin toplamı7 düşük/1 yüksek p95. Kalan dört the sayfası bu yeni ham
body yolunu çalıştırmaz; /0/200 +%1,71 ve2/4 hızlı. Bu fark otomatik olarak
dalgalanma kabul edilip kapatılmadı.
[800 örnekli özet](benchmarks/bounded-raw-proof-pages-10k-20261016-summary.json),
[sıra tasarımı](benchmarks/bounded-raw-proof-pages-10k-20261016-design.json).

Geniş38-case matrix: quixote whale /0/50 6,55→5,02ms (-%23,35,4/4);
İSHMAEL 11,19→10,16ms (-%9,17,4/4); ishmael /130/1 -%9,47,4/4.
Toplam21 düşük/17 yüksek. quixote ilk50 pooled +%2,94 iken3/4 çift daha
hızlıdır; yalnız olumlu bir sayıyı seçip bütün sorguyu kapatmıyoruz.
Her çiftte p95 yükselen transubstantiation (+%1,11), elizabeth (+%6,50),
holmes (+%8,29), galapagos (+%1,72) ve diğer artışlar açık kalır.
[Bütün38 sonuç/ham örnekler ve çift farkları](benchmarks/bounded-raw-proof-matrix-10k-20261016-summary.json).

SQLite aynı slotta yeniden dört-query referansıyla ölçüldü; tek proses,
30 örnek/query,2MiB SQLite cache/mmap0 ve mevcut trigram FTS+LIKE+kesin total
düzeni. Bu, yeni dört çift FDB/SQLite genel kapısı değildir. Ayrı zamanda
çalışan, farklı örnek sayılı sonuçlar aynı dağılım sayılmaz.
[SQLite koşulları ve ham referans](benchmarks/raw-proof-matrix-10k-20261016-sqlite-reference.jsonl).

## Gerçek iş azalması

Ayrı diagnostic build; her modda bir proses/üç warm iteration, zamanlama
kazanımı hesabında kullanılmaz. Her üç iteration'da aynı iş sayaçları:

| Sorgu | Body fold/scan kontrol→aday | Ham deneme / kanıt (aday) | Payload read / CRC / yapı kontrol |
|---|---:|---:|---|
| quixote |815→0 |56 /56 |56 /56 /56, iki sürümde aynı |
| beagle |19.535→19.483 |40 /4 |579 /579 /579, iki sürümde aynı |
| transubstantiation |55.676→55.676 |40 /0 |1.527 /1.527 /1.527, iki sürümde aynı |
| holmes elementary |29.526→29.490 |40 /4 |924 /924 /924, iki sürümde aynı |
| the zx |263.627→263.627 |0 /0 |1.104 read,10k CRC/yapı, iki sürümde aynı |

40 başarısız deneme sınırı beş run ×sekizdir;1.527 adayda tam body'yi üç
kez taramaya devam edilmez. quixote yine1.945.325B okur; kazanım daha az
disk I/O veya kaldırılmış CRC değildir. Önceki815 gövde dönüşümü/scan yerine
56 doğrudan pozitif kanıt vardır. Kaynak ilk doğrulaması/izinleri aynı kalır.
[İş sayaçları](benchmarks/bounded-raw-proof-counts-20261016.json).

## Önceki iki deneme

Yalnız lowercase raw arama genel matrix13 düşük/25 yüksek verdi; reddedildi.
Sınırsız üç yazım denemesi dar sayfaları hızlandırdı fakat genel matrix
17 düşük/21 yüksek, beagle/harpoon/transubstantiation gibi düşük isabetli
aday listelerinde ek tarama arttı. Bu sınırsız varyant da kabul edilmedi;
son kod64KiB ve başarı oranı sınırlarını kullanır. Farklı zamanlarda ölçülen
bu seriler birbirleriyle paired kıyas sayılmaz; her biri kendi eski kontrolüyle
eşleştirildi.
[İlk deneme](benchmarks/raw-proof-matrix-10k-20261016-summary.json),
[sınırsız matrix](benchmarks/raw-case-proof-matrix-10k-20261016-summary.json),
[sınırsız sayfa](benchmarks/raw-case-proof-pages-10k-20261016-summary.json).

## RAM, disk ve kabul sınırı

İlk-kullanım prosesi open/get/ilk search/repeat içerir; corpus veya oracle
resident değildir. Warm query'nin bağımsız tepe belleği değildir.10ms
örnekli private sayaç kısa tepeleri kaçırabilir; WS/private aynı bellek
tanımı değildir. İlk örnekli sonuç tek başına RAM kabul kapısı sayılmaz.
[Örnekli ham sonuç](benchmarks/bounded-raw-proof-first-use-20261016-summary.json).

Bu farkı kontrol etmek için dört dengeli çift ayrıca proses sonlandıktan sonra
açık tutulan handle üzerinden `GetProcessMemoryInfo` ile ölçüldü. İşletim
sisteminin ömür boyu tepe sayaçları kullanılır;10ms örnekleme boşluğu yoktur:

| Sayaç, dört proses medyanı | Kontrol | Aday | Fark |
|---|---:|---:|---:|
| PeakPagefileUsage: private committed byte tepesi |6,920MiB |7,035MiB |+118KiB;3/4 çift yüksek |
| PeakWorkingSetSize: resident/shared image dahil tepe |10,639MiB |10,621MiB |-18KiB;2/4 çift düşük |

Private commit fiziksel RAM veya toplam sistem belleği değildir; working set de
yalnız motor heap'i değildir. İlk10ms serisindeki WS6,619→10,559MiB farkı,
tam-ömür OS sayaçlarında tekrarlanmadı. Bu, örnekli kontrolde kısa son-query
tepesinin kaçmasıyla uyumludur; seriler farklı proses/zaman aralıklarıdır,
farkın bütün nedeni ayrıştırılmadı. O seriden4MiB kod kaynaklı RAM artışı
çıkarılmaz. Öte yandan OS private-commit medyanındaki118KiB artış saklanmaz;
bu ölçümde de sıfır bellek artışı şartı kanıtlanmadı. Warm/query-only tepe,
eşzamanlı yük ve daha büyük veri kapsamı açık kalır.
[OS tepe sayaçları/sekiz ham proses](benchmarks/bounded-raw-proof-os-memory-20261016-summary.json).

Kodda iki query-local byte sayaç ve128-byte stack yazım tamponu vardır;
ek büyük text cache yoktur. Bu, toplam proses RAM'i hiç artmadı iddiası
değildir. İndeksleme/maintenance RAM'i ve yazma süreleri yeniden ölçülmedi;
önceki indexing private9,129MiB/WS13,027MiB ile yazma+bakım +%3 gözlemi
bu okuma testiyle kapanmaz. Normal/default API ve genel heap bütçesi açıktır.

Kaynak/index/WAL+ready canlı boyut621.589.411B; retained
.fdb/.idx/.ready1.521.041.630B; SQLite583.860.224B. Boyut/format aynıdır;
eski nesil temizliği yapılmadı. Kaynak267.900.709B sayısına WAL zaten dahildir.

## Doğrulama

Oracle ile mixed-case/Türkçe/Unicode/AND/chunk sınırları,64KiB sonrasındaki
eşleşmeler, spekülasyon durduktan sonraki pozitifler ve uzun sorgular
denetlendi. Ek bozulma/kesilme testi, matching ASCII needle korunurken bile
record'un exact/window yolunda reddedilmesini kontrol eder.

Son suite/Clippy/fmt, normal executable ve dosya hash kanıtı:
[nihai doğrulama](benchmarks/bounded-raw-proof-final-proof-20261016.json),
[38-case final tam-belge/version/sequence kontrolü](benchmarks/bounded-raw-proof-final-check-20261016.jsonl).
0.1 fixture ve authoritative/derived ayrımı korunur; kalıcı format değişmedi.

`cargo test --all-targets`: default88, builder-budget145, all-features164
geçti; sırasıyla2/4/6 subprocess helper ignore'dur. Süreç kill, torn/byte
corruption,0.1 compatibility ve writer/maintenance testleri dahildir.
`cargo clippy --all-targets -- -D warnings` ve all-features karşılığı,
`cargo fmt --check`, `git diff --check` geçti. Son normal reader'ın `.text`
bölümü ölçülen adayla birebir aynı. Bütün124 FDB/cache/SQLite dosyası ve
2.740.862.279B corpus SHA256'sı önceki source-lease snapshot'ıyla aynı;
fixture Git durumu boş. Engine/index/covering/dependency hash'leri aynı,
runtime değişikliği yalnız `src/segment.rs` içindedir.

[OPEN-WORK](OPEN-WORK.md): kalan arama artışları/uzun-negatif tarama,
yazma-bakım, query/normal API RAM'i, disk/eski nesiller, concurrent ve gerçek
cold yük açık. Default'a geçiş genel kapıyı geçmedi; bu dar hız adımı opt-in
profilde tutulur. Dış proje entegrasyonu yoktur.
