# Offset payload: ölçülen bağımsız prototip

Karar: **payload düzeyindeki kabul kapısı geçti**. Default kapalıdır; motorun
kalıcı JSON formatına uygulanmadı. Rkyv deneyi için önceki **reddedildi** kararı
geçerlidir. Bu yeni codec rkyv kodlamasını kullanmaz ve yeni feature eklemez;
mevcut `experimental-rkyv-payload` deney altyapısını paylaşır.

## Tasarım ve güvenlik

`src/rkyv_payload/binary.rs` içindeki `OFF1`, 44 byte little-endian başlıkta
sürüm ve ardışık bölge offset'lerini saklar: scope, collection, id, title,
text, fields, links ve kayıt sonu. UTF-8 metinler sıkıştırılmadan doğrudan
saklanır. Fields null/bool/i64/u64/f64/string/list/map etiketleriyle, links
uzunluklu anahtar parçalarıyla kodlanır. JSON sayı türü, u64::MAX ve -0.0
dahil korunur. Payload içinde JSON metni veya rkyv arşivi bulunmaz.

`checked` güvenli slice erişimiyle offset sırasını, boyutları, UTF-8'i, bütün
metadata gramerini, benzersiz/sıralı map anahtarlarını, sonlu float değerlerini
ve 128 derinlik sınırını kontrol eder. Tam alan ağacı/belge oluşturmaz.
Başlık/metin/key byte tamponundan ödünç alınır. Metadata tüm adaylarda
doğrulanır; yalnız ilk 50 sonuçta owned alanlar/links ve tam belge üretilir.
Contains normalleştirmesi halen ayrı String tamponlarına kopyalar; bütün
sorgu zero-copy değildir.

`PLOADB01` atılabilir benchmark taşıyıcısına codec etiketi 3 eklendi. JSON=1,
rkyv=2 yolları korunur. Taşıyıcı header/record CRC, ordinal, boyut sınırı ve
tüm payload CRC'sini doğrular. Açılış tüm belgeleri doğrular; her sorgu
okumasında CRC, yapısal kontrol ve key/version kimlik kontrolü tekrarlanır.
CRC ölçümü yapısal doğrulama süresinden ayrıdır. CRC rastlantısal bozulma
kontrolüdür, kriptografik kimlik doğrulama değildir.

Yeni çıktı `data/rkyv-corpus-20261009/binary.payload-bench`; create_new ile
oluşturuldu. Kaynak JSON export'u korundu. `.fdb`, `.ready`, `.idx`, WAL,
manifest, dizin/protokol ve default Database yolları değişmedi. 0.1 fixture'ına
dokunulmadı; SDB/Fundops entegrasyonu yapılmadı.

## Corpus ve protokol

- Aynı 100.000 benzersiz pasaj: 2.048–51.200 byte, toplam gövde
  2.655.160.287 byte, seed 20261009. Sekiz Gutenberg eserinin pencereleri;
  pasaj hash'leri benzersiz, pencereler örtüşebilir. 100k bağımsız eser değildir.
  İngilizce ağırlıklıdır; Türkçe/Unicode doğruluğu ayrıca test edilir.
- [Kaynaklar, hash'ler ve corpus planı](benchmarks/rkyv-corpus-20261009.provenance.json).
  Üretimde tüm 100k ikili belge tekrar tam belgeye çözülerek JSON ile eşitliği
  doğrulandı; bu hazırlık süreleri kabul ölçümüne dahil edilmedi.
- Her codec aynı hazır candidate listelerini, reusable ASCII/Unicode fold'u,
  contains semantiğini, tam toplamı ve sıralı ilk 50 tam belgeyi kullanır.
  Bu bir `Database::search` veya SQLite ölçümü değildir; posting/index
  üretimi, aday üretimi, motor metadata'sı ve delta/merge kapsam dışıdır.
- Altı ayrı native proses, üç eşleşmiş çift. Gerçek sıra: JSON/binary,
  binary/JSON, binary/JSON. Son çiftin de ters sırada olması ölçüm scriptindeki
  PowerShell büyük/küçük harf duyarsız değişken alias'ından kaynaklandı;
  scriptte `$codecOrder` ile düzeltildi. Ham kayıtlar korundu; koşular
  seçilmedi/çıkarılmadı. Sonuçlar bu gerçek protokol içindir.
- Her sorguda bir untimed warmup. Büyük sorgularda proses başına bir timed
  örnek, dar sorguda 20 timed örneğin p50'si; tabloda üç proses p50 medyanı.
  Cache: tam açılış doğrulaması ve warmup sonrası OS cache, zorla eviction yok.
  Tamamen RAM'de sıcak cache ve soğuk disk performansı garanti/ölçülmedi.
- Tek frozen release executable her iki codec için kullanıldı; ana koşular
  sırasında build/test/hash/profil çalışmadı. Önceki uzun SQLite raporundaki
  Windows/Ryzen makinesi, Rust release thin-LTO/codegen-units=1.
- Memory penceresi bütün native proses: oracle/candidate listeleri, açılış
  doğrulaması, sorgular ve tam sayfa oracle'ı. PeakWorkingSet64 20 ms arayla
  gözlendi; private bellek maksimum örneklenen değerdir. OS file cache dahil
  değildir; bu ölçüm tüm DB motorunun RAM'i veya heap sayacı değildir.

## Ana sonuçlar

| Sorgu | Aday / toplam | JSON medyan | Binary medyan | Üç çiftte süre azalması |
|---|---:|---:|---:|---:|
| `the` | 100000 / 100000 | 10.925,67 ms | 7.508,88 ms | %27,97–31,33 |
| `abbreviation` | 529 / 264 | 173,78 ms | 159,32 ms | %2,87–17,89 |
| `of` | 100000 / 100000 | 10.685,61 ms | 7.730,88 ms | %24,09–29,10 |
| `the whale` AND | 75664 / 16537 | 9.168,10 ms | 6.621,31 ms | %25,25–35,81 |

Her sorgu her eşleşmiş çiftte hızlandı: 12/12. Dar sorgudaki küçük fark için
genel/soğuk cache garantisi çıkarılmaz. Bütün tekrarların tam toplamı ve ilk
50 belge/key/version/alan/link oracle'ı eşleşti.

| Ölçü | JSON | Binary |
|---|---:|---:|
| Disk, byte | 2.740.862.279 | 2.686.823.501 |
| Tam doğrulamalı açılış medyanı | 6.196,05 ms | 3.085,05 ms |
| Proses tepe working set medyanı | 16,51 MiB | 12,41 MiB |
| Örneklenen private bellek medyanı | 13,41 MiB | 9,19 MiB |

Disk yaklaşık %1,97, working set yaklaşık %24,82 azaldı. Uzun ham metin
byte'ları değişmediği için disk kazancı sınırlı. Önceki gerçek FDB/SQLite
working set sonuçları (130,22/28,38 MiB) başka kapsamdır; motorun 12 MiB'a
indiği veya SQLite'ı bu bellekte geçtiği sonucu çıkarılamaz.

## Profil ve sınırlar

Ana ölçümden sonra ayrı iki profilli proses çalıştırıldı. CRC, I/O, JSON
decode/ikili validation, normalleştirme, contains ve sonuç çözme ayrı span'ler
olarak kaydedilir. Enstrümantasyonlu süreler ana kabul kapısına dahil değildir.

Aşağıdakiler her profilli prosesin warmup sonrası `repeat=1` süreleridir (ms).
Validation kolonları key/version kontrolünü de içerir; I/O kolonu read/seek
süresidir, fiziksel disk I/O sayacı değildir. JSON ilk 50 sonuç için zaten
çözülmüş belgeyi kullanır, bu yüzden ayrıca page_decode süresi sıfırdır.

| Sorgu | JSON decode/kimlik | Binary doğrulama/kimlik | Normalizasyon JSON / binary | Contains JSON / binary |
|---|---:|---:|---:|---:|
| `the` | 3.863,86 | 886,11 | 3.883,50 / 3.828,32 | 29,92 / 23,21 |
| `abbreviation` | 28,67 | 7,65 | 24,43 / 26,19 | 1,65 / 1,65 |
| `of` | 3.812,89 | 902,49 | 3.914,43 / 3.945,74 | 26,42 / 22,48 |
| `the whale` | 3.127,78 | 725,68 | 3.270,38 / 3.243,98 | 184,01 / 177,28 |

| Sorgu | CRC JSON / binary | I/O JSON / binary | Binary ilk 50 belge çözme |
|---|---:|---:|---:|
| `the` | 1.872,74 / 1.866,36 | 1.063,72 / 944,43 | 0,91 |
| `abbreviation` | 12,72 / 12,81 | 101,85 / 102,95 | 2,09 |
| `of` | 1.934,64 / 1.880,63 | 1.053,82 / 967,40 | 0,90 |
| `the whale` | 1.638,86 / 1.600,91 | 1.006,88 / 969,17 | 1,68 |

Kazanç çoğunlukla JSON parse/owned belge oluşturma maliyetinin azalmasından
gelir. Binary doğrulaması da sıfır maliyetli değildir: gövde UTF-8 taraması,
metadata grameri ve kimlik kontrolü sürer. Normalizasyon iki yolda aynı işi
halen yapar; CRC bütün kayıt byte'larını tarar. Dar sorguda I/O span'i baskındır.
Yalnız bir profilli proses/codec olduğundan span farkları nedensel kesinlik
veya istatistiksel tekrarlanabilirlik iddiası değildir.

[JSON ham profil](benchmarks/binary-payload-20261011-profile-json.jsonl),
[binary ham profil](benchmarks/binary-payload-20261011-profile-binary.jsonl).

## Doğrulama

- Tüm 100k kayıt tam roundtrip; typed değerler, Unicode/Türkçe, boş list/map,
  i64/u64 sınırları ve -0.0 ayrıca test edilir.
- Her byte kesilmesi ve tek byte bozulması JSON/rkyv/binary export'larında
  reddedilir. Açılıştan sonra bozulmuş payload CRC'si de reddedilir.
- Hatalı offset, UTF-8, count, alan etiketi, nonfinite float ve aşırı derinlik
  reddedilir; text view gerçekten giriş tamponunu işaret eder.
- Child process sync sonrası öldürülür: tam export okunur, eksik export
  reddedilir; üç codec aynı testi çalıştırır. Bu atılabilir taşıyıcı testidir;
  yeni bir kalıcı FDB kurtarma sözleşmesi değildir.
- Eski JSON 0.1 fixture açılır ve sorgulanır; fixture kaynak byte'ları korunur.
- `cargo test --features experimental-rkyv-payload`: 59 test geçti, 0 hata.
  Bir child helper standalone olarak ignored; parent process-kill testi onu
  ayrıca çalıştırdı. Son eklenen borrowed Unicode/JSON byte eşitliği kontrolü
  hedefli testte tekrar geçti.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` ve feature
  açık aynı clippy komutu geçti. `tests/engine.rs` ve fixture diff'i boş.
  [Test kaydı](benchmarks/binary-payload-20261011-tests.log),
  [son binary testi](benchmarks/binary-payload-20261011-binary-test.log),
  [default clippy](benchmarks/binary-payload-20261011-clippy-default.log),
  [feature clippy](benchmarks/binary-payload-20261011-clippy-feature.log).

Kaynak JSON SHA-256 önce/sonra aynı:
`11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B`.
[Önce](benchmarks/binary-payload-20261011-source-before.json),
[sonra](benchmarks/binary-payload-20261011-source-after.json).
Yeni binary çıktı SHA-256:
`FC124121688FBA045F825F29AAFD51C29A97CA89535361E252FE82E4EB8F690E`.
[Hash kaydı](benchmarks/binary-payload-20261011-binary.json).

Sonraki [gerçek `Database::search` ve bütün motor ölçümü](ENGINE-BINARY-VALIDATION-20261011.md)
tamamlandı: sorgu kazancı var, toplam RAM kazancı gösterilmedi ve ek disk
maliyeti nedeniyle geçici motor deneyi default için reddedildi. Kalıcı biçime geçiş ancak ayrı sürüm, eski JSON kaynak
uyumluluğu ve gerçek motor kurtarma/bozulma testleriyle değerlendirilir.

## Tekrar üretim

```powershell
cargo build --release --example measure_rkyv_payload --features experimental-rkyv-payload
# Yeni isimle freeze et; eski executable/output dosyalarını ezme.
# Corpus ve binary export hazır; aşağıdaki üretim komutunu mevcut export varsa atla.
# generate-binary aynı isimli mevcut export dosyasını ezmez.
# output/fdb-binary-payload-20261011.exe generate-binary data/rkyv-corpus-20261009 reuse
tools/measure-payload-codecs.ps1 -Prefix <yeni-prefix> -Executable output/fdb-binary-payload-20261011.exe -Strategies reuse -Codecs json,binary -Runs 3
python tools/summarize-payload-codecs.py docs/benchmarks/<yeni-prefix> binary reuse
output/fdb-binary-payload-20261011.exe profile-json data/rkyv-corpus-20261009 reuse
output/fdb-binary-payload-20261011.exe profile-binary data/rkyv-corpus-20261009 reuse
```

[Ham sonuçlar, bellek kayıtları ve kapılar](benchmarks/binary-payload-20261011-summary.json).
[Executable SHA-256](benchmarks/binary-payload-20261011-executable.json):
`EAE347A6A273AAA19FF37907274EDEC943E87A182E8B74C0AC2FECCE722BD1EC`.
Yazma throughput'u, kalıcı format açılışı/kurtarması, pagination,
gerçek `Database::search`, çok segment/delta, compression ve SQLite
karşılaştırması bu prototipte ölçülmedi.
