# Motor-only RAM —50k,30 Ekim2026

**Kapı: GEÇTİ.** Anahtarlar diğer50k benchmarklarındaki10bayt numeric ID biçimindedir; gerçek Frunner anahtarı B henüz yapılmadı. Son temiz modelde3kill/reopen ve eski okuyucuda50000GET/41exact ayrıca geçti: [kanıt](benchmarks/engine-only50-20261030-final-recovery.json).

A/P1–P2 profil sonuçlarından bağımsız ölçüm. OS cache warm;fresh proses cold-cache ölçümü değildir. Aynı881756DA… ilk50k corpus, streamed500 commit, synced WAL, snapshot_reader açık; aktif arama/uzun snapshot yok. Control/candidate/candidate/control aynı native ikilide; kontrol yalnız tam replacement maskeyi seçer, aday artımlıdır. İkili/harness farkı yok. Capped delta ingest/idle sorgusuz olduğundan bu ölçümde çalışmaz. Her ingest öncesi yalnız sabit FDB owner slot reset gerekir: ingest boş DBden başlar. Source corpus/private/ ve SQLite DBye dokunulmaz. Yeni corpus/DB kopyası yok.

Motor ayrı proses: corpus,8MBkatalog/model,oracle,history,query/write profiling parentta veya ayrı doğrulama prosesinde; çocukta yok. Çocukta runtime+motor ve en fazla tek JSON belge+8KiB stdin tamponu var. JSON üst sınırı1MiB; gerçek kaynak belge≤200000UTF8B, decoder geçicileri bu konservatif OS toplamına dahildir. Sayaç Windows PROCESS_MEMORY_COUNTERS_EX: ingest OS lifetime peak private commit, idle fresh strict-open prosesinde current PrivateUsage. Working set ayrı yazılır. Baseline çıkarılmadı; saf Rust heap ölçümü veya bütün bilgisayar RAMi iddiası yok. Bu, motor+zorunlu küçük giriş/runtime için üst sınırdır; benchmark/model toplamı değildir. Input/IPC dolayısıyla ingest süre karşılaştırması yapılmaz.

Ingestte100×500 çağrı, sonra maintain_idle. Temiz idle allocator high-water tutan ingest prosesinden alınmadı: ayrı proses strict-open+snapshot_reader+200ms boş bekleme. OS lifetime peak ingestte bütün callback/worker/maintenance ömrünü kapsar; örnekleme aralığı yok, OS sayacı. idle için current private kullanılır, open lifetime peak ayrıca ham logda. Her kol sonunda model/oracle dış prosesinde50000GET ve41 capped sorgu kontrolü var. Parent,her corpus kaydının CRC ve anahtar eşlemesini denetler.

|Kol|Rol|Ingest peak private MiB|Ingest peak WS MiB|Fresh idle private MiB|Idle WS MiB|60 /15MiB kapısı|
|---|---|---:|---:|---:|---:|---|
|0|control|15.473|20.133|2.270|6.797|GEÇTİ|
|1|candidate|15.387|20.047|2.301|6.816|GEÇTİ|
|2|candidate|15.176|19.875|2.301|6.820|GEÇTİ|
|3|control|14.871|19.465|2.254|6.777|GEÇTİ|

Her kötü kol da tablodadır; tepe değerleri ortalamayla kapatılmadı. [Ham sayaç/program SHA/kapılar](benchmarks/engine-only50-20261030-design.json), [yalıtılmış çocuk kodu](../examples/measure_engine_only.rs). Bu sonuçlar4R+yazıcı tepe RAMini veya sorgu RAMini kapsamaz. RAM kapısı sorgu A kapısını değiştirmez. A41/41 olmadan100lü/B/C/2,6sn yok. Optimizasyon uygulanmadı; format/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k/WAL değişimi yok. Son FDB slotu temiz50k seed,SQLite daha önceki churn durumunda; iki model ayrı kayıtlarda, aynı sequence/yerleşim diye sunulmaz.
