# Güncelleme/silme churn ve eşzamanlı yük — 25 Ekim 2026

Ölçüm durumu: **ana koşular tamamlandı**. Bu rapor yalnız kayıtlı normal motor ölçümlerinden üretilir; kaynak/WAL/manifest/payload biçimi ve motor algoritması değiştirilmedi.

İlk 10.000 mevcut belge yeniden kullanıldı. Gövde, aynı kaynak havuzundaki başka bir belgeden seçilir; donör belgenin title/fields/links alanları da taşınır, hedef key sabitlenir. Bu nedenle yalnız gövdesi değişip metadata’sı sabit kalan dosya güncellemesinin byte özdeşi değildir. UTF-8 sınırında en fazla 51.140 byte tutulup belge/revizyon işareti eklenir (toplam ≤51.200 byte). Yeni kitap/metin corpus dosyası üretilmedi. Kimlik ve revizyon işaretleri eski metnin görünmezliğini bağımsız sınar. Bu kontrollü marker eklenmiş iş yükü, önceki değişmemiş ingest corpus’un byte özdeşi değildir.

FDB normal strict/synced profil. SQLite FTS5 trigram/detail=none, FULL WAL, 2 MiB sayfa cache, mmap=0, temp_store=FILE, 64 statement cache; update/delete trigger’ları yalnız karşılaştırıcı şemasındadır. SQLite owned etiketi altında da tek belgeyle beslenir; FDB Vec giriş belleğiyle simetrik bir caller tahsisi değildir.

Her bölüm iki dengeli motor sırası kullanır: FDB→SQLite, SQLite→FDB. Yalnız sabit FDB/SQLite slotları sahiplik kontrolüyle yeniden kurulur; okuma ilgili kurulmuş DB’yi kullanır. OS cache sıcak, güç/arka plan izolasyonu yok. İki çalışma istatistiksel güven aralığı veya tüm dosya iş yükleri garantisi değildir.

Gereksinim notu `D:/Vibeprojects/frunner/docs/content-index/CODEX-GEREKSINIM.md` salt okunur okundu (istenen repo içi `docs/CODEX-GEREKSINIM.md` yok). Eski nottaki 50k/100k, derived/20ms ve C ABI işleri bu turun 10k/strict/1s talimatının yerine uygulanmadı.

## Kabul kapıları

| Kapı | Gözlenen FDB sonucu | Karar / kapsam |
|---|---|---|
| Tek belge görünürlük ≤1000 ms | En kötü commit→kontrollü arama üst sınırı 184.919 ms | GEÇTİ (gözlenen solo örnekler); eşzamanlı kuyruk ayrıca |
| Boşta ≤15 MB (=15.000.000 B) | Temiz private/WS 3.740/7.692 MB; dirty private/WS 31.003/34.800 MB | Private: **KALDI**; WS: **KALDI**. Her ikisi geçmeden genel RAM geçti denmez |
| Akışlı indeksleme tepe ≤60 MB (=60.000.000 B) | Private 47.079 MB; WS 45.978 MB | Private: **GEÇTİ (gözlenen)**; WS: **GEÇTİ (gözlenen)**. Harness ve sonuç tahsisleri dahil; genel geçti kararı iki sayacı birlikte gerektirir |
| Eşzamanlı sorgu p95 ≤2× idle | En kötü tüm örnekler muhafazakâr oranı 84464.574× | KALDI; aktif kapsama True |
| 10 tur sonrası disk ≤1,3× | Bakım sonrası oranlar 2.501, 2.501× | KALDI |
| Doğruluk oracle | Başarısız worker sayısı 0; kill ayrı aşağıda | Ana tamamlanan koşular geçti; yalnız ölçülen koşullar |

## Tek belge / toplu işlem gecikmesi

Commit zamanına benchmark adapter içindeki mevcut corpus CRC/JSON okuması ve belge hazırlama da dahildir; yalnız motorun hazır belgeyle çağrı süresi değildir. Her homojen commit tipi ayrı ölçülür; mixed satırın süresi her işlem tipine pay edilmez. Mixed n=1’de tipler çağrılar arasında 50/25/25; n=10’da delete/insert yuvarlaması karşıt çağrılarda dengelenir. Her büyüklük grubu bakımı bitmiş 10k mantıksal durumdan başlar; tipler o grup içinde sıralı çalışır ve önceki tipten worker/delta kalabilir. Önceki büyüklüklerde gövdeler güncellenmiş olabilir. Homojen silme/eklemeden sonra canlı sayı ölçüm dışı geri yüklenir. Bu restore süresi latency satırına dahil değildir.

Her turda tip/batch başına n=32/12/6/3, iki tur birleşince n=64/24/12/6 olur. Nearest-rank p95 kullanılır; n<20 satırlarda p95 maksimuma eşittir. Bu az örnekler bir nüfus p95 güven aralığı değildir.

Görünürlük üst sınırı commit başlangıcından sonra get/tam payload, eski marker ve yeni marker sorgularının bitimine kadardır. Tek belgedeki 1s kapısı için muhafazakâr üst sınırdır; büyük batch’te ardışık oracle sorguları biriktiği için son belgenin gerçek ilk görünme anını ayırmaz. Flush/merge bekleme çağrısı eklenmez. **commit_streamed kendisi önceki worker’ı bekler ve başarılı çağrı sonunda delta=0 bırakır**; bu gerçek API maliyetidir.

| Motor/API | Tip | Batch | n | Commit p50/p95/max ms | Görünürlük üst p95/max ms | Red |
|---|---|---:|---:|---|---|---|
| fdb/owned | delete | 1 | 64 | 1.137/1.594/1.989 | 1.676/2.085 | 0 |
| sqlite/owned | delete | 1 | 64 | 2.710/10.927/12.558 | 14.458/16.144 | 0 |
| fdb/owned | insert | 1 | 64 | 1.004/1.599/6.555 | 1.752/6.706 | 0 |
| sqlite/owned | insert | 1 | 64 | 2.790/8.893/21.633 | 15.082/26.630 | 0 |
| fdb/owned | mixed | 1 | 64 | 1.031/1.897/10.185 | 2.122/10.838 | 0 |
| sqlite/owned | mixed | 1 | 64 | 3.484/9.382/12.645 | 18.249/24.671 | 0 |
| fdb/owned | update | 1 | 64 | 1.176/1.780/5.184 | 2.256/12.865 | 0 |
| sqlite/owned | update | 1 | 64 | 4.730/8.753/32.380 | 27.410/39.688 | 0 |
| fdb/owned | delete | 10 | 24 | 5.946/7.475/7.847 | 9.296/9.531 | 0 |
| sqlite/owned | delete | 10 | 24 | 28.179/45.635/50.213 | 106.195/110.343 | 0 |
| fdb/owned | insert | 10 | 24 | 6.984/8.159/8.385 | 10.564/11.219 | 0 |
| sqlite/owned | insert | 10 | 24 | 35.223/46.099/48.422 | 136.109/139.046 | 0 |
| fdb/owned | mixed | 10 | 24 | 6.115/9.428/10.453 | 13.350/18.639 | 0 |
| sqlite/owned | mixed | 10 | 24 | 41.954/166.416/168.287 | 269.587/273.048 | 0 |
| fdb/owned | update | 10 | 24 | 6.624/7.438/7.668 | 13.003/13.472 | 0 |
| sqlite/owned | update | 10 | 24 | 52.382/69.783/78.178 | 201.943/229.683 | 0 |
| fdb/owned | delete | 100 | 12 | 91.357/108.152/108.152 | 220.582/220.582 | 0 |
| sqlite/owned | delete | 100 | 12 | 293.931/333.724/333.724 | 1300.387/1300.387 | 0 |
| fdb/owned | insert | 100 | 12 | 87.858/110.275/110.275 | 229.013/229.013 | 0 |
| sqlite/owned | insert | 100 | 12 | 413.527/723.449/723.449 | 1779.099/1779.099 | 0 |
| fdb/owned | mixed | 100 | 12 | 75.447/84.093/84.093 | 304.759/304.759 | 0 |
| sqlite/owned | mixed | 100 | 12 | 508.814/657.281/657.281 | 2147.603/2147.603 | 0 |
| fdb/owned | update | 100 | 12 | 71.891/104.934/104.934 | 323.606/323.606 | 0 |
| sqlite/owned | update | 100 | 12 | 587.396/822.019/822.019 | 2382.683/2382.683 | 0 |
| fdb/owned | delete | 1000 | 6 | 279.799/298.553/298.553 | 5363.928/5363.928 | 0 |
| sqlite/owned | delete | 1000 | 6 | 3421.531/3639.654/3639.654 | 11748.114/11748.114 | 0 |
| fdb/owned | insert | 1000 | 6 | 780.389/1109.824/1109.824 | 3514.215/3514.215 | 0 |
| sqlite/owned | insert | 1000 | 6 | 3763.533/4289.404/4289.404 | 20076.357/20076.357 | 0 |
| fdb/owned | mixed | 1000 | 6 | 818.336/841.108/841.108 | 10246.733/10246.733 | 0 |
| sqlite/owned | mixed | 1000 | 6 | 4870.266/5552.095/5552.095 | 28893.142/28893.142 | 0 |
| fdb/owned | update | 1000 | 6 | 1027.039/1184.339/1184.339 | 6605.846/6605.846 | 0 |
| sqlite/owned | update | 1000 | 6 | 6192.519/6351.598/6351.598 | 25395.581/25395.581 | 0 |
| fdb/streamed | delete | 1 | 64 | 42.663/134.537/161.665 | 135.012/162.065 | 0 |
| sqlite/streamed | delete | 1 | 64 | 3.349/11.277/15.032 | 15.481/18.476 | 0 |
| fdb/streamed | insert | 1 | 64 | 55.905/144.099/184.055 | 144.708/184.919 | 0 |
| sqlite/streamed | insert | 1 | 64 | 2.893/13.226/26.420 | 18.668/32.003 | 0 |
| fdb/streamed | mixed | 1 | 64 | 49.343/135.394/178.003 | 136.749/179.630 | 0 |
| sqlite/streamed | mixed | 1 | 64 | 3.501/9.486/15.716 | 18.308/25.007 | 0 |
| fdb/streamed | update | 1 | 64 | 55.310/143.962/172.118 | 144.727/172.800 | 0 |
| sqlite/streamed | update | 1 | 64 | 5.032/9.057/26.257 | 30.446/33.633 | 0 |
| fdb/streamed | delete | 10 | 24 | 42.914/155.582/172.659 | 157.802/176.229 | 0 |
| sqlite/streamed | delete | 10 | 24 | 22.050/32.313/34.152 | 80.573/82.485 | 0 |
| fdb/streamed | insert | 10 | 24 | 78.084/226.675/243.752 | 232.621/247.227 | 0 |
| sqlite/streamed | insert | 10 | 24 | 30.696/38.418/39.226 | 104.269/108.086 | 0 |
| fdb/streamed | mixed | 10 | 24 | 75.680/235.376/301.150 | 241.948/309.136 | 0 |
| sqlite/streamed | mixed | 10 | 24 | 35.362/118.733/134.491 | 199.969/225.609 | 0 |
| fdb/streamed | update | 10 | 24 | 73.455/248.762/256.089 | 257.562/263.046 | 0 |
| sqlite/streamed | update | 10 | 24 | 41.063/61.355/61.873 | 183.993/185.613 | 0 |
| fdb/streamed | delete | 100 | 12 | 113.346/493.606/493.606 | 515.386/515.386 | 0 |
| sqlite/streamed | delete | 100 | 12 | 272.880/373.156/373.156 | 1385.441/1385.441 | 0 |
| fdb/streamed | insert | 100 | 12 | 180.264/228.772/228.772 | 284.941/284.941 | 0 |
| sqlite/streamed | insert | 100 | 12 | 407.549/898.297/898.297 | 1862.092/1862.092 | 0 |
| fdb/streamed | mixed | 100 | 12 | 190.889/409.919/409.919 | 476.223/476.223 | 0 |
| sqlite/streamed | mixed | 100 | 12 | 476.084/648.388/648.388 | 2185.383/2185.383 | 0 |
| fdb/streamed | update | 100 | 12 | 202.456/445.782/445.782 | 520.100/520.100 | 0 |
| sqlite/streamed | update | 100 | 12 | 531.678/883.306/883.306 | 2583.646/2583.646 | 0 |
| fdb/streamed | delete | 1000 | 6 | 68.983/81.246/81.246 | 579.279/579.279 | 0 |
| sqlite/streamed | delete | 1000 | 6 | 3601.612/4685.707/4685.707 | 15742.318/15742.318 | 0 |
| fdb/streamed | insert | 1000 | 6 | 1494.099/2051.749/2051.749 | 2513.058/2513.058 | 0 |
| sqlite/streamed | insert | 1000 | 6 | 3808.780/4841.319/4841.319 | 24933.407/24933.407 | 0 |
| fdb/streamed | mixed | 1000 | 6 | 1110.778/1316.254/1316.254 | 2342.938/2342.938 | 0 |
| sqlite/streamed | mixed | 1000 | 6 | 4788.395/6499.064/6499.064 | 29245.253/29245.253 | 0 |
| fdb/streamed | update | 1000 | 6 | 1274.619/1512.829/1512.829 | 2130.502/2130.502 | 0 |
| sqlite/streamed | update | 1000 | 6 | 6328.732/8897.751/8897.751 | 33558.103/33558.103 | 0 |

n=100 ve1000’de örnek sayısı özellikle küçüktür; nearest-rank p95 çoğu satırda maksimumdur, güvenilir kuyruk dağılımı iddiası değildir.

## Aktif delta / 38 durum

Akışlı çağrı sonrası delta=0 olduğu açıkça raporlanır; flush edilmemiş delta kapısı owned API ile ayrıca sınanır. Her durum/run 5 zamanlı örnektir; p95=max. Tam payload/sıra/version ve güncel model oracle’ı zaman dışında çalışır.

| Motor/API/run | Gerçek delta keys | En kötü active/idle p95 | >2× durum sayısı |
|---|---:|---:|---:|
| fdb/streamed/0 | 0 | 1.605× | 0/38 |
| sqlite/streamed/0 | None | 3.751× | 3/38 |
| fdb/streamed/1 | 0 | 1.524× | 0/38 |
| sqlite/streamed/1 | None | 12.779× | 2/38 |
| fdb/owned/0 | 100 | 12.174× | 3/38 |
| sqlite/owned/0 | None | 7.148× | 2/38 |
| fdb/owned/1 | 100 | 11.533× | 4/38 |
| sqlite/owned/1 | None | 5.683× | 2/38 |

38 durumun tamamının ham süreleri ve oranları [özet JSON](benchmarks/churn-concurrent-10k-20261024-summary.json) `delta` bölümündedir.

## On tur churn: disk ve bakım

Her tur 1000 güncelleme,500 silme,500 ekleme; canlı10k sabit,100’lük atomik commitler. Bütün değişikliklerde full payload/get ve eski-yeni marker; tur sonunda bütün canlı/dead get oracle’ı geçer. Disk retained bütün çalışma alanı dosyalarıdır. Source/index live sayaçları FDB stats’tan gelir; SQLite ayrımı aynı değildir. Phase peak 10ms örneklemesi kısa sıçramayı kaçırabilir; OS lifetime peak tur içinde kümülatiftir.

| Motor/run/tur | Commit toplam ms | Bakım ms | Retained önce/sonra MB | Canlı kaynak/indeks MB | Phase private/WS MB | OS lifetime peak private/WS MB |
|---|---:|---:|---|---|---|---|
| fdb/0/1 | 3820.9 | 822.9 | 413.87/402.39 | 307.73/94.66 | 14.39/18.36 | 14.61/18.44 |
| fdb/0/2 | 4416.0 | 715.0 | 466.20/454.22 | 347.28/106.94 | 15.51/19.36 | 15.51/19.38 |
| fdb/0/3 | 4666.8 | 747.9 | 519.04/506.59 | 387.28/119.31 | 16.47/20.19 | 16.51/20.19 |
| fdb/0/4 | 4510.9 | 779.0 | 571.91/558.61 | 426.95/131.65 | 16.08/19.94 | 16.51/20.25 |
| fdb/0/5 | 4102.2 | 651.1 | 624.56/611.72 | 467.63/144.09 | 17.08/20.77 | 17.11/20.77 |
| fdb/0/6 | 3944.2 | 612.1 | 677.78/665.11 | 508.54/156.56 | 18.31/21.96 | 18.50/22.57 |
| fdb/0/7 | 3781.3 | 592.1 | 733.81/718.45 | 549.35/169.10 | 19.53/23.18 | 19.91/23.92 |
| fdb/0/8 | 3856.2 | 548.6 | 787.41/771.66 | 590.01/181.65 | 19.14/23.09 | 20.06/24.03 |
| fdb/0/9 | 3644.1 | 548.2 | 839.29/824.14 | 629.99/194.15 | 20.62/24.92 | 20.80/24.92 |
| fdb/0/10 | 3801.0 | 543.4 | 891.71/876.35 | 669.72/206.62 | 20.14/23.89 | 21.26/24.92 |
| sqlite/0/1 | 9488.1 | 1788.9 | 605.52/597.18 | 597.09/0.00 | 13.05/17.54 | 13.05/17.57 |
| sqlite/0/2 | 9714.0 | 1835.7 | 609.88/602.60 | 602.51/0.00 | 12.92/17.82 | 13.20/17.98 |
| sqlite/0/3 | 9760.7 | 1979.3 | 614.16/605.68 | 605.58/0.00 | 12.61/17.40 | 13.74/18.50 |
| sqlite/0/4 | 9825.2 | 1991.0 | 615.99/609.07 | 608.97/0.00 | 12.41/17.29 | 14.52/18.84 |
| sqlite/0/5 | 9911.1 | 2243.7 | 620.99/613.43 | 613.33/0.00 | 13.23/18.14 | 14.52/18.84 |
| sqlite/0/6 | 9822.1 | 2092.3 | 625.43/617.44 | 617.31/0.00 | 13.52/18.16 | 14.52/18.84 |
| sqlite/0/7 | 9969.0 | 2053.2 | 630.24/620.56 | 620.43/0.00 | 12.79/17.72 | 14.63/18.84 |
| sqlite/0/8 | 9780.4 | 2222.6 | 633.45/623.16 | 623.03/0.00 | 13.59/17.92 | 14.66/19.17 |
| sqlite/0/9 | 9834.3 | 2235.8 | 635.68/627.44 | 627.31/0.00 | 13.43/18.36 | 14.66/19.17 |
| sqlite/0/10 | 9840.2 | 2387.7 | 639.02/630.48 | 630.35/0.00 | 13.95/18.92 | 15.35/19.64 |
| fdb/1/1 | 3520.5 | 758.6 | 413.87/402.39 | 307.73/94.66 | 15.37/19.26 | 15.37/19.26 |
| fdb/1/2 | 4213.4 | 724.9 | 466.12/454.22 | 347.28/106.94 | 15.47/19.57 | 15.56/19.58 |
| fdb/1/3 | 4512.6 | 690.7 | 519.44/506.59 | 387.28/119.31 | 15.68/19.58 | 16.48/20.32 |
| fdb/1/4 | 3893.4 | 722.4 | 570.83/558.61 | 426.95/131.65 | 17.44/20.68 | 17.49/21.32 |
| fdb/1/5 | 3760.5 | 625.3 | 624.57/611.72 | 467.63/144.09 | 17.01/21.26 | 17.65/21.93 |
| fdb/1/6 | 4041.6 | 664.3 | 677.68/665.11 | 508.54/156.56 | 17.93/22.12 | 17.99/22.12 |
| fdb/1/7 | 3506.4 | 568.3 | 733.81/718.45 | 549.35/169.10 | 17.80/21.61 | 18.20/22.12 |
| fdb/1/8 | 3387.1 | 547.5 | 787.41/771.66 | 590.01/181.65 | 17.87/21.36 | 18.59/22.22 |
| fdb/1/9 | 3570.8 | 561.5 | 839.29/824.14 | 629.99/194.15 | 18.96/22.58 | 19.05/22.84 |
| fdb/1/10 | 3397.8 | 576.5 | 891.71/876.35 | 669.72/206.62 | 18.31/22.39 | 19.38/23.38 |
| sqlite/1/1 | 9302.1 | 1905.0 | 605.52/597.18 | 597.09/0.00 | 12.91/17.53 | 12.91/17.53 |
| sqlite/1/2 | 9432.9 | 1801.6 | 609.88/602.60 | 602.51/0.00 | 13.21/17.48 | 13.21/18.06 |
| sqlite/1/3 | 10116.3 | 2002.2 | 614.16/605.68 | 605.58/0.00 | 13.16/17.92 | 13.21/18.06 |
| sqlite/1/4 | 9634.0 | 1969.0 | 615.99/609.07 | 608.97/0.00 | 13.42/17.85 | 14.50/18.83 |
| sqlite/1/5 | 9719.5 | 2109.6 | 620.99/613.43 | 613.33/0.00 | 12.80/17.53 | 15.01/19.23 |
| sqlite/1/6 | 9784.4 | 2092.0 | 625.43/617.44 | 617.31/0.00 | 13.07/17.99 | 15.01/19.23 |
| sqlite/1/7 | 9696.4 | 2162.5 | 630.24/620.56 | 620.43/0.00 | 13.07/18.02 | 15.01/19.23 |
| sqlite/1/8 | 9748.4 | 2331.0 | 633.45/623.16 | 623.03/0.00 | 13.60/17.79 | 15.01/19.23 |
| sqlite/1/9 | 9994.8 | 2284.0 | 635.68/627.44 | 627.31/0.00 | 13.94/18.92 | 15.01/19.23 |
| sqlite/1/10 | 9762.4 | 2298.2 | 639.02/630.48 | 630.35/0.00 | 14.47/19.46 | 15.15/19.46 |

### Büyüme eski nesil dosyalarda mı, canlı segmentlerde mi?

| FDB run | Başlangıç→tur10 retained MB / oran | Manifest-live kaynak+indeks MB | Retained−live MB | Segment sayısı |
|---|---|---:|---:|---:|
| 0 | 350.412679→876.347336 / 2.501× | 876.345944 | 0.001392 | 15 |
| 1 | 350.412679→876.347336 / 2.501× | 876.345944 | 0.001392 | 15 |

Retained büyüme ile live sayaç toplamı neredeyse eşitse eski/manifest dışı generation dosyalarının birikmesi açıklaması desteklenmez. Güncel manifestin hâlâ referans verdiği segmentlerde superseded/ölü revizyonların tutulması ve sınırlı merge seçimi aday kök nedendir; bu satır fiziksel ölü-byte miktarını veya hangi seçim koşulunun tek neden olduğunu ayrı ayrı kanıtlamaz. Önceki eski-nesil temizliği kazanımı geri alınmış sayılmaz.

## 5000 değişiklik

Önce tek atomik5000 isteği denenir; kabul reddi gizlenmez. Red sonrası100’lük parçalara bölünen yol ayrıca ölçülür; bu5000 atomik işlem değildir. Red anındaki kontrol get/version üzerinden eski durumu sınar; tam payload/marker kontrolleri başarılı fallback commit’lerinde ve son model doğrulamasında yapılır. Fallback wall süresi oracle işini içerir; salt motor throughput’u diye kullanılmaz. Burst sonrası38-case/5 örnek, delta/segment stats ve bakım ham dosyalardadır.

| Motor/API/run | Aşama | Sonuç / süre | Private / WS peak MB |
|---|---|---|---:|
| fdb/streamed/0 | burst_atomic | commit 855.7 ms; invalid request: streamed transaction index allowance | 15.31/18.98 |
| fdb/streamed/0 | burst_bounded_fallback | wall 148208.0 ms | 16.96/20.04 |
| fdb/streamed/0 | burst_maintenance | wall 180.8 ms | 26.69/30.56 |
| sqlite/streamed/0 | burst_atomic | commit 30881.7 ms; kabul | 15.29/20.31 |
| sqlite/streamed/0 | burst_maintenance | wall 3202.6 ms | 29.27/34.32 |
| fdb/streamed/1 | burst_atomic | commit 785.2 ms; invalid request: streamed transaction index allowance | 15.44/18.92 |
| fdb/streamed/1 | burst_bounded_fallback | wall 147420.3 ms | 16.42/19.90 |
| fdb/streamed/1 | burst_maintenance | wall 176.6 ms | 25.81/29.92 |
| sqlite/streamed/1 | burst_atomic | commit 30924.4 ms; kabul | 15.41/20.50 |
| sqlite/streamed/1 | burst_maintenance | wall 3208.8 ms | 28.98/33.91 |
| fdb/owned/0 | burst_atomic | commit 300.1 ms; invalid request: transaction exceeds 64 MiB | 291.95/230.66 |
| fdb/owned/0 | burst_bounded_fallback | wall 24666.3 ms | 291.95/230.66 |
| fdb/owned/0 | burst_maintenance | wall 1543.8 ms | 291.95/230.66 |
| sqlite/owned/0 | burst_atomic | commit 32164.2 ms; kabul | 15.81/20.79 |
| sqlite/owned/0 | burst_maintenance | wall 3426.1 ms | 28.99/33.96 |
| fdb/owned/1 | burst_atomic | commit 294.2 ms; invalid request: transaction exceeds 64 MiB | 292.02/232.15 |
| fdb/owned/1 | burst_bounded_fallback | wall 24665.3 ms | 292.02/232.15 |
| fdb/owned/1 | burst_maintenance | wall 1504.2 ms | 292.02/232.15 |
| sqlite/owned/1 | burst_atomic | commit 31701.5 ms; kabul | 15.63/20.67 |
| sqlite/owned/1 | burst_maintenance | wall 3379.0 ms | 28.56/33.67 |

Burst sonrasındaki bütün 38 sorgunun p50/p95/max ve örnek sayıları [CSV tablosundadır](benchmarks/churn-burst-search-20261025.csv). Eşlenmiş bir burst-öncesi sorgu ölçümü veya atomik 5000 commit sürerken ayrı okuyucu yoktur; bu veri bakım beklenmeden sonrasındaki mutlak arama süreleridir, nedensel önce/sonra oranı değildir. Aşağıdaki en yavaş durum, her koşunun 38 sorgusu arasından seçilmiştir; 5 örnekte p95=max.

| Motor/API/run | En yüksek burst-sonrası p95 sorgusu | n | p50 / p95 / max ms |
|---|---|---:|---|
| fdb/owned/0 | the zx /0/50 | 5 | 161.917/193.157/193.157 |
| fdb/owned/1 | zx /0/50 | 5 | 161.145/174.542/174.542 |
| fdb/streamed/0 | zx /0/50 | 5 | 169.767/178.005/178.005 |
| fdb/streamed/1 | zx /0/50 | 5 | 169.570/173.059/173.059 |
| sqlite/owned/0 | the zx /0/50 | 5 | 1622.008/1670.623/1670.623 |
| sqlite/owned/1 | zx /0/50 | 5 | 1505.821/1613.376/1613.376 |
| sqlite/streamed/0 | the zx /0/50 | 5 | 1686.694/1710.632/1710.632 |
| sqlite/streamed/1 | the zx /0/50 | 5 | 1570.440/1651.493/1651.493 |

## Eşzamanlı okuma-yazma

Mevcut FDB API aynı Database için &mut yazma ve tek-open kilidi kullanır; snapshot reader API yoktur. Harness iki iş parçacığını aynı dış Mutex ile sıraya alır. SQLite da aynı admission wrapper’ı kullanır; bu SQLite’ın ayrı native WAL connection okuyucularının en iyi olası concurrency hızı değildir. Okuma wall süresi kilit beklemeyi içerir. Ölçülen wait dış Mutex beklemesidir; motor içindeki kilitler ayrı ayrı instrument edilmedi. Writer giriş planı kilit dışındadır; stream iterator’unun corpus okuması commit’in içindedir. Tam payload oracle kilit dışında, tutarlı küçük descriptor capture süresi ayrıca kayıtlıdır; kalan capture kilit maliyeti gerçek motor maliyeti sayılmaz.

Reader38 durumu10 tur döner. Writer en az100 tekli veya10×100 commit; güvenlik üst sınırı4000/200 commit. writer_finished bayrağı sorgu sonunda örneklendiği için kuyrukta beklerken writer biten istekler kesin ayırt edilemez. **Gate bütün okuma örneklerini kullanır**, en kötü kuyruk bu filtreyle gizlenmez. Ayrıca bayrakla ayrılmış alt örnekler ham özette korunur; bunlardan tek başına geçer kararı verilmez. Aktifcase kapsaması ve sayıları ham özet `concurrent.cases` bölümündedir. 10 örnekte p95=max; gate yalnız gözlenen bu örnekler içindir. En büyük kat, boşta yaklaşık 0,0101ms olan kısa negatif sorgudan gelir: 853ms kuyruklu örnek yaklaşık 84 bin kat çıkar. Bu bütün sorguların 84 bin kat yavaşladığı anlamına gelmez; mutlak gecikme ve kilit beklemesi de birlikte verilmiştir.

| Motor/run/batch | Aktif38 kapsama | En kötü p95/idle | Max read ms | Yazma ops/s solo→reader varken | Writer wait p95/max ms | Önceki worker drain / running gözlemi |
|---|---|---:|---:|---|---|---|
| fdb/0/100 | True | 84464.574× | 1364.1 | 333.87→258.94 | 194.102/563.339 | 61/62 |
| sqlite/0/100 | True | 5405.736× | 8605.6 | 154.59→85.34 | 1169.871/3360.214 | 0/0 |
| fdb/0/1 | True | 20781.238× | 316.1 | 12.01→8.46 | 126.326/359.339 | 107/107 |
| sqlite/0/1 | True | 159.497× | 1587.2 | 126.90→2.19 | 1185.391/3098.224 | 0/0 |
| fdb/1/100 | True | 72126.729× | 1219.8 | 348.64→261.04 | 193.188/510.670 | 61/62 |
| sqlite/1/100 | True | 7262.662× | 7745.2 | 156.35→84.75 | 1215.003/3281.936 | 0/0 |
| fdb/1/1 | True | 21633.686× | 251.9 | 13.71→8.90 | 128.689/416.020 | 106/106 |
| sqlite/1/1 | True | 317.648× | 1758.5 | 136.68→2.14 | 1175.447/3508.122 | 0/0 |

Bütün motor/run/batch/sorgu kombinasyonları için boşta ve yükte p50/p95/max, okuyucu kilit bekleme p50/p95/max, aktif örnek sayısı ve oran [tam CSV tablosundadır](benchmarks/churn-concurrent-20261025.csv). Ham özet ayrıca motor arama süresini, writer commit/bekleme dağılımlarını ve model yakalama ek yükünü içerir.

Stream commit önceki compaction_running=true job’ını bekleyip kurar; bu sayacın pozitif olması reader döngüsü boyunca tamamlanmış worker işine kanıttır. Yalnız döngüden sonraki final maintain sayısı bu kapsamı geçirmez. Owned eşzamanlı ölçüm bu tur yapılmadı; owned active delta, latency/burst/open ayrı ölçüldü.

## Process kill ve yeniden açılış

Elektrik kesintisi iddiası yoktur. Iterator ortasındaki50/100 bariyeri WAL publish öncesi deterministiktir. Flush/merge/cache için yeni geçici builder dosyası dışarıdan gözlenir, process tüm thread’leriyle suspend edilip dosya tanığı kaydedilir ve kill edilir. Bu üretim instruction hook’u değildir; phase yakalanamazsa kapsam sonuçsuzdur. Acknowledged gruplar yeni modelin tamamını korumalı; iterator publish öncesi grup eski modelde kalmalıdır, belge belge karma kabul edilmez.

| Aşama | Phase tanığı | Strict reopen + full payload/38 oracle | Eski/yeni marker |
|---|---|---|---|
| iterator | True | exit 0 | {'checks': 150, 'all_passed': True} |
| flush | True | exit 0 | {'checks': 150, 'all_passed': True} |
| merge | True | exit 0 | {'checks': 150, 'all_passed': True} |
| cache | True | exit 0 | {'checks': 150, 'all_passed': True} |

Kurtarma doğruluğunun geçmesi artık dosya temizliğinin de geçtiği anlamına gelmez: sonrasında kayıtlı input/build geçici dosyaları en fazla 1.273 MB kalmıştır. Bu dosyalar silinmedi; temizleme davranışı ayrıca açık tutulur. Bu sayı olası manifest-dışı .fdb alanının ölçüsü değildir. 10 turluk churn disk sonucu kesinti testlerinden önce ölçülmüştür; bu artıklar 2,501× büyümeyi açıklamaz.

## Dirty strict open ve idle bellek

Bakım sonrası500 mixed **owned** değişiklik sync edilip delta flush edilmeden kapanır. Ham dosyada/API sütununda streamed etiketi bulunan churn bölümünde de dirty durum **owned çağrısıyla zorla oluşturulmuştur**; bu stream API’nin doğal delta durumu değildir (stream başarılı çağrı sonunda delta0). Yeni process strict open replay yapar; open_ms yalnız Database::open çağrısıdır.5 saniye sonraki idle ölçümü bounded metadata oracle modelini de içerir. OS lifetime peak açılış+oracle/model işini içerir; ilk opened satırının peak’i open’a kadar olan tepedir.

| Motor/API/run | Dirty open ms | Open peak private/WS MB | 5s idle private/WS MB | Delta keys |
|---|---:|---|---|---:|
| fdb/streamed/0 | 880.50 | 28.69/32.61 | 31.00/34.80 | 500 |
| sqlite/streamed/0 | 1.98 | 0.82/5.22 | 3.94/8.28 | None |
| fdb/streamed/1 | 866.47 | 28.63/32.53 | 30.90/34.67 | 500 |
| sqlite/streamed/1 | 1.60 | 0.83/5.22 | 3.19/7.54 | None |
| fdb/owned/0 | 616.62 | 27.82/31.72 | 30.40/34.14 | 500 |
| sqlite/owned/0 | 1.87 | 0.83/5.22 | 3.81/8.16 | None |
| fdb/owned/1 | 625.53 | 27.94/31.83 | 29.98/33.81 | 500 |
| sqlite/owned/1 | 1.97 | 0.82/5.22 | 3.81/8.16 | None |

## Yorum, sınırlar ve sonraki iş

Churn turu toplam motor süresinde en pahalı gözlem run0/tur3: commit toplamı 4666.8 ms + son bakım 747.9 ms. Bu toplam seri oracle wall zamanını içermez.
En kötü tekil eşzamanlı okuma run0/batch100: **1364.1 ms**; en kötü vaka p95/idle oranı **84464.574×**. Kilit bekleme ve engine-only süreler ham sonuçta ayrıdır.

Öncelik: kullanıcı aramasını bekleten tek mutable handle/admission yolunu ve commit içindeki worker drain/flush beklemesini giderecek okunabilir snapshot tasarımını ayrı prototipte ölçmek. Seçim yukarıdaki tamamlanmış ölçümlere dayanır; bu tur motor optimizasyonu yapılmadı. Disk ve dirty bellek kapıları ayrıca açık tutulur; aktif delta kapısının stream yolunda bulunmaması da ürün API bulgusudur.

MB=1.000.000 byte, MiB=1.048.576 byte.15 MB≈14,305 MiB;60 MB≈57,220 MiB. Private commit fiziksel/toplam RAM değildir; working set paylaşılan image ve dosya sayfalarını içerir. 10k descriptor/model ve sonuç/oracle tahsisleri ölçüm process’indedir; bütün corpus RAM’e alınmaz. Soğuk cache, elektrik kesintisi, çok okuyucu, farklı corpus, üretim Frunner entegrasyonu ölçülmedi. C ABI/DLL, opak handle, akışlı yazma ve panic yakalama yalnız sonraki aşama notudur; uygulanmadı.

[Ham tasarım/komut kimlikleri](benchmarks/churn-concurrent-10k-20261024-all-design.json), [tam özet ve38-case süreleri](benchmarks/churn-concurrent-10k-20261024-summary.json), [process kill kanıtları](benchmarks/churn-kill-10k-20261025-summary.json). Her mode JSONL/process/stderr ve model dosyası aynı prefix altındadır. [Final kontroller ve çıkış kodları](benchmarks/churn-finalizer-20261025.json) root/native fmt, Clippy warnings-as-errors ve iki harness testinin sonuçlarını içerir. [Bağımsız kayıt/sayısal denetim](benchmarks/churn-report-proof-20261025.json) koşu kapsamı, kuantiller, kilit örnekleri, program kimlikleri ve korunan dosyaları kontrol eder. Önceki yazma/bakım, arama regresyonları, RAM/disk ve eşzamanlı/soğuk başlıklar [OPEN-WORK](OPEN-WORK.md) içinde takip edilir.
