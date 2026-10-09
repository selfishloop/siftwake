# Doğrulama — 0.1 — 07.10.2026

Bu belge ilk 0.1 sürümünün tarihsel kaydıdır.
Güncel sürüm için [0.2 doğrulama sonuçları](VALIDATION-0.2.md) okunmalıdır.

Windows/MSVC üzerinde Rust 1.99.0 ile çalıştırıldı. Minimum sürüm bildirimi
1.89'dur; bu minimum toolchain ayrıca çalıştırılmadı.

| Kontrol | Sonuç |
| --- | --- |
| `cargo test --all-targets` | 17 entegrasyon testi geçti |
| `cargo fmt --check` | geçti |
| `cargo clippy --all-targets -- -D warnings` | geçti |
| `cargo build --release --bins --examples` | geçti |
| `linked_search` | fon/rapor/dosya/içerik bağlantısı, tarih filtresi, compact ve reopen geçti |
| Python `clients/demo.py` | yazma, Türkçe arama, compact ve reopen geçti |
| Request dosyası CLI | commit ve filtreli arama geçti |
| Gerçek SDB `data/c.tsv` aktarımı | ilk 1.000 uygun benzersiz kayıt aktarıldı |
| Python `clients/verify_sdb.py` | kaynakla ad/ID/parent/flags karşılaştırması, arama, scope, compact/reopen geçti |

Kaynak SDB TSV salt okunur açıldı; SDB ve Fundops repository'lerinde değişiklik
yapılmadı. Fundops canlı Hanbase'e bağlanılmadı ve gerçek Fundops verisiyle
entegrasyon testi yapılmadı. Fundops adapter ve örnek rapor testi sentetiktir.

## Sentetik ölçüm

`target/release/examples/measure.exe data/benchmark-optimized-20261007`:
10.000 dosya metadata kaydı, 500'er kayıttan 20 kalıcı batch, benzersiz bir dosya
adını bulan aynı dar sorgu 100 kez. Yerel makine, sıcak işletim sistemi cache'i;
soğuk açılış veya milyonlarca dosya için genellenmez.

| Ölçüm | Değer |
| --- | --- |
| Kayıtların yazılması ve indeks kurulması | 632,52 ms |
| Dar sorgu p50 | 0,0287 ms |
| Dar sorgu p95 | 0,0311 ms |
| Checkpoint | 60,79 ms |
| Yeniden açılış ve indeks kurulması | 642,41 ms |
| Etkin log/checkpoint boyutu | 4.123.431 bayt |

Eski nesiller diskte tutulduğundan etkin log boyutu toplam disk kullanımı değildir.
RAM miktarı ve geniş sorguların p95'i bu çalıştırmada ölçülmedi. İlk ölçümde
posting kümelerinin kopyalanması maliyetliydi; sorgu artık en küçük posting
kümesini referansla gezip diğerleriyle kesişimi doğruluyor. Son ölçüm bu sürümdür.

## Hata testleri

- CLI süreci başarılı yazma yanıtından sonra zorla öldürüldü; veri yeniden açıldı.
- Son atomik batch bütün olası bayt kesilme konumlarında test edildi; önceki
  tamamlanmış işlem korunup yarım batch tamamen atıldı.
- Tam frame'de her bir bayt ayrı ayrı değiştirildi; açılış hata verdi ve dosya
  sessizce kısaltılmadı.
- Bozuk en yeni checkpoint eski nesle otomatik düşmedi.
- Yayımlanmamış yarım nesil/marker atlandı; sonraki compact yeni nesle yazdı.
- Yayımlanmış ama kesilmiş checkpoint ve marker kaybı reddedildi.
- Guard çatışması batch'i diske yazmadan reddetti.
- Güncelleme/silmede metin, exact, range ve bağlantı posting'leri temizlendi.
- Referans BTreeMap modele karşı 240 rastgele güncelleme/silme turu ve düzenli
  compact/reopen test edildi.
- 8 MiB üstü, birden fazla frame içeren checkpoint ve boş DB checkpoint'i test edildi.

Elektrik kesintisi, disk dolması/gerçek write-sync hatası, ağ servisi ve büyük
dosya corpus'u test edilmedi. Kalıcılık sonuçları yukarıdaki kapsamla sınırlıdır.
