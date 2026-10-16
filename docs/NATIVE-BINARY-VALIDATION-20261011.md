# Doğrudan kalıcı ikili payload — 2026-10-11

Bu belge uygulama/kurtarma doğrulamasıdır. Sonraki
[genel performans testi](NATIVE-GENERAL-PERFORMANCE-20261011.md) tamamlandı:
iki 100k corpus, 12 process, uzun aramalarda 27/27 kazanç; yazma ve RAM'de
kalan sınırlar ayrı raporlandı. Aşağıdaki "henüz ölçülmedi" ifadesi uygulama
doğrulamasının kendi kapsamını anlatır.

Kullanıcı üretim öncesi eski JSON veri taşıma/uyumluluk gereksinimini kaldırdı
ve ikili formatın gerçek depolamaya alınmasını istedi. Yeni veritabanları
default build dahil ikili payload yazar. rkyv dependency/feature gerekmez;
codec kendi OFF1 offset/typed metadata kodlamamızdır. Mevcut deneysel feature
sayısı artırılmadı, benchmark binary modülü aynı native codec'i yeniden kullanır.

## Format ve yollar

- Yeni `.fdb`: `FDBLOG04`. Yeni transaction frame: `TXN2`, mevcut 24-byte
  sequence/length/payload-CRC/header-CRC framing ve `FDBDONE1` footer.
- TXN2 payload: checkpoint byte (0/1), little-endian u32 change sayısı;
  her change bir tag (put=0/delete=1), u32 length ve entry byte'larıdır.
  Put entry OFF1 belge, delete entry üç length-prefixed UTF-8 key parçasıdır.
  Frame limiti 64 MiB; tam frame doğrulanmadan WAL batch uygulanmaz.
- Yeni `.idx`: `FDBSEG04` / `FDBEND04`. Belge byte'ları OFF1; posting,
  directory yapısı ve kaynak binding kuralları korunur. Directory halen JSON'dur.
  `.ready` manifest formatı/protokol değişmedi. Bütün dosya tamamen ikiliye
  geçirilmiş değildir; bu değişiklik belge/transaction payload'ını kapsar.
- OFF1: 44-byte header, version u64, sekiz u32 offset; scope/collection/id/
  title/text raw UTF-8, fields null/bool/i64/u64/f64/string/list/map etiketleri,
  links üçlü length-prefixed UTF-8. u64::MAX ve -0.0 korunur. Metin sıkıştırılmaz.
- Contains adayları aynı posting/filter/visibility/count/page yolundan gelir.
  Her yeni format adayında positional read, CRC, offset/UTF-8/metadata grammar
  ve key/version doğrulaması yapılır; field tree veya tam Document oluşturulmaz.
  Query içindeki byte tamponu ve fold tamponları tekrar kullanılır. Tam belge
  get/sonuç hidrasyonu/merge için çözülür. UTF-8, CRC ve normalleştirme taraması sürer.
- WAL, flush/checkpoint, merge ve türetilmiş cache üretimi aynı native codec'i
  kullanır. Default transaction encoder tek frame Vec'ine yazar. Boyut kontrolü
  JSON üretmeden binary encoded size hesaplar; belge limiti binary 4 MiB'dir.
- Deneysel streaming checkpoint writer kayıt kayıt ikili entry yazar, sayıyı
  patch edip CRC'yi 64 KiB bloklarla hesaplar; I/O lease korunur. Büyük tek
  entry geçici buffer'ı ortak lease dışındadır, toplam RAM garantisi değildir.
  Streaming source open bütün frame CRC/footer'ını önce doğrular, sonra tek
  change scratch'iyle emit eder; bütün frame DOM'unu tutmaz. Normal WAL replay
  atomik Batch'i bellekte kurmaya devam eder.
- WAL-delta artık yeni binary entry konumlarını bulur; seçilen belge CRC ve
  identity kontrolüyle okunur. Grouped/paged/reused-posting yolları yeni payload
  ile çalışır; yeni format contains borrowed yolunu kullanır.

Ek `.payload-bench` bağlama veya JSON+binary benchmark çift kopyası yeni
veritabanı için gerekmez. Ancak mevcut mimari payload'ı hem authoritative
`.fdb` içinde hem türetilmiş `.idx` içinde tutmaya devam eder; **payload'ın
diskte tek kopyaya indirilmesi yapılmadı**. Source-offset index/kompresyon
ayrı alan optimizasyonudur. Metadata, public fields Value ve protokol JSON
olarak kalabilir; binary disk codec'i bunları RAM'de değiştirmez.

Mevcut eski parser/fixture testleri kaldırılmadı; yeni migration/export veya
JSON yazma modu eklenmedi. Eski kaynak/cache mevcut reader ile okunabilir;
bu artık ürün için bir kabul şartı değildir. `FDBLOG04` JSON TXN1 frame'i
kabul etmez; formatlar version/magic ile ayrılır. 0.1 fixture byte'ları korunur.
Mevcut veri dizinleri ve frozen benchmark executable'ları değiştirilmedi.

## Doğrulama

Native tests feature gerektirmez: typed fields/string/link/-0.0/u64 roundtrip,
version, title/body AND/Türkçe/Unicode, update/delete/compact/reopen, cache
yeniden üretimi, yeni magic'ler, ek payload export olmaması, binary size/depth
limitlerinin append öncesi reddi. Sağlam binary cache yanında published source
kesilmesi/bozulması reddedilir ve cache byte'ları korunur.

Native batch unit testi prefix içine offset encoding, exact size hesabı,
bütün payload kesilmeleri, geçersiz flag/count/tag/offset ve trailing byte
şemasını kontrol eder. Mevcut engine testleri yeni TXN2 frame üzerinde her
partial WAL cut, committed frame'de her byte corruption, acknowledged commit
sonrası process kill, randomized operations ve multi-frame checkpoint'i sınar.
Deneysel testler merge planlandıktan sonra kill, source/cache corruption,
WAL konumlu okuma ve ortak writer reservation cleanup'ını kapsar.

Son doğrulama:

| Komut | Sonuç |
|---|---|
| `cargo test` | 55 geçti, 0 hata |
| `cargo test --all-features` | 106 geçti, 0 hata; 2 subprocess helper ignored, parent testler çağırdı |
| `cargo test --features experimental-rkyv-payload --test rkyv_payload` | 7 geçti, 0 hata; 2 helper parent testlerden çağrıldı |
| `cargo fmt --check` | geçti |
| `cargo clippy --all-targets -- -D warnings` | geçti |
| `cargo clippy --all-targets --all-features -- -D warnings` | geçti |
| `git diff --check` | geçti |

[Default test log](benchmarks/native-binary-20261011-final-default-tests.log),
[all-features test log](benchmarks/native-binary-20261011-final-all-features-tests.log),
[payload/overlay compatibility test log](benchmarks/native-binary-20261011-final-payload-tests.log),
[fmt](benchmarks/native-binary-20261011-final-fmt.log),
[default clippy](benchmarks/native-binary-20261011-final-clippy-default.log),
[all-features clippy](benchmarks/native-binary-20261011-final-clippy-all-features.log).
`tests/engine.rs` ve `tests/fixtures` için diff yoktur.

Önceki benchmark yüzdeleri bu sürümün ölçümü değildir. Yeni hız, disk/RAM, cold-cache,
eşzamanlı throughput, yazma hızları ve 100k uzun corpus karşılaştırması
**bu uygulama doğrulamasında henüz ölçülmedi**. Yeni format için ölçülmüş
performans kabul kapısı geçtiği iddia edilmez; default geçiş kullanıcı kararıdır.
