# Normalleştirme deneyi — 2026-10-11

Bu belge payload deneyini kaydeder. Ardından optimizasyon default disk sorgu
yoluna taşındı; [gerçek motor ölçümü](ENGINE-FOLD-VALIDATION-20261011.md)
ayrı sonuçlar ve sınırlar içerir.

Kapsam: default kapalı `experimental-rkyv-payload` içinde JSON ve rkyv payload
sorgularının normalleştirilmesi. Normal motorun `index::fold` fonksiyonu,
kalıcı kaynak, cache formatı, WAL, protokol ve dış projeler değişmedi.
Yeni Cargo feature veya bağımlılık eklenmedi. Önceki rkyv deneyi korunur.

## Birincil sonuç ve karar

**Normalleştirme deneyi payload kapsamındaki hız/disk/working set kapılarını
geçti.** Her iki codec'te dört sorgunun tamamı, üç eşleştirilmiş turda da
hızlandı. Default motor yolu ve önceki kalıcı-format kararı değişmedi.

100k aynı uzun metin corpus'unda toplam sayım + ilk 50 tam belge; üç prosesin
medyanı (ms). Karşılaştırmalar aynı codec içinde eski fold → ASCII+tampon:

| Sorgu | JSON baseline | JSON reuse | Değişim | rkyv baseline | rkyv reuse | Değişim |
|---|---:|---:|---:|---:|---:|---:|
| `the` | 12.292,48 | 7.589,95 | −%38,26 | 11.398,61 | 5.846,97 | −%48,70 |
| `abbreviation` | 176,12 | 138,07 | −%21,60 | 164,16 | 132,72 | −%19,15 |
| `of` | 13.029,25 | 7.595,67 | −%41,70 | 11.212,24 | 5.885,06 | −%47,51 |
| `the whale` | 10.821,49 | 6.558,15 | −%39,40 | 9.691,14 | 5.333,07 | −%44,97 |

| Bellek / codec | Baseline | Reuse |
|---|---:|---:|
| Tepe proses working set / JSON | 16,535 MiB | 16,473 MiB |
| Örneklenen private bytes / JSON | 13,461 MiB | 13,547 MiB |
| Tepe proses working set / rkyv | 12,781 MiB | 12,441 MiB |
| Örneklenen private bytes / rkyv | 9,758 MiB | 9,172 MiB |

Working set medyanı JSON'da %0,38, rkyv'de %2,66 azaldı. JSON'da örneklenen
private bytes **90.112 bayt / %0,64 arttı**. Bu farklı sayaç gizlenmedi;
JSON için belirgin RAM kazancı veya bütün bellek sayaçlarının gerilemediği
iddia edilmez. Kabulte önceden seçilen RAM göstergesi proses working set'tir;
örneklenen private bytes ek bilgi olarak raporlanır. OS cache'i dahil değildir.

Disk byte'ları her stratejide aynı: JSON **2.740.862.279**, rkyv
**2.711.444.148**. Açılışın codec maliyeti optimize edilmedi; baseline/reuse
sorgu karşılaştırması codec değişimini kapsamaz. Bu sonuçlar motorun tamamının
veya SQLite karşısındaki hızının ölçümü değildir.

Ham 12 proses sonucu ve her sorgunun tekrar kontrolleri
[özet JSON](benchmarks/fold-20261011.summary.json) içinde korunur. Kabul:
her sorgu her eşleştirilmiş turda hızlanacak, disk değişmeyecek, tepe proses
working set medyanı gerilemeyecek. İki codec'te üç koşul da sağlandı.
Eski 9 Ekim süreleri yeni binary ile karşılaştırılmadı.

## Normalleştirme ve taramayı ayıran profil

Birincil ölçüm bitince aynı binary ile rkyv'de baseline/ascii/reuse sırasıyla
ayrı çalıştırıldı. Warmup sonrası bir profilli örnek (`repeat=1`), ms:

| Sorgu | Normalleştirme baseline | Normalleştirme ascii | Normalleştirme reuse | Yalnız contains baseline | Yalnız contains reuse |
|---|---:|---:|---:|---:|---:|
| `the` | 7.941,34 | 2.980,65 | 2.671,40 | 19,27 | 15,55 |
| `abbreviation` | 54,23 | 23,67 | 18,35 | 1,23 | 1,19 |
| `of` | 7.930,61 | 2.918,53 | 2.769,63 | 18,09 | 16,24 |
| `the whale` | 6.983,39 | 2.312,56 | 2.227,18 | 140,66 | 119,16 |

Bu corpus'ta normalleştirme, alt dize taramasından çok daha pahalıdır.
Yaygın sorguda normalleştirme yaklaşık **%66,36** azaldı. ASCII stratejisi
tek başına bu profilde büyük azalmayı gösterir; yeniden kullanılan tampon
bu örnekte ek azalma sağlar. `ascii` için üç ayrı proses çifti yapılmadığı ve
profiller sırayla çalıştığı için tamponun tek başına kesin kazanç yüzdesi
iddia edilmez. Tekrarlanabilir toplam kazanç, birleşik reuse yolu içindir.

Reuse yaygın sorgusunun profilli toplamı 5.431,93 ms: normalleştirme
2.671,40, yalnız contains 15,55, CRC 1.410,38, okuma 701,87,
bytecheck/graph/kimlik 595,76 ms. Rkyv'nin belgeyi çözmeden alanlara erişmesi
CRC/doğrulama veya normalleştirme maliyetini kaldırmaz. Dar sorguda okuma
86,48 ms ile toplam 122,40 ms'nin yaklaşık %71'idir; bu yüzden toplam kazanç
uzun tam taramalara göre daha küçüktür.

Her adayda ek saat okumaları vardır. Profilli toplamlar birincil hız kapısına
katılmaz; sütunların toplamı döngü ve kalan işlemler nedeniyle toplam süreye
tam eşit olmak zorunda değildir. Ham profiller:
[baseline](benchmarks/fold-profile-20261011-rkyv-baseline.jsonl),
[ascii](benchmarks/fold-profile-20261011-rkyv-ascii.jsonl),
[reuse](benchmarks/fold-profile-20261011-rkyv-reuse.jsonl).

## Uygulama

`TextMatcher` üç strateji sunar:

- `baseline`: mevcut `index::fold` ile yeni String üretir.
- `ascii`: yeni String üretir; tamamen ASCII girdide toplu ASCII lowercase,
  karma UTF-8 girdide ASCII karakterler için basit dönüşüm kullanır. Diğer
  karakterler aynı Türkçe eşlemelerden ve `char::to_lowercase` işleminden geçer.
  Karma girdide çıktı için giriş uzunluğu kadar kapasite önceden ayrılır;
  Unicode lowercase genişlemesi gerektiğinde String güvenli biçimde büyür.
- `reuse`: aynı hızlandırılmış dönüşüm, title/text için iki tekrar kullanılan
  String tamponuna yazar. Tamponlar yalnız bir sorgunun aday döngüsü boyunca
  yaşar; belge veya normalleştirilmiş corpus cache'i değildir.

Başlık bütün terimleri karşılıyorsa gövde normalleştirilmez. AND terimleri
başlık veya gövdede bulunmalıdır; farklı alanları birleştirip sahte alt dize
üreten yol yoktur. Sorgu terimleri bir kez mevcut fold ile hazırlanır.
Tamponun tutulması en uzun aday metne göre kapasite tutabilir; toplam proses
belleği ayrıca ölçülür. Unicode doğrulaması atlanmaz.

Profilde `normalization_ms` başlık/gövde dönüşümünü, `contains_ms` yalnız
alt dize kontrollerini ölçer. Eski 9 Ekim profilindeki `contains_ms` ikisinin
toplamıydı; bu alanlar doğrudan tarih karşılaştırması için kullanılmamalıdır.
CRC, okuma, codec doğrulama/JSON çözme ve ilk 50 tam sonuç dönüşümü ayrıdır.
Terimleri hazırlama/döngü/bırakma gibi kalan işlemler toplam süreye dahildir.

## Corpus ve yöntem

Aynı sekiz Gutenberg kitabından, örtüşebilen fakat birebir aynı olmayan
100.000 UTF-8 pasaj: 2.048–51.200 bayt, toplam 2.655.160.287 bayt.
Corpus İngilizce ağırlıklıdır; Unicode doğruluğu ayrıca test edilir.
[Kaynak hash'leri ve provenance](benchmarks/rkyv-corpus-20261009.provenance.json).
`the`/`abbreviation`/`of`/`the whale` sorguları; aday sayıları sırasıyla
100.000/529/100.000/75.664, sonuçlar 100.000/264/100.000/16.537.

JSON ve rkyv export'ları yeniden kodlanmaz. Her codec kendi baseline/reuse
yoluyla aynı dosyayı, CRC ve aynı yapısal/kimlik doğrulamasını kullanır.
Önceden hesaplanmış ortak adaylar kullanılır; indeks üretimi, gerçek DB sorgu
planlama ve soğuk disk kapsam dışıdır. Bağımsız oracle toplamı, ilk 50 tam
belgeyi, sıralamayı ve version'ı her sorguda doğrular.

Tek release binary, SHA-256:
`0A9F11F64C233ECA73A40492EE9D0A06743E63DD0D411FBDF3B604AB29202F33`.
Üç turda codec ve strateji sırası dönüşümlüdür. Warm cache: tam doğrulamalı
açılış ve bir untimed sorgu; cache zorla boşaltılmaz. Her uzun sorgu için
proses başına bir timed örnek, dar sorguda 20 örnek; rapor üç prosesin
medyanını kullanır. Tek örnekli raw p95 kuyruk gecikmesi kanıtı değildir.
Profilli çalıştırmalar birincil ölçümden sonra ve kabul sürelerinden ayrı yapılır.
Ölçüm sırasında build/test/profil çalıştırılmaz.

Bellek Windows PeakWorkingSet64 ile tüm prosesin açılış+oracle+aday+sorgu
penceresinde ölçülür. Private bytes 20 ms hedef aralıkla örneklenir;
OS dosya cache'i dahil değildir. İç tampon kapasitesi toplam proses RAM'i
olarak sunulmaz. Makine Ryzen 7 7840HS / Windows 11 / yaklaşık 15,31 GiB RAM.

## Doğruluk ve kontroller

Yeni differential test bütün geçerli Unicode scalar'larını mevcut fold ile
karşılaştırır. Ayrıca ASCII, Türkçe, combining karakter, Greek harfler,
genişleyen lowercase, emoji/CJK, boş sorgu, başlıkta eşleşme, AND ve uzun
tampondan sonra kısa metin test edilir. Üç stratejinin eşleşmesi eski
`text_matches` ile karşılaştırılır; corpus oracle ayrıca gerçek uzun metni sınar.

- Feature'lı lib testleri: **11 geçti**.
- Feature'lı payload entegrasyon testleri: **4 geçti**, child helper ignored
  fakat process-kill testi içinde çağrıldı. Kesilme, tek bayt bozulması ve eski
  0.1 JSON kaynağı testleri korundu ve geçti.
- Default, feature'lı ve all-features `cargo clippy --all-targets -- -D warnings`
  geçti.
- `cargo fmt --check` geçti. `tests/engine.rs` ve 0.1 fixture değişmedi.

## Yeniden çalıştırma

```powershell
cargo build --release --example measure_rkyv_payload --features experimental-rkyv-payload
Copy-Item target/release/examples/measure_rkyv_payload.exe output/fdb-fold-20261011.exe
./tools/measure-payload-codecs.ps1 -Prefix yeni-fold -Executable output/fdb-fold-20261011.exe -Strategies baseline,reuse
python tools/summarize-fold-strategies.py docs/benchmarks/yeni-fold
./output/fdb-fold-20261011.exe profile-rkyv data/rkyv-corpus-20261009 baseline
./output/fdb-fold-20261011.exe profile-rkyv data/rkyv-corpus-20261009 ascii
./output/fdb-fold-20261011.exe profile-rkyv data/rkyv-corpus-20261009 reuse
```

Yeni prefix kullanılır; mevcut ölçümler ezilmez. `baseline` isteğe bağlı üçüncü
argümanın default'udur; eski example çağrıları çalışır. Eski sabitlenmiş binary
bu argümanı desteklemez, yeni binary kullanılmalıdır. `ascii` profilinin amacı
ASCII yolunun ve tampon paylaşımının katkısını ayırmaktır; tek profilli örnek
tekrarlanabilir hız kazancı kanıtı olarak kullanılmaz.

Ölçüm script'inde `Strategies` verilmezse eski iki argümanlı binary çağrısı ve
eski dosya adları korunur. Stratejiler açıkça verilirse üçüncü argüman eklenir;
9 Ekim'in sabitlenmiş binary'si değiştirilmez.
