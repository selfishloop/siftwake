# Sonuç tamponlu V3 motorunun etkinleştirilmesi — 18 Ekim 2026

Kullanıcı, genel performans kabulü geçmeyen prototipin motora alınmasını istedi.
Arşivdeki V3 algoritması normal `bounded-engine` profiline alındı. Bu bir
kullanıcı geçiş kararıdır; önceki arama/RAM kabul kapılarının geçtiği anlamına
gelmez. Önceki ölçümler ve reddetme kararı tarihçe olarak korunur.

## Kullanım

Normal derlemede `Database::search_into(&Query, &mut SearchPage)` kullanılabilir.
Çağıran aynı `SearchPage` nesnesini tekrar verir; uygun metin String'lerinin
belleği yeni sonuçlar için kullanılır. Normal `search` her çağrıda sahipli yeni
sayfa döndürmeye devam eder; o çağrı otomatik bir sorgular arası tampon tutmaz.
CLI/protokol ve dış projeler değiştirilmedi.

```rust
use fdb::SearchPage;

let mut page = SearchPage { total: 0, sequence: 0, hits: Vec::new() };
db.search_into(&query, &mut page)?;
// Sonuçları &page.hits üzerinden oku; sonra aynı page'i geri ver.
db.search_into(&query, &mut page)?;
```

İlk çağrı/boş tampon, limit=1, doğrudan okuma ve mevcut checked-payload capture
sorguları normal aramayı kullanır. Diğer gruplu sonuçlarda eski key/title,
fields ve links filtrelemeden önce bırakılır; yalnız en fazla yeni limit kadar
metin taşınır (query limiti1000). Kapasitesi yeni metnin byte uzunluğuyla tam
aynı olan String temizlenip güncel doğrulanmış metinle doldurulur. Diğer metinler
normal decode edilir. Sayfa dışı tamponlar çağrı sonunda bırakılır. Bu sınırlar
toplam proses RAM bütçesi değildir.

Hata eski sonucu da tüketir; page total=0/sequence=0/hits=[] olur. Başarıda
key/title/text/fields/links/version güncel kaynaktan gelir. Eski metin içeriğine
veya önceki sürüme güvenilmez. Kaynak, CRC, schema/UTF-8 ve key/version kontrolleri
korunur. Kalıcı format, Cargo bağımlılıkları ve feature listesi değişmedi.
`--no-default-features` referans profili eski `search` API'sini korur.

## Önceki ölçümün kapsamı ve açık kalan farklar

Aynı mevcut ilk10k corpus ve sabit çalışma alanında, sıcak OS cache ve tam
doğrulamayla normal release V3: yalnız200 sonuçlu aynı sayfanın tekrarında p95
6,6529→4,0728ms (-%38,78), 4/4 süreç çifti ve16/16 blok hızlı. Bu kazanç
önceki sonucu geri veren çağıran içindir; bütün sorgulara veya ilk kullanıma
genellenmez. Değişen dört sayfa düzeninde the200 -%10,18, ishmael60/200 -%5,77;
tek sonuç p95 +%10,68 açık. Geniş38'de the200 p95 yaklaşık aynıydı (+%0,01).

Eski normal okuyucu kontrolünün yoğun tekrarında the200 p95 +%10,01 kaldı.
Oracle'sız OS private medyanı +28KiB/working set +48KiB; RAM kazancı yoktur.
Bu farklar geçiş yapıldı diye kapatılmaz. İlk kullanımda düzenli kazanç
kanıtlanmadı. [Bütün önceki sonuçlar ve koşullar](RESULT-BUFFERS-10K-20261018.md).

Bu geçişte yeni geniş hız/OS RAM A/B koşusu yapılmıyor; önceki sonuçlar yeni
ölçüm gibi sunulmaz. Kod önceki V3 yamasından alındı; yalnız testte aynı byte
uzunluğundaki değişen UTF-8 içerik/title/field kontrolü güçlendirildi.

## Geçiş doğrulamaları

- Normal profil tam test paketi: **191 geçti, 0 başarısız, 7 ignored**.
  Referans profil engine testleri: 17 geçti. Tüm feature'larla değişen UTF-8
  metin, gruplu okuma/bozulma, arama semantiği ve 0.1 fixture kontrolleri geçti.
  `cargo fmt --check` ve normal/tüm-feature/referans profillerinde
  `cargo clippy --all-targets -- -D warnings` başarılı.
  [Komutlar ve çıkış kodları](benchmarks/activate-buffers-20261018-checks.json),
  [tam test sayıları](benchmarks/activate-buffers-20261018-test-counts.json).
- Normal release derlemesinde `search` ve `search_into` aynı **38 sorgunun
  tamamında** bağımsız payload oracle'ıyla tam belge/sıra/sürüm kontrolünü geçti.
  Normal CLI'da stats ve 200 sonuçlu arama da geçti. Bunlar işlev kontrolleridir;
  tek tekrar sürelerinden yeni hız sonucu çıkarılmadı.
  [Arama doğrulamaları](benchmarks/activate-buffers-20261018-oracles.json),
  [CLI kontrolü](benchmarks/activate-buffers-20261018-cli.json).
- Ayrı diagnostic-timing derlemesinde aynı 200 sonuçlu sayfanın ikinci çağrısı
  **200 metin tamponunu** yeniden kullandı. Normal ve reuse yollarında aynı
  115 sonuç okuması, 200 CRC, 200 schema/UTF-8 ve 200 kimlik kontrolü yapıldı.
  Altı adımda tam belgeler doğrulandı; son boş sayfada metin ve sonuç kapasitesi
  sıfır. Bu sayaç kontrolü yeni latency/RAM kabul ölçümü değildir.
  [Sayaç kanıtı](benchmarks/activate-buffers-20261018-reuse-proof.json).
- Kaynak/indeks/SQLite/0.1 fixture içeren **21 dosyanın SHA-256 değeri aynı**;
  bütün corpus SHA-256 değeri de korundu. FDB 349.805.698 B, SQLite
  583.860.224 B. Cargo dosyaları, fixture ve CLI kaynakları değişmedi.
  [Geçiş ve değişmezlik kanıtı](benchmarks/activate-buffers-20261018-proof.json).

Yeni corpus üretilmedi veya indirilmedi. Mevcut
`data/rkyv-corpus-20261009/json.payload-bench` dosyasının aynı ilk10.000 belgesi,
`data/benchmark-work/fdb` ve `data/benchmark-work/sqlite` kullanılır. Okuma
kontrollerinde reset/repack/hazırlama/DB kopyası yoktur. SQLite bu tur yalnız
değişmezlik kontrolündedir; yeni SQLite hız ölçümü yapılmadı.

Yazma/bakım, tek sonuç/Harpoon/genel arama p95'i, normal Vec ingest/query/caller
RAM'i, açılış/disk sıkıştırma ve eşzamanlı/gerçek-soğuk yük açık.
[Açık işler](OPEN-WORK.md).
