# Ölü revizyon geri kazanımı — 10k / 25 Ekim 2026

**Adım 1: KALDI.** Sonraki adımların kapısı bu rapordaki bütün alt koşullara bağlıdır.
Snapshot ve küçük commit yolu bu ölçümün kapsamına dahil edilmedi. C ABI uygulanmadı.

**Yayım kararı:** Geri kazanma uygulaması korunur; normal `Database::open` politikasında varsayılan kapalıdır. İsteyen çağıran `set_revision_reclamation(true)` ile açık bakımda etkinleştirir veya doğrudan `start_revision_reclamation()` çağırır. Benchmark adayları açık, kontroller kapalı ölçülmüştür. Arama kabulü geçmeden normal davranışa alınmadı.

| Adım | Durum |
|---|---|
| 1 — Ölü revizyon temizliği | KALDI: ishmael offset130/limit1 birleşik p95 +%9,77; nedeni aşama bazında henüz açıklanmadı |
| 2 — Snapshot / paralel yazıcı | BAŞLATILMADI: Adım1 kapısı bekleniyor |
| 3 — Küçük commit / dirty open | BAŞLATILMADI: önceki kapılar bekleniyor |

## Kabul tablosu

| Kapı | Gözlem | Karar |
|---|---|---|
| 10 tur sonrası retained ≤1,3× | En yüksek 1.0215×; 357.954 MB | GEÇTİ (gözlenen) |
| Tur içi geçici tepe ≤2× canlı | 5ms örneklenen en yüksek tepe / turdaki en düşük görünür payload: 1.9000× | GEÇTİ (gözlenen); payload gerçek canlı DB alanının alt sınırıdır |
| Tek belge görünürlük ≤1s | Akışlı adayın en kötü üst sınırı 181.024 ms | GEÇTİ (gözlenen) |
| Solo görünürlük p95, kontrolün +%5 sınırı | Sınırı aşan tipler: yok | GEÇTİ (gözlenen) |
| 38 durum pooled p95, kontrolün +%5 sınırı | Birleşik 60 örnekte sınırı aşan 1 / 38; ayrı çiftlerde 5 / 76 | KALDI; bütün artışlar aşağıda |
| Worker sürerken tek belge görünürlük ≤1s | 804.227 ms (tek gözlem; p95 değildir) | GEÇTİ (gözlenen) |
| Oracle / process kill | 90 tamamlanan tur; beş dışarıdan gözlenmiş kill/reopen | GEÇTİ (gözlenen) |

## Deney ve sınırlar

Aynı mevcut ilk 10.000 belge; corpus üretilmedi/indirilmedi. Gövdeler önceki churn iş yükündeki aynı donör ve UTF-8 kesme/revizyon marker kurallarını kullanır; title/fields/links donörden gelir. Her tur 1000 güncelleme + 500 silme + 500 ekleme, 100'lük akışlı commitler ve tam bakım. Canlı belge sayısı 10k kalır.
Aynı executable içinde kontrol yalnız geri kazanma politikasını kapatır. İki dengeli sıra: kontrol→aday→SQLite, SQLite→aday→kontrol. Her biri aynı 10 turda sorgu ve tek belge kıyasına girer. Ayrı sıfırlanan aday slotunda 30 tur sürekli kullanım koşusu yapılır; bu ek koşu SQLite ile eşlenmiş 30 tur kıyası değildir.
Yalnız iki sabit DB slotu kullanılır; reset sahiplik/reparse kontrolleri olan benchmark-workspace.ps1 üzerinden. DB kopyası tutulmaz. Corpus ve 0.1 fixture SHA değerleri önce/sonra eşit.
FDB strict/synced. SQLite FTS5 trigram/detail=none, FULL WAL, 2MiB cache, mmap=0, temp_store=FILE, statement cache=64. Owned etiketi SQLite'ta yine tek belge beslemedir; caller Vec RAM simetrisi yoktur. OS cache sıcak; soğuk-cache/güç kesintisi iddiası yok. Arka plan/power izolasyonu yok.
Commit toplamlarına adapter'ın corpus CRC/JSON ve belge hazırlaması dahildir; oracle süreleri commit toplamına dahil değildir. Görünürlük commit başlangıcı→get/tam payload+eski/yeni marker doğrulaması bitiş üst sınırıdır. Sorgular 30 örnek/durum; nearest-rank p95, iki çalışma güven aralığı değildir. RAM private ve WS ayrı OS sayaçlarıdır; 10ms faz örneklemesi kısa sıçramayı kaçırabilir.
Disk bütün çalışma alanı dosyalarıyla 5ms örneklenir; kısa ömürlü dosyalar kaçabilir. Geçici kapı, modeldeki her 100'lük commit sınırında bağımsız OFF1 boyut formülüyle hesaplanan en düşük görünür payload toplamına karşı sınanır. Metadata/posting/dizin alanını dışarıda bırakan bu alt sınır, 2× gerçek canlı alan koşulundan daha sıkıdır; kaynak liveness toplamlarıyla her tur eşleştirilir.

## Teşhis ve politika

Eski seçim bitişik 2–4 segment, ≤100k kayıt, ≤64MiB toplam kaynak ve en büyük/en küçük kayıt oranı ≤4 koşullarını uygular. Otomatik seçim ayrıca dört benzer input ister. İlk beş ~53–54MB kaynağın ikili toplamı bile 64MiB'ı aşar. Seçim ölü byte oranını hiç dikkate almaz.
Kontrolün 10. turunda ilk beş segmentin payload'ının %100'ü ölüdür; diğer segmentlerde de büyük miktarda ölü payload vardır. Bu gerçek encoded payload uzunluğu hesabıdır; posting/dictionary alanının canlı/ölüye tam fiziksel dağılımı ölçülmedi. Segment başına kaynak/indeks, payload, ölü payload, kayıt ve tombstone sayıları CSV'dedir.
Yeni açık bakım önce mevcut boyut katmanı merge'lerini, ardından ≥%20 ve ≥64KiB ölü payload taşıyan tek segmenti boşaltır. Job başına kaynak ≤256MiB, ID map ≤1MiB, tek worker; eski kaynak/builder admission havuzları korunur. Kaynak kopyası 1MiB parçalarla 32MiB/s'ye pace edilir; kaynak doğrulama okuması ve türetilmiş indeks I/O'su bant genişliği sınırlı değildir, tek segment işiyle sınırlıdır.
Yalnız daha yeni immutable segmentte eş anahtarı bulunan kayıtlar atılır. Aktif/WAL delta'sı silmeye yetki vermez. En yeni tombstone tutulur; eski put canlanamaz. Sıra, payload version ve mevcut codec korunur, posting ID'leri filtrelenerek yeniden eşlenir. Tam kaynak CRC ve indeks doğrulaması çıktı yayımından önce geçer.
En yeni tombstone'lar bu prototipte koşulsuz tutulur. 30 turdaki plato gözlemi süresiz sabit disk üst sınırı kanıtı değildir; silinen anahtar metadata'sının uzun vadeli geri kazanımı ayrıca açıktır.
Worker eski kaynakları değiştirmez. Yayıma kadar okuma eski tutarlı segmenti kullanır; install mevcut atomik manifest yolunu ve Weak/pin emeklilik mantığını kullanır. Planlama paged metadata tarar; otomatik foreground yazma yoluna bu tarama eklenmedi. Uygulama açık bakım çağırınca arka plan işi başlar. Aynı &mut Database'e paralel yazıcı/snapshot sözleşmesi Adım 2'dir; bu değişiklik onu çözmüş sayılmaz.


Kontrol run0/tur10 segment teşhisi:

| Source ID | Kaynak/indeks MB | Ölü payload MB / % | Kayıt/görünür/tombstone |
|---:|---|---|---|
| 10 | 53.800/16.507 | 53.790 / %100.00 | 2000/0/0 |
| 20 | 53.025/16.360 | 53.014 / %100.00 | 2000/0/0 |
| 30 | 52.969/16.358 | 52.959 / %100.00 | 2000/0/0 |
| 40 | 54.141/16.468 | 54.131 / %100.00 | 2000/0/0 |
| 50 | 54.235/16.547 | 54.225 / %100.00 | 2000/0/0 |
| 104 | 39.561/12.416 | 21.212 / %53.65 | 2000/1183/500 |
| 158 | 39.547/12.281 | 21.164 / %53.55 | 2000/1183/500 |
| 212 | 40.000/12.369 | 21.924 / %54.85 | 2000/1183/500 |
| 266 | 39.676/12.347 | 21.748 / %54.85 | 2000/1183/500 |
| 320 | 40.681/12.434 | 12.519 / %30.79 | 2000/1536/500 |
| 374 | 40.909/12.473 | 8.837 / %21.62 | 2000/1683/500 |
| 428 | 40.810/12.534 | 8.742 / %21.43 | 2000/1683/500 |
| 482 | 40.659/12.550 | 8.533 / %21.00 | 2000/1683/500 |
| 536 | 39.973/12.504 | 8.304 / %20.79 | 2000/1683/500 |
| 590 | 39.739/12.472 | 0.000 / %0.00 | 2000/2000/500 |

Bu 15 segmentte **bütün bitişik ikili kaynak toplamları** 79.109–108.376 MB aralığında; hepsi 64MiB (=67,109 MB) sınırını aşıyor. Dolayısıyla bu durumdaki seçim engeli doğrudan byte sınırıdır. Görünür kayıt sayısı en yeni tombstone bariyerlerini de içerir; canlı belge sayısı değildir.

## Tur başına yazma, bakım ve alan

Tam 90 satır ve bellek sayaçları [rounds CSV](benchmarks/revisions-10k-20261025-v2-rounds.csv) içinde. Aşağıda her kontrol/adaya ait bakım sonrası alan, commit toplamı ve bakım ms yer alır; bunlar önceki churn tablosuyla aynı sınırları kullanır.


İki tekrarda 10 turun toplamlarının aritmetik ortalaması (oracle/query zamanı hariç):

| Motor | Commit toplam s | Bakım toplam s | İkisi toplam s |
|---|---:|---:|---:|
| control | 34.734 | 5.617 | 40.351 |
| candidate | 37.461 | 32.415 | 69.876 |
| sqlite | 84.822 | 24.623 | 109.444 |

| Motor/run/tur | Disk MB / oran | Commit toplam ms | Bakım ms | Tepe private/WS MB |
|---|---|---:|---:|---|
| control/0/1 | 402.390 / 1.148× | 3844.5 | 687.7 | 15.184/19.206 |
| control/0/2 | 454.218 / 1.296× | 3720.0 | 622.6 | 15.745/19.751 |
| control/0/3 | 506.587 / 1.446× | 3975.7 | 629.7 | 15.745/19.968 |
| control/0/4 | 558.610 / 1.594× | 3422.7 | 588.7 | 16.609/20.419 |
| control/0/5 | 611.724 / 1.746× | 3338.9 | 538.4 | 17.175/21.455 |
| control/0/6 | 665.107 / 1.898× | 3238.8 | 559.5 | 17.383/21.455 |
| control/0/7 | 718.450 / 2.050× | 3542.5 | 478.1 | 17.388/21.635 |
| control/0/8 | 771.659 / 2.202× | 3301.6 | 466.4 | 17.760/22.544 |
| control/0/9 | 824.136 / 2.352× | 3143.3 | 483.2 | 18.420/23.044 |
| control/0/10 | 876.347 / 2.501× | 3204.7 | 658.5 | 18.600/23.159 |
| control/1/1 | 402.390 / 1.148× | 3300.5 | 686.9 | 14.684/19.063 |
| control/1/2 | 454.218 / 1.296× | 4215.2 | 613.8 | 15.213/19.603 |
| control/1/3 | 506.587 / 1.446× | 4121.3 | 628.7 | 16.265/20.812 |
| control/1/4 | 558.610 / 1.594× | 3507.1 | 593.2 | 16.892/21.496 |
| control/1/5 | 611.724 / 1.746× | 3864.4 | 591.4 | 16.974/21.496 |
| control/1/6 | 665.107 / 1.898× | 3458.0 | 498.1 | 17.412/21.496 |
| control/1/7 | 718.450 / 2.050× | 3126.0 | 474.9 | 17.908/22.565 |
| control/1/8 | 771.659 / 2.202× | 3109.7 | 457.5 | 18.522/22.807 |
| control/1/9 | 824.136 / 2.352× | 3148.8 | 513.0 | 18.522/23.081 |
| control/1/10 | 876.347 / 2.501× | 2883.2 | 464.2 | 20.427/24.982 |
| candidate/0/1 | 349.254 / 0.997× | 3411.8 | 2042.9 | 15.684/19.841 |
| candidate/0/2 | 350.195 / 0.999× | 3820.1 | 3336.1 | 15.684/19.841 |
| candidate/0/3 | 349.942 / 0.999× | 4374.6 | 2907.7 | 15.786/20.353 |
| candidate/0/4 | 350.295 / 1.000× | 3734.4 | 3021.3 | 15.942/20.677 |
| candidate/0/5 | 358.539 / 1.023× | 3688.8 | 2659.0 | 17.129/21.778 |
| candidate/0/6 | 351.207 / 1.002× | 4195.5 | 2711.2 | 17.150/21.864 |
| candidate/0/7 | 350.856 / 1.001× | 3511.7 | 5948.8 | 18.104/22.929 |
| candidate/0/8 | 350.740 / 1.001× | 3999.3 | 2302.2 | 18.829/23.077 |
| candidate/0/9 | 351.511 / 1.003× | 3319.0 | 4131.2 | 18.829/23.155 |
| candidate/0/10 | 357.954 / 1.022× | 3601.0 | 3274.0 | 19.321/23.155 |
| candidate/1/1 | 349.254 / 0.997× | 3826.2 | 2116.3 | 15.225/19.747 |
| candidate/1/2 | 350.195 / 0.999× | 3972.8 | 3302.6 | 15.442/19.747 |
| candidate/1/3 | 349.942 / 0.999× | 4583.5 | 2860.8 | 16.601/20.943 |
| candidate/1/4 | 350.295 / 1.000× | 3748.1 | 2954.3 | 16.601/20.943 |
| candidate/1/5 | 358.539 / 1.023× | 3991.2 | 2605.6 | 16.601/20.943 |
| candidate/1/6 | 351.207 / 1.002× | 3640.3 | 2686.7 | 16.601/20.943 |
| candidate/1/7 | 350.856 / 1.001× | 3481.8 | 6317.6 | 17.072/21.053 |
| candidate/1/8 | 350.740 / 1.001× | 3537.1 | 2201.7 | 17.302/21.938 |
| candidate/1/9 | 351.511 / 1.003× | 3263.2 | 4126.7 | 17.302/22.008 |
| candidate/1/10 | 357.954 / 1.022× | 3222.7 | 3322.5 | 18.764/23.450 |
| sqlite/0/1 | 597.185 / 1.022× | 7703.0 | 1943.2 | 13.152/18.342 |
| sqlite/0/2 | 602.604 / 1.031× | 7776.9 | 1996.8 | 13.152/18.493 |
| sqlite/0/3 | 605.680 / 1.036× | 7768.9 | 2173.6 | 13.513/18.899 |
| sqlite/0/4 | 609.071 / 1.042× | 8051.3 | 2169.4 | 14.537/19.452 |
| sqlite/0/5 | 613.429 / 1.050× | 7867.1 | 2382.5 | 15.229/20.177 |
| sqlite/0/6 | 617.443 / 1.057× | 7935.5 | 2427.1 | 15.229/20.177 |
| sqlite/0/7 | 620.564 / 1.062× | 9246.3 | 2430.5 | 15.229/20.177 |
| sqlite/0/8 | 623.157 / 1.066× | 9915.0 | 2982.7 | 15.229/20.177 |
| sqlite/0/9 | 627.442 / 1.074× | 9253.0 | 2586.0 | 15.229/20.177 |
| sqlite/0/10 | 630.485 / 1.079× | 12576.2 | 4809.6 | 15.229/20.177 |
| sqlite/1/1 | 597.185 / 1.022× | 7903.8 | 1989.5 | 13.369/18.588 |
| sqlite/1/2 | 602.604 / 1.031× | 7955.7 | 2006.8 | 13.369/18.723 |
| sqlite/1/3 | 605.680 / 1.036× | 7976.5 | 2229.1 | 13.746/19.137 |
| sqlite/1/4 | 609.071 / 1.042× | 7960.0 | 2202.9 | 14.553/19.501 |
| sqlite/1/5 | 613.429 / 1.050× | 8131.3 | 2381.8 | 14.553/19.501 |
| sqlite/1/6 | 617.443 / 1.057× | 8365.0 | 2403.1 | 14.553/19.501 |
| sqlite/1/7 | 620.564 / 1.062× | 8316.4 | 2383.8 | 14.553/19.501 |
| sqlite/1/8 | 623.157 / 1.066× | 8507.2 | 2560.7 | 14.553/19.501 |
| sqlite/1/9 | 627.442 / 1.074× | 8045.1 | 2528.7 | 14.815/19.898 |
| sqlite/1/10 | 630.485 / 1.079× | 8388.9 | 2657.3 | 14.815/19.898 |

## Otuz tur

| Tur | Retained MB / oran | Örneklenen tepe MB | Commit / bakım ms |
|---|---|---:|---|
| 1 | 349.254 / 0.997× | 455.872 | 3398.7/2060.5 |
| 2 | 350.195 / 0.999× | 454.011 | 3723.7/3365.9 |
| 3 | 349.942 / 0.999× | 455.939 | 4273.0/2830.9 |
| 4 | 350.295 / 1.000× | 455.528 | 3613.2/2996.5 |
| 5 | 358.539 / 1.023× | 457.984 | 3724.7/2583.2 |
| 6 | 351.207 / 1.002× | 467.040 | 3597.6/2663.7 |
| 7 | 350.856 / 1.001× | 459.238 | 3356.2/6403.7 |
| 8 | 350.740 / 1.001× | 458.349 | 3516.3/2215.6 |
| 9 | 351.511 / 1.003× | 457.127 | 3694.6/4071.2 |
| 10 | 357.954 / 1.022× | 457.830 | 3189.8/3262.3 |
| 11 | 365.731 / 1.044× | 466.076 | 3305.1/3979.8 |
| 12 | 355.538 / 1.015× | 474.229 | 3407.1/5395.6 |
| 13 | 355.954 / 1.016× | 469.462 | 3541.0/5562.7 |
| 14 | 365.397 / 1.043× | 464.607 | 3490.5/3820.8 |
| 15 | 364.842 / 1.041× | 470.565 | 3841.7/3790.8 |
| 16 | 366.041 / 1.045× | 473.074 | 3402.5/2519.9 |
| 17 | 365.823 / 1.044× | 473.585 | 3589.1/3475.4 |
| 18 | 365.359 / 1.043× | 473.814 | 3340.2/2996.7 |
| 19 | 363.983 / 1.039× | 471.225 | 3853.8/4140.9 |
| 20 | 353.431 / 1.009× | 470.387 | 3733.2/2862.3 |
| 21 | 363.671 / 1.038× | 459.084 | 3969.7/3425.9 |
| 22 | 350.833 / 1.001× | 468.323 | 3748.0/2426.9 |
| 23 | 358.986 / 1.024× | 458.798 | 3441.2/4906.9 |
| 24 | 351.437 / 1.003× | 465.449 | 3457.8/4753.3 |
| 25 | 361.956 / 1.033× | 460.216 | 3436.9/4008.5 |
| 26 | 368.761 / 1.052× | 466.515 | 3789.8/3567.1 |
| 27 | 375.357 / 1.071× | 476.106 | 3447.9/3937.2 |
| 28 | 367.286 / 1.048× | 483.678 | 3864.9/3583.7 |
| 29 | 365.101 / 1.042× | 471.924 | 3399.8/3198.5 |
| 30 | 365.633 / 1.043× | 472.194 | 3419.2/1982.1 |

Son 10 tur retained aralığı 350.833–375.357 MB; 30. tur 1.0434×. Bu sonlu gözlem tombstone metadata'sının süresiz üst sınırını kanıtlamaz.

## Tek belge — akışlı esas, owned ayrı

| Motor/API/tip | Commit p50/p95 ms | Görünürlük p95/max ms |
|---|---|---|
| control/streamed/update | 51.374/73.678 | 208.291/233.428 |
| control/streamed/delete | 40.740/125.360 | 270.231/298.564 |
| control/streamed/insert | 53.403/130.981 | 270.678/289.023 |
| control/streamed/mixed | 51.830/70.992 | 220.830/246.529 |
| control/owned/update | 1.141/1.687 | 4.142/4.771 |
| control/owned/delete | 1.107/1.477 | 2.422/2.826 |
| control/owned/insert | 1.065/1.538 | 2.878/3.623 |
| control/owned/mixed | 1.044/1.925 | 3.833/5.574 |
| candidate/streamed/update | 46.053/91.943 | 117.599/133.806 |
| candidate/streamed/delete | 32.393/123.342 | 154.475/170.063 |
| candidate/streamed/insert | 52.252/130.681 | 162.057/181.024 |
| candidate/streamed/mixed | 50.426/97.375 | 126.785/164.704 |
| candidate/owned/update | 1.152/2.016 | 3.039/16.677 |
| candidate/owned/delete | 1.081/1.515 | 1.955/2.447 |
| candidate/owned/insert | 1.103/1.637 | 2.624/2.724 |
| candidate/owned/mixed | 1.022/1.843 | 3.036/3.238 |
| sqlite/streamed/update | 4.989/14.800 | 23.396/56.580 |
| sqlite/streamed/delete | 2.928/14.552 | 17.993/40.523 |
| sqlite/streamed/insert | 3.134/4.814 | 9.582/12.593 |
| sqlite/streamed/mixed | 3.530/8.166 | 15.807/39.154 |
| sqlite/owned/update | 4.940/18.365 | 21.528/41.440 |
| sqlite/owned/delete | 2.543/5.813 | 7.184/13.727 |
| sqlite/owned/insert | 2.683/21.029 | 24.287/27.010 |
| sqlite/owned/mixed | 3.347/7.877 | 13.876/17.976 |

## Bütün sorgu artışları

Sorgular aynı mantıksal 10. tur durumunda karşılaştırılır. 30. tur ek matrisinin mantıksal durumu farklıdır; kontrolün 10. turuyla bir hız oranı verilmez. Tam p50/p95/max, sorgu/offset/limit ve bütün artışlar [queries CSV](benchmarks/revisions-10k-20261025-v2-queries.csv) ve [summary JSON](benchmarks/revisions-10k-20261025-v2-summary.json) içindedir.

| Run/case | Aday p95 ms | Kontrol p95 ms | Fark |
|---|---:|---:|---:|
| 0/15 | 17.853600 | 15.738500 | +%13.44 |
| 0/18 | 1.264600 | 1.164400 | +%8.61 |
| 1/8 | 53.287400 | 49.406100 | +%7.86 |
| 1/20 | 153.489800 | 136.604200 | +%12.36 |
| 1/34 | 7.220900 | 6.061300 | +%19.13 |

Birleşik 60 örnekli p95 artışları:

| Case | Aday p95 ms | Kontrol p95 ms | Fark |
|---|---:|---:|---:|
| 34 | 6.693700 | 6.097700 | +%9.77 |

Ayrı çiftlerde +%5 görülen sorgular iki çiftte aynı yönde tekrar etmedi. Birleşik 60 örnekte kalan case34 (ishmael, offset130/limit1) ise belirtilen sınırı aşıyor. Okuma/doğrulama/normalleştirme/sonuç aşama ayrımı bu tur ölçülmedi; artışı kanıtsız biçimde dalgalanma sayıp kapatmadık. Sonraki iş önce bu sayfanın profilidir. En pahalı ölçülen maliyet ayrıca bakım toplamının 5,77× artmasıdır; CRC, kopyalama/pacing ve indeks yazma payları henüz ayrılmadı.

## Kurtarma, uyumluluk ve doğrulama

Her değişiklikte get/tam payload ve eski-yeni marker, her tur tam canlı/dead get oracle'ı; 10./30. tur matrislerinde toplam/sıra/version/payload. Iterator, flush, merge, yeni revizyon rewrite worker'ı ve cache onarımı aşamalarında dış dosya gözlemi + proses askıya alma/kill kullanılır. Yeniden açmada ack edilmiş gruplar yeni ve tam, yayım öncesi grup eski ve tam kalmalıdır; pending marker oracle ayrıca sınar. Güç kesintisi iddiası yok.
Yeni kalıcı codec/SEG/WAL/manifest sürümü yok. Önceki frozen okuyucu iki aday DB'sinde verify/full payload/matris oracle'ını çalıştırır. Corpus ve 0.1 fixture korunur. Kesik/tek byte bozuk kaynak, pin, eşzamanlı yeni delta ve kaynak CRC binding testleri eklenmiştir. Komut sonuçları ayrı verification JSON/loglarındadır.
İlk kill script'i, rewrite hazırlığında bir geçici `.pending` dosyası listelendikten sonra silinince stat sırasında FileNotFoundError ile durdu. İlk üç gerçek kill/oracle zaten geçmişti. Dizin gözlemcisi bu normal yayım yarışını atlayacak şekilde düzeltildi; başarısız gözlemci logu korundu. Kesintiden sonra tam get/matris ve pending marker oracle yeniden geçti, rewrite ve cache senaryoları ayrı kayıtlarla tamamlandı. Bu harness hatası başarılı bir kill testi diye sayılmadı.
Ayrı background read probunda aynı değişmemiş model üzerinde 38×30 arama yapılır, biten worker yalnız sorgular arasında poll/install edilir. İş pending iken okunan durum kapsamı 38/38; en kötü arama 157.720 ms. Query süresine schedule/poll/install dahil değildir; aynı anda bağımsız foreground writer yoktur, Adım 2 kapısı sayılmaz.
Background write probunda solo tek commit 52.882 ms, başlatılmış rewrite sırasında tek commit 751.524 ms oldu. Bu tek eşlenmemiş belge örneği p95 değildir; commit_streamed önceki job'ı drain ettiği için bakım/yazıcı etkileşimi ayrıca açıktır.
İlk 30 turluk kontrol denemesi 16 tam tur sonrası bilinçli kesildi: 1.195.838.645 byte retained ve artan oracle maliyeti. Raw dosya/SHA ve durdurma gerekçesi korunur; tamamlanmamış tur geçti sayılmaz. Nihai dengeli 10 tur serisi yeniden başlatıldı, sayıların arasından iyi koşu seçilmedi.

## Ham kanıt ve açık işler

- [Tasarım / süreç çıkışları](benchmarks/revisions-10k-20261025-v2-design.json)
- [Her segmentte canlı/ölü payload](benchmarks/revisions-10k-20261025-v2-segments.csv)
- [Tek belge ham örnekleri](benchmarks/revisions-10k-20261025-v2-single.csv)
- [Beş kill/reopen kanıtı](benchmarks/revisions-kill-10k-20261025-summary.json)
- [İlk gözlemci hata kaydı](benchmarks/revisions-verification-20261025.json)
- [Tamamlanan fmt, Clippy ve test çıkışları](benchmarks/revisions-verification-resume-20261025.json)
- [Kesilen kontrol](benchmarks/revisions-10k-20261025-interrupted-control.json)
- [Açık işler](OPEN-WORK.md)

Snapshot/çok okuyucu + yazıcı p95, küçük streamed commit/dirty open RAM hedefleri, genel arama gerilemeleri, yazma/bakım bedeli, soğuk yük ve kill sonrası geçici dosya temizliği yalnız bu turun kanıtı ölçüsünde kapanabilir. C ABI sonraki aşama notudur.
