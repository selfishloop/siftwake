# Blok 2b ana koşu planı — 7 Kasım 2026

**Ana koşu için “başla” onayı bekleniyor.** Kullanıcı ön testte sıcak DiskIO
0 B, purge sonrası FDB 2.048 B ve SQLite 65.536 B, dosya/PID eşlemesi ve
üç exit 0 sonucunu kabul etti. Yeni kayıp olay kuralı birleşik ETL tracerpt
toplamı 0 ve beklenen okumaların eksiksiz olmasıdır. Her ana satırda
**collector bazı ölçülemedi** yazılır.
[Kabul edilen ön test](BLOCK2B-ETW-LINK-PREFLIGHT-50K-20261107.md)

## UTC penceresi ve matris

Kullanıcının ayrı “başla” mesajında yeni başlangıç UTC `T0` yazılır. Ana
deadline UTC `T0 + 30 dk`. Ön teste ayrılan 10 dk ile toplam bütçe 40 dk.
Bekleme ve yönetici komut süresi ana pencereye dahildir.

| UTC aralığı | İş |
|---|---|
| T0 → T0 + 17 dk | Hazırlık, ön doğrulama ve en fazla sekiz eşlenmiş cold/warm deneme |
| T0 + 17 dk → T0 + 25 dk | Ayrılmış son oracle, CRC, kill/reopen ve eski okuyucu doğrulaması |
| T0 + 25 dk → T0 + 30 dk | Rapor ve kapanış |

Başlatma sırasında kayıt, merge ve çözümlemeye yetecek süre ayrılır. Kalan
süre bir sonraki denemeyi ve kapanış rezervini karşılamıyorsa eksik rapor
üretilir. Deneme başına ETW en fazla 60 s. Ön test dahil en fazla 9 purge ve
9 ETW oturumu.

| Çift | Sıra | Cold / warm |
|---|---|---:|
| 1 | FDB → SQLite | 2 / 2 |
| 2 | SQLite → FDB | 2 / 2 |
| 3 | SQLite → FDB | 2 / 2 |
| 4 | FDB → SQLite | 2 / 2 |

Yalnız TR case0 `olduğu`. Mevcut 50k slotları, aynı ayarlar, akışlı API ve
normal strict open. Her denemede cold30, warm30 ve warm A/A için 2×N60.
Toplam 1.440 zamanlı sorgu ve 16 ölçülen open. Doğrulamayla birlikte üst sınır
1.768 sorgu ve 400k GET. Önceden belirlenen digest ile her zamanlı sonuç
doğrulanır. Oracle ve ısıtma işlemleri cold açılışın dışında tutulur.

Kapı **Open-start → ilk sonuç ≤ 1 s**. Açılış ve ilk sorgu ayrı yazılır.
Q30 ve sıcak A/A ham süreleri yan yana verilir. Yakınsamanın sayısal kabulü
kullanıcıya aittir. [Önceki matris ve doğrulama ayrıntısı](BLOCK2B-COLD-CACHE-REDUCED-DESIGN-50K-20261104.md)

## Yönetici PowerShell betiği

Betik `tools/run-fdb-cold-etw.ps1`. Kullanıcının açık PowerShell 7.5+
yönetici penceresinde her hazır deneme için aynı tek satır kullanılır.
Yönetici betiği bir WPR kaydı açar, `-Es` ve `-Et` çalıştırır, normal kullanıcı
okuyucusuna sinyal verir ve ETL'yi kaydeder.

```powershell
& 'C:\Users\Serkan\Documents\ChatGPT\fdb\tools\run-fdb-cold-etw.ps1'
```

Normal kullanıcı yürütücüsü onay sonrası hazırlanır ve her deneme için sabit
çıktı kökünde `arm.json` yayınlar. Alanlar `version=1`, `run_id` (32 küçük
hex karakter), `trial` (1–8), `engine`, `anchor=tr-case0`, `start_utc` ve
`deadline_utc`. Aynı Windows oturumunda `Local\FDBColdMain{run_id}T{trial:00}Go`
ve `Done` olaylarını açar. `Go` purge sonrası, `Done` ölçüm fazları bittiğinde
kullanılır. Betik hazır kayıt yoksa çıkış yapar. Bir `trial-NN` dizini bir
kez alınır. Sonraki deneme, önceki denemelerin doğrulanmış `verdict.json`
kayıtlarına bağlıdır. Çıktılar
`docs/benchmarks/cold-etw-main-20261107/trial-NN/` altında tutulur.

**Hazırlık doğrulaması.** PowerShell AST hatası 0. WPR `-profiledetails`,
collector sınırları eklenen `tools/fdb-cold-etw-bounded.wprp` profilini
okudu. Yönetici betiğinin gerçek kayıt ve merge davranışı yürütmede
doğrulanacak. Her çağrıdan önce ne yaptığını bir cümleyle bildireceğim.

## İz süzme ve geçersiz deneme kuralı

1. Purge öncesinde mevcut manifest ve SQLite envanteriyle beklenen okuma
   dosyalarını ve zorunlu FileIO kanıtını dondur. FDB için normal strict open'ın
   bu slotta okuması gereken kaynakları, SQLite için `.db` ve aktifse kullanılan
   WAL akışını belirle. Kural her fiziksel dosyanın storage'a gitmesini istemez.
2. Birleşik ETL tracerpt özetinde **Total Events Lost = 0** aranır. Sıfır
   değeri, beklenen dosya okumaları ve kapanış tanıklarıyla birlikte değerlendirilir.
3. Geçici CSV'den hedef dosyalara ait FileIO Create/Name/Read/Close/Rundown,
   eşleşen FileObject/FileKey, IRP, ReadInit ve DiskIO Read satırlarını al.
   PID/TID ve faz UTC penceresini birlikte kullan. Yabancı PID'nin hedefe
   erişimleri ayrı tutulur. Disk tamamlanma PID'si bilinmiyorsa IRP'nin başlatma
   olayını kullan. Yeniden kullanılan kimlikleri yalnız kendi handle ve istek
   ömründe eşle. Ham satır numarası, UTC, bayt ve atıf sonucu kaydedilir.
4. Cold için open-start ile ilk sonuç arasında hedef DB verisine atfedilen
   storage okuması gerekir. Warm faz ayrı etiketlenir. Her satıra **Purge + ETW
   doğrulandı** veya **geçersiz** ve **collector bazı ölçülemedi** yazılır.
5. Süzülen satırlar ve özet kapatılıp okunabilirliği doğrulandıktan sonra,
   sadece o deneme için yürütücünün açtığı ham CSV `Remove-Item -LiteralPath`
   ile silinir. Çözümlenmiş mutlak yolun tam olarak o `trial-NN` altındaki
   geçici CSV olduğu ve yürütücünün dosyayı kendisinin oluşturduğu doğrulanır.
   Yönetici betiği bu temizliği yürütmez.

Kayıp > 0, eksik beklenen okuma, belirsiz atıf, hata veya sınır sorunu
denemeyi geçersiz yapar. Ham rapor sonucu saklar. Tekrar için kullanıcı
kararı gerekir. İlk sonuç kapısı veya oracle/CRC/kill/eski okuyucu kalırsa
sonraki denemeye geçilmez. Q30 değerlendirmesi kullanıcıya bırakılır.

## Disk hesabı

Mevcut ön test dahil yaklaşık **200,8 MiB** toplam, **50 MiB ETL**.
Bu başlangıca göre kalan **311,2 MiB toplam** ve **206 MiB ETL**. Eski ETL ve
ön test CSV'si hesapta durur. Yeni denemede geçici collector dosyaları,
birleşik ETL, geçici CSV ve süzülmüş kanıt aynı hesaba girer.

| İş | Bütçe kontrolü |
|---|---|
| WPR collector | Aynı provider ve buffer ayarlarıyla `MaximumFileSize`, Sequential. Sistem 48 MiB ve olay 24 MiB. Limitte duran veya eksik iz geçersiz. |
| Kayıt + merge | Başlamadan 160 MiB boş bütçe gerekir. 72 MiB collector, 72 MiB ikinci kopya payı, 16 MiB ek rezerv. `-stop -compress` birleşik çıktı için kullanılır. |
| CSV çözümleme | Kayıt ve merge bittikten sonra tek geçici CSV. Decoder için en fazla 180 MiB ve ayrıca toplam sınırdan 32 MiB kapanış payı. Daha küçük kalan bütçe esas alınır. Erken durdurma sonucu geçersiz. |
| Tavan | Toplam 512 MiB, bunun içinde ETL ≤256 MiB. Her aşamadan önce ve sonra güncel dosya uzunluklarıyla hesaplanır. Yeterli alan yoksa durup kullanıcıya sorulur. |

160 MiB bir kabul rezervidir. WPR'nin yerel yardımında birleşik çıktıya
ayrı bir sert boyut tavanı doğrulanmadı. Stop/merge ve decoder yazma payı
gözlenemiyorsa ilgili aşama başlatılmaz. Sıkıştırılmış ETL boyutu henüz
ölçülmedi. Ön test ve 160 MiB kayıt rezervi çıkarıldığında sonraki bir
kaydı açmadan önce tutulmuş yeni ETL'lerin toplamına **46 MiB** kalır.
Bu nedenle sekiz iz bütçeye sığmazsa matris yarım kalabilir. Tasarım izinli
tek ham CSV silme işlemiyle ilerler. Ek ETL temizliği kullanıcı kararı ister.
[WPR collector boyut sınırı](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/maximumfilesize)
