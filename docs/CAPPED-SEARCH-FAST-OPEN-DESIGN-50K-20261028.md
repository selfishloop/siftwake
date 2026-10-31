# Capped arama ve hızlı açılış — uygulama öncesi tasarım

Son kullanıcı kararı: açılış50k ilk arama≤1000ms,100k≤2000ms; kaynak başına
lazy şimdilik yapılmayacak, bütün doğrulama korunacak. Bu belgedeki300ms/lazy
kararı tarihsel kalır. [Güncel melez/cancel duruşu](HYBRID-CANCEL-50K-20261028.md)
ve [yeni corpus offline analizi](TRWIKI-OFFLINE-20261028.md): yeni corpus DB
ölçülmedi; eski sonuçlara yeni kapı uygulamak yeni corpus kanıtı değildir.


Durum (28 Ekim güncel karar): **capped N=50 ve hızlı oturum tasarımları onaylı.**
Capped ve biçim değiştirmeyen exact deneyleri uygulanıyor; hızlı oturum arama
kapısından sonra gelir. Önceki <100ms açılış kapısı kaldırıldı; yeni hedef
**arka planda açılış ve ilk kullanılabilir arama ≤300ms**. Oturum dönüşü,
ilk sonuç ve tam doğrulama bitişi ayrı raporlanır. [Ölçülen maliyetler ve ham kanıt](QUERY-COST-OPEN-50K-20261028.md).
Kullanıcının 28 Ekim kararıyla önceki
10k→50k ≤5× kapısı kaldırıldı. Yeni hedefler: yaygın sorgular ≤30 ms;
orta/nadir sorgular 50k'da ≤150 ms, gerçek corpus sağlandıktan sonra 100k'da
≤300 ms. `zx/the zx` geniş taraması bilinen sınırdır. Varsayılan exact-total ve
strict-open davranışları korunur. Bu belge Rust API'siyle sınırlı; C ABI,
protokol, uygulama entegrasyonu ve kalıcı biçim değişikliği içermez.

Güncel uygulama kanıtı: [capped/exact4 arama kapısı GEÇTİ](CAPPED-EXACT-50K-20261028.md).
[İlk arama prototipi KALDI](BACKGROUND-OPEN-50K-20261028.md): kaynak,SEG05
directory ve indeks CRC arka planda4 worker ile denetlendi; ilk cevap bütün
strict kontrolünü bekledi ve306–757ms oldu. Bu sürüm aşağıdaki per-source lazy
yaklaşımın tamamı değildir; Pending ilk sonuç/öncelik/cancel uygulanmadı.
Kötü sonuçlar korundu; bir sonraki düzeltmeye geçilmedi. Normal API aynı.

## 1. Exact-total neden pahalı?

`DiskIndex::visit_matches_with_capture` sonuç bütçesi dolunca aday döngüsünden
çıkmaz. Doğrulanmış eşleşmelerin tümünü sayar; yalnız retained sonuç sayısını
sınırlar. Dolayısıyla sayım ayrı ikinci bir payload geçişi değildir. `total++`
işleminin süresini ölçmek esas maliyeti göstermez: kesin total gereksinimi,
ilk sayfadan sonra da posting kesişimi/payload/CRC/metin kontrolünü zorunlu tutar.

Mevcut proven universal/bigram/trigram yollarında bazı sayımlar payload
taramadan yapılabiliyor. Bu istisna bütün uzun kelimelere genellenemez.
Beş hedef sorgunun aday/okuma/CRC/metin/cache profili ayrı ham logla raporlanır;
tanılama duvar süreleri normal p95 yerine kullanılmaz.

İlk tanılamada harpoon 5212 total/50 hit için10651 payload adayı ve355MB,
presumption 4318/50 için12635 aday ve463MB okudu. Transubstantiation **0**
eşleşme için7563 aday/281MB gerektirdi. Capped sıfır sonuçlu sorguyu tek başına
iyileştirmez; ≤150ms kapısına bu sorgu için kendiliğinden istisna eklenmez.

## 2. Önerilen sınırlı API

```rust
enum Total {
    Exact(usize),
    AtLeast(usize),
}

struct FirstPage {
    sequence: u64,
    hits: Vec<StoredDocument>,
    total: Total,
    validation: ValidationStatus, // Normal strict DB'de Verified.
}

// İsimler öneridir; henüz public API değişikliği yok.
Database::search_first(query: &Query) -> Result<FirstPage>;
Snapshot::search_first(query: &Query) -> Result<FirstPage>;
```

- `query.limit` 1–200, `offset=0`. İlk prototipte sayfalama yok; mevcut exact
  API'nin sayfalama/limit sözleşmesi değişmez.
- Mevcut anahtar sırasının **ilk N** sonucu döner. Gereksinim belgesindeki
  puan/mtime ile küresel en iyi N sonucu garantisi bu iş değildir. İlk N'de
  durarak bütün corpus'un en yüksek puanlı N belgesini bulduğumuz söylenmez.
- N doğrulanmış, görünür, farklı belge bulununca durur: `AtLeast(N)`.
  N+1'i denemez; gerçekte toplam N olsa bile `Exact(N)` iddia etmez.
- Adaylar tamamen biter ve N'e ulaşılmazsa `Exact(k)`; sıfır sonuç da bu yoldur.
  Bu istatistiksel tahmin değildir: açıkça işaretlenmiş alt sınırdır.
- Silinmiş/eski revizyon maskeleri metin doğrulamasından önce uygulanır.
  Bütün dönen belgelerin payload CRC/schema/key/version ve AND/katlama
  doğrulaması korunur. Yayımlanmış tek commit sürümünün sonuçları döner.
- Normal DB ve snapshot aynı sonuç/total kuralını kullanır. Snapshot pin ve
  emekli dosya temizliği kuralları değişmez.

Uygulama: aday cursor'ları anahtar sırasında ilerler. Kaynaklar arası küçük bir
heap görünür adayları birleştirir; yalnız global ilk N'i kanıtlamak için gereken
adaylar doğrulanır. Her segmentten N sonuç üretip yine bütün total'ı saymak
capped sayılmaz. Aktif WAL/delta da aynı sıralama ve görünürlük kuralına dahildir.
İlk prototip mevcut posting/kayıt biçimlerini ve sınırlı scratch bütçelerini
kullanır; corpus ya da tam metin önbelleği eklemez.

Ölçüm: aynı model/snapshot üzerinde harpoon, presumption, darcy,
transubstantiation, holmes elementary; exact ve capped N=1/50/200, ters A/B
sıraları, SQLite aynı semantik ile. Capped hit listesi exact'in ilk N'iyle
birebir karşılaştırılır. `AtLeast(k)` ise exact total≥k; `Exact(k)` ise eşittir.
Normal p95, adaylar, okunan byte/istek, CRC/metin süreleri ve RAM ayrı verilir.
Negatif/seyrek sonuçta bütün adaylar gerekebilir; capped için peşinen hız
garantisi verilmez. Capped N=50 esas yoldur. Kullanıcı ayrıca exact-total için en fazla4 doğrulama
işçisi, komşu payload birleştirme ve pinli doğrulanmış kaynak CRC
yeniden kullanma deneylerini onayladı; orta/nadir p95≤150ms kapısı ölçülür.

## 3. Açılış: yalnız kaynak CRC'sini ertelemek yeterli değil

Üç tanılama açılışında kaynak/native doğrulama 507–618 ms, indeks açılışı
294–344 ms ölçüldü. İndeks içindeki metadata hazırlığı 181–205 ms;
index CRC 99–122 ms. Son iki süre indeks açılışının içindedir, ayrıca toplanmaz.
Bu aşamalar sürerken yalnız kaynak taramasını kaldırmak <100 ms sonucunu
kanıtlamaz. Ölçülen strict yol değiştirilmedi.

Başlık/bağ sınırları ve mevcut MET04 checksum/root okuması için ayrı salt-okuma
probu13–15ms ölçtü. Bu prob Database/Snapshot oluşturmaz, source/index tam
doğrulamasını ve directory hazırlığını yapmaz; işlevsel hızlı açılış geçti
sayılmaz. Yalnız ertelenen işler dışındaki I/O'nun ölçülmüş maliyetidir.

SEG05 değişken uzunluklu kayıt/sözlük alanlarını açılışta okuyor; MET04 kayıtları
SEG05 ile karşılaştırılıyor. Yeni bir bootstrap dosyası/kalıcı format önermiyorum.
İlk arama≤300ms hedefinde bu hazırlık da lazy/arka plan işlerine dahildir.
SEG05 dizin çözümleme(~185ms) ve indeks CRC(~107ms) ön planda zorunlu
açılış işi sayılmayacak; hiçbir denetim sessizce kaldırılmayacak.

## 4. Önerilen opt-in hızlı okuma oturumu

Mevcut `Database::open` strict kalır. Yeni, ayrı bir açılış oturumu düşünülür:

```rust
OpeningDatabase::open_for_search(path) -> Result<OpeningDatabase>;
OpeningDatabase::validation_status() -> ValidationStatus;
OpeningDatabase::search_first(query) -> Result<FirstPage>;
OpeningDatabase::try_finish() -> Result<Option<Database>>;
```

1. Ön planda DB kilidi, manifest CRC/nesil/sınırlar, kaynak dosya kimliği/boyutu,
   native başlık/son sınırlar ve mevcut cache bağları kontrol edilir. Temiz native
   FDBLOG04/SEG05/MET04 ilk hedef. Eksik/geçersiz cache senkron yeniden kurulmaz;
   `ERR_REBUILD` veya strict yol gereksinimi açıkça döner.
2. Kaynaklar ve dosyalar mevcut lease/pin bağlamında tutulur. İndeksler volatil
   lazy durum taşır; yeni dosya/kayıt sürümü yazılmaz. Anahtar aralığı ipuçlarının
   kullanılabilmesi için mevcut metadata'nın CRC/bağ/sınır kontrolleri yapılır.
3. **Bir kaynak sorguda gerçekten kullanılmadan önce o kaynağın mevcut tam
   strict kontrolü ve indeks hazırlığı biter.** Dönen hit için kontrol kaldırılmaz.
   Background kalan kaynakları sınırlı scratch ile doğrular. Aynı kaynak için
   çalışan kontrol varsa sorgu yalnız o kaynağı bekler; bekleme ayrı ölçülür.
4. Global durum `Pending` iken oturum kendisini tam doğrulanmış DB olarak sunmaz.
   İlk N sonucu gösteren cevapta sağlık durumu ayrıca belirtilir. Global exact
   total, tam belge sayısı ve normal writer hazır sayılmaz. Kaynakların tamamı
   doğrulanınca `try_finish` normal DB'yi teslim eder; pinler kesilmez.
5. Doğrulama bitmeden yazma kabul edilmez (`VALIDATION_PENDING`). Boş yeni DB'nin
   doğrulanacak eski kaynağı yoktur. Tamamlanınca mevcut yazma yolları kullanılır.
6. İlk okumada veya background'da bozulma: oturum atomik `Failed` durumuna geçer;
   gelecekteki arama/snapshot/yazma çağrıları `ERR_REBUILD` döner. Hata çağırana
   poll/event ile görünür olur; sağlam cache bozuk kaynağı sağlıklı yapmaz.

### Açık güvenlik ve gecikme bedeli

- Strict açılış tüm bozulmayı ilk sonuçtan önce arar. Hızlı oturumda kullanılmamış
  bir kaynağın bozulması daha sonra bulunabilir. Global bütünlük/completeness
  `Pending` iken garanti edilmez; hata geldiğinde çağıran pending sonuçları
  geçersizleştirmelidir. Bu taviz varsayılan strict API'ye taşınmaz.
- Sadece çabuk dönen boş bir handle'ı “DB hazır” diye raporlamayacağız.
  Oturum dönüşü, ilk kullanılabilir sonuç, kaynak beklemesi ve tam doğrulama
  bitişi ayrı ölçülür. Exact ilk sorgu bütün kaynakları gerektirirse açılış
  maliyetini sorguya taşır. Hiçbir maliyet kaybolmuş sayılmaz.
- Arka planda kontrol başlamış ilk kaynağı beklemek ilk sorguyu yavaşlatabilir;
  soğuk cache sonucu henüz yok. Background için sınırlı I/O/scratch ve öncelik
  ölçülür; mevcut normal writer/reader bütçeleriyle çatışma saklanmaz.
- Dirty WAL'ın commit sınırı/CRC/replay kontrolü atlanmaz. İlk arama≤300ms temiz
  oturum hedefi dirty500'e otomatik uygulanmaz. Dirty500 ayrı, mevcut sıradaki ölçümdür.
- Dönüşten sonra açık source/cache dosyasının değişmesi, metadata bağı bozulması
  veya kontrol işçisinin hata vermesi de `ERR_REBUILD` yönüne gider. Kaynak
  kendiliğinden onarılmaz, dış proje dosyalarına dokunulmaz.

Doğrulama kapıları: sağlam cache + bozuk kaynak, ilk hit/okunmayan kaynakta tek
byte bozulması, kesilme, background hata/panic kapanışı, kaynak başına eşzamanlı
ilk okuma, snapshot pini/son bırakma, kill/reopen ve eski strict okuyucu.
İlk kullanılabilir arama≤300ms, oturum dönüşü ve tam kontrol süreleri, idle/peak RAM ayrı
ölçülmeden bu tasarım geçti sayılmaz. Kalıcı biçim gereksinimi çıkarsa durulur.

## 5. Devam sırası

Önce tasarım kararı ve capped/exact karşılaştırması; sonra seçilen hızlı açılış
prototipi. Ardından 50k tekli commit → dirty500 → 4R+yazıcı → 100'lük
commit/RAM profili; sonra 2c uzun snapshot → 2b gerçek soğuk cache.
Yeni bir kapı tutmazsa durulur. 100k ancak kullanıcının sağlayacağı gerçek
kod/belge/metin corpus'uyla planlanır; sekiz kitap pasajını 100k temsili diye
sunmayacağız. Blok3/4 ve C ABI başlamaz.
