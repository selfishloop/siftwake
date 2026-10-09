# Yerel JSON Lines protokolü

Tek sahip FDB sürecine stdin/stdout üzerinden bağlanılır. İstekler sırayla
yürütülür; yanıt sırası istek sırasıdır. Her istek UTF-8 ve tek satırdır. CLI'ye
verilen request dosyası normal çok satırlı JSON olabilir. Ağ servisi yoktur.

## Yazma

```json
{"op":"commit","mutations":[{"op":"put","guard":{"kind":"missing"},"document":{"key":{"scope":"shared","collection":"funds","id":"ABC"},"title":"Örnek Fon","fields":{"code":"ABC"}}}]}
```

`guard` verilmezse `any` kabul edilir. Güncellemede
`{"kind":"version","version":1}` kullanılır. `delete` için document yerine
`key` verilir. Batch'teki bütün guard'lar önceki durumla kontrol edilir; biri
reddedilirse hiçbir yazma yapılmaz. Başarı: `{"ok":true,"result":{"sequence":1}}`.

## Okuma

```json
{"op":"get","key":{"scope":"shared","collection":"funds","id":"ABC"}}
```

`result`, `{"document":...,"version":1}` veya kayıt yoksa `null`.

## Arama

```json
{"op":"search","query":{"scope":"shared","collection":"kap_reports","text":"teknoloji yatırımları","exact":{"fund_code":"ABC"},"ranges":[{"field":"as_of","min":20261001,"max":20261031}],"linked_to":{"scope":"shared","collection":"funds","id":"ABC"},"offset":0,"limit":50}}
```

Scope zorunludur. Diğer filtreler isteğe bağlıdır; hepsi AND ile uygulanır.
`linked_to` ters bağlantı filtresidir. Range uçları dahildir. `total` sayfalama
öncesindeki eşleşme sayısı; `hits` sürümlü kayıtlar; `sequence` okuma anındaki
işlem numarasıdır. `offset=(sayfa-1)*limit`; sıra anahtar sırasıdır. Kayıtları
alfabetik ID sırasıyla gezer; sayısal/tarih sırası için pad edilmiş doğal ID
kullanılabilir, ayrı sıralama operatörü henüz yoktur.

String eşitliği byte/case duyarlıdır; sadece `text` filtresi Türkçe katlama yapar.
JSON `1` ve `1.0` farklı scalar eşitlik anahtarı olabilir; uygulama sayı formatını
standartlaştırmalıdır. Null exact filtrelenir; array/object exact reddedilir.

## Bakım

```json
{"op":"stats"}
{"op":"flush"}
{"op":"compact"}
{"op":"poll_compaction"}
{"op":"wait_compaction"}
```

Stats: `documents`, `sequence`, `generation`, `log_bytes`, `disk_index_bytes`,
`disk_posting_entries`, `resident_documents`, `resident_text_bytes`, `delta_keys`,
`segments`, `compaction_running`, `source_bytes`. Kaynak/cache baytları yalnız
referans verilen dosyaları içerir; arşivler toplamına eşit değildir. Resident text
sadece RAM delta metinlerini ölçer; toplam proses RAM'i değildir.

`flush` yalnız değişmiş anahtarları yeni disk segmentine yazar ve delta'yı boşaltır.
`compact` flush yapar ve en fazla bir sınırlı birleştirmeyi arka planda başlatır;
**0.2'den farklı olarak birleştirmenin bitmesini beklemez.** İkisi stats döndürür.
`poll_compaction` biten işi beklemeden yayımlar, `wait_compaction` mevcut işi
bekleyerek yayımlar. Son ikisi `{"published":true|false}` döndürür. İş yoksa false.
Sahip motor yazma/flush öncesinde de biten işi yayımlar. Boşta duran motor
worker sonucunu kendiliğinden yayımlamaz. Bekleme sonraki stdio isteğini de bekletir;
worker çalışırken normal commit/get/search istekleri sürdürülebilir.

Bakım işlem numarasını artırmaz, eski kaynakları silmez. Kapanış worker'ı bekler
fakat yayımlamaz. Request-file modundaki compact yeni delta'yı flush eder;
arka plan sonucunu yayımlamak için uzun yaşayan stdio oturumunu kullanın.

Bozuk JSON, bilinmeyen alan veya geçersiz sorgu hata yanıtı verir; stdio oturumu
devam eder. Başlangıçta lock/corruption hatası veya request-file modunda hata
exit code 1 ile sonuçlanır. 96 MiB'ı aşan stdio satırı süreci kapatır. Üretim
host'u kullanıcı doğrulaması, request timeout ve scope yetkisini kendi katmanında
uygulamalıdır. Uzun içeriğin yanıtlara kopyalanması maliyetlidir; büyük içerikte
metadata ve bölüm kayıtlarını ayrı collection'lara ayırın.
