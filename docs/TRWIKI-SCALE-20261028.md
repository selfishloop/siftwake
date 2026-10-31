# Türkçe corpus 50k/100k — 28 Ekim 2026

**KALDI, bağımlı aşamalar durdu.** 50k capped N=50 arama kapısı kaldı.
100k için izin verilen bağımsız ingest de tek kayıt sınırında kaldı:
66498 numaralı metin 444345B, streamed kodlanmış entry limiti 262144B.
FDB'de yalnız ilk 66000 belge/sequence 132 commit edildi; reddedilen
66000..66499 grubunun tamamı görünmez. Corpus kırpılmadı, owned fallback
veya limit/format değişimi yapılmadı. SQLite bağımsız 100k koşuları tamamlandı.
100k FDB açılış/RAM/arama sonucu yerine eksik koşu açıkça gösterilir.

Kullanıcının verdiği 100k farklı Türkçe Wikipedia gövdesi kullanıldı. Kaynak
`D:/frunner-corpus/trwiki-100k/corpus.payload-bench` yalnız okundu; `private/`
okunmadı. Gövdeler kırpılmadı, marker eklenmedi, title/fields/links korunur.
Anahtarların orijinal numeric şablonuyla aynı olduğu audit'te doğrulanmıştır.
Yeni corpus/DB kopyası/format/ön filtre yok. DB'ler sadece C: sabit FDB/SQLite
slotlarında, her ingest kolunda ownership kontrolüyle yeniden kuruldu.

## Kapsam ve yöntem

50k'da iki ingest sırası FDB/SQLite ve SQLite/FDB. 100k FDB ilk kolda
reddedildi; iki SQLite tekrarı bağımsız referanstır, tamamlanmış dengeli
100k A/B sayılmaz. API akışlı, 500 işlem/
commit; owned/Vec ingest ayrı denenmedi. FDB ingest süresine flush/merge ve
son idle cleanup+integrity/GC dahildir; cleanup ayrı ham kalemdir. RAM toplam
process **private commit** ve **working set** olarak ayrı verilir. Ingest
lifetime peak private, corpus/model descriptor harness'ını da içerir; yalnız
motor heap'i veya cihazın toplam RAM'i değildir. Bellek örnekleyicisi 10 ms;
OS peak private/WS ayrıca yakalanır. Idle yeni proseste 2 sn sonra, oracle yüklemeden.

Sıcak arama için full get/version oracle+her seçenek bir warmup; 10+10=20
normal release örneği/sorgu/seçenek, döndürülmüş seçenek/sorgu ve dengeli motor
sırası. N=50/offset=0, 41 kullanıcı sorgusu. Exact serial normal Database::search;
exact4 run-parallel deneyidir, yalnız thread sayısı dışında algoritma farkları
da vardır. Capped serial/4/melez 200 opt-in; join/CRC reuse kapalı. CPU Windows
GetProcessTimes user+kernel, bütün query worker'ları ve process örnekleyicisi
dahil; tek query 0/15,625ms kuantize olabilir. CPU blokları ayrıca raporlanır.

SQLite FTS5 trigram/detail=none/columnsize=0, WAL/FULL, cache 2 MiB, mmap 0,
tempFILE. Aynı payload/version/key ve normalize title/text substring-AND.
SQLite fields/links payload içinde; FDB bunlara kendi scalar/numeric/link
indekslerini de kurar. Capped SQL COUNT'ı çıkarır ama id sıralaması için
adayları tüketebilir. Bu uygulama farkı gizlenmez; genel motor üstünlüğü iddiası yok.

Strict-open normal Database::open'dır, kaynak/indeks CRC ve metadata kontrolü
atlanmaz. İlk-sonuç yeni proseste melez N50; her motorun **kendi bütün DB
dosyaları** zamanlama öncesi stream-read/hash ile ısıtılır. OS eviction yok;
gerçek-soğuk kapısı değildir. İlk-result oracle ve katalog/model yükleme
zamanlama dışındadır. SQLite normal open, FDB'nin source-wide strict kontratıyla
aynı değildir. Her familya 2 örnek: p95=max; üretim tail garantisi sayılmaz.

Kabul: kullanıcı yanıtıyla **capped N50 ana arama kapısı**, exact sonuçlar
bilgi amaçlıdır. Ortak≤30ms, orta/nadir/AND/negatif≤150ms@50k/≤300ms@100k;
ve/if/id/zx geniş2-karakter taramaları kapı dışı. Temiz idle≤15MiB,
streamed ingest peak private≤60MiB; strict-open≤1000/2000ms. Melez budget 200,
önceden kabul edilen %10 negatif/%5 bol-pozitif CPU sınırları da ayrı gösterilir.

## Ingest, temiz disk ve RAM

Hücreler iki ingest örneği; süre tüm bakım/GC dahil saniye. Disk decimal MB;
bellek MiB. İlk işaret phase ingest sonu, oracle sonrası tepe değildir.

| N | Motor | Ingest sn | Disk MB | Peak private MiB | Peak WS MiB | Temiz idle private max MiB | Idle WS max MiB |
|---|---|---|---|---|---|---:|---:|
| 50000 | fdb | 39.132 / 38.441 | 300.837 / 300.837 | 19.945 / 19.602 | 24.695 / 23.934 | 2.223 | 6.027 |
| 50000 | sqlite | 36.185 / 36.416 | 421.736 / 421.736 | 17.945 / 17.922 | 20.395 / 19.883 | 3.016 | 7.238 |
| 100000 | sqlite | 95.199 / 95.122 | 834.961 / 834.961 | 19.008 / 19.133 | 23.438 / 23.684 | 3.281 | 7.492 |
| 100000 | fdb | KALDI: 66000 commit sonrası tek entry limiti | — | — | — | — | — |

| Arama process/harness | Peak private MiB | Peak WS MiB |
|---|---|---|
| 50000 / fdb | 38.418 / 39.414 | 42.367 / 43.512 |
| 50000 / sqlite | 35.113 / 35.578 | 39.531 / 39.938 |
| 100000 / sqlite | 43.570 / 44.059 | 48.047 / 48.059 |

Arama process peak'i katalog/model, full GET oracle ve bütün 41 case'in beklenen
owned sayfalarını içerir. Motorun tek sorgu belleği diye sunulmaz; ingest/idle
kapısıyla karıştırılmaz. Tam 100k FDB arama süreci çalışmadı.

Disk önceki kitap corpus'u ile codec kazanımı diye karşılaştırılmaz: yeni
50k/100k ham text 157.148.362/312.917.193B, uzunluk dağılımı çok farklıdır.
WAL/manifest/source/index biçimleri aynı; eski working DB kopyaları tutulmadı.

## Açılış ve ilk cevap

Strict süre tüm veri doğrulamasını içerir; aşağıdaki idle-open örnekleri ayrıca
gates JSON'undaki ilk-result session örnekleriyle birlikte değerlendirilir.

| N | Strict-open örnekleri ms | Pooled p95 ms | Sınır | Durum |
|---|---|---:|---:|---|
| 50000 | 261.61, 254.26, 255.23, 266.76, 275.28, 257.53, 281.83, 258.78, 261.78, 266.98, 259.18, 273.23, 260.31, 271.14, 254.43, 263.01 | 281.83 | 1000 | GEÇTİ |
| 100000 | Tam FDB DB kurulamadı | — | 2000 | ÖLÇÜLEMEDİ |

| N | Sorgu | Motor | Session p95 ms | İlk-sonuç p95 ms | n |
|---|---|---|---:|---:|---:|
| 50000 | akarsu | fdb | 273.230 | 301.558 | 2 |
| 50000 | furlong | fdb | 275.278 | 289.770 | 2 |
| 50000 | ilçesine köyleri | fdb | 271.139 | 286.709 | 2 |
| 50000 | olduğu | fdb | 259.177 | 265.283 | 2 |
| 50000 | qzxkvwnever | fdb | 263.014 | 263.675 | 2 |
| 50000 | zx | fdb | 281.826 | 365.199 | 2 |
| 50000 | akarsu | sqlite | 1.702 | 108.286 | 2 |
| 50000 | furlong | sqlite | 1.757 | 11.809 | 2 |
| 50000 | ilçesine köyleri | sqlite | 1.617 | 71.273 | 2 |
| 50000 | olduğu | sqlite | 1.589 | 260.790 | 2 |
| 50000 | qzxkvwnever | sqlite | 1.658 | 6.144 | 2 |
| 50000 | zx | sqlite | 1.905 | 447.381 | 2 |

## 50000: bütün 41 sorgu — p95/ms

| Sorgu | Gerçek total | Exact serial | Exact4 | Cap serial | Cap4 | Melez200 | SQLite exact | SQLite capped |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| olduğu | 14299 | 1343.662 | 600.421 | 5.604 | 16.331 | 5.130 | 526.356 | 279.551 |
| büyük | 15316 | 977.290 | 413.978 | 4.994 | 11.002 | 3.395 | 490.816 | 253.982 |
| ayrıca | 14965 | 1397.072 | 634.499 | 6.011 | 18.170 | 5.872 | 621.731 | 324.981 |
| arasında | 15854 | 3293.162 | 1503.397 | 12.778 | 24.961 | 13.591 | 841.905 | 473.892 |
| birlikte | 10219 | 1982.813 | 881.202 | 10.652 | 26.420 | 9.697 | 523.786 | 292.098 |
| sanat | 6171 | 820.437 | 367.099 | 6.440 | 13.440 | 6.414 | 542.838 | 285.657 |
| görenler | 3379 | 1873.037 | 890.551 | 24.972 | 64.655 | 25.182 | 550.797 | 293.331 |
| millî | 4603 | 255.602 | 119.259 | 3.827 | 10.636 | 3.381 | 168.326 | 85.057 |
| ikinci | 9031 | 1203.251 | 547.449 | 10.095 | 20.199 | 8.657 | 613.053 | 315.446 |
| toplam | 5685 | 908.320 | 407.817 | 6.636 | 18.517 | 8.963 | 476.199 | 221.847 |
| isimli | 2763 | 1008.130 | 454.601 | 19.531 | 39.681 | 21.286 | 661.765 | 331.783 |
| öldürür | 402 | 531.944 | 218.343 | 69.354 | 103.515 | 67.778 | 264.688 | 144.824 |
| ölümü | 7952 | 792.319 | 358.944 | 6.434 | 13.210 | 6.123 | 458.059 | 237.622 |
| akarsu | 515 | 250.443 | 107.937 | 28.802 | 47.057 | 36.626 | 223.816 | 105.121 |
| demokratik | 904 | 222.982 | 111.694 | 12.553 | 31.954 | 15.867 | 98.551 | 51.579 |
| çalışmalarıyla | 249 | 2179.428 | 1002.140 | 345.108 | 680.960 | 585.362 | 430.945 | 201.651 |
| üreticisi | 424 | 817.419 | 370.338 | 97.224 | 155.407 | 122.680 | 334.103 | 169.925 |
| joseph | 1027 | 14.971 | 10.370 | 1.636 | 3.384 | 1.257 | 65.176 | 31.985 |
| leyla | 287 | 403.708 | 215.909 | 53.596 | 109.625 | 85.992 | 422.450 | 210.446 |
| protest | 912 | 215.806 | 95.948 | 13.670 | 21.848 | 14.547 | 146.812 | 72.357 |
| furlong | 4 | 5.903 | 3.498 | 5.440 | 3.543 | 5.757 | 12.747 | 5.746 |
| korkaklığa | 2 | 36.909 | 17.293 | 34.262 | 16.562 | 33.178 | 46.831 | 23.666 |
| ormanların | 194 | 1867.510 | 861.252 | 524.428 | 667.095 | 689.569 | 526.558 | 259.127 |
| yanıklarda | 4 | 1261.086 | 555.336 | 1235.903 | 546.434 | 582.049 | 404.364 | 199.918 |
| külliyatı | 27 | 847.575 | 401.092 | 907.112 | 394.781 | 458.676 | 364.206 | 161.627 |
| timmy | 22 | 24.917 | 10.261 | 22.787 | 10.459 | 19.930 | 27.951 | 14.553 |
| yüklük | 508 | 443.197 | 207.047 | 44.204 | 72.946 | 53.175 | 264.887 | 128.588 |
| ilçesine köyleri | 1313 | 431.903 | 221.308 | 16.841 | 38.321 | 14.433 | 151.312 | 70.489 |
| geldiği buldu | 365 | 888.572 | 417.431 | 124.470 | 205.917 | 154.395 | 347.416 | 173.383 |
| chicago studio | 33 | 7.028 | 3.970 | 7.252 | 3.874 | 8.188 | 16.698 | 10.209 |
| kısım dişli | 110 | 1148.817 | 530.173 | 526.794 | 535.346 | 561.599 | 520.282 | 231.823 |
| güzeli zafer | 16 | 43.590 | 22.951 | 44.540 | 21.639 | 42.154 | 49.758 | 25.119 |
| İnternetten hazinem | 1 | 229.808 | 118.623 | 226.427 | 118.625 | 189.174 | 192.735 | 92.841 |
| oyuncudur atlanta | 10 | 199.281 | 94.075 | 195.576 | 97.096 | 175.626 | 101.024 | 50.097 |
| kendisine çıkacağını | 17 | 1035.890 | 452.167 | 1006.805 | 460.566 | 523.053 | 309.376 | 158.422 |
| ve | 47523 | 203.113 | 99.650 | 0.873 | 1.677 | 0.660 | 727.474 | 375.905 |
| if | 22579 | 482.117 | 198.652 | 1.716 | 2.679 | 1.705 | 1036.540 | 521.433 |
| id | 38276 | 389.587 | 167.922 | 1.269 | 2.229 | 0.963 | 876.590 | 442.885 |
| zx | 28 | 223.438 | 105.812 | 248.789 | 110.660 | 111.673 | 921.659 | 488.307 |
| qzxkvwnever | 0 | 0.019 | 0.678 | 0.039 | 0.368 | 0.612 | 0.202 | 0.384 |
| transubstantiationz | 0 | 1.965 | 0.796 | 0.621 | 0.868 | 0.579 | 5.547 | 2.550 |

### 50000: CPU ortalama/toplam ms

Hücre ortalama /20 sorgu toplam process CPU. p50/max ve ham örnekler JSON analizinde.

| Sorgu | Exact serial | Exact4 | Cap serial | Cap4 | Melez | SQLite exact | SQLite capped |
|---|---:|---:|---:|---:|---:|---:|---:|
| olduğu | 1289.06/25781.25 | 1628.91/32578.12 | 7.81/156.25 | 39.06/781.25 | 3.12/62.50 | 497.66/9953.12 | 255.47/5109.38 |
| büyük | 865.62/17312.50 | 1050.00/21000.00 | 2.34/46.88 | 30.47/609.38 | 0.78/15.62 | 460.94/9218.75 | 232.03/4640.62 |
| ayrıca | 1335.16/26703.12 | 1646.88/32937.50 | 5.47/109.38 | 42.19/843.75 | 3.91/78.12 | 585.94/11718.75 | 303.91/6078.12 |
| arasında | 3164.84/63296.88 | 4041.41/80828.12 | 8.59/171.88 | 67.19/1343.75 | 11.72/234.38 | 799.22/15984.38 | 406.25/8125.00 |
| birlikte | 1863.28/37265.62 | 2353.91/47078.12 | 10.94/218.75 | 64.06/1281.25 | 7.81/156.25 | 489.06/9781.25 | 250.00/5000.00 |
| sanat | 760.16/15203.12 | 933.59/18671.88 | 3.12/62.50 | 36.72/734.38 | 6.25/125.00 | 477.34/9546.88 | 242.97/4859.38 |
| görenler | 1811.72/36234.38 | 2278.91/45578.12 | 25.00/500.00 | 160.94/3218.75 | 23.44/468.75 | 509.38/10187.50 | 256.25/5125.00 |
| millî | 239.84/4796.88 | 294.53/5890.62 | 3.91/78.12 | 23.44/468.75 | 3.12/62.50 | 135.94/2718.75 | 65.62/1312.50 |
| ikinci | 1161.72/23234.38 | 1464.06/29281.25 | 4.69/93.75 | 49.22/984.38 | 5.47/109.38 | 575.78/11515.62 | 289.84/5796.88 |
| toplam | 851.56/17031.25 | 1059.38/21187.50 | 4.69/93.75 | 47.66/953.12 | 11.72/234.38 | 420.31/8406.25 | 211.72/4234.38 |
| isimli | 954.69/19093.75 | 1185.94/23718.75 | 18.75/375.00 | 103.12/2062.50 | 18.75/375.00 | 628.12/12562.50 | 317.97/6359.38 |
| öldürür | 446.09/8921.88 | 546.88/10937.50 | 60.94/1218.75 | 267.97/5359.38 | 116.41/2328.12 | 252.34/5046.88 | 129.69/2593.75 |
| ölümü | 753.12/15062.50 | 924.22/18484.38 | 7.03/140.62 | 35.94/718.75 | 5.47/109.38 | 436.72/8734.38 | 228.91/4578.12 |
| akarsu | 221.09/4421.88 | 278.91/5578.12 | 26.56/531.25 | 116.41/2328.12 | 35.16/703.12 | 200.00/4000.00 | 100.78/2015.62 |
| demokratik | 208.59/4171.88 | 275.78/5515.62 | 10.94/218.75 | 82.03/1640.62 | 13.28/265.62 | 89.06/1781.25 | 47.66/953.12 |
| çalışmalarıyla | 2076.56/41531.25 | 2607.03/52140.62 | 339.06/6781.25 | 1650.78/33015.62 | 1326.56/26531.25 | 381.25/7625.00 | 193.75/3875.00 |
| üreticisi | 769.53/15390.62 | 949.22/18984.38 | 93.75/1875.00 | 377.34/7546.88 | 216.41/4328.12 | 304.69/6093.75 | 153.91/3078.12 |
| joseph | 12.50/250.00 | 22.66/453.12 | 0.00/0.00 | 10.94/218.75 | 2.34/46.88 | 54.69/1093.75 | 30.47/609.38 |
| leyla | 354.69/7093.75 | 402.34/8046.88 | 49.22/984.38 | 218.75/4375.00 | 142.19/2843.75 | 398.44/7968.75 | 192.97/3859.38 |
| protest | 191.41/3828.12 | 243.75/4875.00 | 10.94/218.75 | 58.59/1171.88 | 14.06/281.25 | 137.50/2750.00 | 70.31/1406.25 |
| furlong | 5.47/109.38 | 2.34/46.88 | 4.69/93.75 | 7.81/156.25 | 5.47/109.38 | 13.28/265.62 | 2.34/46.88 |
| korkaklığa | 33.59/671.88 | 42.97/859.38 | 31.25/625.00 | 39.84/796.88 | 31.25/625.00 | 42.19/843.75 | 23.44/468.75 |
| ormanların | 1805.47/36109.38 | 2274.22/45484.38 | 479.69/9593.75 | 1816.41/36328.12 | 1766.41/35328.12 | 485.94/9718.75 | 237.50/4750.00 |
| yanıklarda | 1154.69/23093.75 | 1448.44/28968.75 | 1157.03/23140.62 | 1433.59/28671.88 | 1410.16/28203.12 | 368.75/7375.00 | 189.84/3796.88 |
| külliyatı | 810.94/16218.75 | 1014.06/20281.25 | 816.41/16328.12 | 1000.00/20000.00 | 992.19/19843.75 | 317.19/6343.75 | 154.69/3093.75 |
| timmy | 20.31/406.25 | 19.53/390.62 | 19.53/390.62 | 29.69/593.75 | 16.41/328.12 | 25.00/500.00 | 14.06/281.25 |
| yüklük | 420.31/8406.25 | 519.53/10390.62 | 42.97/859.38 | 184.38/3687.50 | 87.50/1750.00 | 243.75/4875.00 | 120.31/2406.25 |
| ilçesine köyleri | 416.41/8328.12 | 529.69/10593.75 | 14.84/296.88 | 89.06/1781.25 | 14.84/296.88 | 136.72/2734.38 | 67.97/1359.38 |
| geldiği buldu | 864.06/17281.25 | 1083.59/21671.88 | 116.41/2328.12 | 485.94/9718.75 | 289.06/5781.25 | 310.16/6203.12 | 158.59/3171.88 |
| chicago studio | 6.25/125.00 | 7.03/140.62 | 5.47/109.38 | 9.38/187.50 | 6.25/125.00 | 12.50/250.00 | 7.81/156.25 |
| kısım dişli | 1117.97/22359.38 | 1395.31/27906.25 | 492.97/9859.38 | 1389.84/27796.88 | 1364.84/27296.88 | 442.97/8859.38 | 217.97/4359.38 |
| güzeli zafer | 42.19/843.75 | 55.47/1109.38 | 39.06/781.25 | 54.69/1093.75 | 40.62/812.50 | 43.75/875.00 | 25.78/515.62 |
| İnternetten hazinem | 214.84/4296.88 | 269.53/5390.62 | 221.09/4421.88 | 261.72/5234.38 | 246.88/4937.50 | 178.12/3562.50 | 92.97/1859.38 |
| oyuncudur atlanta | 191.41/3828.12 | 230.47/4609.38 | 188.28/3765.62 | 238.28/4765.62 | 210.94/4218.75 | 95.31/1906.25 | 47.66/953.12 |
| kendisine çıkacağını | 953.91/19078.12 | 1191.41/23828.12 | 957.81/19156.25 | 1206.25/24125.00 | 1162.50/23250.00 | 290.62/5812.50 | 148.44/2968.75 |
| ve | 193.75/3875.00 | 246.88/4937.50 | 0.78/15.62 | 3.12/62.50 | 0.78/15.62 | 692.19/13843.75 | 353.12/7062.50 |
| if | 428.91/8578.12 | 497.66/9953.12 | 1.56/31.25 | 10.16/203.12 | 0.00/0.00 | 981.25/19625.00 | 500.78/10015.62 |
| id | 359.38/7187.50 | 424.22/8484.38 | 1.56/31.25 | 2.34/46.88 | 0.00/0.00 | 822.66/16453.12 | 407.81/8156.25 |
| zx | 210.16/4203.12 | 274.22/5484.38 | 241.41/4828.12 | 285.94/5718.75 | 278.91/5578.12 | 884.38/17687.50 | 447.66/8953.12 |
| qzxkvwnever | 0.00/0.00 | 4.69/93.75 | 0.00/0.00 | 0.78/15.62 | 0.00/0.00 | 0.00/0.00 | 0.00/0.00 |
| transubstantiationz | 1.56/31.25 | 0.00/0.00 | 0.00/0.00 | 0.00/0.00 | 0.00/0.00 | 7.03/140.62 | 1.56/31.25 |

## 100000: bütün 41 sorgu — p95/ms

**FDB normal arama matrisi önceki kapı nedeniyle DURDU.** SQLite bağımsız referansı tamamlandı; boş hücre0ms değildir.

| Sorgu | Gerçek total | Exact serial | Exact4 | Cap serial | Cap4 | Melez200 | SQLite exact | SQLite capped |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| olduğu | 28407 | — | — | — | — | — | 1282.116 | 631.934 |
| büyük | 30642 | — | — | — | — | — | 1153.482 | 603.382 |
| ayrıca | 29883 | — | — | — | — | — | 1488.031 | 775.850 |
| arasında | 31555 | — | — | — | — | — | 2050.469 | 1002.430 |
| birlikte | 20347 | — | — | — | — | — | 1244.742 | 597.560 |
| sanat | 12436 | — | — | — | — | — | 1185.550 | 613.843 |
| görenler | 6572 | — | — | — | — | — | 1271.976 | 658.926 |
| millî | 9256 | — | — | — | — | — | 316.611 | 153.763 |
| ikinci | 17915 | — | — | — | — | — | 1410.857 | 710.433 |
| toplam | 11405 | — | — | — | — | — | 1031.172 | 528.122 |
| isimli | 5458 | — | — | — | — | — | 1562.681 | 813.250 |
| öldürür | 812 | — | — | — | — | — | 648.448 | 332.126 |
| ölümü | 15800 | — | — | — | — | — | 1122.674 | 579.988 |
| akarsu | 960 | — | — | — | — | — | 530.641 | 266.337 |
| demokratik | 1806 | — | — | — | — | — | 214.762 | 117.242 |
| çalışmalarıyla | 505 | — | — | — | — | — | 979.024 | 481.717 |
| üreticisi | 815 | — | — | — | — | — | 757.673 | 381.236 |
| joseph | 2051 | — | — | — | — | — | 149.696 | 70.107 |
| leyla | 591 | — | — | — | — | — | 977.858 | 507.428 |
| protest | 1822 | — | — | — | — | — | 341.409 | 177.063 |
| furlong | 9 | — | — | — | — | — | 27.234 | 15.811 |
| korkaklığa | 3 | — | — | — | — | — | 107.046 | 58.697 |
| ormanların | 404 | — | — | — | — | — | 1261.428 | 596.896 |
| yanıklarda | 8 | — | — | — | — | — | 928.603 | 459.035 |
| külliyatı | 80 | — | — | — | — | — | 749.343 | 395.081 |
| timmy | 42 | — | — | — | — | — | 66.649 | 34.286 |
| yüklük | 983 | — | — | — | — | — | 655.887 | 321.679 |
| ilçesine köyleri | 2590 | — | — | — | — | — | 322.479 | 179.576 |
| geldiği buldu | 726 | — | — | — | — | — | 781.385 | 387.853 |
| chicago studio | 55 | — | — | — | — | — | 38.175 | 21.649 |
| kısım dişli | 212 | — | — | — | — | — | 1104.477 | 552.884 |
| güzeli zafer | 37 | — | — | — | — | — | 119.277 | 58.399 |
| İnternetten hazinem | 1 | — | — | — | — | — | 455.410 | 232.120 |
| oyuncudur atlanta | 18 | — | — | — | — | — | 239.468 | 117.549 |
| kendisine çıkacağını | 34 | — | — | — | — | — | 753.255 | 358.124 |
| ve | 94949 | — | — | — | — | — | 1796.863 | 882.886 |
| if | 45182 | — | — | — | — | — | 2495.590 | 1246.230 |
| id | 76428 | — | — | — | — | — | 2112.028 | 1019.018 |
| zx | 47 | — | — | — | — | — | 2212.627 | 1095.794 |
| qzxkvwnever | 0 | — | — | — | — | — | 0.181 | 0.419 |
| transubstantiationz | 0 | — | — | — | — | — | 11.267 | 5.275 |

### 100000: CPU ortalama/toplam ms

Hücre ortalama /20 sorgu toplam process CPU. p50/max ve ham örnekler JSON analizinde.

| Sorgu | Exact serial | Exact4 | Cap serial | Cap4 | Melez | SQLite exact | SQLite capped |
|---|---:|---:|---:|---:|---:|---:|---:|
| olduğu | — | — | — | — | — | 1194.53/23890.62 | 596.88/11937.50 |
| büyük | — | — | — | — | — | 1096.09/21921.88 | 557.81/11156.25 |
| ayrıca | — | — | — | — | — | 1419.53/28390.62 | 711.72/14234.38 |
| arasında | — | — | — | — | — | 1922.66/38453.12 | 956.25/19125.00 |
| birlikte | — | — | — | — | — | 1141.41/22828.12 | 570.31/11406.25 |
| sanat | — | — | — | — | — | 1122.66/22453.12 | 566.41/11328.12 |
| görenler | — | — | — | — | — | 1206.25/24125.00 | 600.00/12000.00 |
| millî | — | — | — | — | — | 292.19/5843.75 | 153.91/3078.12 |
| ikinci | — | — | — | — | — | 1359.38/27187.50 | 672.66/13453.12 |
| toplam | — | — | — | — | — | 979.69/19593.75 | 495.31/9906.25 |
| isimli | — | — | — | — | — | 1494.53/29890.62 | 751.56/15031.25 |
| öldürür | — | — | — | — | — | 615.62/12312.50 | 301.56/6031.25 |
| ölümü | — | — | — | — | — | 1053.12/21062.50 | 535.94/10718.75 |
| akarsu | — | — | — | — | — | 481.25/9625.00 | 245.31/4906.25 |
| demokratik | — | — | — | — | — | 200.00/4000.00 | 104.69/2093.75 |
| çalışmalarıyla | — | — | — | — | — | 906.25/18125.00 | 452.34/9046.88 |
| üreticisi | — | — | — | — | — | 711.72/14234.38 | 359.38/7187.50 |
| joseph | — | — | — | — | — | 131.25/2625.00 | 66.41/1328.12 |
| leyla | — | — | — | — | — | 928.12/18562.50 | 467.97/9359.38 |
| protest | — | — | — | — | — | 324.22/6484.38 | 164.06/3281.25 |
| furlong | — | — | — | — | — | 23.44/468.75 | 11.72/234.38 |
| korkaklığa | — | — | — | — | — | 97.66/1953.12 | 52.34/1046.88 |
| ormanların | — | — | — | — | — | 1157.81/23156.25 | 568.75/11375.00 |
| yanıklarda | — | — | — | — | — | 869.53/17390.62 | 435.16/8703.12 |
| külliyatı | — | — | — | — | — | 727.34/14546.88 | 368.75/7375.00 |
| timmy | — | — | — | — | — | 61.72/1234.38 | 31.25/625.00 |
| yüklük | — | — | — | — | — | 578.12/11562.50 | 289.06/5781.25 |
| ilçesine köyleri | — | — | — | — | — | 300.78/6015.62 | 156.25/3125.00 |
| geldiği buldu | — | — | — | — | — | 742.19/14843.75 | 366.41/7328.12 |
| chicago studio | — | — | — | — | — | 36.72/734.38 | 19.53/390.62 |
| kısım dişli | — | — | — | — | — | 1029.69/20593.75 | 512.50/10250.00 |
| güzeli zafer | — | — | — | — | — | 103.91/2078.12 | 57.81/1156.25 |
| İnternetten hazinem | — | — | — | — | — | 435.94/8718.75 | 216.41/4328.12 |
| oyuncudur atlanta | — | — | — | — | — | 225.00/4500.00 | 111.72/2234.38 |
| kendisine çıkacağını | — | — | — | — | — | 698.44/13968.75 | 347.66/6953.12 |
| ve | — | — | — | — | — | 1651.56/33031.25 | 830.47/16609.38 |
| if | — | — | — | — | — | 2360.94/47218.75 | 1189.06/23781.25 |
| id | — | — | — | — | — | 1957.81/39156.25 | 984.38/19687.50 |
| zx | — | — | — | — | — | 2123.44/42468.75 | 1056.25/21125.00 |
| qzxkvwnever | — | — | — | — | — | 0.78/15.62 | 0.00/0.00 |
| transubstantiationz | — | — | — | — | — | 10.94/218.75 | 3.91/78.12 |

## Kısa sorgular: ilk50 ve tam sayım farklı iş

ve/if/id/zx satırlarında exact bütün gerçek eşleşmeleri sayar; capped50 hit'te
AtLeast(50) döner. İlk50 dolmazsa capped de tüketir: zx50k'da28,100k'da47
eşleşme. Bu yüzden kısa negatif/seyrek taramada capped otomatik erken durma
garantisi vermez. Runtime süre ve CPU üstte ayrı;3-gram yokken corpusN üst-sınır
adaydır, actual payload read sayısı diye sunulmaz. İlk50 sonucu bulmak en yeni50
olması için caller ters-mtime key sırası kurar; burada orijinal numeric keys var.

## CPU blokları ve kapı tablosu

| N | Sorgu | Yol | Çağrı | Ortalama CPU ms | Toplam CPU ms |
|---|---|---|---:|---:|---:|
| 50000 | ayrıca | cap | 2000 | 4.687500 | 9375.000 |
| 50000 | ayrıca | cap-four | 2000 | 39.609375 | 79218.750 |
| 50000 | ayrıca | cap-hybrid | 2000 | 4.671875 | 9343.750 |
| 50000 | büyük | cap | 2000 | 3.289062 | 6578.125 |
| 50000 | büyük | cap-four | 2000 | 24.921875 | 49843.750 |
| 50000 | büyük | cap-hybrid | 2000 | 3.265625 | 6531.250 |
| 50000 | olduğu | cap | 2000 | 4.968750 | 9937.500 |
| 50000 | olduğu | cap-four | 2000 | 37.109375 | 74218.750 |
| 50000 | olduğu | cap-hybrid | 2000 | 5.140625 | 10281.250 |

### 50000 kapısı

| Kontrol | Kanıt | Durum |
|---|---|---|
| Ingest/disk/RAM/SQLite | iki dengeli kol, her koşuda oracle | TAMAMLANDI |
| Strict-open / temiz idle / ingest peak | üstteki tam örnekler; 1000ms / 15MiB / 60MiB | GEÇTİ |
| Capped arama ana kapısı | ölçülen 123 sorgu/yol | KALDI |
| capped_query | {"kind":"capped_query","case_id":6,"text":"görenler","variant":"cap-four","p95_ms":64.65469999999999,"limit_ms":30} | KALDI |
| capped_query | {"kind":"capped_query","case_id":10,"text":"isimli","variant":"cap-four","p95_ms":39.6815,"limit_ms":30} | KALDI |
| capped_query | {"kind":"capped_query","case_id":15,"text":"çalışmalarıyla","variant":"cap","p95_ms":345.1081,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":15,"text":"çalışmalarıyla","variant":"cap-four","p95_ms":680.9603,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":15,"text":"çalışmalarıyla","variant":"cap-hybrid","p95_ms":585.3624,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":16,"text":"üreticisi","variant":"cap-four","p95_ms":155.40709999999999,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":22,"text":"ormanların","variant":"cap","p95_ms":524.428,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":22,"text":"ormanların","variant":"cap-four","p95_ms":667.0949,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":22,"text":"ormanların","variant":"cap-hybrid","p95_ms":689.5688,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":23,"text":"yanıklarda","variant":"cap","p95_ms":1235.9034,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":23,"text":"yanıklarda","variant":"cap-four","p95_ms":546.4341000000001,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":23,"text":"yanıklarda","variant":"cap-hybrid","p95_ms":582.0494000000001,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":24,"text":"külliyatı","variant":"cap","p95_ms":907.1116999999999,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":24,"text":"külliyatı","variant":"cap-four","p95_ms":394.781,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":24,"text":"külliyatı","variant":"cap-hybrid","p95_ms":458.67629999999997,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":28,"text":"geldiği buldu","variant":"cap-four","p95_ms":205.91660000000002,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":28,"text":"geldiği buldu","variant":"cap-hybrid","p95_ms":154.3951,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":30,"text":"kısım dişli","variant":"cap","p95_ms":526.7937000000001,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":30,"text":"kısım dişli","variant":"cap-four","p95_ms":535.3459,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":30,"text":"kısım dişli","variant":"cap-hybrid","p95_ms":561.5992,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":32,"text":"İnternetten hazinem","variant":"cap","p95_ms":226.4274,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":32,"text":"İnternetten hazinem","variant":"cap-hybrid","p95_ms":189.1736,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":33,"text":"oyuncudur atlanta","variant":"cap","p95_ms":195.5764,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":33,"text":"oyuncudur atlanta","variant":"cap-hybrid","p95_ms":175.6258,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":34,"text":"kendisine çıkacağını","variant":"cap","p95_ms":1006.8045999999999,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":34,"text":"kendisine çıkacağını","variant":"cap-four","p95_ms":460.56570000000005,"limit_ms":150} | KALDI |
| capped_query | {"kind":"capped_query","case_id":34,"text":"kendisine çıkacağını","variant":"cap-hybrid","p95_ms":523.0534,"limit_ms":150} | KALDI |
| hybrid_negative | {"kind":"hybrid_negative","text":"qzxkvwnever","control_p95_ms":0.3685,"hybrid_p95_ms":0.612,"limit_ms":0.40535000000000004,"passed":false} | KALDI |
| hybrid_negative | {"kind":"hybrid_negative","text":"qzxkvwnever","control_p95_ms":0.3685,"hybrid_p95_ms":0.612,"limit_ms":0.40535000000000004,"passed":false} | KALDI |
| hybrid_negative | {"kind":"hybrid_negative","text":"transubstantiationz","control_p95_ms":0.8681,"hybrid_p95_ms":0.579,"limit_ms":0.95491,"passed":true} | GEÇTİ |
| hybrid_positive_cpu | {"kind":"hybrid_positive_cpu","text":"olduğu","serial_total_cpu_ms":9937.5,"hybrid_total_cpu_ms":10281.25,"limit_ms":10434.375,"passed":true} | GEÇTİ |
| hybrid_positive_cpu | {"kind":"hybrid_positive_cpu","text":"büyük","serial_total_cpu_ms":6578.125,"hybrid_total_cpu_ms":6531.25,"limit_ms":6907.03125,"passed":true} | GEÇTİ |
| hybrid_positive_cpu | {"kind":"hybrid_positive_cpu","text":"ayrıca","serial_total_cpu_ms":9375.0,"hybrid_total_cpu_ms":9343.75,"limit_ms":9843.75,"passed":true} | GEÇTİ |

### 100000 kapısı

**FDB ingest KALDI; arama/iptal aşaması BLOKE.** Tam 100k DB yok;
açılış/RAM/arama kapıları ölçülemedi. SQLite bağımsız referansı tamamlandı.

| Kontrol | Kanıt | Durum |
|---|---|---|
| Bağımsız 100k | FDB ingest KALDI; SQLite iki tekrar ve oracle tamam | KISMEN TAMAMLANDI |
| Capped arama ana kapısı | ölçülen 0 sorgu/yol | BLOKE |
| streamed_entry_limit | {"kind":"streamed_entry_limit","error":"streamed transaction exceeds entry/frame allowance","attempted_documents":100000,"committed_documents":66000,"sequence":132,"rejected_batch":[66000,66499],"offending_document":66498,"text_bytes":444345,"per_entry_limit_bytes":262144,"transaction_limit_bytes":67108864,"rejected_batch_json_bytes":2308679,"raw_process":"trwiki-scale-20261028-v1-n100000-pair0-seed-fdb-scale-seed.process.json","stats":{"compaction_running":false,"delta_keys":0,"disk_index_bytes":178263877,"disk_posting_entries":37009038,"documents":66000,"generation":345,"log_bytes":45,"resident_documents":0,"resident_text_bytes":0,"segments":12,"sequence":132,"source_bytes":225615947},"partial_state":"C:\\Users\\Serkan\\Documents\\ChatGPT\\fdb\\docs\\benchmarks\\trwiki-scale-20261028-v1-partial66000-fdb-state.json","partial_state_sha256":"C404FA1551FCD1E8E5AE80418A824B39CEB118A4F1A5C55A9382DDD8E1FCD256","fdb_100k_ingest_succeeded":false,"fdb_retry":false,"truncate_or_owned_fallback":false} | KALDI |

## Durma sınırı ve doğruluk

Kapı kalınca daha küçük budget/metadata stratejisi, ön filtre veya format
değişimi yapılmadı. Bağımsız sonraki ölçek FDB ingest denendi ve entry limitinde
durdu; SQLite ingest/disk/RAM/arama tamamlandı. Tam 100k FDB metriği yok;
bağımlı FDB arama/cancel/yazma/snapshot/soğuk aşaması ilerlemedi.
444345B uzun belge cancel helper'ı bytes exact filtresiyle tek orijinal belgeyi
seçecek şekilde hazırlandı; fakat koşu çalışmadı. Belge streamed ingestte
reddedildiğinden444KB query cancellation gecikmesi ölçülemedi. Eski kitap
fixture0,411/1,436ms gecikmesi yeni TR
uzun belgelerine taşınmış garanti değildir.

Başarıyla tamamlanan ingestlerde bütün title/text/fields/links/version GET
oracle'ı, ölçülen her
arama sayfasında total/lower-bound ve tam hit oracle'ı geçti. CPU bloklarında
yalnız son sayfa interval dışında tam karşılaştırılır. Donmuş24Ekim okuyucu
commit edilmiş 66000 belgeyi full get ve41 sorguyla tekrar okudu;
reddedilen grubun 500 anahtarı GET ile görünmez doğrulandı (bunlar silinmiş
belgeler değil, oracle'ın absence/dead alanıyla kontrol edilen uncommitted
anahtarlardır). Normal alan signed-zero
tutarsızlığı ayrı açık kalır. Yeni corpus sorguları text-onlydir. Recovery ve
komut kanıtları son doğrulama ekinde; power-loss iddiası yok.

Ham [tasarım/program SHA/sıra/koruma](benchmarks/trwiki-scale-20261028-v1-design.json),
[p50/p95/max/CPU ve bütün örnek analizi](benchmarks/trwiki-scale-20261028-v1-analysis.json),
[50k kapı](benchmarks/trwiki-scale-20261028-v1-n50000-gates.json),
[100k bağımsız kapı](benchmarks/trwiki-scale-20261028-v1-n100000-gates.json),
[100k FDB ingest hatası/limit/commit sınırı](benchmarks/trwiki-scale-20261028-v1-n100000-ingest-failure.json),
[current slot pointer](benchmarks/active-benchmark-slots.json),
[corpus audit](benchmarks/trwiki-provided-20261028-v1-audit.json).

50k tekli/dirty500/4R+yazıcı/100'lük commit kök nedeni profili, uzun snapshot,
gerçek-soğuk cache; case34/parçalı geniş tarama, sorgu RAM'i açık. Signed-zero
birleştirilmedi. Default exact/serial; melez+cancel opt-in, join/CRC reuse kapalı,
reclaim default kapalı. Blok3/4,C ABI ve dış proje entegrasyonu yok.

## Tanı ve son doğrulama

50k araması durduktan sonra mevcut **66000 commit /12 segment** üzerinde
yalnız iki sorgunun counter tanısı alındı. Bu,50k/100k normal p95 matrisinin
yerine geçmez. Diagnostic release mutex/counter maliyeti taşır;1 örnek/yol,
bir warmup ve full GET/model oracle vardır. Kaynak/index değiştirilmedi.
Kaynak başına CRC tekrarları korunur; fiziksel disk erişimi değil, sıcak OS
cache'ine yapılan dosya okuma istekleri sayılmıştır.

| Sorgu | Yol | Süre ms | Posting probe (her biri4B) | Probe span toplamı ms | Doğrulanan aday | Payload read MB | CRC ms | Metin doğrulama ms | Sonuç |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| yanıklarda | cap-four | 448.321 | 380033 | 1247.448 | 6577 | 80.788 | 12.406 | 314.134 | 4 |
| yanıklarda | cap-hybrid | 473.001 | 380033 | 1229.673 | 6577 | 80.788 | 11.825 | 306.291 | 4 |
| yanıklarda | cap | 1229.964 | 380033 | 868.129 | 6577 | 80.788 | 8.165 | 249.987 | 4 |
| olduğu | cap-hybrid | 4.302 | 688 | 1.636 | 54 | 0.533 | 0.054 | 0.693 | 50 |
| olduğu | cap | 3.776 | 688 | 1.509 | 54 | 0.533 | 0.053 | 0.670 | 50 |
| olduğu | cap-four | 20.721 | 11420 | 36.508 | 1536 | 9.605 | 1.468 | 18.749 | 50 |

Seri tanıda posting probe span'leri toplam sürenin **%70.58**'si,
metin doğrulama **%20.32**, payload CRC **%0.66**.
Metin doğrulamanın içindeki body fold ayrı olarak 234.320ms;
bu alt span'ler üst span'lerle toplanmaz. Paralel satırlarda thread span'leri
örtüşür; wall yüzdesi veya CPU toplamı sayılmaz. Seri/4/melez aynı 6577 adayı
doğrular; melez prefix'i tekrar tüketmez.

Kodda 4096 ID'yi aşan posting grupları küçük-group cache'ine alınmıyor;
`contains()` her seed için `id_at()` üzerinden 4B probe'larıyla ikili arama
yapıyor. Ölçülen 380033 probe yalnız 1,52MB veri istediği halde seri tanıda
868ms tutuyor. Bu veri hacmi problemi kadar **okuma isteği sayısı** problemidir.
Sonraki öneri, aynı biçim/CRC/sıralı-ID kontrolleri ve bellek bütçesi içinde
sayfalı posting okuma veya ilerleyen intersection cursor'u denemek; bu tur
uygulanmadı. 50k p95'inin tam aşama yüzdeleri ayrıca ölçülmedi.

Pending background oturumu7.902ms'de döndü;
bu durumda process kill yapıldı. Sonraki normal strict-open+tam payload/version
GET ve reddedilen 500 anahtarın absence oracle'ı geçti. `.fdb/.ready` hash'leri
değişmedi. Bu process-kill kanıtıdır; güç kesintisi garantisi değildir.

### Komut kapıları

Paralel ilk `cargo test --all-targets` paketi ilerlemeden bekledi; elle
durduruldu, **GEÇTİ sayılmadı**. Sebebi teşhis edilmedi, açık işte tutulur.
Tekrar aşağıdaki aynı komutlarda `RUST_TEST_THREADS=1` ile yapıldı. Sadece
testlerin birbirleriyle paralelliği sınırlandı; testlerin kendi worker/thread
ve subprocess-kill senaryoları çalışmaya devam eder. Test değiştirilmedi.

| Komut | Durum | Log |
|---|---|---|
| `cargo fmt --check` | GEÇTİ | [fmt](benchmarks/snapshot-10k-20261026-validation-trwiki-scale-20261028-v2-fmt.log) |
| `cargo clippy --all-targets -- -D warnings` | GEÇTİ | [clippy](benchmarks/snapshot-10k-20261026-validation-trwiki-scale-20261028-v2-clippy.log) |
| `cargo test --all-targets` | GEÇTİ | [test](benchmarks/snapshot-10k-20261026-validation-trwiki-scale-20261028-v2-test.log) |
| `cargo check --no-default-features --lib` | GEÇTİ | [no-default](benchmarks/snapshot-10k-20261026-validation-trwiki-scale-20261028-v2-no-default.log) |
| `cargo check --all-features --lib` | GEÇTİ | [all-features](benchmarks/snapshot-10k-20261026-validation-trwiki-scale-20261028-v2-all-features.log) |
| `cargo fmt --manifest-path tools/native_sqlite/Cargo.toml --check` | GEÇTİ | [sqlite-fmt](benchmarks/snapshot-10k-20261026-validation-trwiki-scale-20261028-v2-sqlite-fmt.log) |
| `cargo clippy --manifest-path tools/native_sqlite/Cargo.toml --all-targets -- -D warnings` | GEÇTİ | [sqlite-clippy](benchmarks/snapshot-10k-20261026-validation-trwiki-scale-20261028-v2-sqlite-clippy.log) |
| `cargo test --manifest-path tools/native_sqlite/Cargo.toml --bin content_churn` | GEÇTİ | [sqlite-test](benchmarks/snapshot-10k-20261026-validation-trwiki-scale-20261028-v2-sqlite-test.log) |
| `cargo test --lib --features diagnostic-timing` (`RUST_TEST_THREADS=1`) | GEÇTİ | [diagnostic](benchmarks/trwiki-scale-20261028-v1-diagnostic-lib.log) |

Default suite: 239 geçti/14 subprocess helper ignored;
diagnostic lib: 145 geçti/12 helper ignored.
Kesilme/tek-byte bozulması, WAL/streamed commit, reclaim/merge/snapshot ve
process-kill kurtarma testleri bu seri tekrarın içindedir. Motorun
68 kaynak dosyasının SHA'sı önceki kabul edilen çalışma
sonuyla aynı; bu tur değişiklikleri ölçüm araçları ve raporlardır.

[Paralel doğrulama duruşu](benchmarks/trwiki-scale-20261028-v1-parallel-validation-stop.json),
[ilk test logu](benchmarks/snapshot-10k-20261026-validation-trwiki-scale-20261028-v1-test.log),
[tanı/kill/hash kanıtı](benchmarks/trwiki-scale-20261028-v1-post-verification.json),
[eski okuyucu66000 GET+500 absence+41 sorgu](benchmarks/trwiki-scale-20261028-v1-compat-partial66000-old-reader-utf8-v2.json).

Otomatik onay denetimi, durdurulan testten kalan 16 Temp klasörünü temizleme
işlemini `blocked by policy` gerekçesiyle reddetti; daha dar literal-path
denemesi de reddedildi. [6729B artık](benchmarks/trwiki-scale-20261028-v1-aborted-test-cleanup.json)
kaldı; corpus veya benchmark DB temizliği değildir.
