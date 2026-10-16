# Kısa bloklarla arama gerilemesini yeniden sınama — 2026-10-16

Yazma optimizasyonundan sonraki arama gerilemesi bu yeni düzende yeniden
üretilemedi. Eski/yeni iki ayrı karşılaştırmanın her birinde sekiz sayfanın
tamamında yeni sürümün pooled p95'i daha düşüktür. İkinci karşılaştırmada
araç etiketleri de ters çevrildi. Motor kodu bu tur değiştirilmedi;
bu sonuç yeni bir arama optimizasyonu kazancı değildir.

Önceki uzun/kesintisiz koşuların bütün farkları açıklanmış değildir.
Negatif kontrol, aynı sürümde bile tek-sonuç sayfasının dört çiftte p95
artışı verebildiğini yeniden gösterdi. Sorguda ek iş bulunmadığına dair
önceki sayaçlar ile bu yeni test birlikte, yazma kazancını yalnız önceki
p95 artışları yüzünden geri almak için yeterli kanıt olmadığını gösterir.
Kod yerleşiminin ve ortamın ayrı payları belirlenmedi; genel/default
kabul kapısı açık kalır.

## Çalışma düzeni

Aynı mevcut corpus'un ilk 10.000 belgesi: 264.842.858 B gövde,
2.057–51.197 B/pasaj. Corpus üretimi/indirme, DB reset veya DB kopyası yok.
Yalnız `data/benchmark-work/fdb` okunur; SQLite sabit slotu yalnız
değişmezlik kontrolündedir, yeni SQLite performans sonucu çıkarılmaz.

İki benchmark süreci aynı anda açık kalır; her biri aynı corpus'tan
bağımsız oracle'ı bir kez yükler. **Veritabanı aynı anda iki süreçte
açılmaz.** Her komut sabit FDB slotunu açar, kaynak kontrolü ve sayfa/get
ısınmasını yapar, 25 karışık tur ölçer, DB'yi kapatıp kilidi bırakır.
Diğer süreç ancak bundan sonra kendi bloğunu açar. Özel kilit atlatma,
motor API'si veya yeni read-only sahiplik yolu eklenmedi.

Her tur sekiz sayfanın sırasını karıştırır, aynı 256 get ve üç missing-key
kontrolünü yapar. Karşılaştırılan iki blok aynı seed ve aynı sorgu/get
sırasını kullanır. Sorgu zamanına Query oluşturma, search ve sequence
kontrolü dahildir. Dönen tam belge/key/version/total kontrolleri,
get işlemleri ve oracle/source ısınması sorgu aralığının dışındadır.
CRC, UTF-8/schema/key/version ve kaynak denetimleri korunur.

Her seride dört bağımsız süreç çifti ×8 blok ×25 tur: sayfa/variant
başına 800 örnek. Her süreç çiftinde ilk/ikinci blok konumu 4/4 dengeli;
başlangıçtaki oracle yükleme sırası da süreç çiftleri arasında değişir.
32 blok bağımsız 32 süreç değildir. İki farklı-sürüm serisi ve bir aynı
sürüm kontrolü toplam 38.400 zamanlanmış tam sayfa doğrulaması yaptı.

Windows/Ryzen 7 7840HS, normal release, `experimental-builder-budget`,
diagnostic timing/memory kapalı. Affinity mask 4 ile aynı mantıksal CPU;
uygulanan mask API üzerinden kontrol edilir. Sıcak OS cache, eviction yok.
Güç planı/frekans ayarı değiştirilmedi; SMT komşusu, diğer işler ve
sıcaklık/frekans etkileri izole değildir. Timed batch'ler sırasında
derleme/test çalıştırılmadı. RAM bu tur ölçülmedi; iki oracle süreciyle
bu deney toplam proses RAM karşılaştırması olarak kullanılamaz.

## Sonuçlar: her sütun yeni / eski sürüm

| Sorgu / offset / limit | İlk A/B p95 farkı | Ters etiketli tekrar p95 farkı | Yeni hızlı süreç çifti, ilk/tekrar |
|---|---:|---:|---:|
| the /0/1 |-%5,09 |-%0,87 |3/4; 1/4 |
| the /0/200 |-%7,89 |-%5,65 |3/4; 3/4 |
| the /5000/50 |-%8,23 |-%1,73 |3/4; 3/4 |
| the /9950/50 |-%11,27 |-%6,60 |3/4; 3/4 |
| ishmael /130/1 |-%6,51 |-%3,05 |4/4; 3/4 |
| ishmael /60/200 |-%7,26 |-%0,63 |4/4; 2/4 |
| quixote /28/10 |-%4,39 |-%2,45 |3/4; 2/4 |
| quixote /57/50 |-%6,92 |-%4,12 |3/4; 2/4 |

Ishmael tek-sonuç p95 ilk seride 7,6295→7,1329 ms, tekrarda
7,3107→7,0875 ms. Quixote /28/10 ilk seride 2,2702→2,1705 ms,
tekrarda 2,2627→2,2072 ms. Bütün süreç çiftleri ve bloklar hızlı değildir.
Örneğin ishmael tek-sonucun eşleştirilmiş blok medyanı farkı yaklaşık
-%0,46 / -%0,94'tür; pooled p95 farkı aynı istatistik değildir. Bu nedenle
"arama genel olarak %6 hızlandı" veya bütün iş yüklerinde kazanç denmez.

Ters serinin ham etiketleri `control=yeni`, `fdb=eski`dir. Ham özetin
8 yüksek demesi eski sürümün p95'inin yüksek olmasıdır. Yukarıda oran
yeniden yeni/eski hesaplandı; yüzde yalnız eksi işaretiyle çevrilmedi.
A/A başka bir zaman aralığında çalıştı; oranlardan çıkarılarak bir
"gürültü düzeltmesi" uygulanmadı.
[Etiketleri doğru yönde birleştiren sonuçlar](benchmarks/interleaved-normalized-results-20261016.json),
[ilk seri tasarım/ham özet](benchmarks/interleaved-ab-pages-10k-20261016-summary.json),
[ters etiketli seri](benchmarks/interleaved-ba-pages-10k-20261016-summary.json).

## Aynı programın kendisiyle kontrolü

Her iki etiket aynı candidate exe SHA'sına işaret eder. Altı pooled p95
düşük, ikisi yüksek; bütün sayfaların eşleştirilmiş blok medyanları
-%2,84…+%0,82 aralığındadır. `the /0/1` pooled p95 +%6,44 ve dört
süreç çiftinde de yüksektir; buna karşın blok medyanı -%1,01 ve 32 bloğun
17'si daha hızlıdır. Bu testte program/kod farkı yoktur.

Bu kontrol, dört çiftte p95 yönünün aynı olmasının tek başına motor
regresyonu kanıtı olmadığını somut gösterir. Önceki A/B farklarının
tamamının yalnız ölçüm gürültüsü olduğunu kanıtlamaz. Daha uzun p99,
çoklu iş yükü veya soğuk yük için güven aralığı çalışması yapılmadı.
[Aynı sürüm kontrolü](benchmarks/interleaved-aa-pages-10k-20261016-summary.json).

## Önceki testten fark ve yorum sınırı

Bu yeni example'ın eski/yeni worker'ları aynı benchmark kaynağı ve aynı
release özellikleriyle ayrı derlendi; engine farkı önceki yazma adımıdır.
Kontrol için önceki `src/index.rs` byte'ları geçici kullanıldı ve sonunda
aday kaynak byte'ları birebir geri yüklendi. Yeni benchmark, önceki
`measure_content_reads` executable'ı değildir. Thin LTO nedeniyle çağıran
example'ın değişmesi motorun makine kodu/yerleşimini de etkileyebilir.

Her blokta DB/cache yeniden açılıp ısındığından önceki kesintisiz 200-tur
oturumla heap/cache yaşam süresi aynı değildir. Kısa bloklar zaman drift'ini
azaltmayı amaçlar; bunu ölçülmüş bir frekans izolasyonu olarak sunmuyoruz.
Yeni düzenin sonuçları, önceki kesintisiz oturumun tüm nedenlerini tek
başına kapatmaz. Önceki sayılar silinmedi veya "yanlış" diye değiştirilmedi.
[Önceki sayaç/derleyici/ortam teşhisi](SEARCH-ATTRIBUTION-10K-20261016.md).

## Kod ve doğrulama

Değişiklikler benchmark example'ı, ortak matrix'in tekrar sayısını alan
girişi ve Python sürücüsüdür. Önceki matrix'in varsayılan tekrar/sıra/
doğrulama mantığı korunur. Motor kaynakları, bağımlılıklar, feature'lar,
kalıcı formatlar ve default seçimi değişmedi. Bu tur yeni depolama veya
kurtarma davranışı yoktur; eski test sayıları yeni koşulmuş gibi sunulmaz.

İki example'ın toplam sekiz testi, `cargo fmt --check`, default ve
all-features `cargo clippy --all-targets -- -D warnings` geçti.
Üç özet ham loglardan bağımsız olarak yeniden üretildi; her 25 örnek,
seed/block kimliği, sonuç doğrulaması, dengeli sıra ve exe hash'i kontrol
edildi. Son diagnostic-kapalı worker `.text`'i ölçülen adayla birebir aynı.
139 FDB/cache/SQLite dosyasının hash'i, corpus SHA'sı ve0.1 fixture aynı.
[Son kanıt](benchmarks/interleaved-final-proof-20261016.json).

Önceki %11,49 yazma+bakım kazancı korunur ve yeniden ölçülmedi. Bu dar
deneyde arama gerilemesi tekrar etmedi; bütün arama artışları kapatılmış
ve genel/default hız/RAM kapısı geçilmiş değildir. Merge/payload tekrar
işleri, query/normal API RAM'i, disk/eski nesiller ve eşzamanlı/gerçek
soğuk yük açıktır. [İş listesi](OPEN-WORK.md).
