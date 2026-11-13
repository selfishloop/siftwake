# Arka planda revizyon temizliği — 50k prototip

## Prototip

Tek revizyon işçisi immutable run referanslarını alır. Mevcut revizyon
seçimi, tombstone bariyerleri, kaynak CRC ve kaynak/indeks hazırlığı işçide
çalışır. Aktif WAL, temizlik için silme kanıtı sayılmaz. Yayımlanmamış
çıktı ve planın kullandığı kaynaklar iş sürerken GC’ye karşı korunur.
Normal prefix ve merge işi ayrı kalır. İşçi ortak kaynak/indeks çalışma
belleği bütçesini ve mevcut pacing’i kullanır. Bu bütçelerdeki dolaylı
beklemeler mümkündür.

Tek yazıcı, hizmet döngüsünde sonucu yalnız işçi bitmişse alır. Planın
dayandığı manifest run öneki kimlik ve sıra olarak aynı olmalı. Yeni run
sondan eklenebilir. Normal merge öneki değiştirmişse sonuç atılır. Normal
merge işi varken revizyon kurulumu ertelenir. Yayımlama mevcut manifest
ve snapshot yollarından geçer. Sonraki uygun hizmet turu emekli dosya
GC’sini çalıştırır. Pinli eski okuyucu kaynakları korunur. Opt-in yalnız
geçici derlemede. Üretim kaynakları, varsayılan ve kalıcı biçim kapsam dışı.

## Ölçüm sırası

Önce önceki senkron bakım deneyindeki olay desenini kullanan 6 eşleştirilmiş
çevrim. Aynı 881756DA… Türkçe 50k ve mevcut FDB slotu, sequence 3750.
Bir akışlı yazıcı ve kısa snapshot’lı tek okuyucu, 41 sorgu döngüsü.
Her çevrim %20 churn ardından aynı dört belge update’i. Olay anları
50, 500, 1.500, 3.000 ms, aday ve bakımsız kontrol sırası dönüşümlü.
Yeni olay varsa yazıcı önce olayı alır. Kuyruk boşsa en çok 5 ms beklemeli
hizmet döngüsü plan/kurulum/GC fırsatı verir. Zamanlanmış olaylar arasında
bekleme ölçülür. İşçi fazı olay anında atomik olarak alınır, planlama ve
hazırlık ayrı. Önceki senkron test farklı başlangıç sequence’ine ait
tarihsel referans. [Önceki senkron ölçüm](RECLAIM-ARRIVAL-50K-20261112.md)

Olay kapısı, adayda tüm ölçülen olayların ≤1 s görünür olması ve gerçek
hazırlıkla örtüşen en az bir olayın bulunması. Üretim geçişi kararı ayrı.
Bu kapı geçerse 30 tur disk ölçümü temiz 50k başlangıçla aynı sabit slotta.
100 adet 100’lük commit ve %20 churn/tur. Aday hem yazılar arasında hem
2 saniye yazmasız pencerede ilerler. Senkron `maintain_idle()` süresi bu
iki saniyeye eklenmez. Eski 30 turla fark bu zamanlamada. Ara proses
yeniden açılışları, bitmemiş arka plan işini iptal etmemek için disk
koşusuna eklenmez. Kapı her tur sonunda ≤1,3×, örnekli geçici tepe ayrı.
OS cache doğal durumda, sorgu döngüsü sıcak. WPR, ETW ve purge yok.
Tek zaman sınırı 50 dakika, 48 olay ve en fazla 30 disk turu.
[Plan penceresi](benchmarks/background-reclaim-50k-20261112/window.json) ·
[Sürücü](benchmarks/background-reclaim-50k-20261112/run.py)

## Dosya olayının aramada görünmesi

| Kol | Olay | Olay → ilk doğru sonuç p50 / p95 / max (ms) | Kuyruk p95 / max (ms) | Commit p95 / max (ms) | >1 s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Bakımsız kontrol | 24 | 57,07 / 101,68 / 119,70 | 0,04 / 0,04 | 21,40 / 23,52 | 0 |
| Arka plan revizyonu | 24 | 43,59 / 127,43 / 129,02 | 43,60 / 44,39 | 12,79 / 14,91 | 0 |
| Aday, işçi planlama veya hazırlıktayken | 19 | 41,33 / 127,43 / 127,43 | 0,06 / 0,06 | 14,91 / 14,91 | 0 |
| Aday, işçi çıktı hazırlarken | 6 | 35,70 / 127,43 / 127,43 | 0,03 / 0,03 | 11,18 / 11,18 | 0 |
| Bu altı olaya eşlenik kontrol | 6 | 50,71 / 99,66 / 99,66 | 0,04 / 0,04 | 21,40 / 21,40 | 0 |

**Olay görünürlüğü GEÇTİ.** 44 iş başlatıldı, 30 çıktı yayımlandı,
14 plan iş bulmadı. Hazır işçiyi devralmanın en uzun süresi 0,12 ms.
Yazıcının yaptığı en uzun çıktı kurulumu 58,28 ms, GC 9,28 ms, tek hizmet
turu 58,30 ms. En uzun kuyruk 44,39 ms. Bu örneğin kuyruk süresi
belirli bir hizmet aşamasına eşlenmedi. Commit dönüşünden okuyucunun
başlamasına p95 87,84 ms, snapshot acquire p95 0,0005 ms, ilk marker
sorgusu p95 23,68 ms. Yüzdelikler ayrı dağılımlardan alındı.

Tarihsel senkron adayda 24/24 olay 1 s üstüydü. Olay p50/p95/max
9,07/12,40/12,48 s. Bu koşuda adayın kontrolü arka plan hizmeti
başlatmayan kol. Eşleme olay ve gövde düzeyinde, kollar aynı canlı
slotta ardışık ilerledi. Önceki senkron koşu başka bir başlangıç
sequence’inde. [48 olay, sequence ve aşamalar](benchmarks/background-reclaim-50k-20261112/queue.jsonl) ·
[Kuyruk özeti ve doğrulama](benchmarks/background-reclaim-50k-20261112/queue-performance.json)

## 30 tur disk ve yazma

| An | Retained disk (MB) | Başlangıca oran | Run | Ölü payload (MB) | Tombstone |
| --- | ---: | ---: | ---: | ---: | ---: |
| Başlangıç | 300,84 | 1,000× | 3 | 0 | 0 |
| 1. tur | 362,84 | 1,206× | 13 | 25,99 | 2.500 |
| 10. tur | 359,49 | 1,195× | 61 | 15,65 | 3.117 |
| 14. tur (en yüksek oran) | 388,21 | 1,290× | 64 | 25,79 | 3.682 |
| 20. tur | 367,81 | 1,223× | 54 | 17,07 | 754 |
| 30. tur | 355,83 | 1,183× | 53 | 8,74 | 0 |

**Disk kapısı GEÇTİ 30/30.** İki saniyelik yazmasız pencere sonrası en
yüksek 1,290×, kapıya 2,88 MB kaldı. Ölçüm anları arasındaki 50 ms örnekli
geçici tepe **449,01 MB / 1,493×**. İşçi 425 kez başladı. 256 çıktı
yayımlandı, 164 plan iş bulmadı, 5 çıktı eski plan nedeniyle atıldı.
En uzun hazır işçi devralması 0,07 ms, kurulum 84,47 ms, GC 27,18 ms.
Bu süreler yazıcı hizmet döngüsündeki işlemlere ait.

| 100'lük commit | p50 / p95 / p99 / p99,9 / max (ms) | >1 s / 3.000 | Yazı duvarı / yazı ve boşluk dahil (işlem/s) |
| --- | ---: | ---: | ---: |
| Arka plan revizyonu | 13,29 / 47,62 / 1.012,51 / 1.794,25 / 2.075,47 | 33 | 1.976 / 1.416 |
| Tarihsel senkron idle adayları | 10,28–10,79 / 34,49–36,14 / 787,87–824,19 / 846,12–1.080,17 / 1.031,06–1.302,09 | 1–14 | 2.707–2.932 / 821–875 |
| Tarihsel SQLite referansı | p95 316–322, max 849–914 | 0 | — |

Yazı duvarı model güncellemesi ve gruplar arasındaki hizmet çağrılarını
içerir. Adayda 151,85 s yazı ve 60,01 s yazmasız pencere. Tarihsel
senkron idle adaylarında aynı birleşik oran bakım süresi ve 60 s boşluk
dahil yeniden hesaplandı. SQLite daha önce aynı iş yükünde ölçüldü.
Bu turda eşzamanlı bir SQLite veya senkron idle disk kolu çalışmadı.
En uzun commit 8. tur sequence 823. Commit içi aşama profili olmadığı
için bu ucu normal prefix/merge, kaynak bütçesi veya WAL’a bağlamak
mümkün değil. 100'lük uçlar açık.
[Senkron adayın ham tablosu](RECLAIM-IDLE-ONLY-50K-20261112.md) ·
[SQLite referansı](LIVE-DISK-50K-20261111.md)

41 sorgu karışımında yazarken p95 **98,59 ms**, yazmasız pencerelerde
**91,12 ms**. 30. tura kadar OS proses private commit tepesi **89,73
MiB**, working set tepesi **90,25 MiB**. Bunlar harness, model, okuyucu
ve işçiyi içeren proses değerleri. Motor-only bellek bu turda ölçülmedi.
Uzun snapshot ve kesintisiz yazıcı altında disk sınırı bilinmiyor.
[Tur ve sequence ayrıntıları](benchmarks/background-reclaim-50k-20261112/summary.json) ·
[Ham 30 tur](benchmarks/background-reclaim-50k-20261112/disk-30.jsonl)

## Doğrulama ve sınırlar

| Kapı | Sonuç |
| --- | --- |
| Dosya olayı ≤1 s, gerçek rewrite örtüşmesi | **GEÇTİ** · 24/24, altı örtüşme |
| 30 turun her birinde retained disk ≤1,3× | **GEÇTİ** · 30/30, en yüksek 1,290× |
| Onaylı commit sonrası process-kill → reopen | **GEÇTİ** · iki kolda 50.000 GET/version, 41 exact sorgu ve silme/güncelleme oracle’ı |
| Dondurulmuş eski okuyucu | **GEÇTİ** · iki kolda GET/version ve 41 digest eşit |
| Pause, snapshot, eski plan ve yayım öncesi process-kill | **GEÇTİ** · yazı devam etti, eski snapshot okundu, eski plan atıldı, yayımlanmamış çıktı recovery sonrası bakımda temizlendi |
| Biçim, Clippy ve varsayılan test paketi | **GEÇTİ** · fmt, Clippy `-D warnings`, 237 test geçti, 16 yardımcı test doğrudan çağrıda atlandı |

Olay kolunda oracle sequence 4399, disk kolunda 3101. FDB sabit slotu
son modelle `ready`, SQLite slot kaydı aynı. Korpus, 0.1 fixture, başlangıç
modeli ve üretim `src` SHA’ları eşit. Ölçüm ve 50k oracle penceresi
19:27:59–19:35:01 UTC. Reclaim üretim varsayılanı **KAPALI**.
Geçici derleme klasörü `%TEMP%/fdb-background-reclaim-20261112`
ölçüm bitiminde 559,36 MiB, tam test derlemesi sonrasında 2,12 GiB.
Klasör korundu. Disk ve görünürlük kanıtı bu yerleşimde tek 30 turluk
koşu ve altı eşleştirilmiş çevrimle sınırlı.
[Yürütme, hash ve oracle kaydı](benchmarks/background-reclaim-50k-20261112/status.json) ·
[Geçici kaynak ve harness farkı](benchmarks/background-reclaim-50k-20261112/candidate.diff.txt) ·
[Tam test ve Clippy kaydı](benchmarks/background-reclaim-50k-20261112/post-validation.log)
