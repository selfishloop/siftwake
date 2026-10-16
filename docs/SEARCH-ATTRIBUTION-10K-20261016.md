# Yazma adımından sonraki arama farkının ayrıştırılması — 2026-10-16

Sonraki [kısa bloklu kontrolde](INTERLEAVED-SEARCH-10K-20261016.md) iki
farklı-sürüm serisinde8/8 sayfada yeni p95 daha düşük çıktı. Bu yeni
benchmark/cache-oturumu koşullarında gerileme tekrarlanmadı; aşağıdaki
kesintisiz koşuların bütün ortam/yerleşim paylarının açıklaması değildir.

Aramanın daha fazla aday işlemesi, payload okuması, CRC veya metin
doğrulaması yapması bu ölçümde elendi. Sekiz sayfanın tümünde bu işlerin
sayıları iki sürümde birebir aynı; yeni body gram çıkarıcısı aramada hiç
çalışmıyor. Bu, bütün makine kodunun veya bütün sürelerin aynı olduğu
anlamına gelmez.

Kesin bulunan sorun ölçüm ortamının kararsızlığıdır. Aynı programın aynı
mantıksal CPU üzerindeki karşılaştırmasında `ishmael /130/1` p95'i bile
%4,23 yükseldi; önceki farklı-sürüm sonucu %4,87 idi. Önceki farkı kesin
bir motor regresyonu diye sunmak için yeterli kanıt yok. Bunun tamamını
dalgalanma sayıp kapatmak da doğru değildir: yeni farklı-sürüm testinde
özellikle `quixote` farkı sürer. Kod üretimi/yerleşim ile ortam etkisinin
payları henüz ayrılmadı; genel/default kapısı açık kalır.

Bu inceleme motorun arama davranışını değiştirmedi. Mevcut deneysel
yazma optimizasyonu ve önceki %11,49 yazma+bakım sonucu korunur; yeniden
yazma veya bakım ölçümü yapılmadı.

## Kontroller ve kapsam

Aynı mevcut corpus'un ilk 10.000 belgesi, 264.842.858 B gövde,
2.057–51.197 B/pasaj. Mevcut sabit FDB ve SQLite slotları kullanıldı;
corpus üretilmedi, DB sıfırlanmadı veya kopyalanmadı. Windows/Ryzen 7
7840HS, normal release + builder-budget, record-read. Normal testlerde
diagnostic sayaçlar kapalıdır. Sıcak OS cache, eviction yok; ilk kaynak
kontrolü ve bağımsız oracle zamanlama dışındadır.

Her normal batch dört dengeli proses çifti: A/B, B/A, A/B, B/A.
Her sayfada 200 örnek/proses, taraf başına 800 örnek. Sekiz sayfa her
turda karıştırılır; ayrıca her turdaki 256 get işlemi ve tam belge
karşılaştırmaları heap/cache durumunu etkiler. Bu karışık iş yüküdür.
Get/sonuç kontrolleri sorgunun süre aralığı dışındadır. SQLite bu incelemede
yalnız dosya değişmezliği kontrolüne dahildir, yeni SQL hız sonucu yoktur.

Sonraki A/B ve A/A batch'leri affinity mask 4 ile aynı mantıksal CPU 2'ye
sabitlendi. Affinity uygulama gecikmesi ve proses wall/CPU süreleri her
koşunun `-process.json` dosyasındadır. OS güç planı Balanced; plan/frekans
değiştirilmedi. Sabitleme diğer işler, SMT komşusu, sıcaklık, frekans veya
dosya sistemi etkilerini izole etmez. Bu etkilerden hangisinin ne kadar
payı olduğu ölçülmedi.

## Negatif kontrol: aynı program farklı sonuç verebiliyor

İlk A/A batch'inde her iki taraf aynı SHA'lı son adaydı, affinity yoktu.
Son çiftte sekiz sayfanın tamamı %44,79–54,91 daha yavaş göründü.
Bu batch'te kod farkı yoktur; bu değişim bir motor değişikliğinin sonucu
olamaz. Önceki bütün A/B farklarının yalnız ortamdan geldiğini kanıtlamaz.
[Aynı program/sıra/ham örnekler](benchmarks/gram-read-cause-aa-10k-20261016-summary.json).

Aynı CPU'daki yeni A/A ile farklı-sürüm A/B'nin pooled p95 sonuçları:

| Sorgu / offset / limit | A/B farkı | A/B hızlı çift | A/A farkı | A/A hızlı çift |
|---|---:|---:|---:|---:|
| the /0/1 |+%1,53 |1/4 |+%0,84 |2/4 |
| the /0/200 |+%7,31 |1/4 |+%1,76 |2/4 |
| the /5000/50 |+%12,00 |1/4 |-%1,22 |2/4 |
| the /9950/50 |+%8,40 |1/4 |+%1,26 |2/4 |
| ishmael /130/1 |+%3,45 |1/4 |+%4,23 |2/4 |
| ishmael /60/200 |+%14,12 |1/4 |-%0,60 |1/4 |
| quixote /28/10 |+%9,64 |1/4 |+%2,81 |1/4 |
| quixote /57/50 |+%7,84 |1/4 |+%2,72 |1/4 |

Bu batch'ler farklı zaman aralıklarındadır; oranları birbirinden çıkararak
"düzeltilmiş motor gerilemesi" hesaplanmaz. A/B'de ishmael /130/1 çift
farkları +%39,88/-27,48/+5,86/+7,69; A/A'da +%14,2/-1,2/-11,5/+32,3.
İlk %4,87'nin dört çiftte aynı yönde olması, sonraki negatif kontrolün
gösterdiği bu ölçüm sorununu ortadan kaldırmaz. 800 örnek bağımsız 800
proses değildir; dört proses çifti üzerinden nedensellik garantisi çıkmaz.
[A/B tasarım ve örnekler](benchmarks/gram-read-cause-pinned-ab-10k-20261016-summary.json),
[aynı CPU A/A](benchmarks/gram-read-cause-pinned-aa-10k-20261016-summary.json).

## İş sayıları: ek JSON/CRC/tarama yolu yok

İki doğru kaynaktan ayrı diagnostic release üretildi. Dört dengeli çift,
aynı CPU, sekiz sayfa ×30 tekrar/proses: 1.920 tam sonuç doğrulaması.
Her tarafın her sayfasında 120 örneğin tüm sayaç adları ve çağrı sayıları
birebir aynı; yalnız medyanları eşit değildir. Body gram çıkarımı çağrısı 0.

| İş, her sorgu çağrısında | ishmael /130/1 kontrol→aday | quixote /28/10 kontrol→aday |
|---|---:|---:|
| Payload adayı |260→260 |56→56 |
| Payload okuma isteği |260→260 |56→56 |
| Okunan payload byte |8.285.621→8.285.621 |1.945.325→1.945.325 |
| Payload CRC |260→260 |56→56 |
| Payload yapı/UTF-8 doğrulaması |260→260 |56→56 |
| Ham metinden pozitif eşleşme kanıtı |260→260 |56→56 |
| Body fold / scan |0→0 |0→0 |

Buradaki tek sonuç sayfası, toplam eşleşme sayısını hesaplamak için yine
260 adayı doğruluyor. Bu mevcut maliyettir; son yazma adımının getirdiği
ek iş değildir. Sonuç belgesinin hydration/doğrulama sayaçları da iki
sürümde aynıdır. Kaynak doğrulaması ve CRC atlanmadı.

Diagnostic sorgular aynı sayfayı art arda çalıştırır; normal karışık
matrix'in araya get ekleyen cache/allocator durumu aynı değildir.
Instrumented medyan wall / çağıran-thread çevrim farkları:

| Sorgu | Wall farkı | Thread çevrim farkı |
|---|---:|---:|
| ishmael /130/1 |-%10,49 |-%9,93 |
| ishmael /60/200 |-%7,82 |-%7,18 |
| quixote /28/10 |-%5,78 |-%7,39 |
| quixote /57/50 |-%2,61 |-%2,72 |

Bunlar normal-release p95 kazancı değildir. Çift bazında çevrimlerin yönü
de değişir: quixote /28/10 için +%48,0/-35,0/-15,1/-2,4. Böylece bütün
farkı yalnız disk beklemesine veya yeni algoritmanın ek CPU işine bağlayan
bir açıklama desteklenmez. Nested wall span'ler toplanmaz veya CPU payı
sayılmaz. `QueryThreadCycleTime` yalnız çağıran thread'in user+kernel
çevrimlerini verir; instruction-retired sayacı, tüm proses CPU'su veya
frekans ölçümü değildir. Microsoft bu sayacı elapsed time'a çevirmemeyi
belirtir. [API sınırı](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-querythreadcycletime),
[tam iş sayıları/süreler/çevrimler](benchmarks/gram-read-cause-profile-10k-20261016-summary.json).

## Kaynak ve derleyici ayrımı

Engine, segment, covering ve kalıcı dosyalar önceki yazma raporuyla aynı.
Sorgu `grams()` kaynak gövdesi de yazma öncesiyle aynı. Ancak thin LTO ve
tek codegen unit altında bir yazma değişikliği, sorgu makine kodunu sabit
tutmaz. Yeniden üretilen normal kontrol/adayın `.text` SHA'ları ölçülmüş
eski kontrol/adayla ayrı ayrı eşleşti; yanlış eski sürüm olasılığı elendi.

Kontrol→aday `grams()` makine gövdesi 6.647→4.527 B; `search_impl`
9.221→9.221 B, `prefix_matches` 10.209→10.209 B, `read_document`
3.078→3.078 B. Bu fonksiyonların adresleri ve 64-byte satır içi konumları
değişir. Boyut veya byte farkı tek başına fazla instruction veya hız
kaybı kanıtı değildir; çağrı/veri adresleri de byte farkı üretir. Kod
yerleşiminin gerçek katkısı izole edilmedi.
[Normal kod eşleştirmesi](benchmarks/gram-read-cause-code-v2-20261016.json).

İlk teşhis kontrol derlemesinde eski dosya mtime'ının korunması nedeniyle
cache'teki aday kodunun kullanıldığını `.text` karşılaştırması yakaladı.
O teşhis çıktıları kullanılmadı. Kaynak byte'ları açıkça yeniden yazılıp
v2 kontrol üretildi; normal kontrolün eski ölçülmüş kontrolle kod eşitliği
doğrulandı. Kullanılan çevrimli diagnostic çift ayrıca bundan sonra taze
üretildi. Geçici kontrol derlemeleri sonunda aday kaynak byte'ları
birebir geri yüklendi. Önceki normal benchmark executable'ları değişmedi.

## Son doğrulama ve durum

Değişiklikler yalnız ölçüm example'ı ve araçlarıdır: `--profile-pages`,
çağıran thread çevrim sayacı, isteğe bağlı affinity/proses metadatası ve
iş sayısı özetleyicisi. Yeni bağımlılık/feature/kalıcı biçim yoktur.
Normal/default motor davranışı değiştirilmedi. Üç normal batch ve
diagnostic batch öncesi/sonrası DB/index hash'leri korundu; SQLite slotu
da normal batch'lerde aynı kaldı.0.1 fixture değişmedi.

İlgili example'ın dört testi, `cargo fmt --check`, default ve all-features
`cargo clippy --all-targets -- -D warnings` geçti. PowerShell araçlarının
parser kontrolü, Python özetleyicilerinin syntax ve gerçek log kontrolü,
`git diff --check` de geçti. Normal diagnostic-kapalı reader yeniden
üretildi; ölçülen adayla `.text` eşitliği doğrulandı. Son corpus SHA'sı,
139 FDB/cache/SQLite dosyasının hash'i, motor kaynakları ve0.1 fixture
önceki snapshot'la aynı kaldı. Bu tur yeni depolama/kurtarma kodu yoktur;
önceki engine test sayıları bu turun yeni testleri diye sunulmaz.
[Son doğrulama](benchmarks/gram-read-cause-final-proof-20261016.json).

Sonuç: sorguda ek iş regresyonu bulunmadı; negatif kontroller ölçüm
ortamının sorununu kanıtladı. Özellikle quixote'deki kalan farklı-sürüm
farkı kod/yerleşim katkısı açısından açık kalır. Motor için tahmine dayalı
bir düzeltme veya default'a terfi yapılmadı. Yazma/bakımın sonraki merge
ve payload tekrarları, query/normal API RAM'i, disk/eski nesiller,
eşzamanlı ve gerçek cold yük de açıktır. [İş listesi](OPEN-WORK.md).
