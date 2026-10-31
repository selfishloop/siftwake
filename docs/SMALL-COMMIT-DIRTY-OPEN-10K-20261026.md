# Küçük commit, artımlı yayın ve dirty açılış —10k /26 Ekim2026

**Adım3 kısmen hedefte; her değişiklik≤50ms kapısı KALDI.**
Yeni küçük WAL yazma yolu `set_streamed_wal_layer(true)` ile **opt-in**;
varsayılan açılmadı. Artımlı maskeler/read-handle ve düşük RAM recovery kalır. Opt-in normal tekli
işlemler ve dirty500 açılış hedefleri gözlenen iki koşuda tutuldu. Aktif katman
dolduğunda foreground flush'a dönen bir işlem379,050ms sürdü. Bu istisna
çıkarılarak başarı sunulmadı; C ABI/entegrasyona geçilmedi. Adım2'nin kullanıcı
kabulü ve o adıma özel kapı kapanışı korunur. Reclaim varsayılan **kapalı**.

| Kapı | Son v6 gözlemi | Karar |
|---|---|---|
| Tekli streamed commit p95≤10ms | Update1,810 /delete1,149 /insert1,950ms; iki koşu pooled | GEÇTİ, gözlenen |
| Normal tekli yazı→arama görünürlüğü≤50ms | Pooled p95≤2,089ms; max11,992ms, n192 | GEÇTİ, bu kısa koşuda |
| Uzun tekli yükte her görünürlük≤50ms | n768, p95 2,712ms; **max379,050ms,1 aşım** | **KALDI** |
| Streamed ingest peak private≤60MB | Aynı10k ingest≤12,85MiB OS peak private; diğer koşular aşağıda | GEÇTİ, gözlenen |
| Temiz idle private≤15MB | 5sn sonra3,52–3,57MiB | GEÇTİ, gözlenen |
| Dirty500 strict open≤300ms | 294,576 /285,147ms; deferred doğrulama yok | GEÇTİ, iki gözlem; marj küçük |
| Dirty500 idle private≤15MB | 5,91 /6,19MiB | GEÇTİ, gözlenen |
| Atomik sınırlar ve non-atomic yardımcı | 5000'lik atomik istek indeks allowance ile reddedildi; helper sonucu aşağıda | BELGELENDİ / doğrulandı |
| Sürekli yazıdan sonra boşta bakım yetişmesi | Aşağıdaki son v6 koşusu; explicit bakım | Ölçülen kapsamla sınırlı |
| Oracle /process kill /eski okuyucu | Aşağıdaki son v6 kanıtları | Gözlenen kapsam |

MB hedefleriyle kıyaslanan tabloda OS ölçümleri **MiB=2²⁰ byte** olarak
etiketlendi. Değerler hem15/60 ondalık MB hem15/60MiB sınırlarının altında;
birimler sessizce birbirine çevrilmedi. Temiz idle, ingest peak ve dirty open
ayrı süreç/fazlardır; private ile working set birbirinin yerine kullanılmadı.

## Uygulama

[Tasarım ve API sınırları](SMALL-COMMIT-DESIGN-20261026.md).
`bounded-engine` opt-in küçük streamed yolu≤32 işlem/≤1MiB için synced WAL
konumlarını tutar; belge/folded metin RAM'de tutulmaz. Snapshot başarılı commit
sonrası aynı sequence'i atomik yayımlar. Küçük işlem önceki worker'ı drain
etmez; bunu worker bilinçli bekletilerek test ettik. Büyük/fallback yolun
mevcut worker/flush bedeli korunur.

Değişmeyen immutable segment önekinin maskesi yeniden kullanılır; yalnız yeni
run ve değişen WAL shadow anahtarları eklenir. Yerleşim değişimi/cache eksikliği
güvenli tam geçişe döner. Maskeler eski snapshot'ta yerinde değiştirilmez.
`mask_keys_scanned` immutable cursor anahtarlarını sayar; değişen shadow key
lookup'ları bu sayacın dışında, `mask_prepare_ns` süresinin içindedir. Sıfır
arama/hazırlık maliyeti iddiası yoktur.

İlk yeni WAL katmanı küçük commit'i hızlandırdı ama aktif kayıtları doğrusal
çözmek4R sorgularını bozdu:23/38 pooled aşım. Bu sonuç
[v3 kötü koşular tablosunda](benchmarks/small-commit-snapshot-10k-20261026-tables.md)
korunur. Son v6, kayıt başına3312byte+Arc overhead'li volatile arama özeti ve
yayında hazırlanmış shadow maskesini kullanır. Kısa ASCII üyelik exact'tir;
diğer adaylar false-negative üretmeyen gram filtresi sonrası gerçek WAL
metniyle doğrulanır. Dönen payload her zaman CRC/key/version ile okunur.
Özet disk biçimine yazılmaz; dirty open süresi yeniden kurmayı içerir.

WAL okuyucu handle'ı her yayında pathname üzerinden yeniden açılmaz; aynı
aktif WAL'in bağımsız salt okunur handle'ı çoğaltılır. Writer append handle'ı
paylaşılmaz. Snapshot pin/son-reader GC ve eski epoch tutarlılığı korunur.
Owned/Vec API'nin intake maliyeti taşınmadı; eski yüksek owned RAM sonucu
bu turda çözülmüş sayılmadı. Dirty recovery'de owned WAL de adreslere döner;
owned çağrının kendisi hâlâ tam input/delta indeks maliyetini taşıyabilir.

Kalıcı format/sürüm, WAL sync politikası, manifest/dizin/protokol değişmedi.
Mevcut FDBLOG04/TXN2/OFF1, SEG ve0.1 okuyucu yolları korunur. Yeni DB/arama
motoru/feature/bağımlılık eklenmedi. Mevcut serde_json raw_value özelliği
önceden var olan WAL adres çözücüsünü default bounded profilde derlemek için
açıldı. Frunner/SDB/Fundops/C ABI değişikliği yapılmadı.

Tablodaki küçük-yazma sonuçları opt-in yolundur. Varsayılan streamed
segment yolunun tek belge gecikmesi1,9ms'ye indi diye sunulmaz.

## Ölçüm düzeni ve kanıt

Aynı mevcut ilk10.000 belge ve önceki donor/revision marker kuralları; yeni
corpus yok. Yalnız sahiplik kontrollü sabit FDB/SQLite slotları kullanıldı.
Corpus/0.1 SHA koruması, frozen executable SHA ve process exit logları vardır.
DB kopyası tutulmadı. Rapor/descriptor ledger/loglar corpus değildir.

İlk v1 ve v2, FDB→SQLite /SQLite→FDB sıralı iki çift: aynı bakımı bitmiş10k,
96 adet tekli mixed işlem (48 update/24 delete/24 insert). SQLite native
FTS5trigram/detailnone,FULLWAL,2MiB cache,mmap0,tempFILE. v6 yalnız FDB'ye
eklenen volatile arama özeti/shadow maskesi için iki kez yeniden ölçüldü;
**SQLite referansı v2'nin değişmeyen iki koşusudur**, v6 ile yeni dengeli
SQLite çifti diye sunulmaz. Eski v1/v2 başarısız ve ara sonuçları korunur.

Her ölçülen commit corpus'tan belgenin akışlı üretilmesini, WAL sync ve
snapshot yayınını içerir. Görünürlük, aynı yayımlanmış snapshot'ta yeni marker
bulunması/eski marker'ın bulunmaması bitene kadar ölçülür; iki sorguyu da
içerdiği için tek arama edinmesinden daha muhafazakâr bir süredir. Bütün10k
payload/version oracle ve38-case oracle ayrıca geçer; ölçülen commit timer'ı
bu son tam oracle'ı içermez.

Dirty500,20'lik25 commit ile oluşturulur; aynı500 değişiklik flush edilmeden
kapanır. Yeniden açılışın `delta_keys=500` olduğu doğrulanır. Tek atomik500
girişli eski owned dirty frame'in peak alloc biçimi bu deneyle eşlenmedi;
burada Frunner'ın akışlı değişiklik yolu ölçülür. Open tam kaynak/frame/CRC/
şema/sequence kontrolünü sürdürür; SQLite open tam integrity_check yapmaz.
5sn idle ölçümünden sonra oracle çalışır; peak sayaçları harness/model dahil
OS proses sayaçlarıdır, motor içi rezervasyon gibi sunulmaz.

OS cache sıcak; tahliye yapılmadı. CPU/power/background izolasyonu yok. Ara
v1/v2 keşif koşularının bazı bölümlerinde derleme/test işi de vardı; bunlar
nihai v6 kabul koşuları sayılmaz. Son v6 ölçümlerinden önce derleme ve test
bitirildi; başka benchmark eşzamanlı başlatılmadı. Open n2'dir, p95 veya
formal üst sınır değildir. Nearest-rank quantile kullanılır.

[v6 opt-in tekli gecikme, görünürlük, clean/dirty open, private/WS ve peak tabloları](benchmarks/small-commit-v6-10k-20261026-tables.md)
/ [JSON özet](benchmarks/small-commit-v6-10k-20261026-summary.json).

## Teşhis: hangi maliyet azaldı

Mask-only dengeli32-yayın kalibrasyonu before/after/after/before:
prepare toplamları416,098 /260,870 /241,406 /402,032ms; iki grubun toplamıyla
yaklaşık%38,6 düşük. Commit ortalaması78,677/77,852/78,075/79,446ms; toplam
kazanç yalnız yaklaşık%1,4. Aynı sabit slotta revision token'ları ilerler;
bakım/publication sayıları43/44 olduğu için bu **dar kalibrasyon**, eşit
fiziksel yerleşimli ingest kabul deneyi değildir.
[Ham tasarım](benchmarks/small-commit-mask-10k-20261026-design.json).

Sonraki profil33 yayında357,956ms hazırlığın352,640ms'sini WAL pathname open,
4,521ms'sini maskeye atfetti. Bu ölçüm değiştirme gerekçesidir; Windows
antivirüs/kernel alt nedeni ölçülmediği için ona bağlanmadı.
[Ham profil](benchmarks/small-commit-open-profile-10k-20261026-existing-slot-fdb-streamed-snapshot-pilot.jsonl).
v2'de ilk97 yayının prepare toplamı5,622ms oldu (WAL handle0,193ms,
mask3,820ms, clone1,402ms). Son v6'ün ek arama özeti bedeli son tablolara dahildir.

## Uzun yük, işlem sınırları ve boşta bakım

Son v6'da768 mixed tekli işlem: commit p95 **2,128ms**, max378,337ms;
yazı→doğrulanmış görünürlük p95 **2,712ms**, max379,050ms.1/768 işlem50ms
sınırını aştı:513. işlem/sequence654, yani512-key eşik geçişi. Aktif katmanın512-key/16MiB sınırında eski segment yoluna dönüş,
source/index/manifest hazırlığı ve varsa worker drain'i foreground'dadır.
Bu faz için exclusive alt-aşama profili yapılmadı; bütün378ms'yi worker kilidi
veya salt fsync'e atfetmiyoruz. Bu yüzden global50ms kapısı açık ve yeni
küçük WAL yolu varsayılan kapalı. Normal hızlı örnekler çıkarılmadığı gibi
bu yavaş örnek de çıkarılmadı. Bu koşunun process peak private14,08MiB.

Aynı mevcut5000 güncellemenin tek atomik isteği **streamed transaction index
allowance** ile reddedildi, sequence değişmedi ve eski durum oracle'ı geçti.
Non-atomic helper aynı işi **263 commit /8,078s**, peak private16,39MiB ile
bitirdi; bütün5000 yeni version ve kalan5000 belge oracle'ı geçti. Bu sadece
ilgili mevcut corpus için gözlemdir; her5000 isteğin aynı sınırda reddedileceği
veya her1000 isteğin sığacağı iddia edilmez.

Boşta yetişme: üç100'lük güncelleme grubunun birinde çağrı başında rewrite'ın
hâlâ çalıştığı gözlendi; diğer ikisi yazı başlamadan bitmişti. Dolayısıyla
**üç gerçek iptal oldu** denmez. Deterministik cancellation/pin testi ayrı
geçti. Yazılar sonrası118,341MB ölü payload, **1sn idle ardından explicit
maintenance1,840s** sonunda0,968MB'ye indi. İlk durumda180,027MB vardı;
yazı aralığında normal bakım/install da çalıştı. Sınırsız sürekli yazı deneyi
veya otomatik idle timer garantisi değildir. Çağıran bakım/poll zamanlamalıdır;
100'lük/fallback yazılar revision işini erteleyebilir. Küçük WAL çağrısı
worker'ı drain/iptal etmez; install poll/wait/maintenance ile ilerler.

[Son v6 uzun yük/helper/idle ham tasarımı](benchmarks/small-commit-final-v6-10k-20261026-design.json).


Atomik `commit_streamed`:1–10.000 işlem, encoded entry≤256KiB, key admission
≤1MiB (anahtar byte'ları+işlem başına512), oluşturulan index allowance≤4MiB
tahmin, eski frame≤64MiB. Belge/gram çeşitliliği nedeniyle “1000 her zaman
sığar” garantisi yoktur. Küçük WAL yolu payload≤1MiB/32 işlem, mevcut shadow
≤512 ve WAL≤16MiB ile sınırlıdır. Options watermark ve önceki owned delta
fallback'e neden olabilir. Rezervasyon toplam proses RAM sınırı değildir.

`Database::commit_streamed_chunks` **ATOMİK DEĞİL**. ≤32 işlem/≤512KiB encoded
girişli parçalar kurar; `ChunkedCommit { atomic:false,sequences,chunk_sizes,
mutations }` döndürür. Hata `ChunkedCommitError.committed` içinde kesin
ack'leri taşır; önceki parçalar geri alınmaz. Geçersiz tek belgeyi parçalayamaz.
Guard/girdi/I/O hatasını sessiz atlamaz. Hata veren parçanın I/O belirsizliği
normal WAL sözleşmesine tabidir; raporlanmayan parçanın kesin reddedildiği
varsayılmaz. Caller'ın retry politikası önemlidir.

## 4 okuyucu+writer izlenen maddeler

Son v6, her grup için iki dengeli sıra (yük→kontrol /kontrol→yük),
38-case×20 örnek/çift, pooled40/sorgu: **tekli ve100'lüde38/38**, ayrıca
her ayrı çiftte38/38 kendi4R boşta sınırını tuttu. <1ms tabanda oran
yerine mutlak artış≤2ms kullanıldı. Tüm p50/p95/max ve1R tabanları da
[ham tablo dosyasında](benchmarks/small-commit-snapshot-v6-10k-20261026-tables.md).

| Grup/çift | Kontrol→yük ops/s | Oran | Writer bekleyen sorgu | Tek bekleme max ms |
|---|---:|---:|---:|---:|
| one/r0 | 215.584→171.735 | 79.66% | 179/760 | 0.0676 |
| one/r1 | 233.272→171.800 | 73.65% | 189/760 | 0.0764 |
| 100/r0 | 314.301→232.081 | 73.84% | 191/760 | 0.0645 |
| 100/r1 | 317.590→241.169 | 75.94% | 219/760 | 0.2014 |

Pooled throughput tekli **%76.65**,100'lü **%74.90**:
**≥%80 izlenen kapı KALDI**. Hiçbir çift sınır üstünde değil;79,66 yuvarlanıp
80 sayılmadı. Bu deneyde iş sayısı okuyucular bitene kadar arttığından
eski57 yazılı koşuyla1539+ yazılı yeni koşunun mutlak ops/s farkı sabit
bakım amortismanından da etkilenir;25× genel motor hızlanması denmez.

Writer-only kontrol de snapshot yayınını açık tutar. İkinci çiftin kontrol
N'si ilk yükten alınır; ikinci yük N'si değişebilir. N'ler tam tabloda
verilir. Bakım ve liveness teşhisi writer wall/throughput içinde; oracle
timer dışında ama snapshot ömrü içinde, CPU/I/O yüküne dahildir. Bu yüzden
oracle'sız Frunner servisinin tam throughput'u gibi sunulmaz.

Her koşuda normal merge ve gerçek revision rewrite tamamlandı;4 reader'ın
da bu bakım aralığıyla örtüşen sorguları var. Yazar bakım interlude'unda
yeni commit yollamaz; kesintisiz yazı altında rewrite ilerlemesi garantisi
değildir. Son4R process peak private **en fazla51,22MiB** gözlendi.

Önceki v3 aktif tam-tarama adayı23/38 sorguda; v4 Bloom-only uzun-terim
elemesi7/38 sorguda kaldı. Bunlar silinmedi. Son v6 aynı volatile exact
ASCII trigram tablosunu uzun terimlerin bütün gramlarında da kullanır;
yeni query kelimesine özel sabit/branch yok. RAM boyutu değişmedi.

Son pooled tekli `quixote whale`7,479→8,147ms; `the /5000/50`4,949→6,140ms.
Önceki izleme maddeleri bu iki tekrarda2× sınırında; genel performans işi
kalıcı kapatılmadı. Negatif `zzxqnever`0,0878→0,2137ms (+0,1259ms),
uzun negatif zx0,0786→0,2133ms (+0,1347ms); kısa sorgu mutlak farkları
gizlenmedi. Boşta4R `zx/the zx`214,76/217,95ms kaldı; tarama işi çözülmedi.

v6 testinde de uzun snapshot ömrü için disk/RAM üst sınırı, gerçek-soğuk cache,
sınırsız sürekli yazı altında bakım ilerlemesi kanıtlanmadı. Bu işleri geçmiş
Adım2 kullanıcı kabulü otomatik olarak kapatmaz. Boşta zx/the zx ve case34
izlenir.50ms flush eşiği çözülmeden genel gecikme garantisi yoktur.

## Doğrulama

226 test geçti,0 başarısız,13 ignored (benchmark/subprocess yardımcıları).
Yeni testler: WAL katmanı/filters/owned interop/reopen, partial-chunk ack,
bekletilen normal worker sırasında küçük commit, exact kısa ASCII ve Unicode
substring filtre oracle'ı; küçük WAL için append öncesi/sync sonrası-apply
öncesi/snapshot ack sonrası3 process-kill durumu. Normal7 kill/reopen/cache
onarımı ve eski okuyucu kanıtları son sürüm için aşağıda yer alır.
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, no-default/
all-features lib check, native SQLite fmt/clippy/test geçti.
[Tam komut/exit kayıtları](benchmarks/snapshot-10k-20261026-validation-small-v7.json).

İlk test denemelerinde immutable kaynak testi, yeni küçük WAL yolunda run
oluşmadığı için explicit flush ile doğru hazırlığa taşındı. Owned dirty-reopen
testindeki resident-doc beklentisi0 yapıldı; count/payload/guard doğrulamaları
korundu. Staging'in33. kaydında Windows cloned-handle cursor etkisi bulundu;
append offset'i açık seek ile sabitlendi, ters key sıralı180 işlem ve tüm
streamed testleri tekrar geçti. Testler davranış hatasını gizlemek için
silinmedi. İlk başarısız lint/test çıktıları da korundu.

Son v6 normal7 pencere: retirement install öncesi/sonrası, rewrite iptali
sonrası, iterator/flush/merge/cache onarımı. Her kill ardından tam10k payload/
version+38-case ve pending old/new marker oracle'ı geçti. Cache onarımı
.fdb/.ready hash'lerini değiştirmedi. İlk3 bariyer, diğer4 gözlenen geçici
kaynak/build dosyası+process suspend penceresidir; cache kill'i open içinde,
snapshot kanalı kurulmadan önce olur. Diğerlerinde eski snapshot pinlidir.
Küçük WAL için ayrıca unit subprocess üç sınırı test etti: append öncesi eski
durum; WAL sync sonrası/uygulama öncesi yeni tam durum; snapshot ack sonrası
yeni tam durum. Kesik frame/tek byte bozulması testleri de full testte geçti.
**Güç kesintisi iddiası yok.**

24Ekim frozen okuyucusu son v6 kaynaklarını, canlı WAL güncellemeleri ve
silme marker'ları dahil, tam oracle ile okudu.0.1 fixture kaynağı güncel testte
okundu; corpus/fixture SHA aynı kaldı. [Son v7 kill kanıtı](benchmarks/small-commit-kill-v7-10k-20261026-summary.json),
[byte/hash ve komut denetimi](benchmarks/small-commit-v6-10k-20261026-audit.json).

v4→son kod farkı: küçük yol açık API tercihiyle etkinleşir, default davranış
testi eklendi; fallback'teki32-document Vec toplanması kaldırıldı; uzun ASCII
sözcüklerin gram elemesi exact tabloyu kullanır. Gate tabloları **son v6** ile
yeniden ölçüldü. Her aşamanın eski olumsuz sonucu arşiv JSONL'de korunur.
Tam test226/0/13; son fmt/clippy/no-default/all-features ve SQLite araç kontrolleri
temiz. Son kod/testten sonra yalnız rapor ve audit araçları düzenlendi.


[Açık işler](OPEN-WORK.md):50ms eşik geçişi,≥%80 throughput,4R case22/32,
zx/the zx,case34, uzun pin ömrü/disk/query RAM/gerçek-soğuk yük. Önceki
10/30tur reclaim disk kabulü geri alınmadı; politikası/eşiği değişmedi,
bu turda30tur yeniden ölçülmüş sayılmaz. Reclaim varsayılan kapalı, C ABI yok.

Son kod denetiminde **v7** ek güvenlik/doğruluk düzeltmesi: snapshot dense
maske bütçeyi aşınca sonradan lazy kurulan sparse kayıt, WAL shadow'u da
kapsıyor sanılmamalı. Coverage artık segment/entry türüne göre kontrol edilir;
artımlı yayım eksik coverage'da tam hazırlığa döner. Atlanmış cache girdisi,
iki tekrar sorgu ve sonraki snapshot için regression testi eklendi. Mevcut10k
maskeleri bütçeye sığdığı için önceki v6 gate verileri korunur; v7 bunların
yeni dengeli throughput tekrarı diye sunulmaz. Son v7 aynı mevcut10k üzerinde
96 tekli işlem ve tam oracle ile ayrıca doğrulandı;7 kill/reopen ve eski okuyucu
yeniden geçti.226 test/fmt/clippy de v7 kodundur.

Son v7 aynı-slot96 işlem doğrulamasında commit p95 update/delete/insert
1,814/1,193/1,809ms, doğrulanmış görünürlük max2,181ms oldu. Bu, yeni temiz
seed/denge çifti değildir; v6 pooled gate tablolarına karıştırılmadı.
