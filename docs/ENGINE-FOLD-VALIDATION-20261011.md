# Gerçek motor sorgusunda normalleştirme — 11 Ekim 2026

**Karar: ölçülen sorgu kapısı geçti.** Dokuz sayfanın her biri üç eşleşmiş
çiftte de hızlandı; tepe working set medyanı ve fiziksel disk boyutu gerilemedi.
Bu normalleştirme optimizasyonu default disk sorgu yolunda tutuldu. Codec
değişimi yapılmadı; önceki rkyv payload deneyinin reddedilme kararı değişmedi.

## Sonuçlar

| Sorgu / offset | Baseline ms | Candidate ms | Süre azalması |
|---|---:|---:|---:|
| `the` / 0 | 12.317,50 | 10.953,43 | %11,07 |
| `the` / 50 | 12.600,19 | 11.313,99 | %10,21 |
| `the` / 99.950 | 12.513,84 | 10.322,31 | %17,51 |
| `abbreviation` / 0 | 305,13 | 257,75 | %15,53 |
| `abbreviation` / 50 | 297,32 | 264,88 | %10,91 |
| `of` / 0 | 13.464,28 | 9.874,73 | %26,66 |
| `of` / 50 | 16.035,99 | 10.683,26 | %33,38 |
| `the whale` / 0 | 21.557,34 | 13.368,40 | %37,99 |
| `the whale` / 50 | 21.213,79 | 15.625,16 | %26,34 |

Tepe working set medyanı: 136.577.024 → 135.901.184 byte
(130,25 → 129,61 MiB). Örneklenmiş private bytes medyanı:
138.752.000 → 138.395.648 byte. Farklar küçük; geniş bir bellek iyileştirmesi
iddiası yapılmaz. Açılış medyanı 22.442,83 → 24.104,82 ms; açılış hız kazancı
gösterilmedi ve bu yolun normalleştirme kodu değiştirilmedi. Makine yükü/cache
koşuları etkiledi; kabul için tek örnek yerine her eşleşmiş çift kontrol edildi.

[Ham üç çift, sayfa/oracle kontrolleri ve otomatik kapı özeti](benchmarks/engine-fold-20261011.summary.json).
Kabul kapısı yalnız ölçülen sorgu hızının her çiftte iyileşmesi ve working set
medyanında gerileme olmamasıdır; açılış kazancı veya genel SQLite üstünlüğü
bu kapının sonucu değildir.

## Kapsam ve uygulama

Payload deneyinden sonra aynı optimizasyon gerçek `Database::search` disk sorgu
yoluna taşındı. `src/segment.rs` içindeki iki contains doğrulama döngüsü ortak
`QueryTextScratch` kullanır. Başlık ve gövde için String tamponları tekrar
kullanılır; tamamen ASCII metin topluca küçük harfe çevrilir, karışık UTF-8
metinde ASCII karakterler daha kısa yoldan işlenir. Türkçe harf eşlemeleri ve
Unicode lowercase genişlemeleri eski `index::fold` ile aynıdır.

Başlık bütün terimleri karşılıyorsa gövde normalleştirilmez. Mevcut 256 KiB ham
gövde → normalleştirilmiş gövde cache'i korunur; cache başlığı paylaşmaz. İki
scratch tamponunun her biri sonraki aday başında 128 KiB'den büyük kapasitesini
bırakır. Büyük tek belge geçici olarak daha büyük tampon gerektirebilir; bunlar
toplam proses RAM sınırı değildir. Sorgu sonunda bütün scratch/cache bırakılır.

Delta zaten normalleştirilmiş metin kullanır. Yazma, gram oluşturma ve sorgu
terimlerinin hazırlanmasındaki `index::fold` değişmedi. JSON çözümleme, kaynak
CRC/kimlik kontrolü, aday seçimi, silinmiş kayıtların gizlenmesi, toplam sayma,
sıralama ve sayfa belgelerinin yüklenmesi aynı motor yolundadır. Yeni feature,
kalıcı format veya WAL/manifest/protokol değişikliği yoktur. rkyv codec hâlâ
default kapalıdır ve motor tarafından çağrılmaz.

## Corpus ve ölçüm koşulları

100.000 benzersiz SHA-256 gövdeli, 2–50 KiB UTF-8 pasaj; toplam gövde
2.655.160.287 byte. Seed 20261009. Sekiz Gutenberg kitabından gerçek metin
pencereleri kullanıldı; pencereler örtüşebilir, bunlar 100.000 bağımsız eser
değildir. İngilizce ağırlıklıdır. Türkçe/Unicode doğruluğu ayrı testlerle
kontrol edilir; bu corpus Türkçe performans iddiası oluşturmaz.
[Corpus kaynakları ve hash'leri](benchmarks/rkyv-corpus-20261009.provenance.json).

Yeni, izole `data/engine-fold-v2-20261011` kaynağı eski `FDBLOG01` JSON formatıyla
üretildi; gerçek motor bunu açıp tek `FDBSEG03` disk indeksini oluşturdu. 200
checkpoint frame'inin sequence değeri 200; belge sürümleri 1–200 korunur.
Ölçümde 100.000 belge, bir disk segmenti ve sıfır delta/resident belge vardır.
Bu fixture üretimi normal ingestion/flush/merge performansı ölçümü değildir.

İlk fixture üretiminde checkpoint sınırının sequence'leri yanlış verilmişti;
motor `invalid transaction sequence` ile reddetti. Üretici düzeltilip ayrı v2
dizininde kaynak oluşturuldu. İlk başarısız dizin ve günlükler korundu; kabul
sonuçlarında kullanılmaz. Hazır indeksin ilk üretimi 348.880 ms sürdü; bu süre
normal yeniden açılış ölçümüne dahil değildir.

Sorgular `the` (100.000), `abbreviation` (264), `of` (100.000), AND `the whale`
(16.537). Her sorguda offset 0 ve 50; ayrıca `the` için offset 99.950 ölçülür.
Her çağrıda bağımsız oracle ile toplam, 50 belgenin tamamı, sıra, key, fields,
links, version ve page sequence doğrulanır. Ölçüm public `Database::search`
çağrısını kapsar; hazır aday listesi veya payload-only tarama değildir.

Üç ayrı eşleşmiş proses çifti; sıra baseline/candidate, candidate/baseline,
baseline/candidate. Aynı DB, release binary, default feature kümesi; diagnostic
timing kapalı. Açılış tam kaynak doğrulaması ve indeks yüklemeyi içerir. Sorgu
cache'i sıcaktır: açılıştan sonra ilk sayfaya bir ölçümsüz sorgu, sonraki
sayfalara önceki sorgular etki eder. Zorlanmış OS cache boşaltma yapılmadı.
Bütün dosyaların OS cache'inde kaldığı ayrıca doğrulanmadı.
Uzun sorgular sayfa başına proses başına bir örnek, dar sorgu on örnek kullanır;
raporlanan değer üç proses medyanıdır. Güvenilir p95/p99 iddiası yoktur.

Working set Windows proses high-water sayacıdır; private bytes yaklaşık 20 ms
aralıkla örneklenir. Pencere açılış + corpus planı/kitaplar/oracle + bütün
sorguları kapsar; OS dosya cache'i dahil değildir. Motorun tek başına heap
kullanımı olarak sunulmaz.

## Fiziksel dosyalar

Kaynak `.fdb`: 2.741.774.867 byte; `.ready`: 28 byte; türetilmiş `.idx`:
3.551.235.145 byte. `stats.source_bytes` bu eski kaynak düzeninde aynı dosyayı
aktif ve kaynak olarak iki kez sayar; fiziksel disk hesabında kullanılmaz.
[Ölçüm öncesi boyut/hash kaydı](benchmarks/engine-fold-files-20261011.before.json).
[Ölçüm ve profiller sonrası kayıt](benchmarks/engine-fold-files-20261011.after.json)
ile üç dosyanın boyutları ve SHA-256 değerleri birebir aynıdır.

## Ayrı diagnostic profil

Üç çiftlik ana ölçüm tamamlandıktan sonra iki ayrı `diagnostic-timing` binary
ile `common` sorgusu seri çalıştırıldı. Aşağıdaki değerler ilk sayfanın
ölçümsüz ısınma sonrası çağrısından alınır; her sürümde bir örnektir. Profilin
ek sayaç maliyeti vardır; bu değerler kabul tablosuyla birleştirilmez.

| Span | Baseline ms | Candidate ms | Çağrı sayısı |
|---|---:|---:|---:|
| `query.fold.body` | 10.009,55 | 3.765,77 | 100.000 |
| `query.fold.title` | 46,93 | 10,51 | 100.000 |
| `payload.read_document` | 6.528,79 | 6.521,28 | 100.050 |
| `payload.decode_crc_identity` | 5.056,78 | 5.095,58 | 100.050 |
| `disk.read_at` | 1.323,45 | 1.268,98 | 100.148 |
| `search.total` | 16.797,63 | 10.478,39 | 1 |

Gövde fold süresi bu profilde %62,38 azaldı. Yeni yolda belirgin kalan maliyet
payload okuma/doğrulamasıdır; JSON çözümleme+CRC+kimlik kontrolü aynı span'da
olduğu için yalnız JSON'a 5,10 saniye atfedilmez. `read_document` diğer payload
span'larını, `search.total` alt sorgu span'larını kapsar; tablo toplanmaz.
Alt dize `contains` taraması gerçek motor profilinde ayrı span değildir;
payload deneyindeki tarama ölçümünden motor için süre uydurulmaz. Sayfalama
yolunda 100.000 adayı doğrulamak ve toplamı saymak hâlâ pahalıdır.

[Baseline profil](benchmarks/engine-fold-profile-20261011-baseline.jsonl),
[candidate profil](benchmarks/engine-fold-profile-20261011-candidate.jsonl).

Sonraki dar araştırma alanı aynı JSON kaynaktan aday doğrulaması için yalnız
key/version/title/text alanlarını okumanın ölçülmesidir; fields/links ve tam
belgeyi yalnız sonuç sayfasında oluşturmak. Bu raporda uygulanmadı; format
değişimi veya CRC/kimlik kontrolünü kaldırma gerektirmez.

## Doğrulama

- Son runtime değişikliğinden sonra `cargo test`: 51 test geçti.
  [Tam default sonuçları](benchmarks/engine-fold-default-final-tests-20261011.log).
- `cargo test --all-features --test disk --test engine --test segments --test rkyv_payload`:
  52 test geçti, iki alt süreç helper'ı doğrudan çalıştırılmadı (parent testleri
  tarafından çağrılır). [Feature sonuçları](benchmarks/engine-fold-feature-tests-20261011.log).
- Yeni test eski fold ile bütün geçerli Unicode scalar'larını ve tamponun daha
  kısa metinle tekrar kullanımını karşılaştırır. Cache testinde ortak gövdenin
  başlıkları karıştırmaması, büyük gövdenin cache bütçesini aşmaması ve sonraki
  kullanımda büyük scratch kapasitesinin bırakılması doğrulanır.
- Mevcut disk/delta, çok segment, Türkçe, AND, sayfalama, update/delete testleri
  geçti. Kesilmiş işlem/kaynak, tek byte bozulması, process kill ve sağlam cache
  yanında bozuk kaynak reddi korunur. Eski 0.1 JSON fixture yeniden yazılmadan açılır.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` ve
  `cargo clippy --all-targets --all-features -- -D warnings` geçti.
- `tests/engine.rs` ve 0.1 fixture için git diff boş. Dış projeler değiştirilmedi.

## Tekrarlama

`examples/measure_engine_fold.rs` her iki binary için aynıdır. Baseline eski
disk fold döngüsü, candidate yeni scratch döngüsüdür. Binary'ler `output/`
altında donduruldu; sonraki derlemeler onları değiştirmez.

SHA-256:

- baseline: `FBEE009CFF59D43DEFF12B8E01E9195D264154662A9FF194AE05E2D3D969C2B4`
- candidate: `FBBA50206524F46A39C93B9C35E62955906D179E100853F5011EF6C2077101B0`
- baseline diagnostic: `22B21640B04E239F7EC5C73B206D56118A4FE7425CFB6459695F488CA80DDA28`
- candidate diagnostic: `563A31237FEA9E8CDEF9653E2108E08670C2C10A9EB20195A09B076F66560DB6`

```powershell
# Kaynak zaten varsa üretici yeni dizin gerektirir; mevcut dosyayı ezmez.
python tools/prepare-engine-passages.py
./output/fdb-engine-fold-baseline-20261011.exe data/engine-fold-v2-20261011 prepare
./tools/measure-engine-fold.ps1 -Prefix yeni-engine-fold
python tools/summarize-engine-fold.py docs/benchmarks/yeni-engine-fold
```

Farklı çıktı prefix'i kullanılır. Proses çiftleri seri çalışır; ölçüm sırasında
derleme, test, profil veya büyük dosya hash taraması çalıştırılmaz. Ayrı
diagnostic binary'ler attribution için kullanılır; profilli süreler kabul
kapısına katılmaz. İç içe zaman sayaçları toplanmaz.

Ölçülmeyenler: soğuk disk I/O, çok segment/delta, concurrent okuma/yazma,
ingestion/merge, Türkçe ağırlıklı performans, SQLite karşılaştırması. Kazanç
yalnız bu gerçek motor fixture'ı ve sorgu seti için değerlendirilecektir.
