# Tam belge kopyasını WAL referansına dönüştürme deneyi

**Hızdan ödün vermeyen bellek çözümü olarak reddedildi.** Existing
experimental-wal-delta default kapalı kaldı. Tam belge RAM'de tutulmayınca
bellek azaldı, fakat flush edilmemiş kayıtların tam belge getirme/arama yolu
belirgin yavaşladı. Normal motor yolu ve kalıcı codec değiştirilmedi.

## Uygulanan prototip

Normal yolda delta owned StoredDocument ve folded arama metnini tutar.
Aday mevcut WAL-delta feature'ında yalnız key/version ve native payload
offset/len/checksum tutar; folded arama metni yine RAM'dedir. Tam belge
gerektiğinde gerçek WAL bytes okunur, CRC/native schema/key/version
doğrulanıp çözülür. Bu mevcut deneyin ölçümüdür, yeni feature değildir.

Bu feature'daki encode_located sadeleştirildi: payload Vec'i oluşturup
frame Vec'ine kopyalamak yerine ortak format::encode ile tek frame üretilir;
locations bu frame'in payload aralığından hesaplanır. Artık kullanılmayan
ikinci frame helper kaldırıldı. Pending flush varsa frame limit hesabı önce,
gerçek encode_located flush sonrası yapılır. Append/sync sonrası frame drop
önceki yaşam süresi düzeltmesini korur. Non-WAL-delta yolu değişmedi.

Encoded bytes ve native row offsets mevcut testte normal encode/replay ile
birebir eşit olmalıdır. Sync, sequence, atomic WAL frame, CRC/schema ve
poison/error davranışı korunur. Source/index/WAL/manifest codec veya protocol
değişikliği yok,0.1 fixture korunur. Bu encode iyileştirmesinin eski WAL-delta
build'e karşı ayrı paired hızı ölçülmedi; feature toplam sonucu aşağıdadır.

## 10k paired yazma ve bellek

Üç dönüşümlü process/variant. Kontrol current merge-postings+delta-bitmaps;
aday bunlara yalnız experimental-wal-delta ekler. Grouped-read, builder-budget
ve diagnostic kapalı. Payload/shared-doc feature'ları ayrıca açılmaz.
Hash/artifact prefix `benchmarks/copy-reduction-10k-20261013`.

| Ölçüm | Normal medyan | WAL referansı medyan |
|---|---:|---:|
| Yazma | 29,271s | 29,853s |
| Son bakım | 1,817s | 2,039s |
| Yazma + son bakım | 31,088s | 31,892s |
| İndeksleme observed private peak | 111,13MiB | 83,87MiB |
| Bakım observed private peak | 102,07MiB | 76,11MiB |
| Tüm ingest prosesinin OS peak working set'i | 114,81MiB | 87,38MiB |
| Boşta private | 20,71MiB | 20,71MiB |
| Tutulan DB dosyaları | 1.513.744.522B | 1.513.744.522B |

İndeksleme private~%24,54, bakım private~%25,43 düşük; üç çiftte de düşük.
Toplam süre medyanı%2,59 yüksek; üç çiftin ikisinde yavaş. Boşta ölçüm delta0
olduğu için tam belge referansının birikmediği durumdur; idle RAM kazancı yok.

| Çift / sıra | Normal toplam s | Aday toplam s | Toplam fark |
|---|---:|---:|---:|
| 0: normal → aday | 27,719 | 32,870 | +%18,58 |
| 1: aday → normal | 31,316 | 31,892 | +%1,84 |
| 2: normal → aday | 31,088 | 30,676 | -%1,33 |

Aynı existing corpus ilk10k:264.842.858 text byte,2.057–51.197B/doc,
sekiz Gutenberg kitabının benzersiz/örtüşen pasajları. Yeni corpus/download
yok. Sabit FDB slotu yalnız ingest öncesi sahiplik/path kontrolüyle sıfırlanır;
DB kopyaları saklanmaz, SQLite slotu değiştirilmez.500 belge/transaction,
20 synced WAL commit; input CRC/JSON decode, indeksleme/commit ve40 batch-edge
tam belge kontrolü ingest timer içinde; son flush/merge ayrı. Her koşu sonunda
bütün10k belge/version ve dört-query total/ilk50 full-page oracle doğrulandı;
her aday eski normal reader ile ayrıca tam doğrulandı.

OS cache eviction yok;20ms private/WS poll,100ms phase poll, kısa peak'ler
kaçabilir. OS peak WS bütün ingest prosesini kapsar. Engine quota veya toplam
sistem file cache değildir. MiB1048576B. Idle strict-open+5s bekleme, search
warmup yok. SQLite, cold cache, izole sistem yükü/concurrent bakım ölçülmedi.

Her koşu same10k,sequence20,generation31,delta0,5 run;
source267.900.709B,index353.242.581B,WAL45B,19.221.340 posting. Disk fark0B.
Summary `tools/summarize-stream-checkpoints.py` ile yalnız loglardan üretildi.

## Flush edilmemiş delta araması

Existing corpus'un ilk1000 belgesi; aynı128MiB/2000key options ile flush yok.
Reverse ingest,40 delete/reinsert ve WAL'den reopen.6 query ×3 state =18case;
her query1 untimed warmup+10 timed sample/process, üç process/variant,
pooled30 sample/case, nearest-rank p95. Bağımsız scalar fold/contains oracle
total/order/ilk50 full StoredDocument/version'ı API timer dışında doğrular.
Normalize, posting lookup, verification ve full-document hydration timer içinde.
Caller/oracle1000 belge tutar; bu alt örnekte proses RAM ölçülmedi.

**1 p95 düşük /17 yüksek.** Pozitif sonuç getiren16 case'in tamamında aday
per-process p95 üç eşleşmede de yavaştır. Negatif istanbul ilk state'te küçük
düşüş; negatif updatedunique küçük artış gösterdi. Bu farklar önceki38-case
kullanıcı dalgalanma kabulüyle otomatik kapatılmaz; burada sistematik hız kaybı var.

| State / sorgu | Normal p95 ms | Aday p95 ms | Hızlı çift |
|---|---:|---:|---:|
| delta / the | 0,6201 | 1,6749 | 0/3 |
| delta / quixote | 0,0587 | 0,1759 | 0/3 |
| delta / he | 0,5826 | 1,3895 | 0/3 |
| delta / the man | 0,8680 | 1,7963 | 0/3 |
| reused / updatedunique | 0,0698 | 0,3926 | 0/3 |
| reopened / quixote | 0,0376 | 0,2624 | 0/3 |
| reopened / the man | 1,0793 | 2,0985 | 0/3 |

Folded metin RAM'de kalsa da döndürülen tam belgeler WAL read+CRC+decode
gerektirir. Bu iş API timer içindedir. Tam attribution phase profili alınmadı;
tablo bu combined read/hydration yolunun hız kaybını gösterir. Her workload
ve her get için genelleme yapılmaz; cold-cache kaybı ayrıca ölçülmedi.

Arama aracı başlangıçta resident_documents1000 varsaymıştı. WAL-delta'da
docs1000/delta_keys1000 iken resident_documents0 doğrudur. Harness bu iki
sayacı ayıracak şekilde düzeltildi; failed ilk candidate log sorgu zamanlaması
üretmedi ve sonuçlara karıştırılmadı. Eski ilk control log da yeni paired
summary'ye katılmadı. Yeniden koşular ayrı `copy-reduction-query-fixed-20261013`
prefix'inde; executable hash'leri `copy-reduction-query-fixed-executables.json`.
Candidate harness yeniden derlendi, core WAL kodu değişmedi. Kontrol aynı
core/feature setinin WAL-delta kapalı build'idir.

`tools/summarize-delta-dictionary-queries.py` prefix parametresiyle yalnız
loglardan summary üretir. Normal1000 delta ve WAL1000 delta source version
kontrolleri, update/delete ve reopen tam sayfaları geçti. Arama alt örneği
sonunda sabit FDB slotu normal yolla aynı10k corpus'a geri döndürüldü.

## Karar ve takip

Default kapalı kalır. Belge kopyalarını kaldırmanın RAM etkisi ölçüldü,
fakat her get'te WAL okuma hız şartını geçemedi. RAM'de aynı belgeyi iki
biçimde tutmadan hızlı tam belge getirme tasarımı hâlâ açık; bu tur elde
edilmiş çözüm olarak sunulmaz. Eski nesil disk ve concurrent yük de açık.
SQLite bu tur ölçülmedi;8–10MiB seviyesine inildi iddiası yok.

Default/all-features `cargo test --all-targets`, fmt --check ve default /
all-features all-targets Clippy -D warnings geçti. Native encoder byte/offset
eşitliği, kesilme/bozulma, acknowledged commit/merge process-kill ve0.1 fixture
testleri korunur. Normal yola dönüşte full10k belge/version+dört-query oracle,
ardından38-case full-page/version correctness kontrolü geçti; son38 kontrolü
performance_not_measured olarak kayıtlıdır. Son release examples normal
merge+bitmap, WAL-delta/grouped/diagnostic kapalı build olarak bırakıldı.
Logs `copy-reduction-{tests-default,tests-all-features,fmt,clippy,clippy-all-features}.log`,
`copy-reduction-normal-{verify,matrix}.jsonl`. Motorun normal RAM modeli aynı
kalır; bu deneyi hız şartı geçilmiş uygulama diye sunmuyoruz.
