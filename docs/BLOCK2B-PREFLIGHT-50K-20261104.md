# Blok 2b preflight — 4 Kasım 2026

**Sonradan gelen yerel yol:** kullanıcı `C:/Users/Serkan/Downloads/RAMMap`
konumunu verdi.16:24:23 UTC'de yalnız dosya metadata'sı incelendi:
RAMMap.exe1.63, Microsoft imzası Valid. [Metadata kanıtı](benchmarks/cold-preflight-20261104-rammap-late-path.json).
İlk preflight'ın15:49:05–16:04:05 UTC süre penceresi geçmişti; executable
başlatılmadı, yardım/otomasyon denenmedi, purge yapılmadı. Bu ek bilgi
aşağıdaki tarihsel sonucu yeniden ölçüm veya cold kanıtı yapmaz. Kalan RAMMap
kontrolleri için yeni sınırlı preflight süresi onayı gerekir.

**GERÇEK COLD KANITLANAMADI; DURULDU.** Ana koşu, reboot ve ETW kaydı
başlatılmadı. Purge0/izinli en fazla1. Yeni araç/özel tracer yazılmadı,
RAMMap indirilmedi. İşlevsel kontroller18:49:05–18:58:20 (Türkiye),555 s;
onaylı üst sınır900 s. Rapor kapanış tanığı dahil kayıtlı toplam777 s
(12 dk57 s); sınır aşılmadı. Kontrollerden sonra yalnız rapor/durum kaydı yazıldı.

| Kontrol | Sonuç / sınır |
|---|---|
| Yerel RAMMap sürümü/kendi yardımı | **DOĞRULANAMADI:** aranan konumlarda exe/zip/chm bulunamadı; yerel yol kullanıcıya soruldu. Sürüm/CLI/otomasyon bilinmiyor; internet purge bayrağı denenmedi. |
| Yönetici kullanım | Kullanıcı onayı var; mevcut shell token'ı elevated değil. RAMMap bulunamadığından UAC/elevated RAMMap başlatılmadı; yetki reddi yaşandı diye sunulmaz. |
| WPR mevcudiyeti | **DOĞRULANDI:** `C:/Windows/System32/wpr.exe`,10.0.26100.9711; yardım/profil/status/ayrıntı ve built-in profil export'u alındı. Başlangıçta kayıt yoktu. |
| File/PID eşlemesi | **Yalnız yapılandırma:** DiskIO/Init, FileIO/Init, ProcessThread/Loader keyword'leri var. Gerçek hedef dosya adı/PID korelasyonu ETW kaydı olmadan doğrulanmadı. |
| Bounded toplama / event kaybı | **DOĞRULANMADI:** file-mode iki40×1024 KiB collector =80 MiB tampon ayarı; bu ETL disk tavanı değildir. Kayıt olmadığından lost-event=0 tanığı yok. |
| Tek purge / resident0 | **YAPILMADI / TANIK YOK:** RAMMap yok; ölçülemeyen resident sayfa0 diye etiketlenmedi. |
| SQLite tek eşzamanlı read-view önkoşulu | **GEÇTİ, dar kapsam:** Node25.8.1 /SQLite3.52.0, readOnly+query_only. İki ardışık BEGIN/read/ROLLBACK; durum false→true→false→true→false, sequence1026311 sabit, `.db` SHA önce/sonra aynı. Gelecekteki Rust/rusqlite2b harness'i doğrulanmış değildir. |

Yerel arama PATH, Downloads/Documents/Desktop, Program Files, AppData,
WinGet/Scoop, Codex, yaygın Tools/Sysinternals konumları, D:/Vibeprojects ve
Sysinternals Appx/Uninstall kayıtlarını kapsadı. Bazı korumalı WindowsApps
alanlarına erişim yoktu; makinenin her yerinde RAMMap yoktur iddiası değildir.

## WPR ve dosya envanteri

WPR'nin **kendi** `-help profiles/start/status` çıktısı kullanıldı. Kayıt
başlatmadan `DiskIO.Light+FileIO.Light` detayları/export'u alındı; yeni profil
tasarlanmadı. Light preset ayrıca antimalware ve başka sistem provider'ları
da içeriyor; yalnız hedef DB dosyalarını kaydettiği iddia edilmez. Memory-mode
ayrıntısı1566+176 adet1024 KiB tampon, yaklaşık1742 MiB gösterdi. Bunlar
**konfigürasyon değerleri**, tahsis edilmiş bellek ölçümü değildir. Büyük
memory-mode, OS-cache deneyini etkileyebilir; “bounded” olması yeterli değildir.
Yerel start yardımında ETL için sert byte tavanı görülmedi. Watchdog ile
durmanın overshoot'u ve event kaybı gerçek kayıt olmadan test edilmedi.

Envanter FDB159 dosya/467167854 B, SQLite3 dosya/477941760 B içeriyor.
Bu mevcut dosyaların metadata listesidir, yeni veri/DB kopyası değildir.
SQLite testinden sonra `.wal`0 B, `.shm`32768 B envanterde; bunlar silinmedi.
Önceki metadata listesi alınmadığından yaratıldı/güncellendi ayrımı yapılmadı.
Ana `.db` SHA değişmedi. SQLite testi cache'i ısıtabilir; purge yapılmadığı
için bundan cold tanığı çıkarılamaz. FDB DB API'si açılmadı. Slot/model kaydı
değiştirilmedi; full50k oracle veya performans ölçümü bu preflight'ın parçası değil.

ETW kaydı bu onayın **dışındaydı**. Bu nedenle gerçek file/PID eşlemesi,
event kaybı ve log üst sınırı bu tur ispatlanamaz; profil keyword'leri veya
“WPR is not recording” çıktısı bu kanıtların yerine geçmez. Yeni araç yazıp
veya izinsiz kısa trace başlatıp bu eksik kapatılmadı.

## Elle işlem ve kapsam seçenekleri — karar kullanıcıda

**CLI desteği bilinmediği için “24 purge kesin elle gerekir” sonucu yok.**
Yalnız GUI gerekirse kabul edilmiş tasarımdaki iki Empty seçimi için en az
4 tıklama/purge,24 için96 tıklama gerekir. Resident tanığı/refresh/kayıt ile
yaklaşık6–8 UI adımı/purge (144–192 adım) planlama tahminidir; yerel RAMMap
görülmediği için doğrulanmış kullanım sayısı değildir. Tahmini30–90 s/purge
ile24 purge12–36 dakika; ilk açılış/UAC, büyük RAM snapshot beklemesi ve
zor dosya eşlemesi bunu artırabilir. Bu **ölçülmüş süre değil**.

| Seçenek | Cold purge | Warm trial | Zamanlı sorgu | Tahmini elle purge süresi |
|---|---:|---:|---:|---|
| Mevcut3 anchor ×4 çift ×2 motor | 24 | 24 | 4320 | 12–36 dk |
| 3 anchor ×2 dengeli çift | 12 | 12 | 2160 | 6–18 dk |
| 2 anchor ×4 çift | 16 | 16 | 2880 | 8–24 dk |
| 2 anchor ×2 dengeli çift | 8 | 8 | 1440 | 4–12 dk |

Seçeneklerde cold30/warm30+ikiN60 ve kapı saatleri aynı kalır; doğrulama
sorguları tabloya dahil değildir. İki çiftte cold tekrar sayısı motor/anchor
başına2 olur, tail kanıtı zayıflar. Anchor azaltılırsa dışarıda bırakılan sorgu
test edilmiş sayılmaz; özellikle zx'nin sessizce çıkarılması yoktur. Kapsam
kendiliğinden küçültülmedi. Bunlar RAMMap ve ETW önkoşullarının yerine geçmez.

Devam için yerel RAMMap yolu/yardımı ve resident tanığı, ayrıca ETW doğrulama
kapsamının kullanıcıca kararlaştırılması gerekir. **Ana koşu için seçilen
kapsamla ayrıca “başla” onayı istenir.** Bu preflight ana koşuyu yetkilendirmez;
başka purge, araç indirme/yazma, reboot veya3a/2c/4/C ABI'ye geçilmedi.
Açılış dahil ilk sonuç≤1 s aynı; yakınsamanın sayısal kabulü kullanıcıdadır.

Kanıt: [özet/süre](benchmarks/cold-preflight-20261104-summary.json),
[yerel WPR yardımı](benchmarks/cold-preflight-20261104-wpr-help.txt),
[profil detayları](benchmarks/cold-preflight-20261104-wpr-profile-details.txt),
[built-in profil export'u](benchmarks/cold-preflight-20261104-wpr-export.wprp),
[memory-mode detayları](benchmarks/cold-preflight-20261104-wpr-memory-details.txt),
[SQLite read-view/SHA](benchmarks/cold-preflight-20261104-sqlite-read-view.json),
[hedef dosya listesi](benchmarks/cold-preflight-20261104-file-inventory.json),
[arama kapsamı hataları](benchmarks/cold-preflight-20261104-rammap-search.stderr).
