# Blok 2b — küçültülmüş cold tasarımı, 4 Kasım 2026

**7 Kasım kanıt yöntemi teklifi.** Elle RAMMap tanığının yerini sekiz
denemede de purge + ETW kanıtı alır. Tek kayıtta sıcak ve purge sonrası
okumalarla ön test yapılması, ardından ana koşu için ayrı “başla” onayı
önerilir. Bu belgedeki ilk/son RAMMap tanığı ve 10 s ETW prova akışı
önceki sürüme aittir. [Onay bekleyen ETW tasarımı](BLOCK2B-COLD-ETW-DESIGN-50K-20261107.md)

**YALNIZ TASARIM; UYGULAMA BAŞLAMADI, kullanıcı onayı bekleniyor.**
Bu belgede onay istenen değişiklikler: tek pozitif kontrol/purge denemesi ve
yalnız TR case0 `olduğu` ile8 cold deneme. Önceki
[tasarımdaki](BLOCK2B-COLD-CACHE-DESIGN-50K-20261104.md)24-denemelik yürütme
matrisinin yerine önerilir; eski sonuçları/kapıları yeniden etiketlemez.

## Mevcut kanıt ve kalan önkoşul

Kullanıcı, yenilenmiş RAMMap File Summary görüntüsünde
`\data\benchmark-work\` aramasının iki yönde de sonuç vermemesini kabul etti:
**o görüntüde hedef klasöre ait RAM sayfası saptanmadı**. Öncesinde listede
görünen bir dosya adı, ardından daraltılmış Path sütununun görünmeyen yol
kısmı pozitif bulundu. Bu kullanıcı destekli sonraki kontrol, önceki purge
anını geriye dönük kanıtlamaz. Önceki5dk preflight'ın10s aşımı kayıtta kalır.

ETW için mevcut kanıt **yalnız yetenek/yapılandırma**: yerel WPR'de
DiskIO/Init, FileIO/Init, ProcessThread/Loader var; file-mode collector ayarı
80MiB. Gerçek hedef yol/PID bağlama, storage okuması ve lost-event0 henüz
kayıtla doğrulanmadı. “Yetenek doğrulandı” bunların GEÇTİ olduğu anlamına gelmez.
ETW prova kaydı da bu tasarımla açıkça onaya sunulur; bugün başlatılmadı.

## 1. Tek pozitif kontrol — en fazla bir deneme

Yeni corpus/DB/kopya yok. Mevcut envanterdeki küçük FDB kaynak dosyası adaydır:
`data/benchmark-work/fdb/run-0000000000001c98.fdb`, tarihsel boyut **1853B**.
Onay sonrası yalnız varlığı/manifest-canlılığı/aynı kimlikte olduğu kontrol
edilir. Uygun değilse durulur; başka dosyalar denenerek sonuç aranmaz.

| Sıra | İş | Kaydedilecek tanık |
|---|---|---|
| 1 | Bütün DB okuyucularını kapalı doğrula, adayın aynı canlı kaynak olduğunu belirle. | Kabul edilmiş mevcut negatif görüntü tarihsel başlangıçtır; ek RAMMap oturumu yok. |
| 2 | Aday küçük dosyayı normal buffered, salt okunur tek handle ile bir kez bütünüyle oku ve handle'ı kapat. | Yol, okunan1853B, okuma bitiş UTC; engine open/GET veya veri yazımı yok. |
| 3 | RAMMap F5 tamamlandıktan sonra **AYNI** `\data\benchmark-work\` metnini ara. | Adayın tam yolu ve **Total>0** görünmeli. Yalnız başka DB dosyasının bulunması pozitif sayılmaz. |
| 4 | Yerel Usage'dan doğrulanan `-Es` ardından `-Et`, **bir kez**. | Başlangıç/bitiş UTC, PID/exit; exit0 tek başına başarı değildir. |
| 5 | Hedefleri yeniden okumadan F5; aynı aramayla baştan Down ve sondan Up. | Aday dahil hedef-prefix kaydı bulunmamalı. Kayıt kalırsa Total/diğer sütunlar kaydedilir ve önkoşul KALDI olur. |

Pozitif Total>0 veya purge sonrası kaybolma gösterilemezse **dur**: ikinci
okuma/purge, başka aday, gizli tekrar veya ana koşu yok. Buradaki başarı yalnız
bu dosyada **okundu→RAMMap'te göründü→purge sonrası kayboldu** zinciridir;
başka zamandaki tüm DB'lerin cold garantisi değildir. Hedef-prefix araması,
yalnız aday adı yerine aynı ana-koşu aramasının sınanmasını sağlar.

### Aynı preflight içinde tek ETW uygunluk kaydı

Yukarıdaki negatif tanık alındıktan sonra en fazla **10s** tek WPR kaydı:
aynı küçük FDB dosyasını ve mevcut `sqlite.db`nin ilk en fazla64KiB'ını normal
buffered, salt okunur handle'larla birer kez oku/kapat. Bu erişimler tanıktan
SONRA yapılır ve preflight dosyalarını tekrar ısıtır; sonraki ana trial'ın
kendi purge'ü zorunludur. SQLite DB büyük olduğu için bütünü okunmaz.

Yerel built-in DiskIO.Light+FileIO.Light file-mode, stack/heap yok. Yerel
mevcut araçlarla iki tam yol, test PID'si, FileIO ile storage DiskIO bağlama
ve collector kayıp-event sayıları doğrulanmalı. FileIO/ReadFile veya proses
byte sayacı tek başına storage okuması sayılmaz. Eşleme/kayıp istatistiği
çıkarılamazsa **bilinmiyor/KALDI**, ana koşuya geçilmez. Yeni parser/tracer,
provider/ayar deneyi veya ikinci kayıt yok. Prova sınırı10s, tüm preflight10dk.

80MiB buffer konfigürasyonu ETL disk tavanı değildir. WPR'nin sert ETL byte
tavanı yerel yardımda doğrulanmadı. Toplam ETL256MiB ve tüm ek çıktı512MiB
**durdurma sınırları** korunur; built-in kayıt/stop/çıktı boyutu bu bütçeyle
denetlenemiyorsa önkoşul geçmez. Stop/flush büyümesi ayrıca raporlanır; sıfır
overshoot garantisi verilmez. Özel elevated servis/task/helper kurulmaz.

## 2. Ana matris — yalnız case0 `olduğu`

Aynı881756DA… corpus'un mevcut ilk50k DB'leri, sabit iki slot; yeniden ingest,
reset, kopya, maintenance veya yazma yok. Accepted try/seri, melez200,
N50/offset0. SQLite önceki FTS5 trigram/detailnone/columnsize0,
WAL/FULL/cache2MiB/mmap0/tempFILE ayarları ve key-sıralı LIMIT50/COUNT'suz yol.
SQLite tek read-view/nested-BEGIN önkoşulu gerçek ince2b harness'inde de
onay sonrası doğrulanmalı; önceki Node testi onun yerine geçmez.

| Çift | Cold sıra no | Sıra | RAMMap elle negatif tanığı |
|---|---|---|---|
| 1 | 1,2 | FDB → SQLite | **1. deneme/FDB**, purge sonrası ve open öncesi |
| 2 | 3,4 | SQLite → FDB | Yok; iki deneme de ETW destekli |
| 3 | 5,6 | SQLite → FDB | Yok; iki deneme de ETW destekli |
| 4 | 7,8 | FDB → SQLite | **8. deneme/SQLite**, purge sonrası ve open öncesi |

**Her cold deneme için ayrı purge**; toplam8. RAMMap elle kontrolü yalnız
1 ve8'de; ayrıca preflight pozitif zincirinin önce/sonra gözlemleri vardır.
Ara6 trial'da başarılı CLI/purge kaydı ve ETW storage izi gerekir. Hedefe
yabancı erişim, atıf sorunu veya event kaybı varsa trial doğrulanmış sayılmaz.
Purge→open arasında hedef read/hash/oracle/Explorer önizleme yok.

**Kanıt sınırlaması ve onay konusu:** ETW disk okuması, okunan bölümün storage'a
gittiğini destekler; tüm DB sayfalarının önceden resident0 olduğunu kanıtlamaz.
Dolayısıyla1/8 “RAMMap+ETW tanıklı”,2–7 “purge+ETW tanıklı, doğrudan RAMMap
resident0 ölçümü yok” diye ayrı etiketlenir. İlk/son tanığı ara denemelere
taşınmaz; sekizinin eşit güçte tam cold kanıtı var denmez. Bu kanıt kapsamı
değişikliği de kullanıcı onayına dahildir, performans eşiği değişmez.

Her cold trial: yeni proses, normal strict-open, aynı anchor30 kez; sonra
kapanış. Ardından mevcut hedef dosyaları stream-read ile ısıt, yeni warm
proses aç; aynı30 çağrı ve aynı handle üzerinde2×N60 warm A/A. ColdQ30 warm
ısıtmadan önce kaydedilir. Her motorun cold+warm kolu bitince sıradaki trial.
Tek ETW oturumu bu cold+warm fazları kapsar, UTC/PID/faz işaretleriyle ayrılır;
bütün kollarda aynı profil, oturum başına en fazla **60s**, gizli trace tekrarı yok.
Warm ısıtma fazı cold storage byte'larına katılmaz.

Saat/kapı **AYNI**: **Open-start→ilk sonuç≤1s**, normal strict doğrulama dahil.
Launch/open/query1 saatleri ayrı. “30. aramada sıcağa yakınsama” sıcak A/A
yayılımı, mutlak ms ve oranla yan yana; sayısal kabul kullanıcıda. Yeni
tolerans/eşik yok. Dört Q30 gözlemi p95 garantisi değildir. zx/timmy veya
başka anchor bu matrise eklenmez; eski41-case yalnız doğrulamada kullanılır.

## 3. Süre, örnek ve disk üst sınırları

Bunlar **önerilen yürütme sınırları**, bugünkü belge çalışması saati değildir.
Onay sonrası başlangıç/deadline UTC yazılır; kullanıcı beklemesi dahil.

| Aşama | Tahmini süre | Kesin üst sınır |
|---|---:|---:|
| Pozitif kontrol + tek ETW prova + önkoşul/hazırlık | 6–9dk | **10dk** |
|8 cold/warm trial, ön/son oracle ve kill/eski okuyucu dahil | 15–22dk | **25dk** |
| Rapor/durum kapanışı | 3–5dk | **5dk** |
| Toplam, elle bekleme dahil | 24–36dk | **40dk** |

Ana25dk'nın son **8dk**'sı zorunlu son doğrulamalara ayrılır; bu rezerv
başladığında yeni trial açılmaz. Ön doğrulama/trial'lar kalan17dk'ya sığmazsa
eksik rapor; hedef8'e ulaşmak için döngü yok. Onay öncesi hiçbir sayaç başlamaz.
Kaynak/harness hazırlanması önkoşul10dk'ya sığmazsa süreyi sessizce uzatmadan
durulur. Hazırlık onayı yalnız ince salt-okunur2b harness'i kapsar; motor
değişikliği/eski3a harness'ini düzeltme yetkisi değildir.

| Kaynak/örnek | Üst sınır |
|---|---:|
| Preflight denemesi / purge dizisi / ETW kaydı | **1 /1 /1** |
| Ana cold / warm trial | **8 /8** |
| Ana purge / ana ETW oturumu | **8 /8** |
| Preflight dahil purge / ETW oturumu | **9 /9** |
| Cold30 sorgu | **240** |
| Warm30 sorgu | **240** |
| Warm A/A,8×2×60 | **960** |
| Zamanlı toplam sorgu / ölçülen open | **1440 /16** |
| Ön/son ana50k GET+41 digest | İki motor toplam **200k GET +164 sorgu** |
| Salt-okunur kill/reopen + eski okuyucu doğrulaması | İki motor toplam ek **200k GET +164 sorgu** |
| Doğrulama dahil toplam sorgu / GET | **1768 /400k** |
| ETW kayıt süresi toplamı | **10+8×60=490s**; export/stop süre bütçesinde |
| Toplam ek disk / bunun içinde ETL | **512MiB /256MiB** |

Doğrulama sorguları performans örneği değildir; kill read-only okuyucuda,
model/source/slot aynı. Oracle ilk30 arasına girmez. Mevcut kaynak/index
CRC/sequence, GET/version, capped sonuç digest'i ve eski okuyucu kontrolleri
korunur. Dört ilk-sonuç gözlemi motor başına tek tek ve eşlenmiş farklarıyla
raporlanır;8 denemeyi havuzlayıp motor başına8 tekrar var denmez.

## 4. Kullanıcının elle işi — sayım ve tahmin

“Adım” bir tıklama, klavye kısayolu veya tek metin yapıştırmadır. UAC sayıları
üst tahmin; zaten açık yönetici RAMMap'in yeniden açılması şart değildir.

| Elle iş | Adet / efor |
|---|---|
| Preflight pozitif RAMMap: F5, adayın aynı prefix ile bulunması/tam yol+Total | **1 oturum,8–12 adım**, yaklaşık1–2dk |
| Preflight purge sonrası negatif: F5, baştan Down ve sondan Up | **1 oturum,12–18 adım**, yaklaşık1–2dk |
| Ana1 ve8 negatif RAMMap tanıkları | **2 oturum,24–36 adım**, toplam2–4dk |
| RAMMap elle tanık toplamı | **4 oturum,44–66 adım**, yaklaşık4–8dk |
|9 purge ×iki ayrı RAMMap RunAs | **En fazla18 UAC onayı** |
|9 ETW oturumunun start/stop'u ayrı RunAs gerektirirse | **En fazla18 UAC onayı** |
| Gerekirse ilk RAMMap açılışı | Ek1 UAC/başlangıç, yaklaşık1dk |
| Toplam kullanıcı aktif eforu | **Yaklaşık81–103 adım** (en yüksek UAC akışıyla), **6–12dk**,40dk pencereye yayılmış |

Her UAC için2–4s ve tanık oturumu başına1–2dk **tahmin**; ölçülmüş süre
değildir. Ara6 trial'da RAMMap menüsü/arama yok; kullanıcı yalnız gerekirse
UAC onaylar. Purge iki CLI çağrısıyla assistant tarafından sıralanır; yeni
elevated launcher/kalıcı task yok. Tanık ekranını assistant okur/kaydeder;
kullanıcının162 dosya adını tek tek yazması istenmez. Sistemin tamamına ait
büyük.rmp dump alınmaz. Gözlem/proses hatasında en fazla tek bağ kurtarma;
süre içinde çözülemezse eksik rapor, tekrar purge yok.

## 5. Durma kuralı / onay

Pozitif zincir veya gerçek ETW atıf/kayıp kontrolü geçmezse **ana koşu yok**.
Oracle/CRC/kill/eski okuyucu hatası, kimlik/yerleşim değişmesi, olay kaybı,
tanıkta resident hedef kaydı, süre/örnek/disk sınırı: **dur ve eksik/kötü
sonucu raporla**. İlk-sonuç performans kapısı kalırsa sonraki denemeye geçme;
çift yarım kalırsa seçimli tekrar veya sıra düzeltmesi yapma. Yakınsama için
otomatik yeni eşik yok, kullanıcı kararını bekle. Kalan denemeler eksik yazılır.

Başarısız denemeyi sıcak diye yeniden etiketleyip matrisi tamamlamak yok.
Reboot, yeni corpus/DB,100k,3a/2c/4/C ABI, motor/format/ayar değişikliği yok.
Tamamlanmayan tüm eski işler OPEN-WORK'te kalır.

**Onay istenen paket:** tek pozitif preflight + tek kısa ETW uygunluk kaydı;
yalnız case0 ile belirtilen sırada8 cold/warm trial; RAMMap ana tanığı1/8,
ara6 için sınırlamaları belirtilmiş ETW destekli purge kanıtı; **40dk,9 purge,
9 trace üst sınırı**. Kullanıcı onayından önce hiçbirinin yürütmesi başlamaz.
