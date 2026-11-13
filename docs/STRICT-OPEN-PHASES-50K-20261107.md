# FDB 50k sıcak strict-open aşamaları — 7 Kasım 2026

**Normal kullanıcı tanısı tamamlandı.** Profilsiz sıcak strict-open p50
**1.272,43 ms**. Profilli ikilide en büyük aşama **açılış görünürlük
maskesi**, p50 **1.213,39 ms**, profilli toplamın **%69,67**’si.
Ayrıntılı profil toplamı yükselttiği için maskenin profilsiz mutlak süresi
ve WPR maliyeti **bilinmiyor**.

## Düzen

- UTC başlangıcı **12:32:12**, yeni örnek sınırı **12:49:12**, deadline
  **12:52:12**. Ölçüm ve hash doğrulaması **12:37:16,123 UTC**’de bitti.
- Önceki ETW denemesindeki FDB dosya envanteriyle aynı ilk 50k Cap200k
  TR yerleşimi (corpus kimliği 881756DA…), 52 run, generation 8185,
  sequence 529967, 16 delta anahtarı. Tek geçici kopya, **159 dosya,
  467.167.854 B (445,53 MiB)**. Corpus yeniden okunmadı ve ingest yok.
- 16 mantıksal işlemci, Windows build 26300, Rust 1.99.0. Default
  `bounded-engine`, önceki kabul edilmiş arama davranışı ve
  `FDB_BENCH_50K_WRITE=1`. Strict-open dönüşünden sonra önceki deneydeki
  `set_streamed_wal_layer(true)` çağrısı ayrı zamanlandı.
- 10 blokta 40 ayrı proses. Çift bloklarda kontrol/profil/snapshot/profil,
  tek bloklarda profil/snapshot/profil/kontrol. Her blok öncesi kopyanın
  tüm dosyaları normal cached I/O ile 467.167.854 B okunarak ısıtıldı.
  Fiziksel residency ve gerçek soğuk cache bu tanıda ölçülmedi.
- Doğrudan ilk sorgu TR case0 `olduğu`, `shared/texts`, offset 0,
  melez200 N=50. Snapshot kolu ayrı bir prosesle aynı sorguyu kullanır.
  Tüm 40 örnekte strict validation durumu Verified, sequence 529967 ve
  50 hit CRC32 725765487 doğrulandı. Açılış boyunca 50.000 görünür belge.
- Tabloda p50 medyan, p95 en yakın üst sıra. **N=10’da p95 maksimumdur.**
  Yüzde payı aşamanın 20 ölçüm toplamının aynı 20 strict-open toplamına
  bölümüdür. Tüm sayaçlar wall time. İç içe süreler bir kez sayıldı.

## Toplam süreler

| Sıcak kol | N | Strict-open p50 / p95 (ms) | İlk sorgu p50 / p95 (ms) | Open-start → ilk sonuç p50 / p95 (ms) |
| --- | ---: | ---: | ---: | ---: |
| Profilsiz, doğrudan | 10 | **1.272,43 / 1.313,83** | 97,49 / 108,53 | **1.374,54 / 1.410,82** |
| Profilli, doğrudan | 20 | 1.742,12 / 1.782,93 | 148,11 / 152,73 | 1.889,61 / 1.933,90 |
| Profilli, ilk snapshot | 10 | 1.738,02 / 1.786,00 | 6,05 / 6,30 | 1.783,73 / 1.830,56 |

Profilsiz strict-open min/max **1.258,52 / 1.313,83 ms**. Her blokta iki
profilli doğrudan örneğin ortalaması tek kontrolle eşlendi. Profilli ikili
on blokta da daha yavaştı. Eşlenmiş fark **+416,99…+495,34 ms**,
medyan **+476,28 ms**. Fark ayrıntılı sayaçlar ve tanı derlemesi birlikte
açıkken ölçüldü, alt kalemlere ayrılmadı. Aşağıdaki süreleri profilsiz
1.272 ms’ye oranlamak veya farkın tamamını maskeden çıkarmak geçersizdir.

Proses başlatma, kapanış ve sorgu sonrası oracle strict-open zamanına
dahil değil. Dış proses süresi profilsiz kolda 1.435,40 / 1.527,06 ms,
profilli doğrudan kolda 1.949,08 / 2.017,73 ms (p50 / p95).

## Strict-open aşamaları — 20 profilli doğrudan örnek

| Ayrık aşama | p50 (ms) | p95 (ms) | Profilli toplam payı |
| --- | ---: | ---: | ---: |
| Yol, klasör ve kilit | 0,44 | 0,56 | %0,03 |
| Envanter ve manifest | 0,40 | 0,55 | %0,02 |
| Kaynak dosya açma ve uzunluk kontrolü | 46,05 | 50,01 | %2,66 |
| Native kaynak geçişi, okuma + CRC + belge/sequence kontrolleri | 159,02 | 165,25 | %9,13 |
| İndeks açma, header/footer, bağlama | 4,18 | 4,47 | %0,24 |
| İndeks CRC/doğrulama | 44,41 | 46,26 | %2,55 |
| SEG05 dizini ve paged kayıt doğrulama | 243,34 | 253,29 | %14,02 |
| Term sayfası CRC hazırlığı | 16,72 | 17,39 | %0,95 |
| Diğer paged metadata hazırlığı | 6,61 | 7,12 | %0,38 |
| Diğer indeks açma | 0,14 | 0,17 | %0,01 |
| Diğer run açma ve bağlama | 1,80 | 2,03 | %0,10 |
| Aktif WAL handle | 0,06 | 0,08 | <%0,01 |
| Aktif WAL replay, CRC, delta ve shadow | 2,73 | 2,83 | %0,16 |
| **Açılış görünürlük maskesi** | **1.213,39** | **1.251,68** | **%69,67** |
| Maskeli görünür belge sayımı | 0,04 | 0,05 | <%0,01 |
| Diğer görünürlük ve delta hazırlığı | 0,05 | 0,06 | <%0,01 |
| Açılış sonu kontrolleri ve dönüş | 1,10 | 1,49 | %0,07 |
| Ayrı atanmayan süre | 0,09 | 0,11 | <%0,01 |

52 run’lık açılışta her run kendisinden yeni run’larla `hide_keys`
çağırıyor. Bu yerleşimde döngü 1.326 run çifti üretiyor. `hide_keys`
örtüşmeyen aralıkları atlıyor. Örtüşen çift sayısı ve bu aşamadaki I/O,
CPU veya scheduler payı ölçülmedi. Kaynak geçişi CRC ile belge
doğrulamasını aynı okumada yapıyor. Bu iki maliyet için ayrı değer
üretilmedi.
[Kod, açılış maskesi](../src/engine.rs#L1312) ·
[Kod, run örtüşmesi ve anahtarlar](../src/segment.rs#L1688) ·
[Kod, native kaynak geçişi](../src/format/native_check.rs#L17)

## İlk okuyucu — açılış saatinin dışında

| Profilli snapshot aşaması | p50 (ms) | p95 (ms) |
| --- | ---: | ---: |
| Snapshot okuyucu hazırlama ve yayımlama | 39,44 | 40,24 |
| Bunun içindeki snapshot maskesi | 38,20 | 38,81 |
| Acquire | <0,001 | 0,001 |
| İlk sorgu | 6,05 | 6,30 |

İlk sorgunun kendi görünürlük maskesi sayacı profilli doğrudan kolda
**142,17 ms p50**, snapshot kolunda **0,018 ms p50**. Snapshot hazırlığı
açılışın dışındaki yoğun maskeyi önceden hazırlıyor. Bu karşılaştırma
profilli iki yolun iş dağılımını gösterir. Profilsiz snapshot kolu
ölçülmedi.

## Doğrulama, sınırlar ve kalan işler

| Kontrol | Sonuç |
| --- | --- |
| Aynı 50k, 52 run ve önceki FDB envanteri | **GEÇTİ** |
| 40/40 case0 sonuç oracle’ı | **GEÇTİ** |
| Her örnek sonu kopya dosya adı, boyut, mtime, öznitelik | **GEÇTİ** |
| Son SHA-256, canlı slot ve kopya 159/159, model SHA | **GEÇTİ** |
| Kurtarma flush/indeks onarımı/WAL kesme tanığı | Gözlenmedi, dosya envanteri ve içerik aynı |
| WPR açık/kapalı eşleştirmesi | **ÖLÇÜLMEDİ**, ayrı onay bekliyor |
| 2b gerçek cold, SQLite referansı ve 30. sorgu | **ÖLÇÜLMEDİ**, 2b açık, kapı aynı |

Üretim motoru ve varsayılanları değişmedi. Sayaç ekleri yalnız geçici
derleme kopyasındaydı. İki release derlemesi ve PowerShell AST kontrolü
geçti. Yeni bir depolama yolu eklenmedi. Önceki 50k tam oracle, kill ve
eski okuyucu sonuçları ana koşu raporunda durur. Bu tanıda kapsam her
örnekte case0 ve envanter kontrolüydü.
[Ham 40 örnek](benchmarks/strict-open-phases-50k-20261107/samples.jsonl) ·
[Aşama analizi ve blok eşleri](benchmarks/strict-open-phases-50k-20261107/analysis.json) ·
[UTC, hash ve doğrulama](benchmarks/strict-open-phases-50k-20261107/run.json) ·
[Geçici sayaç farkı](benchmarks/strict-open-phases-50k-20261107/instrumentation.diff.txt)

### Ayrı onay gerektiren WPR ve cold planı

1. **WPR etkisi** aynı profilsiz ikili ve aynı PID ile sıcak reopen.
   Kapalı→açık ve açık→kapalı, her durumda blok başına beş tekrar.
   Yönetici WPR komutları ölçüm penceresi dışında. Limit ve iz durumu
   kaydedilir. Önceki 1.287 ms tek sıcak ETW değeri ile bugünkü 1.272 ms
   medyanından WPR farkı çıkarılmaz.
2. **Eski izin çevrimdışı kontrolü** mevcut ETL’den tam çözümleme.
   Yerel tracerpt yardımında hedef dosya filtresi görünmüyor. Küçük mevcut
   ETL’de tam CSV dosyası oluşturmadan akışta süzme önce doğrulanmalı.
   PID, zaman penceresi, file object/key ve create/read eşlemeleri
   tutulur. Kullanılabilir akış yolu doğrulanamazsa veya tam çözümleme
   bütçeye sığmazsa yeni purge yapılmaz.
3. **Yeni cold izinin kabulü** eski limitte hangi collector’ın dolduğu
   bilinmiyor. Yeni kayıtta stop öncesi collector adı, boyutu, limiti ve UTC
   tutulur. Warm ve sıcak A/A kayıt dışında kalır. 54/54 beklenen FDB
   FileIO, hedef storage DiskIO ve birleşik kayıp 0 birlikte aranır.
   “Collector bazı ölçülemedi” etiketi, 512 MiB toplam / 256 MiB ETL,
   Open-start → ilk sonuç ≤1 s ve 30. sorgu yakınsama kuralı korunur.
   Dört eksik kaynağın eski izde eksik kalma nedeni henüz bilinmiyor.

Eski `estimator50-20261031-sqlite.exe` doğrulayıcısında
`scale-first-oracle` yolu `Connection::open` ile read-write açıyor ve
`PRAGMA journal_mode=WAL` çalıştırıyor. WAL/SHM kaybının gerçekleştiği adım
belirlenmedi. [Doğrulayıcı kodu](../tools/native_sqlite/src/bin/content_churn.rs#L191) ·
[Önceki ETW raporu](BLOCK2B-COLD-ETW-MAIN-50K-20261107.md)


Rapor kapanışı **12:40:55 UTC**. Kanıt kopyaları SHA-256 ile doğrulandı. Otomatik komut politikası bu işe ait geçici klasörün recursive temizliğini reddetti. 569256286 B geçici dosya şu klasörde kaldı: C:\Users\Serkan\AppData\Local\Temp\fdb-strict-open-20261107-123212.
