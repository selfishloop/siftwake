# Anahtar sırası ve en yeni N

Kullanıcı bu sırayı ve ters-mtime biçimini kabul etti: `{:016x}|yol`;
mtime değişince delete+insert aynı transaction.1007 bayt sonrası uzun yol
stratejisi Frunner kararıdır; FDB'de ek motor değişikliği yapılmadı.


Koddan doğrulanan sıra `(scope, collection, id)` tuple sırasıdır. Her bileşen
Rust `String` sırasıyla, UTF-8 baytlarına göre lexicographic karşılaştırılır.
Arama metninin Türkçe/küçük harf normalleştirmesi anahtarlara uygulanmaz.
Anahtarlar büyük/küçük harfe duyarlıdır. `search_first` kapsam/collection içinde
bu sıranın ilk N eşleşmesini verir; skora veya belge sürümüne göre sıralamaz.

Her bileşen ayrı ayrı **1..1024 UTF-8 bayt**, boş olmayan ve Unicode kontrol
karakteri içermeyen metin olmalıdır (`validate_key`, `src/lib.rs`). Bu karakter
sayısı değildir; Windows UTF-16 yol uzunluğuyla aynı sınır değildir.

Aynı scope ve collection kullanıldığında, unsigned 64-bit mtime için
`format!("{:016x}|{}", !mtime, canonical_path)` ID'si yeni mtime'ı önce getirir.
Sabit 16 hex basamağı gereklidir; değişken uzunlukta ondalık metin sayısal
sırayı korumaz. İmzalı Unix zamanı kullanılırsa önce işaret sırası dönüştürülür:
`ordered = (mtime as u64) ^ (1u64 << 63)`, ardından `!ordered` kodlanır.
Ham big-endian baytlar geçerli anahtar metni olmayabilir; hex kullanılır.
Eşit mtime'da yol bayt sırası bağı çözer. 16 hex + ayraç sonrası yol için
**1007 UTF-8 bayt** kalır; uzun Windows yolları bunu aşabilir.

Mtime değiştiğinde bu şemada kimlik de değişir: çağıran eski anahtarı saklayıp
**eski anahtarı silme + yeni anahtarı ekleme** işlemlerini aynı transaction'a
koymalıdır. Aksi hâlde eski kopya aramada kalır. Casing, ayraç ve yol
canonicalization kararı çağırandadır; FDB bunları eşitlemez. Bu not Frunner
entegrasyonu veya motor değişikliği içermez.

Uzun anahtarlar kaynakta, SEG05/MET04 anahtar kayıtlarında, seyrek fence'lerde,
delta/görünürlük yapılarında ve varsa linklerde daha fazla byte taşır.
Karşılaştırma ortak prefix kadar ilerler; timestamp eşitliğinde uzun yolun
karşılaştırma maliyeti de büyür. Timestamp değişimi iki mutation/WAL kaydı ve
tombstone üretir. Disk/RAM/ingest bedeli anahtarın kaç yerde bulunduğuna ve
prefix sıkıştırmasına bağlıdır; **anahtar byte'ı başına ölçülmüş sabit bir ms
maliyeti yoktur**. Bu turda anahtar düzeni değiştirilmedi.
