# 10k aday seçiciliği ve ASCII-run normalleştirme

## Ölçülmüş darboğaz

Mevcut 10k DB üzerinde, sıcak OS cache ve boş delta ile altı sorgu profillendi.
Her sorguda bir ölçümsüz warmup, ardından üç diagnostic-timing tekrarı var.
Toplam ve tam sıralı StoredDocument sayfası bağımsız corpus oracle'ına eşit.

| Sorgu | Görünür seed ID | Payload adayı | Gerçek sonuç | Posting kesişimi medyan ms | Metin doğrulama medyan ms |
|---|---:|---:|---:|---:|---:|
| beagle | 837 | 579 | 46 | 0,17 | 31,41 |
| holmes elementary | 1.440 | 924 | 65 | 0,71 | 56,01 |
| transubstantiation | 3.619 | 1.527 | 0 | 1,49 | 85,09 |
| zx | 10.000 | 10.000 | 0 | 0,44 | 370,19 |
| quixote | 56 | 56 | 56 | 0,01 | 1,54 |
| the | 10.000 | 10.000 | 10.000 | 0,53 | 19,50 |

Trigramların aynı belgede bulunması sözcüğün bitişik geçtiğini kanıtlamaz.
`beagle`/AND/uzun negatif sorguda çok sayıda false-positive payload okunuyor.
`zx` iki karakter olduğundan trigram üretmez ve bütün görünür belgeleri doğrular.
Posting kesişimi bu örneklerde ana maliyet değildir. Metin doğrulama span'i
normalleştirme, arama, örtüşme taşıma ve scratch işlemlerini kapsar.
`the` erken eşleşir; onda CRC ve payload okuma daha baskındır.

## Küçük değişiklik

`fold_into` tamamen ASCII girdideki mevcut hızlı yolu korur. Karışık girdide
Unicode karakterleri aynı Türkçe/Unicode kurallarıyla indirger; aradaki ASCII
run'larını topluca kopyalayıp ASCII lowercase uygular. Tek Unicode noktalama
işareti nedeniyle bütün 1 KiB blokta char başına String push yapılmaz.
Chunk sınırı/örtüşmesi, tam CRC, OFF1 grammar/UTF-8/kimlik doğrulaması,
kesin toplam ve tam belge hidrasyonu değişmez. Format, feature ve DB değişmez.

Profilde gövde fold medyanı beagle24,44→15,84; AND45,53→32,00;
transubstantiation65,27→40,51; zx302,09→208,31 ms oldu.
Bu alt süreler instrumented ölçümdür; normal performans sonucu değildir.

## Yöntem ve sınırlar

Kaynak: aynı ilk 10.000 belge, `json.payload-bench`; toplam metin264.842.858 B,
belge2.057–51.197 B, sekiz Gutenberg eserinden benzersiz ve kısmen örtüşen
pasajlar. Yeni corpus/DB yok. FDB sabit slot5 segment, seq20, boş delta.
SQLite aynı mevcut external-content FTS5 trigram DB; read-only, cache2 MiB,
mmap0, temp FILE. Bu belirli SQL/schema/cache düzenidir.

Normal ölçüm: aynı38 senaryo ve aynı seed/sıra ile eski fold/yeni fold/SQLite
üçer taze süreç. Her primary20, her sayfa5 tekrar/süreç; primary pooled60,
sayfa pooled15 örnek. Her query/key bir ölçümsüz warmup; OS cache boşaltılmaz.
Percentile nearest-rank ile hesaplanır. 60 primary örnekte p99 en yüksek,
15 sayfa örneğinde hem p95 hem p99 en yüksek örnektir; production kuyruk
gecikmesi kanıtı değildir. Proses p95'leri ve eşleşmiş farklar JSON özettedir.
Her süreç256 başarılı anahtar ve3 olmayan anahtarda20 tur get yapar.
Kaynak oracle, belge eşitliği ve hash kontrolü zamanlama dışındadır.
Tam API cevabı, count, sayfa hidrasyonu ve codec çözümü zamanlamaya dahildir.
Sıra: control0→FDB0→SQLite0→SQLite1→FDB1→control1→control2→FDB2→SQLite2.

Diagnostic sayaçlar normal ölçümde kapalı. Görünür seed/candidate/accepted
sayıları segment yerellerinde toplanır; tek mutex ile segment sonunda yayınlanır.
Posting probe/cache-read, intersection, payload read/CRC/validation ve fold/scan
ayrı kayıtlıdır. **Nested span'ler örtüşür; toplanıp CPU veya wall-time dağılımı
diye sunulmaz.** Profil timer/lock overhead içerir ve altı sorguluk üç tekrar
geniş normal benchmark yerine geçmez.
Sayaçlar benzersiz ID kümesi değil ziyaret sayısıdır; bu corpus/sorgulardaki
tek posting seed gruplarında ID'ler benzersizdir. Genel range union grupları
bir ID'yi birden fazla ziyaret edebilir.

RAM/working set, soğuk cache, concurrent yazma, yeniden indeksleme ve büyük
corpus bu turda ölçülmez. Disk için mevcut kalıcı dosyaların önce/sonra hash ve
boyutları karşılaştırılır. RAM kazancı veya gerileme yokluğu iddiası yapılmaz.
Get/open küçük farkları normalleştirmeye nedensel olarak bağlanmaz.

## Doğrulama ve yeniden çalıştırma

Bütün Unicode scalar değerleri için yeni `fold_into`, referans `fold` ile eşit;
scratch reuse ve Türkçe/Unicode/chunk sınırı testleri geçti.
`cargo test --all-targets --features experimental-merge-postings,experimental-delta-bitmaps`,
`cargo fmt --check`, default ve all-features `cargo clippy --all-targets -- -D warnings`
geçti. Mevcut kesilme, tek bayt bozulması ve process-kill testleri de bu koşuda
geçti; 0.1 fixture değiştirilmedi. Depolama formatı/kurtarma kodu değiştirilmedi.

Normal runner: `tools/measure-selectivity-reads.ps1`; rapor varsa ezmez.
Control exe, değişiklik öncesi normal fold'dan `output/selectivity-read-control.exe`.
Yeni exe aynı feature'larla diagnostic kapalıdır. Hash'ler
`docs/benchmarks/selectivity-reads-10k-20261012-executables.json` içinde.
Profiler: diagnostic build `measure_content_reads --profile-selectivity`.
Ham profiller `docs/benchmarks/selectivity-profile-{before,after}.jsonl`.
Özetleyici yalnız log okur: `tools/summarize-selectivity-reads.py`.

## Normal ölçüm sonucu

Eski/yeni FDB'nin üç eşleşmiş süreci tamamlandı. Dört hedef sorgu **her üç
çiftte** p95 kazancı sağladı:

| Sorgu | Eski pooled p95 ms | Yeni pooled p95 ms | Azalma | Üç çiftte azalma aralığı |
|---|---:|---:|---:|---:|
| beagle | 71,19 | 55,66 | %21,81 | %10,05–24,53 |
| holmes elementary | 120,38 | 99,67 | %17,20 | %9,62–21,70 |
| transubstantiation | 167,96 | 141,36 | %15,84 | %10,18–25,91 |
| zx | 790,50 | 650,78 | %17,67 | %9,71–24,63 |

38 senaryonun34'ünde pooled p95 düştü. Artan dört senaryo saklanmadı:

| Sorgu | Eski p95 ms | Yeni p95 ms | Artış |
|---|---:|---:|---:|
| quixote, ilk sayfa | 9,2062 | 9,5600 | %3,84 |
| the, ilk sayfa | 397,5394 | 406,1391 | %2,16 |
| quixote whale | 8,9427 | 8,9514 | %0,10 |
| 300 bayt negatif terim | 0,1192 | 0,1786 | %49,83 |

Quixote eşleşmiş çift farkları−%22,03 / +%6,14 / +%11,12; iki çiftte daha
yavaş. The−%9,25 / +%3,72 / −%1,03; iki çiftte hızlı ama pooled tail arttı.
Uzun negatifte mutlak artış0,0594 ms; bu sorgu boş trigram posting'inde
elenir ve payload/fold yoluna girmez. Sebebi bu test tek başına belirlemiyor.
**Genel sıfır-regresyon kapısı geçti denmez.** Hedef düşük seçicilikli sorgular
için kazanç tekrarlı; dar/quixote ve yaygın sorgularda aynı iddia yok.

Profilerde beagle/AND/uzun negatif aday sayıları değişmedi. Kazanç daha güçlü
aday filtresinden gelmiyor; aynı adayların normalleştirilmesini ucuzlatıyor.
Yeni indeks/format eklenmedi. Query/CRC/kimlik kontrolleri kaldırılmadı.


## Tam SQLite karşılaştırması ve dosya kontrolü

Dokuz süreç tamamlandı: **5.760 zamanlı search ve 46.620 get** kontrolü geçti
(warmup hariç). Mevcut bütün `.fdb/.ready/.idx` ve `sqlite.db` dosyalarının
önce/sonra SHA256 ve boyutları aynı. Profil öncesi FDB hash'leri de son hash'lere
eşit. Kaynak/cache formatı, 0.1 fixture ve dış projeler değiştirilmedi.

Bu SQLite düzeninde yeni FDB'nin pooled p95'i38/38 senaryoda düşük; en iyi
SQLite ayarı veya genel motor üstünlüğü iddiası yok. SQLite p95'leri önceki
beş-process raporundan farklıdır; bu oturumdaki üç-process karşılaştırmasıdır.

| Sorgu | Offset / limit | Sonuç | Eski p95 ms | Yeni p95 ms | SQLite p95 ms | Değişim % |
|---|---:|---:|---:|---:|---:|---:|
| quixote | 0 / 50 | 56 | 9.2062 | 9.5600 | 19.6543 | +3.84 |
| zeuglodon | 0 / 50 | 32 | 5.6000 | 5.3234 | 11.7906 | -4.94 |
| beagle | 0 / 50 | 46 | 71.1873 | 55.6594 | 189.3297 | -21.81 |
| mesopotamia | 0 / 50 | 31 | 6.4512 | 5.8503 | 14.9626 | -9.31 |
| transubstantiation | 0 / 50 | 0 | 167.9631 | 141.3584 | 681.4418 | -15.84 |
| harpoon | 0 / 50 | 1006 | 206.3920 | 171.9951 | 662.3965 | -16.67 |
| ishmael | 0 / 50 | 260 | 23.8462 | 21.8664 | 75.4509 | -8.30 |
| invertebrata | 0 / 50 | 43 | 32.1453 | 25.8554 | 128.6492 | -19.57 |
| darcy | 0 / 50 | 1220 | 162.5334 | 145.6666 | 555.5427 | -10.38 |
| elizabeth | 0 / 50 | 2280 | 118.4689 | 118.3114 | 479.8473 | -0.13 |
| watson | 0 / 50 | 1707 | 155.3213 | 135.2803 | 444.0353 | -12.90 |
| holmes | 0 / 50 | 1222 | 78.2402 | 73.0223 | 244.2190 | -6.67 |
| frankenstein | 0 / 50 | 607 | 48.3794 | 41.0617 | 153.1651 | -15.13 |
| galapagos | 0 / 50 | 179 | 28.5842 | 25.2835 | 85.3252 | -11.55 |
| fossil | 0 / 50 | 595 | 87.8219 | 75.5860 | 332.4912 | -13.93 |
| geologist | 0 / 50 | 306 | 52.0229 | 44.1846 | 173.8453 | -15.07 |
| the | 0 / 50 | 10000 | 397.5394 | 406.1391 | 1467.9759 | +2.16 |
| man | 0 / 50 | 9932 | 434.3655 | 417.3825 | 1483.9697 | -3.91 |
| he | 0 / 50 | 10000 | 444.4095 | 409.0417 | 1399.6728 | -7.96 |
| zzxqnever | 0 / 50 | 0 | 0.1475 | 0.1459 | 0.6667 | -1.08 |
| zx | 0 / 50 | 0 | 790.5013 | 650.7844 | 1994.6525 | -17.67 |
| the zx | 0 / 50 | 0 | 777.5041 | 661.8355 | 2115.2919 | -14.88 |
| quixote whale | 0 / 50 | 44 | 8.9427 | 8.9514 | 18.7454 | +0.10 |
| holmes elementary | 0 / 50 | 65 | 120.3816 | 99.6731 | 428.2528 | -17.20 |
| fossil geologist | 0 / 50 | 293 | 33.6804 | 29.1070 | 137.1112 | -13.58 |
| whale harpoon | 0 / 50 | 995 | 191.8652 | 165.5197 | 557.8352 | -13.73 |
| QUİXOTE | 0 / 50 | 56 | 9.3470 | 8.5286 | 19.7181 | -8.76 |
| İSHMAEL | 0 / 50 | 260 | 23.0714 | 22.0944 | 72.8015 | -4.23 |
| 300 bayt negatif terim | 0 / 50 | 0 | 0.1192 | 0.1786 | 2.6836 | +49.83 |
| presumption | 0 / 50 | 883 | 272.5798 | 221.3498 | 769.6629 | -18.79 |
| the | 0 / 1 | 10000 | 438.4598 | 404.1168 | 1457.0463 | -7.83 |
| the | 0 / 200 | 10000 | 418.3722 | 416.0852 | 1445.5519 | -0.55 |
| the | 5000 / 50 | 10000 | 432.5825 | 403.1642 | 1520.6029 | -6.80 |
| the | 9950 / 50 | 10000 | 414.4205 | 405.4362 | 1451.5503 | -2.17 |
| ishmael | 130 / 1 | 260 | 20.1054 | 19.6160 | 65.1451 | -2.43 |
| ishmael | 60 / 200 | 260 | 37.6104 | 31.9483 | 87.0337 | -15.05 |
| quixote | 28 / 10 | 56 | 7.0264 | 6.8898 | 17.6611 | -1.94 |
| quixote | 57 / 50 | 56 | 6.0628 | 5.7055 | 15.7135 | -5.89 |

## Get ve açılış

Her değer pooled p95 ms; RAM ölçülmedi.

| Get boyutu | Eski FDB | Yeni FDB | SQLite |
|---|---:|---:|---:|
| 2–4 KiB | 0.0391 | 0.0346 | 0.0977 |
| 4–16 KiB | 0.0514 | 0.0495 | 0.1289 |
| 16–32 KiB | 0.0802 | 0.0741 | 0.1697 |
| 32–50 KiB | 0.1056 | 0.1001 | 0.2188 |
| Olmayan kayıt | 0.0013 | 0.0014 | 0.0279 |

Açılış medyanı (ms): control 983.6049, fdb 982.6731, sqlite 8.4951.
FDB tam source doğrular; SQLite bağlantı/pragma açılışı tam integrity taraması
 değildir. Get ve open değişimi normalleştirme kazancı olarak yorumlanmaz;
 bunlar çağrı başına fold yoluna girmez. Yazma performansı bu turda ölçülmedi.

Karar: küçük normalleştirme değişikliği uygulanmıştır; hedef sorgulardaki
kazanç ölçülmüş ve üç çiftte tekrarlıdır. Bütün sorguların hızlandığı veya
RAM/production/genel sıfır-regresyon kapısının geçtiği iddia edilmez. Dar sorgu
regresyonları ve yaygın sorguda CRC/IO maliyeti sonraki inceleme noktalarıdır.
