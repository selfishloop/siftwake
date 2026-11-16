# WAL prefix yayımından sonra arama doğruluğu — 10k

**Kök neden doğrulandı ve düzeltildi.** Arka plan WAL prefix işi yeni
run'ı ekleyip WAL shadow kümesini kalan tail anahtarlarıyla değiştiriyordu.
Yazıcının önceki run listesi için önbelleğe aldığı maskeler geçerli kalıyordu.
Önceden WAL shadow ile gizlenen bir anahtarın eski revizyonu, güncelleme
run'a taşındığında yeniden aday olabiliyordu. İki revizyon aynı sorguda
eşleştiğinde sorgu içi payload önbelleğinin anahtar kaydı yeni run/slot ile
değişiyordu. Exact sonuç hazırlarken eski run/slot denetimi
`checked page slot mismatch` ile duruyordu.

İki belgeyle yapılan regresyon testi aynı hatayı düzeltme öncesinde
üretti. Test, yazıcının maskesini önce bir sorguyla dolduruyor, ardından
bir güncelleme ve bir silme yapıp WAL prefix'ini yayımlıyor. Düzeltme,
yeni run eklendiği anda yalnız yazıcının görünürlük önbelleğini sıfırlıyor.
Arama maskeyi mevcut run listesi ve WAL shadow anahtarlarıyla yeniden kuruyor.
Eski snapshot kendi maskesini ve belge sürümünü koruyor. Run/slot kimliği,
CRC, kalıcı biçim ve reclaim varsayılanı aynı. Hatanın oluşması için
120 saniye geçmesi gerekmedi.
[Kod](../src/engine/light_flush.rs) · [Regresyon testi](../src/engine/light_flush/tests.rs)

| Kontrol | Sonuç |
| --- | --- |
| Küçük regresyon, düzeltme öncesi | **KALDI** · yeni run yayımlandıktan sonra aynı `checked page slot mismatch` |
| Küçük regresyon, düzeltme sonrası | **GEÇTİ** · yazıcı, güncel ve eski snapshot, strict reopen. Exact/capped, sürüm ve silme görünürlüğü eşit |
| Mevcut 10k slot, strict open, sequence 40 | **GEÇTİ** · 10.000 GET/version, 500 silinen anahtar, 41 exact, 41 seri capped ve 41 melez200 capped |
| Dört değişiklik ACK sonrası process kill | **GEÇTİ** · sequence 41. Strict reopen sonrası 10.000 GET/version, 501 silinen anahtar, 41 exact, 41 seri capped ve 41 melez200 capped |
| Dondurulmuş eski okuyucu, sequence 41 | **GEÇTİ** · 10.000 GET/version, 501 silinen anahtar ve 41 exact toplamı/digest'i yeni okuyucuyla eşit |
| Fmt, Clippy ve varsayılan test paketi | **GEÇTİ** · `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`. 266 geçti, 16 atlandı. CRC/bozulma, kesilme ve WAL prefix process-kill testleri dahil. [Test çıktısı](benchmarks/wal-prefix-visibility-20261114/cargo-test.log) |

Mevcut Cap200k korpusun (`881756DA…`) ilk 10.000 belgesi ve mevcut FDB
slotu kullanıldı. Yeniden ingest yapılmadı. Korpus, katalog, önceki model,
0.1 fixture ve dondurulmuş okuyucu hash'leri karşılaştırıldı. SQLite
slotuna dokunulmadı. Tek kurtarma denemesinde FDB slotu sequence 40'tan
41'e geçti ve oracle sonrasında `ready` olarak kaydedildi.
Doğrulama 06:41:38–06:41:54 UTC sürdü. Geçici derleme dizini yaklaşık
84 MiB. Performans ve 120 saniyelik disk/pin deneyi yeniden ölçülmedi.
Önceki uzun snapshot koşusunun durma kaydı tarihsel sonuç olarak korunur.
[Ham doğrulama](benchmarks/wal-prefix-visibility-20261114/status.json) ·
[Sequence 40](benchmarks/wal-prefix-visibility-20261114/strict-before.jsonl) ·
[Process kill](benchmarks/wal-prefix-visibility-20261114/kill.process.json) ·
[Sequence 41](benchmarks/wal-prefix-visibility-20261114/strict-after-kill.jsonl) ·
[Eski okuyucu](benchmarks/wal-prefix-visibility-20261114/frozen.jsonl) ·
[İlk deney](RECLAIM-LONG-SNAPSHOT-10K-20261114.md)
