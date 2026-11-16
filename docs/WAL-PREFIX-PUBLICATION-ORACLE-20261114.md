# Prefix, merge, reclaim ve GC model testi

**Adım 2 GEÇTİ.** Varsayılan test paketinde çalışan regresyon, 16 sabit
tohumla 64 anahtar üzerinde tohum başına 24 rastgele akışlı işlem grubunu
kontrol ediyor. Toplam 384 rastgele gruba ek olarak, her tohumda aynı
kontrollü yayım adımları çalışıyor. Test, küçük geçici birim-test
veritabanlarını ve mevcut test yardımcılarını kullanıyor.
[Test](../src/engine/snapshot/publication_oracle.rs)

| Kontrol | Sonuç |
| --- | --- |
| Prefix kurulumu | **GEÇTİ** · 48 kurulum. Run eklendi, ilk kurulumda aktif WAL değişti ve işçi çalışırken yazılan tail korundu. Üç farklı görünümde sorgu doğrulandı. |
| Normal merge | **GEÇTİ** · 16 gerçek run değişimi. Eski snapshot kendi run'larını okudu. |
| Arka plan reclaim | **GEÇTİ** · 16 gerçek kısmi rewrite. Eski run 48 kayıttan 16 canlı kayda indi, sonraki run yerinde kaldı. |
| GC | **GEÇTİ** · 16 tohumda eski kaynak ve eski WAL iki snapshot ile, ardından tek snapshot ile pinli kaldı. Son pin bırakıldığında dosyalar kalktı, canlı dosyalar ve manifest kuşağı korundu. |
| Model oracle ve strict reopen | **GEÇTİ** · GET, belge sayısı, sequence, exact toplam ve sıralı tam hit/gövde/sürüm, seri ve melez capped sayfa ve toplam sözleşmesi doğrulandı. Görünürlük değişimleri, yalnız eski/yeni revizyonla eşleşen sorgular, silme, yeniden ekleme ve offset'li sayfa kapsandı. 16 strict reopen ve bütünlük kontrolü geçti. |
| Düzeltme öncesi sessiz sonuç | **KALDI, beklenen regresyon** · yalnız geçici derlemede prefix maske reset'i çıkarıldığında tohum 1, kuşak 6, sequence 3'te yazıcı eski marker sorgusunda hata döndürmeden exact toplamı 1 verdi. Model 0 bekliyordu. Bütün GET kontrolleri bu sorgudan önce geçti. [Ham çıktı](benchmarks/wal-prefix-publication-oracle-20261114/without-prefix-mask-reset.log) |

Model her işlem grubundan beklenen belge ve sürümü kendi sıralı tablosunda
oluşturur. Sorgu eşleşmesini küçük ASCII işaretlerini bağımsız tarayarak bulur.
Test, işçi başlatmayı yayım saymaz. Yeni run kimlikleri ve manifest kuşağı
kuruluma, dosya varlığı da pin ve GC'ye tanık olur. Arka plan reclaim kontrollü
adımda açılır. Melez bütçesi 2, küçük veride işçi devrinin kullanılabilmesi
için seçildi.

Bu adım yalnız test kapsamındadır. Doğrudan çıktısız reclaim emekliliği,
farklı işçi zamanlamalarının tümü ve 120 saniyelik pin ölçümü bu model
testinin kapsamı dışındadır. Önceki gerçek slot sonuçları tarihsel
kayıt olarak durur. İki sabit benchmark slotu kullanılmadı.

Varsayılan debug derlemesinde test tek başına **21,52 s**, diğer
testlerle birlikte kütüphane testleri toplam **38,81 s** sürdü.
`cargo fmt --check` ve `cargo clippy --all-targets -- -D warnings`
geçti. `cargo test --all-targets` sonucu **267 geçti, 16 atlandı**.
Mevcut CRC, kesilme ve process-kill testleri de çalıştı.
0.1 fixture SHA256 değerleri önceki doğrulamayla eşleşti.
[Tek test çıktısı](benchmarks/wal-prefix-publication-oracle-20261114/isolated-test.log)

[Fmt](benchmarks/wal-prefix-publication-oracle-20261114/fmt.log) ·
[Clippy](benchmarks/wal-prefix-publication-oracle-20261114/clippy.log) ·
[Tam test çıktısı](benchmarks/wal-prefix-publication-oracle-20261114/test.log)

| Konum | Mantıksal dosya boyutu |
| --- | ---: |
| `target/wal-prefix-publication-oracle-20261114` | 415,78 MiB |
| `target/wal-prefix-visibility-20261114` | 83,83 MiB |
| `target/reclaim-followup-20261114` | 93,18 MiB |
| FDB slotu | 68,25 MiB |
| SQLite slotu | 491,41 MiB |

Geçici derleme dizinleri korundu. Adım 3'teki performans ve orijinal pin
koşusu için ayrı başlatma onayı bekleniyor.
