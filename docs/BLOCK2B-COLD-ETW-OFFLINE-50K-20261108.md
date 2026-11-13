# Blok 2b mevcut cold ETL’nin çevrimdışı çözümlemesi — 8 Kasım 2026

**Tam izde de 50/54.** Önceki CSV kesmesi dört eşleşmenin eksik
kalmasını açıklamıyor. Dört dosya için cold PID’nin adsız okuma
olaylarında daha sonraki PID’lerde aynı dosya adına bağlanan FileKey
var. Bu ilişki tanı sinyali. Aynı cold penceresinde dosya adı ve
FileObject bağı eksik, dolayısıyla eski deneme **geçersiz** kalır.

## Akış doğrulaması ve sınırlar

- Yalnız mevcut iki ETL, önceki CSV ve kanıt dosyaları okundu.
  Yönetici işlemi, yeni purge, kayıt, DB/corpus kopyası ve benchmark
  yapılmadı. Başlangıç **09:39:02 UTC**.
- Windows’un yerel `tracerpt` aracının CSV çıkışı rastgele adlı,
  duplex ve çok örnekli named pipe’a yönlendirildi. İlk tek yönlü
  deneme access denied verdi, tek duplex örneği ikinci bağlantıda
  busy oldu. Çalışan düzende iki bağlantı açıldı, CSV baytları yalnız
  birinde geldi. EOF ve iki prosesin exit 0 kontrolü yapıldı.
  Her geçişte 180 s ve tüketici tarafında 512 MiB akış tavanı var.
- Kabul edilmiş preflight ETL için **158.083.124 B** aktı. Saklanmış
  tam CSV ile tüm olay satırları bayt düzeyinde eşit. Ham SHA’lar
  başlık ve son boş satırdaki ayraç farkı nedeniyle farklı.
  Saklanmış preflight’ın sıcak 0 B ve purge sonrası FDB 2.048 B /
  SQLite 65.536 B kanıtını taşıyan olay satırları aynı.
- Ana ETL iki farklı süzgeçle EOF’a kadar okundu. Her geçiş
  **302.864.951 B** CSV üretti. Diske yeni ham CSV yazılmadı.
  Yeni süzülmüş kanıt, çalıştırıcılar ve özetler yaklaşık **0,25 MB**.
  [Preflight satır karşılaştırması](benchmarks/cold-etw-offline-20261108/preflight-compare/pipe-run.json) ·
  [Ana decoder çıkışı](benchmarks/cold-etw-offline-20261108/main/pipe-run.json)

## Eski CSV kesmesi ve tam iz

| Kontrol | Sonuç |
| --- | --- |
| Tam çözümleme | **740.731 olay**, tracerpt özetiyle aynı |
| Tarihsel CSV kesme konumu | **188.864.296 B** |
| Bu konuma kadar yeniden bulunan kaynak | **50/54**, eski eksik listeyle aynı |
| Tam EOF sonuna kadar bulunan kaynak | **50/54**, aynı dört eksik |
| Eski tutulmuş kanıtla satır karşılaştırması | 52 örnek satırın kimliği ve içeriği eşit |
| Eski eşleştirme kuralıyla hedef storage DiskIO | **40.930.816 B** |
| Eşleştirilen storage olaylarında belirsiz atıf / hedef öncesi yabancı erişim | 0 / 0 |
| Birleşik ETL kayıp | **0**, **collector bazı ölçülemedi** |

[54 kaynak, olay satırları ve özet](benchmarks/cold-etw-offline-20261108/main/full-filter.json) ·
[Birleşik tracerpt özeti](benchmarks/cold-etw-offline-20261108/main/tracerpt-summary.txt)

## Dört dosya için çapraz kontrol

Cold PID **87776**, pencere open-start → ilk sonuç. Her satırda
`.idx` ve `.records04.idx` için cold PID’de ad taşıyan Create kaydı
var. Dört `.fdb` adının bütün izdeki Create kayıtları ilk sonuç
sonrasında, PID **55696** veya **83480** altında. Cold PID ve pencere
içinde bu dört kaynak için doğrudan ad + Create/Read eşleşmesi **0**.

| Kaynak | Tam izde ad taşıyan Create, hepsi pencere sonrası | Cold PID’de doğrudan eşleşme | Daha sonra bu ada bağlanan FileKey ile adsız cold Read | İlk adsız Read / indeks Create, açılışa göre ms |
| --- | ---: | ---: | ---: | ---: |
| `run-0000000000001ec3.fdb` | 4 | 0 | 8 | 750,674 / 752,059 |
| `run-0000000000001f5d.fdb` | 4 | 0 | 3 | 818,008 / 819,500 |
| `run-0000000000001f9a.fdb` | 4 | 0 | 1 | 859,561 / 859,618 |
| `run-0000000000001fae.fdb` | 2 | 0 | 109 | 862,219 / 872,409 |

Cold PID’nin 30.902 FileIO Read olayının **3.440’ında** eşleşecek
Create bulunmadı. Dört FileKey ipucu için ilk adsız olaylar
104,14–110,72 MB CSV konumlarında, eski kesmeden önceydi. Adı taşıyan
daha sonraki olayların FileKey’i tek başına cold penceresindeki
dosya/handle ömrünü kanıtlamıyor. Bu yüzden bu 121 Read, 54 kaynak
tanığına veya doğrulanmış storage baytına eklenmedi. Ad bağının neden
eksik olduğu bilinmiyor. Cold kaydın hangi collector’ı doldurduğu da
bu birleşik izden belirlenemedi.
[Ad, PID, byte konumu ve FileKey kanıtı](benchmarks/cold-etw-offline-20261108/crosscheck/name-crosscheck.json)

## Kapı ve sonraki onay noktası

Blok 2b **açık**, **0/8 geçerli**. Önceki collector sınırı hatası
yerinde duruyor. Ham 1.770 ms, cold performans kapısı kararı için
geçerli örnek değil. **Open-start→ilk sonuç ≤1 s** ve 30. aramada sıcak
A/A ile yan yana değerlendirme kuralı aynı. [Özgün ana koşu](BLOCK2B-COLD-ETW-MAIN-50K-20261107.md)

Yeni ana koşudan önce ayrı onayla tek kısa kayıt ön testi gerekir.
Kayıt profili PID/iş parçacığı, FileIO ad/yaşam döngüsü ve DiskIO
bağlantısı için gereken olaylarla sınırlandırılmalı. Cold açılış
penceresinde 54/54 dosyanın ad/nesne bağı, hedef DiskIO, birleşik kayıp 0
ve collector tavanına ulaşılmadığı birlikte gösterilmeli. Sonraki warm
PID’lerin ad kayıtları eksik cold bağı yerine kullanılmamalı. Tam CSV
artık disk tavanına takılmadan akışta süzülebiliyor. 50k disk churn
çalışması ayrı aşama olarak bekliyor.


Rapor kapanışı **09:46:34 UTC**.
