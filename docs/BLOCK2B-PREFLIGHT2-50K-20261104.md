# Blok 2b — ikinci sınırlı preflight, 4 Kasım 2026

**GERÇEK COLD KANITLANAMADI; DURULDU.** Yerel yardım/CLI ve SQLite önkoşulu
doğrulandı; hedef dosya resident tanığı alınamadı. Purge **0 / izinli en fazla1**,
ETW kaydı **0**, ana koşu **yok**. Bu sonuç cold performans kapısını KALDI veya
GEÇTİ yapmaz; o kapı **ÖLÇÜLMEDİ**.

Yeni kullanıcı onayıyla başlangıç **2026-11-04 16:37:56 UTC**, son sınır
**16:52:56 UTC** (900 s). Bu belge ana cold koşusu değildir. İlk
[preflight](BLOCK2B-PREFLIGHT-50K-20261104.md) kanıtları korunur; aşağıdaki
`cold-preflight2` dosyaları yeni pencereye aittir.

## Doğrulananlar

| Önkoşul | Bu penceredeki kanıt / sınır |
|---|---|
| Yerel araç | `C:/Users/Serkan/Downloads/RAMMap/RAMMap.exe`, sürüm **1.63**. Önceki metadata kontrolünde Microsoft imzası Valid. `Start-Process -Verb RunAs` ile açıldı; ana pencere `RAMMap64.exe`. UAC onayının kullanıcı tarafından hangi anda verildiği ölçülmedi. |
| Kendi yardım / CLI | `-?` ile **RAMMap Usage** açıldı ve ekrandan okundu. Aynı yerel exe'nin UTF-16 yardım kaynağı metin kanıtına alındı. İnternet bayrağı kullanılmadı. `-Es = Empty System Working Sets`, `-Et = Empty Standby List`; CLI otomasyon desteği **var**. Bu iki komutun başarılı purge çalıştırması henüz denenmiş değildir. |
| Gözlem otomasyonu | Yönetici ana pencerenin accessibility çıktısı **higher Windows integrity** uyarısı verdi. Help'e gönderilen tıklama menüyü açmadı. Ekran görüntüsü okunabiliyor; bu, giriş otomasyonunun çalıştığı anlamına gelmez. Kullanıcıdan File Summary'yi elle açması istendi. Yetki engeli aşılmadı, helper yükseltilmedi. |
| SQLite read-view | **GEÇTİ, yalnız önkoşul örüntüsü.** Node v25.8.1 / SQLite 3.52.0, readOnly + query_only. İki ardışık BEGIN/read/ROLLBACK; transaction durumları `false,true,false,true,false`. Sequence iki okumada **1026311**. Ana `.db` SHA256 önce/sonra **7cbc8d53491047b6189fc2c5eda8848ed06dd3759c9cd87fa6570cb6da6e70a1**. Gelecekteki Rust harness'inin nested transaction yapmadığı bu testle kanıtlanmaz. |
| Hedef envanteri | FDB **159 dosya / 467167854 B**, SQLite **3 dosya / 477941760 B**. Metadata listesi; resident byte ölçümü değildir. SQLite test/hash okumaları purge öncesi yapılmıştır. |
| WPR yeteneği | Yerel `-status`, `-help start`, `-profiledetails DiskIO.Light+FileIO.Light -filemode` tekrar alındı. **WPR is not recording.** DiskIO/Init, FileIO/Init, ProcessThread/Loader keyword'leri var. |
| WPR sınırları | File-mode iki collector, her biri 40 × 1024 KiB: toplam **80 MiB yapılandırılmış tampon**. ETL disk tavanı değildir. Yerel start yardımında sert ETL byte tavanı görülmedi. Gerçek hedef dosya adı/PID eşlemesi ve event kaybı **bilinmiyor**: kayıt bu onayın dışında, kayıt başlatılmadı. |
| Deneme purge / resident0 | **YAPILMADI / TANIK YOK.** Yönetici pencerenin File Summary görünümüne otomasyonla geçilemedi; elle açma isteği kapanışa kadar tamamlanmadı. Tüm159+3 dosyanın aktif/standby/modified sayfaları gözlenmedi; görünmeyen satırlar0 sayılmadı. Tanık alma yolu kurulamadan sistem-geneli cache boşaltılmadı. Bu, purge komutunun başarısız olduğuna kanıt değildir; komut denenmedi. |

DB API bakım/ingest/yazma çağrısı yok; FDB açılmadı. SQLite yalnız belirtilen
read-only önkoşul testiyle açıldı. `.db` içeriği aynı; boş WAL/SHM dosyalarının
metadata etkisi, mantıksal veri değişikliği olarak sunulmaz. Slot/model kaydı
değiştirilmedi. Kaynak kod ve mevcut dirty değişiklikler korunur.

## Elle adımlar ve ana koşu sınırı

**24 purge kesin elle gerekir iddiası yanlış olur:** yerel Usage CLI desteğini
doğruladı. Tasarımdaki tek purge dizisi iki ayrı çağrı (`-Es`, sonra `-Et`)
gerektirir; 24 purge için **48 RAMMap çağrısı**. Bunlar bu tur yürütülmedi.
Her `RunAs` başlatmasının UAC davranışı/purge wall süresi ölçülmeden otomatik
tamamlanır denemez; mevcut akışta 48'e kadar operatör UAC etkileşimi gerekebilir.
Yeni elevated launcher veya kalıcı task kurulmadı.

GUI fallback'te iki Empty seçimi en az **4 menü tıklaması/purge = 96 tıklama**.
Hedef filtreleme, refresh ve tanık kaydı ayrıca gerekir. Yerel File Summary
akışı tamamlanmadan bu ek adımların sayısı ve toplam süre **bilinmiyor**.
İlk rapordaki 30–90 s/purge ve 12–36 dk/24 purge sayıları **planlama tahmini**
olarak kalır; bu pencerede ölçülmüş kullanım süresi değildir.

Kapsam seçenekleri aynı kalır; karar kullanıcıda: 3 anchor ×4 çift ×2 motor
=24 purge; 3×2×2=12; 2×4×2=16; 2×2×2=8. Azaltılan tekrar/anchor kanıt kapsamını
daraltır; sessizce seçim yapılmaz. Saat **Open-start → ilk sonuç ≤1 s**;
30. sorgunun sıcak A/A yayılımıyla yakınsamasını sayısal kabul etme kararı
kullanıcıdadır. Hiçbiri bu preflight'ta ölçülmedi.

## Ham kanıt

- [Süre ve son durum](benchmarks/cold-preflight2-20261104-summary.json)
- [Yerel yardım kaynağı / ekranda doğrulanan Usage](benchmarks/cold-preflight2-20261104-rammap-embedded-help.txt)
- [Sürüm](benchmarks/cold-preflight2-20261104-rammap-version.json)
- [Yönetici pencerenin accessibility sınırı](benchmarks/cold-preflight2-20261104-rammap-ui.txt), [araç prosesleri](benchmarks/cold-preflight2-20261104-processes.json)
- [SQLite read-view / hash](benchmarks/cold-preflight2-20261104-sqlite-read-view.json)
- [Hedef dosya envanteri](benchmarks/cold-preflight2-20261104-file-inventory.json)
- [WPR status](benchmarks/cold-preflight2-20261104-wpr-status.txt), [yerel start yardımı](benchmarks/cold-preflight2-20261104-wpr-start-help.txt), [profil](benchmarks/cold-preflight2-20261104-wpr-profile.txt)
- [Kapanış WPR status: kayıt yok](benchmarks/cold-preflight2-20261104-wpr-final-status.txt)

## Durma / kalan riskler

Üst sınır dolana kadar tekrarlanan purge veya yeni gözlem aracı denenmedi.
Engel gözlem erişimidir; süre bitti veya CLI başarısız oldu denmez. RAMMap
ana/Usage pencereleri açık kaldı; kullanıcı başka iş yaparken körlemesine
kapatılmadı. Kullanıcı pencereyi kapatabilir. Kalıcı ayar/servis/task kurulmadı.

Devam için hedef-file resident tanığının elle alınabildiği bir yol gerekir.
Ayrıca ETW'nin gerçek file/PID eşlemesi, event kaybı ve bounded disk davranışı
bu kayıt-dışı yetenek kontrolüyle ispatlanamaz; bunlar ayrı kapsam kararı
gerektirir. Global standby azalması veya başarılı exit code cold kanıtı değildir.
İlk-son açılış/arama sayıları yok; iyi/kötü performans sonucu uydurulmadı.

**Ana koşu için ayrıca “başla” onayı gerekir; bu raporla başlamaz.**
24/16/12/8 purge kapsam seçenekleri kullanıcıdadır; hiçbirine geçilmedi.
3a KISMEN/özgün kapı ölçülmedi,2c/4 ve önceki yazma/arama/RAM-disk/eşzamanlı
yük açıkları değişmedi. Kaynak değişikliği olmadığı için cargo test/fmt/clippy
bu araç preflight'ında yeniden çalıştırılmadı; motor doğrulaması iddiası yok.

Kapanış UTC: **2026-11-04 16:47:47 UTC**. İşlevsel kontrol sonu16:47:32 UTC; başlangıçtan rapor kapanışına toplam **591.99 s /900 s**. Üst sınır aşılmadı; erişim engeli nedeniyle erken duruldu.
