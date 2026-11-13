# Blok 2b ETW kanıtı tasarımı — 7 Kasım 2026

**Ön test onaylandı.** Yönetici başlangıç sinyali beklerken süre doldu.
[Çalıştırma sonucu](BLOCK2B-ETW-PREFLIGHT-50K-20261107.md)

Kullanıcı önceki 7 Kasım ön testinin 0 purge, 0 ETW ve
0/8 ana deneme sonucunu kabul etti. Bu teklif sonraki ön testin ve ana
matrisin kanıt yöntemini değiştirir. RAMMap'in görevi sırasıyla `-Es` ve
`-Et` çalıştırmaktır. Elle File Summary araması ve Total tanığı kaldırılır.
[Kabul edilen ön test](BLOCK2B-COLD-PREFLIGHT-50K-20261107.md)

## Tek kayıtta sıcak ve purge sonrası kontrol

Aynı canlı FDB kaynağı `run-0000000000001c98.fdb` (1.853 B) ve mevcut SQLite
dosyasının ilk en fazla 64 KiB'ı kullanılır. Başta dosya kimliği ve manifest
canlılığı kontrol edilir. Normal kullanıcı süreci her fazda aynı aralıkları
birer kez buffered ve salt okunur olarak okur, handle'ları kapatır ve PID,
bayt, UTC başlangıç ve bitişini kaydeder. SQLite sorgusu veya motor open'ı
ön testte kullanılmaz.

**Onaya sunulan ek adım.** Sıcak kontrolün başlangıç cache durumuna bağlı
kalmaması için WPR başlamadan önce aynı aralıkları birer kez ısıtmayı
öneriyorum. Böylece ETW kaydı içinde purge öncesinde bir ölçülen sıcak
okuma ve purge sonrasında bir ölçülen cold okuma olur. Ön ısıtma dahil üç
fazda toplam uygulama okuması en fazla **202.167 B**. OS read-ahead miktarı
ayrı bir sayaçtır. İlk ölçülen okuma storage'a giderse sıcak kontrol kalır.

| Sıra | Faz | Her iki hedef için aranan kanıt |
|---|---|---|
| 1 | Ön ısıtma, kayıt dışında | Aynı byte aralığını oku ve kapat. |
| 2 | Tek WPR kaydı, sıcak okuma | Doğru yol ve okuyucu PID'sine ait FileIO var. Eşlenen storage DiskIO read sayısı 0. |
| 3 | Aynı WPR kaydında tek purge | `-Es` tamamlanır, sonra `-Et` tamamlanır. İki exit kodu ve UTC zamanları kayda girer. |
| 4 | Aynı aralıkları tekrar oku | Doğru yol ve PID'ye ait FileIO ve buna bağlanabilen en az bir storage DiskIO read var. |
| 5 | WPR stop ve çözümleme | İki fazın zaman pencereleri ayrılır. Her collector için kayıp olay 0 doğrulanır. |

ETW çözümlemesi dosya adını ve FileObject/FileKey ömrünü, okuma olayının
iş parçacığını ve okuyucu PID'sini birleştirir. Storage olayı ilgili dosya
ve başlatan iş parçacığına veya istek zincirine bağlanır. Dosya içi offset
FileIO'dan, disk transfer boyutu DiskIO'dan alınır. Disk üzerindeki offset
dosya offset'i olarak kullanılmaz. Dosya adı, PID, zaman ve istek bağlantısı
belirsiz kalırsa atıf kapısı kalır. [Microsoft FileIO ad eşlemesi](https://learn.microsoft.com/en-us/windows/win32/etw/fileio-name) · [Microsoft DiskIO alanları](https://learn.microsoft.com/en-us/windows/win32/etw/diskio-typegroup1)

Tek yönetici PowerShell penceresinde hazır WPR ve RAMMap komutları
kullanılır. Her bloğun amacı yapıştırmadan önce bir cümleyle belirtilir.
İki okuma normal kullanıcı sürecinde yürür. Sıcak okumanın kapanışı ve
purge bitişi, aynı oturumda geçici bellek içi faz sinyalleriyle sıralanır.
Sinyal zaman aşımı kaydı sonlandırır. Mevcut WPR DiskIO.Light ve FileIO.Light
profilleri ve yerel çözümleme araçları esas alınır. Bu akış için yeni bir
tracer veya kalıcı görev, servis ya da sistem ayarı gerekmez.

Onay geldiğinde **yeni başlangıç UTC ve başlangıç + 10 dakika deadline UTC**
ilk işlemden önce kayda yazılır. Önerilen tek kayıt süresi **en fazla 30 s**.
Bu süre aynı iz içinde purge ve iki ölçülen okumaya yer açar. Son iki dakika
stop/çözümleme/rapora ayrılır. Hazırlık, kullanıcı beklemesi ve kapanış
10 dakikaya dahildir. Sınır veya kapı kalırsa yarım kanıtla durulur.

Ön test en fazla **1 purge, 1 ETW kaydı**. ETL geçici ve nihai çıktısı
yalnız repo içindeki önceden belirlenen `docs/benchmarks` alt klasörüne gider.
Toplam ETL 256 MiB ve tüm ek çıktı 512 MiB sınırları uygulanır. Başlatmadan
önce kayıt ve stop/merge için yeterli süre ve disk rezervi ayrılır. Mevcut
WPR file-mode tampon ayarı bir dosya boyutu tavanı sayılmaz. Bu rezerv ve
çıktı takibi sağlanamazsa kayıt açılmaz.

Her iki hedefte sıcak FileIO var ve eşlenen DiskIO 0, purge sonrasında
eşlenen DiskIO var, ayrıca dosya/PID zinciri tam ve kayıp olay 0 ise ön test
**GEÇTİ**. Bunlardan biri eksikse **gerçek cold kanıtlanamadı** diye raporlanır.
Sıcak kontrolü düzeltmek için ek ısıtma, ikinci purge veya ikinci trace yok.

## Ana koşudaki kanıt etiketleri

Ön test geçerse raporlanır ve ayrı **“başla”** onayı beklenir. Ana koşu aynı
50k slotlarda yalnız `olduğu` için dört dengeli çift kullanır. Sıra
**FDB/SQLite, SQLite/FDB, SQLite/FDB, FDB/SQLite**. Her cold deneme ayrı
purge ve trace kullanır. Sekiz denemenin hepsi aynı ETW kanıt kuralına tabidir.
İlk ve son deneme de dahil elle RAMMap adımı sıfırdır.

| Etiket | Kural |
|---|---|
| **Purge + ETW doğrulandı** | Purge zinciri tamamlandı. `Open-start → ilk sonuç` aralığında hedef motorun DB veri dosyasına atfedilen storage read var. Yol/PID eşlemesi ve kayıp olay 0 doğrulandı. |
| **Cold kanıtlanamadı** | Hedef veri okuması yok, atıf belirsiz, olay kayıp, purge tamamlanmadı veya hedefe ölçüm öncesi yabancı erişim görüldü. Ham satır saklanır ve koşu durur. |
| **Warm referans** | Aynı denemenin warm fazı, ayrı PID ve faz işaretleriyle. Q30, warm A/A ile yan yana verilir. |

ETW kanıtının gücü izde eşlenen storage okumalarıyla sınırlıdır. Kısmi
purge sonrasında storage okumaları ile cache hit'leri aynı koşuda bir
arada bulunabilir. Raporda her deneme için hangi hedef dosyanın kaç bayt
storage okumasıyla eşlendiği gösterilir. Böylece kanıt görülen dosya ve
istek kapsamına bağlı kalır. Disk izi yoksa neden bilinmiyor olarak kalır.

Ana matris en fazla 8 cold + 8 warm, 8 purge, 8 ETW oturumu, 1.440 zamanlı
sorgu ve 16 ölçülen open. Aynı oracle, CRC, kill/reopen ve eski okuyucu
doğrulamalarıyla toplam 1.768 sorgu ve 400k GET sınırı korunur. WPR izinde
open ve ilk sorgu ayrı raporlanır. İlk sonuç kapısı **Open-start → ilk
sonuç ≤ 1 s**. Yakınsama 30. sorgu ile sıcak A/A yayılımı yan yana verilerek
değerlendirilir ve sayısal karar kullanıcıya aittir.

Ön test 10 dk. Ayrı onaylı ana pencere, önceki bütçeden 25 dk yürütme ve
5 dk raporlama olmak üzere en fazla 30 dk. Toplam aktif bütçe 40 dk ve
en fazla 9 purge / 9 ETW. Ana trial başına önceki 60 s trace sınırı ile
yeni toplam kayıt üst sınırı **30 + 8 × 60 = 510 s**. Ön test ve ana koşu
arasında yeni bir başlangıç ve deadline kaydedilir. Onay bekleme süresi
pencere dışındadır. Kullanılmayan ön test süresi ana koşuya aktarılmaz.
[Önceki süre, oracle ve eşleştirme planı](BLOCK2B-COLD-CACHE-REDUCED-DESIGN-50K-20261104.md)
