# Capped N=50 ve biçim değiştirmeyen exact deneyleri — 50k, 28 Ekim 2026

Karar: **CRC kontrolünü koruyan 4 işçili exact ve capped N=50, beş orta/nadir
sorgunun tamamında p95≤150ms kapısını geçti.** Serial capped, sıfır sonuçlu
transubstantiation'da kapıyı geçmedi. CRC yeniden kullanmak zorunlu değil.
Varsayılan `search` exact/serial, strict-open ve kalıcı biçimler değişmedi.
Yeni API'ler opt-in; normal protokole endpoint eklenmedi. Alt ajan kullanılmadı.

## Kapsam ve ölçüm

Mevcut ilk50k, aynı post-recovery sequence104/50k canlı/100 silinmiş model;
aynı sabit FDB/SQLite slotları, yeni corpus/DB kopyası/reset yok. Havuz SHA ve
bütün .fdb/.ready ile0.1 fixture koruması ham tasarımda. Aynı sekiz kitaptan
örtüşen2–50KiB pasajlar; gerçek bağımsız dosya karışımı sayılmaz. Her sorgu50
belgeyi tüm payload'ıyla döndürüyor. Uzun metinler yalnız sayaçlarla ölçülmedi:
gerçek proses duvar süresi ve kernel+user CPU'su ayrı alındı.

Sıra FDB/SQLite, sonra SQLite/FDB; her kolda10 tekrar, pooled20 örnek/sorgu/
seçenek. Her proses önce tam get/version/deleted oracle, ardından her sorgu/
seçenek için bir ısınma turu. Sorgu ve seçenek sırası döndürülür. OS cache sıcak;
gerçek cache eviction yapılmadı. Kontrol/aday aynı proses ve aynı mantıksal DB.
Normal derleme p95 için; ayrı diagnostic derleme2 tekrar/sorgu/seçenek yalnız
aday/okuma/CRC sayaçları için. Profilin iç içe veya paralel duvar süreleri
CPU diye sunulmaz. Windows GetProcessTimes tüm işçi thread'lerini kapsar;
tek örnekte15,625ms kuantalama/0 görülebilir, CPU toplamı ve ortalaması esastır.
Oracle ve log yazma zamanlama dışındadır; hafif OS bellek örnekleyicisi dahildir.

SQLite FTS5 trigram/detail=none/columnsize=0, WAL/FULL, cache2MiB, mmap0,
temp_store=FILE; önceki ayarlar korunur. Capped karşılaştırması COUNT(*)'yi
çıkarır; ORDER BY id ve payload yüklemesi kalır. Bu SQL planı FTS adaylarını
sıralamadan önce tüketebilir; FDB'nin global erken durma algoritmasıyla aynı
uygulama değildir. Sonuç sırası ve lower-bound total sözleşmesi aynıdır.

## API ve sınırlar

`Database` ve `Snapshot`: `search_first`, `search_first_with_execution` ve
`search_with_execution`. FirstPage: sequence, hits, validation ve
`Exact(n)`/`AtLeast(n)`. N'e ulaşıldığında N+1 okunmaz: tam N eşleşme olsa da
`AtLeast(N)` dürüst alt sınırdır. Tükenince `Exact(n)`. Offset0, limit1..200;
normal exact pagination korunur. Query süresi sonrasında total tamamlama işi yok.

Serial first-page tüm run'ların posting cursor'larını key heap ile birleştirir;
aktif delta en fazla mevcut bounded katman kadar tam kontrol edilir. Heap'ten
sıradaki aday doğrulanır; N sonuçta durur. Bu, run başına N toplama değildir.
Paralel deney ise **run başına bounded prefix** doğrular; her run N'e kadar,
128 adaylık bloklarla ilerler ve global key heap ilkN'i seçer. Negatifte yararlı,
pozitifte serial kadar az iş yapmaz. Her koşunun bütün hit payload/version'ı
exact ilk50 ile eşleşti. Worker sayısı2/4 yalnız açık seçenekle; sorguya özgü
kelime optimizasyonu yok. Süreçte deneysel işler için ortak4 doğrulama permit'i;
başka deneysel sorgu varsa sayı azalabilir veya admission beklenebilir.

Posting scratch serial tüm cursor'larda8MiB, deneysel worker başına512KiB;
metadata/term cache mevcut ortak bütçeleri kullanır. Prefetch256KiB aralık,
8KiB azami komşu boşluğu; seçenek üst sınırı64KiB. Bunlar **toplam proses RAM
sınırı değildir**. Mevcut aktif delta doğrulaması ve dönen belge Vec'i ayrıca
bellek taşır. Çok geniş numeric union workspace'i aşarsa açık Invalid döner.

## Pooled p95 — ms

Deney1=new cursor serial kontrolü;2/4 onun işçi sayısı. Join=8KiB komşuluk;
CRC=pinli doğrulanmış kaynak CRC yeniden kullanımı. Legacy mevcut exact API.

| Sorgu | Legacy exact | Deney1 | 2 işçi | 4 işçi | Join | CRC | 4+Join+CRC |
|---|---:|---:|---:|---:|---:|---:|---:|
| harpoon | 325.243 | 326.644 | 196.621 | 114.303 | 354.572 | 325.021 | 101.497 |
| presumption | 433.673 | 407.367 | 251.376 | 141.766 | 399.094 | 363.335 | 124.397 |
| darcy | 261.044 | 248.711 | 165.162 | 92.407 | 273.305 | 232.734 | 79.936 |
| transubstantiation | 289.170 | 277.579 | 179.517 | 98.380 | 279.607 | 245.182 | 89.650 |
| holmes elementary | 214.399 | 227.170 | 120.316 | 70.733 | 209.189 | 183.543 | 68.919 |

| Sorgu | Capped serial | Capped4 CRC açık | Capped4+Join+CRC reuse | SQLite exact | SQLite capped |
|---|---:|---:|---:|---:|---:|
| harpoon | 6.879 | 37.090 | 32.834 | 2605.966 | 1305.635 |
| presumption | 16.220 | 72.241 | 63.378 | 2887.106 | 1482.609 |
| darcy | 6.835 | 30.936 | 26.107 | 2117.443 | 1094.881 |
| transubstantiation | 288.635 | 99.831 | 91.778 | 2618.580 | 1334.402 |
| holmes elementary | 47.327 | 71.324 | 66.263 | 1698.351 | 854.779 |

## CPU maliyeti — bütün işçiler dahil

Hücre `ortalama CPU ms / 20 sorgu toplam CPU ms`. Her sorgu için duvar p95 ile
aynı örneklerdir; CPU p95 ile duvar p95'in aynı örneğe ait olduğu iddia edilmez.

| Sorgu | Legacy | Deney1 | 2 işçi | 4 işçi | Join | CRC | 4+Join+CRC |
|---|---:|---:|---:|---:|---:|---:|---:|
| harpoon | 308.6 / 6171.9 | 302.3 / 6046.9 | 348.4 / 6968.8 | 379.7 / 7593.8 | 310.2 / 6203.1 | 282.0 / 5640.6 | 329.7 / 6593.8 |
| presumption | 383.6 / 7671.9 | 383.6 / 7671.9 | 449.2 / 8984.4 | 482.0 / 9640.6 | 371.1 / 7421.9 | 340.6 / 6812.5 | 421.1 / 8421.9 |
| darcy | 246.1 / 4921.9 | 243.8 / 4875.0 | 289.1 / 5781.2 | 307.8 / 6156.2 | 240.6 / 4812.5 | 213.3 / 4265.6 | 284.4 / 5687.5 |
| transubstantiation | 264.1 / 5281.2 | 263.3 / 5265.6 | 309.4 / 6187.5 | 330.5 / 6609.4 | 261.7 / 5234.4 | 232.8 / 4656.2 | 296.9 / 5937.5 |
| holmes elementary | 184.4 / 3687.5 | 182.8 / 3656.2 | 221.1 / 4421.9 | 228.9 / 4578.1 | 181.2 / 3625.0 | 172.7 / 3453.1 | 226.6 / 4531.2 |

| Sorgu | Capped serial | Capped4 | Capped4+Join+CRC | SQLite exact | SQLite capped |
|---|---:|---:|---:|---:|---:|
| harpoon | 7.0 / 140.6 | 117.2 / 2343.8 | 97.7 / 1953.1 | 2460.9 / 49218.8 | 1236.7 / 24734.4 |
| presumption | 14.1 / 281.2 | 244.5 / 4890.6 | 218.0 / 4359.4 | 2749.2 / 54984.4 | 1372.7 / 27453.1 |
| darcy | 5.5 / 109.4 | 101.6 / 2031.2 | 87.5 / 1750.0 | 2008.6 / 40171.9 | 999.2 / 19984.4 |
| transubstantiation | 266.4 / 5328.1 | 335.9 / 6718.8 | 305.5 / 6109.4 | 2518.8 / 50375.0 | 1264.1 / 25281.2 |
| holmes elementary | 40.6 / 812.5 | 232.0 / 4640.6 | 228.1 / 4562.5 | 1529.7 / 30593.8 | 787.5 / 15750.0 |

4 işçili exact'in gecikmesi düştü, CPU'su arttı: presumption383,6→482,0ms
ortalama CPU. Serial capped pozitif sorguda çok daha az CPU/I/O harcıyor;
4 işçili per-run prefix bunun bir kısmını kaybediyor. Bu deney CPU tasarrufu
veya4R+yazıcı throughput başarısı değildir. İlk hazır sonuçta öncelik için
CRC'yi koruyan4 işçi kullanılabilir; caller CPU/batarya önceliğinde serial seçer.
Otomatik uyarlama/persistent worker pool bu tur uygulanmadı.

## Okuma birleştirme ve erken durma kanıtı

Tanılama ortalamaları; normal performans tablosu değildir. Payload I/O sayaçları
aday doğrulamayı kapsar; son hit hydration ve posting I/O ayrı olabilir.

| Sorgu | Serial capped aday / MiB | Paralel capped aday / MiB | Exact istek | Join istek | Exact/Join MiB |
|---|---:|---:|---:|---:|---:|
| harpoon | 93 / 2.97 | 3237 / 102.74 | 10651 | 8132 | 338.87 / 340.12 |
| presumption | 160 / 5.56 | 6198 / 212.31 | 12635 | 9073 | 437.52 / 439.49 |
| darcy | 92 / 2.81 | 3248 / 97.69 | 10584 | 8163 | 320.89 / 322.24 |
| transubstantiation | 7563 / 267.81 | 7563 / 267.81 | 7563 | 6297 | 267.81 / 268.59 |
| holmes elementary | 673 / 21.66 | 4620 / 147.00 | 4620 | 4148 | 147.00 / 147.27 |

Join istek sayısını azalttı fakat boşluk byte'ı ve ek metadata işi yüzünden
p95'i tutarlı iyileştirmedi: harpoon326,6→354,6; darcy248,7→273,3ms.
Tek başına join ve tek başına CRC reuse150ms kapısını geçmedi. Bunlar varsayılan
olmadı. Legacy presumption otomatik window yolu441,38MiB okurken yeni deney
exact437,52MiB: iki serial kontrolün I/O davranışı birebir aynı değil; Deney1
satırı bu algoritma farkını görünür kılar. İşçi katkısı Deney1'e göre değerlendirilir.

CRC reuse yalnız strict doğrulanmış native kaynak + Windows immutable pin +
covering metadata guard varsa etkin. Reparse/unpinned/legacy fallback'te CRC
atlamaz. Diagnostic exact-crc'de aday sayısı kadar reuse sayacı gözlendi.
Payload sınır/schema/key/version kontrolleri ve dönen hit'in CRC'si korunur.
**Taviz:** açılıştan sonra donanım/raw disk/RAM değişimi pin tarafından
önlenmez; sonuç olmayan adayda yeni bozulma CRC tekrarından önce fark
edilmeyebilir. Bu opt-in deney güvenlik eşdeğeri diye sunulmaz. Seçilen4 işçili
kapı sonucu CRC reuse gerektirmiyor.

Normal proseslerde fresh strict-open sonrası private4,20–4,22MiB civarı;
bütün query deneyleri +50k oracle descriptor'ları dahil lifetime peak private
30,67 /31,73MiB, working set34,86 /35,52MiB. Bu sayaçlar yalnız motorun
idle/query RAM'i değildir; seçenek başına izole RAM ölçülmedi. Diskte yeni
kalıcı kayıt yok; kaynak1.348.135.347B, index/cache414.334.392B aynı kaldı.

## Kapı ve kalanlar

| Kapı | Sonuç |
|---|---|
| Exact4, beş sorguda p95≤150ms, CRC korunarak | GEÇTİ; en yüksek141,766ms |
| Capped N=50/4 işçi aynı kapı | GEÇTİ; en yüksek99,831ms |
| Capped serial aynı kapı | KALDI; negatif288,635ms; gizlenmedi |
| Coalescing veya CRC reuse tek başına150ms | KALDI |
| Ölçülen bütün sorgularda payload/version/total oracle | GEÇTİ |
| Geniş38-case/yeni API, kill/reopen ve eski okuyucu | Son doğrulama eki aşağıda |

Hızlı açılış kapısı bundan ayrıdır; [güncel tasarım](CAPPED-SEARCH-FAST-OPEN-DESIGN-50K-20261028.md)
arka planda kaynak+SEG05+indeks CRC ve ilk arama≤300ms ister. Query kapısı
geçtiği için bu prototipe geçildi. Biçim değiştiren filtreler yalnız
[maliyet/aday analizi](PREFILTER-OPTIONS-50K-20261028.md), uygulama onayı bekler.
[Anahtar sırası, sınır ve ters-mtime](KEY-ORDER-FIRST-N-20261028.md).

50k tekli commit/dirty500/4R+yazıcı/100'lü commit-RAM profili, uzun snapshot ve
gerçek soğuk cache ayrı açık işlerdir.100k için gerçek dosya corpus'u beklenir.
Reclaim default kapalı; case34 ve parçalı geniş zx/the zx taraması açık.
Blok3/4, C ABI ve Frunner entegrasyonu başlamadı.

Ham kanıt: [program/SHA/sıra/koruma](benchmarks/block2-capped-exact-50k-20261028-v1-design.json),
[pooled bütün seçenekler/CPU ve diagnostic sayaçlar](benchmarks/block2-capped-exact-50k-20261028-v1-analysis.json).
Ham sorgular `block2-capped-exact-50k-20261028-v1-pair{0|1}-{normal|sqlite}.jsonl`;
karşılaştırmadan ayrı profil `...-counters-diagnostic.jsonl`. p50/max dahil
bütün kötü örnekler ham kayıtta korunur; p95 nearest-rank ceil(.95*n).

## Son doğrulama — 28 Ekim

Default `cargo test --all-targets`: **237 geçti,0 başarısız,14 ignored subprocess
yardımcı**. Diagnostic lib: **143 geçti,0 başarısız,12 ignored yardımcı**.
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, diagnostic
clippy, no-default lib ve all-features lib check; native SQLite fmt/clippy/test
geçti. Yeni capped/parallel/coalesced/CRC seçenekleri küçük çok-run, range/link,
delta güncelleme/silme, snapshot sabit epoch ve reopen oracle'ında karşılaştırıldı.
Eski0.1 kaynak kopyası yeni API'lerle okunuyor; fixture'ın kendisi korunuyor.
Sağlam cache+bozuk kaynak testi ve streamed süreç kill sonrası capped/exact4
sonuç kontrolleri genişletildi; bütün eski WAL/flush/merge/cache kill aileleri
pakette tekrar geçti. Güç kesintisi garantisi iddia edilmedi.

50k yeni API oracle'ı38 case için kill öncesi ve sonrası; Pending background
oturumunda gözlenmiş process kill; donmuş24 Ekim okuyucuyla50k payload/version,
100 silinmiş get ve38 özgün pagination sorgusu tekrar geçti.50k'da bu tur yeni
churn/ingest/merge-kill yapılmadı. Corpus,0.1 fixture, .fdb/.ready ve aynı-model
ledger SHA'ları eşit; kalıcı biçim ve varsayılanlar korunur.

[Komutlar/exit code ve loglar](benchmarks/snapshot-10k-20261026-validation-capped-background-50k-20261028-v1.json),
[diagnostic test](benchmarks/block2-capped-background-50k-20261028-v1-diagnostic-tests.log),
[diagnostic clippy](benchmarks/block2-capped-background-50k-20261028-v1-diagnostic-clippy.log).
