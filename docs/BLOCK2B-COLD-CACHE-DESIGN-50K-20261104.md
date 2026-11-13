# Blok 2b — gerçek OS-soğuk cache ölçüm tasarımı, 4 Kasım 2026

**Son tasarım teklifi:** [pozitif kontrol ve8-trial revizyonu](BLOCK2B-COLD-CACHE-REDUCED-DESIGN-50K-20261104.md).
Kullanıcı mevcut RAMMap negatif görüntüsünü yalnız o an için kabul etti.
Yeni teklif yalnız case0,4 dengeli çift, ilk/son RAMMap tanığı ve ara6 ETW
destekli deneme; gerçek ETW atıf/kayıp kontrolü hâlâ önkoşuldur.40dk/9 purge
(preflight dahil) üst sınırı önerisi **onay bekler**. Henüz uygulama yok.
Aşağıdaki3-anchor/24-trial plan tarihsel sürümdür; yeni onayda iki matris
birlikte çalıştırılmaz. Saat/performans eşiği/yakınsama kararı aynı.

3 Kasım'da başlayan tasarım 4 Kasım'da tamamlandı; dış kaynaklar aşağıda.

**Ana koşu tasarımı; ana ölçüm için ayrı onay gerekir.**
İkinci15dk [preflight](BLOCK2B-PREFLIGHT2-50K-20261104.md) yerel RAMMap1.63
Usage/CLI desteğini doğruladı. Yönetici pencereye otomasyon girdisi işlemiyor;
resident0 tanığı ve gerçek ETW kayıpsız eşleme açık. Purge0/ETW0, ana koşu yok.
Saat/kapı ve yakınsama kararı aynıdır; aşağıdaki plan uygulama onayı sayılmaz.

Sonraki kullanıcı onayı yalnız15dk preflight içindi;
[preflight raporunda](BLOCK2B-PREFLIGHT-50K-20261104.md) gerçek cold
kanıtlanamadı ve duruldu. Ana koşu hâlâ onaysız/başlamadı. Aşağıdaki yürütme
planı kendiliğinden başlamaz; preflight'ın kayıt dışı ETW sınırı korunur.
4 Kasım kullanıcı netleştirmesi: kapı saati açılışı içerir; yakınsamanın sayısal
kabulünü kullanıcı sonuçta kararlaştırır. Bu iki tanımın kabulü ölçümü veya
yönetici cache temizliğini yürütme onayı değildir.
3a tekrar açılmaz. 2c,100k, varsayılanlar ve C ABI kapsam dışıdır.
[Onarılmış mevcut50k](BLOCK3A-REPAIR-20261103.md) /881756DA… corpus ve sabit
FDB/SQLite slotları esas alınır; yeni corpus/DB kopyası/ingest/reset/bakım yok.
Ready, “delta boş / bütün eski revizyonlar temiz” demek değildir. Mevcut WAL
kuyruğu/manifest/run yerleşimi önceden kayda alınır, temiz seed'e dönüştürülmez.

## 1. Soğuk tanımı ve Windows yöntemi

Hedef, yeni proses/boş uygulama cache'i ile **DB dosya verisinin OS cache'inden
çıkarılmış olmasıdır**. SSD denetleyici/NAND cache'i veya bütün dosya sistemi
metadata'sının soğuk olduğu iddia edilmez. Windows normal dosya okumalarını
sistem file cache üzerinden yapar; write-through/flush ile cache bypass aynı
şey değildir. [Microsoft File Caching](https://learn.microsoft.com/en-us/windows/win32/fileio/file-caching).

| Yol | Yetki / etkisi | Tasarımdaki yeri |
|---|---|---|
| **Microsoft Sysinternals RAMMap** | Yönetici yetkili operatörle; file-cache çalışma kümesi ve standby temizliği sistem genelindedir. DB prosesleri/mapping'leri önceden kapanmalı. | Önerilen ana yöntem: **Empty System Working Set → Empty Standby List**. Resmî Microsoft kılavuzundaki sıradır; ürün sürümü/menüleri ve yetki uygulanmadan önce doğrulanır. |
| Tam Windows **Restart** | Yerel sistemi kapatma hakkı ve kullanıcının ayrıca reboot onayı gerekir. Açık işler/oturum kesilir. | RAMMap yerine baştan seçilebilen alternatif; otomatik fallback değil. Fast Startup/hibernate dönüşü kabul edilmez. Her cold deneme için ayrı restart gerekir; bir boot'ta ilk FDB'den sonra SQLite'a da cold denmez. |
| `SetSystemFileCacheSize` | Çağıran `SE_INCREASE_QUOTA_NAME` yetkisini etkinleştirmeli. File-cache working-set API'sidir. | `-1/-1` trim tek başına tüm standby/mapped sayfaların boş olduğunu kanıtlamaz; yeni özel purge aracı bu tur önerilmiyor. Kalıcı hard-min/max limiti değiştirilmez. |
| `FILE_FLAG_NO_BUFFERING` / SQLite mmap ayarı / yalnız yeni proses / `FlushFileBuffers` | Bypass bayrağı erişim/hizalama kontratını değiştirir; yeni proses OS cache'ini boşaltmaz; flush eviction kanıtı değildir. | **Kabul ölçümünün yerine geçmez.** FDB/SQLite mevcut buffered I/O ve mmap0 ayarları korunur; no-buffering ayrı motor/VFS deneyi olurdu. |

RAMMap kaynağı yalnız [resmî Sysinternals sayfası](https://learn.microsoft.com/en-us/sysinternals/downloads/rammap).
Menü sırası ve sistem-geneli etkisi: Microsoft [Analysis Services Performance
Guide](https://download.microsoft.com/download/0/F/B/0FBFAA46-2BFD-478F-8E56-7BF3C672DF9D/Microsoft%20SQL%20Server%20Analysis%20Services%20Multidimensional%20Performance%20and%20Operations%20Guide.pdf).
RAMMap komut satırı bayraklarının internetteki varyantları kullanılmaz; yerel
imzalı sürümün kendi yardımında doğrulanmadan bir `-E…` komutu tasarıma sabitlenmez.
Purge yardımcısı/ETW elevated, benchmark prosesi normal kullanıcıyla çalışır.
Bu oturumda araç mevcudiyeti/yetki denenmedi; indirme/kurulum yapılmadı.

[SetSystemFileCacheSize](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-setsystemfilecachesize)
yetki ve trim kapsamını belgeler. Bayrak alternatifi ayrıca
[File Buffering](https://learn.microsoft.com/en-us/windows/win32/fileio/file-buffering)
hizalama gerekleri nedeniyle normal okuyucuyla eşdeğer değildir.
Restart'ın tam boot olması ve Fast Startup farkı
[System power states](https://learn.microsoft.com/en-us/windows/win32/power/system-power-states)
ile [fast startup açıklamasında](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/distinguishing-fast-startup-from-wake-from-hibernation)
belgelenir. Reboot'u kullanıcı kendi hazır olduğunda yapar; zorla kapatma,
restart task'ı, startup servisi veya registry/hibernation ayarı kurulmaz.

## 2. Cold kanıtı: sadece “purge başarılı” demek yetmez

Önce bütün benchmark okuyucuları kapanır; başka uygulamaların DB'yi açmadığı
doğrulanır. Önceden oluşturulmuş file inventory'de FDB kaynak/manifest/index ve
kullandığı `.met`/`.vfy`/diğer cache dosyaları, SQLite db/WAL/shm yer alır.
Purge/boot sonrasında RAMMap File Summary/Details'te **bu dosyaların resident
data sayfaları0** tanığı, saat ve araç sürümü saklanır. Kilitli/aktif/modified
hedef sayfa veya doğrulanamayan dosya varsa o cold kol başlatılmaz. RAMMap
snapshot'ı RAM'i inceler; hedef payload'ı yeniden okumayacak gözlem yolu,
uygulama öncesi preflight'ta doğrulanmalıdır.

[WPR/WPA](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/windows-performance-recorder)
Disk I/O iziyle açılış/ilk sorgu aralığındaki storage okumaları ve yabancı
hedef-file erişimleri kaydedilir. Microsoft
[Disk I/O / File I/O profillerini ayrı tanımlar](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/built-in-recording-profiles).
Yalnız process I/O byte veya ReadFile sayacı fiziksel okuma değildir. Profilin
dosya adı/PID eşlemesini sağlayabildiği, stack/heap kaydı açmadan bounded
toplanabildiği ve event kaybı olmadığı preflight'ta doğrulanır. Bunlar
sağlanamazsa “gerçek cold” kanıtlanamadı diye durulur; yeni özel tracer yazılmaz.

Eviction ile launch arasında **SHA okuma, oracle GET, corpus decode, Explorer
preview veya DB inspeksiyonu yok**. Hash/oracle hazırlığı purge öncesi yapılır.
Tek purge sonrası tek motor denenir. Diğer motor için de yeni purge gerekir.
Tüm sistemin temizlenmiş standby sayısı tek başına DB'nin cold tanığı değildir.

Tanık çıktısı yalnız hedef-file adları/byte sayıları ve küçük ekran görüntüsü
veya metin özeti olur; sistemin tamamına ait büyük RAMMap `.rmp` dump'ları
biriktirilmez. İz ve tanık dosyaları da aşağıdaki toplam disk sınırına dahildir.

## 3. Zaman sınırları ve arama dizisi

Normal `Database::open` strict doğrulaması korunur; deferred/background/lazy
API, CRC reuse, join veya I/O bayrağı kullanılmaz. FDB strict-open kaynakları
tarayarak DB cache'ini **ilk sorgudan önce ısıtabilir**. Bu yüzden sadece
query1 süresi verilerek “soğuk ilk sonuç hızlı” sonucu çıkarılmaz.

Her trial için ayrı saatler:

| Saat | Ölçüm |
|---|---|
| Launch→open-start | Proses/loader/harness başlangıcı; DB açılışıyla karıştırılmaz |
| Open-start→open-return | FDB normal strict-open; SQLite normal connection/schema/WAL recovery. Kontratları eşdeğer değildir. |
| Query1-start→ilk sonuç | Yalnız ilk query süresi |
| **Open-start→ilk sonuç** | **Kullanıcıca onaylı≤1 s kapı saati; açılış dahil** |
| Query2…Query30 | Her çağrı ayrı; opener/same-handle ve aynı sorgu/sonuç sınırı korunur |

**Kullanıcı kararı: ilk-sonuç kapısı Open-start→ilk sonuç≤1 s.** Açılış ve
yalnız-query süreleri ayrıca raporlanır. Eşik değiştirilmedi; ölçüm yapılmadan
GEÇTİ/KALDI kararı üretilmez.

Üç sabit anchor: TR case0 **olduğu**, case25 **timmy**, case38 **zx**. Melez200
N50/offset0, kabul edilmiş try/seri davranışı; SQLite aynı key sıralı LIMIT50,
COUNT yapmayan FTS5 trigram/detailnone/columnsize0, WAL/FULL/cache2MiB/mmap0/tempFILE.
Kısa zx'nin2a kapısından muafiyeti **2b'ye taşınmaz**; ilk-sonuç kapısında
gözlenen kötü değerler de raporlanır.

Her cold trial'da aynı anchor **30 kez** aranır; farklı30 sorgu dizisiyle
30. çağrının tesadüfen daha kolay olmasına izin verilmez. Sonra proses kapanır;
warm faz için hedef DB dosyaları açıkça stream-read/hash ile ısıtılır ve yeni
proses açılır. Warm faz da aynı30 çağrı, ardından iki ayrı N60 warm A/A grubu
yapar. Warm30 öncesinde uygulama cache'i yenidir; N60 grupları uygulama+OS
ısındıktan sonradır. Cold query30 **warm hash işleminden önce** kaydedilmiş olur.
Bu sabit-anchor yerelliğidir; bütün41 sorgunun30 çağrıda ısındığı iddia edilmez.

**Yakınsama kapısı aynen:** “30. aramada sıcağa yakınsama”. Sayısal bir tolerans
verilmediğinden yeni +%5/+%10/2 ms eşiği icat edilmez. Q30/WarmA/WarmB p50/p95,
mutlak ms farkı, oran ve sıcak A/A yayılımı yan yana gösterilir. **Kullanıcı
kararı: sayısal kabulü sonuçta kullanıcı kararlaştırır.** Harness yakınsamayı
otomatik GEÇTİ/KALDI etiketlemez. Tek Q30 gözlemi
p95 değildir; dört cold tekrar da üretim p95 garantisi sayılmaz.

## 4. Sabit düzen ve üst sınırlar — uygulama onayından sonra

Mevcut araç ilk sorgunun ardından otomatik full oracle yapıyorsa bu akışta
aynen kullanılamaz; query2…30'u yapay olarak ısıtır. Gelecekteki uygulama
onayı yalnız2b için ayrı, salt okunur ince harness hazırlığını kapsamalıdır.
Eski3a harness'i düzeltilmez/yeniden çalıştırılmaz. SQLite'ta tek read-view
ömrü ve nested transaction olmaması preflight'ta doğrulanır; preflight yeni
DB/corpus veya ek benchmark denemesi üretmez.

Her anchor için4 dengeli çift: **FDB/SQLite, SQLite/FDB, SQLite/FDB,
FDB/SQLite**. Her motorun cold+warm trial'ı biter, sıradaki motor için yeniden
purge yapılır. FDB ve SQLite aynı physical DB'lerini tekrar kullanır; iki
motorun run/sequence/physical byte eşitliği varsayılmaz. Kaynak/model/run
envanteri ve source/cache digest'leri korunur; kurulum/yerleşim değişirse durulur.

| Kaynak / iş | Kesin üst sınır |
|---|---:|
| Preflight: araç/yetki, frozen binary, oracle hazırlığı, varsa sınırlı harness hazırlığı | **15 dakika** |
| Cold/warm/eviction veya seçilen boot yolu + tüm son doğrulama | **45 dakika** |
| Yürütmenin toplamı (bekleme/boot/elle işlem dahil) | **60 dakika** |
| Cold trial / purge veya baştan seçildiyse restart | **24 /24** |
| Warm trial | **24**, aynı sorgu ve DB, yeni proses |
| Cold query1…30 | **720** |
| Warm query1…30 | **720** |
| Warm A/A, trial başına2×60 | **2880** |
| Zamanlı sorgu toplamı / ölçülen open | **4320 /48** |
| Ön/son full oracle /41-case digest | Motor başına2; **200k ana GET +164 sorgu** |
| Kill/reopen + eski okuyucu doğrulaması | Motor başına1'er; her biri50k GET+41case: ek **200k GET+164 sorgu** |
| Log/trace/snapshot toplam ek disk | **512 MiB**; bunun içinde ETL en çok256 MiB |

Böylece doğrulama dahil sorgu üst sınırı4648, ana GET üst sınırı400k;
üst sınırlar yeni performans kapısı değildir. Eviction tanığı için hedef dosya
resident byte gözlemi ve kaynak SHA taraması sorgu değildir, süre/disk bütçesine
dahildir. Time budget dolmadan kalan zorunlu doğrulama için yer yoksa yeni
trial başlatılmaz; süre dolunca yarım raporla durulur. Başka tur/otomatik retry
veya süre uzatma yok. RAMMap başarısız olursa aynı blokta reboot'a geçilmez.

WPR/ETW tüm eşlenmiş cold/warm kollarda aynı düşük ayrıntılı ayarda olmalı;
dosya cache tanığıyla birlikte ilk-result kanıtını sağlaması şart. Stack/heap
ve sınırsız per-ReadFile logu yok; iz sınırında durulur. Özel profil gerekirse
hazırlık15 dakikayı aşmadan tasarım kapsamı içinde gösterilir; yoksa durulur.
Boot yöntemi seçilirse mutlak UTC başlangıç/deadline ve sonraki trial kimliği
metadata'da korunur; reboot saati sıfırlamaz. Saat geriye gider veya süre
doğrulanamazsa durulur. Kullanıcı beklemesi de60 dakikaya dahildir; otomatik
startup görevi/servis veya sonraki güne uzayan devam yoktur.

Perf kolunun içine oracle veya corpus modeli yüklenmez; beklenen ilk sayfa
descriptor/digest'i purge öncesinde hazırlanır, dönen payload RAM'de doğrulanır.

## 5. Rapor ve durma kuralı

Ham satırlar: anchor/çift/sıra/motor/cache yöntemi, eviction tanığı,
run/manifest/WAL durumu, open/ilk-query/ilk-sonuç/Q1…30/warm A/A, CPU ve
ayrı private/working-set sayaçları, ETW logical/physical ayrımı, oracle sonucu.
RAMMap'teki sistem sayfaları motor RAM'i değildir; süreç/harness belleği
motor-only diye sunulmaz. Her cold ve warm değer yan yana, kötü/yarım kollar
dahil verilir. SQLite open'ın FDB strict source doğrulamasını yapmadığı önde yazılır.

Oracle/CRC/sequence/eski okuyucu/kill hatasında **hemen dur**. Cold kanıtı yok,
hedef dosya yeniden ısındı, kaynak/yerleşim değişti, event/log kaybı oldu veya
süre/örnek/disk sınırı dolduysa eksik raporla dur; seçici tekrar yapma. Sabit
matriste performans kapısı kalırsa başarısızlığı kaydet, yeni anchor/ayar
ve başka bloğa geçme. Sayısallaşmamış yakınsamayı GEÇTİ yapma.

Riskler: sistem-geneli purge başka uygulamaları geçici yavaşlatır; RAMMap
gözlemi de sistemi kısa süre etkileyebilir. Reboot açık işleri keser. AV,
SysMain veya başka okuyucu dosyayı yeniden ısıtabilir; neden kanıtı yoksa
“bilinmiyor” yazılır, servis/AV kapatılmaz. Disk firmware cache'i, metadata
cache'i ve trace overhead'i kalan sınırlardır. Cache/bellek baskısı yaratmak
için sahte büyük dosya veya RAM doldurma yöntemi kullanılmaz.

**Ayrı uygulama onayı bekleniyor:** zaman ve yakınsama tanımları kabul edildi;
yalnız50k Blok2b yürütmesi ile yönetici RAMMap/cache temizliği için ayrıca
açık onay gerekir. Reboot alternatifine ayrıca
açık izin gerekir. Bu belge izinleri etkinleştirmez, araç indirmez, bilgisayarı
yeniden başlatmaz; 3a/2c/4/C ABI için uygulama yetkisi vermez.
