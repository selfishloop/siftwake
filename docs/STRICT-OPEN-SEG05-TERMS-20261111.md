# SEG05 terim döngüsü ve sayfa sınırı deneyi

52 run’lı sıcak 50k kopyada sayfanın son terimini kapanışta taşıyan aday, strict-open p50’yi **415,28 → 383,93 ms** indirdi. Açılış ve açılış dahil ilk sonuç **10/10 eşleşmiş blokta** daha hızlıydı. Aday geçici derlemede. [Önceki SEG05/MET04 ölçümü](STRICT-OPEN-SEG05-MET04-PHASES-20261110.md)

## Terim döngüsünün ölçümü

52 run’daki 789.189 terim/posting girdisinden her 16 girdide biri ölçüldü. Örnek başlangıcı run’a göre döndürüldü. Her profilli açılış 49.324 örnek aldı ve terim içeren bütün run’ları kapsadı. Aşağıdaki oranlar her run’ın terim/örnek oranıyla ölçeklenen süre paylarının medyanıdır. Zamanlayıcıların payı dahil olduğundan bu oranlar yaklaşık iş dağılımını gösterir. [Profil özeti](benchmarks/strict-open-seg05-terms-20261111/profile-summary.json)

| Aşama | Örneklenen süre payı p50 |
| --- | ---: |
| Terim ve posting çözümleme | %37,05 |
| Posting sınırı ve terim sırası | %10,17 |
| Koleksiyon, universal ve diğer özetler | %9,16 |
| Bloom güncellemesi | %15,32 |
| Sayfa sınırı hazırlığı | **%28,17** |

Profilleme kolunda strict-open p50 413,10 → 425,50 ms, eşleşmiş blok farkı medyanı **+7,72 ms** ve aralığı −24,02…+47,55 ms. Term döngüsünde eşleşmiş medyan ek süre **+12,92 ms**. Sayfa sınırı hazırlığı ölçülen süre payının %5’ini aşınca geçici aday karşılaştırması yapıldı. %5 deney seçimi için kullanıldı. [Profil ham örnekleri](benchmarks/strict-open-seg05-terms-20261111/profile-samples.jsonl)

## Sayfa sınırı adayı

SEG05 sıralama kontrolü, önceki terimi zaten sahipli bir değer olarak tutuyor. Aday yeni sayfa başladığında veya son sayfa bittiğinde bu değeri sınırın `last` alanına taşıyor. Açık sayfanın offset, uzunluk, girdi sayısı ve max posting count hesabı aynı kontrollerden geçiyor. Bu prototip sayfa başında mevcut yardımcıyı kullanıyor. Dolayısıyla `first` ve ilk `last` için sayfa başına iki klon var. 6.174 sayfada `Fence` alanlarını doldurmak için yapılan klon sayısı koddan **795.363 → 12.348** olarak hesaplanıyor. Bir audit açılışında kontrol ve adayın 6.174 sınırı alan alan eşit çıktı. [Aday kaynak farkı](benchmarks/strict-open-seg05-terms-20261111/term-pages.diff.txt) · [Sınır eşitliği koşusu](benchmarks/strict-open-seg05-terms-20261111/fence-audit.json)

Her karşılaştırma için aynı release ikilisinde 10 ABBA/BAAB blok ve kol başına 20 yeni proses. Önce profil karşılaştırması, sonra audit, ardından aday karşılaştırması. Her blok öncesi mevcut 159 dosya normal cached I/O ile ısıtıldı. Korpus Cap200k TR 50k, generation 8185, sequence 529967. `olduğu`, melez N=50, strict Verified, 50 hit ve CRC32 725765487 bütün 80 zamanlı örnekte geçti. WPR, ETW ve purge kapalıydı. p95 en yakın üst sıra, N=20’de 19. gözlem. Pencere 15:49:48–15:50:37 UTC, 25 dakika tavanı içinde. Aktif slot, ölçüm kopyası ve model SHA’ları eşit. [Koşu planı](benchmarks/strict-open-seg05-terms-20261111/plan.json) · [Koşu sonucu](benchmarks/strict-open-seg05-terms-20261111/status.json)

| Ölçüm | Kontrol p50 / p95 | Aday p50 / p95 |
| --- | ---: | ---: |
| Strict-open, ms | 415,28 / 489,51 | **383,93 / 416,10** |
| Açılış → ilk sonuç, ms | 509,35 / 588,58 | **479,41 / 515,03** |
| Yalnız ilk sorgu, ms | 95,18 / 104,87 | 95,33 / 100,39 |
| Açılış toplam CPU, ms | 656,25 / 828,13 | **601,56 / 656,25** |
| Terim döngüsü, iki işçide biriken ms | 217,60 / 279,22 | **158,26 / 176,04** |
| Açılış tepe private, MiB | 8,355 / 8,770 | 8,629 / 8,805 |
| İlk sonuca kadar tepe private, MiB | 9,650 / 9,723 | 9,609 / 9,781 |

Açılış blok farkı medyanı **−33,63 ms**, ilk sonuç **−33,17 ms**, açılış CPU **−62,50 ms**. Açılış tepe private p95 farkı **+0,035 MiB**, ilk sonuç dahil **+0,059 MiB**. CPU süreç user+kernel toplamı. RAM süreç private commit sayacı. Temiz yerleşim ve gerçek soğuk sonuçları bu adaya ait değil. [Aday ham örnekleri](benchmarks/strict-open-seg05-terms-20261111/candidate-samples.jsonl) · [Aday blok özeti](benchmarks/strict-open-seg05-terms-20261111/candidate-summary.json)

Adayla process-kill sonrası reopen ve dondurulmuş eski okuyucuyla 50.000 GET/version ile 41 sorgu digest’i geçti. Geçici derlemede `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` ve üç hedefli SEG05 testi geçti. İki subprocess yardımcı testi normal listede ignore. Geçici klasör `C:\Users\Serkan\AppData\Local\Temp\fdb-seg05-terms-20261111`, 565.571.096 B (539,37 MiB). [Okuyucu ve dosya doğrulaması](benchmarks/strict-open-seg05-terms-20261111/verification.json) · [SEG05 testleri](benchmarks/strict-open-seg05-terms-20261111/test.log)
