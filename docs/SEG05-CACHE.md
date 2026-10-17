# SEG05 türetilmiş indeks deneyi

SEG05 yalnız isteğe bağlı cache dönüşümünde üretilir. Normal commit, flush,
merge ve bozuk cache'i yeniden oluşturma yolu SEG04 üretmeye devam eder.
FDBLOG04, WAL ve FDBMAN03/ready biçimleri değişmez. `.fdb/.ready` kaynak,
`.idx` yeniden oluşturulabilir cache olmaya devam eder.

`bounded-engine` okuyucusu SEG03/04/05 karışımını kabul eder. SEG05 desteği
olmayan okuyucu bilinmeyen cache'i kaynak replay'iyle kendi desteklediği
biçimde yeniden oluşturabilir; kaynak formatını ayrıca desteklemesi gerekir.
0.1 fixture kaynakları değiştirilmez; eski JSON kaynak okuması korunur.

## Yerleşim

Tüm sayılar little endian, tüm string'ler `u32 byte_length + UTF-8` biçiminde.
Rust enum yerleşimi veya genel serileştirme kütüphanesinin ABI'si saklanmaz.

| Bölüm | İçerik |
|---|---|
| Header,28 B | `FDBSEG05`, sequence:u64, source_end:u64, source_crc:u32 |
| Payload | Flag0: SEG04 ile aynı binary payload kopyaları. Flag1: boş |
| Posting verisi | Önceki indeksle aynı sıralı little-endian u32 kayıt ID'leri |
| Directory | `FDBDIC01`, flag:u8, record_count:u32, term_count:u32, records, terms |
| Footer,32 B | `FDBEND05`, directory_offset:u64, directory_len:u64, prefix_crc:u32, footer_crc:u32 |

Flag yalnız0/1 olabilir. Kopyalı halde payload header'ın ardından gelir ve
posting'ler son payload'ın bitişinden başlar. Referans halinde posting'ler
28. byte'tan başlar. Posting alanı directory başlangıcına kadar bitişiktir.
Directory256MiB sınırı korunur; her uzunluk kalan veriyle karşılaştırılır.

Kayıt: scope:string, collection:string, id:string, offset:u64, len:u32,
checksum:u32, version:u64, deleted:u8. Anahtarlar artan sırada ve benzersizdir.
Silme işareti yalnız0/1; deleted ile len0 eşdeğer, sürüm1..sequence aralığındadır.
Flag1'de canlı kayıt offset'i kaynak FDBLOG04 entry payload'ını gösterir;
aralıklar artar, çakışmaz ve source_end'i aşamaz. Tombstone offset/len/CRC0'dır.
Flag0'da offset'ler indeks payload alanında bitişiktir.

Her term tag'inden sonra aşağıdaki değerler, ardından posting_offset:u64 ve
posting_count:u32 gelir. Terimler mevcut `Term` sırasındadır; count pozitif
ve kayıt sayısından büyük olamaz.

| Tag | Alanlar |
|---:|---|
|0 | Scope: scope:string |
|1 | Collection: scope:string, collection:string |
|2 | Exact: scope:string, collection:string, field:string, value:string |
|3 | Numeric: scope:string, collection:string, field:string, value:i64 |
|4 | Gram: scope:string, gram:string |
|5 | Link: scope:string, collection:string, id:string |

## Doğrulama ve bellek

Tam kaynak CRC/frame/schema/UTF-8/key/link/version kontrolü normal açılışta
indeks kabulünden önce tamamlanır. İndeksin tam prefix CRC'si ve footer CRC'si
kontrol edilir. İkili directory akışla tamamen taranır; kayıt/terim sırası,
uzunluklar, sürümler ve alan sınırları doğrulanır. Bu deneme bütün dictionary
tarama maliyetini kaldırmaz; JSON ayrıştırmasını değiştirir.

MET04 kayıt sayfaları bağımsız directory doğrulamasıyla karşılaştırılır;
formatları değişmez. Term sayfalarının CRC'leri alınır; sorguda sayfa CRC,
ilk/son term ve sıra kontrolü korunur. Mevcut paylaşımlı256KiB kayıt ve1MiB
term cache'i kullanılır. Kaynak referansları için RAM'de ikinci bir kayıt
adres tablosu veya belge havuzu tutulmaz; aynı kaynak handle'ı klonlanır.

Get/contains/hydration, kaynaktaki aynı payload byte'larını okur. CRC, yapı,
UTF-8 ve key/version kontrolleri sürer. Windows immutable source lease'i
yazma/silme/rename'i engeller. Opt-in deferred API'de kaynak doğrulaması
ilk kullanımdan önce yine tamamlanır.

Toplu payload okuması mevcut256KiB sınırını ve düşük kullanımda kapanma
kuralını korur. FDBLOG04 kaynaklarında bitişik entry'ler arasındaki5 B ve
checkpoint frame sınırındaki42 B header/footer aralığı tanınır. En az üç
komşu istek görülmeden read-ahead başlamaz. Her kayıt aralıktan ayrı dilimlenir
ve doğrulanır. Arama sözcüklerine özel durum eklenmez.

## Dönüştürme ve kurtarma

```rust,ignore
use fdb::{Database, IndexCacheLayout};
let db = Database::open(path)?;
db.repack_index_cache(IndexCacheLayout::BinarySourceReferences)?;
let db = Database::open(path)?;
```

Alternatifler `BinaryPayloadCopy` ve geri dönüş için `JsonPayloadCopy`.
API handle'ı tüketir; delta, çalışan bakım işi veya Legacy kaynak varsa
reddeder. Yalnız değişmez native Run cache'leri dönüştürülür.

Referanslar gerçek source entry sınırlarından, tam kaynak doğrulaması sırasında
üretilir; eski indeksteki key/version/len/CRC ile eşleşir. Posting byte'ları
değişmez; directory offset'leri yeni yerleşime göre ayarlanır. Yeni cache
geçici dosyada sync edilir ve bağımsız açılışla doğrulanır. Sonra eski cache
unlink edilir, yeni dosya hedefe rename edilir. Bu iki adımın arası atomik
bir kaynak yayını değildir: kesilmede cache eksik kalabilir; sağlam kaynak
normal açılışta yeniden indekslenir. Kaynak/ready yayını yapılmaz.

Kesilme/tek-byte bozulması, CRC'si yeniden hesaplanmış geçersiz metadata,
silme/merge, eski biçime dönüş ve cache değişiminin üç aşamasında process kill
test edilir. Process kill elektrik kesintisi değildir. Repack'in tepe belleği,
normal yazma hızı, soğuk/eşzamanlı yük ve otomatik SEG05 builder geçişi bu
deneyin ölçülmüş sonuçları değildir.

[Ölçüm, karar ve açık sınırlar](COMPACT-INDEX-10K-20261017.md).
