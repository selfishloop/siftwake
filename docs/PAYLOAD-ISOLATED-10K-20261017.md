# Ayrı yolda payload gruplama — 2026-10-17

Sonuç: bu ikinci prototip de reddedildi, varsayılan motora alınmadı.
the/200 gerilemesi bu koşuda tekrarlanmadı; darcy'de dar kazanç var,
holmes'ta p95 kazancı tekrarlanmadı. İki normal kelimede tekrarlanan
kazanç kabulü geçmedi. Deneme burada kapatıldı; küçük farklar A/A oranı
çıkarılarak başarıya çevrilmedi. Motor ve window_tests f6d18dd'a byte-eşit
döndü. [Kod ve üç hedefli test arşivi](benchmarks/payload-isolated-20261017-prototype.patch).

Önceki denemenin ardından yalnız planlama/kuyruğu hızlı döngüden ayırma
hipotezi sınandı. Yeni tarama, codec, metadata veya indeks formatı tasarımı
bu deneye eklenmedi. Genel index seçicisi sadece benchmark aracında kaldı.

## Değişiklik ve sınırı

Bilinen adaylar yine64 slot/128KiB, en fazla4KiB gap ve%5 ek veri bütçesiyle
okunur. CRC/schema/UTF-8/key/version ve canonical contains korunur. Önceki
binary_candidate_batch algoritması aynıdır. Bu kez posting/kesişim/aday
kuyruğu ve gruplama ayrı, inline edilmeyen visit_candidate_matches içindedir.
Normal matches/visit_matches fonksiyonlarında sadece başta tek uygunluk
kontrolü ve dispatch vardır. Boş verification_terms için seed sayısı bile
toplanmaz. Mevcut hızlı döngüye pending tamponu veya per-record batch dalı
eklenmez. Büyük belge ve geniş-seed/JSON/rkyv overlay yolları önceki okuyucudadır.

Yeni helper ve iki dispatch kaldırılınca segment.rs taban f6d18dd ile
byte-eşittir; [kaynak yapısı kanıtı](benchmarks/payload-isolated-structure-20261017.json).
Bu, makine kodu yerleşiminin aynı veya zamanlama etkisinin sıfır olduğunun
kanıtı değildir. Önceki the/200 artışının kök nedeni henüz kesin ayrılmadı.

## Önce dar kabul kontrolü

Mevcut38-case oracle'dan genel index seçicisiyle8=darcy,11=holmes,
31=the offset0/limit200 alınır; üçü de her tur ölçülür. Motor sorgu adına
göre yol seçmez. Aynı değişmiş harness hem eski hem yeni motorda derlenir.
Dört süreç çifti×4 strict blok×25 tur:400 örnek/sorgu/variant; başlangıç
sırası blok ve çiftler arasında dengelidir. Tanılama kapalıdır.

Hedef iki normal kelimede tekrarlanan kazanç, the/200'de gerilemenin
tekrarlanmamasıdır. Pooled p95, süreç çiftleri ve eşlenmiş blok medyanları
ayrı değerlendirilir. Dar kontrol başarısızsa bu yaklaşım kapatılır ve
varsayılan motora alınmaz. Geçerse38-case ve ayrı query/get bellek ölçümü
yapılmadan terfi verilmez. Küçük değişiklikler A/A oranları çıkarılarak
başarıya çevrilmez; belirsizlik varsa kabul verilmez.

## Veri, doğrulama ve ölçülmeyenler

Yeni corpus veya DB yok. Mevcut ilk10.000 belge/8 kitap;264.842.858B text,
2.057–51.197B pasajlar. FDB349.805.698B/18 dosya, sequence20/delta0/5 Run;
SQLite583.860.224B ve0.1 fixture de snapshot ile izlenir. Normal strict open
tam source/index kontrolü yapar, pending0; açılış ve oracle süre dışında.
Oracle tüm dönen belge/alan/link/version/total/sıra eşitliğini süre dışında
denetler. OS cache sıcak, eviction yok; gerçek-cold veya concurrent ölçümü
değildir. Windows11/Ryzen7 7840HS, Rust1.99/MSVC release thin-LTO/cgu1,
affinity4/normal priority/Balanced; frekans/SMT/arka plan izolasyonu yok.
Read API çağrısı fiziksel disk transfer sayısı değildir. Diagnostic süreler
ayrı derlemededir; iç içe span'ler toplanmaz veya normal sorgu süresi yapılmaz.

Üç hedefli test geçti: önceki exact-vs-batch/bozulma/kesilme kontrolleri ve
yeni union numeric range+exact field+link+hidden/tombstone+AND/Unicode+
prefix toplam/sıra testi. Son test sadece256 küçük kayıtlık birim fixture'dır;
benchmark corpus'u veya DB slotu üretmez. All-features/all-targets Clippy
ön kontrolü temizdi. Final kontroller aşağıdadır.

## Dar normal A/B sonucu

İki motor aynı güncel harness ile release derlendi; diagnostics ve ek
thread sayaçları kapalı. Her sorguda400 örnek/variant,4 süreç çifti ve16
eşlenmiş blok. Bu16 blok16 bağımsız süreç değildir. Ortam/güç/frekans
izole edilmediği için küçük süre farkları kesin algoritmik etki sayılmaz.

| Sorgu, offset/limit | Kontrol p95 ms | Aday p95 ms | Fark | Tipik blok fark | Hızlı çift/blok |
|---|---:|---:|---:|---:|---:|
| darcy,0/50 |50,1665|48,7942|-%2,74|-%2,32|3/4;12/16|
| holmes,0/50 |26,8812|27,1439|+%0,98|-%1,39|2/4;12/16|
| the,0/200 |4,9337|4,7865|-%2,98|-%0,88|2/4;11/16|

holmes süreç çiftlerinin p95 farkları -%5,78/-%3,90/+%3,58/+%4,33;
önceki denemedeki -%9,18 ve4/4 düşük p95 tekrarlanmadı. Bu, yeni sürümün
kesin%1 yavaşladığı kanıtı da değildir; güvenilir hedef kazancı gösterilemedi.
the/200'de ilk prototipteki +%6–10 artış bu koşuda yoktur. Ancak harness
sorgu karışımı farklı, programlar yeniden derlenmiştir; bunun tamamını
yalnız helper ayrımına bağlamak veya kök nedeni çözüldü saymak doğru olmaz.
Tipik blok ve pooled p95 aynı dağılım ölçüsü değildir.

[Ham sonuç ve bütün süreç/blok farkları](benchmarks/payload-isolated-focus-10k-20261017-summary.json),
[program SHA'ları](benchmarks/payload-isolated-20261017-build-manifest.log).
Dar kapı geçmediği için yeni38-case, ayrı RAM veya diagnostic süre koşusu
yapılmadı; bu alanlarda yeni kazanç veya regresyonsuzluk iddiası yoktur.
Eski prototipin read sayısı azalması bu sürüm için yeni ölçüm gibi sunulmaz.

## Final doğrulama

17 Ekim'de başlayan deneyin final kontrolü18 Ekim'de tamamlandı; artifact
prefix'leri başlangıç tarihini korur. Geri dönülen motorda
`cargo test --all-targets`:180 geçti/0 başarısız/7 ignored. Üç prototip
testi arşiv patch'indedir; final engine'de kullanılmayan helper bırakılmadı.
`cargo fmt --check`, default ve all-features/all-targets Clippy `-D warnings`
geçti. Normal default release CLI ve iki okuma aracı yeniden derlendi.
Bu tur reference/all-features tam test suite'i yeniden çalıştırılmadı;
runtime aynı tabandadır, önceki sonuçlar yeni koşu gibi sunulmaz.

Final gerçek CLI:10.000 belge/sequence20, darcy1220/50 hit, holmes1222/50,
the10000/200; toplam/sayfa/sürüm kontrolleri geçti. Ölçüm önce/sonra ve
final21 dosya hash-eşit:18 FDB dosyası349.805.698B, SQLite ve iki0.1 fixture.
Corpus'un tamamı2.740.862.279B, SHA-256
`11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B`
ile akışlı yeniden doğrulandı. Yeni veri dizini yok. Arşiv patch'i
`git apply --check` ile tekrar uygulanabilir; doğrulamada uygulanmadı.
Kaynak/format/feature/bağımlılıklar aynı, engine ve window_tests byte-eşit.
[Final kanıt](benchmarks/payload-isolated-final-proof-20261017.json),
[denetim aracı](../tools/prove-isolated-payload.py).

## Sonraki sınır

Okuma çağrısını azaltmak, doğrulanan/taranan payload byte'larını azaltmadı.
Bu iki gruplama prototipine yeni ayar turu eklenmeyecek. Arama hedefi açık:
sonraki bağımsız işte mevcut profilde byte doğrulama, normalleştirme/tarama
ve sonuç hazırlamanın kalan maliyeti ayrılmalı; en pahalı tekrara göre tek
değişiklik seçilmeli. Bu rapor yeni bir tarama çözümü veya yeni ölçüm değildir.

## Açık işler

Yazma/bakım, genel arama p95 sapmaları, normal Vec/query/caller RAM'i,
açılış, disk sıkıştırma ve eşzamanlı/gerçek-cold yük açık. Bu dar deneme
bu başlıklarda yeni bir performans sonucu değildir.
