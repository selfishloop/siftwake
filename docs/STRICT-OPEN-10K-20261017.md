# Tam doğrulamalı açılış — aynı 10k, 2026-10-17

Normal `Database::open` açılışı dört dengeli süreç çiftinde **430,303 →
270,116 ms (%37,23 düşük)** oldu; dört çiftin dördü hızlı. Doğrulama ilk
sorguya ertelenmedi: açılış tamamlandığında bekleyen kaynak kontrolü **0**.
Bu dar iyileştirme SQLite'ın normal açılış seviyesine ulaşıldığı anlamına
gelmez. RAM/disk ve diğer açık performans işleri ayrı durur.

## Değişiklik ve korunan kontroller

Yayınlanmış FDBLOG04 Run kaynaklarında önce ayrı bütün-dosya CRC geçişi,
ardından frame doğrulaması ve her belgeyi sahipli `Document`/JSON değerlerine
çeviren replay çalışıyordu. Sağlam indeks zaten mevcut olsa da belge
nesneleri kurulup hemen bırakılıyordu.

Yeni yol kaynak dosyayı tek sınırlı geçişte okur. Frame CRC'si üzerinden
birleştirilen bütün-kaynak CRC'sini `.ready` içindeki bağımsız checksum ile
karşılaştırır. Frame/header/footer, uzunluk, işlem sırası, entry tipi/sınırı,
UTF-8, ikili alan yapısı, belge/key/link limitleri ve version kontrolleri
sürer. `payload::checked` görünümüyle belge sahiplenilmeden doğrulanır.
64 KiB okuma tamponu ve tekrar kullanılan entry tamponu vardır; bir büyük
entry'nin kapasitesi sonraki entry'lerde sınırsız tutulmaz. Belgenin kalan
bölümlerinin doğrulanması atlanmaz.

İndeks ancak kaynak doğrulandıktan sonra açılır. Sağlam cache varsa replay
atlatılır; cache eksik/bozuksa mevcut replay/builder yeniden üretme yolu
çalışır. Kaynak ve indeks sequence kimliği ayrıca karşılaştırılır. Yeni
Run üretimi, WAL, eski JSON/0.1 kaynaklar ve referans profil mevcut
yollarındadır. Windows kaynak handle sahipliği korunur. Kalıcı format,
feature, bağımlılık ve protokol değişmedi.

## Normal derleme ölçümü

Başlangıç commit'i `86a3928`; iki sürüm de normal release/default
`bounded-engine`, diagnostic sayaçlar kapalı. Dört çiftte ilk/ikinci sıra
dengeli; her moddan önce ayrı ölçümsüz ısınma var. Yeni süreçler, sıcak OS
cache; eviction, soğuk-disk veya eşzamanlı yük yok. Derleme/test ölçümlerle
çakışmadı. Süreler süreç medyanı; toplam her süreçte hesaplanıp sonra
medyan alınır.

| Normal strict yol | Önce | Sonra | Dört çift |
|---|---:|---:|---|
| Yalnız açılış |430,303 ms |270,116 ms |4/4 düşük |
| Ayrı ilk-kullanım serisinde açılış |419,134 ms |277,020 ms |4/4 düşük |
| İlk get9999 |0,109 ms |0,086 ms |4/4 düşük |
| İlk `quixote`, offset0/limit50 |4,714 ms |4,637 ms |4/4 düşük |
| Aynı arama tekrar |2,553 ms |2,548 ms |1/4 düşük; kazanç iddiası yok |
| Açılış+ilk get+ilk arama |423,924 ms |281,714 ms |-%33,55;4/4 düşük |

`quixote` total56 ve dönen tam belgeler, get9999 version20 doğrulandı.
Tüm süreçlerde10.000 belge, sequence20,5 segment, delta0; strict açılışta
`source_validation_on_open=true`, `pending_source_checks=0`.

Opt-in `open_record_reads` kontrolünde toplam408,445→409,019 ms (+%0,14,
2/4 düşük); bu yolun algoritması değişmedi ve kazanç iddia edilmiyor.
İlk aramasında kaynak doğrulaması hâlâ yaklaşık240 ms tutuyor.

Ayrı dört yeni-süreç SQLite referansında normal open1,703 ms,
açılış+ilk get+ilk search14,095 ms medyan. Native SQLite3.53.2,
page cache2 MiB/mmap0/prepared cache64; açılışta tam integrity yok.
Bu referans FDB'nin dört dengeli eski/yeni çiftine eklenmiş veya eşit
doğrulama işi sayılmış değildir. **Kullanıcı gecikmesinde büyük açılış
farkı hâlâ var.** [Referans koşulları ve hash](benchmarks/open-strict-sqlite-reference-20261017.json).

Oracle/corpus yüklenmeyen ayrı iki dengeli query/get proses çiftinde24
arama/771 get doğrulandı. OS lifetime peak private commit9,197→8,637 MiB,
working set12,855→12,549 MiB. Bunlar ölçülen prosesin tepe sayaçlarıdır;
toplam sistem RAM'i, OS dosya cache'i veya normal Vec ingest belleği değildir.
Disk dosyaları byte/hash olarak aynı; bu adım disk alanını küçültmez.

[Özet ve çiftler](benchmarks/open-strict-isolated-10k-20261017-summary.json),
[koşullar](benchmarks/open-strict-isolated-10k-20261017-design.json),
[dondurulmuş executable hash'leri](benchmarks/open-strict-isolated-10k-20261017-executables.json).

## Kalan açılış maliyetinin profili

Ayrı `diagnostic-timing` derlemesinin sıcak son örneğinde yeni toplam
266,874 ms; kaynak tek-geçiş kontrolü100,024 ms, indeks açma161,909 ms,
belge replay span'i yaklaşık0 ms. İndeks açmanın içindeki CRC90,886 ms;
bu iç süre161,909 ms'ye yeniden eklenmez. Yaklaşık71 ms kalan indeks
hazırlığı vardır. Beş kaynak tamamen denetlendi, pending0.

Önceki sıcak profil519,140 ms toplam; ayrı kaynak CRC79,584 ms,
sahipli replay243,222 ms, indeks189,912 ms idi. Profiller farklı anlarda
alınmış tekil instrumented teşhislerdir; buradan indeks hızlanması veya
normal derleme için yeni bir yüzde üretilmez. Normal performans kanıtı
yukarıdaki dört dengeli çifttir. Kaldırılan kaynak replay işi ve kalan
indeks maliyeti bu profille görünür; sonraki açılış adımı indeks CRC ve
directory hazırlığındaki tekrarları incelemektir.

[Önceki sıcak profil](benchmarks/open-strict-profile-control-warm-1-20261017.jsonl),
[yeni sıcak profil](benchmarks/open-strict-profile-candidate-warm-1-20261017.jsonl).
Profil sonrası gerçek ürün ve okuma araçları diagnostic kapalı normal
release olarak yeniden derlendi.

## Arama gerilemesi kontrolü ve deneylerin kaydı

İlk çalışan sürüm açılışta423,277→278,194 ms kazandırdı. Ancak geniş
aramada8 düşük/30 yüksek p95 verdi; bazı artışlar4/4 tekrarda aynı yöndeydi.
Bu sonuç dalgalanma diye kapatılmadı. Açılışa özgü iki doğrulayıcıya
`cold`/`inline(never)` sınırı eklendi; sorgu algoritması değiştirilmedi.
Derleyici yerleşiminin ilk farkın kesin nedeni olduğu kanıtlanmış değildir.

Son sürümün normal strict açılışla38 durumluk dört çiftinde23 p95 düşük,
15 yüksek; yükseklerin hiçbiri4/4 aynı yönde değil. `quixote /57/50`
+%26,27, `/28/10` +%15,97; bu ek sayfalarda süreç başına yalnız5 örnek
olduğu için yoğun kontrol ayrıca gerekir. Hiçbir genel arama kazancı veya
bütün iş yüklerinde sıfır gerileme iddia edilmiyor.

[İlk sürümün matrisi](benchmarks/open-strict-read-matrix-10k-20261017-summary.json),
[son sürümün38 durumluk matrisi](benchmarks/open-strict-isolated-matrix-10k-20261017-summary.json).

Yoğun strict sayfa kontrolünde aynı CPU'ya sabitlenmiş dört çift,200
örnek/süreç/sayfa, toplam800 örnek/sürüm/sayfa kullanıldı. Sekiz sayfanın
tamamında pooled p95 yüksek; hiçbirinde4/4 aynı yönde değil:

| Sayfa | Son sürüm / kontrol p95 farkı | Hızlı çift | Aynı executable A/A farkı |
|---|---:|---:|---:|
| `the /0/1` |+%2,31 |1/4 |+%2,47 |
| `the /0/200` |+%2,40 |1/4 |+%3,52 |
| `the /5000/50` |+%1,13 |1/4 |+%1,18 |
| `the /9950/50` |+%2,02 |2/4 |-%1,71 |
| `ishmael /130/1` |+%2,93 |1/4 |+%4,81 |
| `ishmael /60/200` |+%6,22 |1/4 |+%1,92 |
| `quixote /28/10` |+%3,31 |1/4 |+%6,74 |
| `quixote /57/50` |+%2,15 |2/4 |+%6,07 |

Aynı kontrol binary'si iki etiket altında, aynı CPU ve aynı yoğun düzende
ayrıca dört çift çalıştırıldı. Kod eşitken7/8 p95 yüksek, `the /0/1`,
`the /0/200` ve `quixote /57/50`4/4 yüksek olabiliyor. Bu negatif kontrol
ortam/oturum oynaklığını kanıtlar; iki ayrı serinin yüzdeleri birbirinden
çıkarılamaz ve yeni kodun katkısının sıfır olduğunu kanıtlamaz. **Arama
gerilemesi işi kapatılmadı; bu değişikliğin kabul edilen sonucu dar strict
açılış kazancıdır.** Yoğun koşunun8 proses exit code'u0, hash'ler aynı.
Dış çağıran satırının kullanılmayan `$LASTEXITCODE` kontrolü yanlış hata
bildirdi; proses sonuçları denetlendikten sonra özet ayrı üretildi,
ölçümler yeniden çalıştırılmış veya değiştirilmiş değildir.

[Yoğun A/B](benchmarks/open-strict-dense-pages-10k-20261017-summary.json),
[aynı executable A/A](benchmarks/open-strict-dense-aa-10k-20261017-summary.json).

Başlangıçtaki `open-strict-native-10k-20261017` serisinde PowerShell'in
tek string argümanı karakterlere ayırması nedeniyle strict-first probe'u
yerine genel okuma probe'u çalışmıştı. O seri ilk-kullanım kapısından
çıkarıldı; ham loglar tutuldu. Runner artık argüman dizisi ve beklenen
probe fazlarını denetler. Düzeltilmiş `native-checked` ve son `isolated`
serileri doğru modlarla yeniden ölçüldü.

## Corpus, veri sahipliği ve açık sınırlar

Mevcut `data/rkyv-corpus-20261009/json.payload-bench` dosyasının aynı ilk
10.000 belgesi:264.842.858 B gövde,2.057–51.197 B/pasaj, sekiz kitaptan
farklı metinler. Yeni corpus üretilmedi/indirilmedi. Yalnız mevcut sabit
`data/benchmark-work/fdb` ve `/sqlite` kullanıldı; DB reseti veya kopyası
yok. Okuma serilerinden önce/sonra kaynak, indeks, yardımcı cache ve
SQLite DB hash'leri değişmedi.

Mevcut layout'ta canlı source267.900.709 B (45 B WAL dahil),
indeks353.688.186 B; ready dahil yaklaşık621,6 MB. Retained1,52 GB'ın
tamamı normal açılışta taranmıyor. Eski nesilleri temizlemek disk işidir;
bu kaynak/indeks doğrulama maliyetini tek başına kaldırmaz.

Kalan strict açılış işi kaynak kontrolü yanında indeks CRC'si ve disk
sözlüğü hazırlığıdır. SQLite normal open bütün DB'yi doğrulamaz; mevcut
strict sözleşmeyle eşit iş değildir. Bu fark, ölçülen kullanıcı gecikmesi
farkını ortadan kaldırmaz. Yazma/bakım tekrarları, normal Vec RAM'i,
arama artışları, retained/live disk farkı ve concurrent/gerçek-cold yük
[OPEN-WORK](OPEN-WORK.md) içinde açık tutulur. SDB/Fundops/Frunner
entegrasyonu yapılmadı;0.1 fixture değişmedi.

## Son doğrulama

Son kodda `cargo test --all-targets`, `cargo test --no-default-features
--all-targets` ve `cargo test --all-features --all-targets` geçti. Normal
profil154, referans93, tüm özellikler173 test;4/2/6 ignored test ayrı
çalıştırılan child-process helper girişleridir. `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` ve tüm özelliklerle Clippy geçti.
Gerçek normal `fdb.exe` ile sabit10k slotta stdio stats/get/search geçti;
sonrasında134 dosyanın hash'i tekrar aynı bulundu. Normal yeniden
derlenen reader'ın `.text` makine kodu ölçülen adayla byte-eşit. Bütün EXE
hash'leri eşit değil: `.rdata` içinde7 byte fark var; makine kodu eşitliği
bütün dosya eşitliği diye sunulmaz.

Yeni tek-geçiş testleri çok frame, boş checkpoint, tombstone, Unicode ve
nested alanları; her kesilme ve her tek-bayt bozulmasını denetler. Frame
CRC'leri düzeltilen değişikliğin bağımsız kaynak checksum'ı ile reddi,
geçerli CRC altında yanlış belge/key/link/version/limit ve CRC'si
düzeltilmiş yanlış indeks sequence kimliği ayrıca test edildi. Mevcut
cache rebuild, sağlam cache altında bozuk kaynak,0.1 JSON okuma,
commit/merge/yardımcı cache yayınında süreç sonlandırma testleri geçti.
Process kill güç kesintisi simülasyonu değildir.

İlk geliştirme koşusunda yeni Run'ların henüz yayınlanmamış metadata'sı
nedeniyle compact testi hata yakaladı. Hızlı doğrulama yalnız `.ready` ile
kimliği bilinen Run'lara sınırlandı; yeni Run/builder yolu korunup bütün
suiteler tekrar geçti. İlk hata logu silinmedi.

Corpus'un2.740.862.279 B boyutu ve SHA256'sı
`11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B`
aynı.134 sabit-slot dosyasının güncel hash'leri ölçüm öncesiyle aynı;
0.1 fixture byte'ları Git kaydıyla eşit. Root manifest/lock/feature listesi
değişmedi. SQLite'ın bu tur tam integrity testi yeniden çalıştırılmadı;
önceki raporun integrity sonucu yeni ölçüm gibi sunulmaz.

[Son doğrulama ve kaynak/corpus/fixture kanıtı](benchmarks/open-strict-final-proof-20261017.json),
[normal testler](benchmarks/open-strict-final-tests-default-final-20261017.log),
[referans testleri](benchmarks/open-strict-final-tests-reference-20261017.log),
[tüm özellikler](benchmarks/open-strict-final-tests-all-20261017.log),
[gerçek normal ürün kontrolü](benchmarks/open-strict-final-product-cli-20261017.json).
