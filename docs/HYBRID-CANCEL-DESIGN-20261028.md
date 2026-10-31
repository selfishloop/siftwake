# Melez capped ve arama iptali — 28 Ekim

Kullanıcının kararı: normal `search` exact/serial, `search_first` opt-in;
join/CRC reuse kapalı. Kalıcı biçim, Blok3/4 ve C ABI değişikliği yok.
Yeni Türkçe corpus yalnız verilen D: dosyasından okunur; benchmark DB'leri
mevcut C: sabit slotlarda kalır. Metin/başlık/alan/linkler kırpılmaz/değişmez.

## Melez devam

`search_first_hybrid(q, 200)` serial global key heap ile başlar. Maskeden
geçip doğrulanan200 adaydan önce N dolarsa hiçbir worker/permit alınmaz.
N dolmadıysa aynı cursor'lar, kuyruktaki ilk işlenmemiş ID ve son verifier
scratch'i devredilir. Serial prefix yeniden enumerate/verify edilmez.
Dinamik source-job kuyruğunu en fazla4 işçi tüketir. Her source yalnız eksik
N kadar yerel hit tutar; global key heap, serial prefix'in ardına ilk eksik
sonuçları ekler. Negatifte bütün kalan adaylar doğrulanır. Eşleşme bulunmayışı
örnekleme/aday bütçesiyle tahmin edilmez. Snapshot aynı epoch/pin'i korur.

Serial/4/melez aynı sorgu, N50, aynı veri ve döndürülmüş dengeli sıra ile;
p95, CPU toplam/ortalama ve aday/okuma sayıları ayrı verilir. Negatifte4'e
göre kötüleşme gizlenmez.200 aday içinde N bulan pozitiflerde worker0 ve
serial kadar doğrulama yapılması CPU ölçümüyle birlikte değerlendirilir.
CPU kuantalaması için bol-pozitiflere ayrıca ardışık sorgu blokları uygulanır.
Bu ölçüm sonucu gelmeden melez varsayılan veya başarılı sayılmaz.

## CancelToken

Bir UI isteği bir clone edilebilir, sticky `CancelToken` taşır. Yeni tuşta
eski token iptal edilir, yeni token oluşturulur; token resetlenmez. Database
ve Snapshot için serial/4/melez first-page ve experimental exact iptal
metotları vardır. Normal search token gerektirmez, davranışı değişmez.

- Aday setup/seed taraması, source geçişi, her payload adayı, hydration ve
  cevap öncesinde kontrol; permit beklerken2ms aralıklı kontrol.
- İptal `Error::Cancelled`: kısmi sayfa veya yanlış Exact(0) dönmez, DB
  poison edilmez. Bütün işçiler join edilmeden çağrı dönmez; pin/scratch bırakılır.
- Aktif delta iptalli yolda anahtar anahtar okunup aynı filtre/metin semantiği
  uygulanır; iptalsiz yolun indeksli delta yürüyüşü korunur. Bu opt-in yolun
  delta CPU bedeli ayrıca sınırdır. Tek belge boyutu mevcut2MiB sınırındadır.
- Önceden strict doğrulanmış DB/snapshot gerekir. Pending/deferred source'un
  bütün CRC kontrolünü iptal etmek için denetim atlanmaz; bu API açık Invalid
  döner. OpeningDatabase background doğrulaması query token'ıyla kesilmez.
- Senkron OS okuması, kısa cache/metadata mutex kritik bölümü ve tek belge
  CRC/schema/metin kontrolü ortasında zorunlu thread termination yapılmaz.
  Bu işlemlerden sonraki checkpoint beklenir: soğuk/duran I/O için gerçek zaman
  iptal üst sınırı garantisi yoktur. Ölçülen warm gecikme bu garanti değildir.
- Sorgu bitişiyle cancel aynı anda yarışabilir. Cevabı kullanmadan UI kendi
  request/generation ID'sini ve token'ı kontrol eder; DB sequence UI istek
  numarası değildir. Kapanış/ingest/merge cancel kapsamı dışında.

İptal ölçümü ayrı observed token ile aday başlangıcını sayar; normal token
paylaşımlı sayacı artırmaz.32/256 aday başladıktan sonra cancel→bütün thread'ler
join süresi ölçülür. Sorgu daha önce biterse ayrı raporlanır, iptal başarısı
sayılmaz. Admission iptali, iptal sonrası sağlık ve snapshot oracle'ı test edilir.

## Açılış ve filtre kararları

Yeni sınır50k ilk arama≤1000ms,100k≤2000ms. Kaynak başına lazy açılış şu an
yapılmayacak; mevcut background tüm strict kontrolleri tamamlar. Yeni corpus'ta
oturum dönüşü/ilk sonuç/tam doğrulama yine ayrı ölçülür. Önceki kitap ölçümüne
yeni sınırı uygulamak yeni corpus kapısını geçti sayılmaz.

Ön filtre uygulaması yok. Verilen Türkçe corpus'ta yalnız offline3/4-gram
adayları ve tam normalize substring/AND oracle'ı karşılaştırılır. Byte bitseti
ve position listesi yazılmaz. Anahtar/path politikası Frunner kararıdır;
1007 bayt sonrası uzun yol stratejisi FDB'de değiştirilmez.
