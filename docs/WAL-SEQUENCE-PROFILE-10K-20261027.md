# Sequence eşleştirmeli yazma profili — 27 Ekim 2026

**Üç uzun, profil kapalı koşuda gecikme kapısı GEÇTİ:0/12288 commit>50ms.**
Eski419,587ms örneği bu adımda yeniden üretilemedi; onun kök nedeni kesinleşti
veya düzeltildi denmez. Koşullu worker-handle optimizasyonu uygulanmadı.

## Eşleştirme

Önceki normal binary, sequence946/915. commit419,587ms ve aynı koşudaki
max snapshot hazırlığı408,446ms değerini kaydetmişti; bunları bire bir eşleştiren
faz kaydı yoktu. Eski log geriye dönük doldurulmadı.

Yeni opt-in `start_write_profile(capacity)` her commit girişindeki sequence'i
ve hedef işlem sequence'ini kaydeder. Commit içindeki snapshot maskesi,
WAL context hazırlığı, gerçek pathname open veya handle clone, state kopyası,
snapshot hazırlığı/pointer/eski görünüm bırakma ve prefix join/install ayrı
fazlardır. Her faz görünür sequence ve aktif WAL ID taşır. Böylece örneğin
425. sequence'e giden commit'in önce424'ü yeni WAL ile yayımlaması kaybolmaz.
Reddedilen input'ta hedef sequence yalnız prospective'tir; bu ölçümün bütün
4096 çağrısı başarılı olduğundan başarılı logla bire bir eşleştirilmiştir.

Kayıtlar yalnız writer thread'inde, frame sayısıyla sınırlı RAM halkasındadır.
Varsayılan kapalı; take ile kapanır. Capture kaynak/snapshot/WAL pinlemez.
Global `diagnostic-timing` açılmadı; query/document döngülerine kilitli sayaç
eklenmedi. İç içe fazlar örtüşür; süreleri toplayıp elapsed/CPU yüzdesi yapmak
yanlıştır. Capacity8192, her koşuda4096 frame; kayıp0.

## Aynı10k /4 okuyucu +1 yazıcı

Her kol aynı mevcut ilk10k'dan kurulup aynı1100 donor güncellemesiyle hazırlanır.
4096 adet tekli update,38-case döngüsü, writer bitene kadar4 okuyucu; normal
merge/revision interlude aynı harness'tedir. Tam model/payload/version ve her
okumada commit sınırı oracle'ı vardır. Sıcak OS cache; CPU/background/power
izolasyonu/cache tahliyesi yoktur. Yeni corpus/DB kopyası/kalıcı biçim yok.

| Kayıt | n | En kötü commit ms | >50ms | Max snapshot pathname open ms |
|---|---:|---:|---:|---:|
| Profil a1|4096|24,8015|0|15,6025|
| Profil a2|4096|21,9181|0|12,2081|
| Profil a3|4096|30,2436|0|17,0826|
| Profil kapalı r0|4096|29,0220|0|kapalı|
| Profil kapalı r1|4096|21,6879|0|kapalı|
| Profil kapalı r2|4096|26,6412|0|kapalı|

Üç profil koşusunda15'er gerçek yeni WAL open kaydı vardı. Örnek a1: ordinal394,
commit sequence425,total24,8015ms; yeni WAL67 üzerinde görünür sequence424'ün
pathname open'ı15,6025ms, snapshot hazırlığı16,6221ms,prefix install21,3581ms.
Ardından görünür425'in aynı WAL clone'u0,0022ms. Parent kapsamlar çocukları
içerir; bu sayılar üst üste eklenmez.

Bu16ms ile önceki419ms'nin nedeni aynıydı sonucu çıkarılmaz. Yeni handle'ı
worker'a taşıma koşulu doğrulanmadığı için yapısal değişiklik yapılmadı.
Gelecek tekrarda aynı sınırlı tracer gerçek yavaş commit'i doğrudan yakalayabilir.
0 aşım gözlemi gerçek zaman/gelecekte hiç sıçramama garantisi değildir.

SQLite için önceki tekli pooled referans%70,38 korunur; bu fazda yeni SQLite
gecikme A/B sonucu üretilmedi. Nihai100'lük çift her iki backend'de eşit128
commit ile ayrı ölçülür. Owned/Vec performansı bu adımda ölçülmedi.

## Doğrulama

233 test/fmt/clippy geçti; no-default/all-features ve native comparator
kontrolleri geçti. Yeni test tek commit'teki eski/yeni yayınların doğru hedef
sequence'e bağlandığını, ring taşmasını ve varsayılan kapalı olmasını doğrular.
7 yeni kill/reopen, eski okuyucu, cache onarımında authoritative byte koruması,
pin/son-reader GC ve0byte artık testi bu adımda tekrar geçti.

Ham kanıt: `wal-sequence-profile-10k-20261027-a{1,2,3}-*`,
`wal-qualified-10k-20261027-*`, `wal-sequence-kill-10k-20261027-summary.json`,
`snapshot-10k-20261026-validation-sequence-profile-20261027-v1*`.
Trace default kapalı; WAL/reclaim ürün varsayılanları da kapalı kalır.
case34, geniş taramalar/arama, sorgu RAM'i ve soğuk yük açıktır.
Blok2/3/4 ve C ABI başlatılmadı.
