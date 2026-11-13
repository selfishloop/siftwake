# Strict-open — iki işçili run yükleme motorda

## Birleştirme

Normal bounded-engine strict-open, birden fazla immutable run olduğunda ortak havuzdan iki izni tek `try_acquire` ile ister. İki izin hemen alınırsa çağıran thread ve bir yardımcı yalnız run açma ve doğrulamasını paylaşır. İzin eksikse seri yol çalışır. Bu üretim kararı, prototipin izin bekleyen çağrısından farklıdır. Sorgu işçileri için kullanılan mevcut dört izinlik üst sınır korunur. [Run yükleme](../src/engine.rs) · [Ortak işçi izinleri](../src/engine/first.rs)

Paralel adım salt-okuma ve mevcut cache ile sınırlı. Eksik veya bozuk cache adayı, kaynak tekrar doğrulandıktan sonra seri onarım yoluna girer. Sonuçlar manifest sırasına alınır. WAL replay, shadow, görünürlük maskesi, snapshot yayımı ve strict CRC değişmedi. Arka plan açılışı aynı run dağıtıcısını kullanır ve kendi mevcut cache politikasını korur. Kullanıcı seçenekleri ve kalıcı biçim aynı.

İki hedefli regresyon testi eklendi. İlki paralel işçilerin türetilmiş dosyalara yazmadığını, manifest sırasının ve en yeni belge sürümünün korunduğunu, seri onarımın eksik MET04 ve bozuk indeksi düzelttiğini ve sağlam indeksin bozuk kaynağı kabul ettirmediğini kontrol ediyor. İkincisi ortak izinlerin dördü tutulurken strict-open’ın seri yoldan tamamlandığını doğruluyor. [Regresyon testleri](../src/engine/strict_open_tests.rs)

## Doğrulama

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, doctest ve `cargo check --no-default-features` geçti. Tam pakette **261 geçti, 15 ignore**. Gerçek üretim release ikilisiyle kill/reopen sonrası **50.000 GET/version ve 41 sorgu digest’i** geçti. Dondurulmuş eski okuyucu (`B7845966…`) aynı sonuçları verdi. Aktif slotun 159 dosyası ve model SHA’sı aynı. [Tam test günlüğü](benchmarks/strict-open-two-workers-merged-20261110/final-test.log) · [Kontroller](benchmarks/strict-open-two-workers-merged-20261110/final-checks.json) · [Kill/reopen ve eski okuyucu](benchmarks/strict-open-two-workers-merged-20261110/verification.json)

## Kısa sıcak karşılaştırma

52 run ve 16 aktif delta anahtarlı 50k Cap200k kopyası, 159 dosya ve 467.167.854 B. Aynı release ikilisinde altı ABBA/BAAB blok, kol başına 12 yeni proses. Geçici ölçüm derlemesinde seri kontrol anahtarı ve çalıştırılan işçi sayısı tanığı var. Aday birleşik varsayılan yolu kullanıyor. Kopya blok öncesi cached I/O ile ısıtıldı. WPR, ETW ve purge kullanılmadı. `olduğu` melez N=50, 24/24 strict Verified, 50 hit, CRC32 725765487, sequence 529967. 20:30:05–20:30:25 UTC. p95 en yakın üst sıra ve N=12’de maksimum. CPU süreç user+kernel, RAM süreç private commit ve working set. İlk ölçüm ikilisi hazırlığında geçici FFI kodu için kaynak kesiti yanlış seçildi. Kesit düzeltilene kadar örnek alınmadı.

| Ölçüm | Seri p50 / p95 | İki işçi p50 / p95 |
| --- | ---: | ---: |
| Strict-open, ms | 668,20 / 688,18 | **430,17 / 456,71** |
| Açılış → ilk sonuç, ms | 765,63 / 785,80 | **530,08 / 576,06** |
| Yalnız ilk sorgu, ms | 97,57 / 104,11 | 100,03 / 119,35 |
| Açılış CPU, ms | 664,06 / 671,88 | 679,69 / 718,75 |
| İlk sonuca kadar CPU, ms | 765,63 / 781,25 | 789,06 / 843,75 |
| Açılış tepe private, MiB | 7,867 / 8,246 | 8,344 / 8,691 |
| İlk sonuca kadar tepe private, MiB | 9,551 / 9,559 | 9,658 / 9,809 |
| Açılış tepe working set, MiB | 12,029 / 12,219 | 12,258 / 12,551 |
| İlk sonuca kadar tepe working set, MiB | 14,047 / 14,059 | 14,219 / 14,344 |

Açılış ve açılış dahil ilk sonuç **6/6 blokta** daha hızlı. Açılış p50 **%35,6** düştü. Açılış CPU p50 **+15,63 ms**, tepe private p95 **+0,445 MiB**. Yalnız ilk sorguda blok farkı medyanı **+2,36 ms**, p95 farkı **+15,24 ms**. Farkın nedeni bu kısa koşuda ayrıştırılmadı. Gerçek soğuk ve temiz ölçek kanıtı önceki prototip koşularına ait. Yeni koşu bu kanıtın yerine geçmez. Geçici derleme **86.109.688 B (82,12 MiB)**. [Ham 24 örnek](benchmarks/strict-open-two-workers-merged-20261110/samples.jsonl) · [Blok özeti](benchmarks/strict-open-two-workers-merged-20261110/summary.json) · [Önceki soğuk karşılaştırma](STRICT-OPEN-TWO-WORKERS-COLD-RETRY-20261110.md)

## Sıradaki hız adayı

En büyük ölçülmüş bekleme, 500’lük gruplu yazıların eski profilindeki **1,2–1,3 s önceki worker drain**. O örneklerde WAL sync 1–6 ms. Frunner’ın tek okuyucu ve tek yazıcılı canlı churn koşusunda önce disk tavanı ve bakımın yetişmesi ölçülürken commit p95/p99/max ve sequence başına worker drain ayrılabilir. Böylece bir sonraki değişiklik için aynı yükten hem disk hem gecikme kanıtı çıkar. 100’lük grup ve eski 2,6 s uç için aynı neden kanıtlanmış değil. [Worker drain profili](WRITE-PROFILE-50K-20261029.md) · [Açık kuyruk uçları](FRUNNER-READINESS-20261107.md)
