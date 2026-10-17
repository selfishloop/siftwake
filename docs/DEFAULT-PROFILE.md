# FDB varsayılan çalışma profili

Normal `cargo build --release`, ölçülmüş düşük-RAM yazma/sayfalı indeks/
doğrulanmış arama birleşimini `bounded-engine` adıyla içerir. Uygulama bu
birleşimi kullanmak için deneysel Cargo feature'ları seçmek zorunda değildir.
Bu adlandırma bir toplam RAM sınırı veya bütün yüklerde performans garantisi
değildir. Sayılı deneylerin eski sonuçları kendi koşullarıyla korunur.

## Yazma API'sinin seçimi

| API | Kullanım | Bellek ve sınır |
|---|---|---|
| `Database::commit_streamed(iterator)` | Kaynaktan gelen belgeleri tek tek alan atomik toplu yazma | Başarılı işlem sonunda resident delta boş; input/builder/bakım ortak çalışma bütçesi. Encoded kayıt ≤256 KiB, anahtar allowance ≤1 MiB, tahmini indeks allowance ≤4 MiB, işlem ≤10k mutation ve ≤64 MiB frame. Caller/sonuç/OS belleği toplamına sınır değildir. |
| `Database::commit(Vec<Mutation>)` | Önceden hazırlanmış belge listesi veya akışlı API'nin sınırlarına sığmayan mevcut kullanım | Önceki belge/işlem limitleri ve delta watermark davranışı korunur. Caller listesi, özgün/folded delta metni ve indeks belleği vardır; akışlı yolun yaklaşık9 MiB ölçümünün bu API'ye ait olduğu söylenmez. |

Mevcut JSON Lines `commit` isteği normal Vec API'sini kullanmaya devam
eder; istek satırının tutulma/JSON çözme belleği de vardır. Akışlı ölçüm
Rust API'sine aittir. Protokol veya dış proje entegrasyonu değiştirilmedi.

Örnek, özel bir deneysel derleme seçeneği olmadan:

```rust,no_run
use fdb::{Database, Document, Guard, Mutation};

fn write_documents(
    database: &mut Database,
    documents: impl Iterator<Item = Document>,
) -> fdb::Result<u64> {
    database.commit_streamed(documents.map(|document| {
        Ok(Mutation::Put { document, guard: Guard::Any })
    }))
}
```

Her iki API'de de atomik işlem, guard/duplicate kontrolleri ve varsayılan
senkron WAL korunur. Akışlı API kendi admission sınırına sığmayan işlemi
WAL append öncesi reddeder. Normal API bu daha dar sınıra otomatik
yönlendirilmez; mevcut geçerli büyük belgeler reddedilmeye başlanmaz.

## Açılış ve arama

`Database::open` tam kaynak doğrulamasını korur. Sayfalı metadata/terim
sözlüğü, sınırlı read cache'leri, doğrulanmış gram kanıtları ve uygun
sayfalarda grouped hydration varsayılan birleşimin parçalarıdır.
`Database::search` önceki text/AND/Unicode/filter/version/pagination
semantiğini korur. Kullanıcı sorgularına özel sabit kelime listesi yoktur.

`open_deferred` ve `open_record_reads` hâlâ açıkça seçilen API'lerdir.
`open_record_reads` için eksik/stale adres cache'i güvenli eski yola döner;
ilk search bütün pending kaynakları doğrular. Hızlı açılışın karşılığında
ertelenmiş doğrulamaya sahip API'ler normal `open` yerine otomatik seçilmez.

`.fdb` ve `.ready` kaynağı, `.idx` ve yardımcı cache'lerin türetilmiş olma
kuralı değişmedi. Yeni native indeksler artık SEG05 ikili sözlük ve kaynak
referanslarıyla yazılır: belge indekse kopyalanmaz. Flush, merge ve cache
onarımı aynı yolu kullanır. Okunabilir SEG03/04 cache açılışta korunur;
kaynak, WAL ve manifest biçimleri değişmez. Eski JSON/0.1 kaynak uyumluluğu
korunur; fixture değiştirilmez. [Biçim ve kurtarma](SEG05-CACHE.md),
[geçiş doğrulaması](DEFAULT-SEG05-10K-20261017.md).

Tam doğrulamalı açılış ve başarılı flush/akışlı commit/merge sonrası eski
nesiller otomatik temizlenir. Aktif kaynak/WAL ve çalışan okuyucu/worker
korunur; bilinmeyen isimler silinmez. Açıkça `reclaim_obsolete()` çağrısı
full integrity ve geri kazanılan dosya byte'larını raporlar. Cleanup I/O
hatası başarılı commit'i hataya çevirmez; explicit çağrı hatayı bildirir.
Deferred açılışta doğrulanmamış veriden hareketle silme yapılmaz.
[Ayrıntılar ve mevcut10k kanıtı](RECLAIM-10K-20261017.md).

## Derleme seçenekleri ve uyumluluk

| Derleme | Anlam |
|---|---|
| `cargo build --release` | Yeni varsayılan `bounded-engine` birleşimi |
| `cargo build --release --no-default-features` | Kontrol/karşılaştırma için önceki temel yol; akışlı API bu profilde yok |
| `cargo build --release --no-default-features --features experimental-builder-budget` | Aynı bounded birleşim; eski ad yalnız uyumluluk takma adı |

`experimental-builder-budget` için ayrı bir uygulama kopyası yoktur;
`bounded-engine`'e bağlanır. Alt bileşenlerin eski feature adları, bağımsız
kontrol deneylerini ve araçlarını korumak için tutulur. Uygulamanın bunları
tek tek seçmesi gerekmez. Tek bir bileşeni izole etmek isteyen benchmark
komutu, artık `--no-default-features` belirtmelidir.

rkyv payload, WAL'dan delta belge okuma, deneysel belge paylaşımı ve WAL
sync kapatma gibi bağımsız deneyler varsayılan olarak etkinleşmez.
Tanı sayaçları da varsayılanda kapalıdır. SQLite yalnız ayrı comparator
projesindedir; engine bağımlılığı değildir. Comparator `fdb` tiplerini
`default-features = false` ile kullanır.

Geçmiş tarihli raporlardaki "default kapalı" ifadeleri o ölçümün Git
durumuna aittir. Güncel seçim bu belgedir; son stabilizasyon sonuçları ve
açık kalan işler `docs/OPEN-WORK.md` üzerinden izlenir.
