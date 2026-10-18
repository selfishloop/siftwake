# Arka plan işleri kapatıldıktan sonra aynı programla tekrar — 18 Ekim 2026

Aynı frozen executable ve aynı mevcut 10.000 belgeyle yapılan tekrar, dar
quixote kazancını doğruladı. Geniş 38 sorguda iki kapalı kontrole göre p95
-%11,64/-%10,59; yoğun dört sorgu tekrarında -%13,68/-%14,96. Her ikisinde
4/4 süreç hızlı. Uzun negatifte küçük bir artış sürüyor; genel hız/RAM
geçişi bu tur da doğrulanmadı. Aktif motorun kodu değişmedi.

## Ortam ve aynı kalan ölçüm koşulları

Kullanıcı yoğun arka plan işlerini kapattığını bildirdikten sonra koşuldu.
Öncesindeki yaklaşık 15,2 saniyelik kontrolde iki EA süreci 19,3 CPU-s
tüketiyordu. Kapatma sonrası yaklaşık 59,8 saniyelik kontrolde aynı yoğun
EA/Chrome etkinliği görülmedi. Bu iki gözlem farklı uzunlukta ve hayatta
kalan PID karşılaştırmalarıdır; kontrollü scheduler izi veya belirli bir
uygulamanın bütün gecikmeyi açıklaması değildir.
[Önce](benchmarks/reuse-repeat-20261018-before-close-cpu.json),
[sonra](benchmarks/reuse-repeat-20261018-after-close-cpu.json).

Program SHA-256: `0324F3CC03424CB8FB63DE010AB337B22091A1A60E92B3C401D7472B417725B2`.
Önceki deneyin aynı normal, sayaçsız release executable dosyası; yeniden
derleme yok. Windows/MSVC, Ryzen 7 7840HS, Balanced güç planı, affinity mask
4 (tek mantıksal CPU), doğrulanmış Normal süreç önceliği. Sıcak OS dosya
cache: strict kaynak/indeks açılışı ve bağımsız oracle ısınması saat dışında;
eviction yapılmaz, bütün sayfaların sürekli RAM içinde kalması garanti edilmez.
Frekans/SMT/kernel/arka plan işlerinin tamamen yalıtıldığı iddia edilmez.

Corpus aynı ilk 10.000 kayıt: sekiz kitap, 264.842.858 metin byte’ı, belge
başına 2.057–51.197 byte. Sabit `data/benchmark-work/fdb` kullanılır; mevcut
`data/rkyv-corpus-20261009/json.payload-bench` yeniden okunur. Yeni corpus,
DB kopyası, reset/repack veya başka diske taşıma yok. SQLite yalnız byte/hash
koruma kontrolünde yer alır; bu yeni bir SQLite hız karşılaştırması değildir.

Tam belge döndüren contains, iki karakter, AND, Türkçe fold ve sayfa sorguları
aynı eski 38-case setidir. Query hazırlığı ve search saat içinde; total, sıra,
title/text/fields/links/version doğrulaması ve sonucu bırakma saat dışında.
Candidate CRC/schema/UTF-8/key/version aynı; açık denemede yalnız aynı sahipli
doğrulanmış byte’ların ikinci dosya okuma/CRC’si atlanır, şema tekrar kontrol
edilir. Source tam doğrulanır; sağlam indeks bozuk kaynağı geçerli kılmaz.

Dört bağımsız süreçte aynı program içinde off0/off1/on: iki kapalı kontrol
aynı kod yoludur. Her süreç altı olası koşul sırasını birer kez çalıştırır;
aynı tohumlar kullanılır, her koşul her konumda iki kez bulunur. Geniş koşu
6 blok ×5 tekrar ×4 süreç =120 örnek/sorgu/koşul. Sayfalar da her tur çalışır.
Bloklar strict açılıp kapanır; her tur 256 mevcut/üç eksik get vardır.

Kapalı probe da arşiv prototipin ortak kodunu içerir; standart mevcut okuyucu
ile bit-eşit değildir. Aynı-program farkı tek başına genel ürün geçiş kapısı
değildir. Kontrol farkları A/B yüzdelerinden çıkarılmaz. Pooled p95 ve tipik
eşlenmiş blok medyanı farklı istatistiklerdir; ikisi de aşağıda gösterilir.

## Ortam gözlemi

72 blok wall aralığı 4.465–4.822s;
child CPU/wall medyanı %99.60, en düşük %96.44.
Bu, strict açılış/ısınma/get/oracle işlerini de içeren bütün blok ölçümüdür;
tek sorgunun CPU veya scheduler profili değildir. Sayaçlar query saatinin
dışında okunur; eşzamanlı değiller ve zaman çözünürlüğü nedeniyle küçük
%100 üstü oranlar da ham veride korunur. Diğer uygulamalar+sistem toplam CPU
tahmini medyan 0.905 mantıksal çekirdek eşdeğeridir;
ortam boş değildir. Önceki 703ms wall/281ms CPU gibi uç beklemelerle aynı
kapsamda bir örnek bu tur ayrıca ölçülmedi; blokların daha kararlı olduğu
gözlendi. [Ham blok CPU](benchmarks/reuse-repeat-10k-20261018-block-cpu.jsonl),
[özet ve sınır](benchmarks/reuse-repeat-10k-20261018-cpu-summary.json),
[çalıştırılan kaynak](benchmarks/reuse-repeat-20261018-run.py).

## Geniş 38 sorgu: 120 örnek/koşul

off1/off0 25 düşük/13 yüksek p95; on/off0 25/13; on/off1 17/21. Bu sayılar
tek başına başarı/gerileme veya istatistiksel anlamlılık ilanı değildir.
quixote kapalı-kapalı p95 -%1,18, tipik +%0,49; önceki koşudaki -%56,91
kapalı-kapalı sapma bu tekrarda yok. quixote on/off0 ve on/off1 4/4 süreç
ve 23/24 blok hızlı; tipik kazanç %16,36/%15,95. holmes p95 -%2,16/+%1,42,
tipik -%1,91/-%1,70: genel tekrarlanabilir p95 kazanımı diye sunulmaz.
zx’in önceki %15 mertebesindeki artışı bu koşuda tekrar çıkmadı; yeni fark
+%0,68/+%1,58, tipik +%0,18/+%0,03. Diğer eski regresyonların hepsinin
nedeninin çözüldüğü veya kapandığı iddia edilmez.

| Sorgu / offset / limit | Kapalı0 p95 ms | Açık p95 ms | Açık/kapalı0 | Açık/kapalı1 | Tipik blok 0;1 | Hızlı süreç 0;1 |
|---|---:|---:|---:|---:|---:|---:|
| quixote / 0 / 50 | 2.7187 | 2.4023 | -11.64% | -10.59% | -16.36%; -15.95% | 4/4; 4/4 |
| zeuglodon / 0 / 50 | 2.6482 | 2.3106 | -12.75% | -7.22% | -8.34%; -8.84% | 3/4; 3/4 |
| beagle / 0 / 50 | 17.4420 | 17.4952 | +0.31% | -0.10% | -2.01%; -2.17% | 1/4; 2/4 |
| mesopotamia / 0 / 50 | 2.8458 | 2.7695 | -2.68% | +0.94% | -9.34%; -8.56% | 3/4; 2/4 |
| transubstantiation / 0 / 50 | 43.3336 | 45.1591 | +4.21% | +0.63% | +0.04%; -0.22% | 1/4; 0/4 |
| harpoon / 0 / 50 | 50.7625 | 50.6604 | -0.20% | +0.40% | -1.15%; -0.77% | 2/4; 2/4 |
| ishmael / 0 / 50 | 6.4985 | 6.5669 | +1.05% | +3.22% | -6.25%; -8.04% | 2/4; 1/4 |
| invertebrata / 0 / 50 | 10.2405 | 10.1314 | -1.07% | +0.95% | -2.82%; -3.31% | 3/4; 1/4 |
| darcy / 0 / 50 | 42.7262 | 42.4519 | -0.64% | +2.81% | -0.84%; -0.67% | 1/4; 2/4 |
| elizabeth / 0 / 50 | 39.0581 | 39.1947 | +0.35% | +0.55% | -1.00%; -0.41% | 2/4; 3/4 |
| watson / 0 / 50 | 38.1477 | 36.2973 | -4.85% | -1.85% | -0.88%; -0.79% | 3/4; 2/4 |
| holmes / 0 / 50 | 22.8327 | 22.3396 | -2.16% | +1.42% | -1.91%; -1.70% | 3/4; 2/4 |
| frankenstein / 0 / 50 | 13.9484 | 12.6071 | -9.62% | -9.19% | -3.88%; -2.96% | 3/4; 3/4 |
| galapagos / 0 / 50 | 9.0755 | 8.6574 | -4.61% | -9.13% | -3.79%; -4.25% | 2/4; 3/4 |
| fossil / 0 / 50 | 24.0124 | 25.1314 | +4.66% | +6.39% | -1.74%; -2.08% | 2/4; 2/4 |
| geologist / 0 / 50 | 14.7917 | 14.3484 | -3.00% | +0.13% | -3.58%; -3.73% | 4/4; 2/4 |
| the / 0 / 50 | 1.0061 | 0.9576 | -4.82% | -2.44% | +1.67%; +1.24% | 2/4; 3/4 |
| man / 0 / 50 | 1.3799 | 1.3322 | -3.46% | +9.46% | +0.31%; +0.41% | 3/4; 2/4 |
| he / 0 / 50 | 1.1292 | 0.9974 | -11.67% | -13.88% | -0.00%; -0.61% | 3/4; 2/4 |
| zzxqnever / 0 / 50 | 0.0400 | 0.0414 | +3.50% | +2.48% | -1.48%; -1.99% | 1/4; 1/4 |
| zx / 0 / 50 | 116.7691 | 117.5673 | +0.68% | +1.58% | +0.18%; +0.03% | 2/4; 2/4 |
| the zx / 0 / 50 | 115.7072 | 115.0863 | -0.54% | -1.09% | +0.07%; +0.19% | 2/4; 2/4 |
| quixote whale / 0 / 50 | 2.7876 | 2.4745 | -11.23% | -12.06% | -10.54%; -12.99% | 4/4; 3/4 |
| holmes elementary / 0 / 50 | 31.2813 | 31.2366 | -0.14% | -0.58% | -1.80%; -1.35% | 3/4; 2/4 |
| fossil geologist / 0 / 50 | 10.6529 | 10.2843 | -3.46% | -4.70% | -5.68%; -5.00% | 3/4; 3/4 |
| whale harpoon / 0 / 50 | 53.2230 | 51.3286 | -3.56% | +0.97% | -0.94%; -0.96% | 3/4; 1/4 |
| QUİXOTE / 0 / 50 | 2.8090 | 2.4138 | -14.07% | -10.08% | -16.41%; -17.49% | 4/4; 3/4 |
| İSHMAEL / 0 / 50 | 6.4381 | 6.1692 | -4.18% | -4.23% | -8.08%; -7.29% | 4/4; 4/4 |
| zx×150 / 0 / 50 | 0.0326 | 0.0349 | +7.06% | +15.56% | +2.09%; +5.39% | 2/4; 0/4 |
| presumption / 0 / 50 | 64.4684 | 64.9076 | +0.68% | +1.36% | -0.65%; -0.60% | 1/4; 1/4 |
| the / 0 / 1 | 0.2532 | 0.2787 | +10.07% | +9.38% | +0.31%; +4.04% | 0/4; 0/4 |
| the / 0 / 200 | 4.2295 | 4.2929 | +1.50% | +3.63% | -1.19%; -3.29% | 2/4; 2/4 |
| the / 5000 / 50 | 1.2519 | 1.3695 | +9.39% | +3.25% | -1.67%; -2.63% | 2/4; 2/4 |
| the / 9950 / 50 | 1.2680 | 1.1721 | -7.56% | -9.52% | +0.28%; -0.51% | 3/4; 3/4 |
| ishmael / 130 / 1 | 5.4684 | 5.8503 | +6.98% | +7.76% | +1.17%; +0.63% | 0/4; 1/4 |
| ishmael / 60 / 200 | 10.8777 | 10.6318 | -2.26% | -1.11% | -2.86%; -0.55% | 3/4; 3/4 |
| quixote / 28 / 10 | 2.0712 | 2.0014 | -3.37% | -1.34% | +1.04%; +0.57% | 3/4; 2/4 |
| quixote / 57 / 50 | 1.9178 | 1.8771 | -2.12% | +2.83% | +0.30%; +0.74% | 2/4; 2/4 |

[Bütün örnekler, süreç/blok ve leave-one-out sonuçları](benchmarks/reuse-repeat-10k-20261018-summary.json).

## Kısa yoğun kontrol: 240 örnek/koşul

Geniş koşudan sonra quixote, uzun negatif, the/0/1 ve ishmael/130/1 aynı
program/veriyle tekrarlandı: 4 süreç ×6 blok ×10 tur, aynı get serpiştirmesi.
Sorgu komşuları ve ısınan sorgu seti daraldığı için bu yeni bir workload’dur;
38-case sonucuna eklenmez veya onun yerine geçirilmez. CPU gözlem wrapper’ı
bu kısa koşuda kullanılmadı. Seçim gerileme/kontrol içindir; motorda sözcüğe
göre bir optimizasyon yapılmadı.

| Sorgu / offset / limit | Kapalı0/kapalı1/açık p95 ms | Açık/kapalı0 | Açık/kapalı1 | Tipik blok 0;1 | Hızlı süreç 0;1 |
|---|---:|---:|---:|---:|---:|
| quixote / 0 / 50 | 2.2592/2.2931/1.9501 | -13.68% | -14.96% | -15.93%; -15.46% | 4/4; 4/4 |
| zx×150 / 0 / 50 | 0.0315/0.0307/0.0324 | +2.86% | +5.54% | +1.97%; +4.85% | 2/4; 0/4 |
| the / 0 / 1 | 0.1819/0.1683/0.1769 | -2.75% | +5.11% | -1.88%; +0.17% | 3/4; 2/4 |
| ishmael / 130 / 1 | 5.1649/5.2886/5.1701 | +0.10% | -2.24% | -0.19%; -0.13% | 1/4; 2/4 |

quixote iki kontrol karşısında 4/4 süreç ve 24/24 blok hızlı; tipik kazanç
%15,93/%15,46. Uzun negatif p95 0,0315/0,0307→0,0324ms: +0,9/+1,7µs,
+%2,86/+%5,54; ikinci kontrole göre 4/4 süreç yavaş, ilkine göre 2/4.
Tipik +%1,97/+%4,85. Küçük artık artış kapatılmadı; salt payload CRC veya
kopyalama nedeniyle olduğu söylenemez: önceki profilde bu sorguda o işler 0.
the/1 geniş koşuda +%10,07/+%9,38 iken burada -%2,75/+%5,11; ishmael/130/1
genişte +%6,98/+%7,76, burada +%0,10/-%2,24. Bu iki sayfa artışı dar yükte
iki kontrole göre tutarlı tekrarlanmadı; geniş yükteki sonuç yok sayılmadı.
Sorgu sırası/durum/kod yerleşimi etkileri tümüyle ayrılmış değildir.
[Yoğun kontrol ham verisi](benchmarks/reuse-repeat-focus-10k-20261018-summary.json).

## Karar, sonraki küçük iş ve açık sınırlar

Arka plan işleri kapatıldıktan sonra karşılaştırma belirgin biçimde daha
kararlı; quixote dar kazancı tekrarlandı. Genel 38-case p95 geçişi ve
oracle’sız OS proses RAM kabulü bu tur doğrulanmadı. Checked-page prototip
varsayılana alınmadı; aktif motorun src/tests/Cargo kaynakları değişmedi.
Sonraki kod işi hâlâ sonuç Vec kapasitesindeki ölçülmüş 2.464B fazlalığı
gidermek ve ortak boş/negatif yolu koruyarak pozitif helper işini ayırmaktır.
Bu henüz uygulanmış/hızlanmış bir çözüm değildir. Eşik/format/bağımlılık
değişikliği yapılmadı. Önceki [kapasite ve heap kanıtı](REUSE-CONTROLS-10K-20261018.md)
aynı frozen kodla ilgilidir; bu tur yeniden RAM ölçümü yapılmış sayılmaz.

Yazma/bakım, diğer arama regresyonları, normal Vec/query/caller RAM, genel
açılış, disk sıkıştırma ve eşzamanlı/gerçek-soğuk yük işleri açık.
Bu tekrar yeni SQLite, disk boyutu kazancı, cold veya concurrent kıyası değildir.
[Güncel iş listesi](OPEN-WORK.md).

## Doğrulama ve veri bütünlüğü

Her geniş sorgu/koşulda 120, her dar sorgu/koşulda 240 örnek; hepsinde
bağımsız corpus oracle’ıyla tam sonuç kontrolü geçti. Normal kaynak/indeks
doğrulaması ölçüm dışına alınır fakat kaldırılmaz. fmt --check ve
clippy --all-targets -- -D warnings geçti. Rust/depolama değişikliği yok;
bu tur eski tam-suite/kill sonuçları yeni çalıştırılmış gibi sunulmaz.
İki fazın önce/sonra snapshot’ları ve final kontrol aynı 21 dosyada eşit:
18 FDB dosyası 349.805.698B, SQLite 583.860.224B, iki 0.1 fixture. Corpus’un
tamamı 2.740.862.279B ve önceki SHA-256 ile eşleşti. Data altında aynı iki
kök klasör var; yeni veri üretimi/reset/kopya yok.
[Bütünlük, örnek sayıları ve kontroller](benchmarks/reuse-repeat-20261018-proof.json).
