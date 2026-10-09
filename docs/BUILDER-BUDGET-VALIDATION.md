# Posting oluşturucu bütçesi — 2026-10-09

`experimental-builder-budget` default kapalıdır. Posting spill tamponu için
oluşturucu başına tercih edilen 1 MiB, ortak 2 MiB rezervasyon havuzu kullanır.
Rezervasyon I/O tamponundan önce alınır; bekleyen oluşturucular aktif işin I/O
kotasıyla ilerlemesini engellemez. Lease, sahip olunan tamponlardan sonra bırakılır.
Bu kota tahmini posting tamponunu kapsar; allocator, delta, sözlük, büyük tek
terim, worker ve toplam proses RAM'i için sert sınır değildir.

Spill birleştirme en fazla 32 okuyucu açar; daha çok dosyada aynı geçici codec ile
ara birleştirme yapılır. Oluşturulan geçici dosyalar hata dahil Drop ile temizlenir.
Ara dosyalar işlem sonuna kadar diskte tutulur. ID'ler 4 KiB bloklarla okunup
yazılır; sıralama ve CRC kontrolü sürer. Kalıcı codec ve 0.1 fixture değişmez.

Kesilme testinde sıfır byte veya tam terim sınırına kadar kısalmış spill'in boş/
tam dosya sayılabildiği bulundu. Oluşturulan dosyanın beklenen uzunluğu artık
kaydedilir, açılışta ve EOF'ta kontrol edilir. Bu düzeltme ve blok I/O default
yola da uygulanır. `.fdb`/`.ready` kaynak doğrulaması korunur.

## Ölçüm

100 bin belge, tekrarlı 256 UTF-8 byte gövde, batch500, watermarks10k/16MiB.
Yeni DB, sıcak yerel OS cache ve sorgu öncesi zamanlanmayan warmup. Üç dönüşümlü
çift B/C,C/B,B/C; ölçüm sırasında build/test/profile çalıştırılmadı.
Baseline frozen `fdb-shared-keys-v2-20261009.exe`, candidate frozen
`fdb-builder-budget-v3-20261009.exe`. Candidate SHA256:
`98DDFD62AD993CF83EED33874180BB8648B677C737DE759FB2D1638605CEA210`.

V4 medyanları:

| Ölçüm | Baseline | Candidate |
| --- | ---: | ---: |
| Yazma + bakım | 10.9055 s | 10.8127 s |
| Tepe proses working set | 18.6523 MiB | 17.3438 MiB |
| Örneklenen private tepe | 15.7070 MiB | 13.9414 MiB |
| Tüm proses CPU | 14.9531 s | 15.1094 s |
| Yeniden açılış | 664.04 ms | 703.11 ms |

RAM yaklaşık %7 azalır. Yazma duvar saati farkı %0.85'tir; kesin hız artışı
sayılmaz. CPU %1.05 artar, yeniden açılış ve bazı kısa sorgular yavaşlar.
Dolayısıyla bütün hız kapıları geçmiş veya SQLite hedefi tamamlanmış değildir.
Working set pencere boyunca open/ingest/maintenance/reopen/query/update/delete
işlemlerini kapsar; private 20 ms hedef aralıkla örneklenir. Sistem geneli cache
ölçülmez. CPU tüm proses içindir, yalnız yazma timer'ı değildir.

512 KiB V2 yazma+bakımı %6.66 yavaşlattığından seçilmedi. İlk 1 MiB V3
karşılaştırmasında aynı baseline proses süreleri 14→11 s değişti; küçük farklar
için kararlı kanıt sayılmadı. V4 aynı binary'leri CPU sayaçlarıyla yeniden ölçer.
Ham sonuçlar `benchmarks/builder-budget-v{2,3,4}-20261009-*` içinde korunur.

V4 altı koşuda bağımsız oracle toplam/sıralı belge/sürüm, update/delete ve reopen
kontrolleri geçti. Üç çiftin final `.ready`, iki `.fdb` ve iki `.idx` dosyası
byte eşittir: `benchmarks/builder-budget-v4-20261009.byte-equality.json`.
Bu turda SQLite yeniden ölçülmedi; eski SQLite sonucuyla güncel hız oranı kurulmaz.
Unique/büyük gövde, 1M corpus ve cold-cache performansı bu ölçümün kapsamı dışıdır.

## Doğrulama

Yeni testler boş/tam-terim kesilmelerini, 4 KiB ID blok sınırlarını, duplicate
ID'leri, 1057 spill ile iki ara birleştirme seviyesini, ara dosyanın her byte
kesilmesini, CRC bozulmasını ve hata sonrası dosya temizliğini kapsar.
Sekiz eşzamanlı DB testi ortak posting havuzunun 2 MiB sınırını ve iş bitiminde
sıfır rezervasyon kaldığını kontrol eder. Son suite/check sonuçları aynı tarihli
`benchmarks/builder-budget-final-*-tests-20261009.log` dosyalarına yazılır.

Default49, seçilmiş composite87, all-features89 test geçti; mevcut süreç
sonlandırma/kurtarma/bozulma testleri dahildir. `cargo fmt --check` ve default,
composite, all-features için `cargo clippy --all-targets -- -D warnings` geçti.
Clippy'nin önerdiği `as_chunks_mut::<4>()` eşdeğer blok iterasyonu sonrasında
spill testleri ayrıca yeniden çalıştırıldı. Ölçülen frozen binary önceki eşdeğer
`chunks_exact_mut(4)` iterasyonunu kullanır; benchmark yeni binary gibi sunulmaz.
