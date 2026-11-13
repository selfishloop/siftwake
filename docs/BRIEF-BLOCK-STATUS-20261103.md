# İlk sağlamlaştırma brifine göre blok durumu — 3 Kasım 2026

**7 Kasım ETW tasarım teklifi.** 0 purge ve 0 ETW'li ön test kullanıcıca
kabul edildi. Yeni ön test tek purge ve tek izde sıcak/cold okuma
eşleşmesini sınar. Sekiz ana deneme aynı purge + ETW kuralına bağlıdır.
Ön test tasarım onayı ve ayrı ana “başla” onayı bekleniyor.
[ETW tasarımı](BLOCK2B-COLD-ETW-DESIGN-50K-20261107.md)

**7 Kasım güncel durum.** Dar 2b tasarımı onaylandı. Küçük canlı kaynak
okundu. Pozitif RAMMap tanığı tamamlanmadığı için purge 0 ve ETW 0 ile
ön test durdu. Ana ölçüm **BAŞLAMADI**.
[Ön test raporu](BLOCK2B-COLD-PREFLIGHT-50K-20261107.md)

**Son kullanıcı kararı / tasarım:** RAMMap negatif görüntüsü o an için kabul;
önceki purge anına taşınmaz. [Tek-anchor8-trial teklifi](BLOCK2B-COLD-CACHE-REDUCED-DESIGN-50K-20261104.md)
ve tek pozitif kontrol tasarlandı;40dk, preflight dahil9 purge/9 trace,
uygulama **onay bekliyor**. ETW gerçek eşleme/kayıp önkoşulu açık;2b ana
ölçüm **BAŞLAMADI**. Diğer blok durumları değişmedi.

**Son ek preflight:** [5dk raporu](BLOCK2B-PREFLIGHT3-50K-20261104.md).
Tek purge dizisi1/1, parent exit0; resident0 kanıtı yok.2b ana ölçüm
BAŞLAMADI/kanıt eksik; eski purge0 satırları tarihsel, güncel toplam1.

**4 Kasım ikinci preflight:** [yeni rapor](BLOCK2B-PREFLIGHT2-50K-20261104.md).
RAMMap1.63 kendi Usage/CLI desteği doğrulandı; yüksek yetkili ana pencereye
otomasyon girdisi işlemiyor, resident0 tanığı yok. Purge0/1, ETW kaydı0;
gerçek cold kanıtlanamadı, ana2b başlamadı. SQLite view önkoşulu tekrar geçti.

**4 Kasım preflight sonucu:** [2b önkoşul kontrolü](BLOCK2B-PREFLIGHT-50K-20261104.md)
tamamlandı; gerçek cold kanıtlanamadı. Purge/ETW kaydı/ana koşu yok; ana
ölçüm başlamadı. Saat/kapı tanımları aynı; ayrıca “başla” onayı beklenir.
RAMMap1.63 yolu sonradan bulundu ve Microsoft imzası doğrulandı; ilk süre
penceresi dolduğundan kendi yardım/CLI/resident0 kontrolü henüz yapılmadı.

**4 Kasım güncellemesi:** [Blok4 karar tablosu](BLOCK4-DECISIONS-20261104.md)
ve [2b tasarımı](BLOCK2B-COLD-CACHE-DESIGN-50K-20261104.md) tamamlandı;
yalnız belge. Varsayılan değişmedi,2b ölçümü başlamadı. 3a KISMEN/özgün kapı
ÖLÇÜLMEDİ olarak korunur; yeni3a turu yok. Ayrı uygulama onayı beklenir.

İlk sürüm yalnız belge incelemesiydi. Sonraki kullanıcı onayıyla 3a'nın mevcut
50k tanısı 131,430 s'de harness hatasıyla yarım kaldı; [tanı raporu](BLOCK3A-DIAGNOSIS-20261103.md).
Ardından yalnız [çalışma alanı onarımı](BLOCK3A-REPAIR-20261103.md) tamamlandı:
izinli16 kayıt doğrulanıp silindi, iki slot ready. Tanı A/A'sı tamamlanmadı;
bakım ve yeni ölçüm yapılmadı. Sonraki blok2b/2c/4 kararı kullanıcıya ait.
Özgün 10k/38-case kabul kapısı ölçülmedi; sonraki yetki yalnız4 karar/2b tasarımdır.
3a–4c tanımları kullanıcının 3 Kasım'da yeniden verdiği asıl metne dayanır.
KAPANDI, belirtilen kapsamın kullanıcı kabulüdür; sonraki ölçeğin garantisi
değildir. Kullanıcı kabulü ham **KALDI** sonucunu GEÇTİ yapmaz. Ölçülmeyen
kapı KALDI diye etiketlenmez.

| Madde / brifteki iş | Durum | Dayanak rapor | Kalan kapı / sınır |
|---|---|---|---|
| **1a — 513. işlem / flush gecikmesi** | **KAPANDI, 10k** | [WRITE-HARDENING-GATES](WRITE-HARDENING-GATES-10K-20261027.md), [WAL-SEQUENCE](WAL-SEQUENCE-PROFILE-10K-20261027.md) | Kabul edilen koşularda >50 ms yok. Eski 419 ms tekrar oluşmadı; nedeni bilinmiyor, çözüldü denmez. |
| **1b — yazma throughput** | **KAPANDI, 10k** | [GATES](WRITE-HARDENING-GATES-10K-20261027.md), [HUNDRED-PAIR](HUNDRED-PAIR-10K-20261027.md) | Sonradan onaylı ≥%75 ve SQLite pooled üstü geçti: tekli %77,22; 100'lü tek çift %76,59. 50k sonuçları ayrı, 2a'da. |
| **1c — sürekli churn / bakım ilerlemesi** | **KAPANDI, gözlenen 30 tur** | [RECLAIM-IDLE30](RECLAIM-IDLE30-10K-20261027.md) | Disk max 1,199×, son 1,107×; ölü payload son 29,75 MB. Reclaim opt-in; `maintain_idle()` + 2 s boşluk, bakımın 2 s'de bittiği anlamına gelmez. |
| **1d — tombstone / kill artıkları** | **KAPANDI, 10k** | [GATES](WRITE-HARDENING-GATES-10K-20261027.md), [RECLAIM-IDLE30](RECLAIM-IDLE30-10K-20261027.md) | Tombstone tepe 1201 → son 600; artık 0 B, pin/eski okuyucu/kill geçti. Sonsuz kullanım garantisi değil. |
| **2a — 50k/100k ölçek ve yazma** | **KISMEN** | [CAP200K-SCALE](TRWIKI-CAP200K-SCALE-20261029.md), [WRITE-FOLLOWUP](WRITE-FOLLOWUP-50K-20261029.md), [WRITE-REVISED](WRITE-REVISED-PROFILE-50K-20261029.md), [Ek-TC](TERM-CACHE-50K-20261102.md) | 50k/100k okuma/açılış/ingest/RAM/iptal kabul; 50k tekli ve dirty500 kabul. Normal 41 ve Frunner görünürlük **KALDI**; kullanıcı ilerleme kabulü eşikleri değiştirmedi. 4R+100'lü son kayıt 16/41 **KALDI**; yeni varsayılanla tekrarı/profili ve 500 drain tanısı tamamlanmadı. 100k yazma/dirty/eşzamanlı yük ölçülmedi. |
| **2b — gerçek soğuk cache** | **KISMEN, FDB geçti, SQLite izi geçersiz** | [İki çift](BLOCK2B-COLD-2S-PAIRED-50K-20261108.md), [32 MiB tekrar](BLOCK2B-COLD-2S-RETEST-50K-20261108.md), [Önceki geçerli FDB](BLOCK2B-COLD-SHORT-MAIN-50K-20261108.md) | İki çift 2/4 başlatıldı, 1/4 teknik geçerli ve 1/4 süre geçişi. FDB 1.868,88 ms ≤2.000, 54/54 FileIO ve DiskIO. Q30 131,50/102,81 ms, yakınsama kullanıcı kararı. SQLite ham 4.622,54 ms, 56 MiB collector doldu, birleşik iz yok. Ters çift durdu. İki motor oracle/kill/reopen, FDB eski okuyucu ve ana SHA geçti. |
| **2c — 10 dk snapshot + 10 tur churn** | **BAŞLAMADI** | [SNAPSHOT-CONCURRENT](SNAPSHOT-CONCURRENT-10K-20261026.md) yalnız kısa pin kanıtı | Disk ≤2× canlı, RAM, azami ömür/zorla bırakma API'si ölçülmedi/tamamlanmadı. Kısa pin testleri bunun yerine geçmez. |
| **3a — zx/the zx ve parçalı payload** | **KISMEN, tanı yeterli sinyal verdi** | [REVISION-VISIBILITY](REVISION-VISIBILITY-SEARCH-10K-20261026.md), [50k tanı](BLOCK3A-DIAGNOSIS-20261103.md), [onarım](BLOCK3A-REPAIR-20261103.md) | **Özgün kapı (zx/the zx ≥%30, diğer36 ±%5 + A/A) ÖLÇÜLMEDİ.** Gap/run ve adjacency sinyal, kanıt değil. Önceki harness KALDI kaydı korunur; yalnız havuz onarımı GEÇTİ, slotlar ready. Yeni ölçüm/optimizasyon yok; eşikler aynı. |
| **3b — ishmael /130/1; quixote whale; the /5000/50** | **KISMEN** | [CONSOLIDATED](CONSOLIDATED-COMPARISON-10K-20261024.md), [SNAPSHOT-CONCURRENT](SNAPSHOT-CONCURRENT-10K-20261026.md) | İzlenen sapmalar var; hedefli neden/uygun düzeltme veya gerekçeli kapatma tamamlanmadı. Eski kitap case34, TR case34 değildir. |
| **4a — küçük WAL katmanı varsayılan kararı** | **KISMEN; karar tablosu hazır** | [Blok4](BLOCK4-DECISIONS-20261104.md), [GATES](WRITE-HARDENING-GATES-10K-20261027.md) | Genel KAPALI önerisi/Frunner opt-in adayı; default ON onayı yok. Güncel100'lü/fallback/signed-zero ve default kapsamı eksik. Normal WAL açık; yeni sayısal kapı yok. |
| **4b — revizyon geri kazanma varsayılan kararı** | **KAPANDI: arka plan reclaim varsayılan AÇIK** | [12 Kasım kararı ve doğrulama](BACKGROUND-RECLAIM-DEFAULT-50K-20261112.md) | 50k/30 tur, iki saniye yazmasız pencerelerle ≤1,3× **GEÇTİ**. Uzun snapshot, kesintisiz yazı ve gerçek Frunner anahtarıyla disk sınırı açık. 100'lük commit uçları ayrıca açık. |
| **4c — Owned/Vec RAM / API kararı** | **KISMEN; öneri/validasyon listesi hazır** | [Blok4](BLOCK4-DECISIONS-20261104.md), [OWNED-MEMORY](OWNED-MEMORY-10K-20261020.md), [DEFAULT-PROFILE](DEFAULT-PROFILE.md) | Azaltma mümkün fakat güncel kabul kanıtı yok; imkânsız denmez. Frunner yalnız-akışlı işareti öneri, henüz API belgesine uygulanmadı; gerçek giriş yolu, atomiklik ve RAM ayrımı doğrulaması/onay bekler. |

**Numaralama düzeltmesi:** Brifteki **Blok 2 = 2a ölçek, 2b soğuk cache,
2c uzun snapshot**; genel durumu KISMEN. `VISIBILITY-PHASE` bundan böyle
**Ek-VP** (2a görünürlük tanısı), `TERM-CACHE-DESIGN`/`TERM-CACHE` ise **Ek-TC**
(2a takip deneyi, 3a'ya tanı girdisi) olarak adlandırılır. Ek-TC kapandı,
A/D varsayılan olmadı. README, ARCHITECTURE, OPEN-WORK ve bu iki cache belgesindeki
“Blok 2 kapandı” ifadeleri bu anlamda düzeltilmiştir. Ham benchmark kapanış
kaydı/üretici araçlardaki tarihsel adlandırma korunur; brif durumunun kaynağı
bu tablodur. Son turlardaki A/B/C/D/E de yerel 2a takip işaretleridir.

Blok 4'ün bütünü kapanmadı; bu tablo varsayılan değiştirme onayı değildir.
C ABI ayrı yazılı gereksinim bekler. Açık yazma/bakım, arama, RAM/disk ve
eşzamanlı yük işleri yukarıdaki kapsamlarıyla korunur.
