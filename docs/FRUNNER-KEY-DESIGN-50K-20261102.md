# B: Frunner anahtarı ölçüm planı — 2 Kasım 2026

Ölçüm başlamadan yazıldı. Kullanıcı mevcut50k'yı koruyup seçili yazma havuzu
istedi; havuz ayrı collection'da, yeni dosyalar başa eklenmeli. Bu bir Frunner
entegrasyonu veya tüm50k'nın Frunner anahtarına taşınma ölçümü değildir.

Kaynak881756DA… corpus ilk50k; private/ okunmaz. Yeni corpus/DB kopyası veya
yeniden50k ingest yok. Yalnız mevcut sabit FDB/SQLite owner slotları kullanılır.
`texts` içeriği/anahtarları değişmez. Havuz kurulumu/temizliği yalnız kendi
anahtarlarını yazar ve süreleri ölçülen commit'ten ayrı raporlanır.

## Karşılaştırma ve örnekler

- Aynı mevcut donörlerden256 havuz belgesi: dört gövde sınıfı2–4/4–16/16–64/
  64–200KB; dört yol sınıfı40/80/160/300 **UTF-8 bayt**. Yeni corpus dosyası
  üretilmez; orijinal gövde ve runtime oracle revizyon etiketi kullanılır.
- Eşit uzunluklu collection adları: `keybench-numeric` ve `keybench-frunner`.
  Kontrol10 basamak numeric ID; aday `format!("{:016x}|{}", !mtime_u64, path)`.
  Aday ID uzunlukları57/97/177/317B; bir1000B yol uç örneğinde1017B.
  Motorun1024B anahtar ve256KiB entry sınırı değişmez.
- İşlemler: aynı anahtarda gövde güncelleme; silme; yeni belge ekleme; aynı
  path'te mtime değişince eski anahtarı silip yenisini **aynı transaction'da**
  ekleme. Numeric kontrolde de son desen iki mutation/tek commit olur.
- Her tip×yol×gövde hücresinde64 gerçek örnek: tip başına1024, kol başına4096
  commit. Bir1000B uç mtime transaction'ı ayrı N=1 satırında; p95/p99 iddiası
  yok, max/görünürlük ve doğruluk kontrolüne dahil. Gövde/path/key/prepare
  maliyeti ayrı; native commit ve ilk görünürlük p50/p95/p99/max hamları saklanır.
- Dört dengeli blok: FDB numeric / SQLite numeric / SQLite Frunner / FDB Frunner;
  ters sıra; ilk iki sıranın tekrarı. Böylece SQLite da her iki anahtar biçimini
  aynı gövde/işlem dizisiyle ölçer. Toplam16 kol; yeni default her FDB kolunda,
  policy override yok. WAL senkronizasyonu ve SQLite FTS5 trigram ayarları aynı.

## Küçük havuz okuma kontrolü

Havuz collection'ı açıkça filtrelenir. N=50/hibrit200: ortak oracle etiketi
(en yeni50 sırası), gerçek metinde `olduğu`, `üreticisi`, `olduğu birlikte` ve
olumsuz `qzxkvwkeypoolabsent`. Marker sorgusu sıralama/doğruluk içindir;
gerçek içerik aramasının genel performans kanıtı olarak sunulmaz.

Her sorgu için1R idle60;4R idle15 tur/aktör=60;4R+tekli yazıcı en az60 aktif
örnek. Yüklü okumalar yalnız4096 kısa işlem serisinde, yazıcı aktifken sayılır;
bu B turunda yeni16384'lük uzun-seri koşusu yapılmaz. Örnek yetmezse kol eksik
sayılır; sonraki aşamaya geçilmez. p95=ceil(0,95×N), kırpma/yeniden örnekleme yok.
Hazır snapshot ile arama+admission+release wall ölçümü ve oracle ayrı tutulur.

Her insert/yeni mtime, havuzdaki önceki en büyük mtime'dan büyüktür. Ters-mtime
anahtarının byte sırasının başına gelmesi, N'in iki yanı, eşit-mtime path bağı,
update/delete, eski snapshot ve her okumanın tek commit sequence'ine ait olması
bağımsız model oracle'ıyla sınanır. Silinen eski anahtar/revizyon bulunamaz.

## Mevcut eşikler; yeni eşik yok

|Ölçüt|Uygulanacak mevcut kural|
|---|---|
|Kısa tekli commit ve ilk görünürlük, her işlem tipi|p95≤20ms; p99≤50ms; max≤100ms|
|Havuz4R+yazıcı sorgusu|Kendi idle p95≤30ms ise loaded≤30ms; aksi halde loaded≤2×idle|
|Oracle / kaynak CRC / kill-reopen / eski okuyucu|Zorunlu GEÇTİ; hata varsa hemen dur|
|Numeric→Frunner farkı|ms/%/ops ve her çift ayrı; yeni yüzde kapısı uydurulmaz|
|1000B yol|N=1: süre/sonuç ayrı, kısa tekli max100ms kuralı korunur|
|RAM/disk|Süreç private/WS/peak ve disk farkı bilgi; model/oracle belleği motor-only sayılmaz|

N=64 hücrelerde p95 max değildir. Pooled ve hücre/tip/yol tabloları birlikte
verilir; küçük hücrede p99'un max olması gizlenmez. SQLite'a FDB max şartı
yüklenmez. Throughput için ops/s ve commit/s ayrı; okuyucusuz kontrol tavan
sayılmaz. Bu küçük B koşusu D ingest60/idle15 kapılarını yeniden ölçmüş sayılmaz.

## Doğrulama, sonuç ve sınırlar

Havuz hazırlığından önce/sonra mevcut50k `texts` GET/model oracle'ı korunur.
Her havuz snapshot'ı modeline göre kontrol edilir. Process-kill/reopen ve frozen
eski okuyucu aynı kaynakları okur; güç kesintisi iddiası yok. Varsayılan+diagnostic
test,fmt,clippy -D warnings yeniden geçer. Kalıcı biçim/motor değişmez.

Ham loglar `docs/benchmarks`, sonuç `FRUNNER-KEY-50K-20261102.md`. Tüm kötü
sonuçlar, collection hazırlığı ve artık alan raporlanır. Doğruluk kapısı kalırsa
anında dur; performans kapısı kalırsa bu B bloğunu raporla ve sonraki işe geçme.
Önceki normal41 KALDI kaydı kullanıcı kabulüyle korunur; havuzdaki yeni okuma
kapısına geçmiş kararın kabulü otomatik taşınmaz.

100lü,C500drain,2,6sn,WAL handle/File::open,zx iyileştirmesi,format,256KiB,
ön filtre,Blok3/4,C ABI,gerçek-soğuk,uzun snapshot,100k yok.
