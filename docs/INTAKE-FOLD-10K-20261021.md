# Yazma girişinde normalleştirmeyi ayırma — 21–22 Ekim 2026

**Genel geçiş reddedildi;normal motor geri yüklendi.** Ayrı normalleştirme
ön testi yaklaşık-%81,6,gerçek yazma+bakım akışlı-%3,77/owned-%5,62 verdi.
Ancak RAM azalmadı ve yazma sonrası AND p95 artışı aynı çekirdekte4/4 çiftte
tekrarlandı (+%7,20). Kazanç yalnız dar deney olarak kaydedilir;varsayılanda
hız kazancı veya RAM sorununun çözüldüğü iddiası yoktur.

## Kapsam ve önceden belirlenen ölçüm

942aa7f kontrolüne karşı tek runtime değişikliği: normal `bounded-engine`
`BodyCache` cache miss'inde scalar `fold` yerine mevcut, Unicode eşdeğerliği
testli `fold_into` kullanılır. Çıktı için giriş uzunluğunda String ayrılır;
Unicode genişlerse standart String gerektiği kadar büyür. Başlık, cache limiti,
gram üretimi, belge sahipliği, flush zamanı/eşiği ve worker bütçesi aynı.
Referans profil scalar yolda kalır. Feature, dependency, kalıcı format,
WAL senkronizasyonu, protokol ve dış proje entegrasyonu değişmez.

Önceki erken flush deneyi bu değişikliğin bağımsız sonucu değildir.
Bu deneyde eski ve adayda ilk1k yazma sonrasında1k resident delta key beklenir.

İzinli aynı corpus'un ilk10k belgesi:264.842.858 metin byte,2.057–51.197B
pasajlar,8 kitap. Yeni corpus üretilmez veya indirilmez. Yalnız sabit
`data/benchmark-work/fdb` ve `sqlite` slotları yazma testlerinde sahiplik/path
kontrolüyle sıfırlanır; veritabanı kopyası veya başka disk kullanılmaz.
Son durumda aynı10k veritabanı bırakılır;0.1 fixture değiştirilmez.

Ölçüm sırası:

1. Read-only release unit probe:en fazla200 kaynak metin/grup,her iki mod için
   bir ısınma+dört timed tur,4 süreç,doğrulanmış CPU maskesi4. String tahsisi+
   normalleştirme zaman içinde;corpus CRC/JSON,Vec reserve,eşitlik ve çıktı drop
   dışında. Bu DB yazma/arama veya proses RAM ölçümü değildir. Kapasite
   toplamları eşzamanlı canlı RAM gibi sunulmaz.
2. Normal release akışlı ve owned API'lerin her birinde4 ters sıralı
   kontrol/aday/SQLite üçlüsü. Aynı20×500 senkron WAL işlemi. Girdi CRC/JSON,
   commit sırasında bakım ve40 sınır get kontrolü ingest içinde;son drain
   ayrı. Her koşudan sonra10k tam belge+FDB4 contains oracle'ı zaman dışında.
   OS lifetime peak private commit/working set alınır;fiziksel/toplam sistem
   RAM'i veya yalnız bu fonksiyonun belleği değildir.
3. İlk1k/iki500 owned commit sonrası6 sorgu:the,quixote,he,the man,zx,
   transubstantiation;offset0/limit50,4 çift,her süreçte1 ısınma+100 örnek.
   CLI round-trip native JSON+IPC içerir,parent kodlama/çözme ve tam oracle
   hariçtir;saf Rust API zamanı değildir. RAM bu alt testte ölçülmez.
4. Tam flush edilmiş aynı10k üzerinde38-case arama/get oracle'ı ve normal
   eski/yeni okuma kontrolü. Arama gerilemesi kuşkusu varsa ilgili durum
   yoğunlaştırılır;önceki genel p95 sorunları bu değişiklikle kapanmaz.

Yazma ve CLI ölçümleri sıcak OS cache'te,ardışık süreçlerde;eviction,CPU pinning,
güç/arka plan izolasyonu yoktur. Ayrı okuma/micro ölçümlerinin maskesi ve
ısınması kendi design loglarında yazılıdır. Diagnostic timing/allocation
kapalı. Soğuk cache,eşzamanlı yük ve geçici disk tepe alanı kapsam dışıdır.
SQLite mevcut bağımsız FTS5 trigram/FULL WAL/cache2MiB/mmap0/tempFILE yoludur;
girdisi akışlıdır ve FDB'nin field/link/guard kapsamıyla aynı sözleşme değildir.

Kabul:dar normalleştirme kazancı tek başına yeterli değil;gerçek yazma+bakım
kazancı tekrarlanmalı,CPU ve bellek izlenmeli,yazma sonrası sıcak sorgularda
tekrarlanan gerileme olmamalı. Kazanç yoksa veya sorgu gerilemesi kalırsa
runtime değişikliği geri alınır ve deney reddedilir.

## Dar ön test — 21 Ekim

Her süreçte aynı10k'nın dört timed geçişi vardır;200 metin/grup toplam200
timed grup örneği/mod. Dört süreç toplamlarının medyanı scalar3.472,750ms,
hızlı638,823ms;yaklaşık-%81,60. Bu tek10k geçiş süresi veya DB ingest
süresi değildir. Dört eşleşmede de hızlı yol düşük;her metin her turda
byte-eşit. Bir geçiş için toplanmış çıktı String kapasiteleri388.833.280B
ve264.842.858B;eşzamanlı canlı alan veya OS RAM'i değildir.
[Tasarım,örnekler ve korunmuş kaynak SHA'ları](benchmarks/intake-fold-probe-10k-20261021-summary.json).

## Yazma sonrası kontrol ve ayrıştırma — 21 Ekim

İlk1k/iki500 owned commit sonrasında iki motorda da1k delta key var.
Arama yolu değişmediği halde unpinned CLI kontrolünde p95 artışları görüldü:
the+%15,86,quixote+%13,88,he+%15,30,the man+%14,92,zx+%5,70,
transubstantiation+%15,59. zx dışında beş sorguda4/4 çift yüksek;
zx3/4 yüksek.400 örnek/yol/sorgu;native JSON+IPC içeren round-trip.
Tam payload/total/sıra/version oracle'ı her çağrıda zaman dışında geçti.
[Bütün örnekler](benchmarks/intake-fold-boundary-1k-20261021-summary.json).

Ayrı CPU-mask4 native worker'ında `Database::search` ile
`serde_json::to_vec` ayrı zamanlandı. Query çıktıları8 süreçte aynı encoded uzunluk/CRC;
bu warmup'tan üretilen cross-reader kontrolüdür,bağımsız oracle değildir.
Bağımsız tam oracle yukarıdaki CLI kontrolündedir. Native pooled search
p95 farkları the+%13,71,quixote-%15,10,he+%14,12,the man+%6,02,
zx-%1,76,transubstantiation+%0,47;hiçbir search artışı4/4 çiftte değildir.
Bu farklı binary/API/heap/CPU koşulları ilk CLI artışını açıklamış veya
kapatmış sayılmaz. JSON alt ölçümü CLI'nin Value/Display/pipe yolu değildir;
bu süreler CLI toplamına pay olarak toplanamaz veya neden diye atanamaz.
[Native aşamalar ve sınırlar](benchmarks/intake-fold-native-1k-20261021-summary.json).

## Gerçek yazma ve bakım — 22 Ekim

Her API için4 ters sıralı kontrol/aday/SQLite üçlüsü,toplam24 normal release
süreci. Aşağıdaki toplam medyan her sürecin ingest+son bakım toplamındandır;
ayrı faz medyanları toplanmadı. CPU prosesin bütün thread'lerinin OS toplamıdır.

| Medyan | Eski FDB | Aday | SQLite | Aday/eski |
|---|---:|---:|---:|---:|
| Akışlı yazma+bakım |20,251s|19,488s|39,441s|-%3,77;4/4 hızlı|
| Akışlı CPU |17,922s|17,156s|37,438s|-%4,27;4/4 düşük|
| Owned yazma+bakım |19,263s|18,180s|40,890s|-%5,62;4/4 hızlı|
| Owned CPU |17,422s|16,391s|38,867s|-%5,92;3/4 düşük|
| Akışlı OS peak private |10,17MiB|10,24MiB|8,42MiB|+68KiB;3/4 yüksek|
| Owned OS peak private |79,90MiB|80,05MiB|8,42MiB|+150KiB;3/4 yüksek|
| Owned OS peak working set |83,22MiB|83,49MiB|12,79MiB|+274KiB;4/4 yüksek|
| Tutulan core disk |349.805.698B|349.805.698B|583.860.224B|0B|

RAM kazancı yoktur. Private commit fiziksel/toplam RAM değildir;working set
paylaşılan image sayfalarını da içerir. Bunlar20ms faz örneklerinden ayrı,
elde tutulan proses handle'ından alınan OS lifetime high-water sayaçlarıdır.
Akışlı WS medyanı114KiB düşük ama yalnız2/4 çiftte düşük;genel RAM sonucu
diye kullanılmaz. Owned API500 belgelik Vec'i önceden sahipli hazırlar;
SQLite input'u akışlıdır ve aynı caller/RAM/field/link/guard sözleşmesi değildir.

Akışlı son bakım0,679→0,717s (+%5,58),**4/4 çiftte daha yavaş**.
Toplam hız kazancı bunu gizleyerek bağımsız bakım kazanımı sayılmaz.
Owned son bakım3,713→3,509s fakat yalnız2/4 çift hızlı;ayrı bakım kazancı
kanıtlanmadı. Owned ingest3/4,CPU3/4 düşük;bir CPU çifti+%0,96 yüksek.
Bu küçük örnek sayısı genel hız garantisi veya istatistiksel anlamlılık değildir.

Her24 süreç sonrasında10k tam payload oracle'ı;16 FDB sürecinde ayrıca dört
contains tam ilk50 sayfa oracle'ı zaman dışında geçti. Kontrol/aday core
source/index/ready dosyaları her API içinde SHA/byte-eşit. APIs arası yerleşim
aynı sanılmaz. Corpus ve0.1 kaynak SHA'ları korundu. Final optimize/checkpoint
ile FDB flush/merge aynı bakım algoritması değildir;SQLite okuma/açılış
karşılaştırması bu tur tekrar edilmedi.
[Tasarım,sıralar,bütün fazlar ve bellek](benchmarks/intake-fold-normal-10k-20261022-summary.json).

## Tam flush edilmiş10k'da okuma kontrolü — 22 Ekim

Aynı normal owned yerleşiminde4 süreç çifti×2 kısa blok×10 tur. Normal
38-case setinde yaygın/dar,iki karakter,AND,Unicode ve sayfalar var.30 primary
sorguda80,sekiz sayfada24 timed örnek/okuyucu;tam belge/sıra/version/sequence
oracle'ı ve strict source/index doğrulaması zaman dışında. CPU maskesi4;
iki worker'ın corpus/oracle belleği engine RAM'i gibi sunulmaz.

15 düşük/23 yüksek pooled p95;genel arama kabulü geçmedi. quixote+%1,90,
man+%8,35,zx+%8,49,quixote whale+%14,20 bütün4 çiftte yüksek. Bu sonuçlar
attribution değildir ve her blok medyanı/p95 aynı dağılım değildir. Örneğin
the200 pooled p95+%23,90 fakat yalnız2/4 çift yüksek,tipik blok-%4,29;
bu kalemden tutarlı saf algoritma gerilemesi çıkarılmaz. Son sayfa
the9950/50 p95+%3,13,2/4 hızlı;önceki açık son-sayfa işi kapanmaz.
21 DB/cache/SQLite/fixture dosyası okuma öncesi/sonrası SHA-eşit.
[Bütün38 durum ve bloklar](benchmarks/intake-fold-read-10k-20261022-summary.json).

## Aynı çekirdekte yazma sonrası tekrar — 22 Ekim

İlk kontrolün unpinned olması üzerine aynı executable SHA'ları,aynı1k,
iki500 synced commit ve1k resident delta key ile4 çift tekrarlandı. Worker
CPU maskesi4 hem set hem get ile doğrulandı.1 ısınma+100 timed çağrı/proses,
400 örnek/yol/sorgu. Cache sıcak,eviction/güç/arka plan izolasyonu yoktur.
Süre yine native JSON+IPC içeren CLI round-trip;parent JSON/oracle hariç.

| Offset0/limit50 | Eski p95 ms | Aday p95 ms | Fark | Aday hızlı çift |
|---|---:|---:|---:|---:|
| the |11,3788|12,2260|+%7,45|1/4|
| quixote |2,2163|2,2624|+%2,08|1/4|
| he |11,3655|12,1600|+%6,99|1/4|
| the man |11,6130|12,4494|**+%7,20**|**0/4**|
| zx |2,1710|2,1145|-%2,60|3/4|
| transubstantiation |0,9169|0,8670|-%5,44|3/4|

CPU pinning ilk serinin bütün artışlarını kaldırmadı. AND artışı iki farklı
günün kontrolünde4/4 çiftte kaldı;ölçüm dalgalanması diye kapatılmadı.
İki serinin farklı CPU/cache/ortam durumları doğrudan birbirine hız kazancı
diye oranlanmaz. Native alt ölçüm/CLI farkı kök nedeni kesin atfetmez:
derleyici yerleşimi,allocator durumu ve ortam payları bu tur ayrıştırılmadı.
[Tekrarın tüm örnekleri ve SHA'ları](benchmarks/intake-fold-boundary-pinned-1k-20261022-summary.json).

## Karar ve koruma

Dar normalleştirme hızlandı,gerçek yazmada tekrarlanan toplam kazanç var;
**arama gerilememesi şartı geçilmedi**. Runtime değişikliği geri alındı.
`src/index.rs` normal kaynakları942aa7f ile aynı;yalnız dosya sonuna
`cfg(test)` modülü eklendi. Aktif `BodyCache` scalar `fold` kullanır.
Mevcut bounded profil,checkpoint ID remap ve `search_into` korundu.

[Adayın tam yaması ve testi](benchmarks/intake-fold-10k-20261021-candidate.patch)
942aa7f bazına ayrı geçici Git index'inde `git apply --check --cached` ile
doğrulandı. Ana Git index'i bu kontrolle değiştirilmedi. Native attribution
worker'ının ölçülen kaynak sürümü [burada](benchmarks/intake-fold-native-1k-20261021-worker.rs);
aktif örnekte yalnız Clippy'nin istediği eşdeğer çiftlik kontrolü kullanılır.
Kalıcı tool/test ekleri corpus'u hazırlamaz veya yeni belge üretmez.

Adayda194 default/136 all-feature hedef/113 referans test;fmt ve üç profil
all-targets Clippy geçti. Kesilme,bozulma,process kill ve0.1 uyumluluk bu
kümelerde yer alır. Geri dönüşte128 lib/engine/streamed testi+native örneğin
bir CRC testi,fmt ve default/all-feature/referans Clippy yeniden başarılı.
[İlk kontroller](benchmarks/intake-fold-10k-20261021-checks.json),
[son kontroller](benchmarks/intake-fold-10k-20261022-final-checks.json),
[sayılar](benchmarks/intake-fold-10k-20261022-test-counts.json).

Boundary sonrasında sabit FDB slotu eski normal writer ile aynı10k'ya
döndürüldü ve10k tam payload+dört contains oracle'ı zaman dışında geçti.
Son restore süresi yeni hız ölçümü sayılmaz. Final release binary'ler normal
motorla yeniden derlendi;dört programın `.text` kod bölümü eski kontrolle
byte-eşit. Source/core/corpus/fixture/SQLite ve yeni binary
kontrolleri [final kanıtta](benchmarks/intake-fold-10k-20261022-proof.json).

Yazma/son merge,genel/son-sayfa p95,owned/caller/query RAM,açılış,disk/geçici
alan ve eşzamanlı/gerçek-soğuk yük [açık listede](OPEN-WORK.md) kalır.
Bu tur genel RAM veya arama sorunu çözülmedi;format/feature/dependency/WAL/
protokol veya dış proje değişikliği yapılmadı.
