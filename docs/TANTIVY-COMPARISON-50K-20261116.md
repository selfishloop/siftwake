# FDB ve Tantivy, 50 bin belge

İki dengeli blok tamamlandı. Bu iş yükünde **FDB ilk sayfada 39/41,
exact sorguda 40/41 durumda daha düşük p95** verdi. Başarılı Tantivy
koşuları indeksi daha hızlı kurdu, daha az disk kullandı ve sıcak
açılıştan ilk sonuca daha erken ulaştı. İki motorda da dört başarılı
kolun tamamında payload ve arama oracle'ı geçti.

| Ölçüm | FDB | Tantivy |
| --- | ---: | ---: |
| 50k ingest + son bakım, A / B | 46,43 / 39,61 s | 16,44 / 16,79 s |
| Ingest + bakım süreç CPU, A / B | 47,22 / 39,42 s | 20,42 / 21,27 s |
| Bakım sonrası disk | 300,84 MB | 137,82 MB |
| Bakım sonrası run / segment | 3 | 9 |
| Ingest tepe private commit, A / B | 17,86 / 17,85 MiB | 38,40 / 37,42 MiB |
| Ingest tepe working set, A / B | 27,84 / 21,93 MiB | 74,56 / 68,89 MiB |
| Boşta private commit, A / B | 2,38 / 2,37 MiB | 1,11 / 1,10 MiB |
| Boşta working set, A / B | 7,18 / 7,17 MiB | 5,70 / 5,69 MiB |
| Sıcak okuyucu hazır, p50 / p95 (`n=12`) | 203,69 / 215,44 ms | 3,94 / 6,69 ms |
| Sıcak açılıştan ilk sonuca, p50 / p95 (`n=12`) | 206,20 / 217,93 ms | 8,66 / 12,43 ms |
| Yukarıdaki ilk çağrıda yalnız sorgu, p50 / p95 | 2,51 / 4,82 ms | 4,77 / 5,94 ms |
| Ölçülen 82 sorguluk tur, süre p50 / p95 | 6,74 / 7,17 s | 16,88 / 18,25 s |
| Aynı turun süreç CPU p50 / p95 | 7,53 / 8,17 s | 16,81 / 18,09 s |
| Tekli güncelleme commit p50 / p95 / max (`n=48`) | 54,23 / 142,13 / 156,31 ms | 46,58 / 70,97 / 82,06 ms |
| Commit sonrası explicit reload p50 / p95 | FDB commit içinde yayım | 4,87 / 12,03 ms |
| Güncellemeden ilk doğru sonuca p50 / p95 / max (`n=48`) | 55,23 / 143,23 / 157,32 ms | 52,64 / 81,17 / 97,61 ms |

| Sorgu | FDB ilk 50 p95 | Tantivy ilk 50 p95 | FDB exact p95 | Tantivy exact p95 |
| --- | ---: | ---: | ---: | ---: |
| olduğu | 1,85 ms | 4,00 ms | 223,36 ms | 643,19 ms |
| üreticisi | 23,88 ms | 39,06 ms | 152,53 ms | 297,10 ms |
| kendisine çıkacağını | 72,28 ms | 291,66 ms | 147,79 ms | 291,90 ms |
| zx | 97,02 ms | 1.028,16 ms | 218,13 ms | 1.008,87 ms |
| qzxkvwnever | 0,42 ms | 0,11 ms | 0,04 ms | 0,09 ms |
| transubstantiationz | 1,49 ms | 0,48 ms | 1,68 ms | 0,48 ms |

Her sorgu/yol için motor başına 24 zamanlı örnek. [41 sorgunun tamamı](benchmarks/tantivy-comparison-50k-20261116/query-matrix.md) · [Ham örnekler ve dağılımlar](benchmarks/tantivy-comparison-50k-20261116/summary.json)

## Yöntem

- Aynı salt okunur cap200k payload'ın ilk 50.000 belgesi ve doğrulanmış 41-case kataloğu. Payload SHA-256 `881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12`. FDB 0.4.0 kaynak commit'i `56ee1f0`, Tantivy `0.27.0`. Ayrı benchmark paketi iki bağımlılığı birleştirir. Uygulama motorunun manifesti ve çalışma zamanı bağımlılıkları korunur.
- Başarılı kolların sırası FDB → Tantivy → Tantivy → FDB. Her başarılı kolda temiz başlangıç, 500 belge/commit, 100 commit, motorun bakımını bitirme, bütün 50.000 belgeyi ve sorgu sonuçlarını korpus oracle'ına karşı doğrulama. FDB `commit_streamed` ve varsayılan motor seçenekleri ile `maintain_idle()`, Tantivy 1 indeksleme işçisi, 32.000.000 B işçi arena hedefi ve varsayılan segment birleştirme politikası ile `wait_merging_threads()`. FDB bakım süresine kendi bütünlük kontrolü de dahil. Tantivy checksum kontrolü ayrı oracle aşamasında. Test sürücüsü çalışma alanındaki korunan ana FDB ve önceki SQLite verisini açmadı. Geçici FDB alt klasörü her tamamlanan koldan sonra kaldırılır.
- Arama Türkçe normalizasyonlu, her terim için başlıkta veya gövdede substring, terimler arasında AND, sayısal anahtar sırası ve tam belge dönüşü. Tantivy üç karakterli posting kesişimini aday filtresi olarak kullanır. Uygulama katmanı canlı adayları anahtar sırasına birleştirir ve saklanan içeriği doğrular. Yalnız kısa terim içeren çağrılar tüm canlı anahtarları tarar. FDB exact seri ve capped hibrit200. Exact mevcut katalog offset/limit'lerini korur. Capped aynı 41 sorgu için offset 0 ve ilk 50. Tantivy LZ4 ile özgün başlık/gövde ve fields/links saklar. FDB fields ve links için ek indeksler de oluşturur.
- Sıcak arama, her kolda ısınma sonrası 12 tur. Sorgu sırası blok ve tura göre döndürülür. Tam belge üretimi zamanlanan çağrı içinde, oracle karşılaştırması dışında. Tur CPU toplamı süreçteki bütün işçileri ve harness'in oracle/ölçüm kayıtlarını da kapsar. p50/p95 en yakın sıra.
- Sıcak ilk sonuç, dosyalar sıralı okunup Windows dosya cache'i ısıtıldıktan sonra kolda altı ayrı proses. Saat okuyucu açılışından `olduğu` ilk 50 tam belgenin dönüşüne kadar işler. FDB strict-open, Tantivy normal mmap açılışı. Tantivy tam indeks checksum taraması ayrı doğrulama adımıdır. Gerçek soğuk iz bu ölçümün dışında.
- İlk sonuçtan ayrı olarak tekli güncelleme dizisi her kolda 24 mevcut anahtarı aynı anahtarla günceller. Dört belge boyutu bandında altı örnek, benzersiz marker ve yeni içeriğin tam eşitliği sorguda kontrol edilir. İlk örnekte eski snapshot güncellemeyi görmez. FDB sürüm koşullu Put, Tantivy exact ID Delete+Add ve commit sonrası manuel `reload()`. Commit, reload/yayım ve ilk doğru sorgu dönüşü ayrı kaydedilir.
- Private commit ve working set Windows süreç sayaçlarıdır. İndeksleme tepesinde korpus kataloğu ve her iki motorun süreç içi arka plan işi dahildir. Boşta örnekleri bir okuyucu hazırken ve korpus kataloğu yüklenmeden 2 sn sonra alınır. Bunlar motorun tahsis sayacı veya makinenin toplam RAM'i değildir.
- FDB'nin küçük akışlı WAL katmanı opt-in olduğundan bu koşuda varsayılan kapalı. Güncelleme sayıları eski opt-in WAL raporlarıyla aynı ayarı temsil etmez. Açılış örneklerinde n=12 olduğundan nearest-rank p95 maksimumdur. Temiz yerleşimden gerçek soğuk açılış, uzun churn veya eşzamanlı yazma altındaki sorgu performansı çıkarılmaz.


## Kesintiler

Tantivy'nin ilk iki seed denemesinde indeksleme işçisi durdu. İlkinde
alt hata yok, ikincisinde eklenen tanı anahtar 15814'ü kaydetti ancak
işçi hatası hâlâ geneldi. İkinci blokta iki seed denemesinde
`.store.temp` yolu ile Windows OS 5 `Access is denied` döndü.
Başarılı iki seed aynı ayarlarla tamamlandı ve süre tablosunda yer alır.
Toplam altı Tantivy seed girişiminin ikisi tamamlandı. Başarı oranı
makinenin bu oturumuna ait gözlem.
Başarısız seed'ler geçmişe dönük eşik uygulanmadan ayrıca tutuldu.

İlk Tantivy güncelleme dizisi 11 başarılı örnekten sonra `.del` yolu
ile OS 5 hatası verdi. Yeniden açıldığında 12. marker görünür değildi
ve aynı kayıt yeniden gönderildi. Hatalı denemenin süresi ve elle
yeniden başlatma aralığı, başarılı çağrıların dağılımına eklenmedi.
Hangi Windows bileşeninin erişimi engellediği, ayrıca `.del`/`.store.temp`
dosyasını açma ile bu işlemden önceki genel yönetilen-dosya kaydından
hangisinin hatayı döndürdüğü bilinmiyor.

[İlk seed hatası](benchmarks/tantivy-comparison-50k-20261116/0-tantivy-seed-initial-failure.stderr) · [İşçi tanısı](benchmarks/tantivy-comparison-50k-20261116/diagnostic-tantivy-seed.stderr) · [İkinci blok seed hatası](benchmarks/tantivy-comparison-50k-20261116/1-tantivy-seed-before-resume.stderr) · [İkinci seed hatası](benchmarks/tantivy-comparison-50k-20261116/1-tantivy-seed-initial-failure.stderr) · [Güncelleme hatası](benchmarks/tantivy-comparison-50k-20261116/0-tantivy-visibility.stderr)

## Ham kanıt

16:23:03–16:43:58 UTC. Son Tantivy slotunda güncelleme deneyi
sonrasındaki yaklaşık 137,01 MB tutuluyor. FDB geçici alt klasörü
kaldırıldı. Payload SHA ve hazır slot kaydı aynı kaldı.
İki aracın derleme dosyalarının bulunduğu geçici klasör yaklaşık
995,41 MiB. Derleme klasörü korunuyor.

Kök paket ve karşılaştırıcı için format ve Clippy kontrolleri geçti.
Karşılaştırıcı testi geçti. Farklı ikili hash'leri ilk hatanın işçi
tanısını ve kesintiden sonraki güncellemenin kontrolünü ekleyen harness
derlemelerine aittir. İki motorda da bu kapsamda process-kill deneyi
çalıştırılmadı.

[Çalışma planı](benchmarks/tantivy-comparison-50k-20261116/plan.json) · [Devam kaydı](benchmarks/tantivy-comparison-50k-20261116/resume.json) · [Güncelleme devam kaydı](benchmarks/tantivy-comparison-50k-20261116/visibility-resume.json) · [Kapanış kaydı](benchmarks/tantivy-comparison-50k-20261116/complete.json) · [Sürücü](../tools/native_tantivy/run.ps1) · [Arama adaptörü](../tools/native_tantivy/src/tantivy_arm.rs)
