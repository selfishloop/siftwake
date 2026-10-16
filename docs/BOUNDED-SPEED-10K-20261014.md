# Düşük RAM profilinde arama hızını geri kazanma — ilk adım

2026-10-14 kullanıcı, önceki 10k ölçümündeki düşük RAM sonucunu kabul ederek
önceliği bu bellek seviyesinde hızı geri kazanmaya verdi. Bu rapor önceki
`experimental-builder-budget` + `commit_streamed` profilini yeni sürümüyle
karşılaştırır. Normal `commit(Vec<Mutation>)` ve default feature seçimi aynı;
kalıcı source/WAL/manifest/payload codec değişikliği yoktur.

## Dar değişiklik

- İkili contains doğrulaması sayısal kayıt bilgisi ve anahtar için aynı kayıt
  sayfasını sorgu boyunca kullanır. Anahtar, prefix/suffix byte'ları üzerinden
  karşılaştırılır; her adayda yeniden owned `Key` oluşturulmaz. Cursor yalnız
  bir leaf tutar. Payload CRC, UTF-8/schema, key/version kontrolleri sürer.
- AND için bütün zorunlu gramların Bloom/fence kontrolü, ilk sözlük sayfasını
  okumadan yapılır. Kesin yokluk dışında sonuç kabul edilmez; iki karakterli
  terimlerin mevcut içerik tarama yolu korunur.
- Mevcut 1 MiB sözlük cache payının 64 KiB'lik kısmı sık kullanılan küçük
  `(Term, Posting)` konumları içindir; page payı 960 KiB'dir. Owner kimliği
  indeks nesline bağlıdır; farklı nesiller karışmaz, drop sırasında silinir.
  Doküman veya arama sonucu cache'i eklenmedi. Byte charge hesapları gerçek
  allocator/heap sınırı değildir; proses belleği ayrıca aşağıda ölçülür.
- Ölçüm araçlarında her iki varyantın streamed girdisi açıkça seçilebilir;
  logical state adı ile fiziksel delta yerleşiminin karışması düzeltilmiştir.

## Kök neden profili

`bounded-speed-profile-{before,after}.jsonl`: mevcut corpus'un ilk1000 belgesi,
aynı synced streamed işlem, ilk state'teki `the` sorgusunun bir sıcak örneği.
Diagnostic build, karşılaştırma kapısı değildir; span'ler iç içedir, toplanmaz.

| Süre / iş | Önce | Sonra |
|---|---:|---:|
| API toplamı | 12,283 ms | 12,151 ms |
| Kayıt metadata + kimlik span'ları | 1,110 ms | 0,327 ms |
| İçerik okuma | 4,880 ms | 5,143 ms |
| Payload CRC | 2,102 ms | 2,236 ms |
| Payload validation | 2,039 ms | 2,138 ms |
| Body normalleştirme | 0,471 ms | 0,526 ms |

Her iki sürümde1000 aday,27.043.900 requested payload byte,113 fiziksel payload
read ve29.138.419 read byte var. Yalnız kayıt çözme tekrarını kaldırmak bütün
gecikmeyi çözmüyor. Asıl sonraki hedef tam uzun payload'ı tekrar okumadan,
doğrulama güvencesini koruyan metin erişimidir; bu raporda uygulanmış değildir.

## Aktif veri araması — aynı fiziksel yerleşim

`bounded-speed-delta-20261014`: ilk1000 belge; ters ingest,40 silme/yeniden
ekleme ve reopen;6 query×3 state=18 case. Her iki varyant synced streamed,
resident delta0; oracle1000 belgeyi RAM'de tutar, proses RAM'i burada ölçülmez.
Üç paired proses/varyant, çift1 ters sırada; query başına1 warm+10 sample/proses,
30 pooled sample ve nearest-rank p95. Tam total/ilk50 belge ve sürüm kontrolü.
Sorgular: `the`, `quixote`, `he`, `the man`, `istanbul`, `updatedunique`.

**15 p95 düşük /3 yüksek.** İlk state örnekleri:

| Query | Önce p95 | Sonra p95 | Hızlı çift |
|---|---:|---:|---:|
| the | 13,628 ms | 12,859 ms | 2/3 |
| he | 14,587 ms | 13,581 ms | 3/3 |
| the man | 20,066 ms | 17,363 ms | 3/3 |
| quixote | 0,363 ms | 0,640 ms | 0/3 |

`quixote` ilk iki state'te geriledi; reopened state'te pooled p95 %0,62 düşük.
İlk state'teki boş `istanbul` %8,16 / yaklaşık0,4µs yüksek. Bu yeni farklar
önceki grouped-read dalgalanma kabulüyle kapatılmadı. Metadata profili dar
sorgudaki bütün artışı açıklamıyor; inceleme açık. Normal RAM delta'sının eski
alt-milisaniyelik geniş arama düzeyine dönüldüğü iddia edilmez.

## Yazma, bakım, RAM ve disk — üç paired koşu

`bounded-speed-10k-20261014`: aynı mevcut corpus ilk10k,20 atomik500-belge
işlemi; input CRC/JSON decode+commit/indexing ve40 örnek belge kontrolü ingest
süresine dahil, son flush/merge ayrıca ölçülür. Her iki varyant release,
experimental-builder-budget, diagnostic kapalı, synced WAL ve streamed input.
Sıcak OS cache, cache eviction yok; private/working set20ms, phase poll100ms.
Her koşu dışında tam10k belge/version+4 query oracle ve eski executable okuması.

| Medyan | Önceki düşük RAM profili | Yeni |
|---|---:|---:|
| Ingest | 39,475 s | 39,088 s |
| Bakım | 1,189 s | 1,098 s |
| Her koşunun ingest+bakım toplamı | 40,685 s | 40,161 s |
| İndeksleme private tepe | 9,07 MiB | 9,09 MiB |
| Bakım private tepe | 7,80 MiB | 8,26 MiB |
| OS peak working set | 12,93 MiB | 13,00 MiB |
| Idle private | 1,73 MiB | 1,64 MiB |
| Retained disk | 1.521.041.630 B | 1.521.041.630 B |

Toplam2/3 çift hızlı, bir çift %6,07 yavaş; bu bir yazma/bakım hız kazanımı
kapısı değildir. Arama değişikliğinin yazma maliyeti belirgin biçimde çözülmedi.
İndeksleme private yaklaşık9 MiB düzeyinde kaldı; bakımda yaklaşık0,46 MiB
artış var. Private bytes toplam fiziksel RAM değildir; working set ayrı verilir.
Working allowance16MiB/base2MiB sayaçları allocation veya proses RAM'i değildir.
Source267.900.709 B aktif WAL45 B'yi içerir; ikinci kez eklenmez. Live source,
index ve WAL621.588.895 B; ready516 B ile621.589.411 B. Boyutlar aynı.

SQLite bu dar adımda yeniden çalıştırılmadı. Aynı günkü ayrı üç SQL referansının
indexing private8,40 MiB ve toplam46,97s sonucu
[önceki rapordadır](BOUNDED-MEMORY-10K-20261014.md); bu koşuyla paired değildir.

## Geniş arama ve doğrulama

`bounded-speed-matrix-10k-20261014`: aynı son10k veritabanı üzerinde eski/yeni
düşük RAM reader, üç paired proses ve ikinci çift ters sırada. İki reader da
`open_record_reads`; pending source doğrulaması ilk aramadan önce korunur.
Diagnostic kapalı. Aynı38 case: dar/yaygın/olmayan kelimeler, iki karakter,
AND, Türkçe fold, uzun ve sonuçsuz sorgular, offset ve limit varyasyonları.30 temel
case20 sample/proses (60 pooled),8 sayfalama case5 sample/proses (15 pooled).
Sorgular karıştırılarak çalışır; sıcak OS cache, açık cache boşaltma yok.
Sınırlı LRU cache'lerin kendi eviction davranışı normal olarak çalışır.
Tam belge/version/sıralama/total oracle geçti. Bu harness'in resident oracle
belleği proses RAM ölçümü değildir. SQL yeniden çalıştırılmadı.

**36 p95 düşük /2 yüksek.** 22 case üç çiftte de hızlı; pooled p95'nin düşmesi
her case'in üç çiftte hızlı olduğunu göstermez.

| Query / offset / limit | Önce p95 | Sonra p95 | Hızlı çift |
|---|---:|---:|---:|
| quixote /0 /50 | 7,601 ms | 5,665 ms | 3/3 |
| zzxqnever /0 /50 | 1,196 ms | 0,060 ms | 3/3 |
| quixote whale /0 /50 | 8,171 ms | 7,247 ms | 3/3 |
| the /0 /50 | 167,201 ms | 162,445 ms | 1/3 |
| the /5000 /50 | 169,459 ms | 191,661 ms | 1/3 |
| the /9950 /50 | 156,508 ms | 174,614 ms | 1/3 |

İki ileri sayfa p95 artışı %13,10 ve %11,57; dalgalanma diye kapatılmadı.
Yalnız RAM dictionary'li normal yola göre bütün hızın geri kazanıldığı veya
SQLite'tan her sorguda hızlı olunduğu iddia edilmez. Bu dar adım opt-in profile
içinde korunur; bütün hız hedefi ve default'a geçiş kapısı hâlâ açık.
FDB/cache ve SQLite DB bütün dosya hash'leri ölçüm öncesi/sonrası aynı.

Default/exact-feature/all-features all-targets testleri, default/all-feature
Clippy ve fmt geçti. Cache quota/eviction/owner testleri, kayıt leaf sınırları
ve kimlik eşitsizliği, source corruption + valid cache, kesilme, ACK/önce/sonra
process kill ve mixed normal/streamed writers kontrolleri dahil. Loglar
`bounded-speed-{tests-feature,tests-default,tests-all,clippy-all,clippy-default,fmt}.log`.
0.1 fixture ve dış projeler değiştirilmedi. Corpus üretilmedi veya taşınmadı;
yazma testleri yalnız sabit FDB slotunu sıfırlar; SQLite slotu yeniden yazılmaz.
Son slot10k/sequence20/delta0 olarak bırakıldı. Normal eski reader da tam10k
belge/version ve4 query'yi okudu (`bounded-speed-normal-reader-verify.jsonl`).
Son kontrol correctness içindir, yeni performance ölçümü değildir. Tekrar
normal-profil ingest yapılarak gereksiz bir DB yeniden oluşturulmadı.

[OPEN-WORK](OPEN-WORK.md): uzun içerik okuma, yeni dar sorgu farkı, yazma/bakım,
disk/eski nesiller, normal API/global heap ve eşzamanlı/soğuk yük işleri açık.
