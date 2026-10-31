# Hibrit izin bulunamazsa seri frontier — 30 Ekim2026

**Son karar: KALDI; prototip varsayılan yapılmadı, bu adımda duruldu.** İzin beklemesi
kalkıyor ve üreticisi/timmy hızlanıyor. Buna rağmen dört çiftin hiçbirinde 41/41
sağlanmadı; yazma throughput'u düştü, geniş zx taraması ve yüklü motor belleği arttı.
Kapı/örnek düzeni değiştirilmedi. Kabul edilmiş merge maskesi ve capped delta
kazanımları korunuyor. Bu deney onların geri alınması değildir.

## Kapanış kapıları

|Kapı|Kontrol / önce|Aday / sonra|Karar|
|---|---|---|---|
|A: dört çiftin her birinde 41/41|38, 37, 38, 40|34, 36, 38, 36|KALDI|
|Yazma korunum ≥%75 ve SQLite üstü|%96,163|%85,940; SQLite %82,739|Bu sayısal kapı GEÇTİ|
|Yazma kötüleşmesin|331,779 işlem/sn|297,286 işlem/sn, −%10,396|KALDI|
|Geniş tarama kötüleşmesin: ayrı 4R zx p95|894,895 ms|994,314 ms, +%11,110|KALDI|
|CPU kötüleşmesin: profilli loaded zx parent+child ortalaması|472,458 ms/sorgu|522,559 ms/sorgu, +%10,604|KALDI; profil kolu|
|D ingest peak private ≤60 MiB|15,133 / 15,734|15,438 / 14,523|GEÇTİ|
|D fresh temiz idle private ≤15 MiB|2,281 / 2,273|2,297 / 2,250|GEÇTİ|
|4R+yazıcı RAM artmasın, eş 1|32,371 MiB peak private|36,238 MiB, +3,867|KALDI|
|4R+yazıcı RAM artmasın, eş 2|30,770 MiB peak private|33,504 MiB, +2,734|KALDI|
|Oracle/cancel/eski snapshot/sequence/CRC/izin muhasebesi|Referans|Eşit|GEÇTİ|
|Tam test / diagnostic lib|—|251 / 159 geçti|GEÇTİ|
|fmt / default ve diagnostic all-targets clippy -D warnings|—|Hatasız|GEÇTİ|
|Kill/reopen + eski okuyucu|—|Reset öncesi/sonrası 3'er kill; oracle eşit|GEÇTİ|

Eski uzun seri yazma kapısı da geçti: aday loaded pooled p95/p99/p99,9
**4,741 / 14,873 / 27,978 ms**; max **326,974 ms** yalnız bilgi.
100 ms üstü **10/65.536 = %0,01526**, kontrol de 10/65.536. SQLite loaded
10,087 / 20,538 / 194,053 ms, max 950,129 ms; >100 ms 85/65.536.
Max uçlara bu tur kanıtsız neden atanmadı.

## Kabul kolları ve kazanımlar

|Çift|Kontrol geçti/kaldı|Aday geçti/kaldı|SQLite geçti/kaldı|
|---|---|---|---|
|0|38/3|34/7|40/1|
|1|37/4|36/5|40/1|
|2|38/3|38/3|38/3|
|3|40/1|36/5|39/2|

Ana kabulde tek frozen FDB ikilisinde thread-bound scope before=eski blocking,
after=tek try/seri seçer. Dört sıra before/SQLite/after, ters sıra ve bunların tekrarı.
41 sorgu; 1R idle 12 tur, 4R idle 3 tur ve 16.384 tekli commit. Her sorgu model
oracle'lı. Idle N=12 olduğu için p95=max. Idle≤30 ms ise loaded≤30 ms, aksi halde
loaded≤2×idle kapısı aynen uygulandı. Kontrolün de 41/41 olmaması gizlenmedi;
bu durum aday için geçer sayılmadı ve tüm farkların nedenini tek başına kanıtlamaz.

İlk çiftte üreticisi loaded 266,762→75,802 ms, timmy 178,703→44,716 ms.
Aday timmy idle=23,868 ms olduğundan sınır=30 ms, KALDI. Üreticisi aday son
çiftte idle35,023→loaded72,891 ms ile70,046 ms sınırını aştı. Hem mutlak
iyileşmeler hem geçmeyen kapılar raporlandı; uygun taban seçilmedi.

[Bütün 41 sorgu: her çiftte 1R/idle/loaded p95, geçti/kaldı, üreticisi/timmy/akarsu
ham satırları, eski raporla idle min/medyan/max ve bütün yazma tail/SQLite oranları](HYBRID-SERIAL-ACCEPTANCE-50K-20261030.md).
[Tüm sorgular TSV](benchmarks/hybrid-serial50-20261030-cases.tsv).

## CPU, 1R ve geniş tarama

Profilli loaded üreticisi permit p95 138,231→0 ms ve wall p95 189,254→70,028 ms;
timmy permit180,060→0 ms, wall199,347→34,683 ms. Profil kabul p95 yerine
kullanılmadı. Aynı kolda zx CPU472,458→522,559 ms ve wall874,198→993,959 ms.
Parent+child GetThreadTimes kısa aralıkta nicemlenir; verilen CPU çok sayıda
request ortalamasıdır. Ayrı process CPU blokları da parent+temporary workers
toplamını ölçer; oracle CPU aralığının dışındadır. Bütün sayısal artışlar korunur.

Zx ortalama doğrulanan aday49.735→49.720; payload istek sayısı da bu düzeyde.
Profil ortalamasında payload-read wall toplamı359,812→461,102 ms, metadata-page
çağrısı1.877→2.944. Nested/child span'lar örtüşür; toplam wall yüzdesi gibi
toplanamaz. Bu veriler maliyeti gösterir; CPU artışının tamamının tek nedeni,
scheduler veya antivirüs olduğu kanıtlanmadı. İkinci düzeltme uygulanmadı.

Her profilli fallback request'te try≤1, fallback≤1, request thread sayısı1 ve
grant0 doğrulandı. 1R bütün41 sorguda fallback0: izinli eski hibrit iş bölümü
çalışıyor. Kod yolu eşitliği bit-aynı zamanlama iddiası değildir; 1R p95 ve CPU
tablolarındaki iki yönlü değişimler de verildi. Read-only aux/CPU ABBA kolları
boyunca canlı kaynak/WAL/manifest SHA'ları eşit.

TR case34 **kendisine çıkacağını**,35–37 **ve/if/id**,38 **zx**.
Eski ishmael kapsam dışı. **the zx** ayrı metadata oracle/CPU sorgusudur;
ana41 ve kapı kararında yoktur. 4R the zx356,235→258,403 ms iyileşti; fakat
zx894,895→994,314 ms kötüleşti. Kısa capped ve/if/id tam negatif tarama diye
sunulmadı. [42 CPU satırı, 1R/4R aux ve bütün request profilleri](HYBRID-SERIAL-CPU-50K-20261030.md).

## D: yalın motor RAM

Native RAM child'ında corpus/katalog, model, oracle, history veya profil yok.
Runtime, bir giriş belgesi,8KiB stdin buffer, loaded modunda41 küçük sorgu tanımı
ve API sonuçları dahildir. OS private/peak private commit ve working set ayrı
verildi; tahmini pay çıkarılmadı. Before/after/after/before ingest için yalnız
sabit FDB owner slotu sıfırlandı: temiz ingest gerekiyor; kabul/CPU önceden bitmişti.
Her kol sonrasında dış proseste50k GET ve41 sorgu oracle'ı geçti.

60/15MiB ingest/fresh-idle kapıları geçti. 4R+yazıcıdaki iki artış bütün motor
prosesinin yüksek su seviyesidir; yalnız fallback nesnelerinin tahsisi değildir.
Aday daha çok sorgu tamamladı ve snapshot örtüşmeleri farklı olabilir; bu bir
RAM geçer kararı değildir. Benchmark model/oracle/history belleği D diye sunulmadı.
[Bütün16 ölçüm, post-loaded temiz idle,WS ve aktör sayımları](HYBRID-SERIAL-RAM-50K-20261030.md).

## Doğruluk, süreç ve kaynak denetimi

Yeni test620 false-positive prefix ardından eşleşmelerle bütün4 izni tutar:
N50/N200, update/delete, full-shadow, eski snapshot/sequence serial oracle eşit.
Seri201. doğrulamada cancel `Cancelled` döndürür; izin0 kalır. Mutex tutulurken
try beklemeden döner; pre-cancel korunur. Eski admission iptal testi, signed-zero
owned/light semantiği ve seçili WAL payload CRC testleri geçer. Önceki normal
signed-zero tutarsızlığı bu turda birleştirilmedi.

251 default all-targets test geçti (15 ignored; helper/önceden ignored testler),
diagnostic lib159 geçti (13 ignored). Fmt ve default/diagnostic all-targets
clippy `-D warnings` geçti. Paket append/sync/snapshot ack, flush build/yayım,
rewrite iptal/pin, cache değiştirme, kesilme/bozulma ve process-kill testlerini içerir.

Gerçek50k modelde reset öncesi/sonrası3'er kill/reopen ve frozen eski okuyucu
50k GET/41 exact oracle geçti; canlı kaynak hash'ları eşit. Bu altı kill
**strict-open tanığı sonrasında**: commit ortası/güç kesintisi iddiası yok.
Commit ortası senaryoları yukarıdaki test paketindedir.

Kabul/ek/RAM controller'larında15/20/24 child normal exit0. Açıklanmayan kapanış
veya oturum kaybı yok; recovery kill'leri aktör PID ve tanıkla amaçlı kayıtlı.
881756DA… corpus ve0.1 `.fdb/.ready` hash'ları korundu; private/ okunmadı.
Ana ikilinin SHA'sı kapanışta aynı; ikincil CPU/diagnostic/RAM ikilileri ayrı
SHA'larla kayıtlı. [Son test/SHA/süreç/uyumluluk denetimi](benchmarks/hybrid-serial50-20261030-final-audit.json).

## Duruş

Üretim varsayılanında `REFERENCE_WAIT=true` (eski bekleme) korundu. Yeni prototip
yalnız hidden diagnostics `hybrid_wait_reference_for_diagnostics(false)` ile
açılır; policy diske yazılmaz. Exact/serial ve mevcut opt-in semantikleri aynı.
Global izin/bellek muhasebesi, CRC, full-shadow, snapshot sırası/pin ve biçim değişmedi.

A41/41 ve yazma/geniş tarama/CPU/RAM kötüleşmeme koşulları kapanmadı:
**4R+100'lük, Frunner anahtarı,500 drain ve2,6sn uç profiline geçilmedi.**
Reclaim default kapalı. Case34/geniş tarama yerleşimi, signed-zero,
yazma/bakım, sorguRAM/disk ve eşzamanlı kabul açık. Biçim/256KiB/ön filtre/
Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k/WAL handle düzeltmesi yok.
[OPEN-WORK](OPEN-WORK.md) güncellendi. Yeni eşik/tolerans veya ikinci optimizasyon yok.

---

Aşağıdaki metin deney öncesi tasarım kaydıdır; gelecek zamanlı ifadeler yukarıdaki
kapanış raporuyla tamamlandı. Yeni ölçüm planı değildir.

## Uygulama ve muhasebe

`WORKERS.try_lock()` bir kez çağrılır. Mutex meşgulse veya boş izin0 ise None; poison hata olarak yayılır. İzin varsa eski `min(requested,free)` sayısı düşülür, aynı `Permits` RAII guard aynı sayıyı bırakır. Loop/sleep/retry yok. Handoff öncesi source frontier sayısı eski task sayısına eşittir. İzinli1R yolunda aynı hibrit per-source doğrulama/işçiler ve merge kullanılır.

İzin yoksa handoff devreden çıkar, mevcut `cursors/heap/delta/hits/verifier` ve8MiB seri cursor allowance ile aynı seri döngü sürer. Parallel task/prefix listesi veya worker-specific verifier kurulmaz; yeni thread açılmaz. Sonraki adayda yeniden izin denenmez. Mevcut seri caller, ilk200 aday ve normal serial API gibi parallel izin havuzunun dışındadır; fallback sahte/0 izin üretmez. Bu, toplam aktif caller CPU baskısını sınırlamaz: onaylanan ve ölçülecek risk budur. Bellek allowance,CRC,cancel,key sırası,snapshot/sequence ve exact/serial akışları değişmedi.

## Doğruluk

İlk8 ilgili test geçti: yeni try mutex tutulurken beklemez/pre-cancel korunur; bütün4 izin tutulurken≥200 false-positive prefix sonrası N50/N200,delta/source güncelleme/silme,eski snapshot/sequence oracle eşit;seri201. doğrulama sırasında cancel hata döndürür;havuz0 kalır. Eski parallel admission beklemesinde cancel testi korunur. İzinlerin tamamını tutan iki test test-only mutexle ayrıldı;iki testin kısmi izin tutup birbirini beklemesi engellenir. Üretim havuzunda değişiklik yok. Signed-zero ve seçili WAL CRC testleri de korunur. Tam paket/fmt/clippy/kill/eski okuyucu kapanışta raporlanacak.

## Ölçüm planı

Aynı frozen FDB ikilisinde hidden,thread-bound,volatile benchmark scope: `before` eski blocking handoff; `after` yeni try/seri. Varsayılan exact/serial bu kontrolden etkilenmez. SQLite aynı frozen referans. Dört matched çiftin sıra düzeni before/SQLite/after,after/SQLite/before ve tekrarı;iki sıra eşit. Her rol childı eski kabul düzeninin1R idle12round,4R idle3round,41 sorgu,N50,hibrit200 ve16384 tekli writer yükünü aynen çalıştırır. Her sorgu oracle. Ayrı before/after/SQLite okuyucusuz16k control kolları throughput paydasını verir.

Sabit50k DBler yeniden kurulmadan ilerler;model yedekleri yalnız JSON metadata,DB kopyası yok. Yazma/bakım fiziksel yerleşimi ve sequence'i değiştirebilir;bu sınır belirtilir. Loglar `docs/benchmarks/hybrid-serial50-20261030-*`. Kabul binary `output/hybrid-serial-20261030.exe`,writer/query oracle source881756DA…ilk50k. Derleme/test timed kollarla paralel yapılmaz.

TR case34 kendisine çıkacağını,35–37 ve/if/id,38zx. Kullanıcı netleştirmesiyle eski ishmael130/1 kapsam dışı;the zx ana41e eklenmeden ayrı metadata oracle/CPU taramasında ve kapı dışında ölçülecek. CPU parent+child,1R ve loaded before/after ayrı;RAM D model/oracle/history/profil dışarıda,fresh-ingest ve idle60/15MiB,4R+writer peak ayrı. Yeni performans eşiği eklenmeyecek;kötü sonuçlar saklanmayacak.
