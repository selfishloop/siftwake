# Exact-total maliyeti ve açılış profili — 50k, 28 Ekim 2026

28 Ekim son karar/uygulama: capped N50 ve fast tasarım onaylandı.
[Arama kapısı geçti](CAPPED-EXACT-50K-20261028.md);
[background ilk arama≤300ms kapısı kaldı](BACKGROUND-OPEN-50K-20261028.md).
Aşağıdaki “henüz uygulanmadı/yanıt bekliyor” ifadeleri profil anının tarihsel durumudur.

**Ölçüm tamamlandı; yeni arama/açılış API'leri henüz uygulanmadı.**
Kullanıcının son kararıyla önceki ≤5× ölçek kapısı kaldırıldı. Yeni orta/nadir
sorgu hedefi p95≤150ms@50k, ≤300ms@100k; yaygın sorgu≤30ms. zx/the zx
geniş taraması bilinen sınırdır. Bu rapor beş orta sıklıktaki sorgunun
exact yolunu ve mevcut strict-open maliyetini ayırır. API tasarımı
[ayrı belgede](CAPPED-SEARCH-FAST-OPEN-DESIGN-50K-20261028.md).

| Kapı / iş | Kanıt | Durum |
|---|---|---|
| Beş hedef exact-total sorgu ≤150ms | Aşağıdaki20 örnek/case pooled p95; beşi de150ms üstü | KALDI, baseline |
| Yaygın≤30ms | Önceki50k raporunda max7,392ms; bu tur hedef5 sorgu seçildi | Önceki GEÇTİ korunur, yeniden ölçülmedi |
| Capped N=50 ≤150ms | Henüz uygulanmadı; sözleşme/kapı seçimi değerlendirmede | ÖLÇÜLMEDİ |
| Hızlı oturum <100ms | Tasarım kullanıcı tarafından onaylandı; capped kapısından sonra | UYGULANMADI / ÖLÇÜLMEDİ |
| Kaynak/0.1/kalıcı biçim | FDB kaynakları ve protected dosyalar byte-eşit; yazıcı mantığı değişmedi | GEÇTİ |
| Doğruluk/uyumluluk | Her örnekte full page oracle; normal paket233 test, diagnostic139; eski okuyucu50k/38-case | GEÇTİ |
| Sonraki50k yazma, 2c, 2b, 100k | API/kapı kararı beklenir;100k gerçek corpus'u kullanıcı sağlayacak | BAŞLANMADI |

## Aynı veri ve ölçüm sözleşmesi

- Mevcut sabit FDB/SQLite50k DB'leri kullanıldı; sıfırlama/corpus üretme/
  indirme/DB kopyası yok. FDB, önceki kill+temizlikten kalan temiz50k
  durumudur: ilk50k donör havuzu ve bakım testlerinin sınırlı değişiklikleri.
- SQLite başlangıçtaki temiz50k'dan, mevcut FDB ledger'ındaki son
  payload/version'lara dört hazırlık commit'iyle eşitlendi; bu işlemler
  sorgu zamanlaması dışındadır. İki model dosyasının SHA'sı birebir aynı.
- FDB→SQLite, SQLite→FDB;10 örnek/case/koşu, pooled20, nearest-rank p95.
  Aynı5 sorgu her turda döndürülerek çalıştırıldı. Her backend'de full
  get/version oracle ve beş warmup sonrası ölçüm; her sayfa ayrıca doğrulandı.
- Cache: OS cache düşürülmedi. Bu bir warmup prosedürüdür; tüm dosyaların
  RAM'de kalmasını veya arka plan izolasyonunu garanti etmez. Gerçek cold
  test yapılmadı. SQLite FTS5 trigram/detail=none, FULL WAL, cache2MiB,
  mmap0, tempFILE; önceki LIKE/AND count+ordered-page yolu aynı.
- Normal binary p95 için kullanılır. Ayrı diagnostic binary'de iç içe
  duvar süreleri/çağrı/byte sayaçları alınır; bunlar CPU attribution değildir.
  Yüzdeler instrumented query süresine göredir, normal p95'e taşınmaz.
- İki ilk diagnostic koşu aynı sorguyu arka arkaya5 kez ölçer. Ana aşama
  tablosu ayrıca normalle aynı dönüşümlü sıradaki10 diagnostic örneğe dayanır;
  cache etkisini sırayla değiştirmiş iki veri seti birleştirilmez.

## Normal exact-total sonucu

| Sorgu | Gerçek total | Dönen hit | FDB p95 ms | FDB max ms | SQLite p95 ms |150ms |
|---|---:|---:|---:|---:|---:|---|
| harpoon | 5212 | 50 | 330.937 | 382.668 | 2514.381 | KALDI |
| presumption | 4318 | 50 | 407.713 | 408.351 | 2806.036 | KALDI |
| darcy | 5983 | 50 | 276.485 | 280.188 | 2132.974 | KALDI |
| transubstantiation | 0 | 0 | 280.923 | 287.588 | 2558.545 | KALDI |
| holmes elementary | 282 | 50 | 201.813 | 205.117 | 1612.934 | KALDI |

Çıktı50 hit ile sınırlı olsa da exact total işi sınırlamıyor. Sonuç adedi
bilgi amaçlıdır; eski10k/50k oranları bir kabul kapısı değildir. Güncel
50k modelindeki küçük churn farkı nedeniyle önceki10k zamanlarıyla yeni
bir doğrudan A/B yapıldığı iddia edilmez.

## Adaylar, I/O ve metin — dönüşümlü diagnostic ortalamaları

| Sorgu | Payload adayları | Okuma isteği | Okunan MiB | Tanılama toplam ms | Aday doğrulama ms/% |
|---|---:|---:|---:|---:|---:|
| harpoon | 10651 | 10651 | 338.87 | 348.72 | 335.66 / 96.3% |
| presumption | 12635 | 12575 | 441.38 | 461.40 | 441.23 / 95.6% |
| darcy | 10584 | 10584 | 320.89 | 290.91 | 282.36 / 97.1% |
| transubstantiation | 7563 | 7563 | 267.81 | 304.56 | 281.52 / 92.4% |
| holmes elementary | 4620 | 4620 | 147.00 | 212.15 | 194.42 / 91.6% |

| Sorgu | Payload okuma ms | CRC ms | Binary schema ms | Metin kontrolü ms | Posting hazırlık ms | Kesişim ms |
|---|---:|---:|---:|---:|---:|---:|
| harpoon | 111.08 | 35.00 | 39.83 | 132.22 | 6.84 | 2.72 |
| presumption | 146.47 | 47.29 | 45.60 | 179.29 | 10.48 | 5.32 |
| darcy | 108.78 | 34.00 | 34.40 | 87.13 | 3.98 | 1.49 |
| transubstantiation | 88.58 | 28.20 | 25.70 | 124.76 | 15.78 | 4.95 |
| holmes elementary | 55.39 | 15.92 | 19.13 | 91.78 | 12.75 | 2.31 |

Payload/CRC/schema/metin süreleri aday doğrulama üst süresinin içindedir;
bunları üst süreyle tekrar toplamayın. Metin kontrolü katlama, tarama ve
mevcut ASCII positive proof yolunu kapsar. Okunan byte mantıksal source
isteğidir; OS cache miss ya da fiziksel disk byte sayısı değildir.

| Sorgu | Metadata cache hit/miss | Term lookup hit/miss | Term page miss |
|---|---:|---:|---:|
| harpoon | 899.3/858.7 | 12.5/143.5 | 143.5 |
| presumption | 885.4/872.6 | 20.0/240.0 | 216.0 |
| darcy | 886.4/871.6 | 24.4/79.6 | 79.6 |
| transubstantiation | 788.4/817.6 | 9.6/432.4 | 320.4 |
| holmes elementary | 832.2/907.8 | 3.4/334.6 | 262.6 |

Bunlar motorun mevcut metadata256KiB, term-directory1MiB içindeki64KiB
lookup bütçelerinin sayaçlarıdır; OS cache hit/miss ölçümü değildir.
Cache miss vardır ama temel maliyet aday doğrulamadır. Yalnız sözlük
cache'ini artırmanın bütün150ms açığını kapatacağı söylenmez.

## Total sorusunun cevabı ve capped sınırı

Bu beş uzun/AND sorguda **posting kesişiminden geçen bütün görünür
payload adayları** kontrol ediliyor. Bütün50k belge her zaman okunmuyor;
tabloda aday sayıları görülüyor. İlk50 hit capture edildikten sonra
exact count için döngü devam ediyor. Sayım ayrı ikinci geçiş değil;
her doğrulanmış eşleşmedeki increment. Maliyet, bu increment'i yapabilmek
için ilk sayfadan sonra da doğrulamayı sürdürmek.

Capped olumlu sorgularda erken durabilir; bu henüz ölçülmüş bir hız
kazancı değildir. **Transubstantiation total=0:**7563 false-positive
trigram adayı/267,81MiB doğrulanıyor. Capped doğru sıfır sonucu için yine
tüketmelidir. Bu sorgu150ms'yi aşarsa kullanıcı kuralıyla durulacak;
kendi kendimize negatif/uzun kelime istisnası tanımlamadık.

## Strict-open aşamaları

Normal, yeni proses açılış örnekleri(ms): 2378.830, 1011.677, 859.561.
İlk kötü2378,830ms örneği korunur. OS cache evict edilmedi; bu örnek
gerçek-soğuk diye adlandırılmaz, dalgalanmanın kök nedeni bu tur ölçülmedi.
Üç örnek p95 üretim garantisi değildir. Diagnostic aşamalar ayrı:

| Aşama | r0 ms | r1 ms | r2 ms | Ortalama / toplam % |
|---|---:|---:|---:|---:|
| Toplam | 997.458 | 830.523 | 833.245 | 887.076 / 100.0% |
| Kaynak tam native/CRC/schema | 617.607 | 511.275 | 506.769 | 545.217 / 61.5% |
| İndeks açılışı | 343.603 | 294.400 | 297.407 | 311.803 / 35.1% |
| ↳ Metadata hazırlığı | 204.611 | 181.019 | 184.472 | 190.034 / 21.4% |
| ↳↳ SEG05 directory parse | 199.018 | 176.466 | 180.015 | 185.166 / 20.9% |
| ↳ İndeks CRC | 122.348 | 98.930 | 98.521 | 106.600 / 12.0% |

Kaynak ve indeks dışı kalan toplam ortalama 30.055ms. Kaynak ve
indeks üst aşamaları ayrıdır; metadata/parse/CRC satırları indeksin
içindedir. Yalnız kaynak kontrolünü erteleyince bile indeks294–344ms
kalıyor. **CRC ertelemesi tek başına <100ms hedefini sağlamıyor.**

## Açılış sınır I/O probu — işlevsel DB değildir

Beş tekrar 13.006–14.915ms;26 run,81 dosya açma,2.247MB mantıksal okuma.
DB kilidi, manifest CRC, source boyut/magic/footer sınırları, SEG05
başlık/son bağları ve mevcut MET04 checksum/root header kontrol edildi.
**Kaynak bütünlüğü, tam indeks CRC ve SEG05/MET04 record karşılaştırması
yapılmadı; Database/Snapshot kurulmadı, sorgu çalışmadı.** Bu sayı hızlı
açılış geçti değildir; yalnız ertelemeye uygun I/O alt işinin maliyetidir.
Hızlı oturum dönüşü, ilk sonuç/kaynak beklemesi ve tüm doğrulama bitişi
gerçek prototipte ayrı ölçülmeden100ms kapısı kapatılamaz.

## Güvenlik, uyumluluk, devam

- Normal test paketi233 geçti; diagnostic lib139 geçti/12 subprocess yardımcı ignored. fmt/clippy, no-default/all-features, native SQLite kontrolleri geçti.
- Mevcut strict kaynak kesilme/tek-byte bozulma ve process-kill testleri
  tekrar geçti. Önceki gerçek50k dört kill penceresi aynı blokta kabul
  edilmişti; bu read-only profilde yeni50k kill yükü oluşturulmadı.
- Donmuş24Ekim okuyucu50k payload/version,100 deleted get ve38 sorguyu
  tekrar doğruladı. Corpus/0.1 fixture/FDB source bytes aynı. SQLite
  yalnız zamanlama dışı model eşitleme için değişti.
- Ürün değişikliği yalnız mevcut diagnostic-timing altında sayaçtır;
  default query, storage/recovery ve codec mantığı değişmedi. Capped
  API, lazy open, yeni kalıcı dosya/sürüm yok. Reclaim varsayılan kapalı.
- Hızlı oturum tasarımı onaylandı; **capped N=50/150ms kapı seçimi ve
  uygulama yanıtı bekleniyor**. Fast prototype ancak capped adımının
  kapısı izin verince. 50k tekli/dirty500/4R/100'lü yazı profili,2c,2b
  henüz başlamadı. 100k yeni gerçek dosya corpus'unu bekler.
- Case34, geniş zx/the zx, parçalı read sayısı, query RAM ve yazma/bakım
  açık kalır. Blok3/4 ve C ABI başlamadı.

## Ham kanıt

- [Program hash'leri, sıra, ayarlar, ledger/source SHA](benchmarks/block2-query-open-50k-20261028-v1-design.json).
- [Normal p95 ve bütün diagnostic sayaç ortalamaları](benchmarks/block2-query-open-50k-20261028-analysis-v1.json).
- [Dönüşümlü ham diagnostic](benchmarks/block2-query-open-50k-20261028-v2-interleaved-profile.jsonl).
- [Açılış sınır probu](benchmarks/block2-query-open-50k-20261028-boundary-probe-v1.json).
- [Yeni50k eski okuyucu oracle'ı](benchmarks/block2-query-open-50k-20261028-compat-v1-old-reader-utf8-v2.json).
- [Normal test/fmt/clippy komut kanıtı](benchmarks/snapshot-10k-20261026-validation-query-open-50k-20261028-v1.json).
- [Diagnostic lib testleri](benchmarks/block2-query-open-50k-20261028-v2-diagnostic-lib-tests.log).
- Ham normal sorgular: `block2-query-open-50k-20261028-v1-pair-r{0|1}-{normal|sqlite}-scale-target-matrix.jsonl`; kaynak açılışları: `block2-query-open-50k-20261028-v1-open-r{0|1|2}-*`.
