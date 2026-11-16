# FDB 0.4.0 arka plan reclaim ölçüm tasarımı

Başlangıç sürümü `v0.4.0` (`a2d9e08`). Arka plan reclaim varsayılan
açık. Önceki 50k koşusunda iki saniyelik yazmasız pencereyle disk 30/30
turda ≤1,3×, en yüksek 1,281×. 3.000 adet 100'lük commit'in 23'ü 1 s
üstünde. En uzun commit 25. turda, sequence 2511 ve 2.093,48 ms.
[Kabul edilen rapor](BACKGROUND-RECLAIM-DEFAULT-50K-20261112.md)

## Sıra, üst sınırlar ve durma

| Adım | En fazla | FDB slot üst sınırı | Yeni kanıt dosyaları üst sınırı |
| --- | --- | ---: | ---: |
| 1. Dosya olayı | 15 dk, 6 çift, 24 aday + 24 kontrol olayı | 8 GiB | 64 MiB |
| 2. Commit aşama profili | 45 dk, dört 30 turluk kol, toplam 12.000 commit | 8 GiB | 192 MiB |
| 3. Kesintisiz yazıcı | 20 dk, 30 tur, 3.000 commit | 8 GiB | 64 MiB |
| 4. Motor-only RAM | 25 dk, dört ingest, dört temiz idle ve kol başına en çok bir %20 churn örneği | 8 GiB | 32 MiB |
| 5. Uzun snapshot | 8 dk, bir 10k koşusu, bir 120 s pin, bir churn turu | 8 GiB | 32 MiB |

Adım 1–4 için ortak üst sınır 105 dakika ve 352 MiB yeni kanıt.
Adım 5 için ayrıca `başla` gerekir. Her süreye hazırlık, ölçüm ve
doğrulama dahil. Başlangıç ve bitiş sınırları yürütmede UTC olarak
kayda girer. Aynı anda tek deney, aynı sahipliği doğrulanmış
`data/benchmark-work/fdb` slotu. 50k için mevcut 881756DA… korpusun ilk
50.000'i, 10k için aynı korpusun ilk 10.000'i. Korpus ve `private/`
salt okunur. SQLite slotu ve 0.1 fixture korunur. 1. adım mevcut
yerleşimi kullanır. 2–5. adımlarda temiz seed gerekiyorsa yalnız sahipli
FDB slotu sıfırlanır. Adım 5 onayı bu slotun 50k'dan 10k'ya
dönüşmesini de kapsar. DB kopyası ve yeni korpus yok.

Başlangıçta en az 23 GiB boş alan aranır. Süre boyunca 8 GiB slot
tavanı ve 15 GiB boş alan tabanı izlenir. Disk örneklemesi 50 ms,
dosya büyümesinin örnekler arasında devam edebileceği raporda belirtilir.
Kanıt dosyaları için yazma öncesi kapasite kontrolü uygulanır.
Profil ve ölçüm derlemeleri için tek tekrar kullanılan geçici hedef
klasörü, en fazla 2 GiB ek derleme alanı. Onu da 15 GiB korumasına dahil
et, sınırda yeni derleme başlatma. WPR, ETW ve purge yok.

Oracle, CRC veya uyumluluk kalırsa ya da herhangi bir üst sınıra
gelinirse ölçüm durur. Tamamlanan kollar, kısmi kanıt ve slotun son
durumu kayda geçer. Performans kapısı kalırsa o adımın sonucu ve eldeki
örnekler raporlanır, bir sonraki adıma geçmeden karar beklenir.
Her yazmalı deneyde kapanışta model oracle, süreç sonlandırıp strict
reopen ve eski okuyucu doğrulaması. İlk dört adımda 50k GET/version ve
41 sorgu digest'i, son adımda 10k ve o öneğe ait digest. Model, disk ve
oracle işlemlerinin saatleri ölçüm saatinden ayrı. Ayar ve dosya
biçimleri korunur. Profilli ölçüm için gerekirse yalnız geçici derleme
kopyasına gözlem eklenir.

## 1. Üretim varsayılanıyla dosya olayı

Önceki altı eşleştirilmiş çevrim kullanılır. Her çevrimden önce %20
churn, aynı dört kayıt ve donör gövdesiyle 50, 500, 1.500 ve 3.000 ms
olayları. 1 akışlı yazıcı, kısa snapshot kullanan 1 okuyucu, aynı 41
sorgu döngüsü ve doğal sıcak cache. Aday/kontrol sırası dönüşümlü.
Kabul kolunda 0.4.0 reclaim varsayılanı olduğu gibi kullanılır.
Kontrol kolunda yalnız o oturum ve olay penceresi için otomatik reclaim
kapatılır. Önceki koldan kalan işin devreden çıkarılması zamanlama
penceresi dışında yapılır. Eşleştirme aynı olay ve gövdeye dayanır,
ardışık kolların yerleşimi ve sequence'i ayrı kaydedilir.

Olay üreticisinin gerçek enqueue anından doğru marker'ı bulan ilk
sonuca kadar ölç. Kuyruk, commit, commit dönüşü→okuyucu başlangıcı,
snapshot acquire ve ilk sorgu ayrı. Dosya izleyicisi ve içerik çıkarma
bu saatin dışında. 24/24 aday olay ≤1 s kapısı. İşçi planlama, rewrite,
hazır yayım ve GC örtüşmelerini ayrı etiketle. Hiçbir olay gerçek
rewrite ile örtüşmezse yalnız genel görünürlük raporlanır ve
rewrite altında kapı doğrulanamadı kaydı düşülür. Ek olay eklenmez.
[Önceki olay ölçümü](RECLAIM-ARRIVAL-50K-20261112.md) ·
[İlk arka plan prototipi](BACKGROUND-RECLAIM-50K-20261112.md)

## 2. 100'lük commit uçlarının aşamaları

Aynı temiz 50k seed ve 1R+1W düzeninde 30 tur. Önceki 2 saniyelik ara
korunur. A/B/B/A sırası. A profilsiz 0.4.0, B aynı kaynakta geçici
bounded faz profili. İkisi de varsayılan reclaim ile. Her kolun 3.000
commit'i ölçülür. Profil farkını p95/p99/p99,9, 1 s üstü adet ve
throughput ile göster. Tek başına profiler'daki uç sayısı normal yolun
uç oranı olarak kullanılmaz.

Her >1 s olayda tur, commit sequence, giriş sequence, WAL kimliği,
işçi türü ve kimliği kaydedilir. Sıra 2511 (25. tur) dört kolda da
çıkarılır, profilli iki kolda aşamaları verilir. Eski 2,093 s olayın
geriye dönük faz kaydı yok. Tekrar etmezse o olayın nedeni bilinmiyor.

| Ayrılacak aşama | Eşleştirme |
| --- | --- |
| WAL prefix ve normal merge | Foreground join duvarı, işçinin başlangıç/bitişi ve çalıştığı kaynaklar. Revizyonun bitmiş sonucu devralma ayrı. |
| Kaynak/builder bütçesi | Rezervasyon ve kilit edinme beklemesi, tutan işçinin kimliği mevcutsa onun fazı. Bekleme commit'in join süresinin içindeyse iki kez toplanmaz. |
| WAL, kaynak ve yayım | Yetkili WAL ve varsa tail write/fsync ayrı, kaynak ve indeks hazırlığı, manifest write/fsync, snapshot maskesi. |
| GC | Yazıcı ve snapshot release yolları ayrı. Mutex edinme ile envanter/unlink ayrılır. |

Commit duvar saati ve worker saatleri ayrı tutulur. Kapsanmayan
duvar zamanı açıklanamayan kalan olarak verilir. Kayıp trace frame ve
işçi eşlemesi raporlanır. >1 s sayımı tanı içindir, yeni kapı değildir.
[Mevcut commit trace](../src/engine/profile.rs) ·
[Önceki aşama profili](RECLAIM-PROFILE-50K-20261112.md)

## 3. Kesintisiz yazıcı

50k temiz seed, 30 × %20 churn, 100'lük gruplar, 1R+1W. Kuyruk
başlangıçtan sona hazır. Tur arasında iki saniyelik pencere, senkron
bakım veya yazıcıyı bekleten tur sonu disk taraması eklenmez.
Yalnız normal commit içi reclaim ilerler. Harici sampler, tur sınırını
son 100'lük commit'in ACK zamanı ve sequence'iyle eşleştirir. En yakın
tur sonu disk örneğinin zaman farkı yazılır. Manifest canlı ve
retained dosya envanteri ayrı, hareket halindeki envanterin kuşağı
değişmişse örnek yaklaşık olarak etiketlenir.

Her tur oranı, ilk 1,3× aşımı, toplam aşım sayısı, 30. tur, en yüksek
örnekli geçici disk ve kapanıştan önce bitmemiş revizyon durumu
raporlanır. Eski 1,3× çizgisi aynen uygulanır. Kapı tablosunda
yazmasız aralıklı sonuçla kesintisiz sonucu ayrı göster. Yazma
throughput ve 100'lük commit uçları da kaydedilir. Sonradan yapılacak
cleanup disk kapısının ölçümüne eklenmez. Doğrulama kapanışta ayrı.

## 4. Motor-only RAM

Mevcut yalıtılmış proses düzeneği, 50k ingest 100 × 500 belge.
Korpus/katalog, model, oracle, geçmiş ve örnekleyen süreç motor
prosesinin dışında. Arama okuyucusu ve query worker yok. Sınırlı stdin
girdisi, tek belge decode geçicileri ve runtime, çocuk proses OS
ölçümünde kalır. İki varsayılan ve iki oturumluk reclaim-kapalı
kontrol, dönüşümlü A/B/B/A. Kapalı kol yalnız reclaim işçisini
kapsam dışında bırakır. Normal WAL prefix/merge işçileri iki kolda
da aynı politika ile çalışır.

- Ingest için OS lifetime peak private commit ve peak working set
  ayrı. Yazı bitişi ile bakımı da içeren tepe ayrı zaman işaretleriyle.
- Temiz idle için aynı koldaki DB yeni bir strict-open prosesiyle
  açılır. Aktif işçi yokken current private ve working set ölçülür.
  Her kolun temiz idle örneğinden sonra ayrı bir çocukta aynı mevcut
  gövdelerle bir %20 churn turu ve iki saniyelik tik penceresi.
  Reclaim aktivitesi gözlenen anların OS private/working set değerleri,
  tüm churn proses tepesi ve aynı zaman penceresindeki kapalı kol
  ayrı tabloda. Bu değerler temiz idle ve ingest kapısına katılmaz.
- Varsayılan kolun mevcut kapıları ingest tepe private ≤60 MiB,
  temiz idle private ≤15 MiB. İki kol farkı reclaim'in çalıştığı
  koşulun proses farkıdır. Paylaşılan heap, allocator ve normal
  bakım yüzünden bunu tek iş parçacığının RSS'si diye çıkarmak
  mümkün değil. Worker aşaması tanığı planlama ve rewrite'ı ayırır.
  İşçi gerçekten çalışmadıysa o kolun reclaim dahil tepesi
  doğrulanamadı olarak yazılır. Normal motor işçilerini
  kapatarak ayrı bir yapay ingest yolu oluşturulmaz.

[Önceki yalıtılmış RAM düzeneği](ENGINE-ONLY-RAM-50K-20261030.md)

## 5. Uzun snapshot, ayrı onay

10k temiz DB'de tek snapshot 120 s tutulur. Snapshot alınır alınmaz
bir %20 churn turu, ardından mevcut `poll_maintenance()` ile
yazmasız ilerleme. 5, 30 ve 120. saniyelerde aynı snapshot ile
retained, manifest-canlı ve pin nedeniyle kalan emekli dosya baytları
sayılır. Yayım saati ve fiziksel temizliğin saati ayrı. Snapshot
bırakıldığında eski/yeni sorgu doğruluğu ve dosyaların geri kazanımı
kaydedilir. Aynı anda retained/canlı disk için eski ≤2× çizgisi
gösterilir. Bu 120 s tek tur, özgün 2c'deki 10 dakika ve 10 tur
yükünü kapsamaz. Pin süresince kaynak emekliye ayrılmazsa pinin disk
maliyeti bilinmiyor. Süre aşımı/zorla bırakma API'si kapsam dışı.
[Önceki uzun snapshot tasarımı](RECLAIM-DIAGNOSIS-DESIGN-50K-20261111.md)

## C ABI taslağı

### Bakım tiki

Yalnız yazıcı handle'ı üzerinde, tek yazıcı olay döngüsünde çağrılır.
Bekleyen dosya olayı commit önceliğine sahiptir. Kuyruk boşsa
zamanlanmış tek tik çalışır, sonra olay döngüsüne dönülür. Hazır
prefix veya merge/reclaim sonucunu yayımlama, dosya temizliği veya
iş planlama o çağrının olası işleridir. Bitmemiş revizyon işçisine
join yok. Hazır yayım, fsync, snapshot hazırlığı ve GC çağrıyı
uzatabilir. Tik sabit duvar saati garantisi taşımaz ve UI thread'inde
çalıştırılmaz.

Önerilen sonuç, ABI hata kodundan ayrı bir durum alanı.
`PROGRESSED` gözle görülür bir bakım adımı oldu. `PENDING`
iş veya cleanup var, uygun sonraki tikte tekrar çağrılır.
`IDLE` bilinen bekleyen iş yok. Mevcut Rust boolean'ı yalnız
ilerlemeyi bildirir, `false` bekleyen iş bulunmadığını kanıtlamaz.
Üç durum için ileride ayrı bakım durum bilgisi gerekir. Bekleme
ve yeniden çağrı zamanlamasını Frunner yönetir. Bozuk kaynak için
`ERR_REBUILD`, diğer I/O ve kapanmış handle hataları ayrı.
Başarılı bir commit'in sequence'i bakım hatasıyla geri alınmaz.
Eşzamanlı snapshot okumaları kendi handle'larında sürer.
[Mevcut Rust tiki](../src/engine.rs#L2919)

### `fdb_content_stats`

Sabit genişlikli sayaçlar, boyut/sürüm başlığı ve geçerlilik bitleri.
Hızlı çağrı sabit zamanlı durum bilgisini verir. Disk envanteri ve
canlı payload hesabı açıkça istenen ayrıntılı ölçümden gelir.
Son örneğin sequence/generation ve zamanı döner, eski veya
alınmamış değer sıfır sayılmaz.

| Alan | Önerilen anlam |
| --- | --- |
| `document_count`, `sequence`, `generation` | Yayımlanmış belge sayısı, mantıksal commit sequence ve manifest kuşağı. |
| `run_count`, `active_wal_bytes` | Geçerli manifestteki immutable run adedi ve aktif WAL dosya uzunluğu. |
| `retained_bytes` | FDB'ye ait mevcut kaynak, WAL, indeks/cache, manifest ve geçici/emekli dosyaların mantıksal dosya uzunluğu. Yabancı dosya ve OS cache hariç. |
| `live_payload_bytes` | Görünür en yeni belge sürümlerinin kodlanmış gövde baytları, aktif WAL sürümleri dahil. Tombstone, eski revizyon, frame ve indeks hariç. |
| `live_disk_bytes`, `pinned_retired_bytes` | Geçerli görünümün başvurduğu dosya baytları ve yalnız pin nedeniyle kalan emekli dosyalar. Ayrıntılı envanter. |
| `revision_state`, `pending_revision_jobs`, `cleanup_pending` | Reclaim kapalı/boşta/planlıyor/hazırlıyor/yayıma hazır. Bekleyen işçi adedi yazıcı başına 0 veya 1. Emekli dosya temizliği ayrı bayrak. |

Kaynak CRC ve manifest kimliği doğrulamasına tabi ayrıntılı örnek,
hangi snapshot'a ait olduğunu açıkça belirtir. Her stats çağrısında
disk taraması veya belge çözme yapılmaz. Mevcut Rust `Stats` ile
bu alanların tamamı elde edilemiyor. Bu belge ABI ve sayaç taslağıdır.
