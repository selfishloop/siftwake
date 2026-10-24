# RAM gram posting sözlüğüne sınırlı toplu ekleme — 24 Ekim 2026

Aynı mevcut ilk 10k belgeyle normal release ölçümünde akışlı yazma+bakım
14,381→13,783 s (-%4,16), iki çiftte de düşük; proses CPU'su -%6,21.
Owned API'de CPU -%4,96 ve yazma fazı -%3,13 olsa da yazma+bakım
12,779→12,803 s (+%0,19), yalnız 1/2 çift düşük. Son bakım akışlı +%26,07,
owned +%11,97; iki API'de de iki çift yüksek. Genel yazma/bakım veya RAM
başarısı ilan edilmez. Akışlı toplam ve iki API'deki CPU kazancı dar
uygulamayı korumanın gerekçesidir; owned gecikmesi ve son bakım işi açık kalır.
İlk 38 durumda 23 düşük/15 yüksek p95 vardır; yoğun tekrarda the/1 +%11,56,
the/200 +%3,95 ve quixote/28/10 +%8,53 pooled p95 kaldı. Genel arama
kabulü geçmedi; bakım, RAM ve okuma riskleri açıkça korunur.

Önceki hızlı intake fold, sınırlı gram sıralama ve diğer ölçülmüş kazanımlar
korunur. Değişiklik yalnız mevcut RAM gram sözlüğüne ekleme yolundadır;
kalıcı format, feature/dependency listesi, WAL/durability ve sorgu semantiği
değişmez. Kaynak `.fdb`/`.ready`, türetilmiş `.idx` ve 0.1 fixture sınırı aynıdır.

## Teşhis ve ölçülmeyen sınırlar

Tek `diagnostic-timing` akışlı çalışmada aynı ilk 10k belge için gram posting
döngüsü 4,081 s, gövde gram çıkarımı 3,933 s ölçüldü. Döngüye sunulan gram
sayısı 19.128.006'dır. Seyrek örneklerde 16.190 lookup toplam 4.543.500 ns
(örnek başına yaklaşık 281 ns), mevcut posting'e 16.038 insert toplam
1.081.700 ns (yaklaşık 67 ns) verdi. Örneklerde 11.243 dense, 4.275 sparse,
520 tiny posting ve 152 eksik anahtar vardır. Lookup maliyeti bu dar
hipotezi destekler; bütün döngü CPU'sunun kesin ayrıştırılması değildir.

Örnekleme belge/ID tabanlıdır; rastgele veya her temsil için eşit dağılım
garantisi yoktur. Sayaç/Instant ve kilit maliyeti vardır; bu diagnostic
executable normal release performans karşılaştırması değildir. İç içe ve
worker ile örtüşen span'ler toplanamaz. Örnek süreleri bütün gram sayısıyla
çarpılarak toplam lookup/insert süresi veya CPU payı hesaplanmaz. Aday
etkinken ayrıntılı örnek sayaçları yalnız scalar fallback belgelerini,
`index.gram_posting_loop` span'i ise iki yolu da kapsar. Bu tur adayın yeni
diagnostic profili alınmadı. [Ham teşhis](benchmarks/posting-profile-before-20261024-fdb-ingest-10000-0.jsonl).

## Dar uygulama

Başlık ve gövdenin mevcut sıralı String gram kümelerinin birleşimi, 64'e kadar
referans içeren geçici parçalarla mevcut `BTreeMap<String, Posting>` üzerinde
ilerler. Her parçada ilk/son anahtar arasındaki `range_mut`, input gram
sayısının en fazla sekiz katı sözlük girdisini ziyaret eder. Eşleşen
posting'lere mevcut insert uygulanır. Eksik görülen veya ziyaret sınırı
sonrasında kalan anahtarların tamamı range bırakıldıktan sonra exact lookup
yoluna gider; sınırın tükenmesi anahtarın bulunmadığına kanıt sayılmaz.

Başlık+gövde gram sayısı üst sınırı 64'ten azsa veya sözlük bu üst sınırın
16 katından büyükse önceki scalar yol korunur. Aynı gram iki alanda da varsa
bir kez eklenir. Yeniden kullanılan RAM slotu kalıcı sıra sayılmaz. Mevcut
Tiny/Sparse/Dense posting davranışı ve builder gram allocation hesabı
korunur. İki geçici Vec'in her birinde en fazla 64 referans bulunur; bu
yaklaşık 1 KiB eleman alanı allocator, gram kümeleri, sözlük veya toplam
proses RAM sınırı değildir. Cache boyutu veya kalıcı yerleşim değişmez.

## Aynı programda read-only ön test

Yalnız mevcut `data/rkyv-corpus-20261009/json.payload-bench` ilk 10.000 belge,
264.842.858 özgün metin byte'ı kullanıldı. Corpus indirilmedi/üretilmedi;
DB açılmadı veya sıfırlanmadı. RAM haritaları her 500 belgede boşaltıldı;
zamanlı blok 50 belgedir. Her blokta iki yolun sırası değişir. CRC/JSON,
fold, gram çıkarımı, eşitlik kontrolü ve drop süre dışında kalır.

| Dar ölçü | Scalar lookup | Sınırlı toplu ekleme |
|---|---:|---:|
| 200 zamanlı bloğun toplamı | 3,214019 s | 2,499467 s |
| Süre farkı | — | -%22,23 |
| Daha hızlı eşli blok | — | 200/200 |

Toplu yol 9.862/10.000 belgede seçildi. Her blok sonunda bütün anahtarlar,
sıralı posting ID'leri, posting heap byte'ları ve builder gram byte hesabı
birebir eşit. Bu tek süreçte RAM insertion aşamasıdır; bağımsız süreç
tekrarı, DB yazması, bakım veya OS RAM kazancı değildir. OS cache sıcak;
eviction, güç/donanım izolasyonu yoktur. [Ham ön test](benchmarks/posting-merge-probe-10k-20261024-run0.log).

## Normal release yazma ve son bakım

Kontrol, önceki gram sıralamayı içeren dondurulmuş normal program;
aday da normal default build, diagnostic kapalıdır. Her API'de iki süreç
çifti, sıra eski/yeni ardından yeni/eski; toplam sekiz süreç vardır.
İşler ardışık, OS cache sıcak; eviction ve güç/donanım izolasyonu yoktur.
Sadece aynı ilk 10k, 20×500 kayıt ve synced WAL commit kullanıldı.
CRC/JSON ile 40 sınır get'i ingest süresindedir. Son bakım ingest sonrasındaki
flush/merge drain'dir; worker işi ingest ile örtüşebilir. Toplam her
sürecin ingest+drain'inden hesaplanır. CPU bütün proses thread'lerinin OS
toplamıdır. Ayrı faz medyanları toplanıp yeni bir genel sonuç üretilmez.

| Medyan, saniye | Kontrol | Aday | Fark | Düşük çift |
|---|---:|---:|---:|---:|
| Akışlı yazma | 13,8365 | 13,0967 | -%5,35 | 2/2 |
| Akışlı son bakım | 0,5442 | 0,6860 | **+%26,07** | **0/2** |
| Akışlı yazma+bakım | 14,3806 | 13,7828 | **-%4,16** | **2/2** |
| Akışlı proses CPU | 12,8281 | 12,0313 | -%6,21 | 2/2 |
| Owned yazma | 9,9687 | 9,6567 | -%3,13 | 2/2 |
| Owned son bakım | 2,8099 | 3,1463 | **+%11,97** | **0/2** |
| Owned yazma+bakım | 12,7786 | 12,8030 | **+%0,19** | **1/2** |
| Owned proses CPU | 11,6641 | 11,0859 | -%4,96 | 2/2 |

Akışlı toplam çift farkları -%3,45/-%4,87; owned -%1,46/+%1,84.
Son bakım akışlı çiftlerde +%39,89/+%12,51, owned +%12,60/+%11,38.
Bakım artışı küçümsenmez veya CPU azalmasıyla kapatılmaz. Worker
zamanlaması, allocation/bellek erişimi ve executable yerleşimi katkıları
olabilir; hiçbirine kesin neden atfedilmedi. Bu uygulama bağımsız bir
merge hızlandırması değildir. İki çift genel hız veya istatistiksel
anlamlılık garantisi vermez. Önceki tarihli ölçümlerle yüzdeler birleştirilmez.

SQLite bu tur yeniden yazılmadı veya süre ölçümüne katılmadı; önceki SQLite
sonuçları yeni FDB medyanlarıyla güncel eşli kıyas sayılmaz.
[Tasarım ve özet](benchmarks/posting-merge-write-10k-20261024-summary.json),
[bütün süreç kayıtları](benchmarks/posting-merge-write-10k-20261024-records.json).

## RAM, disk ve veri koruması

OS lifetime private commit fiziksel veya toplam RAM değildir. Working set
paylaşılan image sayfalarını da içerir. Faz örneklemesi kısa tepeleri
kaçırabilir; tabloda süreç sonuna tutulan handle'ın lifetime tepeleri vardır.

| OS tepe sayacı | Kontrol | Aday | Medyan farkı | Düşük çift |
|---|---:|---:|---:|---:|
| Akışlı private commit | 10.407.936 B | 10.577.920 B | **+166 KiB** | **0/2** |
| Akışlı working set | 14.585.856 B | 14.991.360 B | **+396 KiB** | **0/2** |
| Owned private commit | 83.951.616 B | 83.947.520 B | -4 KiB | 1/2 |
| Owned working set | 87.711.744 B | 87.517.184 B | -190 KiB | 2/2 |

Akışlı iki OS tepesi de iki çiftte yüksek. Owned private yaklaşık aynı ve
yaklaşık 80 MiB düzeyinde kalır. Küçük geçici batch tamponundan bu OS
farklarının nedeni çıkarılmaz; genel RAM kapısı geçmedi.

Bütün sekiz süreç sonunda core disk 349.805.698 B, değişim 0 B. Aynı API
içinde kontrol/adayı kaynak, indeks ve ready dosyaları byte/SHA-eşit.
Her süreçten sonra zaman dışında 10k tam payload ve dört contains sayfası
(`the`, `quixote`, `he`, `the man`) oracle'ı geçti. Yalnız sahipliği
kontrol edilen sabit `data/benchmark-work/fdb` yazma alanı sıfırlandı;
yeni DB kopyası, corpus veya disk taşıması yoktur. Dinamik geçici alan
tepesi ölçülmedi.

Corpus ve 0.1 fixture kaynakları normal seri öncesi/sonrası SHA-eşit.
Ayrıca diagnostic koşu öncesi koruma manifesti normal seri sonrası
manifestle yol normalize edilerek karşılaştırıldı: aynı corpus ve iki
fixture'ın üç SHA'sı da eşit. Böylece koruma aralığı diagnostic öncesinden
normal seri sonrasına uzanır; bu karşılaştırmada corpus tekrar okunmadı.
[Normal seri öncesi](benchmarks/posting-merge-write-10k-20261024-protected-before.json),
[normal seri sonrası](benchmarks/posting-merge-write-10k-20261024-protected-after.json),
[diagnostic öncesiyle tam aralık eşlemesi](benchmarks/posting-merge-protected-fullspan-20261024.json).

## Okuma: geniş kontrol ve seçilmiş yoğun tekrar

Aynı son 10k veritabanında normal kontrol/aday, 38 sorgu/sayfa için iki
süreç çifti×iki blok×beş tekrarla çalıştı: durum başına 20 örnek/yol.
CPU maskesi 4, blok başlangıç sırası dengeli; her blokta strict kaynak
doğrulaması ve ısınma süre dışında. İki resident oracle worker vardır;
DB sorguları eşzamanlı değildir. Bu düzen sürekli açık tek süreç veya
eşzamanlı yük testi değildir. Tam payload/sıra/version oracle'ı geçti;
öncesi/sonrası izlenen 21 dosya SHA-eşittir. Pooled p95 23 durumda düşük,
15 durumda yüksek; az örnekli bu tablo genel arama kapısını geçirmez.
[İlk 38 durum](benchmarks/posting-merge-read-10k-20261024-summary.json).

Önceki açık beagle/frankenstein/the200 ile yeni geniş kontroldeki the/1
ve quixote/28/10 için bir yoğun tekrar tamamlandı. Aynı program/veri ve
CPU maskesiyle iki çift×iki blok×25 tekrar: durum başına 100 örnek/yol.
Yine tam oracle ve önce/sonra 21 dosyanın SHA eşitliği geçti. İlk geniş
setle seçilmiş yoğun set farklı dağılımlardır; yüzdeleri birleştirilmez.

| Sorgu / offset / limit | Kontrol p95, ms | Aday p95, ms | P95 farkı | Hızlı çift | Tipik blok farkı |
|---|---:|---:|---:|---:|---:|
| beagle /0/50 | 20,8686 | 20,4213 | -%2,14 | 1/2 | -%1,14 |
| frankenstein /0/50 | 14,5203 | 14,2280 | -%2,01 | **0/2** | -%3,19 |
| the /0/1 | 0,2154 | 0,2403 | **+%11,56** | **0/2** | -%5,24 |
| the /0/200 | 4,6102 | 4,7922 | **+%3,95** | 1/2 | -%5,01 |
| quixote /28/10 | 2,0241 | 2,1968 | **+%8,53** | 1/2 | -%0,55 |

Frankenstein pooled p95 düşük olsa da çift p95 farkları +%2,14/+%0,94;
iki çiftte de yavaştır. Pooled kuyruk ile çift dağılımı ayrı ölçülerdir;
yalnız pooled sonuca bakarak bu açık kapatılmaz. Beagle yalnız 1/2 çift
hızlıdır. The/1 iki çiftte +%13,80/+%1,80; 0,0249 ms pooled artış
korunur. The200 çiftleri +%5,86/-%2,78, quixote -%4,24/+%18,28 ile
karma olsa da pooled artışları yok sayılmaz. Tipik blok medyanları beş
durumda düşük; bu p95 artışlarını silmez. Genel arama kapısı geçmedi.

Sorgu algoritması değişmez, ancak executable'ların yerleşimi aynı değildir.
Ortam/oturum/yerleşim katkıları kesin ayrıştırılmadı; artışlar gürültü diye
kapatılmaz. Önceki raporların farklı kontrol ve oturumlarında ölçülmüş
regresyonları bu yeni yüzdeler otomatik kapatmaz. Bu native okuma, CLI JSON
round-trip kuyruğunu veya genel açılış/gerçek-soğuk/eşzamanlı yükü ölçmez.
Query/get OS bellek tepesi bu okuma serilerinde yeniden ölçülmedi.
[Yoğun beş durum](benchmarks/posting-merge-read-focused-10k-20261024-summary.json).

## Doğrulama ve açık işler

Odak testler ziyaret cap'i sonrası mevcut/eksik anahtarları, başlık/gövde
örtüşmesini, UTF-8/NUL, tekrar/yeniden kullanılan ve uzak ID'leri, 65 gramla
parça sınırını, Tiny→Sparse→Dense→Sparse ve tam builder heap muhasebesini
kapsar. Default index 17, all-features index 19, no-default index 5 test;
streamed görünürlük/numaralı sayfa kontrolü 1 test geçti. Ignored corpus
probe'ları olağan suite içinde çalıştırılmaz. `cargo fmt --check`, default/
all-features/no-default `cargo clippy --all-targets -- -D warnings` ve
`git diff --check` geçti. Normal release CLI ve iki ölçüm example'ı derlenip
ayrı programlar olarak donduruldu.
[Ham kontroller](benchmarks/posting-merge-checks-20261024.log),
[normal program SHA'ları](benchmarks/posting-merge-frozen-20261024.json).
Tamamlanmış kanıtların dizini [makine özetindedir](benchmarks/posting-merge-10k-20261024-proof.json);
bu dosya yeni bir test çıktısı değildir.

Bu RAM ekleme değişikliği depolama/kurtarma algoritmasını değiştirmez;
bu tur yeni kesilme/bozulma/kill testi çalıştırılmadı. Genel açılış,
gerçek-soğuk ve eşzamanlı yük yeniden ölçülmedi. Dar CPU/akışlı toplam
kazancı owned toplam gecikmesini, son bakım artışını, mevcut arama p95/CLI
kuyruklarını, caller/query/genel RAM'i veya disk sıkıştırma/geçici alan
işlerini kapatmaz. [Açık iş listesi](OPEN-WORK.md) korunur.
