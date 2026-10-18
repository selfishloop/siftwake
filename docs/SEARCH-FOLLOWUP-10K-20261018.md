# Varsayılan arama yolunda normalleştirme ve sonuç kapasitesi — 18 Ekim 2026

Sonuç: bilinen 2.464 B fazla sonuç kapasitesi giderildi. Beagle'da iki normal
yük düzeninde tipik blok yaklaşık %4,2 düşük; p95 her ikisinde dört süreç
çiftinde hızlı. Harpoon'da tipik blok kazancı var, genel p95 kazancı tutarlı
değil. Daha büyük tarama scratch'i nedeniyle genel sorgu/proses RAM'i
azaldı denmez. Küçük değişiklikler varsayılan motorda korunur; kalan p95,
the200 hydration ve genel RAM işleri açık.

Bu çalışma, `7471c2d` ile varsayılan olan okuyucunun üç açık arama kalemini
inceler: beagle, harpoon ve 200 sonuçlu the sayfası. Önceki karşılaştırmada
p95 sırasıyla+%14,33/+%14,91/+%8,88 idi. Burada aynı mevcut 10k veritabanı
kullanılır; o eski yüzdelerin tamamının kök nedeninin bulunduğu iddia edilmez.

## Değişiklik öncesi iş profili

Release `diagnostic-timing`, strict source/index açılışı, her sorguya bir
ısınma ve üç ölçülen tekrar; bütün 38 sorgunun full payload/version oracle
kontrolleri geçti. Aşağıdakiler üç tekrarın medyanıdır. Span'ler iç içedir;
toplanarak toplam süre/CPU yüzdesi veya normal release p95 üretilemez.
Özellikle çok sayıdaki fold/scan span'ı timer ve mutex maliyeti ekler.

| Candidate contains aşaması | beagle | harpoon |
|---|---:|---:|
| Doğrulanan aday | 579 | 2.089 |
| İstenen/okunan payload byte | 20.798.429 | 69.213.695 |
| Payload read span | 6,4261 ms | 23,7796 ms |
| Candidate CRC | 2,0794 ms | 7,2579 ms |
| Schema/UTF-8 | 1,9409 ms | 8,4336 ms |
| Gövde normalleştirme | 6,8731 ms /19.483 çağrı | 20,9994 ms /46.903 çağrı |
| Normalleşmiş gövde tarama | 2,4784 ms | 7,1549 ms |
| Sayfa birleştirme/belge hazırlama | 0,6337 ms | 0,6994 ms |

Bu iki sorguda hem payload okuma hem gövde doğrulama büyük işlerdir; sonuç
hazırlığındaki tampon büyümesi tek başına eski p95 artışını açıklamaz.
Çok sayıdaki küçük fold/tarama adımı azaltılabilir bir iş olarak seçildi.

The200 başka bir yoldur: yeni checked-page reuse'a girmez. Candidate payload
doğrulaması0,200 belgenin hydration'ı vardır. Sayfa hazırlama medyan5,1618ms,
bunun içindeki CRC/decode/identity3,5905ms; bütün sorgu5,4156ms. Genel disk
read API span'ı1,2406ms/120 çağrıdır; metadata/payload okumaları iç içedir,
fiziksel disk transfer süresi değildir. CRC, schema ve sahipli belge kurma
bu profilde birbirinden tam ayrılmadığı için bu üç alt işten biri kesin
kök neden diye seçilmez. Reuse kapasitesi düzeltmesi bu sayfaya hız kazancı
olarak atfedilemez.
[Bütün baseline aşamaları](benchmarks/search-followup-20261018-baseline-stages.json).

## Dar değişiklikler

- Yeniden kullanılan sonuç tamponu, aday taraması bittiğinde bilinen tam
  total ve sayfa limitinin küçüğü kadar ayrılır. Negatifte kapasite 0'dır.
  Geometrik büyümenin sonuç Vec'ine fazla kapasite taşıması kaldırılır.
- İlk gövde parçası 1 KiB kalır; bu parçada bulunmayan terimler için sonraki
  parçalar 4 KiB olur. Amaç erken eşleşmeyi koruyup fold/window/tarama
  hazırlığının tekrarını azaltmaktır. Sorgu sözcüğü adına göre seçim yoktur.
  Referans `--no-default-features` motorunda 1 KiB yolu korunur.
- Reuse uygunluğu için terimin bütün scalar'ları sayılmak yerine dördüncü
  scalar'ın varlığına bakılır. Koşul aynıdır; uzun negatif sorguda gereksiz
  tam karakter sayımı yapılmaz. Payload doğrulama veya arama semantiği değişmez.

Unicode/Türkçe fold, UTF-8 ve terimlerin parça sınırını aşması için mevcut
canonical fold karşılaştırma testi yeni sınırlarla genişletildi. CRC/schema/
UTF-8/key/version, kaynak lease ve authoritative source kuralları aynı.
Yeni feature, dependency, kalıcı format, WAL/manifest veya dış proje işi yok.

Benchmark örneğine mevcut `diagnostic-memory` altında `--allocation-matrix`
eklenir; standart derlemede allocator sayacı yoktur. Full38 oracle ile
sonuç Vec kapasitesi, requested Rust heap peak/live ve tahsis çağrıları
ölçülür. Bu sayılar OS private/working set veya toplam RAM değildir;
bu derlemenin süreleri hız karşılaştırmasına alınmaz.

## Ölçüm kapsamı

Aynı `data/rkyv-corpus-20261009/json.payload-bench` dosyasının ilk10.000
belgesi, sekiz kitap,264.842.858 metin byte,2.057–51.197 byte/belge.
Sabit `data/benchmark-work/fdb` yeniden kullanılır; corpus hazırlanmaz,
DB reset/repack/kopyası veya başka diske taşıma yapılmaz. SQLite yalnız
dosya değişmezliği kontrolündedir; yeni SQLite hız ölçümü yoktur.

Normal release karşılaştırması, önceki varsayılan frozen okuyucu ile bu
değişikliğin okuyucusunu kullanır. Dört süreç çifti, iki dengeli sıralı blok,
blok başına10 tekrar;38 sorgunun hepsi her tur çalışır:80 örnek/sorgu/motor.
Normal derlemelerde timing/allocation feature'ları kapalıdır. Sorgu hazırlığı
ve search saat içinde; source/index strict açılışı, oracle ısınması ve tam
belge/sıra/version kontrolü saat dışındadır. Affinity mask4, sıcak OS cache,
eviction yok; iki resident oracle worker ve tek DB sahibi vardır.

Çağıran thread CPU/cycle sayaçları arama saatinin dışında okunur. Bunlar
timer/observer maliyeti, CPU zaman çözünürlüğü ve değişken frekans nedeniyle
kusursuz CPU/scheduler atfı değildir; normal p95 ile karıştırılmaz.
Ayrı proses bellek probunda oracle yüklenmez: dört dengeli çift, süreç
başına24 arama/771 get; OS lifetime high-water private commit/working set.
Private commit fiziksel RAM değildir. Yazma, genel açılış performansı,
eşzamanlı veya gerçek-soğuk yük bu ölçümlerde yoktur.

## Normal release: bütün 38 sorgu

Motor/sorgu başına80 örnekte31 düşük/7 yüksek pooled p95. Değişiklik
öncesindeki diğer okuyucuyla alınmış20/18 sonuçla bu sayıların farkı doğrudan
kazanç sayılmaz; aşağıdaki aynı-koşu kontrol/adaya bakılır. İlk koşu öncesi
8s sistem CPU busy%5,66; bu anlık gözlem tam izolasyon değildir.

| Sorgu (offset/limit) | Kontrol p95 ms | Aday p95 ms | Fark | Tipik blok farkı | Hızlı çift/blok |
|---|---:|---:|---:|---:|---:|
| beagle (0/50) | 22,0755 | 17,5876 | -%20,33 | -%4,26 | 4/4;8/8 |
| harpoon (0/50) | 56,9726 | 54,9781 | -%3,50 | -%1,64 | 2/4;8/8 |
| the (0/200) | 4,7198 | 4,5465 | -%3,67 | -%0,54 | 3/4;4/8 |
| quixote (0/50) | 2,5133 | 2,5009 | -%0,49 | -%3,20 | 1/4;4/8 |
| man (0/50) | 1,3021 | 1,4532 | +%11,60 | +%1,34 | 2/4;4/8 |
| İSHMAEL (0/50) | 6,6089 | 7,3035 | +%10,51 | +%0,70 | 0/4;4/8 |
| ishmael (130/1) | 6,0433 | 6,2112 | +%2,78 | +%2,25 | 1/4;3/8 |
| ishmael (60/200) | 11,8105 | 12,6640 | +%7,23 | -%0,03 | 1/4;4/8 |
| uzun negatif (0/50) | 0,0353 | 0,0347 | -%1,70 | -%6,68 | 3/4;6/8 |

Beagle'da p95 dört çiftte, tipik blok sekiz blokta düşük. Harpoon'da tipik
blok8/8 düşük fakat süreç p95 tutarlı değil. The200'ün hydration algoritması
değişmediğinden küçük fark chunk/reuse algoritmasının kanıtı sayılmaz.
Man, Unicode-fold ve sayfa artışlarıyla bütün-sorgu p95 işi kapanmaz.
[Bütün sonuçlar, süreç/blok dağılımları ve thread sayaçları](benchmarks/search-followup-reads-10k-20261018-summary.json).

CPU zamanı15,625ms adımlı olabilir: örneğin2ms sorguda0 veya15,625ms CPU
görülebilir. Bu yüzden wall-CPU farkı küçük sorgularda kesin bekleme süresi
diye sunulmaz. Thread cycle sayıları da sabit frekanslı zaman değildir.

## Aynı derlemelerle yoğun tekrar

Yeni geniş koşudaki Unicode-fold/sayfa artışları ve hedef sorgular için
aynı SHA'lı executable'lar tekrar kullanıldı; yeni kod/derleme yoktur.
Dokuz mevcut case, dört dengeli çift, iki blok ve25 tekrar: her sorguya/
motora200 örnek. Bütün sorgular yerine dokuz sorgu serpiştirildiği için
sorgu geçmişi farklıdır; fazların mutlak süreleri veya örnekleri birleştirilmez.

| Sorgu (offset/limit) | Kontrol p95 ms | Aday p95 ms | Fark | Tipik blok farkı | Hızlı çift/blok |
|---|---:|---:|---:|---:|---:|
| beagle (0/50) | 18,3455 | 17,7057 | -%3,49 | -%4,24 | 4/4;8/8 |
| harpoon (0/50) | 54,5779 | 54,3406 | -%0,43 | -%2,33 | 2/4;8/8 |
| the (0/200) | 4,6529 | 4,6293 | -%0,51 | -%3,89 | 3/4;5/8 |
| quixote (0/50) | 2,6671 | 2,6079 | -%2,22 | -%0,23 | 2/4;4/8 |
| ishmael (0/50) | 6,2531 | 6,3711 | +%1,89 | -%0,31 | 1/4;4/8 |
| man (0/50) | 1,5027 | 1,3851 | -%7,83 | -%1,11 | 3/4;7/8 |
| İSHMAEL (0/50) | 6,1735 | 6,1993 | +%0,42 | +%0,28 | 1/4;4/8 |
| ishmael (130/1) | 5,4585 | 5,5398 | +%1,49 | -%0,17 | 3/4;5/8 |
| ishmael (60/200) | 11,0593 | 11,3254 | +%2,41 | +%0,49 | 2/4;4/8 |

Beagle kazancı tekrarlanır; geniş koşudaki -%20,33 bütün yüklere genellenmez.
Harpoon tipik blok8/8 düşük, p95 hâlâ2/4 süreçte düşük. The200'de genel
tekrarlanabilir p95 kazancı veya eski gerilemenin kök nedeninin giderildiği
kanıtlanmadı. Man ve İSHMAEL'in büyük artışları bu yükte tekrarlanmadı;
Unicode-fold ve sayfa farkları küçük artık artışlarla açık tutulur.
İSHMAEL yoğun p95 cycle artışı yaklaşık%1,3; CPU zamanı15,625ms çözünürlüklü
olduğundan küçük farklara scheduler/kod diye kesin atıf yapılmaz.
[Dokuz sorgu,200 örnek ve thread sayaçları](benchmarks/search-followup-focus-10k-20261018-summary.json).

## İş ve heap farkı

İki release diagnostic profilde de aynı üç tekrar/full oracle kullanıldı:
beagle fold ve scan çağrıları19.483→5.520, harpoon46.903→14.039. Bütün 38
sorguda candidate CRC/schema, kabul edilen kayıt, payload request/byte ve
reuse çağrı sayıları aynı. Bu bir byte normalleştirme hacmi sayacı değildir;
daha büyük bloklar erken bulunan bazı terimlerden sonra ek byte işleyebilir.
Diagnostic timer/mutex çağrılarının azalması da bu derlemenin sürelerine
dahildir; hız iddiası yukarıdaki normal ölçüme dayanır.
[Aşama/call karşılaştırması](benchmarks/search-followup-20261018-stages.json).

| Requested Rust heap, üç sıcak tekrar medyanı | Önce | Sonra | Fark |
|---|---:|---:|---:|
| quixote sonuç Vec kapasitesi | 66 | 52 | -14 eleman |
| quixote canlı sonuç heap'i | 1.802.229 B | 1.799.765 B | -2.464 B |
| beagle sonuç Vec kapasitesi | 66 | 48 | -18 eleman |
| beagle canlı sonuç heap'i | 1.735.251 B | 1.732.083 B | -3.168 B |
| harpoon canlı sonuç heap'i | 1.639.200 B | 1.636.736 B | -2.464 B |
| beagle sorgu ek heap tepesi | 1.798.590 B | 1.810.888 B | +12.298 B |
| harpoon sorgu ek heap tepesi | 1.693.879 B | 1.706.179 B | +12.300 B |

38 sorgunun hepsinde sonuç kapasitesi artmadı; canlı heap farkı kapasite
farkı ×176 B ile tam eşleşti. Bilinen 2.464 B regresyonu giderildi; baseline
okuyucudaki 52 kapasiteye dönüldü, kapasite tam 50'ye indirildi denmez.
Sonuçsuz sorgular canlı heap/capacity 0 ile döner. Trigram/posting'de hemen
elenen negatiflerde tahsis sayıları/tepe aynıdır. Candidate metni tarayan
sonuçsuz transubstantiation'da da büyük scratch nedeniyle tepe+12.322B;
sonuçsuz sorguların hepsinde transient heap azaldı denmez.
[38 sorgunun bütün heap farkları](benchmarks/search-followup-20261018-allocation-summary.json).

| Oracle'sız OS lifetime peak, dört çift medyanı | Önce | Sonra | Fark |
|---|---:|---:|---:|
| Private commit | 8,75391 MiB | 8,78906 MiB | +36 KiB (+%0,40) |
| Working set | 12,71289 MiB | 12,72266 MiB | +10 KiB (+%0,08) |

İki OS sayaç4/4 çiftte arttı; «RAM gerilemedi» kapısı geçti denmez. Birinci
çiftin WS farkı diğer üçünden büyüktür; ham veride korunur. Sonuçta kalan
heap azalırken çalışma scratch'i ve proses high-water'ın küçük artışı ayrı
olgulardır. Bu yüzden genel sorgu RAM'i açık kalır.
[OS RAM ham sayaçları](benchmarks/search-followup-memory-10k-20261018-summary.json).

## Doğrulama

Default 185 test geçti/7 ignored. Referans engine 17 ve scratch 2; all-features
engine 1 ve scratch 4 geçti. Mevcut kesilme/bozulma/process-kill/0.1 fixture
testleri default tam takımda geçti. All-features ve referans tam takım bu
tur yeniden koşulmadı; belirtilen hedefli testler koşuldu. fmt-check ve
default/all-features/no-default-features all-targets Clippy,`-D warnings`
geçti. Her diagnostic ve normal aramada full payload/order/version oracle
kontrolleri geçti. Normal CLI'da 10k/sequence 20 ve quixote/beagle/harpoon/
the200 total, sayfa, key sırası/version kontrolleri de geçti.
[Komutlar ve loglar](benchmarks/search-followup-20261018-checks.json).

Başlangıç/son 21 dosya SHA-eşit: 18 FDB dosyası 349.805.698 B, SQLite 583.860.224 B
ve 0.1 fixture'ın iki kaynağı aynı. Corpus'un tamamı 2.740.862.279 B;
SHA-256 `11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B`
önceki kanıta eşit. Tam corpus hash'i bütün performans koşuları bittikten
sonra okundu. Cargo/dependency/feature/fixture ve kalıcı format değişmez.
[Final kanıtı, kaynak/binary SHA'ları ve ortam](benchmarks/search-followup-20261018-proof.json).

## Açık kalanlar

Yazma/bakım, diğer sorgu regresyonları, genel RAM/açılış, disk sıkıştırma
ve eşzamanlı/gerçek-soğuk yük açıktır. Önceki p95 yüzdelerinin kök nedeni
ile mevcut normalleştirme maliyeti ayrı iddialardır; ölçüm olmadan biri
ötekiyle kapatılmaz. [İş listesi](OPEN-WORK.md).
