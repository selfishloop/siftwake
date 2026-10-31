# Yazma ve 4R profil devamı — 50k / 29 Ekim 2026

**SONRADAN REVİZE:** kullanıcı bu kapıları önceki sonuçları GÖRDÜKTEN SONRA değiştirdi. Eski kapı ve sonuç tabloları [WRITE-FOLLOWUP](WRITE-FOLLOWUP-50K-20261029.md) içinde aynen korunur. Yeni kararlar ham örneklerden yeniden hesaplandı; gözlemler değiştirilmedi. Bu tur optimizasyon yok; snapshot.mask düzeltmesi uygulanmadı.

Aynı881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12 kaynağın ilk50k belgesi, aynı41 sorgu ve sabit C: FDB/SQLite slotları. Yeni corpus/DB kopyası yok; private/ okunmadı. WAL/FULL akışlı API; SQLite FTS5 trigram/detail=none/columnsize=0/cache2MiB/mmap0/tempFILE. OS cache sıcak/kontrolsüz; gerçek-soğuk deneyi yapılmadı. Yüzdelikler nearest-rank ceil(p*n)-1.

## 1. Yeni kapılar: mevcut ham veri

| Kapı | Eski karar | Sonradan revize karar |
|---|---|---|
| Sorgu: idle≤30ms ise loaded≤30ms, aksi halde≤2×idle | KALDI:16/41 | GEÇTİ:41/41 |
| Uzun tekli: p95≤20,p99≤50,p99,9≤100ms; max bilgi | KALDI: yüklü max199,93ms | GEÇTİ: bütün FDB kolları |
| Throughput≥%75 ve SQLite pooled üstü | GEÇTİ | Aynı: FDB%80,11 /SQLite%73,97 |
| Dirty500 strict-open /idle | GEÇTİ | Aynı:291–292ms /4,32–4,44MiB;replay≈35ms |

| Case/sorgu | Idle p95 ms | Loaded p95 ms | Eski sınır/karar | Yeni sınır/karar |
|---|---:|---:|---|---|
| 0: olduğu | 4.671 | 21.040 | 9.342 /KALDI | 30.000 /GEÇTİ |
| 1: büyük | 3.899 | 19.471 | 7.799 /KALDI | 30.000 /GEÇTİ |
| 2: ayrıca | 4.563 | 18.814 | 9.126 /KALDI | 30.000 /GEÇTİ |
| 3: arasında | 5.288 | 28.239 | 10.577 /KALDI | 30.000 /GEÇTİ |
| 4: birlikte | 4.117 | 19.702 | 8.234 /KALDI | 30.000 /GEÇTİ |
| 5: sanat | 4.736 | 18.402 | 9.473 /KALDI | 30.000 /GEÇTİ |
| 6: görenler | 10.711 | 24.716 | 21.423 /KALDI | 30.000 /GEÇTİ |
| 7: millî | 4.549 | 9.527 | 9.099 /KALDI | 30.000 /GEÇTİ |
| 8: ikinci | 4.008 | 16.545 | 8.017 /KALDI | 30.000 /GEÇTİ |
| 9: toplam | 5.505 | 18.587 | 11.011 /KALDI | 30.000 /GEÇTİ |
| 10: isimli | 10.320 | 26.236 | 20.639 /KALDI | 30.000 /GEÇTİ |
| 11: öldürür | 180.978 | 271.021 | 361.956 /GEÇTİ | 361.956 /GEÇTİ |
| 12: ölümü | 5.065 | 25.264 | 10.131 /KALDI | 30.000 /GEÇTİ |
| 13: akarsu | 197.672 | 101.742 | 395.344 /GEÇTİ | 395.344 /GEÇTİ |
| 14: demokratik | 7.843 | 15.399 | 15.686 /GEÇTİ | 30.000 /GEÇTİ |
| 15: çalışmalarıyla | 297.767 | 371.342 | 595.534 /GEÇTİ | 595.534 /GEÇTİ |
| 16: üreticisi | 208.800 | 263.199 | 417.600 /GEÇTİ | 417.600 /GEÇTİ |
| 17: joseph | 3.552 | 9.335 | 7.103 /KALDI | 30.000 /GEÇTİ |
| 18: leyla | 202.538 | 224.675 | 405.077 /GEÇTİ | 405.077 /GEÇTİ |
| 19: protest | 9.299 | 16.043 | 18.598 /GEÇTİ | 30.000 /GEÇTİ |
| 20: furlong | 10.916 | 15.731 | 21.832 /GEÇTİ | 30.000 /GEÇTİ |
| 21: korkaklığa | 40.019 | 64.666 | 80.039 /GEÇTİ | 80.039 /GEÇTİ |
| 22: ormanların | 436.305 | 544.459 | 872.610 /GEÇTİ | 872.610 /GEÇTİ |
| 23: yanıklarda | 668.905 | 471.305 | 1337.810 /GEÇTİ | 1337.810 /GEÇTİ |
| 24: külliyatı | 291.615 | 382.422 | 583.229 /GEÇTİ | 583.229 /GEÇTİ |
| 25: timmy | 160.191 | 161.663 | 320.383 /GEÇTİ | 320.383 /GEÇTİ |
| 26: yüklük | 121.368 | 228.590 | 242.736 /GEÇTİ | 242.736 /GEÇTİ |
| 27: ilçesine köyleri | 9.286 | 18.046 | 18.572 /GEÇTİ | 30.000 /GEÇTİ |
| 28: geldiği buldu | 250.138 | 254.050 | 500.276 /GEÇTİ | 500.276 /GEÇTİ |
| 29: chicago studio | 10.882 | 19.071 | 21.765 /GEÇTİ | 30.000 /GEÇTİ |
| 30: kısım dişli | 412.005 | 557.514 | 824.010 /GEÇTİ | 824.010 /GEÇTİ |
| 31: güzeli zafer | 183.123 | 260.057 | 366.246 /GEÇTİ | 366.246 /GEÇTİ |
| 32: İnternetten hazinem | 166.448 | 308.033 | 332.896 /GEÇTİ | 332.896 /GEÇTİ |
| 33: oyuncudur atlanta | 203.737 | 307.957 | 407.475 /GEÇTİ | 407.475 /GEÇTİ |
| 34: kendisine çıkacağını | 509.680 | 396.502 | 1019.360 /GEÇTİ | 1019.360 /GEÇTİ |
| 35: ve | 2.189 | 4.447 | 4.377 /KALDI | 30.000 /GEÇTİ |
| 36: if | 3.800 | 6.103 | 7.600 /GEÇTİ | 30.000 /GEÇTİ |
| 37: id | 2.590 | 4.567 | 5.180 /GEÇTİ | 30.000 /GEÇTİ |
| 38: zx | 677.224 | 759.710 | 1354.448 /GEÇTİ | 1354.448 /GEÇTİ |
| 39: qzxkvwnever | 0.969 | 4.143 | 2.969 /KALDI | 30.000 /GEÇTİ |
| 40: transubstantiationz | 4.285 | 16.452 | 8.570 /KALDI | 30.000 /GEÇTİ |

| Motor/kol | n | p95 ms | p99 ms | p99,9 ms | Max ms (bilgi) | >100ms adet /oran | Yeni FDB kapısı |
|---|---:|---:|---:|---:|---:|---|---|
| fdb/pair0-control/control | 16384 | 3.450 | 11.703 | 44.407 | 172.378 | 3 /%0.01831 | GEÇTİ |
| sqlite/pair0-control/control | 16384 | 7.617 | 21.925 | 103.130 | 208.019 | 17 /%0.10376 | referans |
| sqlite/pair0-loaded/loaded | 16384 | 10.860 | 23.908 | 140.793 | 358.869 | 39 /%0.23804 | referans |
| fdb/pair0-loaded/loaded | 16384 | 4.550 | 15.119 | 43.325 | 57.974 | 0 /%0.00000 | GEÇTİ |
| sqlite/pair1-loaded/loaded | 16384 | 10.504 | 23.177 | 140.735 | 328.481 | 37 /%0.22583 | referans |
| fdb/pair1-loaded/loaded | 16384 | 4.377 | 14.547 | 44.534 | 199.929 | 4 /%0.02441 | GEÇTİ |
| fdb/pair1-control/control | 16384 | 3.430 | 11.457 | 42.415 | 69.900 | 0 /%0.00000 | GEÇTİ |
| sqlite/pair1-control/control | 16384 | 7.411 | 19.945 | 94.886 | 197.937 | 14 /%0.08545 | referans |
| fdb/pooled/control | 32768 | 3.438 | 11.463 | 43.681 | 172.378 | 3 /%0.00916 | GEÇTİ |
| sqlite/pooled/control | 32768 | 7.513 | 20.902 | 97.633 | 208.019 | 31 /%0.09460 | referans |
| sqlite/pooled/loaded | 32768 | 10.700 | 23.527 | 140.793 | 358.869 | 76 /%0.23193 | referans |
| fdb/pooled/loaded | 32768 | 4.446 | 14.942 | 44.299 | 199.929 | 4 /%0.01221 | GEÇTİ |

SQLite’a kabul/max kapısı uygulanmadı. Yeni uzun-seri kuralı kısa dört-tipli tekli testin 20/50/max100ms kararını değiştirmez. Kısa testin74,840ms ucunun nedeni açık. [Yeniden hesaplanan bütün kararlar + ham SHA’lar](benchmarks/write50-revised-20261029-v1-gate.json).

## 2. Makine, thread sayısı ve oracle

AMD Ryzen7 7840HS:8 fiziksel,16 mantıksal çekirdek (CIM ve Python os.cpu_count doğrulaması). Dört reader thread + writer/main + bellek örnekleyici ve etkin engine worker’ları bulunur. Hibrit dört işçiye kadar çıkar fakat süreç genelinde toplam DÖRT permit vardır; bir işçi çağıran reader’dır. Dolayısıyla4×4 bağımsız ek işçi varsayımı geçersizdir. Yeni dar4R koşusunda ToolHelp ile50ms aralıkla ölçülen process thread tepesi normal10–11,trace11; writer ve engine worker’ları da dahildir. Bu örneklenen alt sınırdır; kısa yaşayan thread kaçabilir. Eski uzun koşuda thread tepesi kaydedilmemişti, ona sonradan kesin thread sayısı atamıyoruz.

Oracle ESKİ idle1R ve idle4R kayıtlarında da aktiftir: dönen hit payload/version/sequence kontrolü her sorguda yapılır. O yüzden idle tabanını yeniden üretmek gerekmedi. Idle’da değişen commit grubu yoktur; yüklü durumda son yayımlanmış gruba ek oracle uygulanır. Oracle arama intervalinin dışında, aynı proseste CPU/I/O ve pin ömrüne dahildir. Yeni bütün deneyler de oracle’lıdır. Model/katalog/history dahil process RAM, saf motor RAM’i diye sunulmaz.

## 3. Uzun commit profili

Profil açık16384 commit: p95=3.881,p99=13.145,p99,9=49.340,max=219.015ms; ops/sn=344.891. Trace dropped=0. Karşılaştırma önceki profil kapalı kontrol387,18ops/sn; bu aynı-anda kontrol değildir, ortam ve worker hazır olma zamanları değişebilir. Profil açık/kapalı farkından optimizasyon kazancı veya saf instrumentation overhead yüzdesi çıkarmıyoruz.

| >50ms sequence | Commit ms | Thread CPU ms | Prefix poll | Merge poll | Stage | WAL auth /tail | Worker drain | Publish aktif | Mask (görünür seq) |
|---|---:|---:|---:|---:|---:|---|---:|---:|---|
| 1465 | 79.318 | 46.875 | 0.001 | 76.335 | 1.147 | 0.539/0.570 | 0.000 | 0.290 | 33.444(1464);0.106(1465) |
| 3497 | 50.649 | 46.875 | 0.000 | 47.933 | 1.058 | 0.485/0.476 | 0.000 | 0.277 | 37.374(3496);0.108(3497) |
| 3873 | 50.230 | 46.875 | 0.000 | 44.438 | 1.334 | 0.699/0.632 | 0.000 | 0.371 | 28.737(3872);0.129(3873) |
| 4517 | 52.322 | 46.875 | 0.001 | 48.075 | 1.608 | 1.019/0.736 | 0.000 | 0.274 | 34.330(4516);0.110(4517) |
| 6362 | 194.929 | 0.000 | 192.596 | 0.001 | 0.975 | 0.640/0.000 | 0.000 | 0.160 | 1.388(6361);0.076(6362) |
| 7630 | 55.788 | 62.500 | 0.000 | 52.802 | 0.990 | 0.681/0.518 | 0.000 | 0.314 | 40.084(7629);0.124(7630) |
| 10063 | 53.812 | 46.875 | 0.001 | 50.816 | 1.108 | 0.652/0.520 | 0.000 | 0.322 | 35.549(10062);0.116(10063) |
| 10709 | 64.139 | 46.875 | 0.000 | 47.683 | 13.685 | 1.301/0.595 | 0.000 | 0.295 | 32.654(10708);0.132(10709) |
| 11010 | 148.133 | 15.625 | 146.104 | 0.001 | 0.838 | 0.587/0.000 | 0.000 | 0.107 | 0.921(11009);0.038(11010) |
| 12296 | 167.135 | 15.625 | 164.976 | 0.000 | 1.138 | 0.554/0.000 | 0.000 | 0.112 | 1.464(12295);0.052(12296) |
| 12447 | 219.015 | 15.625 | 0.001 | 0.000 | 0.868 | 0.490/0.000 | 0.000 | 0.202 | 0.077(12447) |
| 12750 | 74.784 | 31.250 | 0.001 | 71.167 | 1.198 | 0.817/0.631 | 0.000 | 0.311 | 57.330(12749);0.121(12750) |
| 13152 | 53.417 | 46.875 | 0.001 | 50.231 | 1.136 | 0.685/0.569 | 0.000 | 0.379 | 33.984(13151);0.134(13152) |
| 13815 | 59.836 | 46.875 | 0.000 | 56.578 | 1.295 | 0.684/0.584 | 0.000 | 0.242 | 44.285(13814);0.080(13815) |
| 14826 | 51.677 | 46.875 | 0.001 | 48.262 | 1.073 | 0.946/0.675 | 0.000 | 0.241 | 35.260(14825);0.091(14826) |
| 15855 | 63.214 | 62.500 | 0.001 | 60.525 | 0.988 | 0.534/0.491 | 0.000 | 0.255 | 49.258(15854);0.095(15855) |

En büyük ölçülmüş uçların iç aşamaları:

| Sequence | Commit ms | En büyük iç aşama | Wall ms | Thread CPU ms |
|---|---:|---|---:|---:|
| 12447 | 219.015 | prefix.capture_old_wal_open | 214.858 | 15.625 |
| 6362 | 194.929 | snapshot.wal_open | 184.884 | 0.000 |
| 12296 | 167.135 | snapshot.wal_open | 157.904 | 15.625 |
| 11010 | 148.133 | snapshot.wal_open | 140.364 | 15.625 |

Büyük gecikme bu örneklerde handle açma çağrısının wall süresindedir. File::open içindeki dosya sistemi/filter/scheduler ayrımı ölçülmedi; antivirüs gibi bir alt neden kanıtlanmadı. Aynı genlikte yeni uçlar görülmesi eski uçlarla aynı neden olduklarını kanıtlamaz.

Aşama süreleri wall-time; mask/publish/poll iç içedir, sütunlar toplanmaz. WAL authoritative ve tail append süreleri yazma+sync içerir; saf fsync değildir. Manifest/prefix/install/join ayrımı sequence bazlı ham trace’te vardır. GetThreadTimes CPU saati kısa intervalde≈15,625ms kuantalanır. Commit−threadCPU; I/O, kilit, deschedule ve CPU sayaç hatasını içerir. ETW context-switch kaydı alınmadı: saf scheduler payı AYRI ÖLÇÜLMEDİ, kalan süre scheduler diye etiketlenmedi.

İlk profil pilotunda WAL/stage/poll trace’i eksikti; snapshot mask30–33ms görüldü, 53,75–63,05ms üç uç vardı. Bu eksik koşu silinmedi. İstenen aşama dağılımı için yukarıdaki ayrı, tamamlanmış16k koşu eklendi. Tarihsel74,84ms ve125–200ms uçlarla aynı neden olduğu kanıtlanmadı: **BİLİNMİYOR**. snapshot.mask düzeltmesi yapılmadı. [Tam aşamalı profil](benchmarks/write50-revised-20261029-v2-control-design.json) · [>50ms bütün sequence’ler](benchmarks/write50-revised-20261029-v2-control-slow-detail.json) · [Eksik ilk pilot](benchmarks/write50-revised-20261029-v1-control-design.json).

## 4. Dar arama ve worker deneyi

Aynı dört hızlı case,4096 update/kol; normal/trace/trace/normal sırası. Her kol kontrol edilmiş setup+maintenance ile başlar; donor/query ID kümeleri aynı, sequence/yerleşim tam aynı değildir. Yeni dar4R eski41-case yükünün yerini almaz; atıf deneyi ve instrumentation etkisi için ayrıca raporlanır.

| Case | Normal p95 ms | Trace p95 ms | Δms /% | Trace ort. CPU ms/çağrı | Ort. Mcycles/çağrı |
|---|---:|---:|---|---:|---:|
| 0: olduğu | 20.497 | 20.877 | +0.380 /+1.85% | 10.055 | 37.461 |
| 3: arasında | 25.640 | 25.597 | -0.043 /-0.17% | 12.400 | 45.801 |
| 39: qzxkvwnever | 1.898 | 2.123 | +0.225 /+11.86% | 0.624 | 1.178 |
| 40: transubstantiationz | 9.890 | 10.989 | +1.098 /+11.10% | 1.652 | 8.003 |

| P95 çağrı case/sequence | Engine ms | Delta tarama | Posting hazırlık | Intersection | Cache lock | Permit | Mask | Thread CPU ms |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 0/22526 | 20.877 | 16.654 | 0.461 | 0.529 | 0.014 | 0.000 | 0.010 | 15.625 |
| 3/24490 | 25.597 | 21.383 | 0.452 | 0.968 | 0.006 | 0.000 | 0.026 | 15.625 |
| 39/26041 | 2.123 | 0.043 | 1.960 | 0.000 | 1.768 | 0.000 | 0.013 | 0.000 |
| 40/23646 | 10.989 | 0.029 | 9.883 | 0.870 | 4.111 | 0.000 | 0.010 | 0.000 |

Aynı çağrı içindeki nested wall ölçüleri; cache lock posting hazırlığının içinde olabilir, paylar toplanmaz. Case0/3: aktif delta kayıtlarının get/decode/fold/contains taraması baskın (first.delta_filter); bu örneklerde≈%80. Case39/40: posting hazırlığı ve ortak cache mutex beklemesi öne çıkıyor. CPU ortalaması bütün request thread’lerinin GetThreadTimes farkı toplamıdır; yeni/kısa worker thread’lerinde kuantalama sapması olabilir. Mcycles QueryThreadCycleTime’dan ölçüldü, ms’ye çevrilmedi. Gerçek fiziksel OS cache miss/hit ve saf scheduler payı ayrıştırılmadı. Dar trace’te case40 p95’ine≈1,10ms/+%11 instrumentation/yük farkı geldi; profil bozulmasız sayılmadı.

### Aynı eski tabanda1R ve4R: FDB ve SQLite

| Motor/sorgu | Idle1R p95 ms | Idle4R p95 ms | Fark /oran |
|---|---:|---:|---|
| fdb/timmy | 18.066 | 160.191 | +142.125ms /8.87× |
| sqlite/timmy | 24.251 | 31.441 | +7.190ms /1.30× |
| fdb/güzeli zafer | 36.802 | 183.123 | +146.321ms /4.98× |
| sqlite/güzeli zafer | 37.017 | 54.526 | +17.510ms /1.47× |
| fdb/oyuncudur atlanta | 62.803 | 203.737 | +140.935ms /3.24× |
| sqlite/oyuncudur atlanta | 70.738 | 110.134 | +39.395ms /1.56× |
| fdb/İnternetten hazinem | 74.100 | 166.448 | +92.348ms /2.25× |
| sqlite/İnternetten hazinem | 132.188 | 190.830 | +58.642ms /1.44× |

### Ayrı temiz, sabit yerleşimde yalnız ölçüm: hibrit tavan1/2/4

Tek layout/model12 kol boyunca değişmedi; iki ters sıra. Diagnostic limit yalnız çağıran thread’de etkin, global4 permit/bütçeler/CRC aynı. Eski tabanın mutlak süreleriyle birebir karşılaştırılmaz. n=16/case/ayar1R,64/case/ayar4R. Her sorgunun CPU’su oracle dışında capture edilir.

| İşçi/sorgu | 1R p95 ms | 4R p95 ms | 1R/4R ort. CPU ms | 1R/4R ort. Mcycles | 4R permit p95 ms |
|---|---:|---:|---|---|---:|
| 4/timmy | 18.495 | 33.852 | 11.719/13.184 | 47.349/51.235 | 17.673 |
| 4/güzeli zafer | 34.752 | 51.501 | 27.344/27.588 | 106.820/107.064 | 20.662 |
| 4/oyuncudur atlanta | 52.153 | 71.236 | 56.641/56.885 | 207.857/215.783 | 20.642 |
| 4/İnternetten hazinem | 62.474 | 84.194 | 74.219/75.684 | 277.551/289.492 | 10.495 |
| 2/timmy | 21.582 | 25.451 | 10.742/13.428 | 49.288/48.964 | 11.409 |
| 2/güzeli zafer | 40.715 | 43.978 | 26.367/28.320 | 104.657/103.989 | 15.707 |
| 2/oyuncudur atlanta | 50.409 | 60.888 | 56.641/52.734 | 206.388/209.596 | 9.881 |
| 2/İnternetten hazinem | 93.576 | 80.222 | 77.148/75.195 | 281.934/279.551 | 7.446 |
| 1/timmy | 12.850 | 20.341 | 10.742/12.207 | 44.444/51.831 | 0.000 |
| 1/güzeli zafer | 26.927 | 41.264 | 26.367/30.029 | 97.379/109.443 | 0.000 |
| 1/oyuncudur atlanta | 52.465 | 83.944 | 50.781/59.814 | 195.354/222.187 | 0.000 |
| 1/İnternetten hazinem | 73.253 | 114.017 | 69.336/77.881 | 264.702/299.762 | 0.000 |

Gözlenen thread sayısı16 logical core’u aşmadı; “4 reader×4 ayrı işçi=16+thread aşırı aboneliği” açıklaması desteklenmedi. Global permit ve cache/I/O çekişmesi ayrı etkiler.1–2 işçi bazı sorguları iyileştiriyor, bazılarını kötüleştiriyor (örn.İnternetten hazinem1 işçide4R≈114ms,4 işçide≈84ms). Genel ayar değişikliği önerilmedi/uygulanmadı. ETW olmadığı için kısa deschedule/çekirdek dağılımı tam çözüldü denmiyor. OS thread sayısı runnable thread sayısı değildir; SMT’nin8 fiziksel çekirdekteki ortak kaynak çekişmesi ve dış süreç yükü bu sayaçla dışlanamaz. [Makine ve örneklenen benchmark affinity65535/Normal](benchmarks/write50-revised-20261029-hardware.json).

### Araç hataları ve sınırlar

İlk idle denemesi unsupported CLI mode snapshot-idle-only yüzünden exit1; sorgu ölçülmedi. Tamamlanan dört loaded profil ve cleanup tekrarlanmadı; desteklenen snapshot-one + idle-only env ile ayrı12-kol deneyi tamamlandı. Hata logu korundu. Çalışma model dosyası daha sonra özel query-state dosyasına taşındı; ilk control model dosyası aynı başlangıç modelinden byte-for-byte geri kondu. Kaynak DB/corpus kopyası yapılmadı. [Dar kollar](benchmarks/write50-revised-20261029-v1-queries-design.json) · [Worker deneyi/OS thread örnekleri](benchmarks/write50-revised-20261029-v1-idle-design.json) · [P95 aynı çağrı detayları](benchmarks/write50-revised-20261029-query-selected.json).

## 5. Sıralı sonraki işler

4R+100lü kapısı: **KALDI**. [Bütün ham kapılar](benchmarks/hybrid4r50-20261029-v1-b100-gate.json).

Tek TAMAMLANMIŞ dengeli çift kabul hesabında: FDB kontrol→SQLite kontrol→SQLite yüklü→FDB yüklü. Idle n=12/case, FDB loaded n=46–48/case. İlk çiftte yeni kapı kalınca ek tekrarın başlamış SQLite kolu amaçlı sonlandırıldı; bu kol kabul yüzdeliklerine/throughput’una katılmadı. İlk dört tamamlanmış kolun tamamı oracle’lıdır; kötü sonuçlar çıkarılmadı.

Belge sayısı yine50k; her kol1024 commit×100 update=102400 işlem.100k belge ölçeğine geçilmedi. Sorgu kapısı bu koşudan ÖNCE revize kuralla tanımlandı. Uzun tekli-seri20/50/p99,9 kapısı100’lük transaction’a taşınmadı;100’lük commit p95/max/RAM bilgi amaçlı.

| Motor | Kontrol ops/sn | Yüklü ops/sn | Pooled oran |
|---|---:|---:|---:|
| fdb | 2999.364 | 3004.527 | %100.17 |
| sqlite | 460.193 | 332.169 | %72.18 |

| Case/sorgu | FDB idle p95 | FDB loaded p95 | Sınır ms | Karar | SQLite idle/loaded p95 |
|---|---:|---:|---:|---|---|
| 0:olduğu | 5.970 | 74.364 | 30.000 | KALDI | 645.799/718.550 |
| 1:büyük | 4.295 | 63.915 | 30.000 | KALDI | 589.332/696.962 |
| 2:ayrıca | 4.558 | 58.784 | 30.000 | KALDI | 707.411/769.274 |
| 3:arasında | 5.467 | 84.755 | 30.000 | KALDI | 976.880/1092.184 |
| 4:birlikte | 5.000 | 68.490 | 30.000 | KALDI | 596.307/682.275 |
| 5:sanat | 5.062 | 48.743 | 30.000 | KALDI | 561.059/624.839 |
| 6:görenler | 11.418 | 56.011 | 30.000 | KALDI | 609.158/669.021 |
| 7:millî | 4.198 | 27.392 | 30.000 | GEÇTİ | 164.826/196.954 |
| 8:ikinci | 5.262 | 43.189 | 30.000 | KALDI | 657.975/722.387 |
| 9:toplam | 6.406 | 41.619 | 30.000 | KALDI | 509.300/576.097 |
| 10:isimli | 14.003 | 46.657 | 30.000 | KALDI | 673.857/748.346 |
| 11:öldürür | 117.297 | 123.916 | 234.595 | GEÇTİ | 296.334/336.771 |
| 12:ölümü | 5.856 | 43.681 | 30.000 | KALDI | 556.313/621.078 |
| 13:akarsu | 328.943 | 32.346 | 657.887 | GEÇTİ | 226.476/238.702 |
| 14:demokratik | 7.959 | 38.502 | 30.000 | KALDI | 109.970/127.346 |
| 15:çalışmalarıyla | 377.866 | 256.914 | 755.733 | GEÇTİ | 451.859/502.222 |
| 16:üreticisi | 203.986 | 135.784 | 407.971 | GEÇTİ | 344.584/385.897 |
| 17:joseph | 4.806 | 21.104 | 30.000 | GEÇTİ | 71.919/86.714 |
| 18:leyla | 413.025 | 216.529 | 826.051 | GEÇTİ | 446.068/457.856 |
| 19:protest | 9.736 | 27.265 | 30.000 | GEÇTİ | 170.901/192.412 |
| 20:furlong | 11.269 | 37.005 | 30.000 | KALDI | 13.876/18.990 |
| 21:korkaklığa | 42.093 | 57.705 | 84.187 | GEÇTİ | 48.486/59.522 |
| 22:ormanların | 453.050 | 321.779 | 906.099 | GEÇTİ | 559.468/630.034 |
| 23:yanıklarda | 393.836 | 416.267 | 787.673 | GEÇTİ | 424.656/494.285 |
| 24:külliyatı | 354.656 | 360.885 | 709.313 | GEÇTİ | 358.912/404.815 |
| 25:timmy | 244.976 | 108.202 | 489.952 | GEÇTİ | 31.393/38.896 |
| 26:yüklük | 234.115 | 50.786 | 468.229 | GEÇTİ | 287.442/328.635 |
| 27:ilçesine köyleri | 8.843 | 32.968 | 30.000 | KALDI | 163.677/167.563 |
| 28:geldiği buldu | 429.906 | 175.715 | 859.812 | GEÇTİ | 365.144/400.829 |
| 29:chicago studio | 10.487 | 37.422 | 30.000 | KALDI | 18.018/24.117 |
| 30:kısım dişli | 546.256 | 430.673 | 1092.511 | GEÇTİ | 490.244/553.726 |
| 31:güzeli zafer | 414.347 | 195.853 | 828.695 | GEÇTİ | 54.988/62.171 |
| 32:İnternetten hazinem | 207.547 | 273.644 | 415.093 | GEÇTİ | 185.591/208.400 |
| 33:oyuncudur atlanta | 171.479 | 161.950 | 342.959 | GEÇTİ | 108.238/126.153 |
| 34:kendisine çıkacağını | 364.405 | 349.772 | 728.810 | GEÇTİ | 339.034/388.199 |
| 35:ve | 3.909 | 6.442 | 30.000 | GEÇTİ | 900.663/1047.552 |
| 36:if | 4.657 | 6.960 | 30.000 | GEÇTİ | 1162.090/1291.617 |
| 37:id | 4.847 | 4.886 | 30.000 | GEÇTİ | 1018.639/1131.159 |
| 38:zx | 759.777 | 497.020 | 1519.553 | GEÇTİ | 1106.070/1191.149 |
| 39:qzxkvwnever | 1.502 | 16.072 | 30.000 | GEÇTİ | 0.761/4.627 |
| 40:transubstantiationz | 4.576 | 39.262 | 30.000 | KALDI | 6.918/14.986 |

| Motor/kol | Commit p95/max ms | OS peak private/WS MiB |
|---|---|---|
| fdb/pair0-control | 37.718/2601.025 | 57.961/61.980 |
| sqlite/pair0-control | 368.753/1284.253 | 30.281/33.148 |
| sqlite/pair0-loaded | 455.217/1790.894 | 116.922/122.609 |
| fdb/pair0-loaded | 35.492/2669.326 | 162.406/162.922 |

Yazıcı donörleri mevcut ilk50k’dan aynı ilk1100 uygun kayıt: gerçek metin2048–89681B, ortalama6958,13B. Bu100’lük yük,100 adet200KB belgenin aynı transaction’da değişmesini sınamaz; kısa dört-tipli tekli testte200KB ayrıca vardı. Yeniden açılışta delta1100 anahtar doğrulandı.

Bellek bütün benchmark prosesidir; model/history/oracle dahildir. Yüklü oracle her yeni snapshot’ta son100 kayıtlık commit grubunu ek doğrular. Bu iş arama intervalinin dışında fakat CPU/I/O ve pin ömrüyle yükün içindedir; idle’da böyle değişen grup yoktur. Bu ek stresin etkisi ayrı oracle-off koşusuyla ayrıştırılmadı.

**DURULDU:** bu yeni kapı da kalınca100’lük ayrı aşama profili,500’lük drain devamı ve gerçek Frunner anahtarı tekrarı başlatılmadı. Henüz ölçülmeyen işler bitmiş sayılmadı. Önceki500 worker-drain kanıtı WRITE-PROFILE raporunda duruyor; yeni100 profiline genellenmedi.

Ek SQLite kolunun exit4294967295 çıkışı araç kimliği doğrulanarak stop-on-failure nedeniyle yapılan amaçlı TerminateProcess’tir. Wrapper bunu RuntimeError/worker_exit olarak kaydetti; açıklanamayan çökme veya dayanıklılık testi diye sunulmaz. O SQLite slotu aborted_requires_owner_reset işaretlendi, hazır oracle DB’si sayılmaz. [Amaçlı durma kaydı](benchmarks/hybrid4r50-20261029-v1-b100-intentional-gate-stop.json).

## 6. Doğrulama ve kalan işler

İlk tamamlanan100lü yüklü FDB durumu üzerinde3 strict-open tanıklı kill/reopen,50000 payload/version GET ve eski okuyucu41 exact sorgu geçti. Her kill tanığında delta1100 ve aynı sequence vardı. Canlı kaynak/current manifest ve bütün .fdb/.ready hash’leri aynı kaldı. Append ortası veya elektrik kesintisi iddiası yok. [Kurtarma/eski okuyucu/evre envanteri](benchmarks/write50-revised-20261029-b100-recovery.json).

cargo fmt --check,cargo clippy --all-targets -- -D warnings,diagnostic clippy geçti. cargo test --all-targets:243 geçti,15 mevcut ignored. Diagnostic request-local parent/child izolasyon testi ayrıca geçti. No-default/all-features lib check ve izole SQLite fmt/clippy/test geçti. Kaynak değişiklikleri profiling/timing ve benchmark araçlarıdır; varsayılan motor arama/yazma algoritması ve dosya biçimi değiştirilmedi. Normal kabul koşularında trace kapalıdır. [Default test logu](benchmarks/write50-revised-20261029-v2-test.log) · [Clippy](benchmarks/write50-revised-20261029-v2-clippy.log) · [Diagnostic izolasyon](benchmarks/write50-revised-20261029-v2-request-test.log) · [Son kapsam/ham SHA kontrolü](benchmarks/write50-revised-20261029-final-audit.json).

## 7. Sadece öneri — uygulanmadı

En dar sonraki aday: capped first-page aktif delta taramasını key sırasıyla N eşleşmede durdurmak. Mevcut first_delta bütün shadow’ı dolaşıp her belgeyi get/decode/fold/contains’den geçiriyor. İlkN sonucu isteyen offset0 yolunda, delta içindeki ilkN eşleşmeden sonraki bir delta anahtarı global ilkN’e giremez; önünde zatenN daha küçük delta eşleşmesi vardır. Tam shadow görünürlük maskesi korunur, eski revizyon saklama kuralı değişmez. Exact/serial search aynı kalır. Bu bir ön filtre veya biçim değişikliği değildir. Beklenen ms kazancı henüz ÖLÇÜLMEDİ. 100lü yükte ayrı aşama profili durma kuralıyla yapılmadığından onun kesin kök nedeni diye sunulmaz; adayın dayanağı case0/3 için ölçülmüş delta maliyeti ve delta1100 tanığıdır.

Onay verilirse dar oracle kapsamı: N’in iki yanında delta/source anahtar sırası, güncelleme/silme, full-shadow maskesi, snapshot sequence, cancel ve strict CRC davranışı. Önce aynı100lü kapısı tekrar ölçülür; mask hazırlığına/worker tavanına dokunulmaz. WAL handle açmayı worker’da hazırlama ayrı öneridir; bu tur uygulanmadı. snapshot.mask düzeltmesi ayrı onay bekliyor.

Biçim/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k/dış entegrasyon yok. snapshot.mask düzeltmesi ayrı onay bekliyor;74,84ms/tarihsel125–200ms nedenleri açık. Case34/the zx, signed-zero, RAM/disk, yazma/bakım ve eşzamanlı yük açık listede korunur.
