# Kullanıcı korpusu: Türkçe Wikipedia, offline aday/eşleşme — 28 Ekim

**100.000 kayıt ve100.000 farklı metin gövdesi CRC/SHA ile doğrulandı.**
Verilen `D:/frunner-corpus/trwiki-100k/corpus.payload-bench` yalnız okundu;
yeni corpus üretilmedi/indirilmedi/kopyalanmadı. D: üzerinde DB kurulmadı.
Motorun kalıcı biçimi, ön filtre, bitset ve position listesi değiştirilmedi.
Bu rapor **offline içerik analizidir; engine ingest/query/open benchmark'ı değildir.**

Kaynak SHA256: `3D8BF5D4B8868C6C50644020B7998542E2589498BF99E3233DEDE8AD5613E91F`.
Başlık ve kayıt CRC'leri, kayıt sırası/sayısı, EOF, SHA, body uniqueness,
her orijinal key'in shared/texts/10-basamaklı numeric ID ile eşitliği ve link
scope'ları kontrol edildi. Alanlar ve linkler korunacak; kaynakta100.000 link
var. Eski benchmark'ın2–50KiB kırpma/revision-marker yolu bu corpus'ta kullanılmaz.
Uzun yol/ters-mtime key stratejisi hâlâ Frunner kararıdır; benchmark kayıtları
verilen sayısal anahtarları kullanır. Kaynak içeriği ağdan gönderilmedi; gövdeler
loglanmadı, `private/` içeriği okunmadı. Oracle kataloğu yalnız offset/CRC/size/
eşleşme bitmap'i içerir, kaynak belge metinlerini içermez.

İlk deneme bütün CRC/SHA/uniqueness kontrolünü bitirip “linkler boş olmalı”
harness kontrolünde durdu; DB reset/ingest yapılmadı. Bu kontrol gereğinden
katıydı: tüm orijinal anahtarların mevcut ID şablonuyla aynı olduğu yeni
preflight'ta doğrulandı. Key veya link rewrite gerekmiyor. Bu durum ham audit'te
korunur; kaynak verisinin bozukluğu veya motor hatası olarak sunulmaz.

## Corpus sınırı ve boyutlar

Veri Türkçe Wikipedia makaleleridir; kod/PDF/DOCX/gerçek dosya değişikliği
karışımı değildir. Manifest'in tohumlu rastgele sıra beyanı kaynak bilgisi olarak
alındı; rastgelelik istatistiksel olarak ispatlanmadı. Bütün motor karşılaştırmaları
aynı ilkN önekiyle yapılmalı. Verilen word-DF sayıları oracle diye kullanılmadı:
FDB semantiği normalize title OR text substring ve whitespace-separated AND.
Türkçe folding nedeniyle producer word-DF'leriyle sayılar farklı olabilir.

| İlkN | Metin byte | En kısa B | p50 B | p95 B | En uzun B |
|---|---:|---:|---:|---:|---:|
| 10000 | 31735118 | 512 | 1511 | 10799 | 112940 |
| 50000 | 157148362 | 511 | 1479 | 10690 | 200636 |
| 100000 | 312917193 | 511 | 1478 | 10673 | 444345 |

## Offline3-gram presence adayları

Her sorgu kelimesinin bütün3-gramlarının title/text birleşiminde bulunması
aday sayılır; son doğrulama her kelimenin **tek parça substring** olarak title
veya text'te olmasını ister. Bu adaylar FDB'nin runtime measured candidate
sayacı değildir: tombstone/mask, exact universal/bigram proof ve term-page
özetleri burada çalıştırılmadı.2 karakterli kelimede3-gram yok, N belge üst
sınırı aday sayılır. Dolayısıyla kısa sorgu oranını gerçek payload I/O sayısı
olarak okumamak gerekir.0/0 “aday da sonuç da yok”; sonsuz “pozitif aday/0 sonuç”.

| Sorgu | 50k aday | 50k gerçek | Aday/gerçek | 100k aday | 100k gerçek | Aday/gerçek |
|---|---:|---:|---:|---:|---:|---:|
| olduğu | 16062 | 14299 | 1.123× | 31990 | 28407 | 1.126× |
| büyük | 15632 | 15316 | 1.021× | 31285 | 30642 | 1.021× |
| ayrıca | 15687 | 14965 | 1.048× | 31310 | 29883 | 1.048× |
| arasında | 25589 | 15854 | 1.614× | 50958 | 31555 | 1.615× |
| birlikte | 13363 | 10219 | 1.308× | 26587 | 20347 | 1.307× |
| sanat | 11035 | 6171 | 1.788× | 22049 | 12436 | 1.773× |
| görenler | 10852 | 3379 | 3.212× | 21473 | 6572 | 3.267× |
| millî | 4614 | 4603 | 1.002× | 9280 | 9256 | 1.003× |
| ikinci | 11906 | 9031 | 1.318× | 23700 | 17915 | 1.323× |
| toplam | 9115 | 5685 | 1.603× | 18206 | 11405 | 1.596× |
| isimli | 9085 | 2763 | 3.288× | 18123 | 5458 | 3.320× |
| öldürür | 3178 | 402 | 7.905× | 6370 | 812 | 7.845× |
| ölümü | 11415 | 7952 | 1.435× | 22772 | 15800 | 1.441× |
| akarsu | 1866 | 515 | 3.623× | 3622 | 960 | 3.773× |
| demokratik | 1305 | 904 | 1.444× | 2588 | 1806 | 1.433× |
| çalışmalarıyla | 5417 | 249 | 21.755× | 10755 | 505 | 21.297× |
| üreticisi | 3258 | 424 | 7.684× | 6384 | 815 | 7.833× |
| joseph | 1048 | 1027 | 1.020× | 2094 | 2051 | 1.021× |
| leyla | 4250 | 287 | 14.808× | 8453 | 591 | 14.303× |
| protest | 1692 | 912 | 1.855× | 3407 | 1822 | 1.870× |
| furlong | 50 | 4 | 12.500× | 86 | 9 | 9.556× |
| korkaklığa | 185 | 2 | 92.500× | 342 | 3 | 114.000× |
| ormanların | 8310 | 194 | 42.835× | 16580 | 404 | 41.040× |
| yanıklarda | 5013 | 4 | 1253.250× | 9930 | 8 | 1241.250× |
| külliyatı | 3180 | 27 | 117.778× | 6282 | 80 | 78.525× |
| timmy | 231 | 22 | 10.500× | 491 | 42 | 11.690× |
| yüklük | 3148 | 508 | 6.197× | 6154 | 983 | 6.260× |
| ilçesine köyleri | 2150 | 1313 | 1.637× | 4203 | 2590 | 1.623× |
| geldiği buldu | 3627 | 365 | 9.937× | 7237 | 726 | 9.968× |
| chicago studio | 59 | 33 | 1.788× | 122 | 55 | 2.218× |
| kısım dişli | 5452 | 110 | 49.564× | 10874 | 212 | 51.292× |
| güzeli zafer | 279 | 16 | 17.438× | 563 | 37 | 15.216× |
| İnternetten hazinem | 646 | 1 | 646.000× | 1271 | 1 | 1271.000× |
| oyuncudur atlanta | 503 | 10 | 50.300× | 970 | 18 | 53.889× |
| kendisine çıkacağını | 2519 | 17 | 148.176× | 4953 | 34 | 145.676× |
| ve | 50000 | 47523 | 1.052× | 100000 | 94949 | 1.053× |
| if | 50000 | 22579 | 2.214× | 100000 | 45182 | 2.213× |
| id | 50000 | 38276 | 1.306× | 100000 | 76428 | 1.308× |
| zx | 50000 | 28 | 1785.714× | 100000 | 47 | 2127.660× |
| qzxkvwnever | 0 | 0 | 0/0 | 0 | 0 | 0/0 |
| transubstantiationz | 1 | 0 | sonsuz | 1 | 0 | sonsuz |

## 4-gramın yalnız offline karşılığı — uygulama yok

Uzun kelime için4-gram presence,3 harflide eski trigram fallback;2 harflide
ön filtre yok. Buradaki sayılar payload aday potansiyelini gösterir, disk/
ingest/RAM maliyeti veya yeni sorgu p95 ölçümü değildir. Bitset yazılmadı,
position verisi üretilmedi; DB/cache'e alan eklenmedi.

| Sorgu | 50k3-gram | 50k4-gram | 50k gerçek | 100k3-gram | 100k4-gram | 100k gerçek |
|---|---:|---:|---:|---:|---:|---:|
| olduğu | 16062 | 14620 | 14299 | 31990 | 29092 | 28407 |
| büyük | 15632 | 15316 | 15316 | 31285 | 30644 | 30642 |
| ayrıca | 15687 | 14967 | 14965 | 31310 | 29885 | 29883 |
| arasında | 25589 | 18363 | 15854 | 50958 | 36513 | 31555 |
| birlikte | 13363 | 10451 | 10219 | 26587 | 20812 | 20347 |
| sanat | 11035 | 6409 | 6171 | 22049 | 12897 | 12436 |
| görenler | 10852 | 3834 | 3379 | 21473 | 7480 | 6572 |
| millî | 4614 | 4604 | 4603 | 9280 | 9258 | 9256 |
| ikinci | 11906 | 9079 | 9031 | 23700 | 18004 | 17915 |
| toplam | 9115 | 5810 | 5685 | 18206 | 11637 | 11405 |
| isimli | 9085 | 3994 | 2763 | 18123 | 7892 | 5458 |
| öldürür | 3178 | 1088 | 402 | 6370 | 2150 | 812 |
| ölümü | 11415 | 8125 | 7952 | 22772 | 16133 | 15800 |
| akarsu | 1866 | 563 | 515 | 3622 | 1039 | 960 |
| demokratik | 1305 | 958 | 904 | 2588 | 1901 | 1806 |
| çalışmalarıyla | 5417 | 2084 | 249 | 10755 | 4155 | 505 |
| üreticisi | 3258 | 713 | 424 | 6384 | 1394 | 815 |
| joseph | 1048 | 1027 | 1027 | 2094 | 2051 | 2051 |
| leyla | 4250 | 301 | 287 | 8453 | 616 | 591 |
| protest | 1692 | 934 | 912 | 3407 | 1867 | 1822 |
| furlong | 50 | 4 | 4 | 86 | 9 | 9 |
| korkaklığa | 185 | 10 | 2 | 342 | 19 | 3 |
| ormanların | 8310 | 2061 | 194 | 16580 | 4113 | 404 |
| yanıklarda | 5013 | 661 | 4 | 9930 | 1290 | 8 |
| külliyatı | 3180 | 43 | 27 | 6282 | 109 | 80 |
| timmy | 231 | 26 | 22 | 491 | 47 | 42 |
| yüklük | 3148 | 604 | 508 | 6154 | 1183 | 983 |
| ilçesine köyleri | 2150 | 1549 | 1313 | 4203 | 3022 | 2590 |
| geldiği buldu | 3627 | 674 | 365 | 7237 | 1301 | 726 |
| chicago studio | 59 | 34 | 33 | 122 | 57 | 55 |
| kısım dişli | 5452 | 537 | 110 | 10874 | 1029 | 212 |
| güzeli zafer | 279 | 47 | 16 | 563 | 85 | 37 |
| İnternetten hazinem | 646 | 10 | 1 | 1271 | 14 | 1 |
| oyuncudur atlanta | 503 | 22 | 10 | 970 | 42 | 18 |
| kendisine çıkacağını | 2519 | 189 | 17 | 4953 | 362 | 34 |
| ve | 50000 | 50000 | 47523 | 100000 | 100000 | 94949 |
| if | 50000 | 50000 | 22579 | 100000 | 100000 | 45182 |
| id | 50000 | 50000 | 38276 | 100000 | 100000 | 76428 |
| zx | 50000 | 50000 | 28 | 100000 | 100000 | 47 |
| qzxkvwnever | 0 | 0 | 0 | 0 | 0 | 0 |
| transubstantiationz | 1 | 0 | 0 | 1 | 0 | 0 |

Örnek:100k'da çalışmalarıyla10.755/505 (**21,30×**),4-gramda4155/505
(**8,23×**) kalıyor.4-gram burada da yanlış pozitifi tamamen çözmüyor.
Furlong86/9→9/9, akarsu3622/960→1039/960. Negatif
transubstantiationz1/0→0/0; qzxkvwnever zaten0/0. Bu negatifler kitap corpus'undaki
7563/0 kadar ağır değildir; mevcut eski negatif stress case'i melez kontrolünden
çıkarılmadı. Ön filtrelerin hiçbiri bu bulgularla otomatik onaylanmış sayılmaz.

Açılış kabul sınırı 50k≤1000ms, 100k≤2000ms; doğrulama atlanmayacak ve
kaynak başına lazy açma yapılmayacak. Sonraki kullanıcı kararıyla eski melez
sonucunun +%6,50 negatif p95 / +%1,93 pozitif CPU farkı kabul edildi.
Yeni corpus'taki engine ölçümleri ayrı [ölçek raporunda](TRWIKI-SCALE-20261028.md)
gösterilir. Eski corpus'un açılış sonucuna yeni sınırı uygulamak yeni corpus'u
geçmiş saymak değildir; bu offline tablo runtime performansı değildir.

[Ham audit, prefix sayımları ve provenance](benchmarks/trwiki-provided-20261028-v1-audit.json),
[framed offset/CRC ve41-case oracle metadata](benchmarks/trwiki-provided-20261028-v1-catalog.json),
[audit stdout](benchmarks/trwiki-provided-20261028-v1-audit.stdout).

50k tekli/dirty500/4R+yazıcı/100'lü profil, RAM/disk altında eşzamanlı yük,
uzun snapshot ve gerçek-soğuk cache açık. Case34/geniş parçalı tarama izleniyor.
Blok3/4, C ABI, corpus üretimi ve Frunner entegrasyonu yapılmadı.
