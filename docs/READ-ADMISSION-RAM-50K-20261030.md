# Idle tabanı, işçi izin kuyruğu ve motor RAM —30 Ekim2026

**Adım1 ve2 kullanıcı kabulü korunuyor. A kapısı KALDI; D RAM GEÇTİ. Optimizasyon uygulanmadı.**

## Güncel kapılar

|İş|Karar|
|---|---|
|Dört dengeli FDB/SQLite çifti|TAMAMLANDI; aynı frozen ikili/sabit DB,reset yok|
|A tüm41 sorgu|KALDI; FDB çift sonuçları40/41,37/41,39/41,36/41|
|İstenen gerçek cache/permit/CPU profili,E|TAMAMLANDI; diagnostics p95 kabulün yerine kullanılmadı|
|D motor-only private ingest≤60 /idle≤15MiB|GEÇTİ;8/8 kol|
|Oracle/CRC/kill/eski okuyucu|GEÇTİ;kanıtlar aşağıda|
|100lü,B(Frunner anahtarı),C(500 drain),2,6sn uç|BAŞLAMADI:A41/41 olmadı|
|WAL handle/biçim/256KiB/ön filtre/Blok3/4/C ABI/soğuk/uzun snapshot/100k|YAPILMADI|

## Idle tabanı sorusunun cevabı

**Evet, bu kabul düzeninde iki idle tabanı değişiyor.** Üreticisi idle p95 min50,69/max397,12/medyan227,18ms; timmy110,58/435,11/231,06ms. SQLite timmy23,53–38,03ms. FDB akarsu(case13)86,78–382,04ms; ilk çift idle382,04→loaded44,59ms. Bu değişimi yalnız rastgele OS gürültüsüne bağlamak kanıtlanmadı. Aynı DB yeniden kurulmadı ama normal bakım/yazı sequence ve fiziksel yerleşimi ilerletiyor; cache/izin/yerleşim etkileri bu yayılımda birlikte bulunabilir. Idle4R N12/case,p95 nearest-rank=max; kural veya örnek düzeni değiştirilmedi.

[Her çiftin FDB/SQLite kararı,odak ham tablo,min/max/medyan](IDLE-SPREAD-50K-20261030.md) · [Bütün41-case TSV](benchmarks/idle-spread50-20261030-cases.tsv) · [Her query örneği ve bütün kararlar](benchmarks/idle-spread50-20261030-summary.json). Kötü çiftler çıkarılmadı; en yüksek idle tabanı seçilerek gate kapatılmadı.

## Aynı sorgu örneğinde ölçülen gecikme

Diagnostik4R+writer41 matrisinde p95 wall örneğinin **kendi sequence/counter kaydı**:

|Sorgu|Sequence/thread/round|Wall ms|Toplam CPU ms|Permit condvar ms|Delta ms|Posting setup ms|Term mutex ms|Metadata mutex ms|
|---|---|---:|---:|---:|---:|---:|---:|---:|
|üreticisi|92412/2/9|326.041|15.625|289.847|2.764|1.721|0.007|0.700|
|timmy|82944/3/0|177.366|46.875|148.295|0.351|0.947|0.003|0.783|

Bu iki örnekte ölçülen baskın bekleme hibrit doğrulamanın process-wide dört izin için Condvar beklemesidir; wallın yaklaşık%89/%84ü. Bu,CPU oversubscription veya OS scheduler/antivirüs iddiası değildir. Diğer spanlar iç içe/parallel olduğundan ms değerleri toplanmaz. CPU parent+child toplamı,GetThreadTimes nicemlenmesiyle kaba ölçümdür; CPUya göre evre maliyeti ayrı ölçülmedi. Cache kilidini hangi requestin tuttuğu ayrıca izlenmedi. Bütün kaynak-atfı yalnız ölçülen bu koşullarla sınırlıdır.

## Timmy1R/4R ve41/few(E)

Aynı live source/WAL/manifest SHA üzerinde idle timmy:1R41 wall p95 13,90ms,4R41 112,56ms,4Rfew23,75ms. Permit condvar p95 sırasıyla0/97,84/8,81ms; CPU p95 üçünde15,625ms(ölçüm granüler). Loaded41 timmy177,37ms ve loadedfew33,34ms; bunlar ilerleyen yazılı iki ayrı varyanttır,bit-aynı yerleşim karşılaştırması değildir. Few cases16/25/31/33.

Timmy idle4R41 request başına ortalama term lookup hit/miss8,575/19,425;few27,873/0,128. Query-local posting page hit/read841/19 her iki düzende aynı. Cache working-set farkı var;term mutex beklemesi aynı sorgu p95 örneğinde148msyi açıklamıyor. İzin beklemesi doğrudan ölçüldü. Request-local term/metadata edinim spanları uncontended overhead dahil;ayrıca gerçek WouldBlock süresi `lock.cache_wait` ile ölçülüp first-page cache_wait alanına yazılıyor. Writer ownership atfı unavailable; sıfır legacy alanlardan yazar suçlaması yapılmadı.

[Aşama mean/p95,hit-miss/CPU,ham sequence tablosu](QUERY-ATTRIBUTION-50K-20261030.md) · [Sorgu başına TSV](benchmarks/query-attribution50-20261030-requests.tsv) · [Bütün request/thread sayaçları](benchmarks/query-attribution50-20261030-requests.json). Diagnostics instrumentation,request JSON ve oracle etkisi zamanlamayı değiştirebilir; bu koşular frozen kabul kolları yerine geçmez. Global fallback timing sayaçları oracle/worker dışı işlerde de maliyet yaratabilir; ölçülen izin beklemesinin hangi geniş sorgu tarafından tetiklendiği ayrıca nedensel müdahaleyle sınanmadı.

## D: bağımsız motor prosesinde RAM

Control/candidate/candidate/control; ingest tepe private14,87–15,47MiB; aday15,18–15,39MiB. Fresh strict-open idle2,25–2,30MiB;aday2,30MiB. Tüm8 eşik GEÇTİ. Runtime ve tek bounded giriş tamponu dahil,model/oracle/history/profil/katalog dışarıda. Baseline çıkarılmadı. Query/4R+yazıcı RAMi veya saf heap ölçümü iddiası yok. Bu sayılar eski4R benchmark RAMinden çıkarılıp modele atfedilmez; koşullar farklıdır. Ingest için yalnız sabit FDB slotu reset edildi; SQLite korunuyor.

[Sekiz ham sayaç ve süreç sınırı](ENGINE-ONLY-RAM-50K-20261030.md). Her kolun50000GET ve41capped oracleı ayrı proseslerde geçti. Profil/churn modelinde ve son temizD modelinde üçer strict-open witness kill/reopen+eski okuyucu50000GET/41exact GEÇTİ: [profil modeli](benchmarks/engine-only50-20261030-profile-before-reset-recovery.json), [D modeli](benchmarks/engine-only50-20261030-final-recovery.json). Güç kesintisi iddiası yok. Mevcut tam test paketi gerçek mid-WAL/merge/cache kill,kısa kaynak/CRC ve eski fixture testlerini ayrıca kapsar.

## Onay için tek dar öneri — uygulanmadı

Hibrit200 bütçesinden sonra ortak işçi izni hemen alınamıyorsa mevcut cursor/frontier ile çağıran threadde seri doğrulamayı sürdürmeyi öneriyorum; yeni arka plan işçisi açılması gerekmez. Hedef bu örneklerde ölçülen148/290ms izin beklemesidir; bu rakamlar ölçülmüş düzeltme kazancı değil,hedeflenebilecek teorik üst sınırdır. Mevcut veriyle yeni p95 vaat edilemez. Risk,aynı anda daha çok callerın CPU/verifier/cache kaynağı kullanması,negatif geniş taramalar ve yazma throughputunun kötüleşmesidir; global izin/bellek muhasebesi,cancel ve snapshot sırası korunarak ayrı prototipte bütün41 sorgu,CPU veRAM yeniden ölçülmelidir. User onayı gelmeden bu politika uygulanmaz.

## Doğrulama ve açık kalanlar

Varsayılan algoritma,capped/exact semantiği,cache politikası,CRC ve biçim aynı. Değişiklikler diagnostics span/test,benchmark-adapter sayaç raporlaması ve yalın RAM aracıdır. Kabul frozen binary SHA sabit; no-reset çiftleri boyunca derleme/test paralel çalışmadı.249 default test ve157 diagnostic lib testi geçti(bunlar farklı feature koşularıdır,406 ayrı test iddiası yok). fmt ve default/diagnostic clippy `-D warnings` geçti. [Default test](benchmarks/query-attribution50-20261030-v2-tests.log), [diagnostic test](benchmarks/query-attribution50-20261030-final-diag-tests.log), [fmt](benchmarks/query-attribution50-20261030-final-fmt.log), [clippy](benchmarks/query-attribution50-20261030-final-clippy.log), [diagnostic clippy](benchmarks/query-attribution50-20261030-final-diag-clippy.log), [son corpus/fixture/source ve kapsam audit'i](benchmarks/read-admission-ram50-20261030-final-audit.json). İlk instrumentation clippy denemesi TrackedMutex guard tipinin std MutexGuard olmadığı için derlenmedi; mevcut TrackedGuard tipiyle düzeltildi. Başarısız log da korunur. Ölçüm child süreçleri normal exit0;her recoveryde3kill amaçlıdır. Açıklanamayan benchmark kapanması yok.

A kapanmadı;100lü/B/C/2,6sn veWAL handle yok. Reclaim default kapalı;case34/geniş negatif tarama yerleşimi,signed-zero,başka arama artışları,yazma/bakım ve yük altıqueryRAM/disk açık. D ingest/idle kapanması bu başlıkları kapatmaz.
