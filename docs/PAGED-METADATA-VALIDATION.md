# Disk metadata uygulaması — 08.10.2026

Kayıt/ID tablolarını RAM'den bırakan yol `experimental-paged-metadata` feature'ı
ile motora bağlandı. **Default build değişmedi. SQLite'tan düşük toplam proses
RAM'i ve bütün iş yüklerinde gerilemesizlik hedefi henüz tamamlanmadı.** Dış
projelere entegrasyon yapılmadı; hazır DB bağımlılığı eklenmedi.

## Uygulama ve bütünlük

SEG03 payload/posting cache'i korunur. Ayrı, sürümlü FDBMET04 metadata sidecar'ı
64 kayıtlık prefix-compressed yapraklar, yaprak başına entry offset tablosu ve
32 yaprağa bir disk dizini kullanır. RAM'de 2048 slota bir root fence bulunur.
Slot erişimi ikili key araması yapmaz; key araması sıkıştırılmış byte parçalarını
geçici String üretmeden karşılaştırır. Tüm anahtar bileşenleri 1..1024 UTF-8
bayt sınırında ve scope/collection/id sırası korunarak desteklenir.

Foreground/merge worker için proses çapında ortak raw-page cache 256 KiB'dir.
Yeni okuma allocation'ından önce eviction yapılır; pinned Arc sayfaları yoktur.
Database başına görünürlük cache'inin slot payload'ı ayrıca 64 KiB / 256 girişle
sınırlıdır. Yeni run veya merge yayımlanırken bu cache temizlenir. Cache'e
sığmayan görünürlük normal key taramasını kullanır; RAM delta her sorguda ayrıca
gölgelenir. Silme bitmap'i posting adayı başına metadata I/O'sunu önler.

Metin doğrulamasında sorguya ait cursor, bir metadata yaprağını (en fazla
256 KiB ek buffer) ve yakın payload penceresini (en fazla 64 KiB) yeniden kullanır.
Her seçilen payload'ın CRC'si ve key/version identity'si doğrulanır; aradaki
seçilmemiş payload parse edilmez. Büyük tekil belgeler ayrı okunur. Derin
offset'te sıralı, görünür ID listeleri diğer run'ın sıradaki anahtarına kadar
ikili aramayla atlanır. Tam sonuç sayısı ve sayfa sırası korunur.

Sidecar source binding, sayfa/cache/footer CRC32, sınır, UTF-8, slot count,
yerleşim ve tüm record identity kontrollerinden geçer. Açılışta bütün metadata
bağımsız doğrulanmış SEG03 diziniyle karşılaştırılır. `.fdb` kaynağı ayrıca
tam replay edilir; sağlam sidecar kaynak bozulmasını gizlemez. `.fdb`/`.ready`
biçimleri ve 0.1 fixture baytları değiştirilmedi. Bozuk/kayıp sidecar sağlam
SEG03'ten yeniden yapılır; eski motor sidecar'ı yok sayıp SEG03'ü kullanabilir.
I/O hataları key/find/version/search/merge üzerinden Result olarak iletilir.

Bu bütçeler **toplam RAM sınırı değildir**: cache yönetimi, root fence, slot
başına bir bit deletion bitmap, token sözlüğü, transient JSON/build/replay,
payload çıktısı ve büyük tekil belge ayrıca yer kaplar. Büyük offset'in tam
u32 ID listesi hâlâ RAM'de tutulur. Root/bitmap boyutu kayıt sayısıyla büyür.
Builder/replay henüz akışa dönüştürülmedi. `disk_index_bytes` artık sidecar'ı
da içerir; bu corpus'ta aktif cache toplamı 777.425.389 → 822.018.384 bayttır.

## Ölçüm koşulları

Corpus `data/scale-v031-optimized-20261008`: bir milyon başlangıç kaydı,
tekrarlı 256 bayt içerik, sonraki update/delete ve sentinel; 15 aktif run,
1.000.100 fiziksel slot, 999.951 görünür belge, sequence 2002. Kaynak/manifest
değiştiren işlem benchmark'ta gönderilmez; ilk experimental açılış türetilmiş
sidecar'ları oluşturmuştur. Son karşılaştırmada sidecar'lar mevcut ve OS cache
sıcaktır. Cold cache testi yapılmadı; working set'i küçültmek için OS trim/mmap
kullanılmadı.

Baseline normal 0.3.1 key hydration binary'sidir; candidate v4, metadata paging,
visibility cache, gruplu slot/text payload ve derin offset atlamasını birlikte
taşır. Dolayısıyla sonuçlar yalnız metadata disk I/O'sunun bağımsız maliyeti
değildir. CPU ölçümünde allocator sayacı yoktur.

## Gerçek proses RAM'i

İki binary aynı sağlıklı DB'de ayrı proseslerle ölçüldü. Baseline üç tekrar,
son v4 üç tekrar; `stats` sonrası bir saniye boşta beklenir. Aşağıdaki değerler
üç koşunun medyanıdır. Peak, Windows prosesinin raporladığı açılış working set
peak'idir; bütün yazma/ingest/query peak'i değildir. Private alanı boşta tek
örnektir, private peak değildir.

| Ölçüm | Baseline | Deneysel v4 |
| --- | ---: | ---: |
| Boşta working set | 59,13 MiB | 11,23 MiB |
| Boşta private | 58,56 MiB | 8,02 MiB |
| Açılış peak working set | 86,57 MiB | 43,69 MiB |

v4 private örnekleri 7,43..9,84 MiB arasında değişti; sabit 8 MiB sınırı iddia
edilmez. Önceki native SQLite çalışmasında working set yaklaşık 9,48 MiB,
sampled private yaklaşık 5,22 MiB idi. SQLite bu aşamada yeniden ölçülmedi ve
bu FDB sayıları SQLite'ın altında değildir. Önceki sayıları güncel, eşlenmiş
RAM karşılaştırması gibi sunmayız.

Ham veriler `benchmarks/fdb-paged-metadata-before-idle-20261008-{1,2,3}.json`
ve `benchmarks/fdb-paged-metadata-v4-idle-20261008-{1,2,3}.json` dosyalarındadır.
v1/v2/v3 denemeleri ayrı tutulur; nihai kodun RAM tablosu v4'tür.

Son binary ile ayrı birer proseslik ek okuma deneyi de yapıldı. Exact, ikinci
sayfa, range, geniş metin ve son sayfa yanıtlarından sonra v4 working set
örnekleri yaklaşık 10,98..11,42 MiB, private örnekleri 7,64..8,27 MiB aralığındaydı.
Bunlar **yanıt sonrası örneklerdir**, sorgu içindeki transient peak değildir;
Windows cumulative WS peak değeri açılış peak'inin altında sıfırlanmaz.
Ingest/yazma RAM'i bu deneyde ölçülmedi.
[Baseline ham veri](benchmarks/fdb-paged-metadata-before-read-memory-20261008.json),
[v4 ham veri](benchmarks/fdb-paged-metadata-v4-read-memory-20261008.json) ve
[ölçüm aracı](../tools/measure-fdb-read-memory.ps1).

## Sorgu süreleri

`compare-fdb-reads.py` beş bağımsız proses çifti açar; baseline/candidate sırası
her turda ters çevrilir. Her sorgu bir kez zamanlama dışında ısıtılır. Exact,
range ve dar başlık yol başına 30 örnek/tur; geniş metin, ikinci/son sayfa
3 örnek/tur kullanır. Limit 50'dir. Total, sequence, sıra, version ve bütün
payload JSON'u her yanıtta baseline ile birebir karşılaştırılmıştır.
**Süreler JSON Lines istemci wall time'ıdır; stdio/JSON serileştirmesi dahildir.**
Test/derleme süreçleri nihai CPU serisi sırasında çalıştırılmadı.

| Sorgu | Baseline medyan p50 | v4 medyan p50 | v4 / baseline |
| --- | ---: | ---: | ---: |
| source=synthetic, ilk sayfa | 1,1583 ms | 1,0056 ms | 0,868 |
| İkinci sayfa, offset 50 | 1,2322 ms | 0,9779 ms | 0,794 |
| day=2..5 | 3,8719 ms | 3,5513 ms | 0,917 |
| rapor-0000500001 başlığı | 11,8757 ms | 11,3636 ms | 0,957 |
| teknoloji, geniş metin | 4905,84 ms | 2555,19 ms | 0,521 |
| Son sayfa, offset 999900 | 259,2401 ms | 22,6117 ms | 0,087 |

Sütunlar tur p50'lerinin medyanları ve bu medyanların oranıdır; bağımsız genel
hız garantisi değildir. İlk sayfa yaklaşık %13, ikinci sayfa %21, geniş metin
%48 iyileşti; son sayfa yaklaşık 11,5 kat hızlandı. Dar başlıktaki küçük fark
genel hız kazancı kanıtı sayılmaz. İlk turun exact p50'si v4'te daha yüksekti;
range tur p95 maksimumu v4 6,1068 ms, baseline 4,9268 ms idi. Bu nedenle
"her sorgu/her kuyruk gecikmesi gerilemesiz" denmez. Geniş metin/offset yolunda
15 örnek/yol p95 garantisi sağlamaz.

Tam JSON eşitliği bütün beş turda geçti. Cold cache, farklı/benzersiz metin
corpus'u, random get, eşzamanlı okuyucular, ingest/flush/merge süreleri ve yazma
RAM peak'i bu aşamada benchmark edilmedi. Karşılaştırma önceki **default**
motora karşıdır; önceki opt-in grouped-only yolun yerine koşulsuz hız kabulü
anlamına gelmez. Bu kapılar ve SQLite RAM hedefi tamamlanmadan feature default
yapılmadı.

Nihai [ham sorgu verisi](benchmarks/fdb-paged-metadata-v4-reads-20261008.jsonl)
ve [beş koşuluk özet](benchmarks/fdb-paged-metadata-v4-reads-20261008.summary.json).
v2/v3 ham ve özet dosyaları ayrı korunur; v3'ün geniş metin gerilemesi v4'teki
cursor/payload penceresinin gerekçesidir ve nihai tabloya karıştırılmaz.

## Requested heap tanısı

`profile_memory` ayrı, opt-in allocation sayacı kullanır. Açılış sonrası Rust
requested live 2.445.630 bayt; requested peak 37.880.100 bayttır. Önceki
51.968.250 bayt live örneğine göre ana resident tablo kaldırıldı. Bu sayaç
allocator overhead/fragmentation, stack, DLL/OS sayfalarını ve proses RAM'ini
ölçmez; instrumented süreler performans kabulü olarak kullanılmaz.

Index capacity raporunda record Vec, ID String ve grup String capacity sıfırdır.
Root capacity 91.560, deletion bitmap capacity 125.048 bayttır; ortak raw-page
cache'in 262.144 bayt bütçesi ve görünürlük cache'i ayrıca yer tutar. Bu seçilmiş
alanlar toplam heap değildir. Exact/text ilk sayfanın ek requested
peak'leri 84.273 / 84.682 bayttır. Offset 5000'in hâlâ tam ID listesi kullanan
yolu ek requested peak 7.400.506 bayt, toplam requested peak 9.848.600 bayt
üretti. Dolayısıyla düşük idle RAM, bütün sorguların aynı RAM'de kalacağı anlamına
gelmez. [Ham heap tanısı](benchmarks/fdb-paged-metadata-v4-heap-20261008.jsonl).

## Test kapıları

- Default `cargo test --all-targets`: 46 test geçti.
- `cargo test --all-targets --all-features`: 56 test geçti.
- Default/all-features Clippy `-D warnings` ve `cargo fmt --check` geçti.
- Sayfa/dizin sınırları, lookup ve ortak cache accounting/eviction, Unicode
  prefix'leri, maksimum uzunluklu/JSON kaçışlı anahtarlar ve boş index doğrulandı.
- Truncated/magic/binding/checksum bozuk sidecar yeniden yapımı ve runtime kısa
  okuma hatası doğrulandı. Malformed leaf, seçilmiş payload CRC, geçerli CRC'ye
  rağmen yanlış key/version, oversized fallback ve okunmuş ama seçilmemiş
  bozuk payload'ın parse edilmemesi test edildi.
- 7.200 kayıtlık iç içe run/delta modeli, derin sayfa ve overflow offset'ler
  yeniden açılıştan önce/sonra tam belge eşitliğiyle karşılaştırıldı.
- Eski kaynak fixture'ı, valid cache + corrupt source, torn frame/manifest,
  acknowledged-commit kill ve merge/publication kill testleri feature build'de
  geçti. Cache build yazısının her syscall noktasında kill enjeksiyonu veya
  elektrik kesintisi testi yapılmadı; böyle bir garanti iddia edilmez.

## Tekrar

```powershell
cargo build --release --bin fdb --example profile_memory --features experimental-paged-metadata
tools/measure-fdb-idle.ps1 -DatabasePath data/scale-v031-optimized-20261008
python tools/compare-fdb-reads.py output/fdb-before-paged-metadata-20261008.exe output/fdb-paged-metadata-v4-20261008.exe data/scale-v031-optimized-20261008
target/release/examples/profile_memory.exe data/scale-v031-optimized-20261008
```

Ölçülen baseline SHA256:
`41EE5A5A01CBB44EC1E7725E89965DDC8238B8726C83D068414F172EAFBA6643`.
Ölçülen experimental v4 SHA256:
`353EFB832545B9AB487C66215B7181FC2E010E6C312541005911945FC3B2E313`.
Requested heap profile binary SHA256:
`07094E1F781A08EACCB2CBD987330AAB9337525B6455F6CA2869DFCF85F5E7F2`.
İş bitiminde yeniden derlenen **default** CLI SHA256:
`9698262B973884B8E65262D1D715AE601C49D0A7300A154E855DAD05E09FFC98`.
Default CLI paging feature'ını açmaz; deneysel binary `output/fdb-paged-metadata-v4-20261008.exe`
olarak ayrı korunmuştur.

## Kalan hedef

Önceki yaklaşık 47,69 MiB record/ID capacity kaldırıldı; ancak genel RAM bütçesi
tamamlanmadı. Sonraki işler token sözlüğü ve root/bitmap bütçesi, source/build
JSON'unun akışla işlenmesi, büyük offset için tam ID listesi olmadan ranked cursor
ve random get/yazma/cold-cache kapılarıdır. SQLite ile güncel, eşlenmiş gerçek
proses ölçümü bu işler tamamlandığında tekrarlanmalıdır.
