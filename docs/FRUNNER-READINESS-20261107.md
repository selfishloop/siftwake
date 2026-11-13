# Frunner hazırlık tablosu — 7 Kasım 2026

**Rust motoru için pilot kanıtı var. C ABI sözleşmesi ve dağıtım kararı açık.**
Aşağıdaki değerlendirme tek okuyucu, tek akışlı yazıcı, kısa snapshot ve
arka planda açılış varsayımlı sınırlı bir pilot içindir. «Bloklayıcı» sütunu
bu pilotun çalıştırılması için zorunlu eksikleri işaretler. Ürün garantileri
ve sonraki benchmark kapıları ayrı kalır. Tarihsel KALDI kayıtları korunur.

| Alan | Mevcut kanıt | Kalan karar veya sınır | Frunner tek okuyucu + tek yazıcı kullanımı bunu gerektirir mi? | C ABI/pilot için bloklayıcı mı? |
| --- | --- | --- | --- | --- |
| Akışlı bellek ve ölçek | Temiz Cap200k 50k idle/ingest tepe private **2,21/20,37 MiB**, 100k **3,70/28,91 MiB**. ≤15/60 MiB geçti. 50k motor-only D de geçti. [Ölçek](TRWIKI-CAP200K-SCALE-20261029.md) · [D](READ-ADMISSION-RAM-50K-20261030.md) | DLL içindeki ek OS private/working set ölçümü henüz yok. Owned/Vec için 80–292 MB gözlemi ayrı. | **Evet.** Tek yazıcıda da indeksleme Frunner sürecinin bellek bütçesini kullanır. | **Hayır** |
| Arama doğruluğu ve iptal | Cap200k 50k/100k 37/37 kabul sorgusu, 41-case oracle ve uzun belge iptali. AND, doğrulanmış alt dize, N=50 melez capped ve anahtar sırası. [Ölçek ve iptal](TRWIKI-CAP200K-SCALE-20261029.md) | İlk brifteki tüm dar sorgular için ≤30 ms @50k / ≤100 ms @100k ayrı kanıtlanmadı. Geniş 2 karakterli tarama ve 3a kapısı açık. | **Evet.** Kullanıcı yazarken bir sorgu bir sonrakine yer açmalı ve dönen eşleşmeler doğrulanmalı. | **Hayır** |
| Tekli yazma ve görünürlük | 50k tekli/mtime sil+ekle commit ve Frunner anahtar sırası geçti. Görünürlük p95 **20,15–26,75 ms**, ≤20 ms kapısı **KALDI**, kullanıcı ilerlemeyi kabul etti. [Frunner anahtarı](FRUNNER-KEY-50K-20261102.md) | İlk 50 payload farkı anahtar maliyetinden ayrılmadı. İlk sorgu, ölçülen görünürlük gecikmesinin büyük kısmı. [Görünürlük tanısı](VISIBILITY-PHASE-50K-20261102.md) | **Evet.** Dosya izleyicisinin ekleme, silme ve mtime değişiklikleri tutarlı bir commit olarak görünmeli. | **Hayır** |
| Eşzamanlı snapshot | Commit sınırı oracle, pin, kısa process-kill/reopen ve eski okuyucu kanıtı var. [Snapshot](SNAPSHOT-CONCURRENT-10K-20261026.md) | 50k normal 41-case kapısı **KALDI**, kullanıcı ilerlemeyi kabul etti. Bu kapı 4 okuyucu + 1 yazıcı içindi. [Normal41](HYBRID-DEFAULT-50K-20261102.md) | **Evet.** Tek okuyucu bile yazıcıyla örtüştüğünde tutarlı bir commit sınırı ve pinli kaynaklar gerekir. | **Hayır** |
| Açılış | Temiz 50k strict-open **310 ms p95**, dirty500 **291–292 ms**. 52 run churn yerleşiminde sıcak seri strict-open **1.275 ms p50**. [Temiz](TRWIKI-CAP200K-SCALE-20261029.md) · [Dirty500](WRITE-FOLLOWUP-50K-20261029.md) · [52 run](STRICT-OPEN-PARALLEL-50K-20261107.md) | Run sayısı ve yerleşim açılış işini etkiler. 52 run, tekrarlı havuz güncellemeleri sonrası görüldü. Canlı kullanımda eşiğe varma süresi ve üst sınır bilinmiyor. İlk brifteki arka plan doğrulamalı ≤0,5 s ürün sözleşmesi açık. [Yerleşim](TERM-CACHE-50K-20261102.md) · [Pending](BACKGROUND-OPEN-50K-20261028.md) | **Evet.** Servis açılırken indeks doğrulanmalı ve arayüzün ne zaman sonuç alabileceği belli olmalı. | **Hayır** |
| Soğuk ilk sonuç (2b) | Ana koşu 1/8 başlatıldı, **0/8 geçerli**. Ham FDB open-start→ilk sonuç 1.770 ms. İz collector sınırı ve 50/54 dosya eşleşmesi nedeniyle geçersizdi. [2b](BLOCK2B-COLD-ETW-MAIN-50K-20261107.md) | ≤1 s kapısı ve 30. aramada sıcağa yakınsama **ÖLÇÜLMEDİ**. | **Belki.** Servis arka planda önceden açılırsa kullanıcı soğuk ilk sonucu her oturumda beklemez. | **Hayır** |
| Disk ve bakım | Temiz 50k FDB **300,84 MB** / SQLite **421,74 MB** toplam, yaklaşık **0,71×**. İndeks bölümü **129,57 MB** / **157,15 MB** metin, yaklaşık **%82**, ilk %40 hedefinin üstünde. Reclaim opt-in 10k bakım protokolü son **1,107×**, max **1,199×**. [Ölçek](TRWIKI-CAP200K-SCALE-20261029.md) · [Metin baytı](TRWIKI-CAP200K-AUDIT-20261029.md) · [Blok 4](BLOCK4-DECISIONS-20261104.md) | Genel reclaim varsayılanı kapalı. Canlı Frunner 50k churn/pin disk tavanı ve sürekli yükte bakım ilerlemesi açık. Sınırlı pilotun disk sınırı ve durma politikası ürün kararında yazılmalı. | **Evet.** Tek okuyucu eski revizyonların birikmesini önlemez ve kendi snapshot’ıyla emekli dosyaları pinleyebilir. | **Hayır** |
| Derived profil ve API sözleşmesi | Strict CRC, hata yolları ve Rust snapshot/read/update/remove kanıtı var. [Mimari](ARCHITECTURE.md) · [Blok 4](BLOCK4-DECISIONS-20261104.md) | Derived profil, yeniden kurma sinyali, `doc_stat`/`list_doc_ids`, kısa terim hata kodu ve katlanmış ilk UTF-8 bayt konumu ürün sözleşmesi olarak doğrulanmadı. Ters-mtime anahtar sırası ve önceki puan sırasının önceliği yazılmalı. | **Evet.** Dosya uzlaştırma ve bozulmadan sonra yeniden kurma, okuyucu sayısından bağımsız sözleşmelerdir. | **Evet** |
| C ABI ve dağıtım | Motor kanıtı Rust seviyesinde. [Brif durum tablosu](BRIEF-BLOCK-STATUS-20261103.md) | Yazılı sözleşme ve uygulama yetkisi açık. DLL/CRT, ABI ve Frunner içi ölçüm henüz yok. | **Evet.** Aynı süreçte P/Invoke için yaşam döngüsü, hata, tampon ve dağıtım sözleşmesi gerekir. | **Evet** |
| Uzun snapshot (2c) | Kısa snapshot pin ve son bırakma kanıtı var. [Snapshot](SNAPSHOT-CONCURRENT-10K-20261026.md) | 10 dk snapshot + 10 tur churn, ≤2× canlı disk ve azami ömür/zorla bırakma **ÖLÇÜLMEDİ**. [Brif](BRIEF-BLOCK-STATUS-20261103.md) | **Belki.** Tek okuyucu snapshot’ı çağrı sonunda bırakırsa bu yük beklenmez, ancak uzun ömürlü bir handle aynı riski doğurur. | **Hayır** |
| İzlenen sorgular (3b) | Eski kitap case34 `ishmael /130/1` ile 4R+yazıcı `quixote whale` ve `the /5000/50` izleniyor. [Brif](BRIEF-BLOCK-STATUS-20261103.md) · [Snapshot](SNAPSHOT-CONCURRENT-10K-20261026.md) | Neden ve özgün 3b kapanışı açık. Eski case34 ile TR case34 ayrı sorgular. | **Hayır.** Eski sayfalama ve dört okuyucu senaryosu tek okuyuculu capped ilk sayfa pilotunun doğrudan yükü değil. | **Hayır** |
| 100’lük / 500 drain / 2,6 s uç | Eski 4R+100 koşusu **16/41 KALDI**, commit p95 **35,49–37,72 ms**, max **2,60–2,67 s**. Ayrı 500’lük koşuda bazı uçlarda önceki worker drain **1,2–1,3 s** ölçüldü. [100’lük](WRITE-REVISED-PROFILE-50K-20261029.md) · [500’lük](WRITE-PROFILE-50K-20261029.md) | Yeni varsayılanla 4R+100 tekrarı ve uç profili açık. 2,6 s ucunun nedeni ve 500’lük drain ile ilişkisi bilinmiyor. | **Belki.** Tek yazıcı dosya olaylarını büyük gruplarsa veya ilk indekslemede kullanıcı arama yaparsa bu kuyruklar önemli olur. | **Hayır** |
| SQLite WAL/SHM kaybı | Geçersiz 2b koşusu sonunda boş WAL ve 32.768 B SHM eksikti. SQLite ana DB SHA ve oracle geçti, fiziksel envanter **KALDI**. Eski `scale-first-oracle` doğrulayıcısı SQLite’ı read-write açtı. [Olay kaydı](BLOCK2B-COLD-ETW-MAIN-50K-20261107.md) · [Doğrulayıcı](../tools/native_sqlite/src/bin/content_churn.rs#L191) | Yan dosyaların hangi adımda kaybolduğu bilinmiyor. Kayıt SQLite karşılaştırma düzeneğine ait. | **Hayır.** Frunner pilotu FDB çalıştırır, SQLite dosya envanteri karşılaştırma kanıtının bütünlüğünü etkiler. | **Hayır** |

### Run sayısı ve 52 run yerleşiminin kapsamı

**Gözlem.** Aynı mantıksal 50k etrafındaki terim cache deneyinde canlı run
sayısı başlangıçta **44**, normal bloklar sonrasında **7**, tekrarlı yazma
havuzu sonrasında **52** idi. 52 run’lık slot daha sonra strict-open
tanısında 1.275 ms sıcak seri p50 üretti. Temiz 310 ms değeri farklı
yerleşim ve oturumda ölçülen p95. Bu iki değer, run sayısının etkisini
tek başına ayırmaz. Açılışta her eski run kendisinden daha yeni run’larla
aralık kontrolü yapar. 52 run’da **1.326** çiftin **697’si** bu kontrolden
geçti. [Run gözlemi](TERM-CACHE-50K-20261102.md) ·
[Maske ölçümü](STRICT-OPEN-MASK-WPR-50K-20261107.md)

**Koddan çıkarım.** Tek yazıcı, ardışık flush’larla bu tür bir yerleşime
ulaşabilir. Reclaim kapalıyken olağan compaction çalışmaya devam eder.
Otomatik seçim dört ardışık ve benzer run ister, toplam **100.000 anahtar
ve 64 MiB** sınırı uygular. Uygun pencere bulunmazsa veya merge geriden
gelirse yeni run yayımları sayıyı artırabilir. Explicit idle bakımında
olağan compaction daha küçük grupları da seçebilir. Compaction da ilerlemez
ve reclaim kapalı kalırsa, birleşme olmadan her yayımlanan yeni run canlı
listeye bir giriş ekler. Bu modelde **52 aşılabilir**. Manifestin 16 MiB
serileştirme tavanı bir dosya boyutu kısıtıdır. Frunner’daki sürdürülebilir
run üst sınırı, 52’ye kaç gün veya kaç dosya olayıyla ulaşılacağı ve bu
yerleşimin ne sıklıkta oluşacağı **bilinmiyor**. [Merge seçimi](../src/engine.rs#L156) ·
[Flush yayımı](../src/engine/light_flush.rs#L162) · [Idle bakım](../src/engine.rs#L2832) ·
[Manifest sınırı](../src/manifest.rs#L183)

### Açılış seçeneklerinin kapanış kaydı

| Seçenek | Kanıt veya önceki tahmin | Karar |
| --- | --- | --- |
| Paralel maske, 2/4/8 işçi | Byte-eşit. Eşleşmiş açılış farkı **+258 / +907 / +908 ms**, her kolda **0/12** kazanım. `PagedRecords::page` global mutex’i en güçlü açıklama adayı, bekleme payı ölçülmedi. [Deney](STRICT-OPEN-PARALLEL-50K-20261107.md) | **KAPATILDI** |
| Mutex bölme, tembel/artımlı maske ve türetilmiş sidecar | Hafif maske p50 yaklaşık **0,76 s**, 52 ham bitset **10.528 B**. Bunlar önceki seçeneklerin büyüklük girdileri. [Hafif maske](STRICT-OPEN-MASK-WPR-50K-20261107.md) | **KAPATILDI**, yeni açılış deneyi açılmayacak |

## C ABI’ye geçişe hazır mı?

**[KANIT]** Rust motorunda akışlı yazma, doğrulanmış arama, iptal ve kısa
snapshot için pilotu destekleyen ölçümler var. **[SİNYAL]** Yazılı
sözleşme geldiğinde C ABI tasarımına geçilebilir, çalışan DLL pilotunun
bloklayıcıları derived/API sözleşmesinin kararı ve DLL’in
uygulanıp doğrulanmasıdır. **[BİLİNMİYOR]** DLL içindeki ek RAM, canlı
kullanımda run/disk üst sınırı ve geçerli soğuk ilk sonuç henüz
kanıtlanmadı. **[SİNYAL]** Yazacağın gereksinim listesinde opak handle
ve snapshot ömrü, P/Invoke, panic/close ve iptal, UTF-8 tampon ve
uzunluk, ABI sürümü, statik CRT, DLL içi RAM, derived profil, yeniden
kurma sinyali, `doc_stat`/`list_doc_ids`, kısa terim hata kodu, sonuç
sırası ve konum birimi ayrı başlıklar olsun. **[KANIT]** C ABI çalışması
başlamadı.

## Kapı değişikliği önerileri — kullanıcı kararı bekliyor

- **Pilot eşzamanlılık kapsamı.** Tek okuyucu + tek yazıcıyı ayrı bir pilot
  kapısı olarak tanımla ve mevcut sorgu p95 kuralını aynen uygula.
  4R+W kaydı ve KALDI sonucu ürün kapsamı için ayrı kalsın.
- **Açılış kapsamı.** Arka planda açılan pilotta arayüzün hazır oluşu
  ve doğrulanmış ilk sonucu ayrı değerlendir. Ürün için ≤0,5 s
  sözleşmesinin hangi olayda bittiğini yazılı gereksinimde kararlaştır.
  2b’nin Open-start→ilk sonuç ≤1 s kapısı ve 30. arama yakınsama kararı
  aynı kalsın.
- **Yazma ve bakım kapsamı.** Pilotun grup boyutu ve snapshot ömrünü
  sözleşmede sınırla. 100’lük, 500 drain ve 2c kapılarını bu yükler
  pilotta kullanılacaksa pilot önkoşulu yap. Görünürlükte ≤20 ms KALDI
  kaydını ve gözlenen bakım disk sınırının kapsamını koru.

Bu bölüm öneri kaydıdır. Eşikler, kapılar, varsayılanlar ve mevcut
GEÇTİ/KALDI kararları aynı.
