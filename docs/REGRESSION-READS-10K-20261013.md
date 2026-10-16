# Önceki 11 arama artışının incelenmesi

**2026-10-13 kullanıcı kararı:** kalan grouped iki / default üç p95 artışı
ölçüm dalgalanması olarak kabul edildi; arama regresyon incelemesi kapatıldı.
Bu kapanış kullanıcı kabulüdür, ek istatistiksel kanıt değildir. Aşağıdaki
ham sonuçlar ve ölçüm anındaki değerlendirmeler tarihsel kanıt olarak korunur.
Runtime özellikleri bu kararla değiştirilmedi; pencere default kapalıdır.

Önceki V2 raporunda yükselen 11 senaryonun tamamında, aynı özelliklerle
derlenen kontrol sürümüne karşı yeni üçlü ölçümde pooled p95 düşüktür.
Sekizi üç eşleşmede de hızlıdır; watson, geologist ve quixote offset57 iki
eşleşmede hızlıdır. Bu, bütün koşularda bütün sorgular hızlandı demek değildir.
Genel matris 36 düşüş / 2 artıştır; pencere hâlâ default kapalıdır.

## Neden ve değişiklik

Diagnostic profilde 11 senaryonun 10'unda window hit sıfırdır: bunlar
individual payload okumalarını kullanır. Bu nedenle bütün artışları read-ahead
ile açıklamak doğru değildir. Önceki V2 ile final gated build de aynı özellik
seti değildir: grouped-read, direct-slot hydration'ı açar. Eski oturumdaki
mutlak sürelerle yeni oturumdakiler doğrudan karşılaştırılmaz.

Presumption profili gerçek bir verimsizliği gösterdi: 229 window hit'e karşı
92.702.693 requested byte için 113.282.862 byte okuma, yaklaşık %22 overread.
Final profilde aynı requested byte için 93.423.204 byte okuma (~%0,78
overread), 2531 request / 10 window hit görülür. Daha az read-ahead,
yaklaşık19,86 MB gereksiz okumayı kaldırır; request sayısını minimuma indirme
hedefi değildir. Instrumented geologist fold-body span6,09→3,64 ms,
transubstantiation27,95→17,45 ms; bunlar tek diagnostic gözlemlerdir,
paired performans kanıtı yerine geçmez. Profiller:
`benchmarks/regressions-{before,after}-profile.jsonl`.

Korunan değişiklikler:

- Karışık Unicode metindeki ASCII aralığının sonunu güvenli sekiz baytlık
  yüklemelerle bulma. Türkçe eşleme, Unicode lower-case genişlemesi ve chunk
  sınırları aynı; unchecked/unaligned/unsafe erişim yok.
- Query token'larında scalar `fold` yerine aynı sonucu veren `fold_into`.
  Uzun ASCII negatif sorguda gereksiz karakter başına Unicode işi azalır.
- Experimental pencerede tamamlanan pencerenin kullanımını izleme. Kullanılan
  payload baytları pencerenin dörtte üçünden azsa bu segment sorgusunun kalanı
  exact individual read'e döner. Taze pencerenin henüz kullanılmamış baytları
  erken değerlendirilmez. Broad seed eşiği eski yarı-live eşiği olarak kalır;
  üç fiziksel bitişik kayıttan önce read-ahead başlamaz.

Query kelimesine özel runtime koşul yok. CRC, OFF1 UTF-8/schema/identity,
VFY touched-block ve authoritative source kontrolleri korunur. Default
normalleştirme hızlandırmasını kullanır; window alanları/yolu mevcut
experimental-grouped-read altında kalır. Yeni feature, bağımlılık veya
kalıcı format eklenmedi; 0.1 fixture değişmedi.

İki ara deneme kabul edilmedi:

- Reusable memmem Finder: 17 düşüş / 21 artış. Doğrudan memchr bağımlılığı ve
  finder alanları kaldırıldı. `regressions-finder-matrix-*` logları korunur.
- Normalleştirme + daha katı 3/4 seed eşiği: 30 düşüş / 8 artış; man p95
  %3,20 arttı. Seed eşiği geri alındı, yerine kullanım geri bildirimi eklendi.
  `regressions-fold-matrix-*` logları ayrı tutulur.

## Aynı özelliklerle paired ölçüm

Kontrol: `output/regressions-grouped-control-20261013.exe`, değişiklik öncesi
final gated kod. Aday: `output/regressions-adaptive-20261013.exe`. İkisinde
release merge-postings + delta-bitmaps + grouped-read, diagnostic kapalı.
SHA256 değerleri `benchmarks/regressions-adaptive-matrix-10k-20261013-executables.json`.

Aynı mevcut ilk 10.000 belge, `data/rkyv-corpus-20261009/json.payload-bench`;
264.842.858 text byte, 2.057–51.197 byte/doc. Sekiz Gutenberg kitabının
benzersiz/örtüşen pasajları; yeni corpus hazırlanmadı. Mevcut 5-run FDB,
sequence20, delta0. Sabit DB slotları yeniden kullanıldı; prepare/reset/rebuild
yok. OS cache sıcak, eviction yapılmadı. Record-read API; kaynak ilk aramada
tam doğrulanır, timed warm sorgularda aynı source guard sözleşmesi geçerlidir.

Üç paired process/variant, seed0/1/2, sıra dönüşümlü. Aynı 38 sorgu/sayfa;
primary20 sample/process (pooled60), page5 (pooled15). Nearest-rank p95.
Bağımsız corpus oracle total/order/page/full StoredDocument/version kontrolü
API timer dışındadır; okuma/CRC/schema/normalleştirme/tarama ve hydration
timer içindedir. Pooled sonuçlar farklı prototiplerden seçilip birleştirilmez.

| Önce yükselen senaryo | Kontrol p95 ms | Aday p95 ms | Fark | Hızlı çift |
|---|---:|---:|---:|---:|
| zeuglodon | 2,9833 | 2,6604 | -%10,82 | 3/3 |
| transubstantiation | 74,8030 | 61,4932 | -%17,79 | 3/3 |
| harpoon | 87,8052 | 74,9945 | -%14,59 | 3/3 |
| ishmael | 10,3314 | 8,9066 | -%13,79 | 3/3 |
| darcy | 65,5017 | 56,8251 | -%13,25 | 3/3 |
| watson | 58,3492 | 56,1865 | -%3,71 | 2/3 |
| geologist | 21,5269 | 20,3369 | -%5,53 | 2/3 |
| 300-byte negatif | 0,0967 | 0,0834 | -%13,75 | 3/3 |
| presumption | 114,3833 | 95,6732 | -%16,36 | 3/3 |
| ishmael offset130 | 9,8559 | 8,6922 | -%11,81 | 3/3 |
| quixote offset57 | 2,5583 | 2,3469 | -%8,26 | 2/3 |

Genel matriste açık kalan iki artış:

- the offset0 limit1: 122,3972 → 127,6839 ms, +%4,32; iki çift hızlı,
  biri yavaş. Kontrol run p95:122,3972/116,2397/118,8389;
  aday:127,6839/110,8478/111,2870.
- quixote offset28 limit10: 3,0622 → 3,4524 ms, +%12,74; iki çift hızlı,
  biri yavaş. Kontrol:2,6746/2,8936/3,0622; aday:2,5161/3,4524/2,8693.

İki artışın nedeni kesinleşmedi; ölçüm dalgalanması diye kapatılmadı.
Page p95 yalnız15 örneğe dayanır. Genel regresyon kapısı geçti iddiası yok.
Ham log/summary prefix: `benchmarks/regressions-adaptive-matrix-10k-20261013`.

FDB/cache ve SQLite DB byte/SHA256 önce/sonra eşittir; DB disk artışı0 B.
SQLite bu tur yeniden ölçülmedi. RAM/working set, soğuk cache, farklı donanım,
yazma/bakım ve eşzamanlı yük bu ölçümün kapsamında değildir. Diagnostic wall
span'lar nested olduğundan toplanmaz; request sayaçları fiziksel disk I/O değildir.

## Ayrı varsayılan okuma yolu ölçümü

Pencere ve grouped hydration kapalı olarak da aynı38 case, aynı üç paired
process/20-primary/5-page sözleşmesi tekrarlandı. İki tarafta yalnız release
merge-postings+delta-bitmaps; diagnostic kapalı. Kontrol önceki safe UTF-8
`output/window-read-control-20261013.exe`; aday güncel normalleştirme kodudur.
Prefix `benchmarks/regressions-default-matrix-10k-20261013`.

Bu ayrı koşu **35 düşüş / 3 artış** gösterdi. Önceki11 senaryonun10'unda
pooled p95 düşük; 300-byte negatif sorgu0,0819→0,0824 ms (+%0,61,
yaklaşık0,5 mikro saniye) ve2/3 pair hızlıdır. Diğer iki genel artış:
he134,0162→136,2779 ms (+%1,69,1/3 pair hızlı),
zzxqnever0,1116→0,1127 ms (+%0,99,2/3 pair hızlı).
Bu üç fark ölçüm dalgalanması diye kapatılmadı; genel no-regression iddiası yok.

Default geologist19,6658→18,4590 ms (-%6,14,3/3 hızlı), quixote offset57
2,5533→2,1014 ms (-%17,70,3/3 hızlı), presumption106,9976→85,8461 ms
(-%19,77,3/3 hızlı). Varsayılan sonuçlar grouped36/2 ile birleştirilmez;
hydration yolu ve farklı oturum etkileri nedeniyle iki tablonun mutlak
sürelerinden özellik kazancı çıkarılmaz. FDB/cache ve SQLite DB hash'leri aynı.

## Doğrulama ve takip

Unicode scalar karşılaştırması, ASCII byte scan hizalama/sınır testi ve
chunk/cache testleri geçti. Adaptive window testleri dense/sparse geçiş,
prefetched corrupt record, VFY bozuk blok/kesilme, büyük kayıt ve sıra
değişimlerini kapsar.

Final `cargo test --all-targets` ve `cargo test --all-targets --all-features`,
`cargo fmt --check`, default ve all-features `cargo clippy --all-targets --
-D warnings` geçti. Source corruption, kesilme, süreç sonlandırma ve0.1
uyumluluk testleri bu suite'lerde korundu. Loglar:
`benchmarks/regressions-tests-{default,all-features}.log`,
`regressions-fmt.log`, `regressions-clippy{,-all-features}.log`.
Release ölçüm binary'si sonunda grouped/diagnostic kapalı merge+bitmap
build olarak bırakıldı; deneysel ölçüm artifact'i output altında ayrı korundu.

[OPEN-WORK](OPEN-WORK.md): grouped iki sayfa artışı ve default üç artış,
güncel SQLite yazma/bakım,
RAM/disk ve eşzamanlı bakım yükü açık tutulur. Önceki 11 senaryoda yeni
pooled p95 düşüşü, bütün bu işlerin tamamlandığı anlamına gelmez.
