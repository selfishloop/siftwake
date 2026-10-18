# Tek programda yeniden kullanım kontrolü — 18 Ekim 2026

Bu kontrolde büyük uç sürelerde thread CPU dışında bekleme, dar quixote iş
azalması ve sonuç Vec kapasitesinden 2.464 B canlı heap fazlası ölçüldü.
Uzun negatif sorgudaki artış kapanmadı; genel hız/RAM geçişi doğrulanmadı.
Checked-page varsayılana alınmadı; aktif motorun kaynakları başlangıca döndü.

Bu çalışma önceki checked-page prototipinin dar kazancı ile geniş sorgu
testindeki artışları ayırır. Yeni bir cache tasarımı veya eşik ayarı değildir.
Sonuçlar önceki varsayılan geçişini reddetme kararını değiştirmedi.

## Veri ve ölçüm düzeni

Mevcut `data/rkyv-corpus-20261009/json.payload-bench` içindeki aynı ilk
10.000 belge: sekiz kitap, toplam 264.842.858 metin byte'ı, belge başına
2.057–51.197 byte. Corpus üretilmez/indirilmez; sabit FDB/SQLite çalışma
alanları yeniden kullanılır, kopya/reset/repack yoktur. FDB18 dosya 349.805.698 B,
SQLite 583.860.224 B. Her faz öncesi/sonrası 18 FDB dosyası, SQLite ve iki 0.1
fixture için SHA-256 ve uzunluk kontrol edilir. `.idx` cache kabul edilmeden
kaynak kontrolü tamamlanır.

Windows/MSVC, Ryzen 7 7840 HS; worker affinity mask 4 (tek mantıksal CPU).
OS cache sıcak, eviction yok. Tam kaynak/indeks açılışı, bağımsız corpus
oracle'ı, 256 mevcut/üç eksik get kontrolü ve sorgu ısınması sorgu saatinin
dışındadır. Query hazırlığı ve tam sonuç belgesini döndüren search ölçülür;
total/sıra/title/text/fields/links/version kontrolü ve sonucu bırakma saat
dışındadır. Fiziksel disk transferi, güç kesintisi, gerçek soğuk cache veya
eşzamanlı istemci ölçümü değildir.

İlk A/A eski dondurulmuş okuyucunun aynı SHA'lı iki örneğini kullanır:
4 süreç çifti ×4 blok ×10 tekrar, bütün 38 sorguya 160 örnek/etiket.
Sayfa sorguları da her tur çalışır. Kısa bloklarda tek DB sahibi vardır;
iki bağımsız oracle süreci bellekte bulunur. Normal hız ölçümüdür.
[A/A ham sonuçları](benchmarks/reuse-controls-aa-10k-20261018-summary.json).

Ardından tek yeni executable, aynı süreç içinde üç koşul kullanır:
`off0`, `off1` (aynı kapalı yol/plasebo), `on`. Altı olası koşul sırasının
hepsi her süreçte bir kez çalışır; her koşul her sıra konumunda iki kez
bulunur. Sorgu tohumları koşullar arasında aynıdır. Her blok strict açılıp
kapanır; sürekli açık handle veya aynı süreçte kalıcı belge cache'i yoktur.
Tek oracle worker kullanılır. Farklı derleme/ASLR etkisi azaltılır; açık
ve kapalı kodun farklı fonksiyon adresleri ve instruction-cache/branch
etkileri bu yöntemle tümüyle kaldırılmış sayılmaz.
Kapalı probe da geçici prototipin ortak kodunu taşır; eski standart executable
ile bit-eşit değildir. Bu nedenle tek-program farkı, tüm prototipin mevcut
ürün okuyucusuna göre genel hız/RAM geçiş kapısını tek başına karşılamaz.

Normal geniş koşu 38 sorgu ve tur arası 256+3 get; dört süreç ×6 blok ×5 tekrar
=120 örnek/sorgu/koşul. Query-history kontrolünde aynı sekiz seçili sorgu,
get serpiştirmeden, aynı örnek sayısıyla karışık ve tek sorgu tekrarı şeklinde
ayrı koşulur. Tek sorgu tekrarı yeni/soğuk süreç anlamına gelmez; her seri
öncesinde o sorgu ısınır. Seçim: quixote, holmes, he, zx, uzun negatif,
the/200, ishmael/130/1, quixote/57/50. Motor bu kelimelere göre dal seçmez;
seçim eski kazanç/gerileme ve fallback yollarını kapsayan test seçimidir.
Karışık/tek-seri fazları ayrı süreç kümelerinde zaman sırasıyla koşar;
fazlar arası mutlak süre farkından ortam etkisinin tamamen ayrıldığı
iddia edilmez. Her fazın kendi açık/kapalı ve plasebo kontrolleri vardır.

İş profili (`diagnostic-timing`) ve Rust heap tahsisi (`diagnostic-memory`)
ayrı executable'larda, iki süreç ×6 blok ×1 tekrar=12 örnek/sorgu/koşul ile
ölçülür. Normal hız sonuçlarına karıştırılmaz. Zaman span'ları iç içedir;
toplanarak toplam süre veya CPU atfı yapılamaz. Heap sayacı sorgu başındaki
canlı heap'e göre ek tepe, sonuç hayattayken canlı fark ve tahsis çağrısıdır;
proses private/working set, allocator parçalanması veya toplam RAM değildir.
Corpus oracle'ı taşıyan sürecin working set'i motor RAM'i diye sunulmaz.

Kontrol oranları A/B yüzdelerinden çıkarılmaz. Birleştirilmiş p 95, süreç
başına p 95 ve eşlenmiş blok medyanı farklı dağılımlardır; birini seçerek
diğerinin ters işaretini gizlemeyiz. Az/çok p 95 sayısı tek başına kabul
kapısı veya istatistiksel anlamlılık iddiası değildir.

## İlk aynı-executable A/A

Aynı SHA'lı programda 24 düşük/14 yüksek p 95 bulundu. quixote+%3,08,
holmes+%0,62, zx-%0,59, the/200-%0,73; eşlenmiş tipik blok değişimleri
sırasıyla+%0,72/+%0,01/-%1,19/+%2,52. Bütün 38 için tipik blok aralığı
-%11,36…+%3,49; en büyük negatif the/1 gibi çok kısa yoldadır.
Bu, önceki 23 artışın tamamını dalgalanma diye kapatmaz ve önceki iki farklı
executable'ın dağılımlarını açıklayan kesin neden değildir.

## Aynı süreçte geniş kontrol

Her koşulda 120 örnek/sorgu;38 sorgu. off 1/off 0:20 düşük/18 yüksek p 95;
on/off 0:22 düşük/16 yüksek; on/off 1:19 düşük/19 yüksek. Genel, tekrarlanabilir
bir hız geçiş kapısı bu sonuçlarla doğrulanmadı. Özellikle quixote pooled p 95
on/off 0 -%56,83 iken aynı kapalı-kapalı fark -%56,91; on/off 1 +%0,19.
Buna karşılık eşlenmiş tipik blok on/off 0 -%15,55 ve on/off 1 -%15,33.
Bu istatistikler aynı iddia değildir; -%56,83 motor kazancı diye sunulmaz.
holmes on/off 0 p 95 -%9,03 ama yalnız 1/4 süreç hızlı; bir süreç dışarıda
bırakılınca oran +%46,2 de olabiliyor. zx/the 200 aynı fallback yolunda
on/off 0 p 95 -%24,31/-%27,83, tipik blok +%1,36/+%0,96. Bu farklar genel
algoritmik hızlanma kanıtı değildir. Ham örnekler, süreç p 95 ve leave-one-out
[geniş raporda](benchmarks/reuse-controls-matrix-10k-20261018-summary.json).

| Sorgu / offset / limit | off1/off0 p95 | on/off0 p95 | on/off1 p95 | on/off0 tipik blok | on/off0 hızlı süreç/blok |
|---|---:|---:|---:|---:|---:|
| quixote / 0 / 50 | -56.91% | -56.83% | +0.19% | -15.55% | 3/4; 20/24 |
| zeuglodon / 0 / 50 | +18.80% | +2.78% | -13.48% | -13.32% | 1/4; 17/24 |
| beagle / 0 / 50 | +3.33% | -7.83% | -10.80% | -3.36% | 2/4; 15/24 |
| mesopotamia / 0 / 50 | +37.76% | -29.92% | -49.13% | -6.88% | 2/4; 17/24 |
| transubstantiation / 0 / 50 | +4.43% | +18.68% | +13.65% | +1.83% | 2/4; 8/24 |
| harpoon / 0 / 50 | -25.57% | -29.32% | -5.04% | +1.58% | 2/4; 9/24 |
| ishmael / 0 / 50 | +24.55% | -13.04% | -30.18% | -7.07% | 2/4; 19/24 |
| invertebrata / 0 / 50 | -5.99% | +11.77% | +18.89% | -1.71% | 2/4; 16/24 |
| darcy / 0 / 50 | -24.80% | -27.29% | -3.31% | -2.29% | 2/4; 13/24 |
| elizabeth / 0 / 50 | -2.13% | +23.70% | +26.39% | +0.44% | 2/4; 12/24 |
| watson / 0 / 50 | +6.05% | +34.04% | +26.40% | -1.81% | 1/4; 14/24 |
| holmes / 0 / 50 | +14.18% | -9.03% | -20.33% | -4.50% | 1/4; 15/24 |
| frankenstein / 0 / 50 | -8.94% | -15.48% | -7.19% | -3.45% | 1/4; 15/24 |
| galapagos / 0 / 50 | -15.86% | +0.24% | +19.13% | -2.58% | 1/4; 13/24 |
| fossil / 0 / 50 | -23.90% | -9.37% | +19.10% | -1.04% | 2/4; 14/24 |
| geologist / 0 / 50 | +3.01% | -25.73% | -27.90% | -3.01% | 2/4; 14/24 |
| the / 0 / 50 | -38.67% | -22.43% | +26.48% | +13.57% | 2/4; 9/24 |
| man / 0 / 50 | -6.66% | +36.55% | +46.29% | +6.24% | 2/4; 7/24 |
| he / 0 / 50 | +1.42% | +2.05% | +0.62% | +2.42% | 3/4; 11/24 |
| zzxqnever / 0 / 50 | -14.63% | +1.74% | +19.18% | -2.01% | 2/4; 12/24 |
| zx / 0 / 50 | -21.59% | -24.31% | -3.48% | +1.36% | 3/4; 10/24 |
| the zx / 0 / 50 | -23.99% | -4.42% | +25.75% | +0.62% | 2/4; 11/24 |
| quixote whale / 0 / 50 | +112.55% | +56.79% | -26.23% | -15.18% | 1/4; 19/24 |
| holmes elementary / 0 / 50 | +8.94% | +1.61% | -6.73% | +1.48% | 1/4; 12/24 |
| fossil geologist / 0 / 50 | -32.89% | -39.32% | -9.58% | -2.76% | 2/4; 14/24 |
| whale harpoon / 0 / 50 | +10.23% | +6.68% | -3.22% | -0.81% | 1/4; 13/24 |
| QUİXOTE / 0 / 50 | -27.20% | -53.57% | -36.22% | -21.60% | 4/4; 22/24 |
| İSHMAEL / 0 / 50 | +45.88% | +57.73% | +8.12% | -8.01% | 0/4; 17/24 |
| zx×150 / 0 / 50 | +6.34% | +43.74% | +35.17% | -4.05% | 1/4; 16/24 |
| presumption / 0 / 50 | +39.25% | -7.54% | -33.60% | -0.21% | 2/4; 12/24 |
| the / 0 / 1 | -37.68% | -22.40% | +24.51% | +1.60% | 2/4; 12/24 |
| the / 0 / 200 | -33.49% | -27.83% | +8.50% | +0.96% | 2/4; 12/24 |
| the / 5000 / 50 | +44.16% | +4.24% | -27.69% | +1.89% | 1/4; 11/24 |
| the / 9950 / 50 | +26.43% | +11.73% | -11.62% | +2.26% | 1/4; 10/24 |
| ishmael / 130 / 1 | -24.12% | -3.19% | +27.57% | +1.34% | 1/4; 10/24 |
| ishmael / 60 / 200 | -35.72% | -26.83% | +13.83% | +0.43% | 1/4; 12/24 |
| quixote / 28 / 10 | -37.20% | -29.74% | +11.88% | -2.23% | 2/4; 14/24 |
| quixote / 57 / 50 | +0.44% | -52.63% | -52.84% | -4.53% | 3/4; 15/24 |

## Aynı sekiz sorguda karışık / tek sorgu tekrarı

İkisi de 120 örnek/sorgu/koşul, dört süreç; tur arası get yok. Fazlar ayrı
zamanlarda koştuğundan mutlak hız farkı salt query-history etkisi değildir.
quixote iki düzende de off 0 karşısında 4/4 süreç düşük p 95 verdi; holmes
dar, süreçler arasında tutarsız kaldı. Uzun negatif her iki düzende off 0
karşısında 4/4 süreçte daha yüksek p 95 verdi. Bu gerileme kapatılmadı.

| Düzen | Sorgu / offset / limit | on/off0 p95 | on/off1 p95 | tipik blok on/off0; on/off1 | on/off0 hızlı süreç/blok |
|---|---|---:|---:|---:|---:|
| karışık | quixote / 0 / 50 | -24.31% | -16.92% | -11.47%; -16.96% | 4/4; 20/24 |
| karışık | holmes / 0 / 50 | +2.38% | +5.80% | -1.21%; -1.17% | 3/4; 14/24 |
| karışık | he / 0 / 50 | -5.02% | -0.39% | +3.88%; +6.86% | 1/4; 9/24 |
| karışık | zx / 0 / 50 | -0.90% | +2.94% | -1.21%; +2.06% | 3/4; 15/24 |
| karışık | zx×150 / 0 / 50 | +15.13% | +16.96% | +5.83%; +8.72% | 0/4; 11/24 |
| karışık | the / 0 / 200 | -3.36% | +4.11% | +1.62%; +3.98% | 3/4; 12/24 |
| karışık | ishmael / 130 / 1 | -7.70% | +6.33% | -0.90%; +0.58% | 3/4; 14/24 |
| karışık | quixote / 57 / 50 | +14.95% | +7.72% | +5.71%; +0.13% | 2/4; 6/24 |
| tek sorgu | quixote / 0 / 50 | -27.42% | -14.22% | -21.13%; -22.68% | 4/4; 23/24 |
| tek sorgu | holmes / 0 / 50 | -1.04% | +6.16% | -4.46%; -0.49% | 2/4; 16/24 |
| tek sorgu | he / 0 / 50 | -16.55% | +5.74% | -6.88%; -5.08% | 4/4; 15/24 |
| tek sorgu | zx / 0 / 50 | -9.57% | -2.42% | -3.51%; +0.98% | 4/4; 15/24 |
| tek sorgu | zx×150 / 0 / 50 | +6.25% | +2.48% | +3.69%; +1.93% | 0/4; 9/24 |
| tek sorgu | the / 0 / 200 | -3.90% | +12.49% | -4.23%; +7.43% | 2/4; 13/24 |
| tek sorgu | ishmael / 130 / 1 | -8.68% | +8.23% | +0.19%; +2.29% | 2/4; 12/24 |
| tek sorgu | quixote / 57 / 50 | +2.51% | +12.58% | -11.17%; -2.70% | 2/4; 17/24 |

[Karışık ham veri](benchmarks/reuse-controls-mixed-10k-20261018-summary.json),
[tek sorgu ham veri](benchmarks/reuse-controls-single-10k-20261018-summary.json).

## İş sayıları ve süre profili

Aşağıdaki medyanlar ayrı instrumented karışık koşudan 12 örnek/koşuldur.
Read API sayısı fiziksel disk aktarımı değildir; önceki sorgu metadata sayfa
cache durumunu etkilediği için holmes/sayfa sayıları örnekler arasında
değişebilir. Candidate CRC, dönen belgenin hydration CRC/decode sayısından
ayrıdır; ilk sayaç 0 diye returned belge doğrulaması kaldırılmış sayılmaz.

| Sorgu | Read API off0→on | Candidate CRC off0→on | İkinci CRC/decode off0→on | Checked reuse |
|---|---:|---:|---:|---:|
| quixote / 0 / 50 | 131→81 | 56→56 | 50→0 | 50 |
| holmes / 0 / 50 | 1499.5→1452.5 | 1285→1285 | 50→0 | 50 |
| he / 0 / 50 | 35→35 | 0→0 | 50→50 | 0 |
| zx / 0 / 50 | 1251→1251 | 10000→10000 | 0→0 | 0 |
| zx×150 / 0 / 50 | 0→0 | 0→0 | 0→0 | 0 |
| the / 0 / 200 | 123→123 | 0→0 | 200→200 | 0 |
| ishmael / 130 / 1 | 406.5→406.5 | 260→260 | 1→1 | 0 |
| quixote / 57 / 50 | 102.5→102.5 | 56→56 | 0→0 | 0 |

quixote sonuç hazırlama instrumented medyan 1,542→0,476 ms, contains/visibility
1,483→1,976 ms; kopya tutmanın işi ilk aşamaya da taşınır. holmes sonuç
hazırlama 1,366→0,587 ms, contains 32,037→32,380 ms. Bunlar iç içe sayaçlı
sürelerdir; normal hız kazancı olarak kullanılmaz. Candidate CRC ve yapı/UTF-8
kontrolü korunur; owned byte tekrar schema/UTF-8 kontrolüyle çözümlenir.
Uzun negatifte read/payload CRC/schema/retention/reuse hepsi 0: ek metin
kopyası veya CRC bu gerilemeyi açıklayamaz. Sorgu hazırlığı/dispatch, ayrı
üretilen fonksiyonun yerleşimi ve zamanlayıcı/sıra payları tümüyle ayrılmadı.
[Karışık profil](benchmarks/reuse-controls-timing-mixed-10k-20261018-summary.json),
[tek sorgu profili](benchmarks/reuse-controls-timing-single-10k-20261018-summary.json).

## Rust heap tahsisi

Ayrı allocator build, karışık düzende 12 örnek/koşul. Sorgu başlangıcına göre
requested heap byte/call medyanı; oracle, global canlı heap ve çalışma seti
bu sayıların dışında tutulan anlamlardır. Proses RAM sonucu değildir.

| Sorgu | Ek tepe byte off0→on | Sonuç hayattayken canlı fark off0→on | Tahsis çağrısı off0→on |
|---|---:|---:|---:|
| quixote / 0 / 50 | 1.85453e+06→1.85079e+06 | 1.79976e+06→1.80223e+06 | 2371→2157 |
| holmes / 0 / 50 | 1.40537e+06→1.39563e+06 | 1.33872e+06→1.34119e+06 | 2673.5→2794.5 |
| he / 0 / 50 | 1.57365e+06→1.57365e+06 | 1.50511e+06→1.50511e+06 | 1481→1481 |
| zx / 0 / 50 | 268418→268418 | 1534→1534 | 856→856 |
| zx×150 / 0 / 50 | 2403→2403 | 0→0 | 76→76 |
| the / 0 / 200 | 5.82502e+06→5.82502e+06 | 5.74729e+06→5.74729e+06 | 5696→5696 |
| ishmael / 130 / 1 | 109778→109778 | 44956→44956 | 1226.5→1226.5 |
| quixote / 57 / 50 | 120655→120655 | -121→-121 | 710.5→710.5 |

quixote requested ek tepe 3.745 B, holmes 9.737 B daha düşük; sonuç canlıyken
ikisi de+2.464 B fazla. quixote toplam tahsis çağrısı 214 azalırken holmes 121
artıyor. Negatif ve fallback sorgularında bu heap sayaçları aynı kalır.
Bu, önceki oracle'sız OS ölçümündeki +64 KiB private/+170 KiB working set
gözlemini geçersiz kılmaz: farklı sayaçlar, farklı worker ve ölçüm kapsamı.
Bu tur yeni oracle'sız proses RAM koşusu yapılmadı.
[Karışık tahsis](benchmarks/reuse-controls-allocation-mixed-10k-20261018-summary.json),
[tek sorgu tahsisi](benchmarks/reuse-controls-allocation-single-10k-20261018-summary.json).

## Zamanlayıcı ve CPU bekleme kontrolü

Geniş koşudaki büyük sıçramalar görüldükten sonra ek teşhis yapıldı; ilk
örnekler silinmedi veya başarı sayısından çıkarılmadı. Yaklaşık 60,9 s iki
Get-Process gözleminde benchmark 50,27 CPU-s tüketirken başka uygulamalarda
da yoğun CPU etkinliği vardı. Bu gözlem tek bir uygulamayı neden ilan etmez;
aynı CPU üzerindeki scheduler/interrupt/I/O olaylarının izi değildir.
[Gözlem ve sınırı](benchmarks/reuse-controls-20261018-background-observation.json).

Aynı eski frozen okuyucu, iki süreçte Normal/AboveNormal öncelikle ABBA/BAAB
blokları halinde çalıştı. Her koşul 40 örnek/sorgu; aynı dört sorgu, affinity 4,
query wall saatinin dışında thread CPU/cycle sayaçları. Normal zx uç örneği
702,581 ms wall /281,250 ms thread CPU: fark 421,331 ms. holmes 177,612 ms wall
/31,250 ms CPU:146,362 ms fark. CPU sayaçları 15,625 ms katlarında gözlenir;
quixote/the gibi küçük sorgularda 0 CPU veya bir tick görülmesi kullanışlı
oran vermez. Uzun örneklerde fark tek tick ile açıklanamaz. Ölçülen fark
thread'in çalışmadığı/beklediği süreyi kapsar; nedenini scheduler, I/O veya
belirli uygulamaya paylaştırmaz. Bunun önceki her p 95 artışını açıkladığı
iddia edilmez. AboveNormal zx örneği de 521,193/203,125 ms idi; öncelik
artırmak bütün sıçramaları gidermedi. Global güç planı değişmedi.
[CPU/wall ham örnekleri](benchmarks/reuse-controls-scheduling-10k-20261018-summary.json).

Ardından aynı normal reuse executable'ı tek-process off 0/off 1/on düzeninde
dört süreç ×6 blok ×5 tekrar, sekiz sorgu,120 örnek/koşul ile geçici
AboveNormal öncelikte tekrar ölçüldü. Öncelik her child process için Win 32
API ile doğrulandı (32768); süreç kapanınca biter. Ortamın sabitlendiği
varsayılmadı: quixote kapalı-kapalı p 95+%25,60/tipik+%9,98. quixote
on/off 0 tipik-%20,39, on/off 1-%19,25; p 95-%11,69/-%29,69. holmes p 95
-%21,39/-%13,02 olsa da tipik-%0,01/-%1,04; genelleştirilemez.

| Sorgu / offset / limit | off1/off0 p95 | on/off0 p95 | on/off1 p95 | tipik on/off0; on/off1 |
|---|---:|---:|---:|---:|
| quixote / 0 / 50 | +25.60% | -11.69% | -29.69% | -20.39%; -19.25% |
| holmes / 0 / 50 | -9.62% | -21.39% | -13.02% | -0.01%; -1.04% |
| he / 0 / 50 | +19.79% | -3.14% | -19.14% | +1.97%; -6.51% |
| zx / 0 / 50 | -9.82% | -20.24% | -11.56% | +1.65%; +0.04% |
| zx×150 / 0 / 50 | +2.41% | +8.52% | +5.97% | +0.37%; +5.25% |
| the / 0 / 200 | +25.53% | -6.46% | -25.49% | -2.75%; -1.73% |
| ishmael / 130 / 1 | +12.33% | -6.60% | -16.85% | +0.29%; -0.38% |
| quixote / 57 / 50 | -14.59% | -37.58% | -26.91% | +0.28%; +2.59% |

[Öncelikli tekrar](benchmarks/reuse-controls-priority-10k-20261018-summary.json),
[tam çalıştırma kaynağı](benchmarks/reuse-controls-20261018-environment-probe.py).
Bu seçilmiş sekiz sorguluk tekrar, yeni temiz bir 38-sorgu veya oracle'sız
proses RAM kabul koşusunun yerine geçmez.

## Canlı heap farkının kapasite nedeni

Ayrı, süre ölçmeyen aynı fixed DB kontrolünde 50 sonuç için kapalı yol
hits.capacity=52, açık yol 66; StoredDocument 176 B.14×176=2.464 B, hem
quixote hem holmes için bütün heap örneklerindeki canlı farkla eşleşti.
the/200 fallback'te 209/209; ek fark 0. Tam belge sonuçları eşit kaldı.
Arşiv kodunda reused Vec::new→geometrik büyüme ve hydrated.into_iter()
collect, dönüş listesine fazla kapasite taşır. Bu nedenle sorgu sonundaki
+2.464 B'ı hâlâ saklanan encoded payload kopyaları diye açıklamak yanlıştır.
Önceki+64 KiB proses private farkının tamamı da bununla açıklanamaz.
Bu ek kontrol off/on sonuçlarını karşılaştırmak için birlikte canlı tutar;
proses RAM veya peak benchmark'ı değildir.
[Kapasite sonucu](benchmarks/reuse-controls-20261018-capacity.json),
[read-only kontrol kaynağı](benchmarks/reuse-controls-20261018-capacity-probe.rs).

## Karar ve sonraki yön

Checked-page varsayılan geçişi reddedilmiş olarak kalır. Kullanılan motor
değiştirilmedi; yalnız ölçüm aracı ve kanıtlar eklendi. Eski geniş p 95
artışlarının tamamı dalgalanma diye kapatılmadı. Büyük uç sürelerde CPU
dışı bekleme kanıtlandı; farklı executable derleme etkisinin bütün payı
veya belirli uygulamanın sorumluluğu kanıtlanmadı. quixote'ta ikinci okumayı
kaldırmanın somut işi ve dar tipik blok kazancı var. holmes'ta genel
tekrarlanabilir p 95 kazancı yok. Uzun negatif iki normal sorgu düzeninde
off 0'a göre 4/4 süreçte daha yüksek p 95; payload/CRC/heap işi aynı/sıfır
olduğu için CRC veya codec değiştirmek bu kalemi hedeflemez.

Sonraki kod denemesi önce boş/negatif sonuçların ortak yolunu korumalı;
yeniden kullanım işi gerçek pozitif belge yakalandığında ayrı helper'da
çalışmalı. Pozitif sonuç vektörünün kapasitesini sayfa sınırına göre
ayırmak, ölçülen 2.464 B artışı hedefleyen küçük ve doğrulanabilir düzeltmedir.
Encoded kopya yerine zaten doğrulanmış view'dan tek sahipli sonuç üretme
ayrı bir hipotez olarak kalır; hız/RAM kazancı henüz ölçülmedi.2 MiB/50
eşiklerini değiştirme veya yeni kalıcı format planlanmadı. Yeni tasarım
için aynı-program plasebo ve bütün 38 sorgu, oracle'sız proses RAM kontrolü
birlikte geçmeden varsayılana alınmamalı.

Yazma/bakım, diğer arama regresyonları, normal Vec/query/caller RAM, genel
açılış, disk sıkıştırma ve eşzamanlı/gerçek-cold yük işleri açık kalır.
[İş listesi](OPEN-WORK.md).

## Doğrulama ve veriyi koruma

Geçici prototipte default/all-targets 190 geçti/7 ignored, reference 111/3,
all-features 209/9;0 failure. Yeni on/off/default/direct karşılaştırması,
schedule/komut sınırı, cut/tek-byte corruption, süreç kill ve değişmeden
0.1 fixture açılışı bu testlerde yer alır. Power-loss doğrulaması değildir.
Prototipte fmt --check ve default/all-features/reference all-targets Clippy
-D warnings geçti. Normal/timing/allocation build komutları ve executable
SHA'ları [build kaydında](benchmarks/reuse-controls-20261018-build.json),
test komutları [kontrol kaydında](benchmarks/reuse-controls-20261018-tests.json).

Deneme API'sı, worker/example ve checked-page helper kaldırıldı. Dört
değişen Rust dosyası başlangıç raw byte'larına geri yüklendi; src/tests/
Cargo diff'i boş. Kaynak SHA ve arşiv ters/ileri apply kontrolü kayıtlıdır.
[Yedi dosyalı arşiv](benchmarks/reuse-controls-20261018-probe.patch),
[ölçülen kaynak](benchmarks/reuse-controls-20261018-source.json),
[geri dönüş](benchmarks/reuse-controls-20261018-rollback.json).

Geri dönülen motorda 7 window/corruption testi, fmt --check ve default/
all-features all-targets Clippy tekrar geçti;190/111/209 tam sonuçları
geçici prototipe aittir, aktif eski motorda tam-suite yeniden koşulmadı.
[Final kontroller](benchmarks/reuse-controls-20261018-final-checks.json).

On ölçüm fazında 21 dosyanın önce/sonra SHA ve boyları başlangıçla aynı;
kapasite kontrolü sonrası da aynı.18 FDB dosyası 349.805.698 B, SQLite
583.860.224 B ve iki 0.1 fixture korunur. Corpus'un tamamı 2.740.862.279 B
ve önceki SHA-256 ile eşleşti. Data altında yalnız iki mevcut kök klasör
vardır. Yeni corpus/DB/reset/kopya/başka disk kullanılmadı.
[Veri kanıtı](benchmarks/reuse-controls-20261018-data-proof.json).
