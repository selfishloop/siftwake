# Arka planda açılış / ilk kullanılabilir sonuç — 50k, 28 Ekim 2026

Son kullanıcı kararı: açılış50k ilk arama≤1000ms,100k≤2000ms; kaynak başına
lazy şimdilik yapılmayacak, bütün doğrulama korunacak. Bu belgedeki300ms/lazy
kararı tarihsel kalır. [Güncel melez/cancel duruşu](HYBRID-CANCEL-50K-20261028.md)
ve [yeni corpus offline analizi](TRWIKI-OFFLINE-20261028.md): yeni corpus DB
ölçülmedi; eski sonuçlara yeni kapı uygulamak yeni corpus kanıtı değildir.


**KALDI: ilk arama≤300ms kapısı geçmedi. Burada duruldu.** Oturumun17–23ms'de
Pending dönmesi başarı diye sunulmaz. Kaynak CRC/schema, SEG05 dizin çözümleme
ve indeks CRC gerçekten arka plana taşındı, atlanmadı. İlk sonuçtan önce bütün
strict kontrolünü bekleyen bu ilk prototip kaynak başına progressive/lazy sonuç
vermez. Daha sonraki50k yazma/dirty/4R/uzun-snapshot/soğuk-cache işleri başlamadı.

Ön koşul [capped/exact arama kapısı](CAPPED-EXACT-50K-20261028.md) geçti:
CRC korunan4 işçili exact max141,766ms, capped N50 max99,831ms. Açılış
başarısızlığı bu kazanımları geri almadı; default exact/strict aynı kaldı.

## Prototipin gerçek sözleşmesi

`OpeningDatabase::open_for_search` foreground'da mevcut MAN03 CRC/bounds,
DB lock, kaynak native başlık/boyut ve immutable lease/pin'leri alır. En fazla4
işçiye sahip background, kaynağın tam native CRC/schema kontrolü ve mevcut
SEG05/indeks CRC/MET04 karşılaştırmasını yapar. Tüm işler bitene kadar sağlık
Pending; yazı API'si yok. `try_finish` aynı Database, lock ve kaynak pin'lerini
teslim eder. `search_first` hazır durumu bekleyip CRC korunan4 işçiyle N50 verir.

Bu ilk sürümün tercihi, onaylı per-source progressive tasarımın daha muhafazakâr
bir alt adımıdır: **kullanılmayan kaynaklar da bitmeden sonuç dönmez**. Dolayısıyla
bu koşulda ilk sonucun sağlık durumu Verified'dır; Pending sonuç gösterme/
sonradan geçersiz kılma güvenlik tavizi alınmadı. Performans bedeli tabloda.
Gerçek per-source lazy açılış/öncelikli doğrulama henüz uygulanmadı.

Sadece temiz45-byte aktif WAL + native immutable run + mevcut SEG/MET04 cache
kapsanır. Dirty WAL/legacy kaynak bu constructor'da açıkça reddedilir; normal
strict API bunları okumaya devam eder. Eksik/bozuk cache background'da rebuild
edilmez, `OpeningFailure::RebuildRequired`, kodu **ERR_REBUILD**. Kaynak onarılmaz.
Sorgu hataları QUERY_ERROR olarak ayrı kalır; sağlık bozmaz. Runtime corruption/
I/O/poison sağlığı Failed yapar. Worker panic yakalanıp bekleyenler uyandırılır.

`Drop` halen worker'ları join eder; nonblocking cancel/drop süresi garantisi ve
progressive doğrulamada write enable henüz yok.4 worker bütçesi query deneyleriyle
ortak; bu normal default search'ün tüm thread'lerini sınırlayan global ürün
scheduler'ı değildir. Worker başına immutable-source işleri sırayla ve dinamik
paylaşılır; kaynak bazında sonuçlara göre yük dengesi yapılır. I/O bandwidth ve
çok soğuk disk kuyruk etkisi bu tur ölçülmedi.

## Veri, sıra, cache ve zaman sınırı

Aynı mevcut ilk50k/sequence104/100 deleted; sabit FDB/SQLite slotları; kaynak/
model SHA aynı. Yeni corpus, DB kopyası/reset veya format yok. N=50, scope shared,
collection texts; returned payload/version ve lower-bound total önceki oracle ile.

Dört yeni-proses tekrar/sorgu/kol: strict/background/SQLite, ters sıra, sonra
aynı iki sıra tekrar. Sorgu sırası döndürülür. **n=4'te nearest-rank p95 max'a
eşittir; üretim p95 güven aralığı değildir.** Geçmediğini görmek için yeterlidir;
kötü örnekler atılmadı. Saat API çağrısından başlar; shell/proses spawn süresi
hariçtir. Oturum dönüşü, ilk cevap, bütün doğrulama bitişi ayrı ölçülür. Oracle,
corpus/model yükleme ve tam50k get kontrolü ilk-result sayacının dışındadır.
Bellek ilk cevapta oracle yüklenmeden yakalanır; sonuç belgesi Vec'i dahildir.

DB'ler önceki testlerde kullanılmıştır; OS cache zorla boşaltılmadı. FDB kaynak
SHA kontrolü ve önceki oracle OS'i kısmen ısıtır. **Her koldan hemen önce bütün
DB dosyalarını eşit biçimde warm etme yapılmadı; OS residency garanti değildir.**
Bunlar yeni engine cache'li ilk-sorgu ölçümleri; ne tam kontrollü sıcak-cache ne
gerçek-soğuk-cache kapısı sayılır. İstenen gerçek cache eviction işi hâlâ açık.
Bu sınır sonuçların üstünden atlanmaz;300ms başarısı iddia edilmez.

SQLite aynı FTS5 trigram/WAL/FULL/cache2MiB/mmap0/tempFILE ayarlarıyla ve aynı
payload/version modelinde; COUNT çıkarılmış, id sıralı ilk50 SQL. SQLite'ın
source-wide strict kontrolü yoktur. Yeni connection'ın ilk SELECT'i, önceki
sıcak/query-warmup tablosundan çok daha yavaştır; plan/I/O kök nedeni bu tur
ayrıştırılmadı. Bunu FDB'nin bütünlük garantileriyle eşdeğer açılış diye sunmayız.

## İlk sonuç ve doğrulama — p95/ms

| Sorgu | Oturum BG | İlk sonuç BG | Tam doğrulama BG | Oturum strict | İlk sonuç strict | SQLite open | SQLite ilk sonuç |
|---|---:|---:|---:|---:|---:|---:|---:|
| harpoon | 21.501 | 589.571 | 560.384 | 1886.257 | 1917.954 | 3.759 | 7585.120 |
| presumption | 22.090 | 732.096 | 678.088 | 1867.913 | 1922.772 | 2.339 | 8204.131 |
| darcy | 20.120 | 564.196 | 539.685 | 1849.504 | 1873.564 | 2.468 | 7259.471 |
| transubstantiation | 22.681 | 756.744 | 687.371 | 1606.277 | 1671.485 | 2.484 | 6550.609 |
| holmes elementary | 18.138 | 389.845 | 342.971 | 1416.380 | 1465.456 | 2.384 | 4529.457 |

BG ilk cevap örneklerinin tamamı **306.268–756.744ms**. Boş handle
çabuk dönüyor; uzun kalan iş ilk aramaya taşınıyor. Kaynak başına öncelik/erken
sonuç bu beklemeyi azaltma adayıdır; bu raporda uygulanmış sayılmaz.

## CPU ve bellek

| Sorgu | BG ortalama toplam CPU ms | Strict ortalama CPU ms | SQLite ortalama CPU ms | BG ilk-cevap lifetime peak private/WS MiB |
|---|---:|---:|---:|---:|
| harpoon | 1093.75 | 902.34 | 2410.16 | 8.54 / 12.68 |
| presumption | 1136.72 | 1066.41 | 2656.25 | 9.10 / 13.13 |
| darcy | 988.28 | 937.50 | 2265.62 | 8.73 / 12.23 |
| transubstantiation | 1214.84 | 1062.50 | 2023.44 | 8.31 / 12.17 |
| holmes elementary | 1156.25 | 1003.91 | 1398.44 | 8.99 / 13.13 |

CPU kernel+user bütün worker thread'lerini kapsar; kuantalama nedeniyle tek
örnekten pay atfedilmez. BG toplam CPU azalmıyor; wall latency paralelleşiyor.
İlk cevap tepe belleği temiz idle ölçümü değildir.50k oracle descriptor'ları
bu sayaca yüklenmemiştir. Sonucu caller ne kadar tutarsa o kadar bellek tutar.

## Aynı profil kaydında kontrolün nereye gittiği

Ayrı diagnostic derleme, harpoon ilk sorgu, birer örnek. Bu değerler normal
performans p95 yerine kullanılmaz. Paralel kaynak süreleri üst üste biner;
sütunları toplam wall veya yüzde diye toplamak yanlıştır.

| Aşama | Strict toplam span ms | Background toplam span ms |
|---|---:|---:|
| Worker open wall | 836.137 | 273.715 |
| Kaynak native/CRC/schema | 513.273 | 624.152 |
| İndeks (alt kalemleri içerir) | 297.930 | 349.280 |
| ↳ SEG05 directory | 179.979 | 201.312 |
| ↳ İndeks CRC | 98.496 | 127.070 |
| ↳ Term page checksums | 9.925 | 12.004 |

BG worker open273,7ms, normal836,1ms olan diagnostic örnekte bile foreground
hazırlığı ve ilk query eklenir. Normal ölçümlerde full doğrulama tepe687,4ms'ye
çıktı. Kaynak+directory+index CRC kontrolleri kaldırılmadı; hepsi deadline'ın
önünde kalıyor. Bunun hangi kısmının scheduler/I/O dalgalanması olduğu ayrı
ölçülmedi. Sonraki dar öneri: onaylı tasarımdaki **kaynak başına lazy index açma,
sorgunun gerektirdiği kaynağı öncelikli strict hazırlama**, kalan kaynakları
background'da tamamlamak. Kaynak/indeks doğrulamasını sessizce atlama önerisi yok.
Bu tur bu yeni düzeltmeye başlanmadı; başarısız kapı sonrası kullanıcı kararı gerekir.

## Kapı ve doğruluk

| Kapı | Durum |
|---|---|
| Query exact4/capped4≤150ms | GEÇTİ, ayrı raporda |
| Background ilk arama≤300ms | **KALDI**, beş sorgunun her birinde aşıldı |
| Oturum dönüşü / ilk sonuç / bütün doğrulama ayrı | GEÇTİ; Pending handle sonuç sayılmadı |
|50k full payload/version/deleted ve38 sorgu/prefix oracle | GEÇTİ; kill öncesi/sonrası |
| Background Pending iken process kill/reopen | GEÇTİ; mevcut salt-okuma oturumu, power-loss iddiası yok |
| Donmuş24Ekim okuyucu full50k get/deleted/38 original query | GEÇTİ |
| Kaynak cut/tek-byte, sağlam cache+bozuk kaynak, cache eksik | Yeni unit testleri; son doğrulama eki aşağıda |
| Format/corpus/fixture/source/model koruması | GEÇTİ; hash eşit |

Kill Pending marker'ından sonra OS process kill ile yapıldı; yeniden açılışta
tam model ve38 metnin prefix'leri doğrulandı. Yeni corpus/churn yazısı
üretilmedi. WAL/flush/merge process-kill yolları genel test paketinde ayrıca
tekrar koşar;50k üzerinde bu tur yeniden ingest/merge kill fixture'ı yapılmadı.
38-case first oracle offset0/N50 ile çalışır; normal pagination'ın38 özgün
case'i donmuş eski okuyucuyla ayrıca geçti. N1/N200 küçük çok-run/delta/snapshot
unit oracle'ında test edildi;50k'da bu iki limit için performans iddiası yok.

Tam doğrulama öncesi kullanılan bir kaynakla sınırlı strict-check/Pending
cevap mimarisi, eşzamanlı first-source kontrolü ve progressive cancel henüz
uygulanmadığı için bu özelliklerin kapısı geçti denmez. Runtime data pinleri
ve snapshot transferi küçük unit testinde;10dk uzun snapshot bu test değildir.

Ham kanıt: [tasarım/sıra/program SHA ve koruma](benchmarks/block2-background-open-50k-20261028-v1-design.json),
[bütün ilk-result örnekleri/CPU/bellek](benchmarks/block2-background-open-50k-20261028-v1-analysis.json),
[Pending kill witness](benchmarks/block2-background-open-50k-20261028-v1-kill-opening.jsonl),
[eski okuyucu tam oracle](benchmarks/block2-background-open-50k-20261028-v1-compat-old-reader-utf8-v2.json).
Ham ilk-result dosyaları `...-r{0..3}-q{0..4}-*-first-*.jsonl`; kaynak ve indeks
profilleri `...-profile-diagnostic-first-{background|strict}.jsonl`.

Reclaim default kapalı. Case34, geniş zx/the zx/parçalı okumalar, sorgu RAM'i,
50k yazma/dirty500/4R/100'lü profili,2c uzun snapshot ve2b gerçek-soğuk açık.
100k gerçek dosya corpus'unu bekler. Blok3/4, C ABI ve entegrasyon başlamadı.

## Son doğrulama — 28 Ekim

Default `cargo test --all-targets`: **237 geçti,0 başarısız,14 ignored subprocess
yardımcı**. Diagnostic lib: **143 geçti,0 başarısız,12 ignored yardımcı**.
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, diagnostic
clippy, no-default lib ve all-features lib check; native SQLite fmt/clippy/test
geçti. Yeni capped/parallel/coalesced/CRC seçenekleri küçük çok-run, range/link,
delta güncelleme/silme, snapshot sabit epoch ve reopen oracle'ında karşılaştırıldı.
Eski0.1 kaynak kopyası yeni API'lerle okunuyor; fixture'ın kendisi korunuyor.
Sağlam cache+bozuk kaynak testi ve streamed süreç kill sonrası capped/exact4
sonuç kontrolleri genişletildi; bütün eski WAL/flush/merge/cache kill aileleri
pakette tekrar geçti. Güç kesintisi garantisi iddia edilmedi.

50k yeni API oracle'ı38 case için kill öncesi ve sonrası; Pending background
oturumunda gözlenmiş process kill; donmuş24 Ekim okuyucuyla50k payload/version,
100 silinmiş get ve38 özgün pagination sorgusu tekrar geçti.50k'da bu tur yeni
churn/ingest/merge-kill yapılmadı. Corpus,0.1 fixture, .fdb/.ready ve aynı-model
ledger SHA'ları eşit; kalıcı biçim ve varsayılanlar korunur.

[Komutlar/exit code ve loglar](benchmarks/snapshot-10k-20261026-validation-capped-background-50k-20261028-v1.json),
[diagnostic test](benchmarks/block2-capped-background-50k-20261028-v1-diagnostic-tests.log),
[diagnostic clippy](benchmarks/block2-capped-background-50k-20261028-v1-diagnostic-clippy.log).
