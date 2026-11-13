# Revizyon geri kazanımı — 50k tanı tasarımı

Durum **tasarım hazır, ölçüm onayı bekleniyor**. Kaynak çalışma
[`LIVE-DISK-50K-20261111`](LIVE-DISK-50K-20261111.md). Varsayılan kararı
ayrı. [Tek sayfa risk ve fayda](RECLAIM-DEFAULT-DECISION-50K-20261111.md)

## Commit uçları için sebep tablosu

| Aşama | Koddan görülen | Geçici ölçümde ayrılacak |
| --- | --- | --- |
| WAL | Küçük yol yetkili WAL’a, prefix bekliyorsa ayrıca yeni WAL kuyruğuna yazar. `Stage::append` aktarımı ve `sync_all` içerir. | Yetkili ve ikinci WAL ayrı. Aktarım, `sync_all` ve yeni aktif WAL dosyasının oluşturulması ayrı. |
| Flush ve normal merge | Commit girişinde hazır prefix ve merge kurulabilir. Küçük WAL yolu uygun değilse fallback worker’ları bekler, delta’yı flush eder ve yeni run üretebilir. | Küçük yol/fallback, hangi koşulun fallback’e yol açtığı, prefix/normal merge join’i, kaynak yazımı ve indeks kurma. |
| Revizyon planlama, işçi ve iptal | Ölçülen `streamed_wal_layer=true` ayarında, uygun normal merge bulunamazsa otomatik yol revizyon planlayıcısını çağırabilir. Planlama çağıran thread’de anahtar/slot maskesi, tombstone bariyeri ve liveness tarar. Rewrite/retire işçide. Fallback revizyon işçisini iptal edip join edebilir. | Committe senkron planlama, aday çıkmayan çağrılar dahil. İşçi türü, çıktı ID’si, başlatan sequence, rewrite/retire/cancel sonucu ve bekleyen commit. Kaynak CRC, kopyalama, posting remap, indeks ve pacing işçi süresinde ayrı. |
| Manifest, snapshot ve dosya temizliği | Kurulum manifest yayımlar, yeni snapshot hazırlar ve emekli dosyaları toplamayı dener. Manifest öncesi GC kilidi alınabilir. Snapshot hazırlığında maske ve WAL okuyucu açma vardır. | Manifest GC kilidi beklemesi, `.ready` yazma/sync/yeniden adlandırma, snapshot maskesi, `File::open`, snapshot yayımlama kilidi ve dosya tarama/silme ayrı. |
| Diğer beklemeler | Commitin dış saati donör okumayı ve staging’i de kapsar. İşçi süresi foreground duvar saatiyle örtüşebilir. | Donör okuma/staging, ölçülebilen builder/metadata kilit beklemeleri, kapsanmayan süre ve thread CPU. Ayrışmayan süre **bilinmiyor** olarak kalır. |

**Yazı yolunda doğrudan bağlantı var.** Hazır prefix kurulumu ve streamed
fallback sonu `start_compaction_mode(true)` çağırabilir. Bu işlev küçük WAL
katmanı açıkken ve normal merge seçilmemişken, reclaim açıksa
`start_revision_reclamation()` çağırır. `maintain_idle()` aynı karar yoluna
explicit bakım olarak girer. Dosya GC’si (`try_reclaim`) ayrıca çalışabilir
ve revizyon rewrite’ından farklıdır. Bu yolların varlığı, 3,94–4,59 s’lik
eski commitlerin hangi aşamada geciktiğini göstermiyor.
[Streamed commit](../src/engine/streamed.rs#L218) ·
[Prefix kurulumu](../src/engine/light_flush.rs#L102) ·
[Otomatik seçim](../src/engine.rs#L2631) ·
[Revizyon planlayıcı](../src/engine/revisions.rs#L95) ·
[Manifest ve dosya GC](../src/engine.rs#L2433) ·
[İşçi kurulumu](../src/engine.rs#L2783) ·
[Bakım](../src/engine.rs#L2884)

### Onay sonrası ölçüm önerisi

1. Aynı 881756DA… korpusun ilk 50.000 belgesi, aynı sayısal anahtar planı,
   1R+1W, 100’lük akışlı commit, reclaim açık ve tur arası aynı bakım. Yalnız
   mevcut sabit FDB test slotu, her kol için aynı temiz başlangıç. Bu yeniden
   kurulum slotun mevcut `6-off` durumunun yerine geçer. SQLite’ın ölçülmüş
   LIVE-DISK referansı tabloda kalır.
2. Tek geçici release ikilisinde profil kapalı/açık, **ABBA**. Her kol
   13 tur ve 1.300 commit, toplam **52 tur ve 5.200 commit üst sınırı**.
   10. turdaki proses geçişi korunur. Her tur en fazla 128 commit çerçevesi
   ve 512 kısa işçi kaydı. Ham yazım commit zaman aralığının dışında.
   45 dakika üst sınır, yeni geçici derleme için 1,5 GiB ve kanıt logları
   için 64 MiB planlama bütçesi. Slot koruması önceki 8 GiB/15 GiB kuralı.
3. Mevcut `WriteTrace.operation_sequence` ile dış commit zamanını eşle.
   `seq 693` (6. tur, 93. grup) ve `seq 1324` (13. tur, 24. grup) her
   kolda ayrı satır. Tüm **>1 s** commitler için sıra, işçi türü ve çıktı
   ID’si, toplam süre, kapsanan foreground süre ve kalan süre. İç içe
   fazlar özel süreye çevrilir. İşçi zamanı yalnız kanıtlı join beklemesi
   üzerinden foreground’a yazılır. Tarihsel iki uç yeniden oluşmazsa
   güncel uçlar açıklanır, tarihsel uçların nedeni **bilinmiyor** kalır.
4. Profil kapalı/açık p50/p95/p99/max ve >1 s sayısını birlikte ver.
   Her kolda tam GET/version ve 41-case oracle. Oracle hatasında, iz kaybı,
   eşleşmeyen commit veya üst sınırda dur ve ham kısmi raporu yaz. Ek tur
   ile bir ucu zorla üretme. Yeni kabul eşiği yok.

[Mevcut per-commit izleme](../src/engine/profile.rs#L13) ·
[WAL sync](../src/engine/streamed.rs#L150) ·
[Manifest sync](../src/manifest.rs#L177) ·
[Snapshot maske ve WAL hazırlığı](../src/engine/snapshot.rs#L655)

## 23 run’lı iki yerleşim — mevcut veri

| Yerleşim | Strict-open p50 | Manifest kaynak / indeks | Fiziksel kayıt | Eski sürüm slotu | Tombstone | Ölü payload | Posting ID adedi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Kapalı `1-off`, tur 10 | 529,74 ms | 430,61 / 341,36 MB | 150.000 | 75.000 | 25.000 | 257,44 MB | 71,19 milyon |
| Açık `3-on`, tur 30 | 280,63 ms | 196,08 / 164,94 MB | 60.297 | 6.597 | 3.700 | 22,58 MB | 32,71 milyon |

Kapalı tekrarın 23 run yerleşimi aynı kayıt ve bayt sayılarına sahipti,
açılış 509,44 ms. İki yerleşimde de açılış öncesi aktif delta boş. Eski
sürüm slotu, her anahtar için en yeni kayıt dışında kalan fiziksel
kayıtların toplamı. Örtüşen **benzersiz** anahtar sayısı veya heap’e
giren toplam slot sayısı bu kayıtlarda yok. [Özet](benchmarks/live-disk-50k-20261111/summary.json) ·
[Kapalı ham tur](benchmarks/live-disk-50k-20261111/1-off-verified-seed-churn-01-10.jsonl) ·
[Açık ham tur](benchmarks/live-disk-50k-20261111/3-on-churn-21-30.jsonl)

**Kanıt.** Aynı run sayısında açık kol daha az kaynak ve indeks baytı,
daha az kayıt, tombstone ve superseded slot taşıyor. **Kod ilişkisi.**
Strict run yükleme kaynak CRC/belge doğrulaması, indeks CRC ve dizin
çözümlemesi yapıyor. Dizin çözümlemesinin işi için terim/dizin kayıtları
daha doğrudan göstergedir. Tablodaki posting ID adedi, terim adedi değildir.
Açılış maskesi aralıkları kesişen run’ları heap’e alır, eşit anahtarın eski
slotunu gizler ve en yeni tombstone’u da görünürlük bariyeri olarak işler.
Fazla fiziksel kayıt bu işin ölçeğiyle ilgili bir göstergedir. **Bilinmiyor.**
249 ms farkın maske/run yükleme payı, gerçekten örtüşen benzersiz anahtarlar,
heap’in taradığı slotlar ve sayfa yerelliği. İki veri noktası farklı churn
turlarından. Bu veriler ayrı bir neden katsayısı veya faz payı üretmez.
[Strict run yükleme](../src/engine.rs#L151) ·
[Açılış maskesi](../src/engine/open_masks.rs#L34)

## Uzun snapshot — tek küçük deney önerisi

Snapshot bütün ilgili run/aktif WAL kimliklerini pinler. Revizyon seçimi ve
manifest kurulumu bu pinlerin bitmesini bekleyen bir koşula bağlı değil.
Emekli dosyalar GC’nin canlı kümesine eklenir ve son pin düşene kadar
silinmez. Son snapshot bırakıldığında bekleyen GC tekrar çalışabilir.
Aktif arama kopyalama pacing’ini etkileyebilir. Bir handle’ı açık tutmak
tek başına aktif arama sayacını sürekli artırmaz.
[Pin yayımı](../src/engine/snapshot.rs#L798) ·
[GC ve son bırakma](../src/engine/snapshot.rs#L108) ·
[Aktif okuma sayacı](../src/engine/maintenance.rs#L12)

**Ayrı onay için tek koşu.** Mevcut Cap200k ilk 10k, aynı FDB test slotu,
reclaim açık, **tek** snapshot 120 saniye tutulur. Snapshot’tan sonra
bir %20 churn turu ve bir `maintain_idle()` çağrısı. Aynı pin altında
5/30/120. saniyelerde retained disk, manifest canlı dosyaları ve artık
manifestte olmayan pinli kaynakların baytları kaydedilir. Emekliye ayırma
ve manifest install zamanı ile fiziksel silme ayrı. Snapshot bırakılınca
aynı dosyalar tekrar sayılır, cleanup süresi ve eski/yeni görünümün
doğruluğu kaydedilir. Üç zaman noktası aynı snapshot’a aittir. Üst sınır
8 dakika, bir koşu ve bir bakım çağrısı. 120 s içinde emekliye ayrılmış
dosya oluşmazsa pinin disk bedeli **bilinmiyor** olarak raporlanır.

## Geçici klasör envanteri

| Mevcut geçici klasör | Dosya boyutları toplamı |
| --- | ---: |
| `C:\Users\Serkan\AppData\Local\Temp\fdb-seg05-terms-20261111` | 897.217.804 B / 855,65 MiB |
| `C:\Users\Serkan\AppData\Local\Temp\fdb-open-met04-phase-20261110` | 153.487.117 B / 146,38 MiB |
| `C:\Users\Serkan\AppData\Local\Temp\fdb-strict-open-two-workers-20261109` | 538.874.417 B / 513,91 MiB |
| `C:\Users\Serkan\AppData\Local\Temp\fdb-strict-parallel-20261107-131120` | 620.099.198 B / 591,37 MiB |

7 Kasım raporundaki 569.256.286 B’lık `fdb-strict-open-20261107-123212`
yolu bu incelemede bulunamadı. Envanter salt okunur alındı. Tanı derlemesi
ve snapshot deneyi, kendi onaylarından sonra başlar.
