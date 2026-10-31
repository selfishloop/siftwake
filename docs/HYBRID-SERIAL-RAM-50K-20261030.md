# Hibrit seri devam: motor belleği — 30 Ekim 2026

Aynı normal ikili, before/after/after/before sırası. Her ingest için yalnız sabit FDB owner slotu sıfırlandı; kabul ve CPU koşuları bundan önce tamamlandı. Aynı 881756DA… kaynağının ilk 50.000 belgesi, 100 × 500 akışlı commit. SQLite slotu ve corpus değişmedi.

Ölçülen sınır Windows motor **prosesidir**: runtime, en fazla bir giriş belgesi, 8 KiB stdin buffer ve API sonucu dahildir. Corpus/katalog, model, oracle, history ve profil ayrı ebeveyn prosestedir. Bellekten tahmini pay çıkarılmadı. Loaded çocukta dört okuyucu ve bir yazıcı aynı prosestedir; 41 küçük sorgu tanımı ve aynı anda yaşayan API sonuçları da dahildir. Okuyucular hibrit N=50/bütçe 200; yazıcı 16.384 tekli değişiklik yapar. Oracle ölçüm çocuğunda çalışmaz: her kol sonrasında bütün 50k GET ve 41 sorgu bağımsız doğrulanır.

Private = OS PrivateUsage; peak private = OS PeakPagefileUsage (private commit yüksek su seviyesi). Working set ayrıca gösterilir. 60 MiB kapısı ingest tepesine, 15 MiB kapısı temiz boşta private değerine uygulanır. Loaded için yeni mutlak eşik eklenmedi.

|Kol|Rol|Faz|Private MiB|Peak private MiB|WS MiB|Peak WS MiB|Kapı|
|---|---|---|---:|---:|---:|---:|---|
|0|before|ingest|11.988|15.133|16.207|19.898|GEÇTİ|
|0|before|idle|2.281|2.598|6.809|6.934|GEÇTİ|
|0|before|loaded|22.141|32.371|26.859|37.098|bilgi|
|0|before|postloaded_idle|2.547|2.969|7.078|7.301|GEÇTİ|
|1|after|ingest|11.836|15.438|16.375|20.215|GEÇTİ|
|1|after|idle|2.297|2.516|6.793|6.867|GEÇTİ|
|1|after|loaded|27.289|36.238|31.770|41.223|bilgi|
|1|after|postloaded_idle|2.461|2.941|6.992|7.270|GEÇTİ|
|2|after|ingest|8.500|14.523|13.504|19.359|GEÇTİ|
|2|after|idle|2.250|2.531|6.793|6.867|GEÇTİ|
|2|after|loaded|24.652|33.504|29.090|38.219|bilgi|
|2|after|postloaded_idle|2.438|3.199|7.004|7.406|GEÇTİ|
|3|before|ingest|9.469|15.734|14.438|20.496|GEÇTİ|
|3|before|idle|2.273|2.613|6.809|6.934|GEÇTİ|
|3|before|loaded|19.863|30.770|24.598|35.406|bilgi|
|3|before|postloaded_idle|2.543|3.020|7.074|7.355|GEÇTİ|

## Aynı proses 4R + yazıcı

|Dengeli eş|Before peak private MiB|After|Fark MiB|
|---|---:|---:|---:|
|0 / 1|32.371|36.238|+3.867|
|3 / 2|30.770|33.504|+2.734|

Bu fark bütün 4R+yazıcı prosesinin yüksek su seviyesidir; yalnız seri fallback nesnelerine ait tahsis farkı değildir. Önce/sonra okuyucu ilerlemesi ve yaşayan snapshot sayısı farklı olabilir. Kabul koşusunun katalog/model/history içeren bellek rakamları D kapısı olarak kullanılmadı.

60/15 MiB ingest/temiz-idle kapıları GEÇTİ. Buna karşılık her iki 4R+yazıcı eşinde peak private yükseldi: RAM kötüleşmeme koşulu KALDI. Loaded satırlarının bilgi etiketi yalnız yeni bir mutlak loaded eşik eklenmediğini belirtir; göreli artışı geçer saymaz.

|Kol/rol|Her okuyucunun sorgu adedi|Yazma adedi|
|---|---|---:|
|0/before|[654, 676, 654, 682]|16384|
|1/after|[896, 876, 841, 859]|16384|
|2/after|[877, 876, 909, 882]|16384|
|3/before|[654, 645, 702, 686]|16384|

İlk reset öncesi ve son kol sonrası ayrı process-kill/reopen, canlı kaynak hash ve eski okuyucu kontrolleri çalıştırıldı. Native RAM ölçümü sorgu gecikmesi kabul koşusunun yerine geçmez; kontrolün eski bekleme politikası ve adayın tek try/seri politikası thread-bound scope ile seçildi.

[Ham RAM ve SHA/kol dizisi](benchmarks/hybrid-serial-ram50-20261030-design.json) · [Makine okunur tablo](benchmarks/hybrid-serial-ram50-20261030-summary.json) · [Süreç olayları](benchmarks/hybrid-serial-ram50-20261030-lifecycle.jsonl).
