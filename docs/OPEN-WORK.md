# FDB açık işler

17 Kasım — Siftwake kurumsal kimliği ve `0.1.5-preview` yayın
belgeleri hazırlandı. Yeni README, Rust başlangıç/API, CLI, güncel motor
sözleşmeleri, performans özeti, marka kılavuzu ve sürüm eşlemesi eklendi.
Önceki README geliştirme arşivinde korundu. Eski sürüm adları raporların
tarihsel bağlamında kalır. Yerel sürüm etiketleri kullanıcı kararıyla
yeniden numaralanır. Yazma/bakım, arama, RAM/disk ve eşzamanlı yükteki
açık performans maddeleri izlenmeye devam eder.
[Belgeler](README.md) · [Marka](BRAND.md) · [Sürümler](VERSIONING.md)

16 Kasım — FDB ve Tantivy 0.27.0, ilk 50k Türkçe belge üzerinde iki
dengeli blok. FDB ilk 50'de 39/41, exact 40/41 daha düşük p95.
Tantivy'nin başarılı kollarında ingest 16,44–16,79 s ve disk
137,82 MB, FDB 39,61–46,43 s ve 300,84 MB. Tantivy sıcak ilk
sonuçta önde, FDB indeksleme private commit tepesinde düşük.
Tantivy tarafında dört seed kesintisi ve bir OS 5 güncelleme
kesintisi kayıtta. Başarılı kollar 50k payload/41 sorgu oracle'ını
geçti. Gerçek soğuk ve sürekli churn bu koşuda ölçülmedi.
[Karşılaştırma ve ham kanıt](TANTIVY-COMPARISON-50K-20261116.md)

### Tantivy kıyasından kalan yapılacaklar

- [ ] **Tekli güncelleme.** İlk iş, güncel FDB'de küçük WAL katmanı
  kapalı/açık eşleştirmesi. İlk 50k üzerinde rapordaki aynı 24
  dosya güncellemesi ve gövde boyutlarıyla commit, görünüm yayımı
  ve ilk doğru sonuç sürelerini ayır. p95 143 ms (FDB varsayılan)
  ve 81 ms (Tantivy, başarılı örnekler) taban. WAL açık kolda
  foreground flush/merge beklemesi, dirty reopen ve sürekli yazmada
  reclaim/disk ilerlemesini izle. Başarısız deneme ve tekrar süresini
  da kaydet. [Küçük WAL karar tablosu](BLOCK4-DECISIONS-20261104.md)
- [ ] **İlk indeksleme.** 39,6–46,4 sn (FDB) ve 16,4–16,8 sn
  (Tantivy, başarılı koşular) taban. Aynı 100×500 akışlı yükte
  normalleştirme/trigram, posting üretimi, kaynak ve indeks yazımı,
  WAL ve son bakım/bütünlük kontrolü sürelerini ayrı ölç. Profilde
  baskın çıkarsa önce posting işini sınırlı tamponla azaltmayı dene.
  Önceki 4.096 terimli adayın join kazancı ve commit uçlarındaki
  gerilemeler karşılaştırmada kalsın. [Posting üretimi profili](WAL-PREFIX-POSTING-BUILDER-50K-20261115.md)
- [ ] **Disk.** Temiz 50k tabanı 300,8 MB (FDB) ve 137,8 MB
  (Tantivy). FDB'de kaynak payload, içerik indeksi, fields/links
  indeksleri ve WAL payını çıkar. Tantivy'nin LZ4 sıkıştırması ve
  fields/links için saklama sözleşmesi ayrı değişkenler. En büyük paya
  göre Frunner için yalnız içerik indeksleme profili veya blok
  sıkıştırması tasarla. Disk kazancını okuma CPU'su, ilk sayfa p95'i,
  ingest RAM'i ve churn sonrası retained disk ile birlikte ölç. Kalıcı
  biçim etkisi varsa sürüm ve uyumluluk planını önce onaya sun.
- [ ] **Açılıştan ilk sonuç.** Sıcak p95 218 ms (FDB strict) ve
  12,4 ms (Tantivy normal). Tantivy tam checksum kontrolünün maliyetini
  ayrı ölç, FDB'de açılış/ilk doğru sonuç/tam doğrulamayı temiz ve
  churn görmüş yerleşimde ayır. Sonraki tasarım adayı olarak aynı
  run üzerinde tespit edilen mükerrer doğrulama okumalarını birleştirme veya
  mevcut arka plan açılışında doğrulama durumunu açıkça yayımlama
  değerlendirilebilir. Kazancı doğrulanmış ilk sonuçla ölç. Gerçek
  soğuk ölçümü ayrı koşu olarak kaydet. [Strict-open aşamaları](STRICT-OPEN-REMAINING-PHASES-20261109.md)
- [ ] **Boşta RAM.** Temiz private commit 2,37 MiB (FDB) ve 1,10 MiB
  (Tantivy). Daha düşük öncelikle okuyucu hazır, ilk arama sonrası ve
  temiz boşta haritalanan sayfalar, private commit ve working set'i
  ayır. Baskın tahsis doğrulanırsa cache veya scratch başlatmasını
  ertelemenin ilk arama ve sıcak p95 maliyetini ölç. [Motor-only RAM ölçümü](RECLAIM-MOTOR-ONLY-RAM-50K-20261114.md)

Sonraki adaylarda aynı korpus, dengeli sıra ve 41 sorgu oracle'ı
kullanılır. FDB'nin ilk 50'de 39/41 ve exact'te 40/41 p95 üstünlüğü ile
17,9 MiB ingest private commit tabanı korunacak ölçümler. Yazma ve bakım
adaylarında 100'lük commit uçları, kesintisiz churn'de disk, tam
revizyon yeniden yazımında motor-only RAM ve eşzamanlı okuma/yazma
ayrı izlenir.
Mevcut eşikler, varsayılanlar ve onay kuralları geçerli.

15 Kasım — Gerçek soğuk 50k karşılaştırması, SQLite→FDB ve FDB→SQLite
tamamlandı. İşlemci override'ı olmadan güncel FDB strict-open,
39 run. Açılıştan ilk sonuca FDB 885–909 ms, SQLite 6.663–6.696 ms.
Sıcak karşılıkları FDB 627–640 ms, SQLite 297–339 ms. Dört soğuk
iz geçerli, birleşik kayıp 0, collector bazı ölçülemedi. SQLite
süresi için kapı uygulanmadı. Q30 yakınsamasının sayısal kararı ve
başka sorgu/yerleşimler açık. 50k/41 oracle, kill/reopen ve eski FDB
okuyucu geçti. [İki soğuk çift ve zaman ayrımı](COLD-OPEN-PAIRED-50K-20261115.md)

15 Kasım — Prefix kurulunca kısa WAL yoluna yeniden giriş adayı
43/43 grubun senkron delta flush'ını önledi. Kısa çiftlerde p99
%25,2/%32,0 düştü, p95 5,66×/3,93× oldu. Kesintisiz 30 turda kontrol
disk kapısını 30/30 geçti, aday 4/30 (**KALDI**, tepe tur sonu 1,835×).
Son tur farkının %96'sı canlı manifest dosyalarında, revizyon yayımı
226→93. Aday geçici kopyada, üretim ayarları aynı. Oracle/CRC/kill/
eski okuyucu ve tam paket geçti. Prefix çalışma bütçesi, reclaim
ilerlemesi, geniş arama ve motor-only RAM açık.
[Yeniden giriş ve kesintisiz disk raporu](WAL-PREFIX-READMISSION-50K-20261115.md)

15 Kasım — WAL prefix indeks profilinde posting ekleme %57,5,
trigram çıkarma %29,0, gövde normalleştirme %3,5. Geçici, 4.096
terimle sınırlı posting adayı iki çiftte de prefix join toplamını
%14,9/%58,8 ve commit p99'u %15,8/%4,2 düşürdü. >1 s adetleri
7→4 ve 6→5. Commit max iki çiftte arttı, p95 ikinci çiftte %21,4
arttı. Aday geçici kopyada. 50k/41 oracle, kill/reopen ve eski
okuyucu 5/5, tam test paketi geçti. Foreground delta flush ve normal
merge uçları, geniş arama, motor-only RAM ve uzun disk kapıları ayrı açık.
[İki çift, aşamalar ve ham sonuçlar](WAL-PREFIX-POSTING-BUILDER-50K-20261115.md)

14 Kasım — Üç tur, 300 akışlı commit. 12 uzun yolun tek tetikleyicisi
2.048 anahtar sınırı. Sekiz beklemeli prefix işçisinde ilk geçişin
%95,2'si indeks oluşturma çağrısında, %4,8'i WAL satırı edinme ve
doğrulamada. Join toplamında ilk geçiş %42,5, sonraki indeks hazırlığı
%48,4. En uzun commit'te 1,27 s normal merge join; 100'lük kuyruk ve
foreground flush açık. 50k/41 oracle, kill/reopen, eski okuyucu ve tam test
paketi geçti. Ölçüm disk bütçesi içindeydi, ardından debug test çıktıları
geçici derleme tavanını aştı. Üretim kodu ve varsayılanlar aynı.
[İlk geçiş profili ve ham tablolar](WAL-PREFIX-FIRST-PASS-50K-20261114.md)

14 Kasım — WAL prefix beklemesinin çevrimdışı tanısı. Profilli kontroldeki
97 belirgin beklemenin yeniden kurulabilen 86'sında 2.000 + 100 anahtar
2.048 sınırını aştı. Kalan 11 tetikleyici bilinmiyor. 40,57 s beklemenin
%53,8'i kaynak yazımı öncesi, %36,1'i indeks hazırlığında. İlk WAL
doğrulaması ile gram/posting hazırlığının ayrı maliyeti ve 100'lük
commit uçları açık. Mevcut motor ve slotlar üzerinde yeni koşu yapılmadı.
[Sequence eşlemesi ve tanı](WAL-PREFIX-WAIT-DIAGNOSIS-50K-20261114.md)

14 Kasım — Geçici merge erteleme adayı, delta flush sonrası join'i
sıfırladı. Profilsiz p99 1.289 → 1.055 ms, profilli p99
1.318 → 1.328 ms. Profilli toplam normal merge join 8,68 → 9,97 s.
50k oracle/kill/eski okuyucu ve 30/30 disk kapısı geçti (1,2974×).
24/24 olay ≤1 s, rewrite örtüşmesi yok. Sürücü burada durdu, motor-only
RAM ölçülmedi. Aday geçici derlemede. İlk drain ve WAL prefix açık.
[Aday raporu](NORMAL-MERGE-DEFER-50K-20261114.md)

14 Kasım — WAL prefix doğruluk düzeltmesi ve model testi `ddb263b` ile
commit edildi. İki profilli 50k kolda 100 uzun commit'in tamamı akışlı
uzun yoldaydı. Beşinde delta flush yeni normal merge başlatıp hemen
bekledi, toplam 4,23 s. İlk aday yalnız bu işi ACK sonrasına ertelemek.
İlk drain, WAL prefix ve bütçe beklemeleri açık.
[Kod incelemesi ve deney tasarımı](NORMAL-MERGE-WAIT-DESIGN-50K-20261114.md)

14 Kasım — Maske düzeltmesi sonrası tekrarlar tamamlandı. Olay görünürlüğü
24/24 ≤1 s, p95 84,34 → 110,47 ms, başlangıç 52 → 63 run. Kesintisiz
disk 30/30 ≤1,3×, en yüksek 1,285×. Orijinal 120 s pin tekrarında
1,075×, eski WAL temizliği ve canlı yazıcıda 41 exact sorgu **GEÇTİ**.
Kill/reopen ve eski okuyucu tüm ölçüm kollarında geçti. Pin sırasında
revizyon rewrite yayımı 0. ABBA'daki 3,11 s uçta normal merge join
1,89 s. Düzeltmenin bağımsız süre maliyeti ayrıştırılmadı.
FDB slotu ready, 10k sequence 41. 100'lük uçlar ve geniş taramalar açık.
[Önce/sonra raporu](WAL-PREFIX-FOLLOWUP-20261114.md)

14 Kasım — 16 tohumlu prefix/merge/reclaim/GC oracle'ı **GEÇTİ**.
Maske reset'i çıkarılan geçici derleme, yalnız eski revizyonla eşleşen
sorguda sessiz yanlış sonucu yakaladı (exact toplam 1, model 0).
Performans ve orijinal pin tekrarları yukarıdaki ayrı raporda.
[Model testi ve ham kanıt](WAL-PREFIX-PUBLICATION-ORACLE-20261114.md)

14 Kasım — WAL prefix yayımından sonra yazıcının eski run maskelerini
kullanması küçük testte aynı `checked page slot mismatch` hatasını verdi.
Yayımda yazıcının maskesi geçersiz kılındı. Eski snapshot, 10k GET/version,
41 exact/seri capped/melez capped, process-kill/strict reopen ve
dondurulmuş okuyucu **GEÇTİ**. FDB slotu `ready`, sequence 41.
120 s disk/pin testi tekrarlanmadı. Toplu yazma uçları ve geniş taramalar
açık. [Düzeltme ve kanıt](WAL-PREFIX-VISIBILITY-10K-20261114.md)

14 Kasım — 10k / 120 s uzun snapshot tanısında disk örnekleri
≤2× (1,075×), eski snapshot digest ve pinli eski WAL temizliği
başarılı. Revizyon rewrite yayımı 0. Yeni 10k GET/version geçti,
ilk exact sorguda `checked page slot mismatch` ile **doğruluk
KALDI**. Deney sequence 40'ta durdu. Kill/reopen ve eski
okuyucu bu deneyde çalıştırılmadı. [Yarım rapor](RECLAIM-LONG-SNAPSHOT-10K-20261114.md)

14 Kasım — 0.4.0 motor-only RAM **GEÇTİ**, varsayılan ingest
15,49–16,36 MiB ve temiz idle 2,20–2,26 MiB. Planlama ölçüldü,
aktif rewrite tepesi doğrulanamadı. 4/4 kill/reopen/eski okuyucu geçti.
120 s uzun snapshot ayrı başlatma onayı bekliyor. 100'lük uçlar
ve yoğun arama altında RAM açık. [Motor-only ölçüm](RECLAIM-MOTOR-ONLY-RAM-50K-20261114.md)

14 Kasım — Kesintisiz 50k yazıda 30/30 tur ≤1,3× **GEÇTİ**.
En yüksek ve son tur 1,275×, örnekli geçici tepe 1,528×.
92 run ve 78/3.000 adet >1 s commit. 50k/41, kill/reopen
ve eski okuyucu geçti. [Kesintisiz disk koşusu](RECLAIM-CONTINUOUS-DISK-50K-20261114.md)

14 Kasım — 50k/100'lük ABBA commit profili 12.000 commit ve
4/4 kill/reopen+eski okuyucu ile tamamlandı. 144 profilli >1 s
committe 68 prefix join, 15 normal merge join, 6 bütçe ve 55
açıklanamayan süre en büyük kalem. Hazır revizyon join ≤0,02 ms.
Eski seq 2511 ucu hiçbir kolda tekrarlanmadı. [Commit profili](RECLAIM-COMMIT-PROFILE-50K-20261114.md)

14 Kasım — 0.4.0 varsayılan reclaim dosya olayı kapısı **GEÇTİ**.
24/24 aday ≤1 s, p95/max 84,34/106,28 ms. Bir rewrite ve iki
yayım örtüşmesi görüldü. 50k/41 oracle, kill/reopen ve eski
okuyucu geçti. [Ölçüm](RECLAIM-ARRIVAL-DEFAULT-50K-20261114.md)

13 Kasım — FDB 0.4.0 varsayılan reclaim için dosya olayı,
100'lük commit fazları, kesintisiz yazıda disk, motor-only RAM ve
120 s snapshot tasarımı hazır. İlk dört ölçüm için kullanıcı
başlatma onayı, uzun snapshot için ayrı onay bekleniyor. C ABI
bakım tiki ve istatistik alanları taslak. [Tasarım ve sınırlar](RECLAIM-FOLLOWUP-DESIGN-20261113.md)

12 Kasım güncel karar — Arka planda revizyon geri kazanımı (4b)
**varsayılan AÇIK**. Yazıcı hazır işi yayımlar, boşta
`poll_maintenance()` ile ilerletir. 50k/30 tur disk kapısı **GEÇTİ
30/30**, en yüksek 1,281×, son 1,212×. 100'lük commit >1 s
23/3.000, en uzun 2,093 s. 50k/41 kill/reopen ve eski okuyucu geçti.
Uzun snapshot, aralıksız yazı ve gerçek Frunner anahtarıyla disk
sınırı, 100'lük uçların aşama profili, açılış, geniş arama ve C ABI
açık. [Varsayılan karar ve doğrulama](BACKGROUND-RECLAIM-DEFAULT-50K-20261112.md)

Aşağıdaki önceki denemeler kendi ayarlarıyla tarihsel kayıttır.

12 Kasım — İkinci seçenek, yazıcıda dilimli revizyon, 50k/1R+1W
görünürlüğünde **GEÇTİ** (24/24, p95/max 159/166 ms), disk kapısında
**KALDI** (27/30, en yüksek 1,519×, son 1,29985×). Arka plan adayı
30/30, en yüksek 1,290×, son 1,183×. Dilimli adayda 100'lük >1 s
33→80/3.000, en uzun 2,075→1,767 s, yazı+boşluk oranı 1.416→1.307
işlem/s. En uzun yazıcı indeks bitirme adımı 876 ms, olay örneği
bu adımın tamamını beklemedi. 50k/41 kill/reopen, eski okuyucu ve
staged CRC/pin testleri geçti. İki seçenek için üretim kararı açık,
reclaim varsayılanı KAPALI. Uzun snapshot, aralıksız yazı ve gerçek
Frunner anahtarıyla disk sınırı, 100'lük uçların iç profili, açılış,
geniş arama ve C ABI açık.
[Dilimli aday ve karşılaştırma](SLICED-RECLAIM-50K-20261112.md)

12 Kasım — Arka plan revizyonu geçici prototipi 50k/1R+1W dosya olayı
kapısını geçti. 24/24 aday olay ≤1 s, p95/max 127/129 ms. Hazırlıkla
örtüşen altı olayda kuyruk max 0,034 ms. İki saniye yazmasız arayla 30/30
tur disk ≤1,3×, max 1,290×, son 1,183×, örnekli geçici tepe 1,493×.
100’lük committe 33/3.000 >1 s, max 2,075 s; neden aşama düzeyinde
bilinmiyor. Tepe private proses 89,73 MiB; motor-only ölçüm değil.
50k/41 kill/reopen ve dondurulmuş okuyucu iki kolda geçti. Üretim
reclaim varsayılanı KAPALI. Uzun snapshot, aralıksız yazı ve gerçek
Frunner anahtarıyla disk sınırı, açılış, geniş arama uçları ve C ABI açık.
[Arka plan revizyonu deneyi](BACKGROUND-RECLAIM-50K-20261112.md)

12 Kasım — Senkron idle bakımında gelen 24/24 dosya olayı 1 s üstü.
50k/1R+1W adayında olay→ilk sonuç p50/p95 9,07/12,40 s, eşleşmiş bakım
yok kontrolü 66,76/145,27 ms. Toplam gecikmenin %99,24’ü kalan bakım.
48 olay oracle, kill/reopen ve eski okuyucu geçti. Frunner’da senkron
çağrı sırasında yazı kuyruğu açık kalıyor. Arka plan hazırlığı/yayım,
prefix ucu, uzun snapshot, gerçek anahtar disk tavanı ve C ABI açık.
[Bakım sürerken olay ölçümü](RECLAIM-ARRIVAL-50K-20261112.md)

12 Kasım — Idle-only reclaim geçici adayında 50k/30 tur disk kapısı
GEÇTİ 2/2, bakım sonrası max 1,266×. 100’lük commit >1 s 211→15/6.000,
p99 1,91–2,11→0,79–0,82 s. Yazı+bakım throughput’u 746→1.020 işlem/s,
bakım toplamı 304→375 s. Aday yazı yolunda revizyon plan/start/join
0/0/0. Geçici disk tepesi 1,591×. Kill/reopen ve eski okuyucu 4/4.
Genel reclaim KAPALI. Yazı maintenance sürerken gelirse kuyruk etkisi,
seq 624/prefix ucu, uzun snapshot, gerçek Frunner anahtarı ve C ABI açık.
[Idle-only reclaim sonucu](RECLAIM-IDLE-ONLY-50K-20261112.md)

12 Kasım — Reclaim profil ABBA ve ayrı kapalı kol tamamlandı. Açık >1 s
66 commitin 62’sinde join baskın, join’ler toplam %59,27. Foreground
revizyon planı %18,78. B1 seq 724 aynı committe iki revizyon işçisi için
3,92 s bekledi. Kapalı 0/1.300 >1 s, en kötü 960 ms ve 694 ms prefix join.
Tarihsel seq 693/1324 uç büyüklükleri tekrarlanmadı. Genel reclaim kapalı,
idle’a taşıma yalnız kod görüşü. Uzun snapshot, gruplu yazma uçları,
gerçek Frunner anahtarıyla disk tavanı, arama uçları ve C ABI açık.
[Kuyruk tanısı](RECLAIM-PROFILE-50K-20261112.md)

11 Kasım — Reclaim yazma ucu tanı tasarımı hazır. Küçük WAL yolu açıkken
otomatik prefix/fallback akışı revizyon planlamasına girebiliyor. Eski
3,94–4,59 s uçların nedeni bilinmiyor. 23 run karşılaştırmasında fiziksel
kayıt 150.000 / 60.297, maske ve run yükleme payları ölçülmedi. Profil
onayı bekleniyor. Tek pin deneyi ayrı onayda, genel varsayılan KAPALI.
[Tanı tasarımı](RECLAIM-DIAGNOSIS-DESIGN-50K-20261111.md) ·
[Varsayılan karar özeti](RECLAIM-DEFAULT-DECISION-50K-20261111.md)

11 Kasım — Canlı 50k, 1R+1W, 30 tur tamamlandı. Reclaim açık **disk kapısı
GEÇTİ 2/2**, son 1,200–1,251×. Kapalı 5,712× ve 63 run, KALDI 2/2.
Son sıcak strict-open p50 kapalı 1.138–1.199 ms, açık 281–286 ms.
100’lük commit p95 44–48→197–204 ms, max 3,94–4,59 s. Aşama nedeni
bilinmiyor. SQLite son 1,225–1,230×, `busy` checkpoint’lerde en yüksek
bakım sonrası 3,671–4,633×. Oracle/kill 6/6, eski FDB okuyucu 4/4.
Genel reclaim KAPALI. Gerçek Frunner anahtarı/uzun pin disk sınırı,
gruplu yazma uçları, arama uçları ve C ABI açık.
[Canlı disk ve açılış raporu](LIVE-DISK-50K-20261111.md)

11 Kasım — SEG05 dönüşümlü terim tamponu geçici derlemede. Önceki sayfa
sınırı adayına karşı 52 run sıcak strict-open p50 385,51→365,74 ms,
ilk sonuç p50 483,92→462,33 ms, ikisi de 10/10 blok. Açılış tepe private
p95 +0,398 MiB. 789.189 terim/posting ve 6.174 sayfa sınırı eşit, 50k/41
kill/reopen ve dondurulmuş okuyucu geçti. Bu adayın
temiz ve gerçek soğuk açılışı ölçülmedi. Canlı disk tavanı, bakım/yazma
kuyrukları, arama uçları, eşzamanlı yük ve C ABI açık.
[SEG05 tampon deneyi](STRICT-OPEN-SEG05-DECODE-20261111.md)

11 Kasım — SEG05 sayfa sınırı adayı geçici derlemede. 52 run sıcak
strict-open p50 415,28→383,93 ms, ilk sonuç p95 588,58→515,03 ms,
10/10 blok. Açılış tepe private p95 +0,035 MiB. 6.174 sınır alan alan
eşit. 50k/41 kill/reopen ve frozen okuyucu geçti. Terim çözümleme
örneklenen sürenin %37’si. Canlı disk tavanı, bakım/yazma kuyrukları,
eşzamanlı yük, arama uçları ve C ABI açık.
[SEG05 terim ve sayfa sınırı raporu](STRICT-OPEN-SEG05-TERMS-20261111.md)

10 Kasım — SEG05/MET04 iç ölçümünde terim/posting döngüsü baskın.
İki işçide 212,42 ms birikim, bölümün %83’ü. MET04 sayfa/CRC %2,1,
kayıt karşılaştırması %3,45. Ayrıntılı sayaç açılış eşleşmiş medyan
+2,19 ms. Sonraki aday terim decode ile Bloom/fence hazırlığını ayırmak.
[SEG05/MET04 ayrımı](STRICT-OPEN-SEG05-MET04-PHASES-20261110.md)

10 Kasım — iki işçili sıcak strict-open fazları ölçüldü. 52 run yükleme
p50 265,08 ms. En büyük alt aşama SEG05 parse + MET04 sayfa/kayıt
karşılaştırması, iki işçide 252,39 ms toplam ve 174,29 ms duvar kapsamı.
Sayaç açık/kapalı eşleşmiş açılış farkı +1,38 ms. SEG05/MET04 tekrar
okunan mantıksal alan 41,93 MiB. Alt aşama payları ve gerçek kazanç açık.
[Faz raporu](STRICT-OPEN-RUN-PHASES-20261110.md)

10 Kasım — iki işçili strict-open motorda. Ortak iki izin hemen yoksa
seri açılış. Seri cache onarımı, strict CRC ve manifest sırası korundu.
261 test, 50k/41 kill/reopen ve frozen okuyucu geçti. 52 run sıcak
açılış p50 668,20→430,17 ms, ilk sonuç p95 785,80→576,06 ms, 6/6 blok.
Açılış tepe private p95 +0,445 MiB, CPU p50 +15,63 ms. Sıradaki öneri
canlı disk tavanı ile 1R+1W bakım ve gruplu yazma kuyruğunu ölçmek.
[Birleştirme ve doğrulama](STRICT-OPEN-TWO-WORKERS-MERGED-20261110.md)

10 Kasım — iki işçili FDB gerçek soğuk karşılaştırması 4/4 geçerli.
İlk sonuç 1115,91→835,13 ve ters çiftte 1056,93→733,34 ms. 54/54
dosya atfı, fiziksel DiskIO ve birleşik kayıp 0. Collector bazı ölçülemedi.
Sıcak ve soğuk ölçümler prototip kabulünü destekliyor. Motora alma kararı
açık. SQLite/Q30 2b kapsamı, canlı disk tavanı ve C ABI açık.
[Soğuk tekrar ve arşiv](STRICT-OPEN-TWO-WORKERS-COLD-RETRY-20261110.md)

9 Kasım — iki işçi temiz sıcak 50k ve 100k’da 12/12 blokta daha hızlı.
Strict-open p50 268,78→188,94 ve 541,80→304,50 ms. Açılış tepe private
p95 +1,070 ve +1,012 MiB. Gerçek soğuk 0/4 geçerli. Tek işçi oracle’ı
geçti, 32 MiB collector doldu ve birleşik ETL oluşmadı. İki işçi soğuk
kolu başlamadı. Prototip kabulü ve 2b gerçek soğuk açık.
[Ölçek ve soğuk takip raporu](STRICT-OPEN-TWO-WORKERS-FOLLOWUP-20261109.md)

9 Kasım — iki işçili run doğrulaması geçici derlemede, 52 run / 50k
sıcak strict-open p50 701,72→444,91 ms, ilk sonuç p95 816,53→567,01 ms,
12/12 blok. Açılış tepe private p95 +0,871 MiB, CPU p50 +15,625 ms.
50k/41 kill/reopen, frozen okuyucu ve seri cache onarım testi geçti.
Üretime alınmadı. Az run, 100k ve soğuk açılış etkisi açık.
[İki işçi raporu](STRICT-OPEN-TWO-WORKERS-50K-20261109.md)

9 Kasım — sıralı açılış maskesi motorda. 52 run / 50k sıcak strict-open
p50 1322,84→697,11 ms, ilk sonuç p95 1746,19→955,30 ms. Property,
259 test, 50k/41 kill/reopen ve frozen okuyucu geçti. Temiz 50k/100k
açılışında tutarlı süre gerilemesi yok. 100k açılış tepe private p95
4,121→4,270 MiB, fark maske çağrısından önce mevcut. Kullanıcı ölçülen
RAM farkını kabul etti. Kalan aşamaların salt-okuma incelemesi tamamlandı.
Sınırlı run paralelliği yalnız tasarım adayı. 2b gerçek soğuk, canlı disk
tavanı ve C ABI açık.
[Birleştirme raporu](STRICT-OPEN-KEY-SWEEP-MERGED-20261109.md) ·
[Kalan aşamalar](STRICT-OPEN-REMAINING-PHASES-20261109.md)

9 Kasım — sıcak strict-open için sıralı anahtar taraması prototipi,
52 run / 50k kopyada 12/12 blokta hızlandı. Açılış p50 1350,65→730,71 ms,
ilk sonuç p95 1593,08→907,23 ms. Maske bit eşit. İlk sonuca kadar tepe
private p95 +0,20 MiB. Üretim motoru ve varsayılan aynı. 2b gerçek soğuk,
canlı disk tavanı ve C ABI açık.
[Sıralı tarama raporu](STRICT-OPEN-KEY-SWEEP-50K-20261109.md)

8 Kasım — 2b tamamlama penceresinde SQLite sıcak kolu 150 oracle’lı
örnekle bitti. Sıcak ilk sonuç FDB/SQLite 1507,22/302,03 ms,
2–30. sorgular p95 109,01/605,44 ms. Ters çiftin yönetici purge adımı
pencere içinde çalıştırılmadı, yeni cold örnek yok. İki motorun kill/reopen
ve 50.000 GET/version + 41 sorgu oracle’ı geçti. 2b KISMEN.
[Tamamlama raporu](BLOCK2B-COLD-PAIRED-COMPLETION-50K-20261108.md)

8 Kasım — 2b iki dengeli çift denemesi 2/4 başlatıldı, 1/4 teknik
geçerli ve 1/4 süre geçişi. FDB 1868,88 ms ≤2000, 54/54 FileIO ve
DiskIO, birleşik kayıp 0. FDB Q30 cold/warm 131,50/102,81 ms.
SQLite ham 4622,54 ms, System Collector 56 MiB doldu, birleşik iz
yok. Ters çift durdu. İki motor oracle/kill/reopen, FDB eski
okuyucu ve ana dosya/model SHA geçti. 2b KISMEN, disk churn ve C ABI açık.
[İki çift raporu](BLOCK2B-COLD-2S-PAIRED-50K-20261108.md)


8 Kasım — kullanıcı 2b ilk sonuç kapısını **2000 ms** yaptı ve tekrar
istedi. İlk tekrar 1/8 başlatıldı, 0/8 geçerli. Ham FDB 1895,46 ms,
ancak 32 MiB System Collector doldu, WPR birleşik iz çıkarmadı. Warm ve
SQLite başlamadı. 30 ham oracle, kill/reopen/eski okuyucu ve envanter
geçti. Önceki geçerli 1893,59 ms eski 1000 ms kararıyla saklanıyor.
2b açık. SQLite yan dosya oluşumu onaylı, bu tur kullanılmadı.
[2.000 ms tekrar raporu](BLOCK2B-COLD-2S-RETEST-50K-20261108.md)


8 Kasım — dar ETW 2b ana koşusu ilk FDB denemesinde durdu. Ana 1/8
başlatıldı, 1/8 teknik geçerli, süre kapısı 0/8. 54/54 FileIO ve
DiskIO, birleşik kayıp 0. 12 CSV farkı uzlaştırıldı. Cold ilk sonuç
1893,59 ms >1000, warm 1416,95 ms. Q30 cold/warm 96,09/91,52 ms,
yakınsama kararı kullanıcıda. 180 oracle, FDB kill/reopen/eski okuyucu
ve slot/model hash geçti. SQLite performans yok. 2b açık, disk churn
ve C ABI bekliyor.
[Dar ETW ana koşu raporu](BLOCK2B-COLD-SHORT-MAIN-50K-20261108.md)


8 Kasım — kısa cold 2b teknik ön testi GEÇTİ. Bir purge ve bir
kayıt, 54/54 aynı-pencere FileIO ve 54/54 fiziksel DiskIO tanığı,
53.059.584 B hedef okuma, birleşik kayıp 0, collector bazı ölçülemedi.
CSV özet farkı üç iz meta olayı ve cold pencere dışı bir Create
olarak uzlaştırıldı. Tek ilk sonuç 2030,38 ms, 1 s çizgisinin
üstünde. Ana 0/8, 2b açık, kapılar aynı. Disk churn ve C ABI bekliyor.
[Kısa cold ön test sonucu](BLOCK2B-COLD-SHORT-PREFLIGHT-LIVE-50K-20261108.md)


8 Kasım — 2b kısa cold ön testi hazırlandı. Yönetici çağrısı UTC
penceresinde başlamadı, normal okuyucu sinyal süresinde çıktı. 0 purge,
0 WPR, 0 yeni cold açılış. 54/54, DiskIO ve kayıp olay kapıları
ÖLÇÜLMEDİ. 2b açık, ana geçerli 0/8. Disk churn ve C ABI bekliyor.
[Kısa ön test kaydı](BLOCK2B-COLD-SHORT-PREFLIGHT-50K-20261108.md)


8 Kasım — 2b mevcut cold ETL çevrimdışı sonuna kadar çözüldü.
302,86 MB CSV named pipe üzerinden süzüldü, yeni ham CSV yazılmadı.
Eski kesme noktasında ve tam EOF’ta 50/54. Dört eksik kaynak için
cold PID’de adsız FileKey okuma sinyali ve sonraki PID’lerde ad var,
aynı pencere ad/nesne bağı eksik. Birleşik kayıp 0, collector bazı
ölçülemedi. Geçerli cold 0/8, kapı aynı. Yeni purge/ETW ve 50k disk
churn ayrı onay bekler.
[Çevrimdışı iz raporu](BLOCK2B-COLD-ETW-OFFLINE-50K-20261108.md)


7 Kasım — 50k açılış paralel maske deneyi ve Frunner hazırlık tablosu.
Seri strict-open p50 1274,55 ms. 2/4/8 işçi eşleşmiş medyan +258/+907/+908
ms, 0/12 daha hızlı. CPU arttı. 52 run maskesi ve cache 1/2/4/8’de bayt
bayt eşit, 48/48 case0, slot/kopya/model hash geçti. Üretim ve kapılar
aynı. 2b cold, 2c uzun snapshot, 50k 4R+100 ve drain, 3a, C ABI açık.
[Paralel açılış deneyi](STRICT-OPEN-PARALLEL-50K-20261107.md) ·
[Frunner hazırlık tablosu](FRUNNER-READINESS-20261107.md)


7 Kasım — 50k hafif maske tanısı tamamlandı. 20 hafif örnekte açılış
maskesi p50/p95 764,05/801,74 ms, toplamın %59,97’si. 697/1326 çift
aralık kontrolünü geçti. Yeni run’ı 1024–8191 slot olan 288 çift,
ölçülen hide_keys süresinin %91,93’ü. Sayaçsız kontrol 1251,08 ms p50,
hafif kol eşlenmiş fark medyan +36,37 ms. 40/40 case0 ve slot/model hash
geçti. WPR’nin ilk sırası 5+5 ham örnek üretti, System Collector 48 MiB
doldu ve stop 0x800705DE ile kaldı. Ters sıra başlamadı, WPR etkisi
bilinmiyor. 2b açık, yeni purge/çözümleme/optimizasyon yok, kapılar aynı.
[Hafif maske ve WPR raporu](STRICT-OPEN-MASK-WPR-50K-20261107.md)


7 Kasım — 50k sıcak strict-open aşama tanısı tamamlandı. Profilsiz
p50/p95 1272,43/1313,83 ms. Profilli doğrudan p50 1742,12 ms ve
görünürlük maskesi profilli toplamın %69,67’si. Ayrıntılı profil on
eşleşmiş blokta medyan +476,28 ms. Profilsiz maskenin mutlak süresi ve
WPR maliyeti bilinmiyor. 40/40 case0 oracle, canlı slot/kopya 159/159 SHA
ve model hash geçti. 2b açık, cold kapıları ve varsayılan aynı. WPR
karşılaştırması ve cold tekrar ayrı onay bekler.
[Sıcak açılış aşama raporu](STRICT-OPEN-PHASES-50K-20261107.md)


7 Kasım — 2b ana koşu ilk denemede durdu. FDB ham ilk sonuç 1770,33 ms,
warm 1384,58 ms. Collector sınırı ve 50/54 okuma tanığı nedeniyle iz
geçersiz. Birleşik kayıp 0, collector bazı ölçülemedi. Ana 1/8 başlatıldı,
0/8 geçerli. SQLite performans yok. Oracle/kill/reopen/eski okuyucu geçti.
SQLite ana DB SHA aynı, boş WAL/SHM kapanışta eksik, yerleşim KALDI.
Toplam izlenen 410,9 MiB, ETL kapanış 61,8 MiB. Yalnız kendi geçici CSV'si
kanıt süzüldükten sonra silindi. 2b açık ve tekrar kullanıcı kararı bekler.
[Ana koşu raporu](BLOCK2B-COLD-ETW-MAIN-50K-20261107.md)

7 Kasım — kullanıcı ETW ön testinin sıcak/cold zincirini kabul etti.
Ana iz kuralı birleşik tracerpt kayıp 0 ve eksiksiz hedef okumaları.
Her satırda “collector bazı ölçülemedi”. Tek satırdan çağrılan yönetici
betiği AST hatası 0. Disk rezervi ve yalnız kendi geçici CSV'sini silme
kuralı hazırlandı. Ana 0/8, ayrı “başla” bekliyor.
[Ana koşu planı](BLOCK2B-COLD-ETW-MAIN-PLAN-50K-20261107.md)

7 Kasım — tek satırlık bağlantı provası ACK aldı. Sonraki ETW ön testi
1 purge ve 1 kayıt üretti. Sıcak eşlenen DiskIO iki hedefte 0. Purge
sonrası okuyucuya FDB 2 KiB ve SQLite 64 KiB bağlandı. Birleşik kayıp 0,
collector sayaçları açık. Ana 0/8 ve kapılar ölçülmedi. Ana ETL bütçesi
de doğrulanmadı. 2b açık.
[ETW kanıt raporu](BLOCK2B-ETW-LINK-PREFLIGHT-50K-20261107.md)

7 Kasım — onaylı ETW ön testi için okuyucu hazırlandı. Yönetici bloğundan
başlangıç sinyali gelmedi, başlatma penceresi doldu. Okuma fazı 0/3,
purge 0, ETW 0, ana deneme 0/8. **Gerçek cold kanıtlanamadı.** Dosya/PID,
kayıp olay ve performans kapıları ölçülmedi.
[ETW ön test raporu](BLOCK2B-ETW-PREFLIGHT-50K-20261107.md)

7 Kasım — kullanıcı 0 purge, 0 ETW ve 0/8 ana deneme sonucunu kabul etti.
Yeni tasarım, elle RAMMap tanığı yerine tek ETW kaydında sıcak okuma,
purge ve aynı aralığın cold okumasıyla ön test öneriyor. Dosya/PID ve
FileIO/DiskIO eşlemesi ile kayıp olay 0 kapısı açık. Sekiz ana deneme
aynı purge + ETW kanıt etiketiyle planlandı. 10 dk ön test onayı ve
sonrasında ayrı ana “başla” onayı bekleniyor. 2b kapısı ölçülmedi.
[ETW tasarımı](BLOCK2B-COLD-ETW-DESIGN-50K-20261107.md)

7 Kasım — onaylı dar 2b ön testi durdu. Canlı 1.853 B kaynak manifest CRC
kontrolünden geçti ve salt okunur olarak okundu. RAMMap pozitif `Total > 0`
tanığı gelmedi. Purge 0, ETW 0, ana trial 0/8. Cold kapısı ölçülmedi.
[Ön test raporu](BLOCK2B-COLD-PREFLIGHT-50K-20261107.md)

4 Kasım — kullanıcı sonraki RAMMap görüntüsünde hedef-prefix için iki yönde
RAM sayfası saptanmamasını **yalnız o an için kabul etti**; eski purge anını
geriye dönük kanıtlamaz. [Tasarım revizyonu](BLOCK2B-COLD-CACHE-REDUCED-DESIGN-50K-20261104.md)
hazır: küçük canlı DB dosyasında bir pozitif→purge→negatif denemesi, bir kısa
ETW uygunluk kaydı; ana case0 `olduğu`,4 dengeli çift/8 cold+8 warm,
1440 zamanlı sorgu, ilk/son RAMMap, ara6 ETW destekli. Toplam40dk, preflight
dahil9 purge/9 trace önerisi **ONAY BEKLİYOR; bugün yürütme yok**.
ETW gerçek dosya/PID/storage eşleme ve lost-event0 hâlâ doğrulanmadı;
ara6'da disk izi tüm dosyaların resident0 olduğunu tek başına kanıtlamaz,
kanıt kapsamı ayrı belirtilir.≤1s Open-start→ilk sonuç ve yakınsama kararı
aynı.2b ana ölçüm başlamadı;3a KISMEN,2c/4 ve önceki açık işler aynı.

4 Kasım [beş dakikalık ek preflight](BLOCK2B-PREFLIGHT3-50K-20261104.md):
File Summary/Find görüldü; purge öncesi prefix bulunamadı, tam kapsamı bilinmiyor.
Tek `-Es`→`-Et` dizisi denendi (1/1), iki parent exit0. Resident0 tanığı yok;
**gerçek cold kanıtlanamadı**, ana2b/ETW başlamadı. Önceki purge0 kayıtları
yalnız kendi tarihsel pencerelerine aittir; toplam deneme artık1.

4 Kasım — **ikinci 2b preflight:** yerel RAMMap1.63 kendi Usage ekranı
ve `-Es`/`-Et` CLI desteği doğrulandı; menüyle24 purge zorunlu değil.
[Yeni rapor](BLOCK2B-PREFLIGHT2-50K-20261104.md). Yönetici penceresine
otomasyon girdisi işlemiyor; hedef dosyaların resident0 tanığı alınamadı.
**Gerçek cold kanıtlanamadı; purge0/1, ETW kaydı0, ana koşu başlamadı.**
WPR yalnız yetenek kontrolü: gerçek file/PID eşleme ve event kaybı bilinmiyor.
SQLite read-view önkoşulu tekrar geçti (sequence1026311, ana DB SHA aynı).
Süre başlangıç/bitiş kaydı rapor ve JSON'dadır. Slot/model/varsayılan/kod
değişmedi. Ana koşu için ayrıca “başla” gerekir; tanık/ETW önkoşulları açık.
3a KISMEN/özgün kapı ölçülmedi;2c/4 ve yazma/arama/RAM-disk/eşzamanlı
yükte önceki açık maddeler aynı, başka bloğa geçilmedi.

Önceki preflight'ın tarihsel kayıtları:

4 Kasım ek bilgi — RAMMap yolu sonradan geldi:
`C:/Users/Serkan/Downloads/RAMMap/RAMMap.exe`, sürüm1.63, Microsoft imzası
Valid (yalnız metadata). İlk15dk preflight penceresi dolmuş olduğundan
çalıştırma/yardım/purge yapılmadı. [Ek kanıt](BLOCK2B-PREFLIGHT-50K-20261104.md).
Cold hâlâ kanıtlanmadı; kalan kontroller için yeni süre onayı, ana koşu için
ayrıca “başla” gerekir. Diğer bloklara geçilmedi.

4 Kasım 2026 — **2b preflight: GERÇEK COLD KANITLANAMADI; DURULDU.**
[Rapor/kanıt](BLOCK2B-PREFLIGHT-50K-20261104.md): kontroller555s, rapor dahil777/900s;
RAMMap aranan yerlerde yok, yerel yardım/CLI/otomasyon bilinmiyor, kullanıcıya
yol soruldu. Purge0/1, resident0 tanığı yok. WPR yerel yardım/profil metadata'sı
alındı; gerçek file/PID eşlemesi, event kaybı ve sert ETL boyut sınırı kayıt
olmadan doğrulanmadı. ETW kaydı yetki dışıydı; kayıt başlatılmadı.
SQLite readOnly/query_only iki ardışık view önkoşulu NodeSQLite'da geçti;
future Rust harness doğrulaması değildir. DB SHA/sequence aynı; slot kaydı
değişmedi. Ana koşu/boot/yeni araç/diğer bloklar yok. GUI zorunluysa24 purge
12–36dk tahmini;12/16/8 purge seçenekleri raporda, kapsam kararı kullanıcıda.
Önkoşullar kapanınca ana koşu için ayrıca **“başla”** onayı gerekir.
3a KISMEN/özgün kapı ölçülmedi, yeni3a turu yok; diğer açık işler korunur.

Önceki tasarım kayıtları:

4 Kasım 2026 — **Yalnız belge/tasarım tamamlandı.**
[Blok4 karar tablosu](BLOCK4-DECISIONS-20261104.md): mevcut kanıt/artı-eksi/
eksik test/öneri ayrı; küçük WAL genel default KAPALI, Frunner opt-in adayı;
reclaim KAPALI kararı korunur, tek okuyucuda koşulsuz disk tavanı yok;
Owned azaltma imkânsız denmedi, Frunner-only-streamed API işareti onay ve
giriş/atomiklik/bellek doğrulaması bekler. Varsayılan veya API etiketi uygulanmadı.

[2b gerçek-soğuk cache tasarımı](BLOCK2B-COLD-CACHE-DESIGN-50K-20261104.md)
hazır; **ölçüm BAŞLAMADI**. Ana yöntem yönetici RAMMap + cache/disk tanığı;
restart ayrı seçilen/onaylanan alternatif. Önceden sabit24cold/24warm,
4320 zamanlı sorgu; hazırlık15 + ölçüm/doğrulama45 = toplam60dk üst sınır.
4 Kasım kullanıcı kararı: **açılış dahil ilk sonuç≤1s**;30. arama sıcak A/A
yayılımıyla yan yana sunulur, **sayısal kabulü kullanıcı sonuçta kararlaştırır**.
Tanımlar kabul edildi; ölçüm/yönetici cache temizliği için ayrı uygulama onayı
beklenir. Yeni eşik yok.
Bu tur kod/ölçüm/DB işlemi/test/eviction/reboot yapılmadı; slotlar onarımda
doğrulandıkları ready durumunda bırakıldı, güncel DB kontrolü iddiası yok.
**3a KISMEN, özgün kapı ÖLÇÜLMEDİ; yeni3a turu yok.** 2c/diğer bloklar ve
C ABI başlamadı; yazma/bakım, arama, RAM/disk/eşzamanlı yük açıkları korunur.

Önceki kabul/onarım kayıtları:

3 Kasım 2026 — **Yalnız çalışma alanı onarımı TAMAMLANDI.**
[Temizlik raporu/ham kanıt](BLOCK3A-REPAIR-20261103.md): iki motorda izinli16
anahtar tam GET/content/version doğrulamasından sonra silindi. Ana50k'ya
mutation gönderilmedi. Reopen →50k GET/version+41digest →kill/reopen →aynı
kontroller ve eski okuyucular geçti. SQLite'ın gerçek pre-delete sequence'i
1026310; hazırlıktan sonra **1** update gözlendi. Son/model sequence'leri
FDB529967,SQLite1026311. Model rows/dead/next değişmedi; iki slot **ready**,
pending_diagnostic blokesi kaldırıldı. Açık onarım işi kalmadı.

**3a: KISMEN, tanı yeterli sinyal verdi; özgün kapı (zx/the zx ≥%30,
diğer36 ±%5 + A/A) ÖLÇÜLMEDİ.** Gap/run ve zx adjacency sinyalleri raporda
“sinyal, kanıt değil” olarak korunur; önceki kötü/eksik sonuçlar silinmedi.
Yeni ölçüm/harness düzeltmesi/bakım/payload optimizasyonu0. Motor/fixture/
corpus/eski harness SHA'ları aynı. 2b/2c/4 ve diğer açık işler için kullanıcı
kararı beklenir; sonraki bloğa geçilmedi, kapı/eşik değişmedi.

Önceki onarım-öncesi tarihsel durum:

3 Kasım 2026 — **3a tanısı YARIM: ölçüm adaptörü hatasında DURULDU.**
[Son rapor/ham tablo](BLOCK3A-DIAGNOSIS-20261103.md): SQLite eski snapshot ve
yeni acquire aynı bağlantıda nested BEGIN yaptı; motor doğruluk hatası kanıtı
değil. DB bloğu131,430/1200s, derleme/test801,754/1800s. A/A tamamlanmadı;
tek bakım çağrısı0, L1 yok. Yeni tur/optimizasyon yok. FDB tek kol zx/the zx
p95 128,776/46,877ms, gap16,831ms; çift yok, kazanç veya gürültü sonucu değil.
Normal gapte4 aktif okuyuculu örnek0: 4R kabul verisi sayılamaz.

**İki owner slotu `blocked_partial_diagnostic`:** mevcut ayrı
`keybench-numeric` koleksiyonunda16 anahtar (`0000000000..0000000015`) kaldı.
Model dosyaları ana50k için korunur, sequence stale; ready diye kullanılmaz.
Hata sonrası DB reopen/temizlik yapılmadı. Devam onayıyla önce havuz ve ana
oracle doğrulaması, yalnız bu kayıtların temizliği/model eşitlemesi; ölçüm
adaptöründe ayrı pin client ve bounded okuyucu örtüşmesi ele alınmalı.
Bu işler otomatik yeni koşu onayı değildir. Payload birleştirme **ÖNERİ-YOK**;
run–gap nedenselliği bilinmiyor. Özgün3a 10k/38 kapısı ölçülmedi, eşikler aynı.
Default230/diag234 + son sayaç3/example26 test; fmt/normal+diag clippy geçti.
Gerçek50k son kill/reopen/eski okuyucu eksik; fixture testleri yerine sayılmaz.
Yazma/bakım, arama, RAM/disk/eşzamanlı yük ve diğer bloklar açık kalır.

Önceki ara durum:

3 Kasım 2026 — 3a'nın **yalnız 50k tanısı kullanıcıca onaylandı ve başladı**.
[Tanı raporu/ham kanıt](BLOCK3A-DIAGNOSIS-20261103.md),
[1200 s ölçüm / 1800 s derleme-test ve örnek üst sınırları](benchmarks/block3a-20261103-plan.md).
Önce aynı yerleşimde A/A; tek mevcut bakım çağrısından sonra run azalırsa L1.
Yeni aday/ayar/optimizasyon yok; özgün 10k/38-case kapısı henüz ölçülmedi.
Diğer yazma/bakım, arama, RAM/disk/eşzamanlı yük açıkları korunur.

Önceki durum kayıtları aşağıdadır.

3 Kasım 2026 — **Brife göre numaralama düzeltildi.**
[Tek sayfa 1a–4c durum tablosu](BRIEF-BLOCK-STATUS-20261103.md) esas alınır:
Blok 1 kabul edilen 10k kapsamında kapalı; Blok 2 KISMEN (2b/2c başlamadı),
3a/3b kısmen tanılandı; 4a/4c tamamlanmadı, 4b KAPALI bırakma kararı korunur.
[3a tanı-önce tasarımı](BLOCK3A-DIAGNOSIS-DESIGN-20261103.md) ayrı onay bekler;
bu tur kod/ölçüm yok. Eski kitap case34 ishmael /130/1 ile TR case34 farklıdır.
Geniş tarama/yerleşim, yazma-bakım/100'lü ve 500 drain, RAM/disk/eşzamanlı
yük açıkları kendi rapor kapsamlarıyla korunur; kapı/eşik değişmedi.

3 Kasım 2026 — **son cache takip deneyi Ek-TC kapandı; A/D varsayılan yapılmadı, mevcut
try/seri K korunuyor.** [Kapanış/kapılar/ham tablolar](TERM-CACHE-50K-20261102.md).
2 Kasım başlayan sabit K/A/SQLite/D4x4 matris tamamlandı; yeni tur yok.

|Kapı|K|A: tek decoder/kilit dışında|D: yalnız4MiB sayfa|
|---|---|---|---|
|Normal41 N60, blok geçti/41|41/40/40/40 KALDI|41/40/41/41 KALDI|40/41/41/40 KALDI|
|Toplam gap/ilk-query net kazanımı|Referans|2/4 hızlı,2/4 yavaş: KANITLANMADI|2/4 hızlı,2/4 yavaş: KANITLANMADI|
|Havuz tekli commit20/50/100ms|GEÇTİ|GEÇTİ|GEÇTİ|
|Havuz görünürlük20/50/100ms|KALDI|KALDI|KALDI|
|Uzun16k commit20/50/100ms p95/p99/p99,9|GEÇTİ|GEÇTİ|GEÇTİ|
|Yazma korunum >=%75|%88,416 GEÇTİ|%84,821 GEÇTİ|%83,488 GEÇTİ|
|SQLite pooled%85,103 üstü korunum|GEÇTİ|KALDI|KALDI|
|Fresh/warm motor-only idle<=15MiB|GEÇTİ|GEÇTİ|GEÇTİ|
|Oracle/CRC/sıra-pin/kill-reopen/eski okuyucu|GEÇTİ|GEÇTİ|GEÇTİ|

Eşik/kural değiştirilmedi. A gap p95 farkları+2,223/−5,460/−4,875/+0,372ms;
D+3,953/−2,574/−5,108/+2,412ms. K'nın kendisi12,338→32,385ms ilerledi.
Normal bloklar sonrası canlı run7, havuz sonrası52: aynı mantıksal veri,
aynı fiziksel yerleşim değildir; kaynağın ilerlemesi/owner/tahliye paylarının
kesin nedenselliği bu bloktan ayrıştırılmadı. Kilit etkisi ve kapasite etkisi
raporda ayrı; sayaç küçülmesi tek başına başarı sayılmadı.

D fresh idle3,418–3,641MiB, warm10,801–12,980MiB. A'nın normal-page kontrollü
resident+in-flight rezervasyon tepesi960KiB, çıkışta0; allocator/RSS sayacı
değildir. 4R+W proses peak tablosu model/oracle/history içerir, motor-only diye
sunulmaz; A tüm süreç peak artmaması kanıtlanmadı. Ingest60MiB yeniden ölçülmedi.
15 kill tanığı strict-open+oracle sonrasındadır; in-flight/power-loss iddiası yok.
Root test/fmt/clippy ve SQLite adaptör kontrolleri geçti; ölçülen frozen ikili
ve source hashleri sabit, corpus/0.1 kaynakları korundu; havuzlar temiz/ana50k
korundu. Büyük log satırı okuyan tek yardımcı izleyici durduruldu; native kola
dokunulmadı, zamanlama etkisi bilinmiyor ve seçici tekrar yapılmadı.

A/D yalnız diagnostic opt-in; B/C veya yeni aday/ayar/ölçüm turu açılmayacak.
100lü/C500drain/2,6sn/WAL handle/File::open/zx iyileştirmesi/reclaim varsayılanı/
C ABI yapılmadı. Reclaim varsayılan KAPALI; önceki signed-zero, geniş tarama/
case34, yazma/bakım ve disk/sorguRAM/eşzamanlı yük maddeleri açık kalır.

Aşağıdaki kayıtlar bu deneyin başlangıcı ve önceki adımların tarihsel durumudur.

2 Kasım son cache takip deneyi Ek-TC kullanıcı onayıyla uygulama/doğrulama aşamasında.
K/A/SQLite/D dengeli4x4 sıra uygulama öncesi kullanıcıya gösterildi ve
TERM-CACHE-DESIGN başına kaydedildi. D yalnız4MiB page,64KiB descriptor;
A tek decoder/kilit dışı read-CRC-decode. K mevcut varsayılan kalır.
Henüz performans sonucu yok; kapı/varsayılan değişmedi. Yeni DB kopyası/ingest
yok; B/C ve diğer kapalı işler başlamaz. Doğrulukta hemen dur, sabit performans
bloğu sonunda net kazanç yoksa veya kapı kalırsa Ek-TC kapanır.

2 Kasım kullanıcı kararı: VISIBILITY-PHASE (Ek-VP, 2a takip) tanısı kabul edildi. Ek-TC için
**son sınırlı deney tasarımı yazıldı; uygulama/ölçüm başlamadı.**
[Kod incelemesi, üç aday ve değişmeyen kapılar](TERM-CACHE-DESIGN-50K-20261102.md).
Her commit cache'i temizlemiyor: snapshot mevcut Arc/owner'ları paylaşır;
yeni indeks owner'ı ve kapasite tahliyesi ayrı mekanizmalardır. Önceki miss'lerin
bu nedenlere dağılımı bilinmiyor. Öneri tek global decode slotuyla read/CRC/decode'u
cache mutex'i dışına almak; mevcut byte sınırları, legacy/cancel/pin korunmalı.
Tek-uçuş ve sharding yalnız alternatif tasarımdır; uygulama onayı değildir.
Onay gelirse aynı N60 normal41 + Frunner gap, dört dengeli A/SQLite/B çifti;
eşikler aynı, bütün iyi/kötü sonuçlar. Net iyileşme yoksa veya kapı kalırsa
mevcut varsayılan korunup Ek-TC kapanır; yeni aday/ayar/tekrar turu açılmaz.
Doğruluk hatasında hemen durulur. Bu tur kod/DB/corpus/test/benchmark değişikliği
yok. 100lü/C/2,6sn/WAL handle/File::open/zx/reclaim-default/C ABI başlamaz.

2 Kasım görünürlük tanısı **TAMAMLANDI; kapı yok, bu turda duruldu.** Tek
dengeli blok/numeric-Frunner×FDB-SQLite, aynı257-record havuz,4096+1 işlem,
4R veN60+. Motor/format/try-seri default kaynakSHA aynı; optimizasyon yok.

|Ölçülen ortalama ms|FDB numeric|FDB Frunner|SQLite numeric|SQLite Frunner|
|---|---:|---:|---:|---:|
|Commit|2,847|2,905|8,716|8,678|
|Commit→visibility gap|26,936|30,115|5,742|5,689|
|Acquire|0,000359|0,000405|0,141919|0,143148|
|İlk marker sorgusu|26,929|30,108|5,572|5,519|
|Posting/sözlük hazırlığı (sorgu altı)|26,555|29,710|ÖLÇÜLMEDİ|ÖLÇÜLMEDİ|
|Term-cache edinimi (posting altı)|11,501|13,329|ÖLÇÜLMEDİ|ÖLÇÜLMEDİ|
|Term-page read/CRC/decode (posting altı)|8,780|9,454|ÖLÇÜLMEDİ|ÖLÇÜLMEDİ|
|Query maskesi|0,024|0,027|ÖLÇÜLMEDİ|ÖLÇÜLMEDİ|
|Delta filtre|0,239|0,247|ÖLÇÜLMEDİ|ÖLÇÜLMEDİ|

Gapin%99,97'si ilk sorgu,%98,6'sı posting hazırlığı. İki alt kalem olan
cache edinimi ve sayfa read/CRC/decode üst toplamlarla tekrar toplanmaz.
Term-cache kilidinde gerçekten bloklanan genel-cache sayaçları da11,385/
13,197ms ölçüldü; hangi mutex sahibi threadin nedeni olduğu bilinmiyor.
Probe başına306/331 term-page miss ve1,28/1,38MB istenen read byte var;
bunlar fiziksel disk/OS-cache miss sayısı değildir. Snapshot prepare0,185/
0,223ms ve snapshot.mask0,097/0,129ms **commit içinde** ölçüldü. Acquire aynı
sequence'i hemen verdi. Bu koşuda süre yayım kuyruğunda değil, ilk doğrulama
aramasında birikiyor; özgül blocker paylaşılmış terim-cache/sözlük yolunda.

SQLite query'nin numeric/Frunner için5,514/5,459ms SQL-step+payload-copy maliyeti ve
0,040/0,042ms JSONdecode ölçüldü; gerçek EXPLAIN FTS virtual-table L0 yolunu
gösterdi. Yeni tanıdaki fark ölçüldü; eski B bloklarının neden değiştiği
bilinmiyor. Profil açık gap27–30ms: eski~15ms ile aynı sayı üretilemedi.
Profilleme maliyeti/oturum/ilerlemiş yerleşim etkisi ayrı ölçülmedi. Eski
profilsiz örneklerin tam aşama dağılımı bilinmiyor. Scheduler/antivirüs ya da
CRC vsdecodeCPU ayrımı için ölçülmemiş neden atfı yapılmadı.

16.388 işlem timeline'ı monoton; ardışık aşamalar visibility toplamına tam
eşit, toplama/JSON/EXPLAIN visibility dışındadır.256+161+3 test/fmt/default ve
diagnostic clippy,9kill/reopen/eski okuyucu ve ana50k GET/41case/SQLdigest
geçti. Havuzlar iki motorda0 canlı kayıtla temizlendi. Yeni corpus/DB kopyası/
50k re-ingest yok. [Tanı, ham sıralı örnekler ve sınırlar](VISIBILITY-PHASE-50K-20261102.md).
100lü,C500drain,2,6sn,WAL handle/File::open,zx,reclaim varsayılanı,C ABI yok.
Geniş tarama/case34,signed-zero,yazma/bakım,disk/sorguRAM başlıkları açık.

2 Kasım kullanıcı kararı: B sonucu kabul edildi; Frunner anahtarı doğruluk ve
commit açısından kabul. Görünürlük p95 ham kararı **KALDI** olarak korunur.
**SONRADAN REVİZE: kullanıcı kararıyla ilerlendi, eşik değiştirilmedi.**
Gözlemler: numeric kontrol de blok3'te görünürlük31–32ms ile kaldı. Blok0–2'de
Frunner görünürlük p95 farkları işlem tiplerine göre+1,834..+9,164ms; ilk50
metin4247315B vs144412B olduğundan anahtar/payload maliyeti ayrılmadı.
Numeric pooled görünürlük p50 bloklarda3,912→7,977→9,872→19,681ms arttı
(yaklaşık4→20ms). Bunlar gözlemdir, kural/eşik veya neden atfı değildir.

Kabul anındaki tek seferlik görünürlük aşama tanısı planı (yukarıda tamamlandı):
Commit dönüşü, benchmark history yayımı, acquire, sorgu hazırlığı, ilk sorgu
ve release ayrı; mevcut opt-in sayaçlarla snapshot maskesi ve query delta
filtre/maskesi ayrılır. Aynı havuz/gövde/yol/4096 işlem ve4R düzeni; tek
dengeli numeric/Frunner×FDB/SQLite bloğu, yeni DB kopyası/reset yok.
Ölçülmeyen alt nedenler bilinmiyor yazılır. Optimizasyon/sonraki işler yok.

2 Kasım B kapanışı: **Frunner anahtarı ayrı havuzda ölçüldü; performans kapısı
KALDI, bu adımda duruldu.** Ölçüm planı önce gösterildi; kullanıcı küçük havuz
ve ayrı collection istedi. Mevcut50k `texts` aynı;256 mevcut donör+1 uzun-yol
slotu `keybench-numeric`/`keybench-frunner` içinde. Yeniden50k ingest/rekey,
DB kopyası veya yeni corpus yok. Dört dengeli blokta16 native kol tamamlandı.

|B kapısı|Sonuç|Karar|
|---|---|---|
|Frunner tekli commit p95/p99/max20/50/100ms|Normal tip p95 2,22–4,94ms; bütün commit kuyrukları eşik içinde|GEÇTİ|
|Frunner görünürlük p95/p99/max20/50/100ms|p95 20,148–26,749ms; p99/max geçiyor, p95 geçmiyor|KALDI|
|Havuz5 N60 okuma, idle≤30→loaded≤30; aksi≤2×idle|Frunner3/5,3/5,4/5,3/5; numeric3/5,4/5,3/5,3/5|KALDI|
|SQLite aynı havuz okuma referansı|Numeric5/5 her blok; Frunner5/5,5/5,4/5,5/5|REFERANS|
|Ters-mtime başa ekleme;atomik sil+ekle;eski snapshot;1000B yol|Her havuz model oracle'ı geçti;1017B ID sınır içinde|GEÇTİ|
|CRC/kill-reopen/eski okuyucu;ana50k koruma|9 tanıklı kill,8 FDB pool kolu legacy257GET+ilk50;ana50k oracle/digest|GEÇTİ|

Yalnız anahtar boyutuna neden atfı yok: son marker ilk50 metni numeric144412B,
Frunner4247315B. Girdi donörleri aynı fakat ters-mtime farklı ve daha büyük
ilk50 belgeleri döndürüyor; saf key-byte CPU maliyeti izole edilmedi. Kaçırmanın
kanıtlanmış aşaması/kök nedeni yok; profil/optimizasyon yapılmadı. Commit/s
görünürlük ve her-işlem oracle'ı dahil, eski uzun-seri ops/s ile doğrudan
kıyaslanmaz. Kaynak/yerleşim ve sequence de ilerliyor; gürültü iddiası yok.

İki havuz canlı kaydı0,ana50k50000; post-inspect iki motorda doğrulandı.
Logical cleanup başlangıç fiziksel alanına dönüş değildir: FDB canlı kaynak
(WAL dahil)186768163→255349925B,.idx140057305→167065177B; retained son
422418358B. SQLite retained477908992B. Başlangıç toplam retained sayılmadı,
büyüme oranı verilmedi. Reclaim default kapalı. Model/oracle/history/API
sonuçları içeren süreç peak RAM'i D motor-only kapısı diye sunulmadı.

256 default+161 diagnostic+3 SQLite test;fmt/root ve diagnostic clippy,
son ilgili3 havuz testi/SQL3test geçti. 65.552 commit örneği,1024 body×path
hücresinde64'er örnek;240 read-phase satırındaN60–3984. Motor sourceSHA,
korpus/0.1fixture ve frozen ikililer korundu;engine accepted-default ile aynı.
İki hazırlık hatası(DB worker açılmadan donor duplicate/str-Path) düzeltildi;
eski loglar saklandı.35 normal ölçüm/cleanup/inspect child exit0; bilinmeyen
kapanış yok. [B sonuçları ve ham tablolar](FRUNNER-KEY-50K-20261102.md).

Yeni default korunur;100lü,C500drain,2,6sn,WAL handle/File::open,zx iyileştirmesi
başlamadı. Geniş tarama/case34,signed-zero,yazma/bakım/disk/sorguRAM açık.
Aşağıdaki kabul notu ve eşikleri değiştirilmemiş önceki sonuçlar korunur.

2 Kasım 2026 kullanıcı kabulü: HYBRID-DEFAULT sonucu kabul edildi, yeni
try/seri varsayılan kalır. Normal41 ham kararı **KALDI** olarak korunur.
**SONRADAN REVİZE: kullanıcı kararıyla ilerlendi, eşik/kural değiştirilmedi.**
Gözlem: kapı kendi idle değerinin30ms basamağına bağlı olduğundan daha hızlı
mutlak süreyi de bırakabilir. Aynı çiftte korkaklığa: eski42,433→65,630ms
GEÇTİ; yeni26,160→41,535ms KALDI. Kaçırmalar mevcut sınıra göre
0,425–15,239ms; güzeli zafer'in0,425ms aşımı30ms'ye değil61,383ms sınırınadır.
Bazı ağır örneklerde birkaç kat kazanç var: pair0 timmy245,358→36,186ms
(6,78×), pair2 yüklük167,595→33,018ms(5,08×), güzeli zafer308,149→61,808ms
(4,99×). Bu bütün kaçırmalar için4–10× demek değildir: pair1 görenler
25,862→41,747ms geriledi. Bunlar gözlemdir; kural veya ham tablo değişmedi.

Sıradaki B, kullanıcı seçimiyle mevcut50k tabanın yanında **ayrı havuz
koleksiyonları** kullanır; bütün50k yeniden anahtarlanmaz. Numeric kontrol
ve Frunner havuzları karışmaz. En yeni dosyanın ters-mtime ile başa gelmesi,
mtime sil+ekle atomikliği ve küçük N60 okuma kontrolü ölçülür.
[Önceden yazılan ölçümler ve eşikler](FRUNNER-KEY-DESIGN-50K-20261102.md).
100lü,C(500drain),2,6sn,WAL handle/File::open,zx iyileştirmesi başlamaz.

2 Kasım 2026 — **SONRADAN REVİZE: kullanıcı kararıyla try/seri VARSAYILAN.**
Hibrit200 bütçesinden sonra tek non-blocking WORKERS.try_lock(); izin yoksa
aynı cursor/frontier ile çağıran threadde seri devam, yeni thread/tekrar yok.
`REFERENCE_WAIT=true` gizli thread-local tanı/geri dönüş anahtarı olarak kaldı.
Exact/serial, capped sonuç/sequence, cancel, global izin/bellek muhasebesi,
strict CRC, pin ve kalıcı biçim değişmedi. Kabul edilmiş maske+capped delta korundu.

|Önceki sıfır-toleranslı kapı|Ham karar|Kullanıcı kararı|
|---|---|---|
|4R+W peak private artmama|KALDI: +1,5..+4,2 MiB (yaklaşık33→36 MiB)|SONRADAN REVİZE: kullanıcı kararıyla bilinen maliyet olarak kabul edildi, kapı eşiği değiştirilmedi|
|Altılı p95 artmama|KALDI: 4R zx1055→1387ms (+%31)|SONRADAN REVİZE: kullanıcı kararıyla bilinen maliyet olarak kabul edildi, kapı eşiği değiştirilmedi|
|Altılı/full41 CPU artmama|KALDI: altılı4R zx504→607ms/query (~+%20), full41+W zx+%7,6|SONRADAN REVİZE: kullanıcı kararıyla bilinen maliyet olarak kabul edildi, kapı eşiği değiştirilmedi|

Tek-sorgu the zx4R CPU +%30..49; p95 ise475→341ms iyileşti. Bu maliyetler
kapanmış/iyileşmiş sayılmadı. Eski SIDE-COST/ESTIMATOR tabloları değişmedi.
SQLite korunum>%100 olduğu için okuyucusuz kontrol tavan değildir; ops/s esas.
**2ms condvar ara-yolu uygulanmayacak.** N60 dört dengeli çift tamamlandı;
normal41 kapısı **KALDI**, eşik değişmedi:

|Yol, geçen sorgu/41|Çift0|Çift1|Çift2|Çift3|Karar|
|---|---:|---:|---:|---:|---|
|Yeni try/seri varsayılan|40|38|38|39|KALDI|
|REFERENCE_WAIT=true geri dönüş|40|40|40|41|KALDI|
|SQLite FTS5 trigram|41|41|41|41|GEÇTİ|

Yeni defaultta toplam9 pair/sorgu kaçırması: timmy(0,1),görenler(1),
akarsu(1),korkaklığa(2,3),yüklük(2,3),güzeli zafer(2). Örnek: timmy
pair1 idle22,438→loaded45,239ms>sınır30; güzeli zafer pair2
30,692→61,808ms>sınır61,383. Gürültü veya belirli bir motor nedenine atıf
yapılmadı. Default kullanıcı kararıyla korunur; bu tur sonraki işe geçilmedi.
253 default+161 diagnostic test,fmt/iki clippy,CRC/oracle,önce/sonra3'er
kill/reopen ve frozen eski okuyucu GEÇTİ. 1476 faz satırında60–88 örnek;
sayısal p95=max olan satır0; 15 normal child exit0. Corpus/0.1fixture/frozen ikili hashleri aynı;
reset/yeniden ingest/DB kopyası yok. Yeni override'sız default ve true geri
dönüş bekleme/iptal/aynı sonuç yolu test edildi.
Bu tur yazma çıktısı da saklandı: pair1 default throughput kontrolden düşük;
önceki SIDE-COST4/4 tüm-pozitif sonuç yeniden gerçekleşmiş sayılmadı.
CPU/RAM bu tur yeniden ölçülmedi; önceki bilinen bedellerin kabulü devam ediyor.
[Ölçüm raporu](HYBRID-DEFAULT-50K-20261102.md). Reclaim default kapalı;
geniş negatif/case34,signed-zero,yazma/bakım,disk/sorguRAM açık.100lü/B/C,
2,6sn,WAL handle/File::open,zx iyileştirmesi ve diğer onaysız işler başlamadı.

Aşağıdaki kayıtlar önceki turların tarihsel sonuç ve kararlarıdır.

31 Ekim yan maliyet bloğu **TAMAMLANDI**. Kullanıcı kararıyla yazma/CPU/RAM
birlikte ölçülüp raporlandı; sonraki çalışma başlamadı. Motor kaynakları ve
format/bütçe değişmedi. `REFERENCE_WAIT=true` varsayılan; prototip diagnostics
opt-in. Maske+capped delta korunuyor. **2 ms bekleme uygulanmayacak.**

|Ölçüm|Before → after / fark|Mevcut kapı|
|---|---|---|
|N60 yazma paired throughput Δ%|+15,444 / +4,157 / +2,156 / +0,875|Korunum≥%75 ve SQLite pooled üstü GEÇTİ|
|N60 pooled throughput ops/s|322,471 → 341,188; SQLite235,920|—|
|Commit pooled p95/p99/p99,9 ms|4,372/14,171/22,362 → 3,902/9,568/20,938|20/50/100ms GEÇTİ; max bilgi|
|Altılı4R zx normal p95 ms|1055,256 → 1386,805 (+%31,419)|Artmama KALDI|
|Full41+writer zx diagnostic CPU ms/query|559,648 → 602,093 (+%7,584)|Artmama KALDI|
|D ingest peak / temiz idle private MiB|14,63–16,61 / 2,25–2,29|60/15MiB GEÇTİ|
|Dört paired4R+W peak private ΔMiB|+1,527 / +2,145 / +4,176 / +3,332|Artmama KALDI|

Yazma ve normal41 okuma verileri ESTIMATOR N60 dört çiftinin hamlarıdır;
CPU ve RAM bu tur yeniden ölçüldü. Altılı mix 1R/4R60 örnek, dört çift;
full41 profilli1R/4R/loaded≥60 örnek, dört çift. the zx ayrı bilgi amaçlı.
Normal p95 ile diagnostic CPU ayrı; CPU caller+joined child thread toplamı,
profil overhead dahil. RAM motor prosesi; model/oracle/history/profil dışarıda.
Eski/new, her pair, min/medyan/max, SQLite oranları ve bütün örnekler korunur.
[Tek sayfa:41 sorgu + maliyetler](SIDE-COST-SUMMARY-50K-20261031.md) ·
[Tam tablolar, kapsam ve ham kanıt](SIDE-COST-50K-20261031.md).

251 default +159 diagnostic test,fmt/default+diagnostic clippy,oracle,
reset öncesi/sonrası3'er kill/reopen ve frozen eski okuyucu GEÇTİ.
40 CPU,32 altılı,48 RAM/oracle child normal exit0. Source/corpus/0.1 fixture
SHA aynı. Genel okuma A önceki N60 sonucuyla KALDI; kapı/eşik değişmedi.
Yeni optimizasyon veya varsayılan değişikliği yapılmadı. Reclaim default
kapalı; case34/geniş tarama, signed-zero, yazma/bakım/disk/sorguRAM açık.
4R+100lü,B(Frunner anahtarı),C(500 drain),2,6sn,WAL handle,format,256KiB,
ön filtre,Blok3/4,C ABI,gerçek-soğuk,uzun snapshot ve100k başlamadı.
Aşağıdakiler önceki turların tarihsel kayıtlarıdır.

31 Ekim kapanış: **örnek kalitesi GEÇTİ; motor A kapısı KALDI.** Aynı 50k/881756DA…
corpus, dört dengeli çift; 492 sorgu satırı / 1476 faz satırında 60–85 gerçek örnek.
P95 artık max rankı değil; sayısal p95=max olan satır 0. 1R/4R idle 60 örnek,
yüklü en az 60 aktif örnek; 16.384 commit alt sınırı gerektiğinde uzatıldı.
Motor kaynakları / frozen ikililer SHA eşit; varsayılan `REFERENCE_WAIT=true`,
tek try/seri yalnız diagnostics opt-in. Maske+capped delta korunuyor.

|Yeni tahminci, geçen sorgu /41|Çift0|Çift1|Çift2|Çift3|Karar|
|---|---:|---:|---:|---:|---|
|FDB kontrol|39|40|41|41|KALDI|
|FDB aday|41|41|40|40|KALDI|
|SQLite referansı|41|40|40|40|KALDI|

Adayda timmy 30,364 ms (82 örnek) ve isimli 34,047 ms (81) >30 ms; yakın fark
gürültü sayılmadı, eşik değişmedi. SQLite da yeni tahminciyle dört çiftte41/41
değil. N12 ham sonuçları aynen korundu; yeni/eskiler yan yana. Bu tablo tek
başına motor hatası veya gürültü nedeni kanıtı değildir; oturum/yerleşim ve
uzatılmış writer penceresi de farklıdır. [Tam p50/p95/max, örnekler ve kapılar](ESTIMATOR-50K-20261031.md).

251 test/fmt/clippy, her sorgu oracle, 3 kill/reopen ve eski okuyucu geçti;
15/15 normal child exit 0. Corpus/0.1 fixture ve kalıcı biçimler değişmedi.
**2 ms condvar tek bekleme uygulanmadı.** Yeni matristen sonra yalnız eski
profil sayaçları çevrimdışı incelendi: üreticisi/timmy/zx çekişmeli izinlerinin
%5–8'i 2 ms içinde alınmış; CPU/yazma kazanımı kanıtı güçlü değil. Tek tasarım
ve timeout/wall sınırı raporda; uygulama ayrı kullanıcı onayı gerektirir.

Yeni parent+child sorgu CPU / motor-only D RAM koşusu yapılmadı; önceki
CPU/loaded RAM artışları kapanmış sayılmadı. Reclaim default kapalı;
case34/geniş tarama yerleşimi,signed-zero,yazma/bakım/disk/sorguRAM açık.
4R+100lü,B(Frunner anahtarı),C(500 drain),2,6sn,WAL handle,format,256KiB,
ön filtre,Blok3/4,C ABI,gerçek-soğuk,uzun snapshot,100k başlamadı.
Bu ölçüm bloğunda duruldu. Aşağıdakiler önceki turların tarihsel kayıtlarıdır.

30 Ekim kapanış: **hibrit200 sonrası tek try/aynı caller seri prototipi KALDI;
varsayılan yapılmadı, bu adımda duruldu.** Kabul edilmiş merge maskesi ve capped
delta kazanımları korundu. A dört çiftte kontrol38/37/38/40, aday34/36/38/36;
41/41 yok. İzin beklemesi kalktı ve üreticisi/timmy hızlandı; fakat pooled yazma
331,78→297,29 işlem/sn (−%10,40), geniş4R zx895→994ms, profilli loaded zx
parent+child CPU472→523ms. D ingest14,52–15,44MiB/fresh idle2,25–2,30MiB geçti;
aynı proses4R+yazıcı peak private32,37→36,24 ve30,77→33,50MiB arttı.
251 default+159 diagnostic test/fmt/clippy, oracle/cancel/snapshot/CRC,
3'er kill/reopen ve eski okuyucu geçti. Normal child kapanışlarının tamamı0.
Yeni yol yalnız diagnostics scope ile opt-in; eski hibrit bekleme varsayılanı,
exact/serial, izin/bellek muhasebesi ve biçimler aynı. [Kapı tablosu ve bütün ham
kanıt](HYBRID-SERIAL-50K-20261030.md).

|Güncel kapı / açık iş|Durum|
|---|---|
|Kabul edilmiş maske + capped delta|KORUNDU|
|Tek try/seri prototip A|KALDI:34/36/38/36; yeni eşik yok|
|Yazma %75+SQLite korunum|GEÇTİ:%85,94 /SQLite%82,74; önce/sonra kötüleşmeme KALDI|
|Geniş tarama / CPU / loaded motor RAM kötüleşmeme|KALDI|
|D yalın ingest ve temiz idle|GEÇTİ; loaded RAM artışı ayrıca açık|
|Oracle / cancel / kill / eski okuyucu / tam test|GEÇTİ|
|4R+100lü, Frunner anahtarı,500 drain,2,6sn uç|BAŞLAMADI: A ve yan kapılarda duruldu|
|Yeni optimizasyon, WAL handle, format,256KiB,ön filtre|YAPILMADI|

Kısa açık işler: sorgu kabulü ve geniş tarama yerleşimi; **TR case34 kendisine
çıkacağını** izleniyor. **the zx** ayrı bilgi/CPU taraması, ana41 ve kapı dışında;
eski ishmael kapsam dışı. Reclaim varsayılan kapalı; disk/yazma-bakım/500 drain,
sorguRAM ve signed-zero owned/light tutarsızlığı açık. Blok3/4,C ABI,gerçek-soğuk,
uzun snapshot ve başka100k koşusu başlamadı. Başarısız adayın yerine ikinci bir
düzeltme uygulanmadı. Aşağıdaki kayıtlar önceki turların tarihsel sonuçlarıdır.

30 Ekim son durum: **Adım1/2 KABUL; A kapısı KALDI; D motor-only RAM GEÇTİ.** Dört no-reset çiftte FDB40/41,37/41,39/41,36/41. Idle üreticisi50,69–397,12ms(medyan227,18),timmy110,58–435,11ms(231,06); kaynağı yalnız gürültüye bağlanmadı. Gerçek profile p95 örneklerinde üreticisi326msnin290msi,timmy177msnin148msi ortak hibrit işçi izni Condvar beklemesi; term mutex≈0,003–0,007ms.1R/4R/few ve gerçek cache hit/miss/CPU/sequence tablosu var. Motor prosesinde aday ingest peak private15,18–15,39MiB,fresh idle2,30MiB(60/15kapıları geçti);runtime+tek giriş buffer dahil,model/oracle/history/profil/katalog dışarıda. RAM yük altıquery kapsamını kapatmıyor. Optimizasyon uygulanmadı;izin beklerken caller serial devam yalnız öneri.100lü/B/C/2,6sn/WAL handle başlamadı. Reclaim default kapalı;case34/geniş negatif yerleşim,signed-zero,yazma/bakım vequeryRAM/disk açık. [Kapı tablosu,kanıt ve tek öneri](READ-ADMISSION-RAM-50K-20261030.md).

|Bu tur|Karar|
|---|---|
|Adım1/2|KABUL|
|A4 dengeli çift|KALDI:40/37/39/36 sorgu geçti;41/41yok|
|Stage profiling,E|TAMAMLANDI;permit beklemesi ölçüldü|
|D yalın motor process ingest/idle|GEÇTİ:8/8kol|
|Düzeltme|UYGULANMADI;onay bekleyen tek öneri|
|100lü/B/C/2,6sn/WAL|BAŞLAMADI|

Aşağıdaki kayıtlar tarihsel sonuç/kararlardır.

Yeni kullanıcı kararı (30 Ekim): **Adım 2 KABUL**, Adım 1 kabulü korunuyor. A kapısı gevşetilmedi. Bu tur1) aynı slot/ikiliyle4 dengeli FDB/SQLite çiftinde idle yayılımı,2) gerçek request-local mutex/permit/cache/CPU aşama profili ve E karşılaştırması,3) yalnız kanıtlı öneri,4) bunlardan bağımsız motor-only RAM D. Optimizasyon uygulanmayacak. A41/41 olmadan100lü/B/C/2,6sn yok; WAL handle değişimi ayrıca sorulmadan yok. Çalışma sürüyor; aşağıdaki sonuçlar önceki oturumun kaydıdır.

30 Ekim güncel: **Adım1 kullanıcı kabulü korundu; capped delta uygulandı ve ayrı ölçüldü. A4R+tekli sorgu kapısı KALDI (39/41 geçti); duruldu.** Üreticisi idle79,98→loaded288,70ms (sınır159,97); timmy86,67→199,13ms (173,34). FDB throughput%91,40 /SQLite%71,10 ve commit4,90/16,02/23,26ms p95/p99/p99,9 GEÇTİ; max48,48ms,>100ms0.249 test/fmt/default+diagnostic clippy,oracle/kill/eski okuyucu geçti. Yazmasız maske sayıları gerileme/gürültü diye sınıflanmıyor: neden atfı kanıtlanmadı. Yeni yazmasız maske kapı koşusu yapılmadı; delta-only 1R/4R ayrı ölçüldü. Motor-only RAM D ölçülmedi; benchmark process RAM o kapı değildir.100lü/D/B/C/E/WAL düzeltmesi başlamadı. Biçim/256KiB/default exact/serial,full shadow/pin/CRC aynı; yeni100k yok. Reclaim default kapalı. Yazma/bakım, arama regresyonları/case34, disk/RAM/eşzamanlı yük,signed-zero,500 drain ve eski2,6sn uç açık. [Güncel kapılar ve kötü sonuçlar](CONCURRENT-DELTA-1-50K-20261030.md).

| Güncel kapı | Durum |
|---|---|
|Adım1 maske|KABUL, kullanıcı kararı|
|Delta oracle/CRC/signed-zero/cancel/pin|GEÇTİ|
|4R+tekli sorgu|KALDI:üreticisi,timmy;39/41 geçti|
|Tekli throughput/tail|GEÇTİ:%91,4;23,26ms p99,9|
|100lü, motor-only RAM D, B/C/E,WAL|BAŞLAMADI: A kapısında duruldu|

Aşağıdaki kayıtlar önceki kararların tarihsel kaydıdır.

29 Ekim güncel — **SONRADAN REVİZE:** kullanıcı eski sonuçları gördükten sonra sorgu/uzun-tekli kapılarını değiştirdi. Mevcut ham veride sorgu41/41 GEÇTİ; uzun tekli p99,9 kontrol43,68/yüklü44,30ms, >100ms3/4 adet (%0,00916/%0,01221). Eski KALDI tabloları aynen korunuyor. Throughput FDB%80,11 /SQLite%73,97; Dirty500291–292ms,4,32–4,44MiB geçti. 1–4 profil tamamlandı: hızlı case0/3 aktif delta taraması,case39/40 posting/cache beklemesi; büyük yeni commit uçlarında WAL handle açma çağrısı baskın. Eski74,84/125–200ms uçlarla aynı neden oldukları bilinmiyor. [Eski/yeni kapılar ve bütün ölçülen profiller](WRITE-REVISED-PROFILE-50K-20261029.md). **4R+100lü yeni arama kapısı KALDI:16/41.** Tamamlanan tek dengeli çiftte case0 74,36/case3 84,76ms; throughput FDB%100,17 /SQLite%72,18 geçti.100lü p95 35,49–37,72ms ama max2,60–2,67sn;4R process peak private162,41MiB(model/oracle/history dahil). Durma kuralıyla ek SQL kolu amaçlı sonlandırıldı; sonuçlara katılmadı ve slot aborted_requires_owner_reset işaretlendi.100lü ayrı profil/500lü drain ve Frunner anahtarı BAŞLAMADI.3 kill/reopen+eski okuyucu ve testler geçti. Raporda capped delta ilkN erken durma tasarımı yalnız öneri; uygulama yok. Optimizasyon yok; snapshot.mask düzeltmesi ayrı onay bekliyor. Aynı881756DA ilk50k,sabit slotlar; source/private/,kalıcı biçim,256KiB ve varsayılan exact/serial aynı. 100k/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot yok. Önceki sonuçlar aşağıda tarihseldir.

| Güncel, sonradan revize kapı | Durum |
|---|---|
| A: hızlı≤30ms; diğerleri≤2×idle | GEÇTİ:41/41 mevcut ham veri |
| B: uzun tekli p95/p99/p99,9≤20/50/100ms | GEÇTİ: kontrol43,68/yüklü44,30ms p99,9 |
| Dirty500 /throughput | GEÇTİ, aynı sonuçlar |
| İstenen1–4 profil | TAMAMLANDI; optimizasyon yok |
| 4R+100lü, yeni A kapısı | KALDI:16/41; case0 74,36/case3 84,76ms |
| 100lü ayrı profil/500 drain/Frunner anahtarı | BAŞLAMADI: yeni100lü kapısında duruldu |

Aşağıdaki tablo kapı revizyonundan önceki tarihsel karardır.

| Bu tur iş | Durum |
|---|---|
| Kısa tekli commit/görünürlük, yeni p99 kapısı | GEÇTİ |
| Dirty500 strict-open ve idle | GEÇTİ |
| 4R+tekli: her sorgu p95 kapısı | KALDI: 16/41 |
| 4R+tekli: throughput ≥%75 ve SQLite üstü | GEÇTİ: %80,11 / %73,97 |
| 4R+tekli: commit 20/50/100 ms | KALDI: max 199,93 ms |
| 4R+100lü / 100lü profil / Frunner anahtarı | BAŞLAMADI, önceki kapıda duruldu |

Öncelikli açık iş: sequence eşlemeli arama-içi maliyet profili ve uzun tekli uçlar. Admission süreleri ayrı ölçüldü; melez içi cache/permit beklemesi ayrıştırılmadı. 4R process peak private 112,64–114,88 MiB; motor+katalog/oracle/history dahil, saf motor payı ölçülmedi. Revizyon geri kazanma varsayılan kapalı. Ek gözlem sınırı: ilk recovery aracının tüm dosya envanteri assert’i oracle sonrası kaldı; başlangıç listesi saklanmadığı için dosya farkı belirlenemedi. Aynı DB’de aşama envanterli ayrı tekrarın canlı kaynak/hash, kill ve eski okuyucu kontrolleri geçti; ilk assert’in açıklaması kanıtlanmış sayılmadı.

29 Ekim sonuç: **posting sayfası + ilerleyen cursor uygulandı; melez N=50
arama kapısı 37/37 GEÇTİ.** [Tam37 sorgu p95/CPU/SQLite ve kötü sonuçlar](POSTING-PAGES-50K-20261029.md).
Aynı 50k DB, iki dengeli sıra,20 örnek/yol: yanıklarda 751,58→133,53ms,
külliyatı 569,47→105,04ms, ormanların 877,46→119,86ms,
çalışmalarıyla 768,32→78,26ms, kısım dişli 709,20→136,81ms,
kendisine çıkacağını 666,37→106,50ms; İnternetten hazinem 76,62,
oyuncudur atlanta 59,87, geldiği buldu 37,90ms. Exact/serial ayrıca ölçüldü.

Kök neden aynı 50k seri tanısında: yanıklarda556367 adet4B probe→250 sayfa
okuması;5013 doğrulanan aday aynı. Ara page-only denemesi103–110bin tekrar
okumada iki sorguda kaldı; ilerleyen konum bu yüzden eklendi. Bütçeler
256KiB legacy /8MiB serial first-page /512KiB worker aynı; optional alan
zorunlu cursor'lardan sonra ayrılır. CRC/range/sıralı-ID kontrolleri korundu.
Temiz idle private2,129MiB; sorgu process peak katalog/model/oracle dahil
yaklaşık34,63MiB, eski max34,84MiB. Kalıcı dosyalar değişmedi.

**Kalan CPU kapısı:** büyük melez, yeni serial capped'e göre+%6,06
(+0,09375ms/çağrı),+%5 sınırında KALDI. Diğer pozitifler+%1,81/+%3,95.
Gürültü/toleransla kapatılmadı. Negatif yeni mutlak/+%10 kuralı GEÇTİ;
tarihsel qzxkvwnever+0,2435ms artık GEÇTİ. Melez200/cancel opt-in,
varsayılan exact/serial; join/CRC reuse/reclaim default kapalı.

[Paralel test deadlock'u](PARALLEL-TEST-DEADLOCK-20261029.md) çözüldü: test global kaynak kilidini
tutarken başka DB fallback'i 14MiB bütçeyle o kilidi bekliyor, testin 6MiB
girişi bloklanıyordu. DB'ye özel worker pause ve dört iznin tamamını tutan
admission testiyle düzeltildi. Üretim pool/kilitleri artırılmadı;243 default
paralel test+149 diagnostic/fmt/clippy geçti. Oracle, Pending kill/reopen,
eski okuyucu 50k GET+41 sorgu ve 0.1 fixture koruması geçti.

100k **yeni kullanıcı-kırpılmış max200KB corpus BEKLİYOR**;256KiB entry
sınırı değişmedi. Yeni dosyada audit/oracle yenilenecek; private/ okunmayacak.
Aktif sabit slotlar aynı eski/onaylı corpus'un50k FDB+SQLite DB'leridir.
Açık işler: CPU+%5 kararı; yeni corpus 100k;50k yazma/bakım profili, tekli,
dirty500,4R+yazıcı,100'lük commit/RAM; uzun snapshot ve gerçek-soğuk cache;
case34/parçalı geniş tarama ve normal light/owned signed-zero farkı.
Format/ön filtre/Blok3/4/C ABI/dış entegrasyon yok. Önceki sonuçlar tarihseldir.

| Güncel kapı | Durum |
|---|---|
|37-case melez N50 arama|GEÇTİ 37/37|
|Negatif mutlak/+%10|GEÇTİ|
|Pozitif CPU+%5|KALDI: büyük +%6,06 (+0,094ms)|
|Idle /kontrollü sorgu bütçeleri|GEÇTİ|
|Oracle /CRC /kill /eski okuyucu|GEÇTİ|
|Paralel test deadlock'u|ÇÖZÜLDÜ; seri moda kaçılmadı|
|256KiB entry sınırı|AYNI|
|100k yeni max200KB corpus|BEKLİYOR|


28 Ekim güncel sonuç: **50k capped N=50 arama KALDI; 100k FDB ingest KALDI;
bağımlı aşamalar durdu.** [Türkçe corpus raporu](TRWIKI-SCALE-20261028.md).
50k iki dengeli kolda: ingest 38,44–39,13sn, disk 300,84MB, temiz idle private
2,23MiB altı, ingest peak 19,95MiB altı, strict-open p95 281,83 ms: RAM/açılış
kapıları geçti. Melez arama 37 kapılı sorgunun 9'unda kaldı (yanıklarda 582ms,
külliyatı 459ms). Exact ayrıca raporlandı; varsayılan exact/serial değişmedi.

100k ingest, tek encoded entry 256 KiB sınırında durdu: 66498 numaralı text 444345B.
İlk 66000 belge/sequence 132 tutarlı; reddedilen 500 anahtar görünmez. Corpus
kırpılmadı, owned fallback/limit/format değişimi yok. SQLite 100k iki bağımsız
ingest/disk/RAM/41-case tekrarı tamam; tam 100k dengeli A/B ve FDB açılış/arama
ölçümü yok. 444KB cancel ölçülemedi. [Aktif slot kimliği](benchmarks/active-benchmark-slots.json) artık FDB 66000
commit + SQLite 100k; eski kitap-DB slot pointer'ları güncel değildir.

Tanı: 66000 commit'te yanıklarda 380033 adet 4B posting probe'u, seri tanı
süresinin %70,58'i; metin doğrulama %20,32, payload CRC %0,66. Sonraki aday
biçim değiştirmeden posting okuma isteğini azaltmak; henüz uygulanmadı.
Oracle, eski okuyucu ve Pending kill/reopen geçti. İlk paralel test paketi
takıldı/durduruldu; sebebi açık. Seri tekrar ve fmt/clippy geçti; ayrıntı raporda.

Açık sıra: bu iki engel için kullanıcı kararı → 50k tekli/dirty500/4R+yazıcı/
100'lük commit-RAM kök nedeni → uzun snapshot/maks ömür → gerçek-soğuk cache.
Case34/parçalı geniş tarama, sorgu RAM'i, signed-zero normal light/owned farkı
ve paralel test takılması açık. Melez200+cancel opt-in; join/CRC reuse ve
reclaim default kapalı. Ön filtre/format değişimi, Blok3/4, C ABI ve dış proje
entegrasyonu yok. Testten kalan 6729B Temp temizliği otomatik denetimce reddedildi.
Aşağıdaki eski karar ve sonuçlar tarihsel kayıttır.

| Güncel kapı | Durum |
|---|---|
|50k temiz idle≤15MiB / ingest peak≤60MiB / strict≤1000ms|GEÇTİ|
|50k capped N50: ortak≤30ms, diğer≤150ms|KALDI: melez9/37|
|Bol-pozitif melez CPU≤serial+%5|GEÇTİ: en kötü+%3,46|
|Negatif melez p95≤4 işçi+%10|KALDI:0,369→0,612ms (+0,244ms)|
|100k FDB ingest|KALDI: tek kayıt 256 KiB|
|100k FDB open/query /444KB cancel|BLOKE, ölçülmedi|
|SQLite 100k bağımsız iki tekrar|TAMAMLANDI|
|Oracle / eski okuyucu / Pending kill-reopen|GEÇTİ: ölçülen kapsamda|
|fmt / clippy / tek-thread test tekrarları|GEÇTİ; ilk paralel hang açık|



28 Ekim son kullanıcı kararı ve ölçüm: [melez/cancel raporu](HYBRID-CANCEL-50K-20261028.md).
**200 adaylı melez REDDEDİLDİ / kapı KALDI; burada duruldu.** Aynı eski50k,
iki dengeli FDB/SQLite sırası; negatif p95 serial233,912 /4 işçi84,412 /
melez89,896ms:4'e göre+5,484ms/+%6,50. CPU1000-query blokları harpoon melez
+%1,93, presumption−%1,74, darcy eşit; yeni tolerans eklenmedi. Positiflerde
93/160/92 aday ve worker0; negatifte7563 toplam, serial prefix tekrar okunmuyor.
Holmes elementary melez64,895ms/serial36,478ms; kötü sonuç korunur.

Query cancel240/240 çalışan sorguda Cancelled; pooled p950,4108ms,
max1,4362ms (sıcak2–50KiB kitap fixture). Seri/4/melez/exact4,32/256
candidate-start witness. Gerçek-soğuk I/O,200–444KB yeni TR metinleri ve
Pending background açılış cancel latency'si ölçülmedi. Senkron I/O zorla
kesilmez. Melez/cancel API opt-in; normal search exact/serial ve search_first
serial/opt-in, join/CRC reuse default kapalı. Aktif delta signed-zero exact
uyumsuzluğu yeni iptal yolunda düzeltildi; son239 test/diagnostic145/fmt/clippy/
compat/kill geçti. Mevcut normal light/owned exact signed-zero eşitliği farklı;
semantiği birleştirmek ayrı açık iş, bu tur normal davranış değişmedi.

**Yeni corpus kullanıcıdan geldi:** D:/frunner-corpus/trwiki-100k/.
100k kayıt/100k unique body, CRC/SHA/anahtar-link kimliği doğrulandı.
[Türkçe41-case offline3/4-gram aday/gerçek analizi](TRWIKI-OFFLINE-20261028.md):
100k çalışmalarıyla10755/505 (21,30×),4-gram4155/505; uygulama yok.
Kaynak yalnız okundu, private/ okunmadı, gövdeler kırpılmadı/çoğaltılmadı.
Veri Türkçe wiki, kod/PDF/DOCX karışımı sayılmaz. Eski havuz da korunur.
**Yeni corpus'ta DB ingest/query/open henüz başlamadı:** melez kapısı
geçmediğinden slotlar resetlenmedi. Sonraki aşamaya geçmek için kullanıcı kararı.

Açılış yeni hedef50k ilk arama≤1000ms,100k≤2000ms. Eski300ms kapısı
kullanıcıca değiştirildi; önceki306–757ms aynı kitap fixture'ına ait tarihsel
sonuç, yeni corpus'ta GEÇTİ sayılmaz. **Kaynak başına lazy yapılmayacak;
doğrulama atlanmayacak.** Ön filtre/bitset/position uygulaması yapılmayacak;
yalnız offline oranlar. Key sırası kabul; {:016x}|yol ve mtime değişiminde
atomik delete+insert;1007 bayt sonrası uzun yol stratejisi Frunner kararı.

Açık sıra: bu kapının kullanıcı kararı → yeni corpus50k/100k temel ölçümler ve
yeni açılış sınırları → kalan tekli/dirty500/4R/100'lük commit-RAM profili →
uzun snapshot/maks ömür ve gerçek-soğuk. Yazma/bakım/diskte önceki kabul edilen
10k kazanımları korunur; yeni corpus'a otomatik taşınmış sayılmaz. Case34,
geniş/parçalı okumalar, sorgu RAM'i ve4R throughput izlemeleri açık. Reclaim
default kapalı. Biçim değişikliği, Blok3/4,C ABI ve dış proje entegrasyonu yok.
Aşağıdaki eski kararlar/takvim tarihsel kayıttır; son karar bu giriştedir.


28 Ekim son durum: **capped N=50 ve exact deneyleri uygulandı; hızlı açılış
prototipi KALDI, duruldu.** [Arama raporu](CAPPED-EXACT-50K-20261028.md): iki dengeli
FDB/SQLite çifti, aynı50k/model. CRC korunan exact4 p95 max141,766ms; capped4
max99,831ms:150ms kapısı GEÇTİ. Serial capped negatif288,635ms: KALDI.
4 işçi CPU tüketimini yükseltiyor; pozitiflerde serial capped daha az aday okur.
Join ve CRC reuse tek başına kapıyı geçmedi, default yapılmadı.

Yeni opt-in Rust API `search_first[_with_execution]`, dürüst `Exact/AtLeast`
total ve snapshot desteği. Normal exact/strict, WAL/manifest/payload/SEG05
biçimleri korunur. [Anahtar sırası/ters-mtime notu](KEY-ORDER-FIRST-N-20261028.md).
[Ön filtre maliyet/aday analizi](PREFILTER-OPTIONS-50K-20261028.md): yalnız offline
analiz, **format değişikliği uygulanmadı; uygulama kullanıcı onayı bekliyor**.
4-gram adayları transubstantiation7563→0, holmes elementary4620→530; yaklaşık
+599MB ham posting tahmini. Trigram bitset mevcut posting presence bilgisini
tekrarlar; ek yanlış-pozitif eleme sağlamaz.

[Hızlı oturum tasarımı](CAPPED-SEARCH-FAST-OPEN-DESIGN-50K-20261028.md) onaylı;
yeni kapı oturum dönüşü değil **ilk kullanılabilir arama≤300ms**. Yeni
`OpeningDatabase` prototipi kaynak/SEG05 dizin/indeks CRC'yi birlikte en fazla4
işçiye taşır; ilk sonuçtan önce bütün strict kontrolü bekler. Pending yazı API'si
yok; cache yok/bozuksa ERR_REBUILD, kaynak ve cache sessizce onarılmaz.
İlk cevap **306,268–756,744ms**; her sorguda4 örneğin p95'i300ms üstü:
**KALDI ve duruldu**, sonraki50k işlerine geçilmedi. Oturum17–23ms
Pending döner; bütün strict kontrolü bitmeden sonuç dönmez.
[İlk cevap/tam kontrol/CPU/bellek ve kötü örnekler](BACKGROUND-OPEN-50K-20261028.md).
Default237 test, diagnostic143 lib, fmt/clippy/no-default/all-features ve native
kontrolleri geçti.50k yeni API38-case, Pending kill/reopen ve donmuş eski
okuyucu50k/100 deleted/38 original query geçti.0.1/corpus/source/model SHA aynı.
Kaynak başına gerçek lazy index/öncelikli kontrol henüz uygulanmadı; başarısız
kapı nedeniyle yeni düzeltme için kullanıcı kararı bekleniyor. Varsayılan yapılmadı.

50k tekli commit → dirty500 →4R+yazıcı →100'lük commit p95/RAM profili →
2c uzun snapshot/maks ömür →2b gerçek-soğuk cache sırası açık.100k gerçek
dosya corpus'unu kullanıcı sağlayacak; kitap havuzuyla100k koşusu yok.
Reclaim default kapalı. Case34, zx/the zx/parçalı1112→2013 read, sorgu RAM'i,
4R throughput≥%80 ve quixote whale/the5000 izlemeleri açık. Önceki Blok1
yazma/bakım/disk kapıları kabul edildi;50k'ya otomatik taşınmış sayılmaz.
Blok3/4, C ABI ve dış proje entegrasyonu başlamaz.

Aşağıdaki eski5× kapısı ve sonuçlar tarihsel kanıttır; yeni mutlak kapı
yaygın≤30ms, orta/nadir50k≤150ms / gerçek100k≤300ms. SQLite güncel aynı-model
[slot pointer](benchmarks/block2-query-open-50k-20261028-v1-slot-sqlite.json).

28 Ekim: **Blok1 kabul edildi. Blok2, ilk 50k kontrol kapısında KALDI ve durdu.**
[Güncel kapı tablosu, bütün 38 sorgu ve ham kanıt](BLOCK2-SCALE-50K-20261028.md).
İki dengeli FDB/SQLite tekrarı, aynı mevcut ilk 10k/50k ve sabit slotlar:

- 50k FDB temiz idle private max **4,203 MiB**, ingest tepe **20,246 MiB**;
  ortak the/man/he ve sayfalarında pooled p95 max **7,392 ms**: GEÇTİ.
- Ingest+bakım **65,365–76,628 s**, SQLite **163,807–206,204 s**;
  disk **1754,921 MB / 2906,251 MB**. Temiz sıcak strict-open FDB **813–815 ms**;
  SQLite açılışı aynı kaynak doğrulama sözleşmesine sahip değil.
- **15/38 pooled p95**, 10k→50k için 5× sınırını aştı: KALDI.
  Beagle **16,589→109,066 ms (6,575×)**; harpoon **53,917→352,960 ms (6,546×)**.
  Corpus frekansı ve case37'de boş sayfanın 50 hit'e dönüşmesi bazı artışları
  açıklar; tüm aşımın kök nedeni henüz aşama profiliyle ölçülmedi.
- Tam payload/version ve sorgu oracle'ları, dört gözlenmiş 50k kill/reopen,
  eski 24 Ekim okuyucuyla 50k get/38-case, **233 test / fmt / clippy** geçti.
  İlk merge fixture'ı merge başlatamadı; kapsam sayılmadı, retry ve ayarı
  raporda. Python cp1252 metadata okuması iki Unicode sorguyu bozdu; açık UTF-8
  düzeltmesi ve tüm sayısal sonuçları aynı kalan v2 audit raporda korunur.
  Kill sonrası tek idle çağrısı/kısa pin ve eski okuyucu yeniden geçti;
  unreferenced kaynak ve scratch artık 0 byte. Bu, 10dk/4R ölçümü değildir.
- **100k, gerçek-soğuk, 10dk snapshot/maks ömür API'si başlamadı.** 50k tekli
  commit, dirty500, snapshot+yazıcı ve 100'lü commit/bellek profili yapılmadı;
  durma kapısının ardından yeni kullanıcı kararı bekleniyor. Sonraki dar öneri:
  10k/50k posting, metadata/cache, payload/CRC ve text doğrulama aşamalarını
  ayırıp artışın nedenini bulmak; henüz motor optimizasyonu yapılmadı.

Motorun 64 Rust kaynak SHA'sı kabul edilmiş Blok1 ile aynı; biçim, varsayılanlar,
corpus ve 0.1 fixture değişmedi. Reclaim varsayılan KAPALI; Blok3/4 ve C ABI
başlamayacak. Havuzda mevcut 100000 farklı gerçek kitap pasajı doğrulandı;
8 kitaptan örtüşen pencereler, 100k bağımsız dosya iddiası yok. Yeni corpus yok.
[Havuz kanıtı](benchmarks/block2-20261028-corpus-audit.json).
Case34 izleniyor; geniş zx/the zx, parçalı okuma1112→2013, önceki 100'lü4R
arama/RAM, yazma/bakımın kalan sınırları ve soğuk/uzun pin işleri açık kalır.
İlk N kullanımı yalnız ölçek bloğunun kullanıcı istisnası; diğer testlerde ilk10k.

27 Ekim son tur tamamlandı: [tek sayfa güncel kapılar](WRITE-HARDENING-GATES-10K-20261027.md).
Yeni≥%75/SQLite ölçütünde tekli önceki%77,22 ve yeni100'lü%76,59 (%61,23SQLite)
GEÇTİ.1c/1d iki30turda son1,107×/max1,199× disk; son600 tombstone, artık0byte,
oracle/kill/eski okuyucu geçti. Üç trace+üç trace-kapalı4096'lık4R koşuda0>50ms;
kapalı max29,022ms.419ms yeniden üretilmedi: nedeni kanıtlandı/düzeltildi denmez,
koşullu worker-handle değişikliği yapılmadı; sequence izleyicisi varsayılan kapalı.
Her adımın ayrı7kill'i (21toplam),son233 test/fmt/clippy geçti. Format/corpus/0.1
değişmedi;Blok2/3/4,C ABI için yeni onay beklenir.
**Açık kötü sonuçlar:**100'lü4R'de20/38 sorgu eski2× sınırının üzerinde;proses
peak private77,94MiB (harness/oracle dahil);100'lücommit p95≈1004ms,max1110ms.
Case34,zx/the zx,sorgu RAM'i,soğuk yük ve uzun pin/geçici disk açık. Bu aşım ve
bellek bileşenleri profillenmedi; throughput geçti diye kapatılmadı.
Raporlar:[30tur](RECLAIM-IDLE30-10K-20261027.md),[sequence](WAL-SEQUENCE-PROFILE-10K-20261027.md),
[100'lük](HUNDRED-PAIR-10K-20261027.md). Aşağıdaki devam/beklenti satırları tarihsel durumdur.

27 Ekim yeni30tur sonucu: [1c/1d GEÇTİ, gözlenen kapsam](RECLAIM-IDLE30-10K-20261027.md).
İki FDB koşusunda bakım sonrası max1,199×/son1,107×; metadata tepe1201,
son600 tombstone; ölü payload son29,75MB. Yeni7kill/eski okuyucu/pin geçti,
artık0byte. Motor src SHA önceki nihai kodla aynı. Bakım72,5–74,1s/30tur ve
geçici disk örnekli1,45–1,47× maliyeti saklanmadı;30tur sonsuz garanti değildir.
Şimdi sequence profili;100'lük tek çift nihai motor üzerinde. Blok2/3/4,C ABI kapalı.

2026-10-27 son kullanıcı kararı: [güncel tek sayfa kapılar](WRITE-HARDENING-GATES-10K-20261027.md).
1b tekli sınırı **≥%75 ve SQLite pooled üstü** oldu: mevcut%77,22/%70,38
bu ölçüte göre GEÇTİ.100'lük grup tek çift ölçülecek. Öncelik gerçek10k/30tur
1c/1d: her tur2000 kesintisiz işlem, bir maintain_idle(), sonra2sn idle.
Motoru değiştirmeden yeni ölçüm başladı; kötü sonuçta düzeltme/sonraki adıma
girilmez. Sonra419ms için sequence eşleştirmeli profil ve gerekirse worker'da
reader handle hazırlığı; üç uzun koşu0 adet>50ms şart. Blok2/3/4,C ABI yeni
onaya kadar kapalı. Aşağıdaki%80 KALDI sonuçları tarihsel ölçüm olarak korunur.

2026-10-27 **Blok1 KALDI; kullanıcının durma kuralı uygulandı.**
[Sonuç ve bütün kötü örnekler](WRITE-HARDENING-10K-20261027.md),
[arka plan WAL kesiti tasarımı](WRITE-HARDENING-DESIGN-20261027.md).
İlk diagnostic768 koşusunda513. commit446,804ms;268,484ms indeks hazırlığı,
113,027ms source/index hazırlığı, worker drain0,0054ms. Bunlar diagnostic
build süreleridir; önceki379ms ile doğrudan A/B sayılmaz.
Arka plan prototipinin dört normal768 koşusu:0/3072 aşım,max26,366ms;
normal p95 bazı çiftlerde yükseldi. İlk20,555ms tek keşif koşusu ayrı korunur.
**1b tekli throughput KALDI:**%77,85/%76,94/%76,86; gözlenen3 çift pooled%77,22,
0 geçen.İlk iki başarısızlıktan sonra≥3/4 imkânsız olduğundan kalan koşular
durduruldu;100'lük throughput ve gerçek10k/30tur bu turda ölçülmedi. SQLite
iki tekli referans pooled%70,38; dördüncü çift sonucu uydurulmadı.
Ek4096'lık4R yükte915. commit419,587ms: genel uç gecikme sorunu kapanmadı.
Bu koşudaki max snapshot hazırlığı408,446ms; toplam WAL reader hazırlığı
151,845→587,413ms, fakat tek commit ile sayaç sequence eşleştirmesi yok.
Sonraki dar öneri: yeni WAL reader handle'ını worker'da hazırlamak ve
bakım aralığı kontrol≈0,3s/yük≈1,1s farkını alt aşamalara ayırmak.
4R pooled aramada case6/22 önceki2× sınırını aştı; her çiftin diğer kötü
sonuçları ham tabloda. Writer-owned max read wait7,361ms; private peak
58,23–60,50MiB (harness/oracle dahil), genel60MB hedefi geçmiş sayılmaz.
Tek idle çağrısı ve strict-open scratch temizliği eklendi; küçük30tur
tombstone testi gerçek10k sürekli yazı ölçümünün yerine geçmez. Son incelemede
scratch temizliği recovery flush/worker başlangıcından önceye taşındı;
kirli-open regresyon testi eklendi. Son kaynak232 test/fmt/clippy geçti;
nihai kill/reopen/eski okuyucu ve tek-idle cleanup kanıtı rapordadır.
Küçük WAL yolu ve revizyon reclaim varsayılan KAPALI; biçim ve0.1 fixture
değişmedi. Sonraki blok/C ABI/entegrasyona geçilmedi. case34, geniş zx/the zx,
parçalı tarama read sayısı, sorgu RAM'i ve gerçek-soğuk yük açık kalır.

2026-10-26 **Adım3: kısmi hedefler tutuldu; genel kapı KALDI.**
[Rapor/ham tablolar](SMALL-COMMIT-DIRTY-OPEN-10K-20261026.md),
[tasarım ve atomik sınırlar](SMALL-COMMIT-DESIGN-20261026.md).
Son kodda bütçe-miss sonrası sparse maskenin shadow coverage'ını yanlış
bildirmesi de segment bazında düzeltildi; v7 regression/oracle/kill geçti.
Yeni küçük WAL yazma yolu **varsayılan KAPALI**, açık
`set_streamed_wal_layer(true)` çağrısıyla denenebilir. Artımlı snapshot maskesi,
WAL read-handle yeniden kullanımı ve adresli düşük RAM recovery korunur.
Reclaim ve önceki query pruning deneyi de varsayılan kapalı.

- Opt-in normal tekli commit p95 update/delete/insert1,810/1,149/1,950ms;
  görünürlük p95≤2,089ms.226 test,7 normal kill+3 küçük-WAL sınırı ve eski okuyucu
  geçti. Kalıcı format/entegrasyon/C ABI değişmedi.
- **Asıl engel:**768'lik koşuda513. işlem eşik flush'ı nedeniyle379,050ms
  görünürlük;1/768 adet50ms üstü. P95 2,712ms; kötü örnek atılmadı. Sonraki dar
  hedef bounded aktif katmanı boşaltırken foreground source/index/manifest
  hazırlığını asenkron tasarlamak; tek seferlik maliyeti saklamak değil azaltmak.
- Dirty500 strict open294,576/285,147ms; idle private5,91/6,19MiB. Temiz idle
  private3,52–3,57MiB; normal ingest peak≤12,85MiB, helper16,39MiB,4R≤51,22MiB.
  Bunlar sıcak OS cache/n2 open ve ölçülen proseslerdir;300ms marjı küçük,
  gerçek-soğuk ve büyük/owned dirty frame tepesi için garanti yok.
- Son4R+writer38/38 her grup ve her çiftte2×/kısa sorgu+2ms içinde.
  quixote whale/the5000 izleme tekrarda tutuyor; kalıcı arama işi kapanmadı.
  **≥%80 throughput açık:** pooled tekli%76.65,100'lü%74.90.
  Boşta zx/the zx, parçalanma1.112→2.013read ve case34 izleniyor.
- Atomik sınırlar belgelendi; non-atomic `commit_streamed_chunks` ack sequence/
  chunk_sizes/partial failure döndürür. Aynı5000 işlem atomik reddedildi;
  helper263 parçayla8,078s/16,39MiB'de oracle geçti.
- Sürekli100'lük/fallback writer revision işini erteleyebilir. Üç100'lük grup
  sonrasında1sn idle+explicit bakım1,840s: ölü payload118,341→0,968MB.
  Yalnız1/3 çağrı başında worker hâlâ aktifti;3 gerçek iptal iddiası yok.
  Otomatik idle timer/sınırsız sürekli yazıda disk üst sınırı kanıtlanmadı.

Adım2 kullanıcı kabulü ve o adıma özel kapı kapanışı korunur. Önceki10/30tur
disk/bakım kabulü geri alınmadı, bu tur30tur yeniden ölçülmüş sayılmaz. Genel
query RAM, uzun snapshot pinleri/disk/geçici tepe ve gerçek-soğuk yük açık.
C ABI ve Frunner/SDB/Fundops entegrasyonu yapılmadı.


2026-10-26 kullanıcı kararı: **Adım2 kabul edildi; o adımın kapısı kapatıldı.**
Throughput%76–79 ve iki sorgu gerilemesi aşağıdaki ölçümlerde korunur;
açık izleme: (a)4R+writer `quixote whale` ve `the /5000/50`, (b)throughput≥%80,
(c)boşta `zx/the zx`, önceki parçalanma ve case34. Reclaim varsayılan kapalı.
**Adım3 başladı:** önce artımlı yayın maskesi; sonra düşük RAM'li küçük streamed
commit(p95≤10ms/görünürlük≤50ms), dirty500 open≤300ms/idle≤15MB, açık atomik
sınırlar ve non-atomic parçalı helper, sürekli writer sonrası boşta bakımın
yetişmesi. Oracle/kill/eski okuyucu zorunlu; C ABI kapsam dışı.


2026-10-26 **Adım2 snapshot prototipi: KALDI; Adım3 başlatılmadı.**
[Rapor/kapılar](SNAPSHOT-CONCURRENT-10K-20261026.md).
Hazır immutable görünüm kısa pointer kilidiyle ediniliyor; segment/aktif WAL
pinleri, son-reader GC, rewrite cancellation ve atomik epoch oracle çalışıyor.
Güncel kod9 snapshot testi dahil216 test geçti;7/7 yeni process-kill/reopen
ve24Ekim eski okuyucu geçti. Kalıcı biçim/protokol/entegrasyon değişmedi.

Dengeli v2, grup başına iki çift: writer throughput pooled tekli%75,96,
100'lü%78,68 (<%80); yalnız1/2 çift/grup sınır üstünde. Aynı4-reader boşta
tabanda38/38 geçti,1-reader tabanda9/38 ve10/38 kaldı. Son-release sayaç
yarışı `Arc::into_inner` ile giderildi; v3 ek yükte admission0/1520 bekleme,
writer-owned gözlenen tek bekleme max1,325ms, release max14,893ms.
v3 ek100'lüde case22 `quixote whale`7,678→15,691ms ve case32
`the /5000/50`3,628→8,023ms 2× kapısını aştı; **arama kapısı da açık**.
v3 ek koşu writer-only/dengeli throughput tekrarı değildir; v2 başarısızlığı
silinmedi veya v3 kabulü sayılmadı. Sonraki dar iş bakım/worker drain bedeli ve
yayın başına görünürlük hazırlığını ayrıştırıp artımlı hazırlama imkânını ölçmek.

Revizyon geri kazanımı **varsayılan KAPALI**. Önceki görünürlük adımının ±%5
kapısı yalnız o adım için kullanıcı kararıyla kapalı; geniş `the zx`
yerleşim/parçalanma1.112→2.013 read ve case34 **izleniyor**. Genel RAM/temiz
boşta15MiB, küçük commit/dirty500 açılış, query RAM, uzun snapshot pinlerinin
disk/RAM etkisi, geçici disk ve gerçek-soğuk yük açık. Önceki10/30tur disk ve
bakım kabulü korunur; bu tur bu ölçümler yeniden yapılmış sayılmaz. Sürekli
writer'ın revision işlerini iptal edip boşta bakıma ertelemesi Adım3 raporuna
bilinen sınır olarak taşınacak. C ABI bu işler bitmeden yapılmayacak.

2026-10-26 kullanıcı kararı: görünürlük raporu kabul; ±%5 pooled p95 kapısı
**bu adım için kapatıldı**, ek pruning reddi korunur. Revizyon geri kazanımı
varsayılan kapalı. Açık arama işleri: (a) geniş `the zx` taramalarında parçalı
yerleşim1.112→2.013 read, (b) case34 izleniyor.
Adım2 başlangıcındaki karar: ≤100ms mutlak kapı kaldırıldı. Yeni kapı her sorguda
loaded p95≤2×idle, idle<1ms ise mutlak artış≤2ms; 4 reader+1writer ve writer
throughput≥%80. Admission/release beklemeleri, model oracle, tam merge/rewrite,
pin/son-reader GC ve rewrite cancellation testleri zorunlu.
[Ön tasarım](SNAPSHOT-DESIGN-20261026.md). Sürekli writer'ın revision işlerini
iptal edip bakımı boşta zamana ertelemesi Adım3 bilinen sınırı olarak kalır.

2026-10-26 ayrı arama kapısı **KALDI: 6/37 pooled p95 +%5 sınırının üstünde**.
Maske zaten seed döngüsünde ölü ID'yi payload doğrulamasından önce eliyordu;
`the zx`: aday11.951 ID→1.951 hidden skip→10.000 canlı doğrulama;
referans10.183→183→aynı10.000. İstenen268,304MB aynı, ama aday2.013 read /
7.987 window hit; referans1.112 /8.888: yerleşim okuma pencerelerini parçalıyor.
Dar deney (boş segmenti önce atla, secondary küçük posting cache'ini maskeyle
temizle) canlı payload sayısını değiştirmedi; varsayılan kapalı kalır.
**Ek pruning deneyi reddedildi.** Aynı oturumda eski aday da referansa göre
6/37 sınır üstündeydi; yeni deney kendi eski adayına göre14/37 durumda
pooled p95+%5 üstünde çıktı. Başarı veya kararlı hızlanma diye kapatılmadı.
`zx`157,564→173,816ms (+16,252ms); diğer kalanlar zeuglodon+0,167ms,
mesopotamia+0,227ms, invertebrata+2,079ms, quixote whale+0,287ms ve uzun
negatif zx+0,0293ms. Küçük farklara kapı istisnası tanınmadı. Önceki24→6
değişimi tek başına deney kazancı sayılmadı; kontrol zamanları da değişti.
Ek10tur bakım13,111s, retained420,644MB (1,2004×); politika/eşik/kalıcı biçim
aynı. Case34 izleniyor. Arama kapısı geçmeden snapshot Adım2 başlatılmadı.
Snapshot'ın100ms mutlak maksimum kapısı için boşta başlangıç engeli de var:
bu matriste zx en kötü197,378ms. Kilit beklemesini kaldırmak bu tarama
maliyetini tek başına çözmez; 4 okuyucu+1 yazıcı koşusuyla geçilmiş sayılmadı.
[Yeni profil, mutlak farklar ve kanıtlar](REVISION-VISIBILITY-SEARCH-10K-20261026.md).

2026-10-26 kullanıcı kararı: bakım maliyeti adımı **kabul edildi** (12s hedefi
13,5–13,95s ile ölçümde tutulmadı; kabul bu farkı açıkça kapsıyor).
Revizyon geri kazanımı varsayılan **kapalı** kalır. Snapshot öncesi ayrı kapı:
case34 dışında 37 sorguda referansa göre pooled p95 +%5 sınırı; küçük sürelerde
mutlak ms farkı da raporlanacak. Önce posting/görünürlük/payload profili,
sonra görünürlük maskesiyle erken elemenin ölçülebilen ek imkânı araştırılır.
Kod zaten seed ID'sini payload doğrulamasından önce maskede eliyor; daha geç
eleme varmış gibi varsayılmayacak. Case34 izlenen maddede kalır.
Adım3 raporuna bilinen sınır: sürekli yazma, revision worker'larını iptal edip
bakımı boşta zamana erteleyebilir; bu koşulda kalıcı disk üst sınırı kanıtlanmadı.
Arama kapısından sonra Adım2 snapshot/4 okuyucu+1 yazıcı ölçülecek. Genel RAM,
küçük commit/dirty open ve gerçek-soğuk yük işleri açık; C ABI henüz kapsam dışı.

<!-- revision-reclaim-cost-20261025 -->
2026-10-26 maliyet kapısı **KALDI; varsayılan kapalı, Adım2/3 başlatılmadı**.
İki dengeli normal koşuda eski reclaim bakım toplamı35,351/36,679s;
aday13,950/13,500s:12s hedefi tutulmadı. Disk10. turda420,644MB (1,2004×),
önceki daha agresif reclaim357,954MB idi. Kaynak kopyası sırasında64 tek belge
commit örneğinde en kötü82,594ms; bu bütün CRC/posting/fsync/install evrelerine
varış üst sınırı değildir. Eşlenmiş sıcak yük okumalarında38/38 p95≤2×idle,
en kötü oran1,533×. Normal38 sorgunun24'ünde kontrol+%5 aşıldı; bu arama işi
kapanmaz. Case34 bu kez+%2,83; kullanıcı kararıyla **izlenen** kalır.
[Aşama profili, ham örnekler ve bütün kapılar](REVISION-RECLAIM-COST-10K-20261026.md).
Son diagnostic profilde pacing0; posting remap/akış yazımı4,127s (%32,78),
indeks yazma/açma2,051s (%16,29) en büyük kalan işler. Bakım pencerelerinde
29 posting stream /82,638 milyon giriş ID'si var; bu salt map lookup süresi
değildir. Sonraki öneri arama kapısını koruyarak bu işi azaltmak; eşik gevşetme
tek başına kabul edilmedi. 30 tur400,074MB (1,1417×), en yüksek bakım sonrası
1,2036×; son10 tur399,542–419,329MB aralığında. Süresiz plato garantisi yok.
90 normal/SQLite/uzun tur oracle'ı, yedi gözlenen kill/reopen ve eski okuyucu
geçti. İlk retirement-after hazırlığı hedefi normal merge'e kaptırdı; bu deneme
geçti sayılmadı, ayrı retry loguyla kapsandı. Kalıcı codec/0.1 fixture aynı.
Maliyet düşüşü daha az rewrite ve idle pacing kaldırılmasıyla geldi; tam ölü
tombstone0 kaynaklar çıktı yazmadan emekli edilir, writer yayımlanmamış işi
iptal ettirir. Sürekli writer altında bakım ilerlemesi/disk ve kaynak kopyası
dışındaki evrelerde en kötü yazma gecikmesi ayrıca açık. Yeni kalıcı biçim yok.

2026-10-25 kullanıcı kararı: önceki Adım1'in disk/30tur/oracle/kill kazanımları
**kabul edildi**. Case34 (ishmael offset130/limit1, önceki pooled p95+%9,77)
**izlenen** madde; tek başına snapshot'a geçiş engeli sayılmıyor.
Adım2'den önce yeni bağımsız kapı: 10tur bakım≤12s, disk≤1,3×;
rewrite altında tek commit p95≤100ms/max≤250ms, okuma p95≤2×idle,
38-case kontrol±%5, oracle/kill/eski okuyucu uyumluluğu.
Önce aşama profili, sonra ölçüme göre iyileştirme; politika varsayılan kapalı.
Snapshot/küçük commit ve dirty open/RAM, çok okuyucu, soğuk yük ve uzun vadeli
tombstone/geçici dosya temizliği açık kalıyor. C ABI uygulanmayacak.

<!-- revision-reclaim-20261025 -->
2026-10-25 sıralı kapı çalışması [revizyon raporunda](REVISION-RECLAIM-10K-20261025.md).
**Tarihsel ilk rapor kararı: Adım 1 KALDI; Adım 2 ve 3 başlatılmadı.**
Kullanıcı disk/oracle/kill kısmını sonradan kabul etti; yeni maliyet kapısı
yukarıdadır. Ölü payload teşhisi kesinleşti:
kontrolün ilk beş ~53–54MB segmentinde payload %100 ölü; ikili toplamları
64MiB merge sınırını aşıyor. Seçim ölü ağırlığına bakmıyordu.
Tek segment rewrite prototipi mevcut codec/posting ve Weak/pin yollarını kullanır.
İki dengeli 10 tur kıyasında retained 876,347→357,954MB (1,0215× başlangıç),
SQLite 630,485MB. Ayrı 30 tur adayı 365,633MB (1,0434×), en yüksek bakım sonrası
375,357MB; örneklenen geçici tepe 483,678MB ve görünür payload alt sınırına göre
en kötü 1,900×. Akışlı churn OS lifetime tepe private 28,111MB.
90 tur oracle, 38×30 background query, beş gözlenmiş kill/reopen ve eski frozen
okuyucu kontrolleri geçti; fixture/corpus SHA ve kalıcı biçimler aynı.
**Açık arama kapısı:** 60 örnekli birleşik p95, ishmael offset130/limit1
6,098→6,694ms (+%9,77). Ayrı çiftlerde 5/76 +%5 artış var; aynı sorguda iki
çiftte tekrar etmiyor. Bu nedenle bütün artışları dalgalanma sayıp kapatmadık.
Prototip kodu korunur, varsayılan kapalı; `set_revision_reclamation(true)` veya
doğrudan `start_revision_reclamation()` ile açıkça denenebilir.
**Yazma/bakım bedeli:** 10 tur ortalama commit toplamı 34,734→37,461s (+%7,85),
bakım 5,617→32,415s (5,77×), ikisi toplam 40,351→69,876s (+%73,17).
Bakım sonrasında solo görünürlük kapısı tutuldu; aktif rewrite sırasında tek
akışlı yazı 751,524ms, görünürlük804,227ms (tek gözlem, p95 değil).
Önce kalan ishmael sayfasını okuma/doğrulama/normalleştirme/sonuç aşamalarına
ayırmak ve bakımın pahalı aşamasını profillemek gerekir; bunlar bu tur yapılmadı.
En yeni tombstone'lar korunuyor; 30 tur gözlemi süresiz sabit alan garantisi değil.
Snapshot/4 okuyucu+1 yazıcı, küçük commit p95≤10ms, dirty500 açılış/RAM,
genel arama, soğuk yük ve kill sonrası geçici dosya temizliği açık.
Background query probu bağımsız foreground writer içermez; Adım 2 kapısı sayılmaz.
Gözlemci bir geçici `.pending` stat yarışında durdu; hata logu korundu, gözlemci
düzeltildi ve kalan iki kill ayrı kayıtla geçti. Güç kesintisi iddiası yok.
C ABI/DLL tasarımı yalnız sonraki aşama notudur.

<!-- churn-concurrent-20261025 -->
2026-10-25 churn/concurrent ölçümü [yeni raporda](CHURN-CONCURRENT-10K-20261025.md).
92 ana worker bölümü ve dört process-kill/reopen denemesi tamamlandı; bütün
oracle'lar geçti. Motor/format değişmedi. Solo streamed tek belge görünürlük
üst sınırı184,919ms ve stream tepe private47,079MB/WS45,978MB gözlenen kapıları
tuttu. Kirli500 owned delta ile private31,003MB/WS34,800MB:15MB kapısı kaldı.
Churn bakım sonrası350,413→876,347MB(2,501×) iki koşuda da tekrarlandı;
alanın neredeyse tamamı15 manifest-live segmentte, eski nesil temizliği değil
güncel segmentlerde superseded kayıtların tutulması araştırılmalı.
OrtakMutex ile concurrency2× kapısı kaldı; en kötü FDB read1,364s,
kısa negatif sorgu örnek-p95 oranı84464,6× (5/10 örnekte p95=max).
Öncelik okunabilir snapshot/admission ve ardından churn merge politikası;
bu tur optimizasyon yok. Stream her commit sonunda delta0 bırakır; owned
active delta ayrı ölçüldü. Atomik5000 her iki API'de reddedildi;100'lük
fallback ayrı ve atomik değil. Kill sonrası input/build artıkları da açık.
C ABI/DLL/opaque handle/stream/panic-catch sonraki aşama notudur, uygulanmadı.
Önceki yazma/son bakım, arama regresyonları, RAM/disk, gerçek-soğuk ve çok-okuyucu
başlıkları yalnız bu raporun kanıtladığı kapsamda kapanabilir.

2026-10-24 toplu kıyas: `8a92384` (24 Ekim gram-sort/posting/cursor
öncesi,hızlı intake fold dahil) / `2a9fc2c` / native SQLite aynı ilk10k,
iki ters sıralı çift/API. Güncel akışlı toplam17,386→14,944s(-%14,04),
owned14,791→12,289s(-%16,92),ikisinde2/2 düşük;CPU-%16,67/-%18,19.
**Ayrı son bakım+%4,27/+%4,85 ve iki API’de2/2 çift yüksek**; sonraki
yazma/bakım hedefidir,toplam kazanç bunu kapatmaz. Akışlı OS private
10,133→10,154MiB/SQLite8,418MiB,WS+208KiB;owned80,029→79,955MiB/
SQLite8,385MiB. Owned caller Vec / SQLite streaming aynı input RAM işi
değildir;genel RAM açık. DiskFDB349.805.698B/SQLite583.860.224B;
dinamik geçici alan/sıkıştırma açık.12 süreçte full10k oracle,API içi
source/index/ready byte eşitliği,corpus/0.1 SHA koruması geçti.
Sıcak okuma 38 durum×100 örnek/motor: güncel/eski p95 29 düşük/8 yüksek/
1 eşit; güncel/SQLite 38/38 düşük. quixote /28/10 +%12,37, man +%5,50,
zzxqnever +%3,50 iki çiftte yüksek. Diğer artışlar the /0/1 +%7,54,
ishmael /130/1 +%5,45, frankenstein +%1,91, fossil +%1,81,
elizabeth +%0,54 (hepsi1/2 çift düşük). Uzun zx pooled p95 eşit ama
tipik blok +%5,68; arama işi açık. Get p95 dört dolu boyut grubunda eskiye
göre yüksek, eksik key’de eşit; SQLite’a göre beş grupta düşük.
SQLite JSONL’nin UTF-8/cp1252 rapor
okuma hatası yalnız 8 sorgu adı alanındaydı; ham UTF-8 ile diğer tüm
alanlar/süreler eşit doğrulandı, özgün kayıt korundu, benchmark tekrarı yok.
Önceki arama/CLI regresyonları,açılış ve eşzamanlı/gerçek-soğuk yük
bu sonuçla kapanmaz. [Birleşik sonuç ve kanıtlar](CONSOLIDATED-COMPARISON-10K-20261024.md).

2026-10-24 merge posting cursor tekrar kullanımı dar toplam kazancı için
korundu. Input başına mevcut8KiB tampon merge ömrünce tutulur;terim
değişiminde tüketim/pencere/sıra durumları sıfırlanır. Normal aynı ilk10k/
iki çift/API akışlı toplam19,841→19,243s(-%3,01),owned14,903→14,525s
(-%2,54),ikisi2/2 düşük. Akışlı CPU-%1,75 ama1/2 çift düşük;owned-%3,19/
2 çift düşük. Ayrı son bakım akışlı-%4,45/owned-%0,69,ikisinde yalnız1/2
çift düşük;owned çiftleri+%6,18/-%6,88. Bakım işi ve önceki+%11,97 artışın
kesin nedeni açık. Tek diagnostic owned koşusunda drain öncesi9run+
1000resident/compactionfalse,ingest merge0;beş merge drain'de tamamlanır:
bu koşuda çalışan worker carryover yoktur,genel nedensellik kanıtı değildir.
Dar read-only aynı-program posting probe-%6,09/4 blok düşük;gerçek DB
drain/OS RAM sonucu sayılmaz. Akışlı private/WS+2/+244KiB,owned+136/
+304KiB,bütün çift yönleri karma;genel RAM kapısı geçmedi. Disk349.805.698B
ve API içi source/index/ready byte'ları aynı;sekiz full10k/dört contains
oracle,corpus/0.1 SHA eşitliği ve kesik/bozulma/kill kontrolleri geçti.
Geniş38-case20 örnek/yol12 düşük/26 yüksek p95;seçilmiş yedi durumda
100 örnek/yol QUİXOTE+%12,60(iki çift yüksek),the1+%5,68(1 çift yüksek)
p95 ve tipik blok artışları açık. The200-%13,32/iki çift düşük olsa da
önceki farklı oturum artışları otomatik kapanmaz;uzun negatif/son sayfa/
quixote28/10 düşük p95 yanında tipik blok artışları korunur. Oracle ve
iki okuma serisinde21 dosya SHA eşitliği geçti. Önceki arama p95/CLI
kuyrukları,caller/query/genel RAM,kaynak encode/yazma,
açılış,disk sıkıştırma/geçici alan ve eşzamanlı/gerçek-soğuk işler açık.
Yeni corpus/DB kopyası/format/feature/dependency/WAL/protokol yok.
[Sonuç ve sınırlar](MAINTENANCE-WORKSPACE-10K-20261024.md).

2026-10-24 RAM gram posting ekleme dar adımı normal bounded profilde tutuldu.
64 referanslı sınırlı sözlük taraması/exact fallback;önceki fold ve gram
sıralama kazanımları korunur. Normal aynı ilk10k/iki çift/API akışlı toplam
14,381→13,783s(-%4,16),CPU-%6,21;ikisi2/2 düşük. Owned CPU-%4,96/2 çift
düşük fakat toplam12,779→12,803s(+%0,19),yalnız1/2 düşük:owned gecikmesi
açık. Son bakım akışlı+%26,07/owned+%11,97,ikisi2/2 yüksek;worker/yerleşim/
bellek nedenleri kanıtlanmadı,bakım işi kapanmadı. Akışlı private+166KiB ve
WS+396KiB,ikisi2/2 yüksek;owned private-4KiB/1 çift düşük,WS-190KiB/2 çift
düşük;genel RAM kapısı geçmedi. Disk349.805.698B ve API içi source/index/
ready byte'ları aynı;sekiz full10k/dört contains oracle geçti. Corpus/0.1
SHA'ları diagnostic öncesinden normal seri sonrasına eşit. İlk38-case20
örnek/yol23 düşük/15 yüksek. Yoğun100 örnek/yol beagle-%2,14/1 çift hızlı,
frankenstein-%2,01 pooled ama0/2 çift hızlı;eski açıklar otomatik kapanmaz.
The/1+%11,56 iki çiftte yüksek;the200+%3,95/quixote28/10+%8,53 pooled,
ikisi1/2 çift hızlı;tipik blok kazancı tail artışını silmez. Genel arama,
önceki CLI kuyrukları,caller/query/genel RAM,owned latency,son bakım,açılış,
disk sıkıştırma/geçici alan ve eşzamanlı/gerçek-soğuk yük açık. Yeni corpus,
DB kopyası,format,feature veya bağımlılık yok. Yeni SQLite/açılış/cold/
concurrent ölçümü yapılmadı. [Sonuç ve sınırlar](POSTING-MERGE-10K-20261024.md).

2026-10-24 gram sıralamanın dar yazma adımı normal bounded profilde etkin.
Önceki hızlı intake fold korunur;uzun gövdede aynı packed anahtarlar en fazla
8192 pending girdide sort/dedup,taşmada sayısal ağaç kullanır. Normal iki
çift/API:akışlı toplam21,410→19,218s (-%10,24),owned22,436→18,138s
(-%19,16);CPU-%14,86/-%18,05,toplam/CPU her iki API'de2/2 düşük.
Owned toplam çiftleri-%13,17/-%24,51;genel veya evrensel hız tahmini yoktur.
Akışlı son bakım+%9,36/1 çift düşük,owned-%6,08/1 çift düşük:ayrı bakım
işi açık. Private yaklaşık aynı,owned yaklaşık80MiB;akışlı WS+400KiB/2 çift
yüksek,owned+170KiB/1 çift yüksek;genel RAM kapısı geçmedi. Disk349.805.698B
aynı,geçici alan tepesi ölçülmedi. Sekiz süreç full10k+dört contains oracle
ve API içi source/index/ready byte eşitliği geçti;corpus/0.1 kaynakları aynı.
Yeni corpus/DB kopyası/format/feature/WAL/protokol yok. İlk38-case durum başı
20 örnekte13 düşük/25 yüksek p95. Altı seçilmiş durumda100 örnek/yol yoğun
tekrarda beagle+%6,38,frankenstein+%8,96,the200+%1,96 p95 iki çiftte de
yüksek kaldı. Man-%15,93/son sayfa-%0,58 iki çiftte düşük,zx-%1,36/1 çift
düşük. Genel sorgu kapısı geçmedi;artışlar ve önceki p95/CLI kuyrukları
kapatılmadı. Bu tur yeni açılış/soğuk/eşzamanlı yük/tam SQLite kıyası yok.
Caller/query RAM,son bakım,
arama,açılış,disk sıkıştırma/geçici alan ve eşzamanlı/gerçek-soğuk yük açık.
[Tasarım,ölçüler ve sınırlar](GRAM-SORT-10K-20261024.md).

2026-10-22 yazma kazanımı korunarak normal bounded profilde etkinleştirildi.
Yeni `fold_for_intake`,hızlı normalleştirme+scalar ile son kapasite eşlemesi+
write-only call sınırı kullanır. Referans scalar,normal API/format/feature/
WAL/guard/bütçe/delta eşikleri aynı;test-only mod seçimi üretimde yoktur.
Normal4 çift/API:akışlı toplam19,661→18,774s (-%4,51),owned17,922→17,060s
(-%4,81);CPU-%5,11/-%4,95,toplam ve CPU iki API'de4/4 düşük. RAM yaklaşık
aynı:owned private79,89→79,85MiB;fiziksel/toplam RAM değildir. Akışlı son
bakım+%1,85/3 çift yüksek,owned son bakım2/4 hızlı:ayrı bakım işi açık.

Arama ayrıştırmasındaki ilk worker'ın ara Value ömrü CLI ile farklıydı;
düzeltilmiş tek binary/üç mod kontrolünde büyük JSON artışı tekrarlanmadı.
NormalCLI eski/eski A/A da quixote+%3,94 verdi;yeni A/B+%4,23/4 çift yüksek.
Bu iki değer çıkarılıp kesin etki hesabı yapılmaz,CLI kuyruğu açık. AND yeni
A/B+%1,30/2 çift hızlı;önceki+%7,20/4 çift yüksek aynı tutarlılıkta kalmadı.
Yeni aynı-program native kontrolünde altı sorguda rounded p95 düşük,fakat
bu release test binary'si normal genel arama performans garantisi değildir.

Karma dört sorgu yoğun kontrolde man p95+%19,74/4 çift yüksek,tipik blok
+%0,53;zx+%7,16/3 çift yüksek. Bunlar ayrı açık tail işleri. Man tek başına
kontrolde p95+%1,26/3 çift hızlı,cycle p95 yaklaşık aynı. Karma yük/ortam
payları kesin ayrıştırılmadı;"dalgalanma" diye kapanmadı. Genel38-case tam
doğruluk az-örnek smoke geçti,p95 kolonları genel hız sonucu sayılmaz.
Yazma kazanımı bu açıkları çözülmüş ilan etmeden kalıcı tutuldu.
16 normal süreçte10k tam payload+dört contains oracle,API içi core SHA-eşit;
SQLite bu follow-up'ta yazılmadı. Diğer caller/query RAM,açılış,disk/geçici
alan,eşzamanlı/gerçek-soğuk yük işleri açık. [Ölçüler ve etkin karar](INTAKE-FOLD-RECOVERY-10K-20261022.md).

2026-10-21–22 bağımsız intake fold deneyi tamamlandı/reddedildi. Aynı10k,
normal4 çift/API:akışlı yazma+bakım20,251→19,488s (-%3,77),owned19,263→
18,180s (-%5,62),ikisi4/4 hızlı. Akışlı son bakım+%5,58/4 çift yüksek;
owned CPU3/4 düşük,ayrı son bakım2/4 hızlı. RAM kazancı yok:owned private
+150KiB/3 çift yüksek,WS+274KiB/4 çift yüksek;akışlı private+68KiB/3 çift
yüksek. Bunlar OS lifetime sayaçları,fiziksel/toplam RAM değildir.
Unpinned1k sıcak CLI kontrolünde5/6 sorgu4/4 p95 yüksek;aynı çekirdekte
AND+%7,20 yine4/4 yüksek. Native API/JSON ayrımı kök nedeni kesin atfetmedi.
Tam10k/38-case15 düşük23 yüksek;quixote/man/zx/quixote-whale p95 dört
çiftte yüksek,son-sayfa+%3,13/2 çift hızlı. Genel arama kapısı geçmedi;
runtime fold değişikliği geri alındı,eski normal motor korundu. Dar fold
ön testindeki-%81,6 DB'nin toplam kazancı gibi sunulmaz.
24 normal yazma sürecinde10k tam oracle,16 FDB'de dört contains oracle;
API içinde core byte'ları aynı. Boundary sonrası normal10k restore edildi.
194 default/136 all-feature hedef/113 referans;geri dönüşte128+1 test,
fmt ve üç profil Clippy geçti. Yalnız test/probe/worker/tool/kanıt eklendi;
format/feature/dependency/protokol/dış entegrasyon veya yeni corpus yok.
Owned/caller/query RAM,yazma/son merge,genel/son-sayfa p95,açılış,disk/
geçici alan,eşzamanlı/gerçek-cold yük açık. Normalizer'a yeniden geçmeden
önce ölçülen query artışının nedeni ve kayıpsız geçişi kanıtlanmalı.
[Bütün ölçüler,ret ve final kanıt](INTAKE-FOLD-10K-20261021.md).

2026-10-20 RAM sıkıştırma ön testi tamamlandı/reddedildi:aynı ilk10k,
en fazla200 kaynak metin,4 read-only süreç. Bağımsız LZ4 block+UTF-8 alanı
-%34,17;50 metin yeni sonuç p95 medyan0,462→1,250ms,tampon tekrar
0,095→0,930ms.1/50/200 ve CRC-açık/kapalı bütün karşılaştırmalarda4/4
süreç yavaş. Bu yalnız metin allocation kapasitesi/geri-hazırlama zamanıdır;
proses RAM'i veya gerçek DB get/arama kaybı değildir. Ön kapı geçmediği
için motorun RAM yapısı değiştirilmedi;LZ4 bağımlılığı yalnız ayrı araçta.
5 test/root+probe fmt/Clippy geçti;21 DB/cache/0.1 dosyası ve corpus SHA-eşit.
DB açma/reset,yeni corpus/kalıcı codec/engine feature veya dış entegrasyon
yok. Normal Vec/caller/query RAM,son sayfa/genel p95,yazma/son merge,
açılış/disk sıkıştırma,eşzamanlı/gerçek-cold işler açık.
[Ön test ve kanıtlar](RAM-TEXT-LZ4-10K-20261020.md).

2026-10-20 owned RAM kök nedeni ölçüldü: sonraki500 input, önceki1k
özgün+folded delta ile üst üste; max aşama71,39MiB requested-live, bunun
41,13MiB'si input+özgün belge kökeni,28,91MiB'si RAM arama indeksi. Bu
OS/physical RAM değildir; o anda WAL frame0. Erken flush+hızlı intake fold
normal4 çiftte private80,10→66,75MiB/toplam15,726→15,110s,CPU-%3,91,
disk/core byte'ları aynı verdi. Ancak ilk1k watermark sonrası6 CLI sorgusunun
tamamı4/4 çiftte yavaş: +%19–516p95. Genel geçiş reddedildi, iki runtime
değişikliği geri alındı; normal motor ve önceki yazma kazanımı korundu.
Opt-in aşama gözlemleri/paired araçlar kaldı. Yeni corpus/DB kopyası yok,
boundary sonrası sabit FDB slotu eski normal10k'ya döndü. Normal Vec RAM'i
kapanmadı: yalnız flush zamanını değiştirmek yeterli değil; sıcak yeni
belge okumasını koruyan input/delta tasarımı gerekli. Son sayfa/genel arama
p95,yazma/son merge,query/caller RAM,açılış/disk sıkıştırma,eşzamanlı/cold
yük açık. [Rapor ve bütün örnekler](OWNED-MEMORY-10K-20261020.md).
Geri dönüş193 default/135 all-feature hedef/112 referans ve diagnostic example2
test,fmt/üç profil Clippy,10k/38-case/256-key oracle ve CLI ile doğrulandı;
core/corpus/0.1 fixture byte'ları aynı,SQLite read-only integrity başarılı.

2026-10-18–19 yazma/bakımın dar kimlik eşleme adımı uygulandı/ölçüldü:
19.221.340 posting girdisi10.000 Key aramasıyla eşleniyor. Normal akışlı
yazma+bakım27,277→22,334s (-%18,12;4/4 çift),owned23,274→17,246s
(-%25,90;2/2); CPU bütün çiftlerde düşük. Kaynak/indeks byte'ları API başına
aynı. Streamed son bakım +%0,96/2/4 hızlı: ayrı merge kazancı kanıtlanmadı.
Owned private+310KiB/WS+454KiB,ikisi2/2 yüksek ve açıklaması açık. Ayrı
query/get private -52KiB/WS -72KiB,3/4 düşük; owned RAM'i kapatmaz.
Geniş38'de11 düşük/27 yüksek p95; yoğun9/200'de8/1. Son sayfa
1,8581→1,9965ms (+%7,45) açık; genel arama/RAM kabulü geçmedi.
SQLite yazma kıyası ve iki çift38-case FTS5+LIKE okuma kıyası tamamlandı;
38/38 p95 FDB düşük,bu kendi okuyucumuza karşı artışları kapatmaz.
193 default/135 all-feature hedef/112 referans test;fmt/üç profil Clippy,
kesilme/bozulma/kill,full10k/38-case oracle ve CLI geçti.21 okuma dosyası,
corpus/0.1 fixture SHA-eşit;format/feature/protokol/entegrasyon değişmedi,
yeni corpus/DB kopyası yok. [Rapor ve ham kanıtlar](WRITE-STAGES-10K-20261018.md).
Metin parçalama/RAM posting CPU'su,son merge,normal Vec/caller RAM'i,kalan
arama p95'i,açılış/disk sıkıştırma ve eşzamanlı/gerçek-soğuk yük açık.

2026-10-18 kullanıcı "bu motora geç" kararıyla ölçülen V3'ü normal bounded-engine
profiline aldı. `search_into` etkin; önceki sayfayı tekrar veren çağıran uygun
metin tamponlarını kullanır. `search` uyumlu kalır. Geçiş doğrulandı: normal
paket 191 test/7 ignored; fmt ve üç profil Clippy geçti. Normal release'te
search38 + search_into38 tam belge/sıra/sürüm oracle kontrolü başarılı.
Sayaçlar ikinci the200 çağrısında 200 tamponun yeniden kullanıldığını ve aynı
okuma/CRC/schema/kimlik kontrollerini doğruladı.21 veri/fixture dosyası ve
bütün corpus SHA-eşit; yeni corpus/DB/reset/kopya/format/feature yok.
Önceki kabul reddi aşağıda tarihçedir; kapı geçildi denmiyor. Tek sonuç/normal
okuyucu p95 artışları, OS private+28KiB/WS+48KiB ve geniş sorgu farkları açık.
[Güncel geçiş kaydı](RESULT-BUFFERS-ACTIVE-20261018.md).

Yazma/bakımın ilk yeni adımı yukarıda ölçüldü. Normal Vec ingest/query/
caller RAM'i, Harpoon/genel arama p95'i, açılış/disk sıkıştırma ve eşzamanlı/
gerçek-soğuk yük kapanmadı.

## Geçiş öncesi kabul değerlendirmesi

2026-10-18 sonuç tamponu deneyinin genel geçişi reddedildi. Arşiv prototip
aynı the200 tekrarında p95 -%38,78 (4/4 çift,16/16 blok), değişen dört sayfada
-%10,18 verdi; tek sonuç p95 +%10,68 açık. Geniş38'de the200 p95 yaklaşık aynı
(+%0,01). Eski normal okuyucuya karşı 38-case kontrolünün ardından yoğun400
tekrarda normal API/the200 +%10,01 p95 kaldı (yalnız1/4 çift hızlı); tipik
blok +%0,76. Bekleme/dalgalanma denilerek kapatılmadı. Oracle'sız OS private
medyan +28KiB/WS+48KiB; RAM kazancı yok. Motor/test kaynakları f89b827'ye
döndü; yeni API/helper yalnız source patch'te. Prototip191/7ignored ve üç
profil Clippy geçti. Geri dönüşte engine18/payload2/grouped5/fixture1/aktarim1,
fmt/üç profil Clippy ve normal38-case full oracle geçti.21 veri/fixture dosyası
ve tüm corpus SHA-eşit; yeni corpus/DB/reset/kopya/format/feature yok.
[Güncel rapor](RESULT-BUFFERS-10K-20261018.md).

Sonraki bağımsız öncelik yazma/bakım aşamalarını ayırmak. Normal Vec ingest
RAM'i, tek sonuç/Harpoon/genel arama p95'i, açılış/disk sıkıştırma ve eşzamanlı/
gerçek-soğuk yük açık. Büyük sayfa tekrarındaki dar kazanç bu işleri kapatmaz.

## Önceki hydration profili

2026-10-18 the200 hydration profili tamamlandı; bu tur hız kazancı yok.
Diagnostic30 tekrar: read115/1,810ms, CRC200/0,577ms, schema200/0,586ms,
materialize200/3,631ms; içindeki text String tahsisi+kopyası2,907ms baskın,
fields0,557ms, key0,050ms, links0,042ms. Span'ler nested/instrumented;
normal p95/CPU yüzdesi değildir. Dönen metin5.383.655B; tahsis/memcpy/
ilk erişim payı ayrılmadı. Codec/CRC ayarı ana maliyeti açıklayan çözüm
diye seçilmedi. Sonraki hipotez caller-owned sonuç tamponu tekrar kullanımı;
ayrı API/ömür sözleşmesi ve ölçüm gerekir, henüz uygulanmadı.

İki tek-geçiş prototipi reddedildi: ilk400 örnek the200 p95+%11,58,
tipik+%11,33; eski tahsis sırasını koruyan ikincide p95-%0,42, yalnız8/16
blok hızlı. Ayrı sonuç penceresi64→128KiB çağrıyı115→47 azalttı fakat
normal400 örnekte the200 p95+%8,32/tipik+%16,38,1/4 çift hızlı; reddedildi.
Üçü de kod/test patch'inde arşivlenip geri alındı. Algoritma/source-check/
codec/format aynı, yalnız mevcut diagnostic feature'da sayaçlar kaldı.

The200 uçtan uca hedefi, Harpoon p95, kalan arama farkları, normal Vec/
query/caller RAM, yazma/bakım, genel açılış/disk sıkıştırma ve eşzamanlı/
gerçek-soğuk yük açık. Hız kabulü geçmediği için yeni38-case hız/OS RAM
koşusu yok; final normal38-case full oracle var. Default185/7 ignored,
referans engine17, all-features payload5/grouped5, fmt/üç profil Clippy
geçti; son restore kaynak checkpoint'e byte-eşit, grouped/fmt/default+
all-features Clippy tekrar geçti. Yeni corpus/DB/reset/kopya yok.
[Son hydration raporu](HYDRATION-10K-20261018.md).

## Önceki arama ve kapasite takibi

2026-10-18 sonraki dar takip tamamlandı: ölçülmüş2.464B ek canlı sonuç
kapasitesi giderildi. 38/38 sorguda kapasite artmadı; canlı heap farkı
tam olarak kapasite farkı×176B. Boş sonuçta capacity/live delta0; hemen
elenen negatiflerin tahsis/tepe sayıları aynı. Candidate metni tarayan
negatif ve pozitiflerde yeni4KiB scratch ile geçici peak yaklaşık12KiB
artabilir; genel RAM işi kapanmadı.

Profil beagle19.483→5.520, harpoon46.903→14.039 body fold/scan çağrısı;
38/38 candidate CRC/schema/accepted/payload request-byte/reuse sayıları
aynı. Normal80 örnek/case/variant:38 sorgu31 düşük/7 yüksek p95, beagle
-%20,33 (4/4 çift,8/8 blok). Aynı SHA'larla yoğun9-case200 örnekte beagle
-%3,49 (4/4,8/8), tipik -%4,24. Harpoon tipik -%2,33/8/8 hızlı fakat
yoğun p95 -%0,43/2/4: genel p95 kazancı kanıtlanmadı. The200 hydration
algoritması değişmedi, yoğun p95 -%0,51; büyük kazanım/kök neden işi açık.
Geniş man+%11,60/İSHMAEL+%10,51 yoğun yükte tekrarlanmadı; Unicode-fold
ve sayfa artık artışları açık tutulur. Önceki38/80 ve9/200 örnekler ayrı
dağılımlardır; «bütün regresyonlar çözüldü» denmez.

Oracle'sız4 çift/24search771get OS peak private8,75391→8,78906MiB (+36KiB),
WS12,71289→12,72266MiB (+10KiB); ikisi4/4 artış. Yazma/bakım, diğer sorgu
regresyonları, normal Vec/query/caller RAM, genel açılış, disk sıkıştırma,
eşzamanlı/gerçek-soğuk yük açık. Default185/7ignored, referans engine17+
scratch2, all-features engine1+scratch4, fmt ve üç profil Clippy geçti.
Normal bütün38+yoğun9 full oracle ve CLI kontrolleri geçti.21 dosya ve
tüm corpus SHA-eşit; yeni corpus/DB/reset/kopya/format/feature yok.
[Son arama/kapasite takibi](SEARCH-FOLLOWUP-10K-20261018.md).

## Varsayılan geçişi ve önceki ölçümler

2026-10-18 güncel karar: kullanıcı talebiyle ölçülen checked-page algoritması
varsayılan `bounded-engine` motoruna alındı. Aynı arşiv kodu/beş test;
feature/API/kalıcı format değişikliği yok. Kaynak ve candidate doğrulaması
korunur. Default185 test (7 ignored), referans engine17 ve all-features
hedefli5 test, fmt ve üç profil Clippy geçti. Normal release38 sorgu tam
oracle ve CLI kontrolü, diagnostic default reuse kanıtı geçti.

Son normal release A/B: aynı10k,38 sorgu,80 örnek/motor/sorgu, sıcak cache,
dört dengeli süreç çifti. P95:20 düşük/18 yüksek; quixote -%6,62
(3/4 çift, tipik blok -%18,40,8/8 hızlı), quixote-whale -%11,03 (4/4).
Beagle +%14,33 (4/4 yüksek), harpoon +%14,91, the/200 +%8,88 ve
ishmael/130/1 +%13,12 açık; he/zx ve uzun negatif tipik blok maliyeti de
kapanmadı. Ayrı oracle'sız24 search/771 get,4 çift: OS peak private
8,7090→8,7363MiB (+28KiB,3/4 artış); WS12,5859→12,6797MiB
(+96KiB,4/4 artış). Genel bütün-sorgu/OS RAM kapısı geçti denmiyor;
varsayılan geçiş kullanıcı kararıdır. Önceki tek-executable tekrar ve
bu iki-executable sonuç birbirine karıştırılmaz.

Öncelikli küçük takip: ölçülmüş2.464B fazla sonuç Vec kapasitesi ve ortak
boş/negatif yol. Bu geçişte kapasite veya eşik ayarı yapılmadı; yeni hız
iddiası bunlara atfedilmez. Beagle/harpoon/sayfa p95 payları, yazma/bakım,
normal Vec/query/caller RAM, genel açılış, disk sıkıştırma, eşzamanlı ve
gerçek-soğuk yük açık.21 dosya ve tüm corpus SHA-eşit; FDB349.805.698B,
SQLite583.860.224B, fixture/Cargo aynı. Yeni corpus/DB/reset/kopya yok.
[Varsayılan geçiş raporu](DEFAULT-CHECKED-PAGE-10K-20261018.md).

## Önceki ölçümler ve kararlar (varsayılan geçişinden önce)

2026-10-18 arka plan işleri kapatıldıktan sonraki tekrar tamamlandı.
Aynı SHA’lı program, aynı 10k, 38 sorgu /120 örnek/koşul: quixote p95
-%11,64/-%10,59 (iki kapalı kontrol), ikisinde 4/4 süreç ve 23/24 blok hızlı.
Yoğun dört sorgu /240 örnek: quixote -%13,68/-%14,96; 4/4 süreç, 24/24 blok.
Holmes genel p95 kazancı hâlâ tutarsız; zx’in eski büyük artışı bu koşuda
tekrarlanmadı. Uzun negatif yoğun p95 +0,9/+1,7µs (+%2,86/+%5,54), ikinci
kontrole göre 4/4 süreçte yüksek; açık. the/1 ve ishmael/130/1 artışları
dört-sorguluk yükte iki kontrole göre tutarlı tekrarlanmadı; geniş sorgu
setindeki artışlar kapatılmadı. Sıra/durum/yerleşim payları tam ayrılmadı.

Geniş koşuda blok CPU/wall medyanı %99,60; blok süreleri kararlı, tamamen izole sistem iddiası yok. Genel 38-case/OS RAM kabulü geçmedi;
aktif motor kaynakları aynı, prototip varsayılana alınmadı. Sonraki küçük
kod işi: 2.464 B sonuç kapasitesi fazlasını gidermek ve ortak boş/negatif
hızlı yolu korumak. Henüz uygulanmış çözüm değil. Yazma/bakım, diğer arama
regresyonları, normal Vec/query/caller RAM, açılış, sıkıştırma ve eşzamanlı/
gerçek-soğuk işler açık. fmt/Clippy ve tüm oracle kontrolleri geçti; 21 dosya
ve tüm corpus SHA-eşit, yeni corpus/DB/reset/kopya yok.
[Son tekrar](REUSE-REPEAT-10K-20261018.md).

2026-10-18 tek-program kontrolü tamamlandı; yeni çalışma yönü daraldı.
38-case /120 örnek/koşulda off1/off0 20 düşük/18 yüksek p95; on/off0
22/16, on/off1 19/19. Genel p95 kabulü yok. Quixote dar tipik blok kazancı
iki kapalı kontrol karşısında sürüyor; holmes genel kazancı kanıtlanmadı.
Uzun negatif karışık/tek-sorgu normal düzende off0'a göre 4/4 süreçte daha
yüksek p95; kapatılmadı. Payload read/CRC/retention sıfır, requested heap
sayaçları aynı: CRC/codec ayarı bu kalemi hedeflemez. Büyük wall/CPU
farkları bekleme payını kanıtladı; bütün eski artışları veya belirli bir
uygulamayı açıklayan kök neden olarak sunulmadı. AboveNormal da plasebo
oynaklığını gideremedi.

Pozitiflerde sorgu sonrası +2.464B canlı heap: sonuç Vec kapasitesi
52→66, 14×176B ile eşleşti. Bu, önceki +64KiB private/+170KiB WS'nin
tamamının açıklaması değildir; yeni oracle'sız OS RAM koşusu yok. Sonraki
kod işi: ortak boş/negatif yolu korumak, yeniden kullanım helper'ını gerçek
pozitif aşamasında tutmak ve sonuç vektörünün fazla kapasitesini düzeltmek.
Bu henüz uygulanmış/hızlanmış bir çözüm değil. 2MiB/50 eşik turu veya
format değişimi yok. Genel geçiş için plasebo kontrollü bütün 38 sorgu ve
ayrı proses RAM kapısı gerekiyor.

Prototip 190/111/209 test (7/3/9 ignored), fmt ve Clippy geçti; aktif
runtime/test/Cargo başlangıç byte'larına döndü. Geri dönen motorda 7 window
testi, fmt/default+all-features Clippy geçti. On fazda 21 dosya ve bütün
corpus SHA-eşit; yeni corpus/DB/reset/kopya yok. Yazma/bakım, diğer arama
regresyonları, normal Vec/query/caller RAM, genel açılış, disk sıkıştırma,
eşzamanlı/gerçek-cold yük açık. [Son rapor](REUSE-CONTROLS-10K-20261018.md).

2026-10-18 pozitif contains sonrası ikinci payload okuması ölçüldü. Son
izole prototipte quixote read API131→81, ikinci CRC/decode50→0, candidate
CRC56→56; holmes1514→1467 read/1285 candidate CRC korunur. Aynı mevcut10k,
400-örnek/variant quixote p95-%11,11/tipik-%17,92 (4/4 süreç,15/16 blok),
holmes-%13,52/tipik-%3,82 (4/4,15/16). Dar8-case8/8 düşük, önceki he/the200
artışları bu tekrarda yok. Buna rağmen geniş38-case15 düşük/23 yüksek p95;
quixote-%6,17, zx+%15,14, the200+%10,83. Ortam/yerleşim/kod payları tam
ayrılmadı; artışlar kapatılmaz. Query/get peak private medyan+64KiB,
WS+170KiB; ikisi4/4 süreçte arttı. Genel geçiş reddedildi, prototip/beş
test patch'te saklanır. Sonraki iş bu dar iş azalmasını genel sorgu ve bellek
regresyonu olmadan koruyan tasarım olmalı; henüz çözülmedi. Daha fazla2MiB/
50/eşik ayar turu yok. Yazma/bakım, genel arama, normal Vec/query/caller RAM,
açılış, disk sıkıştırma ve eşzamanlı/gerçek-cold yük açık kalır.
[Bütün sonuçlar ve sınırlar](CHECKED-PAGE-10K-20261018.md).
Final: prototip185/107/204 test, fmt ve default/all-features Clippy geçti.
Runtime ve test kaynakları97f6ef7'a byte-eşit döndü;7 window testi ve fmt/
Clippy yeniden geçti. Normal CLI10k/sequence20, dört sorgu total/hit/version
kontrolü ve21-file SHA eşitliği geçti;12 ölçüm snapshot'ı ve tüm corpus da
SHA-eşit. Yeni hız/RAM sonucu aktif, geri dönülen motora atfedilmez.
[Final doğrulama](benchmarks/checked-page-final-proof-20261018.json).

2026-10-18 CRC/UTF-8 parçalama hipotezi ölçüldü ve reddedildi. Baseline
12 diagnostic örnekte holmes CRC3,491ms/yapı4,215ms; zx26,849/26,569ms.
Normal4-case/400 örnek kontrolünde holmes p95+%1,80 (3/4 çift yüksek),
zx+%0,32 (4/4 yüksek), tipik blok+%0,79/+%1,77. Yeni helper'a girmeyen
the/200-%2,35 farkı algoritma kazancı sayılmaz; kod/yerleşim/ortam payları
tam ayrılmadı. Runtime cb2c19e'a byte-eşit döndü; private unsafe modül
kaldırıldı, kod/testler patch'te saklandı. Yeni38-case/RAM koşusu yok.
Gruplama ve CRC chunk ayar turları kapalı. Ayrı sonraki hipotez: eşleşen
belgenin sonuç hazırlamada tekrar okunması/doğrulanması; henüz uygulanmadı.
Yazma/bakım, genel arama, normal Vec/query RAM'i, açılış, sıkıştırma ve
eşzamanlı/gerçek-cold işler açık. [Son kanıt](VALIDATION-CHUNK-10K-20261018.md).
Üç prototip ve yedi geri dönüş testi, fmt ve default/all-features Clippy
geçti; normal release tekrar hazır.21 dosya ve corpus SHA-eşit. Tam-suite
bu tur yeniden koşulmadı. Pozitif tekrarda quixote56+50, holmes1285+50
CRC/decode çağrısı ölçüldü; negatif zx'te ikinci hydration işi yoktur.

2026-10-17 gruplamayı hızlı döngüden ayıran ikinci prototip de reddedildi.
Kaynakta eski döngüler korundu; aynı10k/4 çift/16 blok/400 örnek kontrolünde
darcy p95-%2,74 (3/4 çift), holmes+%0,98 (2/4 düşük), the/200-%2,98.
Tipik blok -%2,32/-%1,39/-%0,88; holmes'ta tekrarlanan p95 kazancı yok.
the/200 artışı bu koşuda tekrarlanmadı; kök nedeni kesin çözüldü sayılmaz.
Dar kapı geçmedi, yeni38-case/diagnostic/RAM koşusu yapılmadı. Runtime ve
window_tests f6d18dd'a byte-eşit döndü; kod/testler patch'te saklandı.
Gruplama ayar denemeleri burada kapandı; sonraki bağımsız arama işi kalan
byte doğrulama/tarama/hydration tekrarını ölçüp tek hedef seçmek olmalı.
Yazma/bakım, genel arama, normal Vec/query RAM'i, açılış, disk sıkıştırma,
eşzamanlı/gerçek-cold işler açık. [Son kontrol](PAYLOAD-ISOLATED-10K-20261017.md).
18 Ekim final kontrolü:180 default/all-targets test, fmt ve default/all-features
Clippy geçti;21 dosya ve tüm corpus SHA-eşit. Yeni RAM/38-case sonucu yok.

2026-10-17 payload adaylarını gruplama prototipi reddedildi/default'a alınmadı.
Aynı mevcut10k'da holmes read1285→1106, darcy2129→1700, watson1954→1553;
ek byte%0,1 altı, bütün38 senaryoda doğrulama ve tarama iş sayıları aynı.
Normal38-case34 düşük/4 yüksek; holmes p95-%9,18 (4/4 çift), tipik-%2,73.
the/200 ilk p95+%10,38, yoğun800 örnek/variantta+%6,02 ve tipik+%2,78.
A/A the/200 p95+%1,69/tipik-%3,74; kod/yerleşim/ortam payı ayrılmadı,
artışlar dalgalanma sayılıp kapatılmadı. Query/get peak private+46KiB,
WS+72KiB medyan; disk349.805.698 B/hash aynı. Runtime ve test kaynağı
6eb3a89'a döndü, prototip patch'te saklandı. Sonraki adayda planlama ayrı
yolda tutularak mevcut hızlı döngü korunmalı; bu henüz ölçülmüş çözüm değil.
Yazma/bakım, genel arama/RAM, normal Vec, açılış, sıkıştırma, eşzamanlı/
gerçek-cold işler açık. [Son deney ve kanıt](PAYLOAD-BATCH-10K-20261017.md).
Final default180/reference107/all199 test (örnekler dahil), fmt ve default/
all-features Clippy geçti;21 dosya snapshot'ı ve tüm corpus SHA-eşit.

2026-10-17 seçilen belgelerde metadata tekrarı azaltıldı: aynı 350 MB /10k
DB'de the/200 metadata çağrısı 714→514; CRC/schema/key/version korunur.
İki A/B dizisinde dar kazanç var; yoğun 400 örnek/sayfa/variantta p95
6,496→5,779 ms (-%11,04), 4/4 süreç ve16/16 blok hızlı, tipik -%12,75.
Ayrı query/get private medyanı -30 KiB, WS -6 KiB; genel RAM kapısı değil.
Geniş 38-case23 düşük/15 yüksek; p95 sapmasının kesin nedeni ayrılmadı.
Yeni A/A'da 29 düşük/9 yüksek, yoğun aynı-mod A/A tipik farklar küçük;
kontrol oranları A/B'den çıkarılmaz ve kalan artışlar kapatılmaz.
holmes payload okuması hâlâ1285 /35,9 MB; quixote56 /1,95 MB aynı.
Sıradaki arama hedefi normal kelimelerde gerçek payload okuma/aday maliyeti.
Yazma/bakım, normal Vec/query/caller RAM'i, açılış, disk sıkıştırma ve
eşzamanlı/gerçek-cold yük açık. Default180/reference107/all199 test geçti,
fmt/Clippy temiz; format/feature/bağımlılık ve0.1 fixture değişmedi.
[Bu adımın kanıtı ve ayrıntılar](SEARCH-HYDRATION-10K-20261017.md).
Güncel aynı-veri SQLite read-only karşılaştırması tamamlandı: 38/38 p95,
her sorguda4/4 çift FDB düşük. Ayrı query/get peak private FDB8,78 /
SQLite12,83 MiB, WS12,53/16,70 MiB; disk350/584 MB. Aynı mevcut8 kitap/
ilk10k, query/get full-payload oracle ve8 snapshot çiftinde hash eşitliği
geçti; corpus'un tamamı SHA-eşit. İlk iki hız çifti kota kesintisi öncesi,
son ikisi aynı binary/DB hash'leriyle sonra tamamlandı; koşullar raporda.
Bu sonuç önceki FDB'ye göre15 p95 artışını, payload okuma maliyetini,
normal Vec ingest RAM'ini, açılışı veya yazma/bakımı kapatmaz.

2026-10-17 dar arama kazancı tamamlandı: aynı350MB/10k DB'de uygun kısa
ASCII yolunda gövde fold263.627→0; CRC/schema/UTF-8/key/version aynı.
Son normal strict blok p95 zx277,138→190,633ms (-%31,21), the-zx
266,181→175,714ms (-%33,99); ikisi4/4 süreç çiftinde,32/32 blok hızlı.
Tipik blok kazancı%30–31. Tüm38-case28 düşük/10 yüksek; genel p95 kapısı
açık. Aynı SHA A/A uzun38/38, kısa36/38 yüksek tail; tipik blok farkları
-%2,41…+%2,74. Kalan artışlar dalgalanma sayılıp kapatılmadı.
Diğer36 sorgunun izlenen bütün iş sayıları aynı.4KiB chunk reddedildi.
Son query peak private/WS medyanı -%0,38/-%0,37; genel RAM kapısı değil.
Açılış gözlemi -%6,81, algoritmik kazanç sayılmadı.18 dosya349.805.698 B
hash-eşit, yeni corpus/reset/repack yok. Yazma/bakım, normal Vec/query/
caller RAM'i, genel açılış, sıkıştırma, concurrent/cold hâlâ açık.
[Sonuç, bütün38 tablo, A/A ve sınırlar](SEARCH-STAGES-10K-20261017.md).

Standart eski nesil temizliği tamamlandı: mevcut10k'da105 dosya,
622.859.277 B kaldırıldı; toplam972.664.975→349.805.698 B (-%64,04).
Canlı kaynak/cache/ready byte'ları ve10k/38-case oracle aynı; yeni
corpus/kopya yok. İki yeni ingestte de350MB, canlı17 core hash-eşit.
Yazma+bakım iki çiftte+%1,34 (1/2 düşük), bakım+%2,82; open ilk+%8,40,
tekrar-%2,86, aynı-program+%3,00. Performans etkileri gürültü diye
kapatılmadı. Default159/reference90/all177 tam test, ayrıca üç profilde
automatic publication/cleanup kill1'er test; fmt/Clippy temiz.
Arama22/38 artış, yazma/kabul/CRC ve spool tekrarları, normal Vec/query RAM,
açılış, eşzamanlı/gerçek-cold ve güç kesintisi hâlâ açık. Dynamic geçici
isimler ve root'tan ileri orphan'lar için genel alan geri kazanma sınırları
raporda. [Güncel temizlik raporu](RECLAIM-10K-20261017.md).

Kullanıcı kararı: SEG05 flag1 yeni native indekslerde varsayılan. Commit/
flush, merge ve cache onarımı gerçek kaynak entry adresini doğrudan yazar;
önce kopyalı SEG04 oluşturulmaz. Okunabilir eski cache'ler korunur, kaynak/
WAL/manifest/0.1 fixture değişmez. Default147/reference78/all165 test,
fmt ve üç profil Clippy temiz. Yeni4 çiftte yazma+bakım28,559→25,744s
(-%9,86), strict open259,443→169,180ms (-%34,79), canlı indeks353,7→
81,9MB, retained core1,521GB→972,7MB. Ayrı query private8,883→8,766MiB;
akışlı write private10,156→10,301MiB (+%1,42;4/4 yüksek).38-case16 düşük/
22 yüksek p95; arama ve genel RAM kapısı kapanmadı. Yeni Run'ın son
kabulündeki sahipli replay tekrarı, geçici JSON spool maliyeti, normal Vec
RAM'i, eski nesiller ve concurrent/
gerçek-cold açık. [Geçiş raporu](DEFAULT-SEG05-10K-20261017.md).

Önceki isteğe bağlı prototipin tarihsel sonucu:
Son SEG05 deneyi: JSON dictionary73→41ms (kopya korunurken instrumented),
source referansıyla267.849.329B payload kopyası çıkarıldı. Son normal4 çiftte
strict open260,075→170,024ms (-%34,62;4/4), ilk kullanım264,784→173,884ms.
Canlı indeks353.688.186→81.904.473B; ayrı query/get private8,891→8,795MiB,
WS13,301→13,260MiB,4/4 düşük. Kayıp read-ahead1104→10000 isteğe çıkmıştı;
5/42B kaynak başlıkları tanınınca1104/8896 hit geri geldi,10k CRC/schema aynı.
Geniş38-case32 düşük/5 yüksek/1 eşit, yoğun8 sayfa5/3; artışlar kapanmadı.
Normal yazıcı SEG04; isteğe bağlı prototype otomatik/default'a alınmadı.
Sonraki kalıcı adım source adresini streamed builder'a bağlamak ve yazma/
bakım/query/get/bellek kapılarını birlikte ölçmek. Vec RAM'i, eski nesiller,
kernel/file cache, concurrent/gerçek-cold açık. Yeni corpus/reset/DB kopyası
yok; eski okuyucu38-case kontrolüyle SEG04'e döndü,133 dosya byte-eşit.
[Son rapor](COMPACT-INDEX-10K-20261017.md).

Önceki indeks açılışı adımı: kaynak/WAL ve sözlük yolunu koruyan bağımsız
exact-read CRC helper'ı. İki ayrı dört dengeli çiftte strict open+ilk
get+search279,712→267,971ms (-%4,20),282,370→264,915ms (-%6,18);8/8 hızlı.
Yalnız open -%1,08/-%8,16; tek sabit yüzde iddiası yok. Yoğun8 sayfa p95
düşük; son adayda geniş38-case performansı yeniden ölçülmedi. Ayrı dört
query/get memory çiftinde private/WS tepe artışı yok; genel/Vec RAM kapısı
kapanmaz. İlk COW/sözlük adayı yoğun8/8 p95 +%5,17…10,08 ile geri alındı.
Metadata/sözlük maliyeti, yazma/bakım, önceki arama artışları, disk ve
eşzamanlı/gerçek-cold yük açık. Kaynak/indeks byte'ları korunur; yeni
corpus/reset/kopya yok. Son dört-çift instrumented profilde CRC83,831→
78,336ms, toplam indeks155,886→158,377ms; indeks toplamı hızlandı iddiası
yok. Son testler155/94/174, fmt/Clippy ve normal CLI geçti.
[Son rapor](INDEX-OPEN-10K-20261017.md).

Güncel dar açılış adımı: aynı mevcut10k/dört dengeli çiftte normal strict
open430,303→270,116ms (-%37,23;4/4), açılış+ilk get+search423,924→281,714ms
(-%33,55;4/4). Kaynak CRC/schema/key/link/version kontrolleri tam; pending0.
Sağlam cache varken kaynak belge nesneleri kurulmaz, tek doğrulama geçişi
kullanılır.38-case son kontrol23 düşük/15 yüksek p95; genel arama kapısı
kapanmadı. Kaynak/cache/SQLite hash'leri aynı; yeni corpus/reset/kopya yok.
İndeks CRC/directory hazırlığı, normal Vec RAM'i, yazma/bakım tekrarları,
disk ve concurrent/cold açık. Yoğun8 sayfa kontrolünde p95 +%1,13…6,22;
aynı-kod A/A'da7/8 yüksek, kod payı sıfır diye kapatılmadı. Son testler
154/93/173 ve fmt/Clippy geçti. Yeni sıcak teşhiste kaynak100ms/indeks162ms;
sonraki açılış hedefi indeks CRC/directory hazırlığıdır.
[Güncel rapor](STRICT-OPEN-10K-20261017.md).

Son SQLite karşılaştırması: aynı mevcut10k, normal default release,
dört dengeli akışlı/SQLite çifti ve dört38-case okuma çifti. Yazma+bakım
23,598/34,509s (-%31,62;4/4);38/38 p95 ve her durumda4/4 FDB hızlı.
Ayrı oracle'sız query/get peak private9,189/12,740MiB, WS12,824/16,695MiB.
Akışlı ingest private9,156/8,402MiB (+%8,97); normal Vec78,912MiB hâlâ
yüksek. Strict open433,686/1,535ms, disk1.521/584MB; Vec son bakım5,104/
1,646s. Bu karşılaştırma tamamlandı; önceki eski/yeni arama artışları,
mutlak olumsuz tarama gecikmesi, Vec/query RAM kapsamı, bakım tekrarları,
disk/eski nesil ve concurrent/gerçek-cold kapıları açık. Yeni corpus/DB
kopyası/engine değişikliği yok; tüm belge, query ve hash/integrity kontrolleri
geçti. [Güncel kanıt ve sınırlar](SQLITE-DEFAULT-COMPARISON-10K-20261016.md).

Kullanıcı isteği: sonraki performans sonuçlarında bu listeyi hatırlat ve
tamamlanan işi kanıt/rapor bağlantısıyla kapat. Zamanlanmış otomasyon değildir;
bu repo içindeki çalışma takibidir. Yeni corpus yok; aynı ilk10k belge ve sabit
iki DB slotu kullanılır. Büyük koşu için önce10k sonuçları iyileştirilir.

Güncel varsayılan `bounded-engine` olarak kalıcılaştırıldı: aynı ölçülmüş
birleşim, eski feature adı uyumluluk alias'ı, özel bayrak gerektirmeyen
streamed API; normal Vec/strict open/WAL sync/format aynı. Default151,
no-default92,all170 test ve Clippy/fmt geçti. Streamed tepe private9,252MiB
(ayrı tekrar9,182MiB), WS13,139/13,043MiB; Vec79,111MiB, boşta1,889MiB.
İlk write+bakım+%4,88, ayrı tekrar-%0,87; A/A-%2,33: artış kanıtsız
kapatılmadı. Geniş arama24/14, yoğun sayfa5/3; üç küçük sayfa artışı
%0,26…1,30.16 streamed koşu122 core hash'i aynı; önceki profile de eşit.
Bu dar taşıma tamamlandı; genel hız/Vec-query RAM/disk/concurrent/cold
kapıları açık kalır. [Son ölçüm ve karar](DEFAULT-PROFILE-VALIDATION-10K-20261016.md),
[normal kullanım ve sınırlar](DEFAULT-PROFILE.md).

Önceki kısa-blok kontrolü: iki farklı-sürüm serisinde8/8 sayfanın yeni p95'i
düşük; ishmael tek-sonuç -%6,51 / -%3,05, quixote /28/10 -%4,39 / -%2,45.
Aynı-kod kontrolünde the tek-sonuç +%6,44 ve4/4 yüksek olabilir. Bu düzende
gerileme tekrarlanmadı; yeni worker/codegen ve cache oturum ömrü nedeniyle
eski kesintisiz koşulların tüm payları veya genel kabul kapısı kapanmadı.
Motor değişmedi; %11,49 yazma+bakım önceki ölçümden korunur, RAM/disk ve
concurrent/cold performansı yeniden ölçülmedi.
[Son kontrol ve sınırlar](INTERLEAVED-SEARCH-10K-20261016.md).

Önceki teşhis: sekiz sayfada tüm query iş sayaçları birebir aynı ve yazma gram
çıkarımı0 çağrı. Aynı program/aynı CPU kontrolünde ishmael p95 +%4,23;
farklı-sürüm yeni aynı-CPU testinde +%3,45, quixote +%9,64. Ortam
kararsızlığı kanıtlandı; ilk farkın tamamı kod regresyonu veya tamamen
gürültü diye kapatılmadı. Kod/yerleşim katkısı ve genel kabul kapısı açık.
[Son teşhis ve araçlar](SEARCH-ATTRIBUTION-10K-20261016.md).

Önceki yazma adımının yoğun sayfa kontrolü:800 örnek/sayfa,1 düşük/7 yüksek
p95; ishmael /130/1 +%4,87 ve4/4 yüksek, quixote /28/10 +%4,03 ve3/4
yüksek. Bunlar dalgalanma diye kapatılmadı. %11,49 yazma+bakım kazancı
deneysel profilde tutulur; genel/default kabul kapısı geçmedi.
[Yoğun kontrol ve sınırlar](GRAM-BODY-WRITES-10K-20261016.md).

| İş | Durum / kapanış kanıtı |
|---|---|
| CRC ve payload/UTF-8 geçişlerini yakın parçalarla yürüt | Reddedildi. Aynı10k/400 örnekte holmes p95+%1,80/3 çift yüksek, zx+%0,32/4 çift yüksek; tipik blok da yavaş. Kontroller korunmuştu, runtime geri alındı; private unsafe helper yalnız arşivde. Yeni38-case/RAM sonucu yok; chunk ayar turu açılmayacak. Pozitif sonuçta aynı belgenin tekrar okunması ayrı, uygulanmamış hipotez. [Ölçüm/patch](VALIDATION-CHUNK-10K-20261018.md) |
| Gruplamayı hızlı sorgu yolundan ayır | İkinci prototip reddedildi; aynı10k/400 örnekte darcy -%2,74, holmes +%0,98/2 çift düşük, the/200 -%2,98. the/200 artışı bu koşuda yok; hedefte tekrarlanan p95 kazancı yok. Eski döngüler byte-eşit korunmuştu, runtime geri alındı. Yeni38-case/RAM koşusu yapılmadı. Gruplama için yeni ayar turu yok; genel arama ve diğer performans işleri açık. [Kanıt/patch](PAYLOAD-ISOLATED-10K-20261017.md) |
| Bilinen payload adaylarının yakın okumalarını birleştir | 2026-10-17 ilk prototip reddedildi; runtime 6eb3a89'a döndü. 19/38 senaryoda read azaldı, holmes 1285→1106 ve p95 -%9,18; the/200 ilk +%10,38, yoğun tekrarda +%6,02. A/A oynaklığı kod payını sıfırlamaz. Ayrı query/get private +46KiB/WS +72KiB medyan, disk aynı. Kuyruk/planlamayı hızlı döngüden ayırma henüz ölçülmüş çözüm değil; genel arama, yazma/bakım, Vec/genel RAM, açılış, sıkıştırma ve eşzamanlı/soğuk yük açık. [Patch ve kanıt](PAYLOAD-BATCH-10K-20261017.md) |
| Aynı temiz veride arama gerilemelerini ayır, en pahalı aşamayı azalt | Dar kısa-ASCII adımı tamam: body fold263.627→0; iki negatifte normal p95 -%31,21/-%33,99 (4/4 süreç,32/32 blok). Tüm38-case28/10; A/A36/38 tail artışı nedeniyle genel p95 kapısı açık. Diğer36 senaryoda izlenen iş sayıları aynı; dar/uzun kelimelerde payload okuma ve sonuç hazırlama hâlâ maliyetli. Query peak medyanları artmadı, disk349.805.698 B/18 dosya aynı. Yazma/bakım, Vec/genel RAM, açılış, sıkıştırma ve concurrent/cold kapanmadı. [Son ölçüm/kanıtlar](SEARCH-STAGES-10K-20261017.md) |
| JSON sözlük ve indeksteki267,8MB belge kopyası | SEG05 flag1 kullanıcı kararıyla native default yazıcıya taşındı: commit/flush, merge ve cache onarımı doğrudan kaynak adresi yazar, payload kopyası üretmez. Eski okunabilir SEG03/04 korunur; kaynak/WAL/manifest/fixture aynı. Varsayılan ve referans/all-feature testleri, kesilme/bozulma ve builder/repack kill kontrolleri geçti. Yeni4 çiftte write+bakım-%9,86 ve open-%34,79; canlı idx353,7→81,9MB, retained core1,521GB→972,7MB. Yeni38-case16/22; write private +%1,42/4 çift yüksek, query private düşük. Önceki yoğun sayfa artışları da açık.72 kaynak/ready ve prototip wire karşılaştırmasında122 core dosya hash-eşit. Yazma/kabul tekrarları, RAM/disk-retention ve concurrent/cold açık. [Güncel geçiş/ölçüm](DEFAULT-SEG05-10K-20261017.md), [ilk prototip](COMPACT-INDEX-10K-20261017.md) |
| İndeks açılışında checksum okuma ve hazırlığı azalt | Dar okuma adımı ölçüldü: indekse özel max1MiB exact reads; bütün indeks CRC'si ve diğer doğrulamalar tam, kaynak/WAL ve sözlük algoritmaları aynı. İki4-çift seride ilk kullanım -%4,20/-6,18;8/8 hızlı. Yalnız open kazancı oturumlara bağlı%1–8. Yoğun8 sayfanın p95'i düşük; genel arama kapısı değil. Ayrı query/get tepesinde artış yok; Vec ve concurrent RAM ölçülmedi. COW/reuse prototipi %5–10 arama artışıyla geri alındı. Hazırlık maliyeti ve genel açılış işi açık. [Son rapor](INDEX-OPEN-10K-20261017.md) |
| Tam doğrulamalı açılışta kaynak tarama ve belge hazırlığını azalt | Dar kaynak adımı ölçüldü:4 dengeli normal release çiftinde430,303→270,116ms (-%37,23;4/4), pending0; ilk kullanım toplamı423,924→281,714ms. Frame+source CRC, UTF-8/schema/key/link/version/sınırlar ve source lease sürer. Sağlam cache yoksa önceki builder yolu, WAL/0.1 kaynaklar aynı. Query/get peak private9,197→8,637MiB; bu Vec ingest veya toplam sistem RAM'i değildir. Son38-case23 düşük/15 yüksek; genel arama garantisi yok. İndeks CRC/directory maliyeti ve genel açılış işi açık; SQLite normal open seviyesine ulaşılmadı. [Rapor ve ham kanıt](STRICT-OPEN-10K-20261017.md) |
| Varsayılan FDB'yi SQLite ile aynı10k'da yeniden karşılaştır | Tamamlandı, kapsam sınırlı:4 dengeli streamed ve2 Vec yazma çifti;38-case ve256 get için4 dengeli okuma çifti;4 opening ve2 oracle'sız query/idle memory çifti. Streamed toplam23,598/34,509s;38/38 p95 ve bütün durumlarda4/4 FDB hızlı. Query private9,189/12,740MiB; normal Vec78,912/8,402MiB, strict open433,686/1,535ms ve retained disk2,61 kat. İlk iki SQLite write süresi yüksekliği ayrıştırılmadı; SQL prepared cache64, FTS5/LIKE/native3.53.2; validator3.53.1. Kaynak/fixture/corpus/hash/10k payload/FTS integrity aynı. Motor, storage, protokol değişmedi; eski/yeni arama regresyonları ve genel kapılar bu kıyasla kapanmaz. [Rapor](SQLITE-DEFAULT-COMPARISON-10K-20261016.md) |
| Kabul edilen birleşimi normal derlemeye taşı ve yerelde sabitle | Tamamlandı (dar kapsam): checkpoint42116f7; bounded-engine default, eski feature alias'ı ve no-default reference korunur. Runtime algoritmaları/bağımlılıklar/kalıcı biçim/protokol aynı; bağımsız rkyv/WAL-delta/shared-documents/sync-off deneyleri kapalı.151/92/170 test, Clippy/fmt, eski alias ve comparator check, varsayılan fdb.exe stats/get/search geçti.2 çift Vec,4 çift streamed+2 SQLite referansı,2 çift A/A ve2 çift tekrar,38-case/yoğun sayfa/açılış/idle ölçüldü. İlk streamed+%4,88 tekrar-%0,87; broad24 düşük/14 yüksek ve pages5/3. Kalan hız artışları kapatılmadı; genel hız/Vec-query RAM/disk/eşzamanlı/cold kapıları açık.122 dosya16 streamed ingestte ve önceki profille byte-eşit. Yeni corpus/DB kopyası yok; yalnız sabit slotlar sıfırlandı. [Sonuç/karar](DEFAULT-PROFILE-VALIDATION-10K-20261016.md) |
| Kısa bloklarla eski/yeni aramayı ve etiket etkisini sınama | Ölçüldü:4 süreç çifti×8 blok×25 tur,800 örnek/sayfa/variant/seri; ilk A/B ve ters etiketli BA8/8 yeni p95 düşük. Ishmael /130/1 -%6,51 / -%3,05 (4/4 ve3/4 hızlı), quixote /28/10 -%4,39 / -%2,45 (3/4 ve2/4). A/A6 düşük/2 yüksek, the /0/1 +%6,44/4 çift yüksek fakat blok medyanı-%1,01; tek p95 yönü kod nedeni ilan edilmez. Kilit/CRC/kaynak/format/default ve motor aynı; yeni corpus/reset/DB kopyası yok,139 dosya hash'i aynı. Bu kısa-oturum/codegen düzeninde gerileme tekrarlanmadı; kesintisiz oturumun yerleşim/ortam katkısı ve genel kabul kapısı açık. Yazma/bakım, query/normal API RAM'i, disk ve concurrent/cold işleri korunur.8 example testi/fmt/Clippy geçti. [Kanıt/sınırlar](INTERLEAVED-SEARCH-10K-20261016.md) |
| Yazma sonrası arama gerilemesinin nedenini ayır | İncelendi, tam kapanmadı. Normal dört-çift aynı-CPU A/B8 yüksek p95; A/A2 düşük/6 yüksek, aynı kodda ishmael +%4,23. İlk A/A son çift bütün sayfalarda+%44,79…54,91. Profilde sekiz sayfa/1.920 doğrulamanın aday/read/byte/CRC/doğrulama/ham-kanıt sayıları iki sürümde aynı; ek body fold/scan ve write gram çağrısı0. Normal yeniden derlemeler eski measured kontrol/aday `.text`'leriyle eşleşir; sorgu kaynakları aynı ama machine code/yerleşim aynı değil. Motor aramasına tahmine dayalı düzeltme yapılmadı; kalan quixote ve kod/yerleşim katkısı açık. %11,49 yazma+bakım kazancı önceki ölçümdür, yeniden ölçülmedi. Yeni RAM/disk kazancı veya concurrent/cold sonucu yok. [Kanıt/sınırlar](SEARCH-ATTRIBUTION-10K-20261016.md) |
| Uzun gövdede indeks parçası üretim CPU maliyetini azalt | Dar kazanç uygulandı/ölçüldü: son body-only sürüm, aynı10k/dört dengeli çift, yazma30,859→27,290s; yazma+bakım31,908→28,241s (-%11,49),4/4 hızlı, çift farkları-%10,81…11,93. CPU-%11,86/4/4. Son bakım0,988→0,940s,3/4 düşük; ayrı merge optimizasyonu sayılmaz. Geçici8KiB tablo/63-bit scalar keys yalnız BodyCache'te, query grams eski haliyle aynı. Yeni format/feature yok,122 core dosyanın byte'ı sekiz koşuda aynı. OS private commit9,021→9,107MiB (+88KiB),WS12,986→13,014MiB (+28KiB); sıfır RAM artışı şartı kanıtlanmadı. Read matrix33 düşük/5 yüksek; genel/default kapısı açık, feature kapalı. Payload tekrar çöz/kodla, merge tamponları, kalan arama artışları, query/normal API RAM'i, disk/eski nesil ve concurrent/cold açık. SQLite önceki adımın iki unpaired referansı, son FDB'yle paired gate değil. [Sonuç/kanıtlar](GRAM-BODY-WRITES-10K-20261016.md) |
| Dar aramada gereksiz body normalleştirmesini azalt | Dar kazanç uygulandı/ölçüldü: aynı10k/dört dengeli pair/800 örnekte quixote /28/10 -%10,41, /57/50 -%15,04; ishmael /130/1 -%7,28, /60/200 -%4,17; dördü4/4 hızlı. Geniş38-case21 düşük/17 yüksek, AND quixote whale -%23,35/4/4. ASCII raw pozitif kanıt56 adayda815→0 body fold/scan yapar;56 payload read/CRC/schema korunur.64KiB/8 denemede6 başarı sınırı düşük isabetli ek taramayı durdurur; yeni büyük cache/feature/format yok. Builder-budget içinde/default kapalı; genel kabul kapısı geçmedi.17 artış, yazma+bakım önceki+%3 gözlemi, warm query/normal API RAM'i, disk/eski nesil ve concurrent/cold açık. İlk kullanım proses belleği ve ölçüm sınırları raporda; indeksleme RAM'i/yazma yeniden ölçülmedi. [Sonuç/ham kanıtlar](RAW-TEXT-PROOF-10K-20261016.md) |
| Arama artışlarını aynı sürüm kontrolüyle değerlendir | Ölçüm aracı düzeltildi/doğrulandı; motor hız işi açık. Aynı SHA'lı reader üç çiftte8/8 pooled p95 yüksek (+%0,31–6,39); bunlar kod regresyonu olamaz. Dengeli dört çift/800 örnekte aynı kod3 düşük/5 yüksek (-%1,79…+%2,94); sıfır fark veya otomatik tolerans iddiası yok. Runner artık dengeli4 çift, açık candidate, sıra/hash tasarımı ve örnek doğrulaması; özet paired fark/medyanını da verir ve eski logları okur. Dar sorguya adaptif pencere prototipi hedefte56→56 read/0→0 window hit,38-case9 düşük/29 yüksek; reddedildi ve runtime geri alındı. Source lease korunur; önceki24 artış, yazma/bakım, RAM/disk ve concurrent/cold işler açık. Yeni corpus/DB reset/RAM/write/SQLite performans ölçümü yok. [Kanıt ve sınırlar](BENCHMARK-CONTROL-10K-20261015.md) |
| Tek-sonuç sayfasının guard maliyetini azalt | Dar adım uygulandı/ölçüldü: Windows/plain-path immutable source doğrulamadan önce read-share ile açılır, aynı handle index sahibi/merge worker ömrünce tutulur; write/truncate/delete/rename engellenir. WAL/Legacy/reparse/non-Windows eski guard'da; index guard ve ilk10k source doğrulaması kalır. Aynı10k/üç pair/600 örnekte the /0/1 p95 0,8123→0,2459ms (-%69,73), ileri5000/9950 -%18,27/-%20,34; üçü3/3 hızlı. Geniş38-case14 düşük/24 yüksek; the /0/1 burada da -%72,18/3/3. Üç yoğun-sayfa artışı, diğer matrix artışları ve önceki+%66,89 farkın tam nedensel açıklaması açık. Ayrı instrumented guard0,3663→0,0357ms, source path çağrı5→0/index5→5. Genel kapı geçmedi/default kapalı; RAM/write sonuçları son raporda. Query tepe/normal API, yazma-bakım, RAM/disk, uzun tarama ve concurrent/cold yük açık. [Son kanıt/sınırlar](SOURCE-LEASE-10K-20261015.md), [ilk teşhis/reddedilen deneyler](PAGE-GUARD-10K-20261015.md) |
| Değişmeyen Unicode işaretlerinde normalleştirme aralığını koru | Dar kazanç ölçüldü: mevcut10k/üç çift matrix35 düşük/3 yüksek; zx309→222ms, the zx307→236ms, quixote whale5,83→5,00ms, üçü3/3 hızlı. Query normalizer'ı aynı byte sonucunu daha az aralıkla kopyalar; ek buffer/cache/format/feature yok, builder-budget içinde. Üç çift indexing private9,13→9,18MiB, bakım7,54→8,48MiB, WS12,84→13,21MiB; disk/hash aynı. Toplam32,34→32,03s,2/3 hızlı; genel yazma kazancı değildir. Tek-sonuç the sayfası+%66,89/0,5843ms,0/3 hızlı; diğer iki p95 artışı da açık. Genel RAM/hız kapısı geçmedi/default'a geçiş reddedildi. Query tepe/normal API/genel heap, kalan normalleştirme-tarama, yazma/bakım, RAM/disk ve concurrent/cold yük açık. [Kanıt/sınırlar](CHANGED-RUNS-10K-20261015.md) |
| Metin parçalarının ikinci kopyasını azalt | İki deneme reddedildi ve runtime değişiklikleri geri alındı (2026-10-15). Aynı mevcut10k/üçer pair doğrudan fold32 düşük/6 yüksek; tek residual varyant21/17. quixote whale her iki batch'te0/3 hızlı (+%6,14/+%10,58). İlk the zx3/3 kazancı genel kapıya yetmedi. Prototype Unicode/chunk/reuse ve final default/feature/all-features all-targets, cut/byte/kill/interop tests, Clippy/fmt ve38-case check geçti. Normal release önceki kodla yeniden build edildi. DB yazması/RAM checkpoint yok; önceki RAM sayıları yeni ölçüm değil. Corpus/FDB/cache/SQL hash/format aynı. CPU normalleştirme/tarama, mevcut arama artışları, yazma/bakım, RAM/disk ve eşzamanlı yük açık. [Reddedilme kanıtı](CHUNK-COPY-REJECTED-10K-20261015.md) |
| Uzun/olumsuz taramada normalleştirmeyi hızlandır | İlk dar adım ölçüldü: builder-budget içinde güvenli32-bayt ASCII-prefix kontrolü; Unicode/fold/source/format aynı. Aynı10k/3 pair zx336→309ms/the zx337→317ms,3/3 hızlı; matrix33 düşük/5 yüksek. Default geçişi reddedildi, beş yeni artış açık. Tek RAM/write teyidi private9,14MiB/WS13,34MiB; bakım+0,66MiB, toplam32,36→33,47s; disk aynı. Tek instrumented the zx normalleştirme135,81ms/scan20,84ms; normalizer payı hâlâ büyük, ayrı paired normalizer oranı iddia edilmez. Query tepe ve1000 aktif performansı tekrar ölçülmedi. Tests/Clippy/fmt geçti. Kalan normalleştirme/tarama, chunk kopyaları, genel yazma/bakım/RAM/disk/eşzamanlı yük açık. [Kanıt/sınırlar](WIDE-FOLD-10K-20261014.md) |
| İki karakterli olumlu aramada genel indeks kanıtı | Dar adım ölçüldü: tam-live gramdan scoped alt-parça kanıtı, 64 entry/16 KiB/run; aynı 10k/3 pair he132,27→1,81ms,3/3 hızlı. Profilde candidate payload read0; source kontrolü/hit hydration korunur. Yazma+bakım32,87→32,25s,3/3 düşük; indexing private9,09MiB (+4KiB medyan), disk aynı, WS+0,30MiB. Genel matrix15 düşük/23 yüksek; kanıtsız/olumsuz iki karakter ve uzun parça taraması, diğer p95 artışları açık. Default kapısı başarısız. Default/feature/all-features tests, cut/byte/kill/interop, Clippy/fmt ve son38-case check geçti. Query tepe/1000 aktif belge performansı bu adımda ölçülmedi. [Kanıt/sınırlar](SHORT-COVERAGE-10K-20261014.md) |
| Kesin gram kanıtıyla payload tekrarını azalt | Dar adım ölçüldü: genel3 karakterli parçalar, early-out öncesi source izni/fingerprint guard ve mevcut posting-count. Son3 pair the145→2,30ms/man147→2,63ms; offset5000/9950 önceki büyük fark3/3 çözüldü. Geniş matrix15/38 p95 düşük,23 yüksek; aktif-data17/18 düşük,1 yüksek. Diğer uzun/2 karakterli tarama ve yeni p95 artışları açık. Son tek RAM teyidi private9,19MiB/WS13,32MiB; disk aynı. Önceki3 pair bakım+0,40MiB/yazma+bakım33,22→33,79s, hız işi kapanmadı. İlk source kontrolü228ms search'ün225ms'si; sıfırlanmadı. Default kapısı başarısız/feature aynı. [Kanıt/sınırlar](COVERED-TEXT-10K-20261014.md) |
| Düşük RAM profilinde hızı geri kazan | İlk adım geçmiş kanıtı: borrowed record cursor, genel tüm-gram preflight, mevcut1MiB içinde64KiB hot posting cache. İlk üç paired aktif-data15/18/geniş36/38 p95 düşük;10k indexing private9,07→9,09MiB, disk aynı/bakım+0,46MiB. Yazma+bakım40,69→40,16s (2/3 hızlı) bir hız kapısı değildi. O adımın the ileri offset5000/9950 farkı sonraki covered ölçümünde3/3 çözüldü; güncel23/38 p95 artışı ve iki karakter/uzun tarama işi üst satırdadır. Toplam hız hedefi/default geçiş açık. [İlk adım kanıtı](BOUNDED-SPEED-10K-20261014.md), [son durum](COVERED-TEXT-10K-20261014.md) |
| Giriş/delta/bakımı ortak bütçeye al, disk sözlüğünü sayfalardan oku | Uygulandı/ölçüldü; `experimental-builder-budget`+`commit_streamed`, default kapalı. Corrected3 çiftte indexing111,13→9,13MiB,bakım100,11→7,94MiB,idle21,48→1,74MiB; WS114,73→12,92MiB. SQL3 ayrı referans indexing8,40MiB. Toplam27,24→31,54s (3/3 yavaş), immutable matrix17/21 vewrite-after1000 belge0/18 p95: default hız kapısı reddedildi. Genel Bloom/AND/cache/incremental gram admission var; cold-cache/budget deadlock'u düzeltildi, tam suite/mixed writer/cut/kill/old-reader kontrolleri geçti. Normal API/global heap hedefi kapanmadı. [Kanıt/sınırlar](BOUNDED-MEMORY-10K-20261014.md) |
| SQLite / FDB RAM farkının kök nedenini önce teşhis et | Teşhis raporlandı; RAM çözümü açık. Eski tek allocation snapshot'ında belge27,35MiB+delta indeks29,48MiB+metadata12,51MiB; güncel live5 run103.667 terim girdisi.16MiB payload eşiği total RAM değil ve bu corpus'ta~26MiB'ye aşılır.500 owned girişin yalnız title/text'i12,20–13,08MiB; SQLite aynı transaction'ı tek belgeyle besliyor. Genel field/link indeks kapsamı da farklı. Yeni performans/uygulama iddiası yok. [Kanıt ve sınırlar](ROOT-CAUSE-MEMORY-10K-20261014.md) |
| Tam belge/folded metin çift kopyasını WAL referansıyla azalt | Ölçüldü, hız şartı nedeniyle reddedildi/default kapalı: indexing111→84MiB,bakım102→76MiB; toplam31,09→31,89s,2/3 yavaş. Delta arama1/18 p95 düşük,pozitif16 case3/3 yavaş. Tek-frame encode ve harness row/resident ayrımı düzeltildi; kalıcı codec aynı. Hızlı tek temsilli belge tasarımı açık. [Kanıt](COPY-REDUCTION-10K-20261013.md) |
| Mevcut builder/write bütçeli kombinasyonu ölç | Ölçüldü ve hızdan ödün vermeyen çözüm olarak reddedildi: üç çift, toplam35,17→38,59s (3/3 yavaş), bakım private99,86→96,07MiB.256KiB/2MiB pool'ları total RSS sınırı değil; peak reservation64KiB/1MiB, waits0. Default kapalı kaldı. Gerçek delta/body/caller/metadata bütçesi açık. [Kanıt](BUDGET-MEASURE-10K-20261013.md) |
| Delta RAM gram sözlüğündeki scope/kopya tekrarını azalt | Dar adım ölçüldü: nested scope sözlüğü,3/3 pair toplam hızlı ve indexing/bakım private düşük. Medyan toplam42,995→36,803s, indexing111,75→110,36MiB, bakım100,54→97,58MiB; disk aynı. Delta alt örnek arama17/18 p95 düşük,tek+14µs fark kayıtlı. Belge/body kopyaları hâlâ açık. [Kanıt](DELTA-DICTIONARY-10K-20261013.md) |
| Çok sayıda küçük payload okumasını birleştir | Adaptive kullanım kontrolü tamamlandı; presumption overread%22→%0,78. Yeni38-case36 düşük/2 yüksek; önceki11'in tümü düşük. Kalan artışlar2026-10-13 kullanıcı kararıyla ölçüm dalgalanması olarak kabul edildi. Grouped-read default kapalı kalır. [Kanıt ve kabul](REGRESSION-READS-10K-20261013.md) |
| Normalleştirmede ASCII aralığı ve query token maliyetini azalt | Güvenli8-byte prefix araması ve query fold_into eklendi; tüm Unicode scalar/hizalama/chunk testleri geçti. Aynı grouped features ile11 önceki case p95%4–18 düşük. Reusable finder deneyi17/21 sonucu ile reddedildi ve kaldırıldı. [Kanıt](REGRESSION-READS-10K-20261013.md) |
| OFF1 UTF-8 doğrulama maliyetini düşür | Dar adım ölçüldü: first208→173 ms,5/5 pair hızlı; dört warm query p95%18–27 düşük,12/12 pair hızlı. WS first-use~30 MiB; sampled proses ölçümü, geniş RAM kapısı değil. Strict ilk5 median%3,73 arttı, ek5 karışık; genel opening işi kapanmadı. [Kanıt](UTF8-READS-10K-20261013.md) |
| Kaynak frame tamponunu reuse/pre-plan ile değiştir | Reddedildi ve runtime değişiklikleri geri alındı. İlk deneme RAM+8 MiB; ikinci deneme ilk search hızlı fakat bütün warm p95 eşleşmeleri yavaş. [Kanıt](FRAME-PLAN-10K-20261013.md). Reuse kaldırıldı; bounded multi-frame testi ve proses bellek gözlem aracı korundu |
| Ortak IEEE checksum aritmetiğini hızlandır | Bu dar adım ölçüldü: ilk search381→229 ms, strict open962→582 ms;5/5 eşleşme hızlı. Dört warm query p95 toplam12/12 eşleşmede düşük. CRC/formatlar ve DB hash'leri aynı. [Kanıt](CRC-READS-10K-20261013.md). RAM/yazma ve38-query tekrarlı performans kapsamı açık |
| İlk deferred aramada kaynak belge kopyalama maliyetini kaldır | Bu dar adım ölçüldü: aynı record-read API ve5 pending source ile5/5 çiftte ilk search529→336 ms (%36,44); open+get+search602→407 ms. [Kanıt](BORROWED-SOURCE-10K-20261013.md). Bütün kaynak kontrolleri korunur; CRC/read ve genel açılış işi açık |
| Açılış CRC / source replay / index maliyetini ölç ve tekrarları azalt | Strict ilk adım946→747 ms. Opt-in deferred ikinci adım:752→319 ms, ilk get92 /ilk search360 ms; [kanıt](DEFERRED-OPEN-10K-20261012.md). VFY blok kontrolü298→60 ms open,383→150 ms open+get; [kanıt](BLOCK-OPEN-10K-20261012.md). Record-get ilk107→0,20 ms; [kanıt](SOURCE-RECORD-READS-10K-20261013.md). Warm search%1–4 arttı, opt-in/default değil. SQLite~9 ms seviyesine ulaşılmadı; kaynak search kökeni ve directory maliyeti açık |
| Güncel kodda yazma + flush/merge bakımını SQLite ile yeniden ölç | 2026-10-13 merge+bitmap profili karşılaştırıldı: FDB35,81+1,92=37,73s / SQLite44,95+1,46=46,41s; FDB2/3 çift hızlı, bakım SQLite hızlı. Bu tarihli sonuç korunur. Güncel source-lease builder-budget/streamed profili üç çiftte kontrol26,667+1,007 / yeni27,445+0,991s; toplam medyan27,609→28,436s (+%3,00),1/3 hızlı; bakım2/3 hızlı. Güncel profilin SQLite yazma eşleştirmesi bu adımda yapılmadı; genel yazma/bakım işi açık. [Güncel kontrol](SOURCE-LEASE-10K-20261015.md), [SQLite ile önceki profil](WRITE-MAINTENANCE-10K-20261013.md) |
| Bütün temel test başlıklarını birlikte raporla | Açık: yazma, bakım, get,38 arama/sayfa, açılış, RAM ve disk |
| Arama senaryolarındaki p95 artışını incele | Kapalı —2026-10-13 kullanıcı kararıyla kalan grouped2/default3 artış ölçüm dalgalanması kabul edildi. Ham36/2 ve35/3 sonuçlar korunur; ek istatistiksel kanıt veya bütün iş yüklerinde hız garantisi değildir. [Ölçüm ve kullanıcı kabulü](REGRESSION-READS-10K-20261013.md) |
| RAM: boşta ve indeksleme/bakım tepesini düşür | Bu10k streamed/sayfalı profilin düşük RAM sonucu2026-10-14 kullanıcı tarafından kabul edildi; öncelik bu seviyede hız. Son source-lease3 çift indexing private9,195→9,129MiB ve bakım8,457→8,145MiB, ikisi3/3 düşük; idle1,695→1,527MiB,2/3 düşük. OS peak WS13,000→13,027MiB,2/3 yüksek; sıfır RAM artışı iddiası yok. Bunlar arama/oracle olmayan ayrı proses sayaçları; query tepe belleği, kernel handle belleği ve toplam sistem RAM'i değildir. Normal/default API, genel heap, büyük entry, WAL replay ve concurrent kapsamı açık; düşük RAM kabulü default'a geçiş değildir. [Son üç çift](SOURCE-LEASE-10K-20261015.md), [önceki üç çift](CHANGED-RUNS-10K-20261015.md), [kök neden/uygulama](BOUNDED-MEMORY-10K-20261014.md) |
| Disk: source/index kopyaları ve tutulan eski nesiller | Standart eski nesil temizliği tamam:105 dosya/622.859.277 B, retained core972.664.975→349.805.698 B; source+WAL267.900.709 B, idx+MET81.904.473 B, ready516 B. Canlı byte/hash aynı,10k full-doc ve38-case oracle geçti; eski okuyucu da açar. İki streamed ingestte aynı350MB; current17 core hash-eşit. GC güvenliği, root/source/WAL bozulması, interrupted unlink, explicit/automatic kill, reader/worker pin ve retry testleri geçti. Genel sıkıştırma, dynamic temp/orphan sınırları, concurrent/cold ve genel RAM/hız kapıları açık. [Güncel ölçüm ve sınırlar](RECLAIM-10K-20261017.md) |
| Bakım sürerken okuma/yazma gecikmesini ölç | Açık; eşzamanlı yük ve bloklanma kapsamlı ölçülmedi |
| Soğuk cache ve farklı iş yüklerinde sonuçları doğrula | Açık; mevcut karşılaştırmalar sıcak OS cache, belirli SQLite ayarları |

Bağımsız Rust motoru; SDB/Fundops/Frunner entegrasyonu kapsam dışı.
0.1 fixture korunur; `.fdb/.ready` kaynak, `.idx` cache. Valid cache bozuk kaynağı
gizleyemez. Yeni format gerekirse sürüm/uyumluluk testleri şarttır.


29 Ekim profil devamı açık notları:100lü arama kapısı ve162,41MiB4R process tepe;100lü max2,67sn aşama nedeni henüz ölçülmedi. Büyük yeni tekli uçlarda bağımsız WAL File::open çağrısı140–215ms ölçüldü; eski74,84/125–200ms örneklerinin aynı nedenle olduğu bilinmiyor. Saf scheduler ve fiziksel OS cache miss payı ölçülmedi.1–2 işçi sonuçları karışık; varsayılan ayar değişmedi. İlk idle CLI unsupported-mode hatası, amaçlı SQL iptali ve önceki envanter-audit gözlem açığı korunuyor.
