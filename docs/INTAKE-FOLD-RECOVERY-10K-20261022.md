# Yazma kazanımını ayırıp koruma — 22 Ekim 2026

**Yeni yazma normalleştirmesi normal bounded profilde etkin tutuldu.**
Toplam yazma+bakım akışlı-%4,51,owned-%4,81;CPU da iki API'de4/4 düşük.
Genel arama/RAM kapısı geçti denmiyor;karma sorgu p95'leri ayrı açık tutulur.

Amaç1f5e0ba'da geri alınmış yazma normalleştirme kazanımını,
arama/CLI ve ölçüm aracının etkilerini ayırarak kalıcılaştırmak.
Önceki+%7,20 CLI sonucu geçerli bir round-trip ölçümüdür;tek başına motor
aramasının aynı oranda yavaşladığını veya nedenini kanıtlamaz.

## Kontrol tasarımı

Yalnız mevcut corpus'un ilk1k/10k belgeleri,sabit FDB/SQLite slotları.
Yeni corpus veya DB kopyası yok. Boundary testlerinden sonra slot normal10k'ya
dönecek. Feature/format/WAL/durability/protokol ve dış entegrasyon değişmez.

1. Tek release test executable'ında test-only scalar/fast seçimi. Böylece
   arama ve serileştirme kodu her iki modda aynıdır. Bu test binary'sinden
   normal release yazma veya proses RAM kazancı çıkarılmaz.
2. İlk ayrıştırma worker'ında ara Value yanlışlıkla format aşamasında da
   canlıydı. CLI match kolunda bu Value format başlamadan bırakılır. İlk
   `intake-fold-causal-1k-20261022` serisi bu ömür farkını taşır;özellikle
   +%52 JSON artışı normal CLI/DB regresyonu diye kullanılamaz. Loglar
   tarihçe olarak korunur;aynı veriyle düzeltilmiş kontrol yeniden koşuldu.
3. Düzeltilmiş tek binary'de scalar,ilk fast ve kapasiteyi eşleyen rounded
   karşılaştırıldı. `Database::search`,page→Value,Value response ve Display→
   fresh Vec ayrı zamanlanır. Ara Value CLI'deki noktada düşer. Gerçek pipe/
   IPC/OS I/O yoktur;bu alt süreler önceki CLI toplamına pay olarak atanamaz.
   Her mod4 süreç×200 örnek/sorgu,CPU maskesi4 doğrulanmış;sıra dengelenir.
   Parent bağımsız total/key/version/ilk50 oracle'ını kontrol eder;encoded
   byte sayısı/CRC bütün modlarda eşittir. Tam payload'ın bağımsız oracle'ı
   olduğu söylenmez. Core source/ready byte'ları da aynı.
4. Normal release'te eski CLI'nin kendisiyle A/A kontrolü ve eski/yeni A/B
   kontrolü aynı1k/CPU maskesi4/4 çift/100 örnekle yapılacak. A/A ölçüm tabanı
   görülmeden küçük farklılıklar otomatik geri alma kararı olmayacak.
5. Yeni aday normal release'te akışlı ve owned yazmada4 çiftle ölçülecek.
   Bugünkü SQLite karşılaştırması zaten mevcut;bu dar tekrarda SQLite yeniden
   yazılmayacak. Yeni kıyas medyanlarında eski SQLite süreleri karıştırılmaz.
   Tam10k belge+dört contains oracle ve source byte eşitliği kontrol edilir.
6. Önceki dört okuyucu artışı yoğun kontrolle izlenir;genel38-case doğruluğu
   da kontrol edilir. RAM,son merge,genel p95,açılış/disk ve cold/concurrent
   işlerinin bu dar sonuçla kapandığı söylenmez.

## Aday

Bounded intake için ayrı `fold_for_intake` işlevi mevcut `fold_into` CPU
yolunu kullanır. İlk kapasite scalar String'in güç-iki sınıfına yuvarlanır;
Türkçe dönüşümle içerik kısalırsa son kapasite de folded uzunluğuna göre
küçültülür. Unicode genişlemesinde normal String büyümesi geçerlidir.
Boşta0,küçük metinde en az8 byte;kontrol edilen metinlerde scalar kapasitesi
ve içerikle eşittir. Ayrı call sınırı write-only gövdeyi çağırana inline etmez.
Bu allocator/kod yerleşimi için dar bir izolasyondur;tek başına önceki
yavaşlamanın nedenini kanıtlayan sonuç diye sunulmaz.

Test-only mod seçimi normal library/CLI'de derlenmez. Referans profil scalar
fold'u korur. Delta/flush/işlem/bütçe eşikleri,gram ve sorgu mantığı aynı.

## Düzeltilmiş aynı-program sonucu

Rounded modunda scalar'a göre pooled native search p95:the-%10,45,
quixote-%4,46,he-%12,71,the man-%2,52,zx-%3,15,
transubstantiation-%4,38. Çift kazanımı sırasıyla3/4,3/4,3/4,2/4,4/4,2/4;
genel üretim araması hız garantisi değildir. Rounded page→Value p95 de
altı durumda düşük. İlk exact-capacity fast modunda önceki büyük JSON artışı
da kalmadı. Yalnız yuvarlamaya bütün farkın nedeni atfedilmez;Value ömrü
de kontrol edildi ve ortam değişkendir.
[Tasarım,12 süreç ve bütün aşamalar](benchmarks/intake-fold-capacity-causal-1k-20261022-summary.json).

## Normal CLI: A/A ve yeni A/B

İki seride de ilk1k,iki500 synced owned commit,iki okuyucuda1k delta,
CPU maskesi4,4 çift,400 örnek/yol/sorgu,tam payload/total/sıra/version oracle.
Süre JSON+IPC round-trip;parent kodlama/çözme/oracle hariç. A/A'nın iki
program yolu/SHA'sı aynıdır;A/B eski normal ile yeni rounded/call-boundary
sürümüdür. OS cache sıcak;eviction/güç/arka plan izolasyonu yoktur.

| Sorgu | Aynı program A/A p95 farkı | Eski/yeni A/B p95 farkı | A/B hızlı çift |
|---|---:|---:|---:|
| the |+%0,68|+%1,27|1/4|
| quixote |+%3,94|+%4,23|0/4|
| he |yaklaşık0|+%0,64|2/4|
| the man |-%0,21|+%1,30|2/4|
| zx |+%1,16|+%2,06|2/4|
| transubstantiation |-%0,42|+%0,31|3/4|

Önceki AND+%7,20/4 çift yüksek sonucu yeni sürümde aynı büyüklük ve
tutarlılıkta tekrarlanmadı. Quixote yeni CLI p95'i2,0101→2,0952ms,
+85,1µs/4 çift yüksek;aynı binary A/A'da da+80µs var. Bunlar çıkarılarak
"gerçek fark5,1µs" hesabı yapılamaz;iki ayrı seri güven aralığı veya kesin
nedensel ayrıştırma değildir. Quixote CLI kuyruğu kapanmadı. Bu nedenle
bütün sorgular hızlandı veya hiç regresyon yok iddiası yapılmaz.
[A/A](benchmarks/intake-fold-recovery-aa-1k-20261022-summary.json),
[A/B](benchmarks/intake-fold-recovery-ab-1k-20261022-summary.json).

## Normal release yazma — kalıcı kazanım

Her API4 eski/yeni çift,toplam16 süreç. Diagnostic ve test-only mod seçimi
kapalı. Aynı ilk10k/20×500 synced commit;giriş CRC/JSON,commit sırasında
bakım ve40 sınır get'i ingest içinde. Son drain ayrı,toplam her sürecin
ingest+drain'inden hesaplanır. CPU bütün proses thread'lerinin OS toplamıdır.

| Medyan | Eski normal | Yeni normal | Fark | Düşük çift |
|---|---:|---:|---:|---:|
| Akışlı yazma+bakım |19,661s|18,774s|**-%4,51**|**4/4**|
| Akışlı CPU |18,180s|17,250s|-%5,11|4/4|
| Owned yazma+bakım |17,922s|17,060s|**-%4,81**|**4/4**|
| Owned CPU |16,727s|15,898s|-%4,95|4/4|
| Akışlı peak private |10,10MiB|10,03MiB|-78KiB|3/4|
| Owned peak private |79,89MiB|79,85MiB|-48KiB|3/4|
| Tutulan core disk |349.805.698B|349.805.698B|0B|eşit|

Bu küçük private farkları genel RAM sorununun çözümü değildir. OS lifetime
private commit fiziksel/toplam RAM değildir. Akışlı WS+94KiB/3 çift yüksek,
owned WS-24KiB fakat yalnız1/4 düşük;WS paylaşılan image sayfalarını da içerir.
Owned/caller RAM yaklaşık80MiB düzeyinde açık kalır. Akışlı son bakım
0,671→0,683s (+%1,85;3 çift yüksek),owned3,355→3,354s/2 çift hızlı;
ayrı bakım kazancı iddiası yoktur. Dört çift genel hız garantisi değildir.

Her16 süreç sonrasında10k tam payload+dört contains tam ilk50 oracle'ı
zaman dışında geçti;API içinde old/new source/index/ready SHA'ları aynı.
SQLite bu follow-up'ta yazılmadı;önceki SQLite süreleri bu yeni medyanlara
karıştırılmadı. Yalnız mevcut SQLite DB'nin read-only bütünlüğü kontrol edilir.
[Sıralar,bütün örnekler ve sayaçlar](benchmarks/intake-fold-recovery-write-10k-20261022-summary.json).

## Okuma: kazanımı silmeden izlenen açık kuyruk

Tam flush edilmiş aynı10k'da önceki dört artış;4 çift×2 blok×25 tur,
200 örnek/sorgu/okuyucu,strict kaynak doğrulaması ve tam oracle zaman dışında.
Quixote p95+%1,97/1 çift hızlı,zx+%7,16/1 çift hızlı,quixote-whale
+%0,12/2 çift hızlı. Man p95 1,3392→1,6035ms (+%19,74),4 çift yüksek;
tipik blok medyan farkı+%0,53. Bu karma yükteki tail artışı açıktır.
[Yoğun dört durum](benchmarks/intake-fold-recovery-read-dense-10k-20261022-summary.json).

Man tek başına,aynı executable'lar/4 çift/200 örnek/CPU maskesi4 ile
ölçüldüğünde p95 1,4739→1,4924ms (+%1,26),3/4 çift yeni düşük.
Çağıran thread cycle p95 5.583.674→5.583.296 (yaklaşık aynı),cycle p50
yaklaşık-%0,37. Counter okumaları wall bracket dışında ama cycle değerine
counter maliyeti girer;cycle sabit frekans saati değildir. Thread CPU-ms
kuantizasyonu1ms sorgu için kullanılabilir kesin CPU yüzdesi vermez.
Bu kontrol karma kuyruk artışının tek başına ek CPU işi diye açıklanmasını
desteklemiyor;karma yükün etkisi/ortam payları kesin ayrıştırılmadı.
Artış "dalgalanma" diye kapatılmadı ve bütün okuyucu kabulü geçti denmedi.
[Tek sorgu ve cycle kontrolü](benchmarks/intake-fold-recovery-man-cycles-10k-20261022-summary.json).

Ayrıca38-case tam payload/sıra/version/get oracle'ı2 çift×2 blok×1 turla
geçti. Bu çok az örnekli correctness smoke'unun p95 kolonları yeni genel
performans sonucu sayılmaz.21 okuma dosyası bütün okuma serilerinde SHA-eşit.
[38-case doğruluk](benchmarks/intake-fold-recovery-read-oracle-10k-20261022-summary.json).

## Karar ve kalan işler

Kanıtlı yazma toplam/CPU kazanımı normal `bounded-engine` içinde kalıcıdır.
Geçici String kapasitesi scalar ile eşlenen genel normalleştirici kullanılır;
kelimeye özel üretim kodu yoktur. Genelleştirilmiş RAM/arama kapısı ayrı
açıktır. Açık tail, bütün yazma adımının otomatik geri alınma gerekçesi
yapılmadı;yeni hata/kayıp doğruluk bulunursa bu karar tekrar değerlendirilir.
Caller/query RAM,son bakım,karma sorgu/CLI kuyruğu,açılış,disk/geçici alan,
eşzamanlı/gerçek-soğuk yük [açık listede](OPEN-WORK.md) izlenir.

Normal/candidate/reference testler,fmt,üç profil Clippy,fixture/corpus/core/
SQLite ve normal release kontrolleri [final kanıtta](benchmarks/intake-fold-recovery-10k-20261022-proof.json).

Son doğrulamada157 default lib/engine/streamed/segments/native-binary,
119 all-feature lib/streamed ve57 referans lib/engine testi geçti;ignored
benchmark/child helper'lar normal suite'te çalıştırılmadı. Kesilme,bozulma,
process kill ve0.1 uyumluluk ilgili kümelerde korundu. `cargo fmt --check`
ve default/all-feature/referans all-targets Clippy `-D warnings` başarılı.
Son normal release programları ölçülen aday SHA'larıyla aynı;test-only
mode env adı normal executable'larda bulunmuyor. Final normal10k tam
payload+dört arama oracle'ı ve CLI stats(10k/sequence20/delta0) geçti.
17 core dosyası başlangıçtaki normal10k ile,21 okuma dosyası read snapshot'ı
ile,corpus ve0.1 fixture kaynakları da eski SHA'larıyla eşit. SQLite yalnız
read-only integrity/count kontrolünden geçti. Kalıcı karar normal
`cargo build --release` içindir;ayrı deneysel feature açılması gerekmez.
