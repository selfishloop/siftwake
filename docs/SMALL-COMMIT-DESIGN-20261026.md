# Adım3: artımlı yayın ve küçük WAL katmanı

27 Ekim güncellemesi: opt-in yol128 işlem/8MiB çağrı ve256-key/8MiB arka plan
kesitiyle genişletildi. Aşağıdaki32/512 sınırları önceki ölçümün tarihsel
tasarımıdır. [Yeni sınırlar ve başarısız Blok1 kapısı](WRITE-HARDENING-10K-20261027.md)
esas alınır; WAL/reclaim varsayılan kapalı kalır.

Adım2 kullanıcı tarafından kabul edildi; throughput≥%80,4R+writer case22/32,
boşta zx/the zx ve case34 izleme maddeleri kalır. Reclaim varsayılan kapalı.

Kalıcı biçim değişmez: FDBLOG04/TXN2/OFF1 ve mevcut manifest/SEG sürümleri
korunur. Kaynak WAL yine sync_all ile kalıcı olmadan commit görünmez.
Yeni bağımlılık/feature yok; mevcut serde_json raw_value desteği, mevcut
WAL konum çözücüsünün default bounded profilde de derlenmesi için açılır.
0.1 kaynağı ve eski okuyucu sözleşmesi korunur.

Yayın maskesi değişmeyen immutable segment öneki için önceki snapshot'tan
kopyalanır. Yalnız eklenen segment anahtarları önceki segmentleri gizler.
Merge/rewrite yerleşimi değiştiğinde güvenli tam cursor geçişine dönülür;
cache bütçesi yüzünden eksik maske varsa da tam hazırlık yapılır. WAL shadow
kuralları ayrı kalır; eski snapshot'ın maskesi yerinde değiştirilmez.
Mask full/incremental sayısı ve taranan anahtar, prepare/open/clone süreleri
ayrı sayaçtır. Bu sayaçlar toplam proses RAM ölçümü değildir.
Snapshot maskesi WAL shadow'unu da kapsar: aynı segment yerleşiminde yalnız yeni
shadow anahtarları işlenir. Append'te önceki shadow yeni run'a gerekiyorsa
uygulanır; yerleşim değişiminde tam maskeye dönülür. Hazır shadow maskesi olan
snapshot sorgusu aktif500 anahtarı her sorguda disk sözlüğünde yeniden aramaz.

Küçük WAL yolu **varsayılan kapalıdır**; `set_streamed_wal_layer(true)` ile
açılır.50ms eşik/arama kapısı kapanmadığı için otomatik etkinleştirilmedi.
Bu ayar yeni commit'leri etkiler; disable mevcut WAL satırlarını kaybetmez.
Artımlı snapshot maskesi/read-handle kullanımı ve adresli recovery korunur.

Küçük opt-in `commit_streamed`: önce girdinin tamamını eski staging koduyla doğrular.
≤32 işlem, encoded transaction≤1MiB, owned delta yok, toplam shadow≤512,
aktif WAL≤16MiB ve Options watermark henüz aşılmamışsa yalnız WAL append/sync
yapar. RAM'de key + offset/len/CRC/version kalır; payload ve folded metin
tutulmaz, inverted index kurulmaz. Önceki merge drain edilmez. Snapshot yeni
sequence ve WAL konumlarını atomik yayımlar. Aktif WAL append-only kaldığından
eski konumlar sabittir; son snapshot bırakılana kadar eski WAL emekli olmaz.

Arama bu küçük aktif katmanın uygun scope/collection kayıtlarını CRC/kimlik
kontrolüyle WAL'den okur, mevcut Türkçe katlama/AND/alt-dize/fields/range/link
kurallarını doğrular. Immutable posting araması aynı kalır; shadow eski kayıtları
gizler, sonuçlar aynı global key sırasına girer. Aktif kayıt sayısına doğrusal
tarama maliyeti vardır; düşük RAM kazanımı bedelsiz sorgu hızı diye sunulmaz.
Son adayda kayıt başına3312byte+Arc overhead'li volatile özet bulunur; folded
metin tutulmaz. ASCII1–3 harf alt-dizeleri için exact bit üyeliği, daha uzun/
diğer terimler için yanlış negatif üretmeyen byte-gram filtresi kullanılır.
Uzun terim adayları WAL payload'ıyla doğrulanır. Kısa exact üyelik, owned RAM
indeksi gibi doğrulanıp synced olmuş frame'in girişinden üretilir; sayım için
metin çözmek gerekmez. Sonuç payload'ı her zaman WAL CRC/key/version kontrolünden
geçer. Özet disk/cache biçimine yazılmaz; reopen doğrulanmış WAL'den yeniden
kurar. Filtre ve onun CPU/RAM bedeli dirty açılış ve indeksleme ölçümündedir.

Flush gerektiğinde yalnız o zaman WAL kayıtlarından posting hazırlanır;
hazırlık yayınlanmamış yerel indekstedir, hata mevcut sorgu indeksini kirletmez.
Büyük/fallback streamed çağrı mevcut segment yolunu kullanır ve önce worker'ı
bekleyebilir. 512-key/16MiB sınırı çevresindeki tekil flush gecikmesi küçük
commit'in p95'iyle gizlenmemeli; maksimum ve sınır ayrıca raporlanır.

Dirty reopen tam frame/CRC/şema/sequence doğrulamasını sürdürür, WAL adreslerini
geri kurar. Payload/ters indeks/folded metin RAM'de tutulmaz. Büyük eski dirty
WAL watermark'ı aşarsa normal flush yine açılışta çalışabilir; dirty500 ölçümü
bütün olası WAL boyutları için açılış garantisi değildir.

Salt okunur snapshot WAL handle'ı yazıcının handle'ından bağımsız açılır.
Aktif WAL değişmiyorsa sonraki yayın mevcut read handle'ını çoğaltır; her
yayında pathname open yapılmaz. Yalnız explicit-offset read kullanılır;
yazıcının append cursor'ı ile handle paylaşılmaz. Açılış/ilk WAL değişimi
gerçek read-only open yapmaya devam eder. Pin/GC ve kök doğrulaması korunur.

Atomik `commit_streamed` üst sınırları:1–10.000 işlem; encoded entry≤256KiB,
key admission≤1MiB (key byte'ları+işlem başına512), indeks admission≤4MiB
tahmin, frame≤64MiB. İlk aşamada küçük çağrı için indeks kurulmadığından
indeks allowance o yolun kabul kriteri değildir. Bu üst sınırlar her10.000
belgenin sığacağı garantisi değildir; belgenin gram çeşitliliği belirleyicidir.

`commit_streamed_chunks` **atomik değildir**:≤32 işlem/≤512KiB encoded girişli
parçalarla ilerler. Önceki parçalar daha sonraki girdi/guard/I/O hatasından geri
alınmaz. Sonuç `atomic=false`, ack sequence listesi, her parçanın işlem sayısı ve toplam ack mutation sayısıdır.
Hata `ChunkedCommitError.committed` içinde o ana kadar kesin ack edilmiş
parçaları taşır; başarısız parçanın I/O hatasında kalıcı olup olmadığı normal
WAL hata sözleşmesine tabidir. Çağıran gerçek guard/kimlik ve yeniden deneme
politikasını belirler. Helper en fazla küçük bir input grubu tutar; owned API'nin
tam5000 belgeli Vec'ini toplamaz. Tek bir büyük/geçersiz belgeyi küçültemez.

Sürekli100'lük/fallback yazılar revision worker'ını iptal edip işi boşta
bakıma erteleyebilir. Küçük WAL çağrıları worker'ı drain/iptal etmez; install
poll/wait/maintenance ile olur. Motor otomatik bir idle timer servisi kurmaz.
Boşta1sn sonra explicit maintenance ile yetişme ölçülür; entegrasyon/servis
zamanlayıcısı bu repository kapsamı değildir. Sürekli yazıda reclaim ilerlemesi
ve uzun snapshot ömründe disk/RAM üst sınırı garantisi yoktur.
