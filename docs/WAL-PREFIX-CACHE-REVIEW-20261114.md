# WAL prefix yayımı ve önbellek geçerliliği

14 Kasım 2026, adım 1. Kod incelemesi ve mevcut deney kayıtları.
**Kanıt** kaynakta izlenebilen davranışı veya kayıttaki doğrulamayı,
**sinyal** kapsamı sınırlı gözlemi gösterir. **Bilinmiyor** eldeki
kayıtların cevap vermediği noktadır.

## Düzeltmeden önce hangi sonuçlar çıkabilirdi?

Örnek düzen. Bir sorgu eski run'ın maskesini doldurur. Aynı anahtara
WAL'da bir güncelleme ya da silme yazılır. WAL shadow eski run'ı geçici
olarak gizler. Prefix yayımlandıktan sonra anahtar yeni run'dadır ve tail'de
tekrar yazılmadıysa shadow'dan çıkar. Eski kod, hazır maske bulduğunda
sonraki run'ları tekrar taramaz. Eski slot bu noktada yeniden aday olabilir.
[Prefix yayımı](../src/engine/light_flush.rs#L147) ·
[Exact görünürlük](../src/engine.rs#L2161) ·
[Capped görünürlük](../src/engine/first.rs#L421)

| Durum | Düzeltme öncesi sonuç | Kanıt |
| --- | --- | --- |
| Yalnız eski gövde sorguyla eşleşiyor | Exact arama eski gövde ve sürümü sessizce döndürebilir. Son sayfaya girmeyen eski aday da exact toplamını şişirebilir. Capped seri/melez, görünür kabul ettiği eski slotu okuyabilir. Aynı durum daha yeni run bir silme kaydı taşıyorsa da mümkün. | Exact seçimde checked kopya ya da seçilen run/slot okunur. Capped aynı run/slot üzerinden sonuç verir. CRC, anahtar ve sürüm seçilmiş kayda karşı sınanır. [Exact sonuç](../src/engine.rs#L2296) · [Grouped sonuç](../src/engine.rs#L2385) · [CRC ve kimlik](../src/segment.rs#L2170) · [Capped seri](../src/engine/first.rs#L578) · [Melez devam](../src/engine/first.rs#L266) |
| İki revizyon da eşleşiyor | İkisi de görünür aday olursa çift sayım veya çift hit mümkün. Checked capture eski kaydın kopyasını yeni run/slot ile değiştirirse ve sayfa eski slotu isterse `checked page slot mismatch` oluşur. Capture koşulları, bütçe ve sayfa seçimi hatanın görünmesini etkiler. | Hata iki belgeli regresyonla gözlendi. [Capture kapsamı](../src/engine.rs#L2101) · [Anahtar ve slot kontrolü](../src/segment/checked_page.rs#L53) · [Regresyon](../src/engine/light_flush/tests.rs#L108) |
| Yalnız yeni gövde eşleşiyor | Bu anahtar özelinde eski slot metin kontrolünden geçmez. Yeni kayıt dönebilir ve sorgu doğru görünebilir. | [Aday doğrulaması ve sonuç sayımı](../src/segment.rs#L2861) |
| GET veya anahtarla sonuç hazırlayan alternatif yol | GET aktif WAL ve delta'ya, sonra run'lara yeniden eskiye bakar. Yeni sürümü veya silinmişse boş sonuç verir. Hatalı adayı anahtarla hazırlayan arama bu sırada yeni gövdeyi döndürebilir veya silmede hata verebilir. Metin kontrolü ve exact toplamı o noktada baştan hesaplanmaz. Varsayılan grouped yol disk hit'lerinde bu GET adımını kullanmaz. | [GET](../src/engine.rs#L1630) · [Varsayılan yönlendirme](../src/engine.rs#L2009) · [Alternatif sonuç hazırlama](../src/engine.rs#L2317) |

**Bilinmiyor.** Sessiz hatanın eski gerçek iş yüklerinde kaç kere oluştuğu
ölçülmedi. Küçük regresyon iki revizyonun eşleştiği hatayı yeniden üretti.
Tek revizyonlu sessiz sonuç kod yolundan çıkarıldı.

## Önbellek ve yayım tablosu

Yazıcıda görünürlük maskesinin kalıcı taşıyıcısı `MaskCache`.
Aramadaki `mask`, oradan okunan bitler ve o görünümün WAL shadow anahtarları
ile oluşturulan geçici vektördür. Yazıcıdaki kayıtlar run'lar arası
gizlemeyi taşır, shadow her sorguda ayrıca uygulanır. Snapshot'ta dense
kayıt shadow'yu da kapsayabilir. Bütçe yüzünden eksik kalan dense kayıt
sonradan sparse olarak doldurulursa shadow ayrıca uygulanır.
[MaskCache](../src/engine.rs#L308) · [Açılış maskesi](../src/engine/open_masks.rs#L6) ·
[Snapshot maskesi](../src/engine/snapshot.rs#L655) ·
[Sparse fallback testi](../src/engine/snapshot/tests.rs#L51)

| Olay | Yazıcı maskesi ve delta | Snapshot ve dosya önbellekleri |
| --- | --- | --- |
| Aynı WAL'a commit veya prefix işinin başlatılması | Run listesi sabit, yazıcı maskesi tutulur. Commit delta ve shadow'yu günceller. Prefix işçisi ayrı WAL ve dondurulmuş girdi hazırlar. Yeni WAL henüz aktif değildir. | Commit yeni snapshot yayımlar. Aynı aktif WAL kimliğinde salt okunur handle klonlanabilir. Çalışan prefix'in hazırlığı tek başına görünümü değiştirmez. [Akışlı commit](../src/engine/streamed.rs#L361) · [Prefix hazırlığı](../src/engine/light_flush.rs#L25) · [Snapshot WAL](../src/engine/snapshot.rs#L692) |
| Asenkron prefix kurulumu ve aktif WAL rotasyonu | Yazıcı maskesi yeni run eklenince sıfırlanır. WAL'da yalnız tail anahtarları kalır, light kayıtlarının ofsetleri yeni WAL'a çevrilir. Önceki sürümde eksik adım maske sıfırlamasıydı. | Yeni snapshot önceki **snapshot** maskelerini alabiliyorsa eklenen run'ın anahtarlarını eski maskelere işler. Shadow farkını ve yeni run'a düşen kesişimi ekler. Shadow'dan çıkan prefix anahtarlarının gizlenmesi yeni run üstünden devam eder. Aktif WAL kimliği değiştiği için okuma handle'ı açılır. [Kurulum](../src/engine/light_flush.rs#L133) · [Snapshot ekleme yolu](../src/engine/snapshot.rs#L381) · [WAL handle](../src/engine/snapshot.rs#L692) |
| Senkron flush veya büyük akışlı commit ile WAL rotasyonu | İkisi de run ekleyip yazıcı maskesini sıfırlar. Flush, bellekteki belgeler, light kayıtları, shadow ve delta `Index`ini temizler. Büyük commit yolu önce mevcut delta'yı flush eder. | Yeni snapshot güncel run/delta'yı alır ve yeni aktif WAL dosyasına bağlanır. [Flush](../src/engine.rs#L2509) · [Flush sıfırlamaları](../src/engine.rs#L2585) · [Büyük commit](../src/engine/streamed.rs#L454) |
| Normal merge veya revizyon reclaim yayımı | İkisi de `install_output` ile run listesini değiştirir ve yazıcı maskesini sıfırlar. Aktif WAL ve delta korunur. Reclaim, planın run öneği değişmişse sonucu kurmaz. | Yeni snapshot, uyumlu merge penceresinde bitleri yeniden eşler. Shadow değişmişse, çıktı anahtarları/sırası tutmazsa veya bir run kaldırılmışsa tam maske kurulur. Eski snapshot giriş run'larını kullanabilir. [Ortak kurulum](../src/engine.rs#L2835) · [Reclaim önek kontrolü](../src/engine/background_revision.rs#L112) · [Snapshot eşleme](../src/engine/snapshot.rs#L487) |
| GC ve son snapshot bırakma | Canlı run veya WAL listesi değişmediği için reset gerekmez. GC manifest kimliğini ve pinleri kontrol edip emekli dosyaları toplar. `maintain_idle()` sonunda yapılan aynı run/WAL listeli manifest yayımı da ayrı bir sıfırlama gerektirmez. O çağrının içindeki flush ve merge kendi kurallarını uygular. | Snapshot önce view ve pin'i bırakır, ardından GC yürüyebilir. Eski run'ın son sahibi bırakıldığında owner önbellek girdileri de bırakılır. [GC](../src/engine/snapshot.rs#L111) · [Snapshot bırakma](../src/engine/snapshot.rs#L177) · [Son manifest](../src/engine.rs#L2947) |

| Diğer durum | Ömür ve geçerlilik |
| --- | --- |
| Terim sayfaları ve term → posting lookup | Proses genelinde sınırlı; sayfa lookup `owner+page`, terim lookup `owner+term`. Negatif lookup da owner'a bağlı. Yeni açılan/üretilen index yeni owner alır. Manifest değişimi ortak cache'i boşaltmaz. Eski owner girdileri kapasite nedeniyle elenebilir veya son owner kapanınca silinir. [Terim lookup](../src/segment/term_pages.rs#L117) · [Owner ömrü](../src/segment/term_pages.rs#L209) · [Sayfa ve term kullanımı](../src/segment/term_pages.rs#L335) |
| Kayıt metadata sayfaları ve doğrulama durumu | `PagedRecords` owner+ofset ile ayrılır, owner kapanınca girdiler gider. `Run`, immutable `DiskIndex`i ve varsa ertelenmiş kaynak kontrolü / REF01 kaynak tablosunu `Arc` ile paylaşır. VFY blok bitleri ve covering yetkisi kendi dosya bağlamında kalır. Yeni run eski run'ın doğrulama bitlerini devralmaz. [Sayfa owner ve Drop](../src/segment/paged.rs#L76) · [Sayfa lookup](../src/segment/paged.rs#L299) · [Run](../src/engine.rs#L105) · [Kaynak doğrulaması](../src/engine.rs#L1471) · [VFY yükleme](../src/segment/block_check.rs#L17) · [REF01 bağlama](../src/source_records.rs#L27) |
| Aktif WAL/delta ve paylaşılmış içerik | `docs`, `light`, `shadow` ve `Index` güncel delta durumudur. Snapshot kendi kopyasında doğru WAL'a bağlı kalır. `BodyCache` içerikle anahtarlanır, belge sürümünü seçmez ve snapshot'a kopyalanmaz. Opsiyonel `doc_cache` aynı içeriği paylaşır, snapshot boş cache ile başlar. Normal merge/reclaim bu yapılara dokunmaz. [Delta alanları](../src/engine.rs#L388) · [BodyCache](../src/index.rs#L328) · [Index kopyası](../src/index.rs#L424) · [Snapshot kopyası](../src/engine/snapshot.rs#L730) |
| Sorgu içi payload, posting ve normalleştirme | `CheckedPage`, posting pencereleri ve normalize metinler çağrıya özeldir. `CheckedPage` aynı anahtarın kopyasına run/slot iliştirir ve çağrı bitmeden düşer. `search_into` önceki sonuçlardan yalnız yeniden kullanılabilir metin tahsisini alır. Bu nedenle görünürlük hatasını gösterebilirler, sonraki manifest kuşağına eski sonuç taşımazlar. [CheckedPage](../src/segment/checked_page.rs#L1) · [Sorgu içi posting/metin](../src/segment.rs#L2763) · [Sonuç tamponu](../src/engine.rs#L2020) |
| Önceki snapshot ve reclaim planı | Yeni snapshot hazır görünümü atomik değiştirir. Önceki snapshot, maskeleri ve immutable kaynaklarıyla yaşamaya devam eder. Arka plan planı hazırladığı run öneğine bağlıdır. Yayım öncesinde önek doğrulanır. [Snapshot yayımı](../src/engine/snapshot.rs#L793) · [Arka plan planı](../src/engine/background_revision.rs#L54) |

**Açık bulgu.** Normal yazı yollarında düzeltmeden sonra eksik bir
run-listesi invalidasyonu görülmedi. Ancak `diagnostic_mask_pair`,
yazıcının run listesine doğrudan iki run ekliyor ve maskeyi orada
sıfırlamıyor. Başarılı merge'in ortak kurulum yolu reset yapıyor.
Eklemelerden sonra merge öncesi hata ile dönülür ve aynı handle üzerinde
aramaya devam edilirse eski maskenin kullanılabilmesi koddan görülüyor.
Bu yol diagnostic feature ve 100k probuyla sınırlı. Tarihsel denemede
gözlenip gözlenmediği bilinmiyor. [Tanı probu](../src/engine/snapshot/probe.rs#L4) ·
[Feature kapsamı](../src/engine/snapshot.rs#L881)

## Önceki ölçümlere etkisi

| Ölçüm | Kanıt | Sınır |
| --- | --- | --- |
| 24+24 olay görünürlüğü, reclaim açık/kapalı | Zamanlanan marker sorgusu snapshot üstünde çalıştı, doğru anahtar ve beklenen sürümü zorunlu tuttu. Yazıcıdaki exact eski-marker araması zamanlamadan sonra ayrıca sıfır sonucu doğruladı. | O 48 örnek için sessiz eski hit kanıtı yok. Arka plandaki 41 sorgu her çağrıda tam modelle eşlenmedi. Düzeltme sonrası zaman farkı bilinmiyor. [Ölçüm](RECLAIM-ARRIVAL-DEFAULT-50K-20261114.md) · [Marker kontrolü](../target/reclaim-followup-20261114/source/tools/bench_support/maintenance_arrival.rs#L51) · [Yazıcı kontrolü](../target/reclaim-followup-20261114/source/tools/bench_support/maintenance_arrival.rs#L240) |
| 30 tur disk, commit kuyruğu ve kesintisiz yazı | Disk envanteri dosya boyutlarından, kuyruk/commit saatleri yazı yolundan geliyor. Sıcak yük okuyucusu snapshot ile aradı. Sıra ve ilk/son hit GET'i örneklendi. Son 41-case oracle strict reopen'da, yeniden kurulan maskeyle geçti. Reclaim planının liveness hesabı kendi `revision_masks`ini kullanır. | Tarihsel disk ve commit kayıtlarını iptal edecek bir bulgu yok. Her yayım sonrasında aynı canlı **yazıcı handle'ındaki** exact/capped sorguları o koşular kapsamıyordu. Yeni reset'in cache ısısı ve süre bedeli yeniden ölçülmeli. [Açık/kapalı koşular](LIVE-DISK-50K-20261111.md) · [Varsayılan koşu](BACKGROUND-RECLAIM-DEFAULT-50K-20261112.md) · [Kesintisiz koşu](RECLAIM-CONTINUOUS-DISK-50K-20261114.md) · [Reclaim maskesi](../src/engine/revisions.rs#L148) |
| Motor-only RAM, A/B/B/A | Ingest, temiz idle ve tek churn örneğinin ölçülen prosesinde sorgu/okuyucu yoktu. Dolayısıyla bayat arama sonucu bu RAM değerlerine giremez. | Yazıcı ve bakım işçileri yine çalıştı. Maske reset'inin allocator veya ortak sayfa cache'i üzerinden yeni ölçüme etkisi bilinmiyor. Tam rewrite tepesi önceki ölçümde de dışarıda. [RAM raporu](RECLAIM-MOTOR-ONLY-RAM-50K-20261114.md) · [Sürücü kontrolü](benchmarks/reclaim-followup-20261114/04-run.py#L113) |

## Adım 2 için onay taslağı

- Mevcut küçük test altyapısıyla **16 sabit tohum, tohum başına 24 kısa
  işlem grubu**. Yaklaşık 40–80 anahtarda tekrar yazma, silme ve yeniden
  ekleme. Bağımsız sıralı model her commit'te gövdeyi, sürümü ve
  sequence'i saklar. Ortak terim, yalnız eski/yeni revizyonu eşleyen terim,
  silinen anahtar, negatif ve sayfalı sonuç sorguları kullanılır.
- İlk yazıdan önce yazıcı maskesini ısıt. Prefix kurulumu, normal merge,
  reclaim rewrite/emeklilik ve gerçek GC için kısa, zorunlu kontrol
  noktaları oluştur. Rastgele akışa ayrıca ek kontroller koy. Olayı
  çağrının dönmesiyle varsayma. Prefix'te aktif WAL ve run listesi,
  merge/reclaim'de yayımlanan girdi/çıktılar, GC'de eski dosyanın
  kalkması ve canlı/pinli dosyaların yerinde olması tanık olur.
  Uygun reclaim işi için kontrollü olarak ölü revizyon eşiğini aşan
  küçük bir başlangıç düzeni kullan.
- Her yayım öncesi ve sonrası aynı yazıcı handle'ı, güncel snapshot ve
  önceki pinli snapshot için GET, exact toplam, sıralı tam hit/sürüm,
  capped sınırı ve sequence modelle birebir karşılaştırılır. Pinliyken
  GC ve son pin bırakıldıktan sonraki GC ayrı doğrulanır. Çelişkide
  tohum, işlem dizisi, olay, kuşak ve sorgu hata kaydına girer.
  Her olayın en az bir gerçek yayımı zorunlu. CI süresi uygulama turunda
  raporlanır. Mevcut regresyon ve test yardımcıları kullanılabilir.
  [Önceki rastgele oracle](../tests/segments.rs#L736) ·
  [İşçi kontrolü](../src/engine/test_jobs.rs#L1) ·
  [Reclaim fixture](../src/engine/background_revision/tests.rs#L17) ·
  [GC testi](../src/engine/snapshot/tests.rs#L135)

## Çalışma alanı

Dosya uzunlukları dizin envanterinden toplandı, sıkıştırılmış fiziksel
disk tahsisi ölçülmedi. Bu adımda test, benchmark, DB açılışı ve temizlik
yapılmadı.

| Konum | Bayt | MiB |
| --- | ---: | ---: |
| `target/wal-prefix-visibility-20261114` | 87.903.890 | 83,83 |
| `target/reclaim-followup-20261114` | 97.709.499 | 93,18 |
| FDB slotu | 71.560.276 | 68,25 |
| SQLite slotu | 515.284.992 | 491,41 |
