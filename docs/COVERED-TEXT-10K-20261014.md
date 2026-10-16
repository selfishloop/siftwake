# Uzun payload okumayı indeks kanıtıyla azaltma

Bu adım mevcut `experimental-builder-budget` içinde kalır; yeni feature,
kalıcı format, corpus veya dış proje entegrasyonu yoktur. Default seçimi aynı.
Kontrol: önceki [düşük RAM hız adımı](BOUNDED-SPEED-10K-20261014.md).
Amaç, RAM'e tam metni geri koymadan contains doğrulamasının tekrarını azaltmaktır.

## Hangi arama doğrudan indeksten karşılanır?

Motorun `Gram` posting'i, normalleştirilmiş title veya text içinde tam olarak
üç Unicode karakteri bulunan belgeleri tutar. Arama parçası da tam üç karakterse
posting eşleşmesi contains için kesin kanıttır. Bu tüm üç karakterli parçalar
için genel bir kuraldır; benchmark kelimelerine bağlı bir tablo kullanılmaz.
Örneğin `the man` iki zorunlu kesin posting'in AND kesişimidir. Fields içindeki
metin, title/text aramasına dahil olmaz. Başlık ile gövde arasındaki newline
üzerinden sahte bir üç karakterli eşleşme üretilmez.

İki karakter, daha uzun parça veya gramı olmayan sorgu içerik kontrolünden
geçer. Uzun bir kelimenin bütün gramlarının bulunması yeterli sayılmaz:
`aba xxx bab`, `abab` sorgusunu karşılamaz. Karışık AND'de yalnız kesin
parçaların doğrulaması çıkarılır; kalan parçalar mevcut CRC/checked payload,
normalleştirme ve substring taramasını kullanır. Rkyv benchmark overlay'i
bu yola geçirilmez; eski JSON/SEG03 okuması mevcut doğrulamayı kullanır.

## Kaynak ve cache kontrolleri

- `Database::ensure_run` ancak mevcut kaynak kontrolü başarıyla tamamlandıktan
  sonra bu kullanımın iznini işaretler. Deferred open'daki pending source
  kontrolleri ilk search'te sürer; sağlam cache bozuk `.fdb` kaynağı gizlemez.
- Source yolu, boyutu ve modification time ile indeks handle'ının boyut/time
  bilgileri bağlanır. Örtülen sorgu öncesi tekrar karşılaştırılır; değişiklik
  hata verir; kontrol gram/scope early-out'tan önce yapılır, boş sonuç da
  bozuk kaynağı kabul etmez. Timestamp bilgisi alınamazsa içerik kontrolüne dönülür. Source
  writer binding'den önce kapanır; Windows'un close sırasında timestamp
  güncellemesi normal commit'i sahte corruption olarak işaretlemez.
- Fingerprint CRC yerine kullanılmaz: açılış/source kontrolü ve okunan indeks
  bloklarının CRC yolu korunur. Fingerprint sıradan değişimi yakalar; timestamp
  korunarak yapılan dış değişiklik veya eşzamanlı harici dosya yazımı için
  atomik snapshot/hash garantisi değildir. DB dosyaları motorun exclusive
  ownership modelindedir; yeniden açılışta mevcut source CRC kontrolü çalışır.
- Örtülen adaylarda payload'ın tamamı yeniden okunmaz. Sonuç sayfasına giren
  belgeler yine normal payload checksum/schema/key/version doğrulamasıyla
  hydrate edilir. Kaynak doğrulaması ve visibility/tombstone kontrolü atlanmaz.
- Tüm zorunlu posting'ler live kayıtları kaplıyorsa mevcut kesin-count yolu
  kullanılır. Diğer durumda posting kesişimi bütün görünür adayları sayar.
  Yeni bir arama sonucu veya tam metin cache'i eklenmez.

## 10k geniş arama: üç eşleştirilmiş koşu

`covered-text-final-matrix-10k-20261014`: aynı mevcut ilk10k belge, aynı değişmeyen
FDB slotu, her iki varyant release/experimental-builder-budget/record-read.
Üç çift, ikinci çift ters sıra.30 temel case20 sample/proses (60 pooled),8
sayfalama case5 sample/proses (15 pooled); nearest-rank p95. Sıcak OS cache,
manuel cache eviction yok; sınırlı page/hot cache normal olarak evict eder.
Tam total, sıralama, belge ve sürüm oracle'ı geçti. Oracle RAM'i bu testte
resident olduğundan buradan motorun proses RAM'i çıkarılmaz. SQLite bu dar
adımda yeniden ölçülmedi; FDB/cache+SQL DB hash'leri öncesi/sonrası aynı.

**15 p95 düşük /23 yüksek;8 case üç çiftte de hızlı,8 case üç çiftte de yavaş.**
Bu sayılar tüm sorguların hızlandığı anlamına gelmez.

| Query / offset / limit | Önce p95 | Sonra p95 | Hızlı çift |
|---|---:|---:|---:|
| the /0 /50 | 144,97 ms | 2,30 ms | 3/3 |
| man /0 /50 | 147,34 ms | 2,63 ms | 3/3 |
| the /0 /1 | 127,36 ms | 1,51 ms | 3/3 |
| the /0 /200 | 156,55 ms | 9,35 ms | 3/3 |
| the /5000 /50 | 150,83 ms | 3,28 ms | 3/3 |
| the /9950 /50 | 146,26 ms | 2,94 ms | 3/3 |
| he /0 /50 | 141,92 ms | 145,78 ms | 0/3 |
| quixote /0 /50 | 4,66 ms | 5,07 ms | 1/3 |

Önceki iki ileri `the` sayfasının büyük maliyeti bu üç çiftte çözüldü.
Diğer uzun/iki karakterli sorgularda aynı kazanç sağlanmadı. Artan p95 case'leri
rapordaki ham summary'de durur; önceki grouped-read dalgalanma kabulüyle
kapatılmadı. Örneğin `holmes elementary` %10,73 ve `the zx` %6,53 yüksek;
ikisi0/3 hızlı. `QUİXOTE` %17,50 yüksek fakat1/3 çift hızlı. İlk prototipte
fingerprint kontrolü posting lookup sonrasındaydı ve sonuç27/11 idi; son sürümde
boş/erken-result sorgularında da değişim kontrolü önce yapılır. Eski
`covered-text-matrix-*` logları geçmiş kanıttır; son kod sonucu yerine sunulmaz.

## 1000 aktif belge / değişiklik / reopen

`covered-text-final-delta-20261014`: mevcut corpus ilk1000; ters ingest,40 silme ve
yeniden ekleme, reopen;6 query×3 logical state. İki varyant da synced streamed,
delta0/resident document0. Üç paired proses, ikinci çift ters sıra;1 warm+10
sample/query/proses,30 pooled p95. Tam total/ilk50 belge/version oracle'ı geçti.
Oracle belgeleri RAM'de tutar; proses RAM'i burada ölçülmedi.

**17 p95 düşük /1 yüksek.** `the` ve `the man` üç state'in hepsinde3/3 hızlı;
%89,56–93,86 p95 azalması. İlk state `the`14,35→0,92ms,
`the man`14,19→0,87ms. Reopen `istanbul`0,80→1,04ms (%29,78 yüksek,
1/3 hızlı). Diğer iki `istanbul` state'i düşük. İlk prototipin11/7 sonucu
`covered-text-delta-*` loglarında korunur; son sonuç yerine sunulmaz.
Bunlar açık regresyon başlıklarıdır. Küçük aktif veri üzerindeki bütün eski RAM-delta hızına dönüldüğü
iddia edilmez; bu karşılaştırma iki düşük RAM sürümü arasındadır.

## Okuma/normalleştirme kanıtı ve ilk kullanım

`covered-text-profile.jsonl`: son kod + diagnostic-timing; record-read open,
aynı10k ve sıcak OS cache, manuel eviction yok. Süreler instrumented tek proses
örnekleridir; paired performans kapısı değildir. İç içe span'ler toplanmaz.

İlk `the` araması: pending source5→0,10000 source document kontrolü,
API228,03ms/source.verify_total225,29ms. Açılış159,07ms. İlk aramanın kaynak
doğrulaması kaldırılmadı; warm2,30ms sonucu ilk kullanım süresi gibi sunulmaz.
Sonraki örneklerde query doğrulamasındaki payload byte/read sayıları:

| Query / offset | Requested payload byte | Payload read | Örtülen aday |
|---|---:|---:|---:|
| the /0 /limit50 | 0 | 0 | 250 |
| man /0 | 0 | 0 | 9932 |
| the /5000 | 0 | 0 | 10000 |
| he /0 | 267.849.329 | 1104 | 0 |
| quixote /0 | 1.945.325 | 56 | 0 |
| the zx /0 | 267.849.329 | 1104 | 10000 |

Bu byte/read sayaçları adayların contains doğrulamasıdır; sonuç sayfasının
payload hydration I/O'sunu veya source kontrolünü içermez. `the zx`te `the`
kanıtı hazır olsa da `zx` hâlâ bütün uygun içeriği tarar. `he` filter span'ı
131,51ms; iki karakterli yolun ana maliyeti hâlâ bu büyük okumadır.
İndeks kanıtı olmayan uzun parçalar için de aynı sorun kısmen sürer.

## RAM, yazma/bakım ve doğrulama

`covered-text-10k-20261014`: fingerprint kontrol sırası değiştirilmeden önce,
aynı metadata/bellek yapısıyla mevcut ilk10k belge,20 synced500-belge
işlemi. Kontrol ve aday ikisi de release/experimental-builder-budget ve
streamed input; ikinci çift ters sırada. Input CRC/JSON decode, commit/indexing
ve40 belge kontrolü ingest süresinde; son flush/merge ayrıca ölçülür. Her
koşunun dışında tam10k belge/version+4 query ve önceki executable kontrolü.
Proses private/WS20ms örneklenir; phase poll100ms, sıcak OS cache/eviction yok.
Sayaçlar gerçek proses belleği yerine sunulmaz. Medyanlar:

| Ölçüm | Önce | Sonra |
|---|---:|---:|
| Ingest | 32,145 s | 32,781 s |
| Bakım | 1,079 s | 1,006 s |
| Her koşunun ingest+bakım toplamı | 33,224 s | 33,787 s |
| İndeksleme private tepe | 9,05 MiB | 9,17 MiB |
| Bakım private tepe | 7,93 MiB | 8,33 MiB |
| OS peak working set | 12,98 MiB | 12,83 MiB |
| Idle private | 1,64 MiB | 1,64 MiB |
| Retained disk | 1.521.041.630 B | 1.521.041.630 B |

İndeksleme private+118.784 B (~0,11 MiB), bakım+417.792 B (~0,40 MiB),
idle+8.192 B.9MiB seviyesi korundu, RAM'de sıfır artış iddiası yok. Private
bytes toplam fiziksel RAM değildir; WS ayrı verilir. Query oracle bulunan
diğer iki testten RAM çıkarılmadı. Query sırasında tepe bellek bu ingest/idle
ölçümünde ayrıca örneklenmedi.

Toplam2/3 çift düşük fakat medyan %1,69 yüksek; yazma/bakım hızını çözdüğü
iddia edilmez. Kontrol ve adayın disk byte'ları bütün üç koşuda aynı. Live
source/index/WAL621.588.895 B; ready516 B ile621.589.411 B. Source267.900.709 B
WAL45 B'yi zaten içerir; iki kere toplanmaz. Yeni metin kopyası/corpus
üretilmedi; yalnız sabit FDB çalışma alanı sıfırlandı. Son slot10k/seq20/delta0
bırakıldı. Önceki normal reader da bütün10k belge/version+4 query'yi okudu:
`covered-text-normal-reader-verify.jsonl`; bu correctness kontrolü yeni timing
karşılaştırması değildir. SQLite aynı günkü ayrı referansta8,40MiB indexing
private idi; burada yeniden ölçülmedi ve bu koşuyla paired değildir.

Fingerprint kontrolü early-out öncesine taşındıktan sonra son release
artifact'ında ayrıca **tek ingest/idle teyidi** yapıldı:
`covered-text-final-confirmation-10k-20261014`. Paired üç koşu yerine sunulmaz.
İndeksleme private9,19MiB, bakım8,19MiB, OS peak13,32MiB, idle1,65MiB;
ingest34,117s+bakım1,075s. Retained/live disk aynı; bütün10k belge/version ve
4 query hem son executable hem eski normal reader ile tekrar doğrulandı.
Loglar `covered-text-final-{verify,normal-reader-verify}.jsonl`. Son slot10k
olarak yeniden bırakıldı. Query-only düzeltme bir corpus oluşturma veya yeni
kalıcı format değildir.

Yeni ilgili testler: tam Unicode gramı, mixed AND, uzun kelimede ayrı gramların
sahte eşleşmesi, fields izolasyonu, title/text sınırı, silme/güncelleme,
offset/reopen; kaynak izni olmadan covering çalışmaması; kaynak ve seçilmeyen
indeks payload'ı warm search sonrasında değişince reddetme. Bozuk kaynak+
sağlam indeks/address cache testi gerçek `run-*.fdb` kaynağını değiştirir.
0.1 fixture korunur. Son code default/exact-feature/all-features all-targets
suite, default/all-features Clippy ve fmt başarılıdır. Son normal release38-case
correctness teyidi de geçti (`covered-text-final-matrix-check.jsonl`); bu
performance ölçümü değildir. Normal profile binary'leri hazır bırakıldı,
diagnostic build normal read executable'ı olarak bırakılmadı.
Corrected loglar `covered-text-corrected-{tests-feature,tests-all,clippy-all}.log`,
son default suite `covered-text-final-tests-default.log`, default Clippy/fmt
`covered-text-{clippy-default,fmt}.log`.
Son exact-feature suite'te paralel test fixture'larında yalnız timestamp ile
adlandırılan kökün çakışmasına uygun bir failure görüldü: concurrent child'ın
dizinleri test sırasında kayboldu. Engine deadlock'u gibi sunulmadı. Fixture
adına process içi benzersiz AtomicU64 eklendi; iş verisi ve benchmark workspace'i
bu değişiklikten etkilenmez. Başarısız log `covered-text-final-tests-feature.log`
korunur; sonraki exact-feature/all-features tam tekrarlar ve mixed-writer/kill
kontrolleri başarılıdır. `covered-text-fixture-correction-test.log` dar tekrar
da geçti. İlk Clippy test yazımı hatası düzeltildi; failure log'u
`covered-text-clippy-all.log`, son başarılı log corrected-clippy-all'dır.

Dar üç karakterli kazanç opt-in kodda korunur. **Genel hızdan ödün vermeden
default'a geçiş kapısı başarısızdır:**23/38 p95 yüksek ve kapsamlı diğer işler
açıktır. Önceki kullanıcı dalgalanma kabulü bu yeni sonuçlara uygulanmadı.

[OPEN-WORK](OPEN-WORK.md): iki karakterli/uzun içerik taraması, diğer p95
artışları, yazma/bakım, disk/eski nesiller ve eşzamanlı/soğuk yük işleri açık.
