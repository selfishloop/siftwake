# Türkçe cap200k: 50k/100k yeni taban — 29 Ekim 2026

**GEÇTİ.** Sonraki yazma/snapshot/soğuk-cache aşamaları ayrı onay bekler.

Kullanıcının onayladığı payload SHA256: `881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12`. Kaynak `D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench` yalnız okundu; `private/` okunmadı. İlk8510408E… sürümü DB ölçümünde kullanılmadı. [Tam audit](TRWIKI-CAP200K-AUDIT-20261029.md): 100000 benzersiz gövde; eskiyle aynı sıra/anahtarlar ve41 sorgu. Yalnız45192/66498 metni kırpılmış; 45192 ilk50k içindedir. Bu rapor yeni bir tabandır; eski kitap/TR50k sonuçlarına birebir hız oranı kurulmaz.

Motor/format değişmedi. FDBLOG04/MAN03/SEG05/MET04,256KiB entry sınırı, kaynak+indeks CRC ve sıralı-ID kontrolleri aynı. Varsayılan exact/serial; capped/melez200/cancel opt-in. Join/CRC reuse kapalı. Ön filtre, Block3/4, C ABI veya proje entegrasyonu yapılmadı.

## Yöntem ve ölçüm sınırları

Her N için FDB/SQLite ardından SQLite/FDB; yalnız ownership kontrolünden geçen C: sabit `data/benchmark-work/fdb` ve `sqlite` slotları resetlendi. Yeni corpus/DB kopyası yok. Akışlı ingest500 işlem/commit; owned/Vec denenmedi. Kaynak title/text/fields/links/version korunur. Akışlı WAL katmanı bu ölçümde açık, revizyon geri kazanımı kapalıdır. FDB toplamına flush/merge ve son idle cleanup dahildir. Bu500lü ingest, sonraki onaylı100lü yazma profilinin yerine geçmez.

SQLite FTS5 trigram/detail=none/columnsize=0; WAL/FULL,cache2MiB,mmap0,tempFILE. Aynı numeric key sırası ve normalize title/text substring-AND oracle. FDB ayrıca fields/link indekslerini kurar; SQLite bunları payload içinde saklar. SQLite capped COUNT yapmaz, fakat key sıralaması için adayları tüketebilir.

Sıcak matriste full GET/version oracle+her yol bir warmup; sorgu/seçenek sırası döndürülür. Her case/yol10+10=20 normal release örneği. Ana kapı yalnız melez200,N50,offset0; exact serial/exact4 ve capped serial/4 karşılaştırmadır. Ortak≤30ms; diğer≤150ms@50k/≤300ms@100k. ve/if/id/zx kapı dışı. Windows process CPU user+kernel bütün query işçilerini kapsar; tek çağrı0/15,625ms kuantize olabilir; pozitif CPU ayrıca2000 çağrı/yol bloklarıyla ölçülür. Arka plan uygulamaları/OS tamamen izole edilmedi.

Bellek process private commit ve working set olarak ayrıdır. Ingest OS lifetime peak, katalog/model descriptor harness dahil; yalnız motor heap’i değildir.10ms örneklemeye ek OS peak sayaçları kullanılır. Temiz idle, oracle/katalog yüklenmeden yeni proseste2sn sonra. İlk sonuç yeni proseste; her kolun kendi bütün DB dosyaları zamanlama öncesi stream-read/hash ile ısıtılır. Gerçek soğuk-cache ölçümü değildir. Normal strict-open hiçbir doğrulama atlamaz; SQLite open aynı source-wide strict kontratı değildir.

## Ingest, disk ve bellek

İki örnek / ile ayrıldı. Disk decimal MB, bellek MiB. Ingest sonu tepe, sonradan yüklenen full oracle tepesinden ayrıdır.

Ölçüm kesintisi: ikinci100k SQLite matrisi sırasında driver/worker süreçleri kapanmış bulundu; kesin sonlandırma nedeni ölçülmedi. Tamamlanmış ingest/idle/ilk çift tekrarlanmadı. [25 çağrılık yarım log](benchmarks/cap200k-scale-20261029-v1-n100000-pair1-matrix-sqlite-scale-query-experiments.jsonl) korundu; en büyük kayıtlı çağrı 1946.650ms. Bu eksik kol pooled p95'e katılmadı; ikinci SQLite/FDB arama çifti baştan tamamlandı. Kaynak/binary/model hashları yeniden doğrulandı. Araya giren oturum farkı ölçüm sınırıdır.

| N | Motor | Ingest sn | Commit toplam sn | Bakım sn | Idle cleanup sn | Disk MB | Peak private MiB | Peak WS MiB | Idle private max MiB | Idle WS max MiB |
|---|---|---|---|---|---|---|---|---|---:|---:|
| 50000 | fdb | 57.976 / 51.973 | 51.153 / 45.333 | 6.439 / 6.262 | 0.384 / 0.378 | 300.836 / 300.836 | 20.371 / 19.152 | 24.516 / 23.559 | 2.207 | 6.059 |
| 50000 | sqlite | 50.112 / 49.548 | 47.807 / 47.432 | 2.305 / 2.115 | 0.000 / 0.000 | 421.736 / 421.736 | 18.012 / 17.949 | 19.895 / 19.938 | 3.020 | 7.238 |
| 100000 | fdb | 103.883 / 105.063 | 91.644 / 92.701 | 11.458 / 11.585 | 0.781 / 0.776 | 600.690 / 600.690 | 28.910 / 27.445 | 33.074 / 31.719 | 3.695 | 7.352 |
| 100000 | sqlite | 93.620 / 95.095 | 89.525 / 91.194 | 4.095 / 3.901 | 0.000 / 0.000 | 834.888 / 834.888 | 17.941 / 18.051 | 22.582 / 22.551 | 3.281 | 7.488 |

| N | Motor | Source MB | Index MB |500lü ingest commit p50/p95/max ms (pooled) |
|---|---|---|---|---|
| 50000 | fdb | 171.265 / 171.265 | 129.570 / 129.570 | 376.32/1417.07/1698.74 |
| 50000 | sqlite | — | — | 443.17/689.64/1228.60 |
| 100000 | fdb | 340.905 / 340.905 | 259.784 / 259.784 | 338.11/1372.23/1570.41 |
| 100000 | sqlite | — | — | 409.03/652.56/1792.50 |

SQLite aynı dosyada source/index tuttuğundan burada ayrıştırılmadı; retained üstte tamdır.
500lü commit tail’i tekli/100lü commit kapısı değildir; henüz kök-neden optimizasyonu yapılmadı.

| N | Arama süreci | OS peak private MiB | OS peak WS MiB |
|---|---|---|---|
| 50000 | fdb | 42.301 / 17.805 / 40.758 / 18.953 | 46.227 / 19.270 / 44.910 / 20.348 |
| 50000 | sqlite | 35.484 / 35.395 | 39.824 / 39.871 |
| 100000 | fdb | 53.133 / 19.512 / 53.398 / 20.152 | 57.125 / 23.562 / 57.992 / 24.848 |
| 100000 | sqlite | 43.129 / 43.289 | 47.520 / 48.293 |

Bu son arama tepesinde full GET oracle, model/katalog ve41 case’in beklenen owned sayfaları da vardır; tek sorgu motor RAM’i diye sunulmaz.

## Açılış ve ilk sonuç

| N | Motor | Temiz idle-open örnekleri ms | p95 ms |
|---|---|---|---:|
| 50000 | fdb | 309.867 / 263.607 / 261.371 / 257.501 | 309.867 |
| 50000 | sqlite | 1.608 / 1.532 / 1.854 / 1.504 | 1.854 |
| 100000 | fdb | 533.681 / 542.149 / 521.234 / 529.483 | 542.149 |
| 100000 | sqlite | 1.834 / 1.754 / 1.579 / 1.706 | 1.834 |

İlk sonuç: session=normal open, first=oturum başlangıcından hit dönüşüne toplam. Her familyada2 örnek; p95=max, üretim tail garantisi değildir. Tam doğrulama FDB strict session sonunda tamamdır.

| N | Sorgu | Motor | Session p95 ms | İlk sonuç p95 ms | Session sonrası fark p95 ms |
|---|---|---|---:|---:|---:|
| 50000 | akarsu | fdb | 283.311 | 295.739 | 12.428 |
| 50000 | furlong | fdb | 274.853 | 281.072 | 7.067 |
| 50000 | ilçesine köyleri | fdb | 286.731 | 291.622 | 4.891 |
| 50000 | olduğu | fdb | 259.964 | 262.305 | 2.394 |
| 50000 | qzxkvwnever | fdb | 261.391 | 262.067 | 0.723 |
| 50000 | zx | fdb | 283.989 | 384.779 | 100.790 |
| 50000 | akarsu | sqlite | 4.271 | 107.235 | 103.548 |
| 50000 | furlong | sqlite | 1.628 | 11.656 | 10.072 |
| 50000 | ilçesine köyleri | sqlite | 1.913 | 72.162 | 70.333 |
| 50000 | olduğu | sqlite | 1.738 | 252.422 | 250.684 |
| 50000 | qzxkvwnever | sqlite | 1.614 | 4.981 | 3.366 |
| 50000 | zx | sqlite | 1.901 | 455.684 | 453.783 |
| 100000 | akarsu | fdb | 555.066 | 569.010 | 14.444 |
| 100000 | furlong | fdb | 550.428 | 562.542 | 12.114 |
| 100000 | ilçesine köyleri | fdb | 559.031 | 565.660 | 7.047 |
| 100000 | olduğu | fdb | 569.013 | 573.458 | 4.444 |
| 100000 | qzxkvwnever | fdb | 546.846 | 548.139 | 1.292 |
| 100000 | zx | fdb | 566.315 | 713.718 | 147.402 |
| 100000 | akarsu | sqlite | 1.705 | 235.538 | 233.833 |
| 100000 | furlong | sqlite | 12.202 | 29.579 | 17.377 |
| 100000 | ilçesine köyleri | sqlite | 1.648 | 136.703 | 135.055 |
| 100000 | olduğu | sqlite | 2.136 | 593.485 | 591.668 |
| 100000 | qzxkvwnever | sqlite | 2.105 | 7.872 | 5.956 |
| 100000 | zx | sqlite | 12.039 | 971.914 | 959.897 |

## 50000:41 sorgunun tamamı

| Sorgu | Exact total | Exact seri | Exact4 | Cap seri | Cap4 | Melez200 | SQLite exact | SQLite cap |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| olduğu | 14299 | 306.275 | 127.564 | 2.256 | 4.372 | 2.376 | 729.233 | 360.281 |
| büyük | 15316 | 287.148 | 114.759 | 2.629 | 4.086 | 2.158 | 691.203 | 345.915 |
| ayrıca | 14965 | 323.306 | 126.910 | 2.530 | 4.793 | 2.500 | 856.757 | 422.913 |
| arasında | 15854 | 446.846 | 178.770 | 2.848 | 4.332 | 2.863 | 1080.484 | 553.408 |
| birlikte | 10219 | 319.609 | 125.771 | 2.358 | 4.734 | 2.926 | 690.468 | 382.420 |
| sanat | 6171 | 309.204 | 123.489 | 3.372 | 5.533 | 3.118 | 747.009 | 371.881 |
| görenler | 3379 | 409.344 | 159.333 | 7.411 | 13.193 | 6.768 | 809.159 | 368.515 |
| millî | 4603 | 90.795 | 36.195 | 2.671 | 4.154 | 2.080 | 221.613 | 110.818 |
| ikinci | 9031 | 321.977 | 117.178 | 2.940 | 5.218 | 2.707 | 851.377 | 434.531 |
| toplam | 5685 | 289.069 | 110.164 | 2.612 | 6.553 | 2.815 | 612.603 | 312.824 |
| isimli | 2763 | 365.427 | 140.357 | 7.592 | 13.268 | 8.578 | 874.868 | 442.418 |
| öldürür | 402 | 188.760 | 80.559 | 26.113 | 35.325 | 27.486 | 375.671 | 198.867 |
| ölümü | 7952 | 289.045 | 116.515 | 3.547 | 5.449 | 3.324 | 696.057 | 333.069 |
| akarsu | 515 | 111.898 | 47.894 | 14.484 | 21.809 | 15.901 | 270.941 | 143.657 |
| demokratik | 904 | 63.917 | 24.688 | 4.203 | 8.443 | 5.123 | 129.743 | 70.846 |
| çalışmalarıyla | 249 | 306.629 | 122.502 | 51.566 | 82.585 | 75.364 | 540.555 | 280.753 |
| üreticisi | 424 | 216.314 | 89.493 | 27.139 | 39.636 | 31.238 | 430.207 | 216.947 |
| joseph | 1027 | 20.492 | 13.427 | 2.505 | 4.135 | 2.028 | 85.046 | 45.236 |
| leyla | 287 | 247.717 | 100.381 | 45.731 | 57.772 | 50.834 | 562.114 | 301.401 |
| protest | 912 | 100.449 | 46.306 | 7.664 | 10.599 | 7.931 | 209.476 | 116.706 |
| furlong | 4 | 8.749 | 4.433 | 7.168 | 4.316 | 7.289 | 18.886 | 9.698 |
| korkaklığa | 2 | 29.886 | 15.848 | 30.510 | 14.523 | 30.683 | 72.066 | 37.477 |
| ormanların | 194 | 349.681 | 141.978 | 95.738 | 115.334 | 116.762 | 710.411 | 361.930 |
| yanıklarda | 4 | 272.402 | 112.807 | 282.554 | 112.696 | 129.380 | 525.118 | 269.177 |
| külliyatı | 27 | 235.585 | 92.708 | 220.569 | 96.831 | 110.712 | 432.993 | 233.332 |
| timmy | 22 | 17.752 | 8.042 | 15.743 | 7.779 | 16.342 | 42.488 | 21.975 |
| yüklük | 508 | 192.128 | 76.403 | 19.738 | 26.073 | 21.667 | 365.354 | 201.590 |
| ilçesine köyleri | 1313 | 88.722 | 36.237 | 6.172 | 7.347 | 4.522 | 189.727 | 98.630 |
| geldiği buldu | 365 | 222.872 | 90.572 | 35.766 | 44.749 | 38.289 | 435.724 | 245.433 |
| chicago studio | 33 | 6.914 | 3.533 | 7.033 | 3.751 | 8.309 | 22.592 | 13.236 |
| kısım dişli | 110 | 305.296 | 121.568 | 132.233 | 122.896 | 135.793 | 592.963 | 294.378 |
| güzeli zafer | 16 | 34.966 | 17.550 | 34.450 | 16.000 | 36.312 | 62.418 | 41.669 |
| İnternetten hazinem | 1 | 94.022 | 43.727 | 92.368 | 45.299 | 75.230 | 253.551 | 128.193 |
| oyuncudur atlanta | 10 | 74.699 | 31.728 | 74.721 | 33.909 | 63.926 | 143.230 | 69.294 |
| kendisine çıkacağını | 17 | 221.094 | 87.724 | 225.391 | 90.146 | 107.511 | 447.981 | 219.625 |
| ve | 47523 | 275.273 | 127.164 | 1.024 | 2.034 | 1.087 | 1006.770 | 546.692 |
| if | 22579 | 601.939 | 238.735 | 2.042 | 3.098 | 2.301 | 1468.078 | 682.061 |
| id | 38276 | 475.825 | 204.271 | 1.594 | 2.725 | 1.477 | 1172.166 | 625.513 |
| zx | 28 | 309.630 | 132.615 | 341.813 | 130.988 | 138.802 | 1232.697 | 621.388 |
| qzxkvwnever | 0 | 0.028 | 0.581 | 0.042 | 0.385 | 1.066 | 0.233 | 0.423 |
| transubstantiationz | 0 | 2.735 | 1.002 | 0.838 | 0.848 | 0.716 | 7.933 | 3.290 |

Süreler pooled p95 ms. — önceki kapı nedeniyle çalıştırılmadı,0ms değildir.

### 50000:CPU ortalama/toplam ms

| Sorgu | Exact seri | Exact4 | Cap seri | Cap4 | Melez200 | SQLite exact | SQLite cap |
|---|---:|---:|---:|---:|---:|---:|---:|
| olduğu | 279.688/5593.750 | 311.719/6234.375 | 3.125/62.500 | 7.812/156.250 | 0.000/0.000 | 617.188/12343.750 | 311.719/6234.375 |
| büyük | 253.906/5078.125 | 289.062/5781.250 | 2.344/46.875 | 1.562/31.250 | 1.562/31.250 | 571.094/11421.875 | 289.062/5781.250 |
| ayrıca | 298.438/5968.750 | 325.781/6515.625 | 2.344/46.875 | 10.938/218.750 | 1.562/31.250 | 747.656/14953.125 | 373.438/7468.750 |
| arasında | 400.781/8015.625 | 455.469/9109.375 | 1.562/31.250 | 11.719/234.375 | 1.562/31.250 | 985.156/19703.125 | 486.719/9734.375 |
| birlikte | 284.375/5687.500 | 327.344/6546.875 | 0.781/15.625 | 8.594/171.875 | 1.562/31.250 | 609.375/12187.500 | 314.844/6296.875 |
| sanat | 269.531/5390.625 | 310.156/6203.125 | 2.344/46.875 | 10.156/203.125 | 5.469/109.375 | 606.250/12125.000 | 314.062/6281.250 |
| görenler | 368.750/7375.000 | 416.406/8328.125 | 5.469/109.375 | 28.125/562.500 | 8.594/171.875 | 650.781/13015.625 | 322.656/6453.125 |
| millî | 79.688/1593.750 | 86.719/1734.375 | 0.781/15.625 | 2.344/46.875 | 0.000/0.000 | 165.625/3312.500 | 86.719/1734.375 |
| ikinci | 278.125/5562.500 | 301.562/6031.250 | 3.906/78.125 | 7.812/156.250 | 0.781/15.625 | 726.562/14531.250 | 376.562/7531.250 |
| toplam | 250.000/5000.000 | 271.875/5437.500 | 3.125/62.500 | 14.062/281.250 | 1.562/31.250 | 526.562/10531.250 | 270.312/5406.250 |
| isimli | 306.250/6125.000 | 347.656/6953.125 | 8.594/171.875 | 32.031/640.625 | 5.469/109.375 | 785.938/15718.750 | 388.281/7765.625 |
| öldürür | 169.531/3390.625 | 191.406/3828.125 | 24.219/484.375 | 90.625/1812.500 | 46.875/937.500 | 318.750/6375.000 | 162.500/3250.000 |
| ölümü | 264.062/5281.250 | 289.844/5796.875 | 1.562/31.250 | 10.156/203.125 | 3.125/62.500 | 561.719/11234.375 | 282.031/5640.625 |
| akarsu | 99.219/1984.375 | 109.375/2187.500 | 13.281/265.625 | 49.219/984.375 | 15.625/312.500 | 244.531/4890.625 | 122.656/2453.125 |
| demokratik | 50.000/1000.000 | 60.938/1218.750 | 5.469/109.375 | 10.156/203.125 | 3.125/62.500 | 110.938/2218.750 | 60.938/1218.750 |
| çalışmalarıyla | 277.344/5546.875 | 314.844/6296.875 | 47.656/953.125 | 200.781/4015.625 | 164.844/3296.875 | 477.344/9546.875 | 239.062/4781.250 |
| üreticisi | 189.844/3796.875 | 225.781/4515.625 | 23.438/468.750 | 85.156/1703.125 | 48.438/968.750 | 382.031/7640.625 | 192.188/3843.750 |
| joseph | 17.188/343.750 | 26.562/531.250 | 3.125/62.500 | 1.562/31.250 | 2.344/46.875 | 71.094/1421.875 | 36.719/734.375 |
| leyla | 221.094/4421.875 | 250.781/5015.625 | 40.625/812.500 | 136.719/2734.375 | 99.219/1984.375 | 493.750/9875.000 | 250.000/5000.000 |
| protest | 86.719/1734.375 | 99.219/1984.375 | 7.031/140.625 | 16.406/328.125 | 8.594/171.875 | 175.781/3515.625 | 91.406/1828.125 |
| furlong | 9.375/187.500 | 5.469/109.375 | 7.812/156.250 | 7.031/140.625 | 4.688/93.750 | 14.844/296.875 | 7.031/140.625 |
| korkaklığa | 25.781/515.625 | 36.719/734.375 | 24.219/484.375 | 39.062/781.250 | 25.781/515.625 | 56.250/1125.000 | 26.562/531.250 |
| ormanların | 318.750/6375.000 | 356.250/7125.000 | 89.062/1781.250 | 292.969/5859.375 | 274.219/5484.375 | 623.438/12468.750 | 304.688/6093.750 |
| yanıklarda | 251.562/5031.250 | 279.688/5593.750 | 248.438/4968.750 | 287.500/5750.000 | 277.344/5546.875 | 450.000/9000.000 | 228.125/4562.500 |
| külliyatı | 206.250/4125.000 | 231.250/4625.000 | 206.250/4125.000 | 238.281/4765.625 | 232.031/4640.625 | 389.844/7796.875 | 199.219/3984.375 |
| timmy | 15.625/312.500 | 14.062/281.250 | 13.281/265.625 | 18.750/375.000 | 12.500/250.000 | 32.031/640.625 | 17.188/343.750 |
| yüklük | 164.062/3281.250 | 188.281/3765.625 | 17.969/359.375 | 69.531/1390.625 | 26.562/531.250 | 306.250/6125.000 | 150.000/3000.000 |
| ilçesine köyleri | 75.000/1500.000 | 92.188/1843.750 | 5.469/109.375 | 16.406/328.125 | 3.125/62.500 | 165.625/3312.500 | 86.719/1734.375 |
| geldiği buldu | 202.344/4046.875 | 235.156/4703.125 | 26.562/531.250 | 105.469/2109.375 | 59.375/1187.500 | 378.906/7578.125 | 199.219/3984.375 |
| chicago studio | 6.250/125.000 | 7.812/156.250 | 3.906/78.125 | 6.250/125.000 | 5.469/109.375 | 18.750/375.000 | 10.938/218.750 |
| kısım dişli | 266.406/5328.125 | 304.688/6093.750 | 124.219/2484.375 | 303.125/6062.500 | 308.594/6171.875 | 540.625/10812.500 | 266.406/5328.125 |
| güzeli zafer | 30.469/609.375 | 44.531/890.625 | 31.250/625.000 | 41.406/828.125 | 31.250/625.000 | 57.031/1140.625 | 32.031/640.625 |
| İnternetten hazinem | 80.469/1609.375 | 93.750/1875.000 | 83.594/1671.875 | 94.531/1890.625 | 96.875/1937.500 | 220.312/4406.250 | 112.500/2250.000 |
| oyuncudur atlanta | 65.625/1312.500 | 71.094/1421.875 | 64.062/1281.250 | 75.781/1515.625 | 67.969/1359.375 | 121.875/2437.500 | 58.594/1171.875 |
| kendisine çıkacağını | 188.281/3765.625 | 217.188/4343.750 | 191.406/3828.125 | 213.281/4265.625 | 212.500/4250.000 | 371.875/7437.500 | 189.844/3796.875 |
| ve | 239.062/4781.250 | 302.344/6046.875 | 0.000/0.000 | 3.906/78.125 | 0.781/15.625 | 871.875/17437.500 | 443.750/8875.000 |
| if | 535.938/10718.750 | 620.312/12406.250 | 0.781/15.625 | 3.125/62.500 | 1.562/31.250 | 1251.562/25031.250 | 612.500/12250.000 |
| id | 435.156/8703.125 | 525.781/10515.625 | 3.125/62.500 | 4.688/93.750 | 0.000/0.000 | 1014.844/20296.875 | 519.531/10390.625 |
| zx | 262.500/5250.000 | 335.156/6703.125 | 309.375/6187.500 | 333.594/6671.875 | 341.406/6828.125 | 1100.781/22015.625 | 544.531/10890.625 |
| qzxkvwnever | 0.000/0.000 | 0.000/0.000 | 0.000/0.000 | 0.000/0.000 | 0.781/15.625 | 0.000/0.000 | 0.000/0.000 |
| transubstantiationz | 0.781/15.625 | 6.250/125.000 | 0.000/0.000 | 0.781/15.625 | 0.781/15.625 | 9.375/187.500 | 1.562/31.250 |

## 100000:41 sorgunun tamamı

| Sorgu | Exact total | Exact seri | Exact4 | Cap seri | Cap4 | Melez200 | SQLite exact | SQLite cap |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| olduğu | 28407 | 659.943 | 225.260 | 2.471 | 7.384 | 2.975 | 1499.700 | 744.333 |
| büyük | 30642 | 618.343 | 208.360 | 3.639 | 7.171 | 2.167 | 1348.268 | 677.769 |
| ayrıca | 29882 | 719.851 | 232.270 | 3.729 | 8.871 | 2.986 | 1644.520 | 864.019 |
| arasında | 31555 | 950.471 | 324.342 | 3.393 | 7.730 | 5.497 | 2362.697 | 1141.362 |
| birlikte | 20347 | 664.573 | 226.425 | 3.392 | 7.993 | 4.690 | 1421.408 | 717.955 |
| sanat | 12436 | 656.643 | 228.426 | 3.608 | 9.474 | 4.054 | 1401.668 | 743.506 |
| görenler | 6572 | 862.682 | 277.540 | 9.667 | 23.456 | 8.063 | 1478.779 | 746.953 |
| millî | 9256 | 175.504 | 61.257 | 3.087 | 7.400 | 2.284 | 370.420 | 187.343 |
| ikinci | 17915 | 635.887 | 210.257 | 3.544 | 9.569 | 4.214 | 1625.406 | 763.064 |
| toplam | 11405 | 581.316 | 188.448 | 3.288 | 10.934 | 4.770 | 1195.116 | 570.036 |
| isimli | 5458 | 694.739 | 240.766 | 8.032 | 23.632 | 7.836 | 1654.346 | 894.837 |
| öldürür | 812 | 385.169 | 127.093 | 27.051 | 68.617 | 36.395 | 752.672 | 356.417 |
| ölümü | 15800 | 615.812 | 202.487 | 4.779 | 9.134 | 3.868 | 1282.363 | 662.492 |
| akarsu | 960 | 232.941 | 79.620 | 14.267 | 39.120 | 16.669 | 569.561 | 305.420 |
| demokratik | 1806 | 117.360 | 40.186 | 5.065 | 14.716 | 7.375 | 233.817 | 117.508 |
| çalışmalarıyla | 505 | 644.845 | 218.583 | 62.743 | 151.317 | 133.090 | 1062.150 | 576.259 |
| üreticisi | 815 | 460.983 | 154.977 | 31.803 | 70.103 | 56.178 | 1002.868 | 450.051 |
| joseph | 2051 | 45.339 | 18.562 | 4.023 | 7.683 | 2.425 | 192.246 | 83.165 |
| leyla | 591 | 525.394 | 176.274 | 49.254 | 105.748 | 80.643 | 1124.919 | 543.059 |
| protest | 1822 | 217.025 | 70.686 | 7.984 | 19.466 | 10.563 | 396.706 | 199.827 |
| furlong | 9 | 16.441 | 5.826 | 14.499 | 6.053 | 15.071 | 31.902 | 16.116 |
| korkaklığa | 3 | 62.014 | 23.851 | 62.649 | 22.056 | 45.134 | 129.662 | 62.610 |
| ormanların | 404 | 738.246 | 240.884 | 105.452 | 202.123 | 194.666 | 1368.209 | 702.764 |
| yanıklarda | 8 | 590.968 | 192.875 | 606.939 | 191.014 | 197.829 | 1025.673 | 537.500 |
| külliyatı | 80 | 472.751 | 157.527 | 353.369 | 160.797 | 166.256 | 865.275 | 422.678 |
| timmy | 42 | 35.891 | 14.923 | 33.516 | 13.798 | 22.685 | 69.698 | 38.629 |
| yüklük | 983 | 380.522 | 131.794 | 22.056 | 54.578 | 32.537 | 658.657 | 325.092 |
| ilçesine köyleri | 2590 | 170.194 | 57.607 | 9.350 | 12.825 | 6.629 | 367.722 | 182.892 |
| geldiği buldu | 726 | 467.913 | 165.072 | 32.212 | 86.783 | 54.513 | 855.235 | 465.102 |
| chicago studio | 55 | 16.413 | 7.151 | 12.180 | 7.434 | 15.989 | 43.751 | 25.897 |
| kısım dişli | 212 | 635.765 | 212.543 | 145.400 | 204.459 | 210.080 | 1275.121 | 624.203 |
| güzeli zafer | 37 | 70.833 | 28.246 | 76.713 | 26.808 | 43.351 | 139.329 | 74.022 |
| İnternetten hazinem | 1 | 196.613 | 64.376 | 199.699 | 66.285 | 87.657 | 508.844 | 269.281 |
| oyuncudur atlanta | 18 | 146.900 | 48.731 | 147.238 | 51.068 | 69.873 | 288.670 | 143.311 |
| kendisine çıkacağını | 34 | 449.573 | 148.848 | 437.865 | 146.962 | 159.153 | 804.293 | 431.376 |
| ve | 94949 | 557.014 | 221.491 | 1.155 | 2.890 | 0.970 | 1887.957 | 1008.258 |
| if | 45182 | 1244.041 | 436.806 | 2.218 | 7.038 | 2.500 | 2732.517 | 1338.827 |
| id | 76428 | 1049.808 | 377.682 | 1.700 | 4.939 | 1.647 | 2508.327 | 1172.756 |
| zx | 47 | 631.996 | 236.284 | 753.613 | 245.193 | 237.652 | 2656.389 | 1284.059 |
| qzxkvwnever | 0 | 0.043 | 0.590 | 0.068 | 0.552 | 1.474 | 0.297 | 0.586 |
| transubstantiationz | 0 | 6.038 | 1.054 | 1.365 | 1.198 | 1.294 | 14.640 | 6.827 |

Süreler pooled p95 ms. — önceki kapı nedeniyle çalıştırılmadı,0ms değildir.

### 100000:CPU ortalama/toplam ms

| Sorgu | Exact seri | Exact4 | Cap seri | Cap4 | Melez200 | SQLite exact | SQLite cap |
|---|---:|---:|---:|---:|---:|---:|---:|
| olduğu | 584.375/11687.500 | 649.219/12984.375 | 4.688/93.750 | 21.094/421.875 | 3.125/62.500 | 1257.812/25156.250 | 631.250/12625.000 |
| büyük | 521.875/10437.500 | 590.625/11812.500 | 0.781/15.625 | 21.875/437.500 | 1.562/31.250 | 1149.219/22984.375 | 565.625/11312.500 |
| ayrıca | 611.719/12234.375 | 663.281/13265.625 | 3.906/78.125 | 19.531/390.625 | 3.125/62.500 | 1449.219/28984.375 | 733.594/14671.875 |
| arasında | 821.875/16437.500 | 917.188/18343.750 | 3.906/78.125 | 21.875/437.500 | 4.688/93.750 | 1978.125/39562.500 | 994.531/19890.625 |
| birlikte | 587.500/11750.000 | 654.688/13093.750 | 0.781/15.625 | 26.562/531.250 | 3.125/62.500 | 1178.125/23562.500 | 589.062/11781.250 |
| sanat | 556.250/11125.000 | 639.844/12796.875 | 3.125/62.500 | 21.875/437.500 | 3.906/78.125 | 1189.062/23781.250 | 610.938/12218.750 |
| görenler | 744.531/14890.625 | 827.344/16546.875 | 5.469/109.375 | 76.562/1531.250 | 6.250/125.000 | 1239.844/24796.875 | 628.125/12562.500 |
| millî | 152.344/3046.875 | 179.688/3593.750 | 0.000/0.000 | 25.000/500.000 | 2.344/46.875 | 312.500/6250.000 | 158.594/3171.875 |
| ikinci | 548.438/10968.750 | 629.688/12593.750 | 0.781/15.625 | 27.344/546.875 | 2.344/46.875 | 1395.312/27906.250 | 696.875/13937.500 |
| toplam | 492.969/9859.375 | 554.688/11093.750 | 2.344/46.875 | 21.094/421.875 | 2.344/46.875 | 1024.219/20484.375 | 515.625/10312.500 |
| isimli | 608.594/12171.875 | 674.219/13484.375 | 7.031/140.625 | 63.281/1265.625 | 7.031/140.625 | 1522.656/30453.125 | 773.438/15468.750 |
| öldürür | 329.688/6593.750 | 375.000/7500.000 | 19.531/390.625 | 191.406/3828.125 | 75.781/1515.625 | 616.406/12328.125 | 314.844/6296.875 |
| ölümü | 516.406/10328.125 | 574.219/11484.375 | 2.344/46.875 | 34.375/687.500 | 1.562/31.250 | 1132.031/22640.625 | 561.719/11234.375 |
| akarsu | 196.094/3921.875 | 214.062/4281.250 | 10.156/203.125 | 115.625/2312.500 | 24.219/484.375 | 500.000/10000.000 | 250.781/5015.625 |
| demokratik | 96.875/1937.500 | 113.281/2265.625 | 3.125/62.500 | 41.406/828.125 | 7.812/156.250 | 215.625/4312.500 | 104.688/2093.750 |
| çalışmalarıyla | 551.562/11031.250 | 607.031/12140.625 | 46.875/937.500 | 418.750/8375.000 | 357.031/7140.625 | 960.156/19203.125 | 487.500/9750.000 |
| üreticisi | 372.656/7453.125 | 428.906/8578.125 | 26.562/531.250 | 193.750/3875.000 | 117.188/2343.750 | 767.188/15343.750 | 375.000/7500.000 |
| joseph | 32.812/656.250 | 48.438/968.750 | 1.562/31.250 | 25.781/515.625 | 1.562/31.250 | 139.844/2796.875 | 66.406/1328.125 |
| leyla | 444.531/8890.625 | 496.094/9921.875 | 42.188/843.750 | 314.062/6281.250 | 205.469/4109.375 | 981.250/19625.000 | 481.250/9625.000 |
| protest | 182.812/3656.250 | 205.469/4109.375 | 4.688/93.750 | 56.250/1125.000 | 7.031/140.625 | 345.312/6906.250 | 170.312/3406.250 |
| furlong | 13.281/265.625 | 10.156/203.125 | 10.938/218.750 | 17.969/359.375 | 10.938/218.750 | 23.438/468.750 | 12.500/250.000 |
| korkaklığa | 51.562/1031.250 | 65.625/1312.500 | 53.125/1062.500 | 57.812/1156.250 | 50.000/1000.000 | 102.344/2046.875 | 51.562/1031.250 |
| ormanların | 626.562/12531.250 | 690.625/13812.500 | 89.062/1781.250 | 571.094/11421.875 | 560.938/11218.750 | 1203.906/24078.125 | 603.125/12062.500 |
| yanıklarda | 513.281/10265.625 | 561.719/11234.375 | 521.094/10421.875 | 552.344/11046.875 | 560.938/11218.750 | 897.656/17953.125 | 461.719/9234.375 |
| külliyatı | 413.281/8265.625 | 455.469/9109.375 | 303.906/6078.125 | 471.875/9437.500 | 458.594/9171.875 | 764.062/15281.250 | 375.000/7500.000 |
| timmy | 29.688/593.750 | 27.344/546.875 | 26.562/531.250 | 35.156/703.125 | 26.562/531.250 | 62.500/1250.000 | 32.031/640.625 |
| yüklük | 320.312/6406.250 | 378.906/7578.125 | 17.969/359.375 | 168.750/3375.000 | 60.156/1203.125 | 593.750/11875.000 | 297.656/5953.125 |
| ilçesine köyleri | 143.750/2875.000 | 160.938/3218.750 | 10.156/203.125 | 25.781/515.625 | 7.812/156.250 | 316.406/6328.125 | 162.500/3250.000 |
| geldiği buldu | 408.594/8171.875 | 458.594/9171.875 | 27.344/546.875 | 236.719/4734.375 | 129.688/2593.750 | 760.156/15203.125 | 382.812/7656.250 |
| chicago studio | 15.625/312.500 | 4.688/93.750 | 11.719/234.375 | 16.406/328.125 | 12.500/250.000 | 34.375/687.500 | 21.094/421.875 |
| kısım dişli | 543.750/10875.000 | 610.156/12203.125 | 121.875/2437.500 | 616.406/12328.125 | 621.094/12421.875 | 1056.250/21125.000 | 535.156/10703.125 |
| güzeli zafer | 64.062/1281.250 | 78.906/1578.125 | 64.844/1296.875 | 68.750/1375.000 | 72.656/1453.125 | 112.500/2250.000 | 60.156/1203.125 |
| İnternetten hazinem | 171.875/3437.500 | 190.625/3812.500 | 176.562/3531.250 | 189.062/3781.250 | 187.500/3750.000 | 444.531/8890.625 | 221.875/4437.500 |
| oyuncudur atlanta | 124.219/2484.375 | 137.500/2750.000 | 128.906/2578.125 | 147.656/2953.125 | 139.844/2796.875 | 232.812/4656.250 | 120.312/2406.250 |
| kendisine çıkacağını | 392.188/7843.750 | 432.812/8656.250 | 382.031/7640.625 | 435.938/8718.750 | 442.969/8859.375 | 715.625/14312.500 | 374.219/7484.375 |
| ve | 488.281/9765.625 | 624.219/12484.375 | 0.000/0.000 | 7.812/156.250 | 0.000/0.000 | 1700.000/34000.000 | 861.719/17234.375 |
| if | 1077.344/21546.875 | 1291.406/25828.125 | 1.562/31.250 | 17.188/343.750 | 1.562/31.250 | 2461.719/49234.375 | 1212.500/24250.000 |
| id | 914.062/18281.250 | 1092.188/21843.750 | 3.125/62.500 | 10.938/218.750 | 1.562/31.250 | 2089.844/41796.875 | 1021.094/20421.875 |
| zx | 534.375/10687.500 | 682.031/13640.625 | 642.188/12843.750 | 687.500/13750.000 | 685.156/13703.125 | 2275.781/45515.625 | 1124.219/22484.375 |
| qzxkvwnever | 0.000/0.000 | 0.781/15.625 | 0.000/0.000 | 2.344/46.875 | 3.125/62.500 | 0.000/0.000 | 0.781/15.625 |
| transubstantiationz | 3.125/62.500 | 6.250/125.000 | 0.000/0.000 | 8.594/171.875 | 0.000/0.000 | 7.812/156.250 | 8.594/171.875 |

### Gösterilen sonuç sayısının ölçekle ilişkisi

Belge sayısı2×. N50 bol sorgularda gösterilen hit sayısını50’de sabitler; bu bir doğrusal süre kapısı değildir. En az bir ölçekte50’den az hit bulunan sorgular aşağıdadır; exact toplamlar üstte ayrı verilir.

| Sorgu |50k gösterilen |100k gösterilen | Gösterilen oranı |
|---|---:|---:|---:|
| furlong | 4 | 9 | 2.250× |
| korkaklığa | 2 | 3 | 1.500× |
| yanıklarda | 4 | 8 | 2.000× |
| külliyatı | 27 | 50 | 1.852× |
| timmy | 22 | 42 | 1.909× |
| chicago studio | 33 | 50 | 1.515× |
| güzeli zafer | 16 | 37 | 2.312× |
| İnternetten hazinem | 1 | 1 | 1.000× |
| oyuncudur atlanta | 10 | 18 | 1.800× |
| kendisine çıkacağını | 17 | 34 | 2.000× |
| zx | 28 | 47 | 1.679× |
| qzxkvwnever | 0 | 0 | 0/0 |
| transubstantiationz | 0 | 0 | 0/0 |

## Kısa sorgular ve CPU kapıları

ve/if/id/zx iki karakterlidir; trigram reddi kullanılamaz. Exact bütün gerçek eşleşmeleri sayar. Capped50 hit dolunca AtLeast(50), dolmazsa Exact(total) döner; sıralı ilk50 sonuçlar exact ile aynıdır. Zx50k/100k total audit ve üstteki tabloda verilir;50 dolmazsa melez de bütün adayları tüketir. Capped ve tam tarama süre/CPU üstte ayrı gösterildi. Bu genel tarama maliyeti ana kapı dışıdır.

| N | Pozitif sorgu | Yol | Çağrı | CPU ms/çağrı | Toplam CPU ms |
|---|---|---|---:|---:|---:|
| 50000 | ayrıca | cap | 2000 | 1.640625 | 3281.250 |
| 50000 | ayrıca | cap-four | 2000 | 9.054688 | 18109.375 |
| 50000 | ayrıca | cap-hybrid | 2000 | 1.632812 | 3265.625 |
| 50000 | büyük | cap | 2000 | 1.421875 | 2843.750 |
| 50000 | büyük | cap-four | 2000 | 7.843750 | 15687.500 |
| 50000 | büyük | cap-hybrid | 2000 | 1.523438 | 3046.875 |
| 50000 | olduğu | cap | 2000 | 1.632812 | 3265.625 |
| 50000 | olduğu | cap-four | 2000 | 7.570312 | 15140.625 |
| 50000 | olduğu | cap-hybrid | 2000 | 1.656250 | 3312.500 |
| 100000 | ayrıca | cap | 2000 | 1.750000 | 3500.000 |
| 100000 | ayrıca | cap-four | 2000 | 18.296875 | 36593.750 |
| 100000 | ayrıca | cap-hybrid | 2000 | 1.820312 | 3640.625 |
| 100000 | büyük | cap | 2000 | 1.593750 | 3187.500 |
| 100000 | büyük | cap-four | 2000 | 17.335938 | 34671.875 |
| 100000 | büyük | cap-hybrid | 2000 | 1.640625 | 3281.250 |
| 100000 | olduğu | cap | 2000 | 1.851562 | 3703.125 |
| 100000 | olduğu | cap-four | 2000 | 18.242188 | 36484.375 |
| 100000 | olduğu | cap-hybrid | 2000 | 1.851562 | 3703.125 |

Negatif:4 işçi p95<1ms ise melez mutlak artış≤2ms, aksi+%10. Pozitif:serial CPU<10ms/çağrı ise mutlak artış≤0,2ms, aksi+%5. Önceki raw posting raporu değiştirilmedi; yeni kabul ayrı JSON’dadır.

## Kapı tablosu

| N | Kapı | Ölçüm | Durum |
|---|---|---|---|
| 50000 | Strict-open | 309.867≤1000 | GEÇTİ |
| 50000 | Idle private | 2.207≤15 | GEÇTİ |
| 50000 | Ingest peak private | 20.371≤60 | GEÇTİ |
| 50000 | Melez 37 ana sorgu | 37/37 | GEÇTİ |
| 50000 | negative_hybrid:qzxkvwnever | {"kind":"negative_hybrid","text":"qzxkvwnever","control_p95_ms":0.3028,"hybrid_p95_ms":0.041999999999999996,"limit_ms":2.3028,"passed":true} | GEÇTİ |
| 50000 | negative_hybrid:transubstantiationz | {"kind":"negative_hybrid","text":"transubstantiationz","control_p95_ms":0.707,"hybrid_p95_ms":0.5814,"limit_ms":2.707,"passed":true} | GEÇTİ |
| 50000 | positive_cpu:ayrıca | {"kind":"positive_cpu","text":"ayrıca","serial_ms_per_call":1.640625,"hybrid_ms_per_call":1.6328125,"limit_ms_per_call":1.840625,"passed":true} | GEÇTİ |
| 50000 | positive_cpu:büyük | {"kind":"positive_cpu","text":"büyük","serial_ms_per_call":1.421875,"hybrid_ms_per_call":1.5234375,"limit_ms_per_call":1.621875,"passed":true} | GEÇTİ |
| 50000 | positive_cpu:olduğu | {"kind":"positive_cpu","text":"olduğu","serial_ms_per_call":1.6328125,"hybrid_ms_per_call":1.65625,"limit_ms_per_call":1.8328125,"passed":true} | GEÇTİ |
| 50000 | Full payload/version + bütün41 sorgu oracle, Pending kill/reopen, eski okuyucu | aşağıdaki kanıt | GEÇTİ |
| 100000 | Strict-open | 569.013≤2000 | GEÇTİ |
| 100000 | Idle private | 3.695≤15 | GEÇTİ |
| 100000 | Ingest peak private | 28.910≤60 | GEÇTİ |
| 100000 | Melez 37 ana sorgu | 37/37 | GEÇTİ |
| 100000 | negative_hybrid:qzxkvwnever | {"kind":"negative_hybrid","text":"qzxkvwnever","control_p95_ms":0.28219999999999995,"hybrid_p95_ms":0.06470000000000001,"limit_ms":2.2822,"passed":true} | GEÇTİ |
| 100000 | negative_hybrid:transubstantiationz | {"kind":"negative_hybrid","text":"transubstantiationz","control_p95_ms":0.6572,"hybrid_p95_ms":0.9125,"limit_ms":2.6572,"passed":true} | GEÇTİ |
| 100000 | positive_cpu:ayrıca | {"kind":"positive_cpu","text":"ayrıca","serial_ms_per_call":1.75,"hybrid_ms_per_call":1.8203125,"limit_ms_per_call":1.95,"passed":true} | GEÇTİ |
| 100000 | positive_cpu:büyük | {"kind":"positive_cpu","text":"büyük","serial_ms_per_call":1.59375,"hybrid_ms_per_call":1.640625,"limit_ms_per_call":1.79375,"passed":true} | GEÇTİ |
| 100000 | positive_cpu:olduğu | {"kind":"positive_cpu","text":"olduğu","serial_ms_per_call":1.8515625,"hybrid_ms_per_call":1.8515625,"limit_ms_per_call":2.0515625,"passed":true} | GEÇTİ |
| 100000 | Full payload/version + bütün41 sorgu oracle, Pending kill/reopen, eski okuyucu | aşağıdaki kanıt | GEÇTİ |

## Kesinti, oracle ve uyumluluk

- 50000: Pending oturum dönüşü tanıklandıktan sonra child process kill; exit=1, yeniden strict-open ve full GET/version oracle geçti. Eski dondurulmuş okuyucu 50000 tam GET ve41 exact sorguda geçti; source SHA aynı.
- 100000: Pending oturum dönüşü tanıklandıktan sonra child process kill; exit=1, yeniden strict-open ve full GET/version oracle geçti. Eski dondurulmuş okuyucu 100000 tam GET ve41 exact sorguda geçti; source SHA aynı.

Bu kill, açık/okuma sırasında proses sonlandırmadır; churn/flush/rewrite içine yeni hata enjeksiyonu veya güç kesintisi iddiası değildir. Bu tur kaynak yazma/kurtarma algoritması değiştirilmedi.0.1 fixture ve eski10k corpus SHA’ları korundu. Yeni corpus baştan sona aynı SHA’da kaldı. Oracle sayfa/total/sequence ve owned payload/version alanlarını doğrular; signed-zero eski açık konu ayrı kalır.

## 200 KB belgelerde iptal

Exact bytes alanı uzun belgeyi tekil seçer;zx iki karakteri payload doğrulamasına ulaştırır. Cancel isteği aday doğrulama başladı tanığından sonra verilir.40 deneme/belge/yol, cancel→bütün query thread’lerinin join’i ölçülür. Sıcak doğrulanmış kaynak; senkron OS okuması zorla kesilmez.

| ID | Text B | Yol | Deneme | İptal | Bitiş yarışı | p50/p95/max ms |
|---|---:|---|---:|---:|---:|---|
| 45192 | 199999 | exact-four | 40 | 40 | 0 | 0.157/0.187/0.223 |
| 45192 | 199999 | four | 40 | 40 | 0 | 0.163/0.182/0.192 |
| 45192 | 199999 | hybrid | 40 | 40 | 0 | 0.108/0.132/0.136 |
| 45192 | 199999 | serial | 40 | 40 | 0 | 0.114/0.131/0.140 |
| 66498 | 200000 | exact-four | 40 | 40 | 0 | 0.173/0.206/0.217 |
| 66498 | 200000 | four | 40 | 40 | 0 | 0.165/0.200/0.267 |
| 66498 | 200000 | hybrid | 40 | 40 | 0 | 0.118/0.132/0.134 |
| 66498 | 200000 | serial | 40 | 40 | 0 | 0.116/0.141/0.162 |

İptal sonrası bütün sonuç yarışı/oracle ve check_integrity geçti. Tanık aday başlangıcıdır; metin taramasının rastgele bir noktasında iptal veya cold I/O/üretim SLA garantisi değildir.

## Ham kanıt ve açık kalanlar

- [Tasarım/komutlar/hashlar/bütün loglar](benchmarks/cap200k-scale-20261029-v1-design.json).
- [p50/p95/max,CPU,örnekler ve bütün kapılar](benchmarks/cap200k-scale-20261029-v1-analysis.json).
- [Audit/katalog](benchmarks/trwiki-cap200k-20261029-v3-audit.json); [kaynak onayı](benchmarks/cap200k-source-acceptance-20261029.json).
- [Fmt/clippy/default-parallel test ve feature/SQLite kontrolleri](benchmarks/snapshot-10k-20261026-validation-cap200k-20261029-v1.json).

Sonraki50k yazma profili (tekli,dirty500,4R+yazıcı,100lü commit p95/RAM nedeni),uzun snapshot, gerçek soğuk cache ayrı onay bekler. Reclaim default kapalı; bakım/yazma, sorgu RAM/disk ve4R throughput eski açık sınırları [OPEN-WORK](OPEN-WORK.md) içinde kalır. Eski case34 (ishmael /130/1),the zx parçalı okuma, signed-zero ve eski4R uç sorgular ölçüm olmadan kapatılmadı.
