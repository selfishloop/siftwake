# Okunabilir snapshot prototipi — 26 Ekim 2026

Kalıcı biçim/bağımlılık değişmez. Geri kazanım varsayılan kapalı kalır.
Önceki ±%5 arama kapısı kullanıcı tarafından bu adım için kapatıldı;
geniş tarama parçalanması ve case34 OPEN-WORK'te izlenir.

`Database::snapshot_reader()` isteğe bağlı yayın kanalını açar. Okuyucu
hazır `Arc` görünümünü kısa RwLock altında alır; belge okuma, sorgu,
WAL/fsync, merge/rewrite veya görünüm hazırlama bu kilitte yapılmaz.
Başarılı commit ve bakım install sınırlarında yeni görünüm hazırlanır,
sonra pointer değiştirilir. Henüz yayımlanmamış/yarım transaction görünmez.
Snapshot'ın sequence/generation alanları sorgu süresince sabittir.

Run/indeks/kaynak handle'ları Arc ile paylaşılır; eski Run pinleri mevcut
Weak emeklilik düzeniyle korunur. Aktif WAL kimliği ayrıca snapshot pin
listesinde tutulur. Son pin bırakıldığında en son doğrulanmış root ve
güncel writer/job listesi üzerinden GC çalışır. Root yayımı ve GC kendi
mutex'inde sıralanır; bu mutex snapshot edinme kilidi değildir. Son-reader
release/cleanup beklemesi de ölçülür; yalnız admission ölçüp gizlenmez.

Sorgu kodu tek kalır: snapshot, yazıcı metotları dışarı açılmayan özel bir
read-only engine görünümü taşır. Varsayılan streamed commit delta0 bırakır:
payload kopyalanmaz, yalnız descriptor/Arc ve görünürlük bit maskeleri
hazırlanır. Owned API delta bırakırsa belge ve RAM indeksinin o anki içeriği
yayın başına bir kez kopyalanır; dört okuyucu bu tek görünümü paylaşır.
Bu owned maliyeti ayrı raporlanır, düşük RAM'li stream yolu sayılmaz.

Görünürlük maskeleri snapshot yayımından önce hazırlanır. Anahtar/metadata
taraması yayın hazırlama süresine dahildir; okuyucuya ertelenip gizlenmez.
Reader başına maskeler yeniden üretilmez. Query scratch ve sonuçlar reader'a
aittir; bu tasarım tüm proses RAM'ine bir üst sınır vermez.
Mask cache'i görünüm başına64KiB'dir; hazırlıktaki geçici dense maskeler toplam
segment kayıt sayısının yaklaşık1/8 byte'ını ister. Uzun yaşayan farklı epoch
snapshot'ları ayrı görünüm/delta/maskeleri ve emekli dosyaları pinleyebilir.
Bu prototip uzun snapshot ömrü için disk/RAM üst sınırı veya zaman aşımı koymaz.

Yayın hazırlığı I/O hatası verirse yazıcı hata döndürür ve zehirlenir; kaynak
commit'i diske geçmiş olabilir. Kanal son başarılı tutarlı görünümü korur.
I/O hatası sonrası commit'in kesin reddedildiği varsayılmaz; normal kurtarma
ve yeniden açma sözleşmesi geçerlidir. Bu çalışma güç kesintisi iddiası yapmaz.

Ölçüm: aynı mevcut ilk10k, sabit slotlar. Tekli/100'lük commit, en az bir tam
merge ve tamamlanan rewrite; ayrıca rewrite cancellation + eski snapshot.
Her sorgu için single-idle ve 4-reader/no-writer tabanı ayrı verilir;
4-reader+writer p95'i her ikisiyle karşılaştırılır. <1ms tabanda fark≤2ms,
diğerlerinde oran≤2. Writer-only ile 4-reader throughput oranı≥%80.
Admission ve release GC beklemeleri, writer kaynaklı bekleme sayısı/maksimum
ayrı ölçülür. Mutlak100ms artık kapı değildir. Genel RAM/dirty-open ve sürekli
yazmada revision iptali/boşta bakım sınırı Adım3 için açık kalır.
