# Yazma ve bakım: checkpoint kimlik eşlemesi — 18–19 Ekim 2026

Normal motorun belge anahtarını her posting girdisinde tekrar araması kaldırıldı.
Kalıcı kimlikler ve dosya byte'ları aynı kalır. Genel arama/RAM kabulü, önceki
sonuç tamponu geçişi ve diğer açık işler bu dar değişiklikle kapanmaz.

## Kök neden ve teşhis

Aynı mevcut ilk10k belgede birer diagnostic-timing koşusu/API/sürüm. İç içe
ve worker wall span'leri örtüşür; toplam CPU yüzdesi gibi toplanamaz. Giriş
CRC/JSON çözümü ayrıca ölçüldü. Bu profil normal release hız tablosu değildir.

İlk akışlı profilde `body.grams`6,320s, `index.insert_grams_and_text`5,667s,
`builder.finish_delta`7,262s; WAL kopyalama+sync0,434s (içindeki sync0,194s).
Giriş corpus CRC/JSON1,448s. Toplu API'de toplam delta aktarımı8,270s idi.
Belge kodlama/yeniden çözme, spool ve kaynak doğrulama ölçümleri ham profilde
ayrı durur; bunların hiçbiri varsayımla sıfır kabul edilmedi.

Eski `Index::disk_ids`, her posting girdisinde RAM slotundan Key'e, oradan
`BTreeMap<Key,u32>` üzerinden checkpoint kimliğine gidiyordu. Aynı belge çok
sayıda üçlü parçada bulunduğundan aynı ağaç araması yeniden yapılıyordu.
Yeni profilde19.221.340 posting girdisi için yalnız10.000 Key araması var.
Eski kodun girdiye bir araması, yeni sayaçlarla sayılan aynı iş hacmiyle
birlikte değerlendirilir; eski koşuda ayrı Key arama sayacı yoktu.

| İç içe diagnostic aşaması | Önce | Sonra |
|---|---:|---:|
| Akışlı delta aktarımı,20 çağrı |7,262s|1,695s|
| Toplu API delta aktarımı,10 çağrı |8,270s|1,136s|

[Dört ham profil ve sayaçlar](benchmarks/write-stages-10k-20261018-profile-summary.json).
Enstrümantasyon farkları nedeniyle bu tablo genel hız kazancı değildir.

## Değişiklik ve sınırlar

Normal `bounded-engine` profili aktarım başında RAM slotu→checkpoint ID
tablosunu bir kez kurar. Her canlı slotun anahtarı ve hedef sınırı kontrol
edilir. Boş/silinmiş slotlar sentinel kalır; bilinmeyen slot reddedilir.
Posting listesinin yeni kimlikleri yine sıralanıp yinelenen kimlik kontrolünden
geçer. Silme/update sonrası yeniden kullanılan slotlar kalıcı sıralama sanılmaz.
Tombstone metadata girdileri canlı RAM slotu gerektirmez.

Tablo slot başına4 byte geçici Vec'tir; sorgu önbelleğine veya diske saklanmaz.
Boş girdi Vec tahsis etmez. Akışlı500-belgelik işlemde eleman alanı2000 byte'tır;
bu bir toplam proses RAM sınırı veya RAM kazancı iddiası değildir. Referans ve
bounded olmayan `experimental-merge-postings` profili eski ağaç eşlemesini korur.

WAL sync, transaction boyutu, kaynak/manifest/cache/payload biçimleri,
CRC/schema/UTF-8/kimlik kontrolleri, feature listesi ve bağımlılıklar değişmedi.

## Normal release ölçümü

Dört akışlı ve iki toplu/owned süreç çifti; her çiftte eski/yeni FDB ve SQLite
çalıştı. Sıra bitişik çiftlerde ters çevrildi. Diagnostic timing/allocation
kapalı. OS cache sıcak; eviction, güç veya donanım izolasyonu yok. Giriş
CRC/JSON çözümü,20 senkron WAL işlemi ve40 sınır belge/get kontrolü yazma
süresinde; her süreçten sonra10k tam belge kontrolü zaman dışında. Yazma
sırasında flush/merge çalışmaları da vardır; son bakım yalnız son drain'dir.

| Medyan, saniye | Eski FDB | Yeni FDB | SQLite | Yeni/eski |
|---|---:|---:|---:|---:|
| Akışlı yazma |26,512|21,567|42,832|-%18,65;4/4 hızlı|
| Akışlı son bakım |0,754|0,761|1,411|+%0,96;2/4 hızlı|
| Akışlı yazma+bakım |27,277|22,334|44,242|**-%18,12;4/4 hızlı**|
| Akışlı proses CPU |24,641|20,297|42,008|-%17,63;4/4 düşük|
| Toplu API yazma |19,432|14,074|35,184|-%27,57;2/2 hızlı|
| Toplu API son bakım |3,842|3,172|1,172|-%17,43;2/2 hızlı|
| Toplu API yazma+bakım |23,274|17,246|36,356|**-%25,90;2/2 hızlı**|
| Toplu API proses CPU |21,656|15,859|34,781|-%26,77;2/2 düşük|

Toplam medyan her prosesin yazma+bakımından hesaplandı; iki faz medyanının
toplamı değildir. Akışlı çiftlerin toplam farkları -%12,40/-16,85/-19,32/-7,92;
owned çiftleri -%24,87/-26,65.2/4 son bakım artışı kapatılmaz: streamed son
merge'de ayrı kazanç kanıtlanmadı. CPU, prosesin bütün thread'lerini kapsayan
OS toplamıdır; profil span'lerinin toplamı değildir.

SQLite: bağımsız native FTS5 trigram, FULL WAL,2MiB page cache,mmap0,tempFILE.
Her işlem500 belge; iki grupta da SQLite girdisi akışlıdır. Owned FDB satırı
aynı caller bellek sözleşmesi anlamına gelmez. SQLite final bakımında FTS
optimize+WAL checkpoint vardır; FDB flush/merge ile aynı algoritma değildir.
FDB'nin tüm alan/link/guard işlevleri SQLite karşılaştırıcısında yoktur.
İki SQLite grubunun farklı medyanları farklı zamanlı koşulardır; birbirine
API optimizasyonu diye oranlanmaz. Dört/iki çift istatistiksel anlamlılık
veya bütün iş yüklerine hız garantisi oluşturmaz.

[Koşullar, sıralar, bütün örnekler ve OS bellek](benchmarks/write-stages-normal-10k-20261018-summary.json).

### Yazma belleği ve disk

OS lifetime private commit fiziksel RAM değildir. Working set paylaşılan image
sayfalarını da içerir. Faz örnekleri kısa tepeleri kaçırabilir; bu tablo süreç
sonunda elde tutulan handle'dan alınmış lifetime tepe sayaçlarını kullanır.

| Medyan OS tepe sayacı | Eski FDB | Yeni FDB | Fark |
|---|---:|---:|---:|
| Akışlı private commit |10.715.136B|10.706.944B|-8KiB;2/4 düşük|
| Akışlı working set |14.661.632B|14.776.320B|+112KiB;3/4 yüksek|
| Owned private commit |83.812.352B|84.129.792B|**+310KiB;2/2 yüksek**|
| Owned working set |87.220.224B|87.685.120B|**+454KiB;2/2 yüksek**|

SQLite private medyanı8.826.880B. Yeni owned FDB private yaklaşık80,23MiB;
normal toplu API RAM sorunu çözülmedi.2–4KiB ID tablosu, gözlenen310KiB OS
artışının tam açıklaması değildir. Bu farkın kaynağı ayrıştırılmadı; RAM kabul
kapısı geçmedi, artış dalgalanma denilerek kapatılmaz.

Bütün FDB koşularında son disk349.805.698B, SQLite583.860.224B. Her API kendi
kontrolüyle kaynak/indeks/MET04/ready SHA-eşit; iki API'nin işlem/nesil düzeni
farklıdır. Bu değişiklik disk küçültmez. Dinamik geçici dosya tepe alanı ve
genel sıkıştırma kapsam dışı/açık.

## Arama ve okuma belleği kontrolü

Aynı son owned yerleşiminde eski/yeni okuyucu,4 dengeli çift×2 kısa blok×10
tur.30 primary sorguda80,8 sayfada24 örnek/okuyucu. Maske4; her blokta açılış,
tam kaynak/indeks doğrulaması ve ısınma zaman dışında. Get'ler sorgularla
karışık; tam belge/sıra/version/sequence oracle'ı her sonuçta zaman dışında.

Geniş38'de11 düşük/27 yüksek pooled p95 var. Genel arama kabulü geçmedi.
Artışlardan seçilen9 senaryo aynı programlarla4 çift×2 blok×25 turda, her
sorguya200 örnekle tekrarlandı. Yoğun tekrarda8 düşük/1 yüksek p95:

| Sorgu / offset / limit | P95 farkı | Daha hızlı süreç çifti |
|---|---:|---:|
| quixote /0/50 |-%2,34|1/4|
| transubstantiation /0/50 |-%3,27|3/4|
| darcy /0/50 |-%1,98|2/4|
| zx /0/50 |-%1,62|2/4|
| İSHMAEL /0/50 |-%2,11|3/4|
| presumption /0/50 |-%5,41|3/4|
| the /0/1 |-%3,51|3/4|
| the /9950/50 |**+%7,45**|1/4|
| ishmael /60/200 |-%1,53|1/4|

Son sayfa1,8581→1,9965ms; tipik blok medyanı -%10,18 ve5/8 blok hızlı.
Thread cycle p50 -%1,43/p95 +%2,14. Bu dağılımlar aynı şeyi ölçmez; kalan
138,4µs p95 artışı kapatılmadı. Diğer sekiz pooled düşüş de bütün çiftlerde
kazanç değildir. Geniş set ile yoğun alt set farklı sorgu/durum dağılımlarıdır.
Sorgu algoritması değişmedi; okuyucu `.text` bölümleri byte-eşit değil ve
kod/yerleşim, oturum durumu ve ortam payları tam ayrıştırılmadı. Cycle sayacı
sabit frekanslı değildir; thread CPU zamanı quantized olabilir. Artışlar
«dalgalanma» diye topluca kapatılmaz.

[Geniş38 ve thread gözlemleri](benchmarks/write-stages-reads-10k-20261018-summary.json),
[yoğun9/200](benchmarks/write-stages-reads-dense-10k-20261018-summary.json),
[PE bölüm kimlikleri](benchmarks/write-stages-10k-20261018-pe-sections.json).

Ayrı oracle'sız proseslerde4 dengeli çift,24 arama/771 get, normal `search`
API'si; açılış dahil OS lifetime medyanları:

| Sayaç | Eski FDB | Yeni FDB | SQLite |
|---|---:|---:|---:|
| Tepe private commit |8,7676MiB|8,7168MiB|12,5820MiB|
| Tepe working set |12,7246MiB|12,6543MiB|16,6758MiB|

Private -52KiB,WS -72KiB; ikisi3/4 çiftte düşük. Bu okuma ölçümüdür; normal
Vec yazma belleğini veya genel RAM kabulünü kapatmaz. FDB strict açılış ve
SQLite ordinary açılış bu proses kapsamına dahildir; bu değişikliğe açılış
algoritması kazancı atfedilmez. Corpus/full payload oracle'ı bu proseslerde
yüklü değildir. [Ham OS sayaçları](benchmarks/write-stages-query-memory-10k-20261018-summary.json).

Güncel SQLite okuma kıyası: aynı son DB'lerde2 ters sıralı çift, normal
release,40 örnek/primary ve10 örnek/sayfa,256 get anahtarı ve missing-key
durumları.38/38 sorguda FDB p95 düşük; her38 durum iki çiftte de hızlı.
Beş get boyut/eksik-key grubunun pooled p95'i de FDB'de düşük. Tam payload,
sıra ve sürümler bağımsız oracle'la doğrulandı. Bu koşuda CPU maskesi
uygulanmadı; warm OS cache ve aynı strict-FDB/ordinary-SQLite açılış kuralları
vardır. Açılış maliyeti warm query süresinde değildir.

Bu sonuç mevcut **FTS5 trigram+LIKE**,2MiB page cache,mmap0,tempFILE ve64
hazır statement cache'li karşılaştırıcı içindir. MATCH veya alternatif SQL
planları/şemaları ölçülmedi; SQLite ürününün en iyi olası hızı diye sunulmaz.
İki çift ve bu38 senaryo genel ürün sıralaması değildir. SQLite karşısındaki
sonuçlar, önceki FDB okuyucusuna karşı kalan p95 artışını kapatmaz.
[Bütün sorgu/get değerleri](benchmarks/write-stages-read-checks-10k-20261018-summary.json),
[ölçüm koşulları](benchmarks/write-stages-read-checks-10k-20261018-design.json).

## Doğrulama ve veri koruma

- Normal tam `cargo test --all-targets`: **193 geçti,0 başarısız,7 ignored**.
  Tüm-feature lib/segments/streamed hedefleri135 geçti/7 ignored; referans
  `--no-default-features --all-targets`112 geçti/3 ignored. Kesilme, bozulma,
  süreç sonlandırma, geri dönüşüm/silme/update ve0.1 fixture kontrolleri geçti.
- Yeni map testleri ters sırada giriş, silme, yeniden kullanılan/boş slot,
  tombstone metadata, eski Key lookup'la bütün term/posting eşitliği, eksik
  anahtar, sınır dışı ve yinelenen ID hatalarını kapsar.
- `cargo fmt --check`; default/all-feature/referans
  `cargo clippy --all-targets -- -D warnings` başarılı.
  [Komutlar/çıkış kodları](benchmarks/write-stages-10k-20261018-checks.json),
  [test sayıları](benchmarks/write-stages-10k-20261018-test-counts.json).
- Bütün18 normal yazma prosesinden sonra10k tam payload doğrulandı. FDB'de
  ayrıca4 contains sorgusunun tam ilk50 sayfası doğrulandı. Aynı API'nin
  eski/yeni kaynak/indeks/MET04/ready dosyaları byte/SHA-eşit.
- Eski ve yeni normal okuyucu yeni yazılmış DB'yi açıp bütün38 oracle'ı geçti.
  Yeni `search_into` için de ayrı normal38-case full-payload işlev kontrolü
  geçti; bu kontrol yeni tampon hız kazancı ölçümü değildir.
- Son normal CLI'da stats ve200 sonuçlu sıralı/sürüm kontrollü arama başarılı.
  Normal CLI ve dört ölçüm example'ı son adımda diagnostic kapalı derlendi.
- SQLite immutable/read-only `PRAGMA integrity_check` sonucu `ok`,10k belge;
  validator Python SQLite3.53.1, native karşılaştırıcı3.53.2. Bu bütünlük
  kontrolü arama zamanının dışındadır.
- Üç okuma serisinin öncesi/sonrası ve final21 kaynak/cache/SQLite/fixture
  dosyası SHA-eşit. Bütün corpus ve0.1 fixture SHA değerleri baştakiyle aynı.
  Cargo dosyaları, protokol, SQLite karşılaştırıcısı ve corpus/matrix araçları
  Git'te değişmedi. [Final kod/binary/veri kanıtı](benchmarks/write-stages-10k-20261018-proof.json).

Ölçülmüş yazma/CPU adımı normal bounded profilinde tutuldu. Genel arama ve
OS RAM kabul kapısının geçtiği söylenmiyor; yukarıdaki artışlar açık.

Yeni corpus üretilmedi veya indirilmedi. Aynı
`data/rkyv-corpus-20261009/json.payload-bench` ilk10.000 belge kullanılır:
264.842.858 metin byte,2057–51197 byte pasajlar,8 kitap.20×500 işlem.
Yalnız araçların sahip olduğu sabit `data/benchmark-work/fdb` ve `sqlite`
alanları yazma/indeks oluşturma ölçümünde sıfırlanır. Veritabanı kopyası veya
yeni tarihli veri klasörü tutulmaz. SQLite bu görevde bağımsız benchmark'tır;
motor bağımlılığı veya dış proje entegrasyonu değildir.

Metin parçalama ve RAM posting ekleme, son merge/bakım, normal Vec input RAM'i,
tek sonuç/Harpoon/genel arama p95'i, genel açılış, disk sıkıştırma ve bakım
sırasında eşzamanlı/gerçek-soğuk yük açık. [İş listesi](OPEN-WORK.md).
