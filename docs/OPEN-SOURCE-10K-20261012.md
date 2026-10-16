# Aynı 10k DB: açılışta tekrar kaynak taramasının kaldırılması

Beş eşleşmiş taze proses çiftinde normal açılış medyanı **945,63 → 746,62 ms**
oldu (%21,04 azalma). Her çift hızlı: %21,24 /18,89 /19,04 /18,92 /21,87.
Bu hâlâ yaklaşık0,75 saniye; açılış hedefinin bütünü karşılanmış değildir.

| Koşu | Eski ms | Yeni ms |
|---|---:|---:|
| 0 | 945,6329 | 744,8137 |
| 1 | 934,4285 | 757,8694 |
| 2 | 958,4834 | 775,9978 |
| 3 | 920,8338 | 746,6232 |
| 4 | 949,5275 | 741,8998 |

## Değişiklik ve bütünlük

Normal immutable kaynak açılışında ayrı prefix_crc taraması kaldırıldı.
Replay zaten payload CRC'sini gerçek baytlardan hesaplıyor; header, doğrulanmış
payload CRC ve footer sırayla IEEE CRC birleştirmesine katılıyor. Sonuç,
manifestteki aynı tam-source checksum ile karşılaştırılıyor. Header/footer,
frame sequence, belge biçimi, anahtar/sürüm ve bütün payload kontrolleri korunur.
CRC(A||B) bileşimi için [zlib checksum sözleşmesi](https://www.zlib.net/manual.html)
referans alındı; runtime zlib/DB bağımlılığı eklenmedi. GF(2) kaydırma tabloları
Rust içinde sabit8 KiB; frame başına heap ayrımı yok. CRC sınırlamaları değişmez.

Callbacks son source CRC karşılaştırmasından önce geçici builder'a yazabilir;
karşılaştırma başarısızsa builder finish/publication yapılmaz, Drop geçici
dosyaları temizler. Eksik cache'in yeniden oluşturulması hâlâ desteklenir.
Legacy unknown-checksum ve experimental-streaming-open yolu ayrı prefix
taramasını korur; streaming feature ile aynı hız kazancı iddia edilmez.
Yeni kalıcı format/feature yok; WAL kurtarma sözleşmesi değiştirilmedi.

## Profil ve kalan maliyet

Tek instrumented önce/sonra örneği, ms:

| Span | Önce | Sonra |
|---|---:|---:|
| open.total | 1.060,41 | 762,89 |
| Ayrı source CRC | 199,67 | Yok |
| Source CRC birleştirme | Yok | 0,0517 |
| Source replay/doğrulama | 528,01 | 453,53 |
| Index open | 328,70 | 305,83 |
| Index CRC (index open içinde) | 267,29 | 254,56 |

Nested span'ler örtüşür, toplanmaz; profiller normal benchmark değildir.
Replay/decode farkının tamamı bu değişikliğe bağlanmaz; asıl kaldırılan iş ayrı
kaynak taramasıdır. İndeks CRC ve tam belge decode/doğrulaması hâlâ maliyetlidir.

## Yöntem ve sınırlar

Mevcut `data/benchmark-work/fdb`: aynı ilk10.000 belge,5 segment,seq20,boş delta;
aktif source267.900.709 B, index353.242.581 B. Metin264.842.858 B,2.057–51.197 B
belge başına,8 eserden benzersiz kısmen örtüşen pasajlar. Yeni corpus/DB yok.
Query yok: yalnız Database::open + stats10k/delta0 kontrolü; rapor ve dosya
hash'leri timer dışında. Her motor bir ölçümsüz açılışla ısıtıldı, OS cache
boşaltılmadı; **sıcak cache**. Normal release thin-LTO/codegen-units1,
experimental-merge-postings + experimental-delta-bitmaps, diagnostic kapalı.
Çiftlerde control/new sırası dönüşümlü; executable SHA256 kayıtlı.
SQLite tekrar ölçülmedi; SQLite bağlantısını tam kaynak kontrolüyle eşdeğer
saymıyoruz. RAM, cold cache, yazma/bakım, eşzamanlı yük ve büyük corpus ölçülmedi.
Mevcut `.fdb/.ready/.idx` boyut/SHA256 değerleri önce/sonra aynı.

## Testler ve kanıt

Default, merge+bitmap ve all-features all-targets testleri geçti. Source kesilme,
tek bayt bozulması, process kill ve0.1 fixture testleri geçti. Yeni CRC bileşimi
referans IEEE CRC ve farklı kesim noktalarıyla eşit; büyük uzunluk operatörleri
heap'e GB ayırmadan kontrol edildi. Çok frame ve trailing unpublished byte
prefix kontrolü geçti.

Değiştirilmiş payload'ın frame/header CRC'leri düzeltilse bile sağlam cache
bozuk source'u gizleyemiyor; beklenen source CRC açılışı reddediyor. Cache yokken
de reddediliyor, geçici rebuild `.idx` yayımlamıyor. Bu ek test default ve
all-features koşularında geçti. Son `cargo fmt --check` ve default/all-features
`cargo clippy --all-targets -- -D warnings` geçti.

Son normal read kontrolünde3.000 get ve dört sorguda120 timed search'ın tam
sayfa/sürüm/toplam oracle'ları geçti; `open-read-smoke.jsonl`. Bu tek koşu yeni
arama hız karşılaştırması değildir. Sonrasında kaynak/cache hash'leri yine aynı.

Araç: `tools/measure-open-source.ps1` (mevcut raporu ezmez). Ham sonuçlar:
`docs/benchmarks/open-source-10k-20261012-{control,fdb}-0..4.jsonl`, executable
hash'leri ve summary aynı prefix'te. Profiller `open-profile-{before,after}.jsonl`;
test logları `open-tests-*`, `open-native-tests*`, `open-clippy-*`, `open-fmt.log`.

## Açık işler

[Güncel iş listesi](OPEN-WORK.md): yazma/bakımın güncel SQLite karşılaştırması,
arama regresyonları, RAM/disk, bakım sırasında okuma/yazma ve soğuk cache açık.
Açılışın ilk optimizasyonu tamamlandı; kalan0,75 saniye için inceleme sürmeli.
