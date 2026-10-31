# Yazma devam ölçümleri — 50k / 29 Ekim 2026

Tekli yazma yeni kullanıcı kapısıyla GEÇTİ; [p99 ve eski/yeni kapı karşılaştırması](WRITE-PROFILE-50K-20261029.md).74,84ms ucun nedeni açık. Bu tur optimizasyon yok.

## 1. Dirty500 — GEÇTİ

Aynı881756DA… corpus ilk50k; mevcut sabit FDB/SQLite slotları. İki dengeli sıra FDB/SQLite ve SQLite/FDB. Her kolda temiz bakımı bitmiş DB, ardından ilk500 uygun (2–200KB) donörle beş ayrı100lü streamed commit. Süreçler normal kapanır; bitmiş prefix worker sonucu bu kısa kurulum oturumlarında install edilmez. Her adımda100/200/300/400/500 delta anahtarı doğrudan doğrulandı. Bu,500 değişikliği varsaymak değildir. SQLite kısa bir setup okuma transaction’ı ile checkpoint’i bekletir, sonrasında setup tutucusu kontrollü kill edilir; uzun snapshot stres koşusu değildir.

OS cache sıcak/kontrolsüz; eviction ve doğrulama atlama yok. Idle yeni proseste2sn sonra, corpus/model oracle yüklenmeden. Private ve working set ayrı süreç sayaçlarıdır. Profil kapalı strict-open kabul içindir; replay ayrı diagnostic koşudadır ve CRC/şema/konum/filter hazırlığını içerir. Timer yalnız mevcut diagnostic feature altında eklendi; normal yol ve kalıcı biçim aynı.

| Kol | Motor | İlk open ms | Idle private MiB | Idle WS MiB | Delta anahtarı |
|---|---|---:|---:|---:|---:|
| pair0-dirty-open | fdb | 292.102 | 4.320 | 8.973 | 500 |
| pair0-dirty-open | sqlite | 13.420 | 3.082 | 7.953 | None |
| pair1-dirty-open | sqlite | 13.057 | 3.082 | 7.961 | None |
| pair1-dirty-open | fdb | 290.572 | 4.438 | 9.035 | 500 |

| FDB kapısı | İki örnek | Karar |
|---|---|---|
| Strict-open≤1000ms | [292.1017, 290.572]ms | GEÇTİ |
| Idle private≤15MiB | [4.32, 4.438]MiB | GEÇTİ |
| WAL replay (bilgi amaçlı) | [34.7452, 35.0021]ms | Ayrı ölçüldü |

SQLite source-wide FDB strict-CRC kontratı sunmaz; ayrıca SQLite C iç WAL-replay süresi ayrıştırılmadı. İlk open toplamı referanstır. FDB’de ilk açılış sonrası delta500 ve sequence105 kaldığı doğrulandı. Profil/zamanlama dışında bütün50k payload/version oracle’ı geçti. Üç kez dirty500 strict açılış tanığından sonra process kill, ardından reopen ve eski okuyucu50k GET+41 exact sorgu geçti; source/fixture hashları aynı. Kill append sırasında değil, açılmış dirty WAL üzerindedir; elektrik kesintisi iddiası yok.

[Ham tasarım ve bütün loglar](benchmarks/dirty50-20261029-v1-design.json) · [Kapı](benchmarks/dirty50-20261029-v1-gate.json) · [Kurtarma/eski okuyucu](benchmarks/dirty50-20261029-v1-recovery.json) · [Süreç/oturum kaydı](benchmarks/dirty50-20261029-v1-lifecycle.jsonl).

## 2. 4 okuyucu + yazıcı — KALDI

Aynı881756DA… ilk50k, akışlı API, iki dengeli kol sırası. Her kol16384 tekli update/commit;1100 uygun mevcut2–199960B donör döngüde. Kaynak gövdeleri korunur, yalnız tekil revizyon etiketi değişir;41 sorgunun sonuç-ID kümeleri sabittir. Bu, yeni içerik türleri/insert/delete karışımını temsil etmez; önceki tekli karma test ayrı kalır. Her kontrol/yüklü kol aynı commit sayısı ve8. commit sonrası aynı bakım çağrısını kullanır. FDB okuyucuları search_first_hybrid(bütçe200),N50; SQLite capped N50, WAL/FULL,FTS5 trigram. Her motorda yüklü ölçümden önce idle1R12tur ve idle4R3tur; kapı idle4R ile karşılaştırılır. Pooled idle n=24/sorgu, yüklü örnek sayısı aşağıda. Warm OS/engine cache, eviction yok.
Bu dört okuyuculu idle baseline, kabul edilmiş tek okuyuculu ölçek raporunun p95’iyle aynı yük değildir; bu adımda göreli eşzamanlı yük kapısı kullanılır.

Okuma süresi snapshot admission+arama+bırakma; tam payload/version/model oracle aralığın dışında, ancak aynı proseste CPU/I/O ve snapshot pin ömrüyle yüke katılır. Oracle maliyeti arama süresi diye sunulmaz. Writer throughput loop süresi bakım ve model/history bookkeeping dahil; salt fsync hızı değildir. OS private/working set motor, katalog/model, tüm okuyucular, history ve sonuç vektörleri dahil bütün benchmark prosesidir; engine-only RAM ayrıştırılmadı. Dış Mutex yok. İç melez cache/permit beklemeleri ayrıca instrument edilmedi; JSON’daki placeholder sıfırlar ölçülmüş sıfır bekleme anlamına gelmez.

| Kapı | Sonuç | Karar |
|---|---|---|
| Her41 sorgu: loaded p95≤2×idle; idle<1ms ise+2ms | 25/41 geçti;16 kaldı | KALDI |
| Yazma pooled oran≥%75 ve SQLite üstü | FDB%80.11; SQLite%73.97 | GEÇTİ |
| Yüklü tekli commit p95/p99/max≤20/50/100ms | 4.446/14.942/199.929ms | KALDI |
| Payload/version/commit-sınırı oracle;3 kill/reopen;eski okuyucu | Tam50k GET+41 exact ve korunan kaynaklar geçti | GEÇTİ |

| Motor | Kontrol ops/sn | 4R ops/sn | Pooled oran |
|---|---:|---:|---:|
| fdb | 387.183 | 310.167 | %80.11 |
| sqlite | 281.471 | 208.213 | %73.97 |

| Kol | Motor | Kontrol ops/sn | 4R ops/sn | Oran |
|---|---|---:|---:|---:|
| pair0 | fdb | 384.075 | 306.484 | %79.80 |
| pair0 | sqlite | 275.334 | 203.026 | %73.74 |
| pair1 | fdb | 390.342 | 313.939 | %80.43 |
| pair1 | sqlite | 287.887 | 213.671 | %74.22 |

| Case/sorgu | FDB idle p95 | FDB 4R+yazıcı p95 | Kapı ms | Δms /oran | FDB n yüklü | Karar | SQLite idle /yüklü p95 |
|---|---:|---:|---:|---|---:|---|---|
| 0: olduğu | 4.671 | 21.040 | 9.342 | +16.368 /4.50× | 121 | KALDI | 657.364/807.436 |
| 1: büyük | 3.899 | 19.471 | 7.799 | +15.571 /4.99× | 121 | KALDI | 598.214/777.848 |
| 2: ayrıca | 4.563 | 18.814 | 9.126 | +14.250 /4.12× | 121 | KALDI | 709.067/863.426 |
| 3: arasında | 5.288 | 28.239 | 10.577 | +22.951 /5.34× | 121 | KALDI | 980.497/1206.013 |
| 4: birlikte | 4.117 | 19.702 | 8.234 | +15.585 /4.79× | 121 | KALDI | 632.372/768.840 |
| 5: sanat | 4.736 | 18.402 | 9.473 | +13.666 /3.89× | 121 | KALDI | 571.072/701.094 |
| 6: görenler | 10.711 | 24.716 | 21.423 | +14.005 /2.31× | 121 | KALDI | 618.731/742.169 |
| 7: millî | 4.549 | 9.527 | 9.099 | +4.978 /2.09× | 121 | KALDI | 176.757/219.817 |
| 8: ikinci | 4.008 | 16.545 | 8.017 | +12.536 /4.13× | 121 | KALDI | 677.601/828.152 |
| 9: toplam | 5.505 | 18.587 | 11.011 | +13.082 /3.38× | 123 | KALDI | 515.761/636.039 |
| 10: isimli | 10.320 | 26.236 | 20.639 | +15.916 /2.54× | 123 | KALDI | 687.303/838.969 |
| 11: öldürür | 180.978 | 271.021 | 361.956 | +90.043 /1.50× | 123 | GEÇTİ | 301.082/374.653 |
| 12: ölümü | 5.065 | 25.264 | 10.131 | +20.199 /4.99× | 122 | KALDI | 558.566/675.009 |
| 13: akarsu | 197.672 | 101.742 | 395.344 | -95.930 /0.51× | 122 | GEÇTİ | 220.443/259.589 |
| 14: demokratik | 7.843 | 15.399 | 15.686 | +7.556 /1.96× | 122 | GEÇTİ | 115.099/143.854 |
| 15: çalışmalarıyla | 297.767 | 371.342 | 595.534 | +73.575 /1.25× | 122 | GEÇTİ | 464.944/560.177 |
| 16: üreticisi | 208.800 | 263.199 | 417.600 | +54.399 /1.26× | 121 | GEÇTİ | 350.593/475.933 |
| 17: joseph | 3.552 | 9.335 | 7.103 | +5.784 /2.63× | 121 | KALDI | 74.981/108.350 |
| 18: leyla | 202.538 | 224.675 | 405.077 | +22.137 /1.11× | 121 | GEÇTİ | 442.767/528.552 |
| 19: protest | 9.299 | 16.043 | 18.598 | +6.744 /1.73× | 121 | GEÇTİ | 174.642/212.108 |
| 20: furlong | 10.916 | 15.731 | 21.832 | +4.815 /1.44× | 121 | GEÇTİ | 14.204/19.572 |
| 21: korkaklığa | 40.019 | 64.666 | 80.039 | +24.646 /1.62× | 121 | GEÇTİ | 49.280/73.227 |
| 22: ormanların | 436.305 | 544.459 | 872.610 | +108.154 /1.25× | 121 | GEÇTİ | 588.697/683.057 |
| 23: yanıklarda | 668.905 | 471.305 | 1337.810 | -197.600 /0.70× | 123 | GEÇTİ | 434.052/527.830 |
| 24: külliyatı | 291.615 | 382.422 | 583.229 | +90.808 /1.31× | 123 | GEÇTİ | 363.430/431.750 |
| 25: timmy | 160.191 | 161.663 | 320.383 | +1.472 /1.01× | 122 | GEÇTİ | 31.441/38.161 |
| 26: yüklük | 121.368 | 228.590 | 242.736 | +107.222 /1.88× | 122 | GEÇTİ | 300.673/381.541 |
| 27: ilçesine köyleri | 9.286 | 18.046 | 18.572 | +8.760 /1.94× | 122 | GEÇTİ | 156.942/185.204 |
| 28: geldiği buldu | 250.138 | 254.050 | 500.276 | +3.912 /1.02× | 122 | GEÇTİ | 372.387/448.476 |
| 29: chicago studio | 10.882 | 19.071 | 21.765 | +8.188 /1.75× | 122 | GEÇTİ | 18.666/25.292 |
| 30: kısım dişli | 412.005 | 557.514 | 824.010 | +145.509 /1.35× | 122 | GEÇTİ | 502.574/598.809 |
| 31: güzeli zafer | 183.123 | 260.057 | 366.246 | +76.934 /1.42× | 121 | GEÇTİ | 54.526/86.686 |
| 32: İnternetten hazinem | 166.448 | 308.033 | 332.896 | +141.585 /1.85× | 122 | GEÇTİ | 190.830/229.302 |
| 33: oyuncudur atlanta | 203.737 | 307.957 | 407.475 | +104.220 /1.51× | 122 | GEÇTİ | 110.134/145.415 |
| 34: kendisine çıkacağını | 509.680 | 396.502 | 1019.360 | -113.178 /0.78× | 122 | GEÇTİ | 346.334/424.265 |
| 35: ve | 2.189 | 4.447 | 4.377 | +2.258 /2.03× | 121 | KALDI | 928.894/1160.636 |
| 36: if | 3.800 | 6.103 | 7.600 | +2.303 /1.61× | 121 | GEÇTİ | 1182.131/1425.700 |
| 37: id | 2.590 | 4.567 | 5.180 | +1.977 /1.76× | 121 | GEÇTİ | 1061.002/1289.314 |
| 38: zx | 677.224 | 759.710 | 1354.448 | +82.486 /1.12× | 121 | GEÇTİ | 1110.354/1384.919 |
| 39: qzxkvwnever | 0.969 | 4.143 | 2.969 | +3.174 /4.28× | 120 | KALDI | 0.802/9.513 |
| 40: transubstantiationz | 4.285 | 16.452 | 8.570 | +12.167 /3.84× | 120 | KALDI | 6.564/18.743 |

### Commit uçları, RAM ve bekleme

| Kol | Motor | Mod | Commit p95/p99/max ms | >100ms adet | OS peak private/WS MiB |
|---|---|---|---|---:|---|
| pair0-control | fdb | kontrol | 3.450/11.703/172.378 | 3 | 49.746/54.629 |
| pair0-control | sqlite | kontrol | 7.617/21.925/208.019 | 17 | 42.734/47.641 |
| pair0-loaded | sqlite | 4R | 10.860/23.908/358.869 | 39 | 97.410/100.559 |
| pair0-loaded | fdb | 4R | 4.550/15.119/57.974 | 0 | 114.883/117.742 |
| pair1-loaded | sqlite | 4R | 10.504/23.177/328.481 | 37 | 100.668/104.133 |
| pair1-loaded | fdb | 4R | 4.377/14.547/199.929 | 4 | 112.637/115.359 |
| pair1-control | fdb | kontrol | 3.430/11.457/69.900 | 0 | 50.363/55.316 |
| pair1-control | sqlite | kontrol | 7.411/19.945/197.937 | 14 | 42.445/47.395 |

SQLite için max kapısı uygulanmadı. Kontroldeki100ms üstü FDB uçları da tabloda saklanmadan gösterilir; bunlar16384 uzun update serisidir, önceki dört tipli512/tip kısa testin kabulünü geriye dönük değiştirmez. Ancak uzun yazma uçları açık kalır; bilinen74,840ms örnekle aynı neden oldukları kanıtlanmadı.

| FDB kol | Sequence | Commit ms | Kol içi bitiş ms |
|---|---:|---:|---:|
| pair0-control | 4649 | 125.7226 | 11753.3643 |
| pair0-control | 4747 | 149.6352 | 12137.7143 |
| pair0-control | 7624 | 172.3777 | 19615.1286 |
| pair1-loaded | 7149 | 149.1070 | 21801.6838 |
| pair1-loaded | 7696 | 198.5254 | 23837.3202 |
| pair1-loaded | 12025 | 175.8124 | 37889.1352 |
| pair1-loaded | 14223 | 199.9291 | 45009.1627 |

Bu profil kapalı uçların WAL/worker/yayın/scheduler ayrımı kaydedilmedi; nedenleri açık.

Yüklü FDB toplam4982 okuma: admission contended=0, admission max=0.0144ms, ölçülen admission wait max=0.0000ms; bırakma dış süresi max=24.9713ms. Arama içindeki writer/cache beklemesi ayrı ölçülmediğinden “hiç yazıcı beklemesi yok” iddiası kurulmaz. Aşağıdaki satırlar her kol/sorgunun p95’ine denk gelen AYNI çağrının ayrımıdır; ayrı yüzdelikler toplanmaz.

| Kol/case | Sequence | Toplam ms | Arama ms | Admission ms | Bırakma ms |
|---|---:|---:|---:|---:|---:|
| pair0-loaded/0 | 15285 | 22.5868 | 22.4729 | 0.0007 | 0.1132 |
| pair0-loaded/3 | 10424 | 33.7669 | 33.7662 | 0.0001 | 0.0006 |
| pair0-loaded/39 | 6374 | 4.7648 | 4.7641 | 0.0003 | 0.0004 |
| pair0-loaded/40 | 12975 | 22.1049 | 22.1042 | 0.0001 | 0.0006 |
| pair1-loaded/0 | 13175 | 20.8158 | 20.7253 | 0.0002 | 0.0903 |
| pair1-loaded/3 | 16229 | 25.0791 | 25.0785 | 0.0002 | 0.0004 |
| pair1-loaded/39 | 12338 | 3.6080 | 2.7081 | 0.0003 | 0.8996 |
| pair1-loaded/40 | 5677 | 10.0549 | 10.0539 | 0.0003 | 0.0007 |

Ölçülen artış bu örneklerde ağırlıkla arama çağrısındadır; admission kapısında uzun bloklanma baskın neden değildir. Arama içi payload/posting/CPU/scheduler payları bu profil kapalı koşuda ayrılmadı; kesin alt neden veya çözüm iddiası yok. Ayrı dar, sequence eşlemeli arama profili sonraki onay için öneridir.

### Kapsam, kurtarma ve süreç kaydı

8. commit bakım çağrısının job sayaçları ham tasarımda yer alır. Bu koşuda varsayılan reclamation kapalı; bağımsız revision-rewrite stresini veya uzun snapshot sınamasını tamamlandı diye sunmuyoruz. Okuyucular her snapshot’ın sequence’ine göre tam hit payload/version ve son commit grubu oracle’ını doğruladı; eski revision etiketi görünmedi. Her kol sonu50k GET oracle geçti. Ayrı kurtarma kontrolünde3 kez strict-open tanığından sonra process kill, ardından reopen ve dondurulmuş eski okuyucu50k GET+41 exact oracle geçti. Bu kill’ler append ortası veya elektrik kesintisi deneyi değildir.

**Doğrulama aracında ilk deneme başarısızdı:** üç kill/reopen ve eski okuyucu oracle geçti, ardından klasördeki bütün .fdb/.ready envanterinin aynı kalması assert’i kaldı. İlk başlangıç hash listesi dosyaya kaydedilmediği için değişen dosya adı/byte farkı geriye dönük kanıtlanamadı. Strict-open eski/manifest dışı dosyaları toplayabilir; bu olası açıklamadır, ilk farkın kanıtlanmış kök nedeni değildir. Motor değişmedi. Araç mevcut CRC kontrollü MAN03 referanslarını ve bütün dosya hash listesini her aşamada saklayacak şekilde düzeltildi. Aynı DB’de ayrı üç kill/reopen + eski okuyucu tekrarında canlı kaynaklar/current manifest hash’leri ve bütün kalan envanter aynı kaldı. İlk başarısız araç koşusu silinmedi; başlangıç envanterinin eksikliği açık gözlem sınırı olarak kaldı. [İlk başarısız assert](benchmarks/hybrid4r50-20261029-v1-b1-recovery-inventory-assert-failure.json) · [Ayrı tekrar](benchmarks/hybrid4r50-20261029-v1-b1-recheck-recovery.json) · [Aşama bazlı canlı/tam hash envanteri](benchmarks/hybrid4r50-20261029-v1-b1-recheck-recovery-inventory.json).

Wrapper her çocuğun PID/start/exit kodunu fsync’li lifecycle’a yazar; ayrı observer process handle ve oturum/OS olaylarını izler. WMI start/stop aboneliği erişim hatasıyla kullanılamadı; kayıt bunu açıkça taşır. Bu turdaki16 ölçüm/seed çocuğu exit0; beklenmedik ölçüm süreci kapanması gözlenmedi. İlk recovery Python driver’ı yukarıdaki assert nedeniyle exit1 oldu; çocuğun açıklanamayan çökmesi değildir. İlk recovery ve tekrarın üçer nonzero çocuk çıkışı wrapper’ın tanıklı/amaçlı kill’idir. Tüm uygulama/job birlikte kapanırsa bu izleyici kesin fail eden aktörü her durumda saptayamaz; önceki yarım oturumların nedeni geriye dönük kanıtlanmadı.

[Ham kapı ve41×2 motor matrisi](benchmarks/hybrid4r50-20261029-v1-b1-gate.json) · [Tasarım/tüm kol logları/RAM](benchmarks/hybrid4r50-20261029-v1-b1-design.json) · [Kill/reopen/eski okuyucu](benchmarks/hybrid4r50-20261029-v1-b1-recovery.json) · [Lifecycle](benchmarks/hybrid4r50-20261029-v1-b1-lifecycle.jsonl) · [Observer](benchmarks/hybrid4r50-20261029-v1-b1-process-events.jsonl).

## Güncel sıralı kapılar

| İş | Karar |
|---|---|
| Kısa tekli test yeni p95/p99/max20/50/100 | GEÇTİ;74,84ms açıklanmamış uç açık |
| 1: Dirty500 strict≤1000ms, idle≤15MiB | GEÇTİ |
| 2: 4R+tekli yazıcı | KALDI |
| 2: 4R+100lü yazıcı | BAŞLAMADI |
| 3: 100lü profil /500lü drain önerisi | BAŞLAMADI |
| 4: Gerçek Frunner anahtarı | BAŞLAMADI |

**Durma kuralı uygulandı:** 4R okuma/tekli yük kapısı kapanmadan100lü eşzamanlı kol, 100lü aşama profili ve Frunner anahtarı testi başlatılmadı. Optimizasyon yapılmadı. snapshot.mask35ms düzeltmesi ayrı onay bekliyor; bu sonuç onu uygulama izni değildir.

Arama regresyonları (case34/the zx parçalı yerleşim), signed-zero, yazma/bakım kuyrukları, RAM/disk ve eşzamanlı yük açık iş listesinde korunuyor. 100k/format/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/dış entegrasyon yapılmadı.

## Kod ve test sınırı

cargo fmt --check ve cargo clippy --all-targets -- -D warnings geçti. cargo test --all-targets:243 geçti,15 mevcut ignored; normal paralel çalıştırma. No-default/all-features lib check ve izole SQLite fmt/clippy/test geçti. Melez adaptörün JSON etiketi düzeltildikten sonra fmt/clippy ve donmuş benchmark ikilileri yeniden oluşturuldu; zamanlı koşular sırasında build/test çalıştırılmadı. [8 komutun exit/log kanıtı](benchmarks/snapshot-10k-20261026-validation-hybrid4r50-20261029-v1.json).

Önceki WRITE-PROFILE kaynak hash’lerine göre70 engine dosyasından69’u aynı; src/format.rs içindeki mevcut diagnostic feature’a bağlı iki satırlık WAL replay span’i çıkarılınca o dosyanın hash’i de öncekiyle aynı. Normal motor davranışı/kalıcı biçim/limit değişmedi. Kalan değişiklikler benchmark adaptörü, test aracı ve raporlardır. [Son kapsam/kanıt kontrolü](benchmarks/write-followup50-20261029-final-audit.json).


## Sonradan revize edilen kapılar — kullanıcı kararı

Kullanıcı, yukarıdaki sonuçları gördükten sonra sorgu ve uzun-seri kabul kurallarını değiştirdi. Bu tarihsel tablolar ve KALDI kararları aynen korunmuştur. Yeni kurallarla mevcut ham veride sorgu41/41 ve bütün FDB uzun-seri kolları GEÇTİ. Bu bir motor iyileştirmesi değildir. [Yeni/eski yan yana, p99,9, >100ms oranı ve ölçülen profiller](WRITE-REVISED-PROFILE-50K-20261029.md). Sonraki100lü yük kapısı ayrıca ölçülür.
