# Toplu yazma belleği: kök neden ve reddedilen erken flush — 20 Ekim 2026

**RAM sorunu kapanmadı; varsayılan motor korunuyor.** Erken flush + hızlı
normalleştirme prototipi dört çiftte hem yazma/bakımı hem belleği iyileştirdi.
Fakat watermark'a yeni ulaşmış belgelerde altı sorgunun tamamı dört çiftte de
yavaşladı. Genel geçiş reddedildi; prototip kodu arşiv yamasında tutuluyor.

## Bellek nerede birikiyor?

49ee3e5 normal motoruna yalnız `diagnostic-memory` aşama callback'leri eklendi.
Bir ayrı tagged-allocator koşusu, aynı ilk10k, aynı20×500 senkron WAL işlemi.
Normal derlemede callback/allocator yoktur. Etiket allocation'ın ilk kökenidir;
`caller_documents` kökenli nesne commit sonrasında DB'nin deltasına taşınabilir.
Bu etiket tek başına hâlâ çağıranda duran nesne veya sızıntı demek değildir.

En yüksek gözlenen aşama snapshot'ı, sequence17 `commit.validated`:

| Aynı snapshot'taki canlı requested allocation | Byte | MiB |
|---|---:|---:|
| Yeni input + önceki deltaya taşınmış özgün belgeler |43.127.304|41,13|
| Folded metin + RAM arama indeksi |30.313.075|28,91|
| Delta tabloları |639.792|0,61|
| Packed metadata |702.060|0,67|
| Diğer + source batch |72.058|0,07|
| **Snapshot total_live** |**74.854.289**|**71,39**|
| WAL frame / builder origin |0 / 0|0 / 0|

Allocator'un bütün koşudaki requested-live tepesi76.007.184B/72,49MiB.
Bu, stage snapshot'ları arasındaki kısa allocation'ları da kapsar. Ayrı
origin'lerin bağımsız peak'leri toplanmadı. Header/alignment, allocator'un
realloc sırasında eski+yeni alanı tutması ve worker örtüşmesi normal motorla
aynı değildir; bunlar OS private/working set veya fiziksel RAM değildir.
Snapshot worker'ları durdurmaz; atomik sayaçlar ardışık okunur. Bu örnekte
origin toplamı total_live'a eşittir; bütün snapshot'lar için atomik bir
çok-sayaç görüntüsü garantisi yoktur.

Kök neden: normal owned API, watermark'a ulaşmış delta'yı **sonraki geçerli
commit içinde** boşaltır. Önceki1.000 belgenin özgün+folded metni dururken
çağıran sonraki500 belgeyi hazırlamıştır. İlk input snapshot'ında, delta henüz
yokken caller kökeni14.465.146B/13,80MiB'dir;500 belgeyi önceden sahipli
hazırlayan API'nin input sözleşmesi, akışlı SQLite karşılaştırıcısıyla aynı
değildir. WAL frame'in önceki yaşam süresi düzeltmesi zaten etkindir: yeni
frame önceki flush sırasında tutulmaz, sync sonrası indekslemeden önce bırakılır.
Bu snapshot'taki büyük tepe WAL tamponundan gelmez.

Tam belgeleri WAL'dan okuma deneyi geçmişte RAM'i azaltıp delta sorgularını
yavaşlattığı için yeniden etkinleştirilmedi. [Önceki karar](COPY-REDUCTION-10K-20261013.md).

[Ham aşama logu](benchmarks/owned-memory-profile-control-20261020-fdb-ingest-10000-0.jsonl),
[snapshot özeti](benchmarks/owned-memory-profile-control-20261020-summary.json).
Profil sonrasında10k tam belge/4 sorgu oracle'ı geçti; core dosyaları başlangıçla
SHA-eşit. Profil süreleri normal performans sonucu olarak kullanılmaz.

## Deneyler: normal release, iki ayrı dört-çift seri

İlk deney bounded owned commit sonunda dolan delta'yı flush eder. İkinci deney
buna, intake `BodyCache` için mevcut `fold_into` normalleştiricisini ekler.
Scalar `fold` bağımsız referans olarak kalır; Unicode genişlemesi ve gram
eşitliği test edilir. Belge/işlem/watermark sınırları,20×500 işlem ve WAL sync
değişmez. Yeni format, dependency veya feature yoktur.

Her seride4 ters sıralı FDB kontrol/aday/SQLite üçlüsü; diagnostic kapalı.
Giriş corpus CRC/JSON, commit/index ve40 sınır belge kontrolü ingest içinde;
son flush/drain ayrı. Toplam medyan, proses bazındaki ingest+bakım toplamından
hesaplanır. Her12 proses sonrası10k tam payload oracle'ı zaman dışında geçti;
FDB'de dört contains sorgusunun tam ilk50 sayfası da kontrol edildi.

| Erken flush tek başına | Eski normal FDB | Aday | Aday/eski |
|---|---:|---:|---:|
| Toplam yazma+bakım, medyan |15,262s|15,321s|+%0,39;1/4 hızlı|
| Proses CPU, medyan |14,086s|14,188s|+%0,72;4/4 yüksek|
| OS lifetime private tepe |79,86MiB|66,71MiB|-%16,46;4/4 düşük|

Tek başına erken flush hız kapısını geçmedi. Son bakımdaki -%10,42, yazma
içine taşınmış bakım maliyetini gizleyerek ayrı genel hız kazancı sayılmadı.

| Erken flush + hızlı intake normalleştirme | Eski normal FDB | Aday | SQLite |
|---|---:|---:|---:|
| Toplam yazma+bakım, medyan |15,726s|**15,110s**|33,582s|
| Proses CPU, medyan |14,586s|**14,016s**|32,484s|
| OS lifetime private tepe |80,10MiB|**66,75MiB**|8,42MiB|
| OS lifetime working set tepe |83,47MiB|**69,85MiB**|12,78MiB|
| Son tutulan core disk |349.805.698B|349.805.698B|583.860.224B|

Birleşik adayda toplam -%3,92,CPU -%3,91,private -%16,67,WS -%16,32;
her biri4/4 çiftte düşük. Toplam süre çift farkları -%3,75/-1,87/-6,32/-5,66.
Son çiftte iki program da önceki çiftlerden yavaştı; çıkarılmadı. Bu sonuç iki
değişikliğin birleşimine aittir; hızlı normalleştirmenin tek başına ölçülmüş
kazancı değildir. İki seri farklı zamanlarda koştu; birinden diğerine doğrudan
oranla yeni bağımsız hız kazanımı çıkarılmaz.

Private commit fiziksel RAM değildir; working set ortak image sayfalarını da
içerir. Sayaçlar proses kapandıktan sonra elde tutulan handle'dan alınmış OS
lifetime high-water değerleridir; yalnız20ms faz örneklerinin tepesi değildir.
SQLite bağımsız native3.53.2,FTS5 trigram,FULL WAL,page cache2MiB,mmap0,tempFILE;
girdi iki seride de akışlıdır. Owned FDB'nin caller belleğiyle aynı sözleşme veya
tüm field/link/guard işlevleri değildir. Final SQLite optimize/checkpoint ve FDB
flush/merge aynı bakım algoritması değildir. Bu tablodan genel ürün üstünlüğü
veya owned FDB için8MiB RAM sonucu çıkarılamaz.

İki seride de eski/yeni FDB core source/index/MET04/ready dosyaları **byte/SHA
eşit**, disk fark0B. Corpus sıcak OS cache'te; eviction,CPU pinning,güç/arka
plan yükü izolasyonu veya gerçek cold/concurrent yük yoktur.

[İlk seri ve bütün çiftler](benchmarks/owned-memory-eager-10k-20261020-summary.json),
[birleşik seri ve bütün çiftler](benchmarks/owned-memory-combined-10k-20261020-summary.json).

## Kabulü durduran kontrol: watermark sonrası arama

Aynı izinli ilk10k'nın ilk1.000 belgesi, iki500-belgelik default owned commit;
yazma zaman dışında.4 ters sıralı eski/yeni CLI çifti. Kontrolde1.000 resident
delta key, adayda0 delta key/1 published run vardır. Böylece erken flush'ın
değiştirdiği gerçek okuma yolu ölçülür; yalnız tamamen flush edilmiş10k DB'de
okumak bu farkı yakalamazdı.

Her sorguya1 ısınma +20 timed örnek/proses,80 örnek/yol. Sorgular dönüşümlü
karıştırılır; tam payload/total/sıra/version/sequence oracle'ı API dışında.
Süre CLI round-trip'idir: native JSON serileştirme ve IPC dahildir, parent
JSON kodlama/çözme ve oracle kontrolü hariçtir. **Saf Rust API süresi değildir.**
Warm OS cache,eviction/affinity/güç veya arka plan izolasyonu yok. Ayrı parent
corpus/oracle tutar; bu alt örnekte proses RAM'i ölçülmedi. SQLite yeniden
arama kıyası veya38-case genel arama serisi değildir.

| Sorgu, offset0/limit50 | Eski p95 ms | Aday p95 ms | Fark |
|---|---:|---:|---:|
| the |7,856|10,545|+%34,22|
| quixote |1,608|2,274|+%41,40|
| he |7,730|9,664|+%25,02|
| the man |7,957|9,508|+%19,49|
| zx |2,137|12,739|+%496,11|
| transubstantiation |0,817|5,037|+%516,31|

**Altı sorgunun tamamında dört çiftin dördünde aday p95 yüksek.** Negatif/kısa
sorgularda büyük fark, yalnız sonuç JSON'unun maliyeti diye kapatılmaz; yine
de read/CRC/fold/hydration attribution bu alt koşuda ayrı profillenmedi. Saf
motor yüzdesi veya CPU attribution iddiası yoktur. Rakamlar bu watermark
sınırı/ilk1k ve CLI yükü içindir; bütün corpus/sorgulara genellenmez.

[Tasarım ve ham çiftler](benchmarks/owned-memory-boundary-1k-20261020-summary.json).

## Karar, koruma ve sonraki iş

Erken flush + hızlı normalleştirme **genel geçiş için reddedildi**. Her iki
runtime değişikliği geri alındı; hızlı normalleştirme tek başına da ölçülmeden
varsayılana taşınmadı. Normal checkpoint ID remap ve `search_into` kazanımları
korunur. Aktif kaynakta yalnız opt-in bellek aşama gözlemleri kaldı.

[Tam prototip ve üç özel testi](benchmarks/owned-memory-10k-20261020-candidate.patch)
49ee3e5 bazına uygulanır; `git apply --check --cached` ile o bazda kontrol
edildi. Prototipte102 lib+3 özel test geçti: watermark'ta içerik/sıra/version,
geçersiz batch atomikliği,stream admission'dan büyük owned belge ve durable
WAL sonrası flush I/O hatasında poison/reopen kurtarma. Bunlar arşivde kalır.
Normalleştirme/flush'ın varsayılanda kaldığı veya arama kapısını geçtiği
söylenmez. Geri dönen motorun test/fmt/Clippy ve veri koruma kanıtı final
proof/checks dosyalarındadır.

Geri dönüşte193 default,135 all-feature lib/segments/streamed,112 referans
ve diagnostic-memory example2 test geçti;7/7/3 ignored helper korunur.
`cargo fmt --check` ve default/all-feature/referans all-targets Clippy
`-D warnings` başarılı. Kesilme,tek-bayt bozulması,süreç sonlandırma ve0.1
uyumluluk kontrolleri bu kümelerdedir. [Komutlar ve loglar](benchmarks/owned-memory-10k-20261020-checks.json),
[test sayıları](benchmarks/owned-memory-10k-20261020-test-counts.json).
Son normal release CLI ve example'lar diagnostic kapalı derlendi. Geri dönen
10k DB'de tam payload/4 contains oracle,38-case/256-key oracle,CLI ilk tam belge
ve SQLite read-only integrity/count kontrolleri geçti. Bu final okumaların
sürelerinden yeni hız kazancı iddiası çıkarılmaz.

Yeni corpus üretilmedi/indirilmedi. Aynı ilk10k:264.842.858 metin byte,
2.057–51.197B pasajlar,8 kitap; tek başına tekrarlı256B corpus değildir.
Boundary alt deneyi bunun ilk1k'sıdır; bağımsız yeni corpus değildir. Yalnız
sahiplik/path kontrolünden geçen sabit FDB/SQLite slotları yazma için
sıfırlandı; DB kopyası/tarihli veri klasörü tutulmadı. Boundary sonrası FDB
slotu eski normal motorla aynı ilk10k'ya döndürüldü; final veri/oracle
kontrolleri zaman dışındadır.0.1 fixture,kalıcı codec,protokol ve dış projeler
değişmedi. [Final kanıt](benchmarks/owned-memory-10k-20261020-proof.json).

Sonraki RAM tasarımının kapısı artık nettir: input/delta çakışması azalırken,
watermark sonrası sıcak belgelerin okuma hızı da korunmalıdır. Yalnız flush
eşiği/zamanı değiştirmek veya eski WAL-ref deneyini açmak yeterli çözüm değildir.
Caller Vec/request RAM'i,genel query RAM'i,son sayfa p95,son merge,açılış,
disk sıkıştırma ve eşzamanlı/gerçek-cold yük [açık listede](OPEN-WORK.md) kalır.
Önceki checkpoint remap'teki +310KiB OS private farkı ayrıca ayrıştırılmadı;
bu tur owned80MiB'nin ana yaşam süresini açıklamak o küçük farkı kapatmaz.
