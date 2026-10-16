# Aynı sürüm kontrolü ve dar okuma denemesi — 2026-10-15

Toplu p95 artışı tek başına motor regresyonunun kanıtı değildir. Aynı
executable'ın kendisiyle karşılaştırıldığı üç çiftte sekiz sayfanın sekizinde
de toplu p95 yükseldi. Bu bir kod değişikliğinin sonucu olamaz. Ölçüm aracı
artık çalışma sırasını dengeler ve proses çiftlerinin farklarını ayrıca verir.
Bu bulgu önceki açık artışları otomatik kapatmaz.

Dar sorgulara adaptif okuma penceresi açma prototipi **reddedildi** ve runtime
değişiklikleri geri alındı. Hedef sorgunun gerçek okuma sayısı azalmadı;
geniş testte tekrar eden genel kazanç yok. Önceki source-lease iyileştirmesi
korunur. Yeni kalıcı format/feature/bağımlılık veya default değişikliği yok.

## Kapsam

Windows/release/experimental-builder-budget/record-read. Aynı mevcut corpus'un
ilk10k belgesi ve `data/benchmark-work/fdb`; SQLite sabit slotunun hash'i de
izlenir. Yeni corpus, DB kopyası veya DB reset yok. Sıcak OS cache, eviction
yok; CPU affinity/power ayarı yapılmadı. Zamanlar wall time'dır.

Kontrol `output/adaptive-seed-reads-control-20261015.exe`, önceki source-lease
normal release'idir. İlk A/B sayfa ve38-case matrix üçer proses çifti;
sıra A/B, B/A, A/B. Sayfa testinde200 örnek/proses, toplam600; geniş testte
primary20/page5, toplam60/15. Her sonuç total/tam belge/key/version/sequence
oracle ile doğrulanır. Zamanlama dışında aynı256 get anahtarı da kontrol edilir.
SQLite performansı bu adımda yeniden ölçülmedi; önceki
[tek referansın koşulları](SOURCE-LEASE-10K-20261015.md) korunur.

## Önceki artışın sınırı ve mevcut maliyet

Önceki source-lease profillerinde quixote /28/10 ve /57/50 için her iki sürüm:
56 aday,56 payload read,1.945.325B,815 gövde fold/scan; covering guard0 çağrı.
Kaynak korumasının hızlı yolu bu sorguda çalışmaz. İş sayıları aynı olsa da
derleme/yerleşim ve ortam etkilerinin tamamı ayrıştırılmış değildir; bunun
üstünden bütün artışların nedeni ilan edilmez.

Önceki üç proses ×3 warm instrumented quixote /28/10 profil medyanları:
payload read0,6639ms, CRC0,2460ms, yapı/UTF-8 doğrulama0,2294ms,
gövde fold0,5029ms, scan0,0885ms, posting intersection0,0149ms;
toplam2,8608ms. İç içe wall span'ler toplanmaz, CPU attribution veya normal
p95 değildir. [Kaynak profiller](benchmarks/source-lease-profile-20261015-summary.json).

## Prototip ve reddedilme

Prototip yalnız mevcut builder-budget içinde dar posting seed'inde de okuma
penceresine izin verdi. Üç fiziksel ardışık kayıt şartı,256KiB pencere ve
%75 kullanım şartı korundu. Pencere sınırı metadata çağrısı ilk gerçek
prefetch'e ertelendi; CRC/UTF-8/identity kontrolleri atlanmadı.

600 örnekli sayfa testi3 düşük/5 yüksek pooled p95: quixote /28/10
-%6,36 ve3/3 hızlı; quixote /57/50 +%3,17 ve2/3 hızlı. Geniş38-case
yalnız9 düşük/29 yüksek; quixote ilk50 +%4,10, zx +%5,14,
the zx +%8,29. Büyük pooled sayfa farkları da her çiftte tekrar etmedi.
Bu oranlar nedensellik veya istatistiksel anlamlılık ilanı değildir.
[Sayfa](benchmarks/adaptive-seed-pages-10k-20261015-summary.json),
[geniş matrix](benchmarks/adaptive-seed-matrix-10k-20261015-summary.json).

Son ayrı instrumented süreç çiftinde üç warm iteration'ın iş sayaçları:

| Sorgu | Read kontrol→deneme | Pencere hit kontrol→deneme | Okunan byte kontrol→deneme |
|---|---:|---:|---:|
| quixote /28/10 |56→56 |0→0 |1.945.325→1.945.325 |
| quixote /57/50 |56→56 |0→0 |1.945.325→1.945.325 |
| the zx /0/50 |1104→1104 |8896→8896 |287.040.309→287.040.309 |

Bu hedefte ardışıklık şartı yeni bir toplu okuma üretmedi. Zamanlama farkını
azalan I/O başarısı diye sunamayız. Prototip kaldırıldı; önceki okuma yolu
aynen geri yüklendi. [İş sayaçları](benchmarks/adaptive-seed-profile-counts-20261015.json).

## A/A kontrolü ve korunan araç değişikliği

Üç çiftte iki etiket de aynı executable SHA'sına işaret etti. Sekiz pooled
p95'in tümü yüksek: +%0,31 ile +%6,39; quixote /28/10 +%3,10,
the /0/1 +%5,33. Her sayfada yalnız1/3 proses çifti daha hızlıydı.
Bu seride ilk çalışan taraf2/1 dengesizdi; sıra/ortam etkileri birbirinden
tam ayrıştırılmadı. Bu kontrol, bu serideki değişimlerin kod farkından
kaynaklanmadığını gösterir; bütün A/B farklarını geçersiz kılmaz.
[Aynı executable kontrolü](benchmarks/adaptive-seed-aa-pages-10k-20261015-summary.json).

Korunan değişiklikler:

- Runner açık `-Control`/`-Candidate` executable yollarını kabul eder.
- Varsayılan4 çift, çift sayı zorunlu; A/B,B/A,A/B,B/A ile ilk/ikinci
  konumlar eşit sayıda kullanılır. Tasarım/sıralar ayrı JSON'a yazılır.
- Özet aynı executable kontrolünü hash'le etiketler, örnek sayılarını ve
  her çiftin p95 değişimini/medyanını gösterir. Toplu p95 de korunur.
- Her logun örnek sayısı ve p95'i ham örneklerle doğrulanır. Eski üç-çift
  loglar tasarım dosyası yokken hâlâ okunur; geçmiş raporlar yeniden yazılmaz.

Dört dengeli çiftte (her iki etiket aynı executable),800 örnek/sayfa:

| Sorgu / offset / limit | Pooled p95 farkı | Hızlı çift | Çift farklarının medyanı |
|---|---:|---:|---:|
| the /0/1 |-%1,79 |3/4 |-%3,66 |
| the /0/200 |-%0,69 |3/4 |-%0,47 |
| the /5000/50 |-%0,51 |3/4 |-%4,16 |
| the /9950/50 |+%0,85 |1/4 |+%2,93 |
| ishmael /130/1 |+%2,94 |1/4 |+%5,58 |
| ishmael /60/200 |+%0,22 |1/4 |+%0,90 |
| quixote /28/10 |+%0,23 |2/4 |+%1,01 |
| quixote /57/50 |+%2,00 |2/4 |+%0,67 |

Yeni A/A serisinde3 pooled p95 düşük/5 yüksek; kod aynı olduğundan bunlar
motor optimizasyonu veya regresyonu değildir. Sıralama tasarımında ilk/ikinci
konum sayısı artık eşit. İki A/A serisi farklı zaman aralıklarındadır;
aralığın daralmasının tümü sıralama düzeltmesine atfedilmez. Aynı sürüm
kontrolü sıfır fark garantisi veya otomatik bir yüzde toleransı oluşturmaz.
Önceki24 artış ve yazma/bakım işi açık kalır; her yeni A/B sonucu ham örnek,
çiftler, iş sayaçları ve aynı-sürüm kontrolüyle birlikte değerlendirilmelidir.
[Dört çift sonuçları](benchmarks/adaptive-seed-aa-balanced-pages-10k-20261015-summary.json),
[açık sıra tasarımı](benchmarks/adaptive-seed-aa-balanced-pages-10k-20261015-design.json).

## Son doğrulama

Son doğrulama2026-10-16; ölçüm loglarının20261015 prefix'i korunur.
Prototipte altı window testi (bozulma/kesilme/UTF-8/identity ve aşırı büyük
payload dahil) geçti. Geri yüklenen runtime ile builder-budget all-targets
142 test geçti (4 subprocess helper ignore). Kaynak/cache bozulması,
kesilme, eski fixture, WAL/merge process kill ve okuyucu uyumluluğu dahil.
Default ve all-features all-targets Clippy `-D warnings`, fmt ve diff
whitespace kontrolü geçti. Yeni4-çift tasarımı ve eski3-çift logların ham
örnek/p95 kontrolü geçti; geçmiş source-lease raporları yeniden yazılmadı.

Geri yüklemede kaynak dosyası tarihi de eskiye döndüğü için bir ilk build
önbellekteki reddedilmiş kodu kullanmıştı. Kaynak SHA'sı doğruyken `.text`
bölümü deneme executable'ıyla eşleşti; son byte karşılaştırması bunu yakaladı.
Kaynak tarihi yenilenip test/Clippy/normal release yeniden derlendi. Nihai
normal reader'ın `.text` bölümü kabul edilmiş kontrol ile birebir aynı;
son38-case tam belge/version kontrolü de geçti. Bu ara build'in kontrolü
başarılı sayılmaz; sonuç final build'e dayanır.
[Ara bulgu](benchmarks/benchmark-control-stale-binary-20261015.json),
[son doğrulama](benchmarks/benchmark-control-final-proof-20261015.json),
[son38-case kontrolü](benchmarks/benchmark-control-restored-final-check-20261015.jsonl).

Son124 FDB/cache/SQLite dosyasının hash'i deneme öncesiyle aynı. Corpus
2.740.862.279B ve SHA256 aynı;0.1 fixture Git durumu boş. Engine/index/
covering/normalizer kaynakları ve Cargo.toml/lock önceki source-lease
snapshot'ıyla aynı SHA'ya döndü. Kalıcı değişiklikler ölçüm aracı ve raporlardır.

## RAM, disk ve açık işler

Yeni RAM veya yazma/bakım ölçümü yok. Önceki indexing private9,129MiB ve
WS13,027MiB bu turda yeniden ölçülmüş sayılar değildir; query tepe belleği
ayrıdır. Önceki yazma+bakım +%3,00 gözlemi de bu A/A okuma testiyle kapanmaz.
Dosya formatı/0.1 fixture aynı, kaynak/index/cache dosya boyutları değişmez.

[OPEN-WORK](OPEN-WORK.md): diğer arama artışları, uzun/olumsuz tarama,
yazma/bakım, query RAM/normal API/genel heap, disk/eski nesiller ve
eşzamanlı/soğuk yük açık. Dış proje entegrasyonu yok.
