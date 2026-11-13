# Blok 4 — kullanıcı kararı için tablo, 4 Kasım 2026

12 Kasım güncellemesi: 4b için aşağıdaki tarihsel önerinin ardından
kullanıcı arka planda revizyon geri kazanımını varsayılan olarak açtı.
[Güncel karar ve doğrulama](BACKGROUND-RECLAIM-DEFAULT-50K-20261112.md)

3 Kasım'da başlayan belge incelemesi 4 Kasım'da tamamlandı.

**Yalnız belge/kod okuması ve karar önerisi. Kod, varsayılan, DB veya ölçüm
değişikliği yok.** Bu tablo uygulama onayı değildir. Kaynakta iki setter da
constructor'da KAPALI; normal senkron WAL açıktır. Seçenekler runtime'dır,
DB'ye kalıcı servis tercihi yazmaz. Kabul edilmiş try/seri arama varsayılanı
bu kararlardan ayrıdır. [Brife göre durum](BRIEF-BLOCK-STATUS-20261103.md).

| Madde | Mevcut kanıt | Artı / eksi | Eksik doğrulama | Önerim / karar durumu |
|---|---|---|---|---|
| **4a — `set_streamed_wal_layer`** | [Küçük commit](SMALL-COMMIT-DIRTY-OPEN-10K-20261026.md), [10k sağlamlaştırma](WRITE-HARDENING-GATES-10K-20261027.md), [50k tekli](WRITE-PROFILE-50K-20261029.md), [dirty500](WRITE-FOLLOWUP-50K-20261029.md): küçük synced commit ve düşük RAM recovery doğrulandı; dirty500 ≈291–292 ms /4,32–4,44 MiB. Oracle/kill/eski okuyucu kanıtları var. | Önceki worker'ı beklemeden WAL-adresli görünür katman; özgün metin delta'da tutulmaz. Buna karşılık admission dışı büyük gruplar fallback'e gider; aktif katman arama maliyeti, flush/install ve replay bedeli vardır. 4R/100'lü uçlar bu tabloda çözülmüş sayılmaz. | Güncel ikiliyle 4R+100'lü ve500 drain/2,6 s uç eksik; global default ON/OFF eşlenmiş kapsam testi yok. Float signed-zero owned/light tutarsızlığı açık. Kabul edilmiş görünürlük/normal41 KALDI kayıtları sürer. | **Genel default şimdilik KAPALI kalsın. Frunner'ın küçük akışlı metin/key/version yükünde açıkça ON seçimi güçlü adaydır.** Genel ON için aşağıdaki eksikler ve kullanıcı onayı gerekir; bu tur seçim yapılmadı. |
| **4b — revizyon geri kazanma** | Reclaim kapalı ilk churn'de350→876 MB /2,501×; canlı segment içi ölü revizyon. [Maliyet](REVISION-RECLAIM-COST-10K-20261026.md):10tur≈13,5–13,95 s, eski≤12 s kapısı KALDI ama kullanıcı kabul etti; disk1,20×. [1c/1d](RECLAIM-IDLE30-10K-20261027.md):30tur bakım sonrası max1,199×/son1,107×, ölü payload son29,75 MB; oracle/kill/pin geçti. | Sürekli update/delete'de gerçek disk geri kazanımı ve tombstone azaltma. Bedeli kaynak/index rewrite, I/O/CPU ve bakım zamanı; snapshot pin'i eski dosyaları tutar. Tek okuyucu bu iki sorunu ortadan kaldırmaz. | Frunner'ın ters-mtime/uzun yol ve2–200 KB dağılımında tek okuyucu+uzun süreli güncelleme için disk tavanı gösterilmedi. Uzun snapshot2c yok; sürekli yazıda bakım ilerlemesi yalnız ölçülen idle protokolünde doğrulandı. | **Mevcut genel KAPALI kararı korunsun.** Canlı güncellenen Frunner indeksinde geri kazanma stratejisi gerekir; “kapalı ve sonsuza kadar sınırlı disk” iddiası yapılamaz. İhtiyaç/limit ayrımı aşağıda; uygulama profili için yeni kullanıcı kararı gerekir. |
| **4c — Owned/Vec** | [Churn](CHURN-CONCURRENT-10K-20261025.md) ve [toplu bellek profili](OWNED-MEMORY-10K-20261020.md):80–292 MB süreç tepe örnekleri; üst uç reddedilen büyük istek/fallback'i de içerir. Eski erken-flush+fold adayı≈80,10→66,75 MiB fakat6 delta sorgusunda geriledi ve reddedildi. [Akışlı motor-only50k](ENGINE-ONLY-RAM-50K-20261030.md) ≈14,9–15,5 MiB ingest /2,25–2,30 MiB idle; aynı API/caller ölçümü değildir. | Owned API hazır Vec ve mevcut büyük/atomik caller sözleşmesini korur. Caller input + özgün/folded delta + indeks üst üste binebilir. Akışlı API, caller da gerçekten akışlıysa bu örtüşmeyi sınırlar. | Güncel motorla owned allocation/OS profili ve başarılı/reddedilen istek ayrımı yok. Azaltmanın imkânsızlığı gösterilmedi. “Frunner yalnız akışlı” etiketinden önce giriş yolu ve atomiklik/hata kontratı doğrulanmalı. | **Owned API korunsun; Frunner için yalnız akışlı profil önerilsin.** Bunu teknik imkânsızlık kararı diye sunmayalım. API belgesine eklenecek işaret aşağıda öneridir; kullanıcı onayı ve belirtilen doğrulamalar beklenir. |

## 4a: güncel sınır ve kanıtın kapsamı

Kaynak okuması: [setter](../src/engine/light.rs),
[admission/fallback](../src/engine/streamed.rs), [arka plan prefix](../src/engine/light_flush.rs).
Güncel hızlı dal staged row≤128, staged byte≤8 MiB, shadow+staged row≤2048,
WAL+stage≤64 MiB ve `needs_flush=false`, owned docs boş koşullarını arar.
Shadow≥256 veya WAL≥8 MiB olduğunda prefix worker başlatabilir. Bunlar
güncel kod koşullarıdır; eski rapordaki512/16 MiB sınırı güncel sınır diye
aktarılmaz. Genel atomik/frame/256 KiB entry/admission kontrolleri ayrıca sürer.
100 belge×200 KB bu küçük dala sığmaz; “100'lü her zaman küçük commit” değildir.
Setter'ı kapatmak bekleyen prefix worker'ını bekletebilir; anlık, bedelsiz bir
iptal anahtarı olduğu söylenmez. WAL kapatma/fsync azaltma önerilmiyor.

10k'daki513. işlem ve uzun tekli kapıları kullanıcıca kapatıldı. 50k tekli
p95≤20/p99≤50/max≤100 ms kapısı kabul edildi;74,84 ms ucun nedeni bilinmiyor.
Normal41, Frunner görünürlük ve geniş tarama/CPU/RAM bedelleri farklı işlerin
birleşik sonucudur: bunların tümü küçük WAL katmanına neden olarak atanamaz.
[Ek-TC](TERM-CACHE-50K-20261102.md) K kolunda uzun tekli geçti, görünürlük ve
normal41 KALDI; önceki kullanıcı kabulleri ham kapıları değiştirmedi.

Genel ON kararından önce gerekli liste: aynı ikilide setter OFF/ON; küçük
update/delete/insert ve atomik mtime delete+insert; admission sınırı/fallback,
dirty recovery, normal/first/owned interop, float filtre semantiği;
oracle/CRC/kill/eski okuyucu ve motor-only RAM. Önceden tanımlı performans
kapıları kullanılır; yeni eşik veya bu tur için ölçüm yetkisi verilmez.

## 4b: Frunner tek okuyucusunda ne sınırlıdır, ne zaman gerekir?

**Kanıtlanmış sınır:** sabit canlı10k ve aynı gövde dağılımında, reclaim ON,
her tur20×100 commit, bir `maintain_idle()` ve ardından2 s idle ile30 tur
sonunda/ara bakım sonlarında retained/S0≤1,3 gözlendi. S0 o protokolün bakımı
bitmiş başlangıç source+index/retained boyutudur. 30tur bakım toplamı72,5–74,1 s
(≈2,4 s/tur); çağrı2 s'de bitiyor değildir. 50 ms disk örneklemesi geçici
tepede1,448–1,474× gördü; daha kısa tepeyi kaçırabilir. Bu, eski10tur13,7 s
protokolüyle aynı test veya yeni bir maliyet kazanımı değildir.

**Frunner için bugün koşulsuz disk üst sınırı yok.** Tek okuyucu snapshot'ı
hızlı bıraksa da superseded kayıtlar manifestte hâlâ canlı kaynakların içinde
kalabilir. Uzun süre tutulan tek snapshot bile emekli kaynakları pinler;
2c'nin10 dakika/10tur/≤2× canlı kapısı ölçülmedi. Uzun yol metadata'sı ve
değişen gövde/terim çeşitliliği de kitap10k oranından doğrudan türetilemez.
Dosya sayısı veya meşru canlı içerik büyürse retained/S0 artışı tek başına
çöp değildir; salt eski nesil temizliği ile segment içi ölü alan karıştırılmaz.

Gerekli göstergeler: retained byte, manifest-canlı source/index byte, pinli
emekli byte, segment başına canlı/ölü payload, tombstone, son bakım ilerlemesi.
`live_payload_bytes` kompakte edilmiş DB/source+index boyutu değildir; bu
paydaya aynı1,3 kapısını uygulayarak hedef yeniden tanımlanmaz.

Append-only/az değişen veri için hemen rewrite zorunlu değildir. Sürekli
update/delete ve mtime yeniden anahtarlama ölü alan biriktirdiğinde geri
kazanma gerekir. Mevcut responsive politikanın dead/live>20% bütçe işareti ve
ölü oranı seçimi ihtiyaç sinyalidir, yeni ürün garantisi değildir. Uygun
sessiz arada explicit bakım düşünülebilir; flag açmak bakımın kendiliğinden
tamamlandığı anlamına gelmez. Senkron `maintain_idle()` yazıcı sahibini meşgul
edebilir; snapshot okuyucunun sürmesi, yeni yazının hiç beklememesi garantisi
değildir. Çağıran servis için planlama/iptal/ilerleme sözleşmesi ayrıca gerekir.

[Seçim kodu](../src/engine/revisions.rs) ayrıca segment source≤256 MiB ve
ID-map≤1 MiB gibi uygunluk koşulları taşır; bunlara sığmayan kaynakta politika
disk oranını otomatik sağlayamaz. Tombstone/barrier ve son pin koruması
kaldırılmaz. Frunner profili için opt-in kararından önce gerçek anahtar/gövde
dağılımında tek okuyucu+uzun kullanım, bakım maliyeti, RAM, yazma/görünürlük,
pin/kill/eski okuyucu doğrulaması gerekir; bu belgede yapılmadı.

## 4c: ne azaltılabilir, API işareti için ne doğrulanmalı?

Azaltılabilir alan vardır; ölçülmüş erken-flush adayı bunu gösterdi, fakat
okuma bedeli nedeniyle kabul edilmedi. Önceki profilde aynı anda≈41,13 MiB
özgün/input kökenli veri ve≈28,91 MiB folded/indeks vardı; allocation kökeni
nesnenin hâlâ caller'da olduğu veya sızıntı olduğu anlamına gelmez.
Güncel sürümde aynı dağılım ölçülmüş sayılmaz. Caller'ın çağrıdan önce
kurduğu Vec kendi başına büyükse, API içi düzeltme o sürecin geçmiş tepesini
geri alamaz; keyfi büyüklükte Vec için akışlı60/15 MiB garantisi verilemez.

Önerilen API notu (henüz uygulanmadı):
**“Frunner bu Owned/Vec yolunu kullanmaz; içerik indeksleme ve dosya olayları
için yalnız akışlı yazma API'sini kullanır. Owned/Vec genel API olarak korunur;
akışlı API'nin ölçülen bellek sonuçları bu yola ait değildir.”**

Bu işaretin dürüst olabilmesi için gereken doğrulamalar:

- Gerçek giriş yolu `commit_streamed` olmalı; önceden tam Vec kurup
  `into_iter()` geçirmek veya bütün JSON isteğini belleğe almak akışlı bellek
  kanıtı değildir. Mevcut JSON Lines `commit` hâlâ Vec yoludur; gelecekteki
  Frunner/C ABI bağlantısı bu tur tasarlanmadı veya doğrulanmadı.
- Caller, motor delta/builder/worker, sonuç ve oracle bellekleri ayrılmalı;
  başarılı/reddedilen büyük işlem ve allocator high-water ayrı raporlanmalı.
  80–292 MB/MiB sayaç/protokolleri tek aynı tepe gibi birleştirilmemeli.
- 256 KiB entry, key/index admission,64 MiB atomik frame ve guard/duplicate
  hataları korunmalı. 100–1000 işlem boyut nedeniyle her zaman sığar denmemeli.
  Tekli/dirty, oracle/CRC/kill ve eski okuyucu aynı kontratta geçmeli.
- Parçalı yardımcı **atomik değildir**. Ters-mtime değişimindeki delete+insert
  çifti aynı atomik işlemde kalmalı; helper'ın chunk sınırına bölünmemeli.
  Kısmi başarı/ack listesi ve güvenli retry sözleşmesi görünür olmalı.
- Owned API'yi düşük RAM diye otomatik başka yola yönlendirmek yerine mevcut
  exact/filter/float/version/snapshot semantiği korunmalı. Açık signed-zero
  farkı çözülmüş sayılmamalı; API kapsamı bunu gizlememeli.

Sonuç: karar tablosu hazır, genel varsayılanlar değişmedi. 4b'nin mevcut
KAPALI kararı korunur; Frunner'a özgü disk politikası/API işareti ve 4a genel
ON kararı bu belgeyle kendiliğinden onaylanmaz.
