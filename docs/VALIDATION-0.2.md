# Doğrulama — 0.2 — 07.10.2026

Windows/MSVC, Rust 1.99.0. Çalıştırmalar yalnız FDB motoru ve sentetik verilerle
yapıldı; dış proje veya canlı servis entegrasyonu yoktur.

| Kontrol | Sonuç |
| --- | --- |
| Motor testleri | 17 test geçti |
| Disk segmenti testleri | 11 test geçti |
| `cargo fmt --check` | geçti |
| `cargo clippy --all-targets -- -D warnings` | geçti |
| Release binary ve örnekler | derlendi |
| Python stdio demo | yazma, arama, compact ve yeniden açılış geçti |
| 0.1 checkpoint fixture | kaynağı yeniden yazmadan açıldı, ilişkili arama ve 0.2 yazması geçti |

Minimum Rust bildirimi 1.89; bu toolchain ayrıca çalıştırılmadı.

## Disk ve kurtarma kapsamı

Checkpoint sonrasında uzun içeriklerin ve posting listelerinin RAM delta'sından
çıktığı doğrulandı. Disk tabanı ile RAM delta'sındaki sonuçların anahtar sırasında
birleşmesi, güncelleme/silmenin eski metin/alan/bağlantı eşleşmelerini gölgelemesi,
blok sınırlarında sayfalama, Unicode ve birden fazla collection'da range/exact
filtreleri test edildi.

Kayıp, checksum'u bozulmuş veya yanlış nesle ait `.idx` cache'i sağlam log'dan
yeniden üretildi. Diskten okunan kayıt payload'ının çalışma sırasında bozulması
get ile yakalandı. Otomatik checkpoint guard reddinde çalışmadı; doğru sürümlü
bir sonraki yazmayı kabul edip eski kayıt sürümlerini korudu.

Gerçek CLI süreci, başarılı 4 MB yazma yanıtından sonra compact başlatılırken
yeni log dosyası oluşturulur oluşturulmaz öldürüldü. `.ready` yayımlanmadan
sonlandırıldığı doğrulandı. Yeniden açılışta önceki kabul edilmiş sekiz kayıt ve
işlem numarası korundu; kaynak log baytları değişmedi. Önceki testlerde her
son-işlem kesilme konumu ve tam frame'de her tek bayt bozulması da kontrol edildi.

## 50.000 kayıt / 51,2 MB metin

Komut:

```powershell
.\target\release\examples\measure_disk.exe data/disk-benchmark-cached-20261007 50000 1024
```

Her kayıtta 1.024 bayt UTF-8 metin, benzersiz rapor başlığı, `source` ve `day`
metadata'sı var. 500'er kayıttan 100 batch yazıldı. Otomatik bakım varsayılan
eşiklerle çalıştı. Son açık compact ardından motor kapatılıp tekrar açıldı;
ölçülen bütün sorgular disk tabanında çalıştı. Yerel dosya/OS cache'i sıcaktır.

| İşlem | Medyan | p95 |
| --- | --- | --- |
| Tek rapor başlığını bulan metin araması, 30 tekrar | 0,357 ms | 0,647 ms |
| Bütün kayıtlarda source eşitliği, 5 tekrar | 0,859 ms | 1,111 ms |
| Source + day 2..5 aralığı, 5 tekrar | 0,466 ms | 0,580 ms |

Yazma ve otomatik bakım toplam 14,208 s; son compact 4,287 s;
tam doğrulamalı sıcak yeniden açılış 752,53 ms.

| Sayaç | Son compact sonrası |
| --- | --- |
| Kayıt | 50.000 |
| Mantıksal metin | 51.200.000 bayt |
| RAM delta kayıtları | 0 |
| RAM delta kaynak+katlanmış metin | 0 bayt |
| Disk posting girdileri | 2.588.501 |
| Türetilmiş disk indeksi | 77.081.166 bayt |
| Etkin kaynak checkpoint/log | 61.379.842 bayt |

Son compact öncesinde 40.000 kayıt disk tabanında ve 10.000 kayıt delta'daydı;
RAM metin sayacı 19.640.000 bayttı. Compact bu içerikleri disk tabanına taşıdı.
Anahtar ve token dizinlerinin RAM'i, allocator kapasitesi, işletim sistemi cache'i
ve proses working set'i bu sayaçtan ölçülmez. 0 bayt, toplam RAM'in 0 olması değildir.

Sorgu başına küçük posting ID'leri için 256 KiB mantıksal veriye kadar geçici
önbellek kullanılır; Vec kapasitesi/metadata ek maliyettir. İlk önbelleksiz dar
sorgu medyanı 1,854 ms idi; son ölçüm geçici önbelleği içerir.

Eski nesiller korunur; yukarıdaki etkin dosya boyutları toplam disk kullanımı
değildir. Log ve türetilmiş segment aynı canlı payload'ları iki yerde saklar.

## Ölçülmeyen sınırlar

Milyonlarca kayıt, gerçek corpus çeşitliliği, geniş içerik metin sorguları, soğuk
I/O, toplam RSS ve checkpoint derleme belleği ölçülmedi. Posting sözlüğü/anahtar
dizini RAM'dedir; segment derlemesi geçici u32 posting'leri RAM'de kurar. Directory
JSON sınırı 256 MiB'dır. Otomatik eşik kesin bellek limiti değildir; batch bunu
aşabilir. Kaynak log ve indeks açılışta tamamen taranır. Elektrik kesintisi ve
disk dolması kaynaklı sync hatası test edilmedi.
