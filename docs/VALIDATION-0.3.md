# Doğrulama — 0.3 — 07.10.2026

Bağımsız Rust DB motoru üzerinde çalışıldı. SDB/Fundops değiştirilmedi,
servislerine bağlanılmadı, gerçek proje verisi taşınmadı. Hazır DB/arama motoru
eklenmedi; bağımlılıklar serde ve serde_json.

Windows 11 Pro 10.0.26200, AMD Ryzen 7 7840HS, yaklaşık 15,3 GiB görünür RAM,
Rust 1.99.0/MSVC, release optimizasyonları. Minimum Rust bildirimi 1.89;
bu toolchain ayrıca çalıştırılmadı. Disk modeli/soğuk cache ölçülmedi.

| Kontrol | Sonuç |
| --- | --- |
| `cargo test --all-targets` | 38 test geçti: 17 motor, 11 disk, 8 çoklu segment, 2 spill |
| `cargo fmt --check` | geçti |
| `cargo clippy --all-targets -- -D warnings` | geçti |
| `cargo build --release --bins --examples` | geçti |
| Rust linked_search örneği | ilişkili arama + kalıcı yeniden açılış geçti |
| Python stdio demo | commit/search/compact/wait/reopen geçti (`-X utf8`) |
| Bir milyon sentetik kayıt | yazma, sorgu, sayfalama, değişiklik ve yeniden açılış geçti |
| Milyon kayıt DB'sinde gerçek process kill | yanıtı alınmış WAL işlemi ve önceki değişiklikler korundu |

## Depolama ve uyumluluk

Flush yalnız değişmiş anahtarları yeni run'a yazdı; önceki kaynak baytları
değişmedi. Yeni/eskimiş metin, scalar, range ve bağlantı posting'leri kaynaklar
arasında doğru gölgelendi. Tombstone birleştirme/reopen sonrasında kaydı
diriltmedi; silinmiş anahtar Missing guard ile yeniden eklenebildi.

Arka plan işi sürerken yeni commit, update, delete ve flush yapılabildi.
İşin yayımlanması yeni aktif WAL'ı ve sonradan eklenen segmentleri korudu.
Rastgele 250 işlemlik model karşılaştırması metin, range, links, flush,
birleştirme ve yeniden açılışı kapsadı. 0.1 checkpoint fixture baytları
değişmeden açıldı, 0.3 yazma ve manifest sonrasında eski/yeni kayıtlar korundu.
0.2 kaynak biçimi 0.1 ile aynıdır; eski cache biçimi yeniden kurulur.

Sağlam cache varken eski referans kaynağının bozulması açılışı reddetti.
Eksik/bozuk/yanlış kaynağa bağlı cache sağlam kaynaklardan yeniden kuruldu.
Manifest header/nesil/payload/CRC bozulmaları ve marker'ın başka nesil adına
kopyalanması reddedildi; eski manifest'e sessiz geri dönüş olmadı.

Posting spill testleri çok küçük bütçeyle 40 ayrı geçici dosyanın sıralı
birleştirilmesini, ID sırasını, normal temizliği ve header/term/ID checksum
bozulmalarının reddini doğruladı. Eski frame biçimi aynı kaldığından her olası
son işlem kesilmesi ve her tek bayt bozulması testleri de çalıştırıldı.

Gerçek CLI süreçleri başarılı commit ardından, flush yayını öncesinde ve
arka plan birleştirme başlatıldıktan sonra zorla sonlandırıldı. Açılış son
yayımlanmış kaynaklar/WAL üzerinden kabul edilmiş veriyi korudu.
Elektrik kesintisi, disk dolması ve sync hataları bu sonuçların kapsamında değildir.

## Bir milyon kayıt ölçümü

```powershell
cargo build --release --bins --examples
.\tools\measure-scale.ps1 -DatabasePath data/scale-new -Count 1000000 -TextBytes 256 -OutputPrefix output/scale-new
python -X utf8 tools/verify-scale-recovery.py target/release/fdb.exe data/scale-new 1000000
```

Corpus: 1.000.000 kayıt, shared/contents collection, pad edilmiş 10 haneli ID,
benzersiz `rapor-<ID>` başlığı, her kayıtta 256 bayt tekrarlı Türkçe UTF-8 metin,
`source=synthetic` ve `day=id%30` alanları. Toplam içerik 256.000.000 bayt.
500 kayıtlık 2.000 batch, varsayılan delta eşikleri. Bağlantı, gerçek PDF,
çeşitli uzun belge/token corpus'u yok; tekrar eden metin sözlüğü görece küçüktür.

| İşlem | Sonuç |
| --- | --- |
| Ingest + otomatik flush/birleştirme | 276,530 s (~3.616 kayıt/s) |
| Commit medyan / p95 / maksimum, 500 kayıt | 67,450 / 482,561 / 2.041,788 ms |
| Son flush + mevcut iş + bütün uygun sınırlı işler | 15,983 s |
| Tam kaynak/cache kontrolüyle yeniden açılış | 11,753 s |
| 50 update + 50 delete sonrası yeniden açılış | 11,784 s |

Commit örnekleri bakımın yazma sahibindeki flush/publish maliyetini içerir;
arka plan birleştirme yazma gecikmesini sıfırlamaz. Son bakım sonrasında
1.000.000 kayıt 14 segmentteydi. Yaygın corpus arama ölçümleri motor yeniden
açıldıktan sonra, disk tabanında ve sıcak yerel OS/file cache ile yapıldı.

| Sorgu | Eşleşme / tekrar | Medyan | p95 |
| --- | --- | --- | --- |
| `text=rapor-0000500000` | 1 / 30 | 96,900 ms | 104,546 ms |
| `exact source=synthetic` | 1.000.000 / 5 | 176,629 ms | 180,615 ms |
| source + day 2..5 | 133.336 / 5 | 38,346 ms | 45,925 ms |

Bu sorgular default limit=50 ile toplam eşleşme sayısını da hesaplar.
Başlık sorgusundaki tekrarlı rakam trigramları çok sayıda adayı elemek zorunda
kalır; bu sonuç küçük 0.2 corpus'unun dar sorgu ölçümüyle doğrudan karşılaştırılamaz.
Offset=999.950 olan son sayfanın ilk anahtarı ayrıca doğrulandı; ayrı zamanlanmadı.

Ölçüm sonunda corpus'un her bölgesine yayılan 100 anahtardan 50'si güncellendi,
50'si silindi. Flush/reopen sonrasında hepsi kontrol edildi: 999.950 kayıt,
15 segment, sequence=2001. Ardından gerçek CLI sahibi Missing guard ile bir
sentinel yazdı; sequence=2002 yanıtı alındı, süreç kill edildi. Yeni süreçte
999.951 kayıt, sentinel sürümü ve önceki 50 update/50 delete doğrulandı.

Bu yeni süreçte `text=teknoloji` geniş içerik sorgusu 999.900 eşleşme döndürdü.
Üç sıcak tekrar, istemci/stdin/stdout dahil: 9.899 / 9.779 / 9.591 s; medyan
9.779 s. Motor bütün adayların içeriğini okuyup katladığından geniş metin
sorgusu, metadata filtresinden çok daha pahalıdır. Bu ek proses ana ölçümün
bellek izlemesine dahil değildir.

## Gerçek proses belleği ve disk

PowerShell aracı ingest, bakım, iki reopen, sorgular ve update/delete boyunca
örnek prosesini 200 ms aralıkla izledi; 1.481 örnek, 321,050 s toplam çalışma.
Working set için OS'nin o ana kadarki PeakWorkingSet64 alanının en büyük
gözlenen değeri; private bytes için örneklenen PrivateMemorySize64 maksimumu
alındı. Son örnek ile proses çıkışı arasında oluşan tepe kaçabilir.

| Ölçüm | Bayt | Yaklaşık |
| --- | --- | --- |
| Gözlenen tepe working set | 436.944.896 | 416,70 MiB |
| Örneklenen tepe private committed bytes | 542.613.504 | 517,48 MiB |
| Son değişikliklerden sonra referans kaynaklar + aktif WAL | 465.132.539 | 443,59 MiB |
| Referans verilen cache'ler | 861.537.514 | 821,63 MiB |
| Toplam klasör, eski dosyalar dahil; kill sentinel öncesi | 3.986.712.575 | 3,71 GiB |

Working set resident fiziksel sayfaları (paylaşılanlar dahil), private committed
bytes özel commit edilmiş belleği gösterir; birbirine eşit olmak zorunda değildir.
OS dosya cache toplamı, her aşamanın ayrı tepe belleği veya kesin RSS sınırı ölçülmedi.
Son flush'ta `resident_text_bytes=0`, toplam proses RAM'i sıfır demek değildir.
Kayıt/token dizinleri hâlâ RAM'dedir; geçici payload/allocator kapasitesi de maliyettir.

Disk toplamı önceki WAL, run, index ve marker'ları içerir. Otomatik GC yoktur;
stats yalnız referans verilen dosyaları sayar. 64 MiB/100.000 anahtar bütçesini
aşan segmentler tekrar birleştirilmez. Uzun süreli güncellemelerde segment,
tombstone ve arşiv büyümesi için kontrollü saklama/backup politikası gerekir.

## Kanıt ve ölçülmeyen sınırlar

Ham ölçümler: [JSON Lines](benchmarks/scale-0.3.jsonl),
[bellek/disk örnekleme sonucu](benchmarks/scale-0.3.memory.json),
[kill kurtarma ve geniş metin](benchmarks/scale-0.3.recovery.json).
Kaynak: [measure_scale](../examples/measure_scale.rs),
[proses ölçüm aracı](../tools/measure-scale.ps1),
[kurtarma aracı](../tools/verify-scale-recovery.py).

Gerçek SDB/Fundops corpus'u, soğuk I/O, farklı disk/donanım, milyonlarca çeşitli
uzun içerik, ani güç kaybı ve disk dolması ölçülmedi. 4 MiB spill bütçesi bütün
derleme belleğinin sınırı değildir. Geniş substring araması hâlâ içerik
doğrulamasına bağlıdır. MVCC/snapshot pagination, sıralama puanı, otomatik GC ve
çalışan motordan tutarlı backup/restore API'si bu sürümde uygulanmadı.
