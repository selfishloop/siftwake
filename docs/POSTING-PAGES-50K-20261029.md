# Posting sayfaları ve paralel test düzeltmesi — 29 Ekim 2026

**Ana arama kapısı: GEÇTİ; pozitif melez CPU kapısı: KALDI.** Sonuçlar aynı ilk 50k Türkçe
belgede, aynı temizlenmiş DB dosyalarında eski/yeni okuyucu karşılaştırmasıdır.
Kalıcı biçim, 256 KiB entry sınırı ve varsayılan exact/serial semantiği aynı.
Melez N=50/bütçe 200 opt-in; join/CRC reuse ve reclaim default kapalı kalır.

## Uygulama ve güvenlik

`contains()` üyeliği önce sınırlı sayfalardan bir lower-bound bulur. Ardışık
artan adaylarda posting cursor'u oradan ilerler; aday ID geriye giderse veya
sınırlı hint cache'inden düşerse güvenli ikili aramaya döner. Sayfa 1024 ID + iki komşu sınır
ID'sini kapsar. Okunan bütün ID'ler range/strict-order denetiminden geçer;
sayfa sınırı duplicate/inversion gizlemez. `DiskIndex::read_at` üzerindeki
mevcut indeks doğrulaması ve kaynak/payload CRC kontrolleri korunur.

LRU sayfaları ve ilerleme konumları en fazla 29×(4104B +384B metadata allowance)=130152B kontrollü
alan alır. Legacy toplam posting workspace 256 KiB sınırında; serial first-page
8MiB ve worker 512 KiB sınırlarında kalır. İlk sayfada bütün zorunlu cursor'lar
kurulduktan sonra yalnız kalan bütçe kullanılır. Alan yoksa eski küçük-probe
yolu sürer; optional cache yeni mandatory-admission hatası oluşturmaz.
Bu bütçeler allocator veya process RSS ölçümü değildir; OS bellek ayrıca verilir.
Cache sorgu ömründedir, snapshot/source pin mekanizması ve dosyalar aynı.

## Corpus, query, cache, sıra

Kaynak `D:/frunner-corpus/trwiki-100k/corpus.payload-bench` eski/onaylı orijinal
dosyanın ilk 50k'sıdır; SHA/audit korunur. `private/` okunmadı, metin kırpılmadı,
belge/corpus/DB kopyası üretilmedi. V1'de hazırlanan sabit C: FDB/SQLite slotları
yeniden kullanıldı. Hazırlama süresi yeni motorun ingest kazancı sayılmaz.
FDB fiziksel kaynak/indeks yerleşimi bütün okuyucular için aynıdır.

Sıcak OS/engine cache: full GET/version oracle + seçenek başına bir warmup.
Gerçek soğuk cache ölçülmedi. İki ayna sıra: eski/SQLite/yeni ve yeni/SQLite/eski.
İlk sıranın eski okuyucu/SQLite kolları, bir önceki denemede tamamlanmış,
oracle geçen aynı frozen program ve DB koşularıdır; yeniden pool edilmedi,
yalnız bir kez kullanıldı. Aradaki geliştirme/derleme aralığı tasarımda açıktır.
Nihai aday ve ters sıra yeni sürüm doğrulandıktan sonra çalıştı. Önceki aday
sonuçları nihai adayın 20 örneğine katılmadı. Bu aralık tamamen izole laboratuvar
koşulu değildir; sıra dengesi ve büyük kazançların sınırı buna göre okunmalıdır.
10+10=20 normal-release örneği/sorgu/yol; offset 0,N=50. Ana yol melez 200; exact
serial ayrıca zamanlandı. Serial capped/4 işçi bütün 37 için p95 ölçülmedi;
pozitif CPU bloklarında, negatif kapıda ve tanı koşularında ayrıca çalıştırıldı.
Exact-total ve capped sayfalar full hit/payload/version oracle'ıyla aynı.

CPU Windows GetProcessTimes user+kernel, bütün query thread'leri dahil;
tek çağrıda 15,625ms kuantizasyon mümkün. Pozitiflerde ayrıca 2000 çağrılık CPU
blokları verilir. SQLite FTS5 trigram/detail=none/columnsize=0,WAL/FULL,
cache 2 MiB,mmap 0,tempFILE; aynı payload/fold/key sırası. SQLite fields/links'i
payload olarak tutar; FDB bunlara ek scalar/numeric/link indeksleri de kurar.
SQLite capped COUNT yapmaz fakat ORDER BY için adayları tüketebilir.

V1 ön denemesi cursor bütçesi korunumu son düzenlemesi sırasında durduruldu;
hiçbir V1 süresi bu tablolarla pool edilmedi. Derleme ile çakışan yarım candidate
kolu da dışarıda: [duruş kanıtı](benchmarks/posting-pages-20261029-v1-measurement-stop.json).
Son sürüm tüm doğrulamalar bittikten sonra ölçüldü; DB tekrar üretilmedi.

Ara sayfalama denemesi yedi problemli sorguyu 150 ms altına indirdi, fakat
çalışmalarıyla 342,349ms ve kendisine çıkacağını 343,162ms ile KALDI
(ilk sıra 10 örnek p95=max). Tanıda bu iki sorguda 103359/109753 sayfa okuması
görüldü: küçük LRU'da ikili aramanın tekrar ziyaretleri kalıyordu.
[Ara deneme](benchmarks/posting-pages-50k-20261029-v2-pair0-matrix-candidate-scale-query-experiments.jsonl),
[ara tanı](benchmarks/posting-pages-20261029-v2-long-phrase-profile.jsonl).
İlerleyen cursor bu ölçülen maliyet üzerine seçildi; sorgu kelimesine özel
dal veya daha geniş bellek bütçesi eklenmedi. Son pilot 3 örnekte melez
iki sorguda 78/105ms altına inerken serial capped kendisine 238,555ms gördü;
bu küçük pilot ana kapı değildir, kötü serial sonucu korunur.

## Melez N=50 — bütün 37 sorgu

CPU sütunu 20 sorgunun ortalama process CPU ms değeridir; p50/max/toplam CPU
ve bütün ham örneklerin adları JSON analiz/tasarım dosyalarındadır.

| Sorgu | Total oracle | Eski p95 ms | Yeni p95 ms | Fark % | Eski CPU ms | Yeni CPU ms | SQLite cap p95 ms | SQLite CPU ms | Kapı |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| olduğu | 14299 | 8.160 | 2.915 | -64.3 | 8.594 | 3.125 | 345.371 | 313.281 | GEÇTİ (30ms) |
| büyük | 15316 | 5.180 | 2.156 | -58.4 | 4.688 | 4.688 | 316.291 | 287.500 | GEÇTİ (30ms) |
| ayrıca | 14965 | 6.882 | 2.526 | -63.3 | 6.250 | 1.562 | 438.072 | 376.562 | GEÇTİ (30ms) |
| arasında | 15854 | 15.487 | 2.738 | -82.3 | 13.281 | 0.781 | 560.156 | 509.375 | GEÇTİ (30ms) |
| birlikte | 10219 | 14.087 | 3.054 | -78.3 | 11.719 | 3.125 | 335.474 | 302.344 | GEÇTİ (30ms) |
| sanat | 6171 | 10.272 | 3.646 | -64.5 | 8.594 | 3.125 | 340.919 | 291.406 | GEÇTİ (30ms) |
| görenler | 3379 | 38.518 | 7.164 | -81.4 | 31.250 | 4.688 | 409.627 | 323.438 | GEÇTİ (30ms) |
| millî | 4603 | 6.797 | 2.496 | -63.3 | 7.031 | 1.562 | 112.588 | 85.938 | GEÇTİ (30ms) |
| ikinci | 9031 | 13.615 | 3.466 | -74.5 | 8.594 | 1.562 | 412.123 | 358.594 | GEÇTİ (30ms) |
| toplam | 5685 | 11.056 | 3.289 | -70.3 | 9.375 | 3.906 | 291.709 | 261.719 | GEÇTİ (30ms) |
| isimli | 2763 | 28.681 | 7.615 | -73.4 | 23.438 | 3.906 | 447.304 | 391.406 | GEÇTİ (30ms) |
| öldürür | 402 | 87.224 | 28.113 | -67.8 | 147.656 | 52.344 | 181.406 | 166.406 | GEÇTİ (150ms) |
| ölümü | 7952 | 9.066 | 3.545 | -60.9 | 9.375 | 2.344 | 308.675 | 279.688 | GEÇTİ (150ms) |
| akarsu | 515 | 39.590 | 15.763 | -60.2 | 36.719 | 14.062 | 143.142 | 132.031 | GEÇTİ (150ms) |
| demokratik | 904 | 17.628 | 4.261 | -75.8 | 14.844 | 6.250 | 66.678 | 59.375 | GEÇTİ (150ms) |
| çalışmalarıyla | 249 | 768.320 | 78.258 | -89.8 | 1639.062 | 160.156 | 290.757 | 252.344 | GEÇTİ (150ms) |
| üreticisi | 424 | 150.808 | 32.559 | -78.4 | 268.750 | 45.312 | 220.350 | 194.531 | GEÇTİ (150ms) |
| joseph | 1027 | 3.148 | 2.894 | -8.0 | 3.125 | 3.906 | 44.306 | 38.281 | GEÇTİ (150ms) |
| leyla | 287 | 111.035 | 49.874 | -55.1 | 182.031 | 98.438 | 316.880 | 249.219 | GEÇTİ (150ms) |
| protest | 912 | 17.593 | 7.997 | -54.5 | 14.844 | 7.812 | 105.412 | 85.938 | GEÇTİ (150ms) |
| furlong | 4 | 6.841 | 7.386 | +8.0 | 3.906 | 7.812 | 9.911 | 10.156 | GEÇTİ (150ms) |
| korkaklığa | 2 | 46.582 | 31.690 | -32.0 | 43.750 | 31.250 | 38.664 | 25.000 | GEÇTİ (150ms) |
| ormanların | 194 | 877.462 | 119.863 | -86.3 | 2132.031 | 279.688 | 368.683 | 300.000 | GEÇTİ (150ms) |
| yanıklarda | 4 | 751.583 | 133.525 | -82.2 | 1725.000 | 292.969 | 264.475 | 226.562 | GEÇTİ (150ms) |
| külliyatı | 27 | 569.470 | 105.035 | -81.6 | 1225.000 | 235.938 | 215.164 | 195.312 | GEÇTİ (150ms) |
| timmy | 22 | 30.453 | 16.032 | -47.4 | 25.781 | 14.844 | 19.155 | 16.406 | GEÇTİ (150ms) |
| yüklük | 508 | 71.588 | 21.693 | -69.7 | 109.375 | 38.281 | 167.905 | 152.344 | GEÇTİ (150ms) |
| ilçesine köyleri | 1313 | 26.302 | 5.692 | -78.4 | 19.531 | 2.344 | 99.064 | 85.938 | GEÇTİ (150ms) |
| geldiği buldu | 365 | 198.711 | 37.902 | -80.9 | 359.375 | 65.625 | 236.360 | 195.312 | GEÇTİ (150ms) |
| chicago studio | 33 | 10.922 | 8.032 | -26.5 | 9.375 | 5.469 | 10.897 | 9.375 | GEÇTİ (150ms) |
| kısım dişli | 110 | 709.202 | 136.815 | -80.7 | 1670.312 | 308.594 | 308.517 | 265.625 | GEÇTİ (150ms) |
| güzeli zafer | 16 | 62.608 | 38.228 | -38.9 | 54.688 | 34.375 | 34.994 | 30.469 | GEÇTİ (150ms) |
| İnternetten hazinem | 1 | 245.111 | 76.623 | -68.7 | 307.812 | 94.531 | 129.584 | 113.281 | GEÇTİ (150ms) |
| oyuncudur atlanta | 10 | 235.412 | 59.872 | -74.6 | 264.062 | 70.312 | 69.807 | 62.500 | GEÇTİ (150ms) |
| kendisine çıkacağını | 17 | 666.375 | 106.504 | -84.0 | 1438.281 | 211.719 | 199.615 | 183.594 | GEÇTİ (150ms) |
| qzxkvwnever | 0 | 0.687 | 0.696 | +1.2 | 0.000 | 0.000 | 0.527 | 0.000 | GEÇTİ (150ms) |
| transubstantiationz | 0 | 0.890 | 1.005 | +12.9 | 0.000 | 0.000 | 3.456 | 0.781 | GEÇTİ (150ms) |

## Exact/serial — bilgi amaçlı, aynı 37 sorgu

| Sorgu | Eski p95 ms | Yeni p95 ms | Eski CPU ms | Yeni CPU ms | SQLite exact p95 ms | SQLite CPU ms |
|---|---:|---:|---:|---:|---:|---:|
| olduğu | 1739.443 | 318.255 | 1598.438 | 281.250 | 676.638 | 606.250 |
| büyük | 1174.986 | 287.110 | 1064.844 | 260.938 | 668.198 | 567.188 |
| ayrıca | 1817.508 | 331.665 | 1675.000 | 300.781 | 827.388 | 737.500 |
| arasında | 4352.228 | 445.736 | 3992.969 | 408.594 | 1119.781 | 998.438 |
| birlikte | 2593.866 | 330.706 | 2336.719 | 297.656 | 713.769 | 603.906 |
| sanat | 1056.588 | 315.929 | 985.938 | 279.688 | 698.038 | 607.031 |
| görenler | 2587.434 | 409.398 | 2286.719 | 378.906 | 761.757 | 632.031 |
| millî | 397.711 | 92.450 | 314.844 | 79.688 | 181.838 | 158.594 |
| ikinci | 1693.907 | 315.034 | 1481.250 | 280.469 | 821.475 | 725.000 |
| toplam | 1308.528 | 274.617 | 1092.969 | 246.094 | 609.229 | 526.562 |
| isimli | 1411.709 | 338.792 | 1232.812 | 311.719 | 889.097 | 790.625 |
| öldürür | 705.958 | 201.289 | 579.688 | 171.875 | 381.946 | 332.812 |
| ölümü | 1063.200 | 300.924 | 943.750 | 260.938 | 641.162 | 569.531 |
| akarsu | 328.267 | 109.608 | 281.250 | 101.562 | 285.046 | 251.562 |
| demokratik | 304.721 | 56.343 | 271.094 | 49.219 | 128.564 | 112.500 |
| çalışmalarıyla | 2981.084 | 305.438 | 2630.469 | 275.781 | 574.804 | 480.469 |
| üreticisi | 1061.277 | 213.379 | 955.469 | 196.094 | 477.216 | 382.812 |
| joseph | 21.479 | 22.974 | 18.750 | 18.750 | 84.346 | 70.312 |
| leyla | 504.524 | 256.028 | 437.500 | 225.000 | 561.644 | 492.969 |
| protest | 277.953 | 98.767 | 242.969 | 89.062 | 236.765 | 178.906 |
| furlong | 7.824 | 8.153 | 6.250 | 7.031 | 18.537 | 11.719 |
| korkaklığa | 45.638 | 30.601 | 40.625 | 23.438 | 64.187 | 54.688 |
| ormanların | 2540.683 | 343.569 | 2272.656 | 316.406 | 692.859 | 603.906 |
| yanıklarda | 1599.118 | 281.229 | 1430.469 | 255.469 | 537.059 | 454.688 |
| külliyatı | 1158.142 | 226.906 | 1028.906 | 200.000 | 442.161 | 387.500 |
| timmy | 30.940 | 16.464 | 23.438 | 12.500 | 39.273 | 31.250 |
| yüklük | 611.343 | 183.700 | 531.250 | 171.094 | 363.214 | 311.719 |
| ilçesine köyleri | 616.402 | 87.796 | 535.156 | 75.781 | 179.776 | 164.844 |
| geldiği buldu | 1234.422 | 223.472 | 1098.438 | 207.031 | 419.152 | 374.219 |
| chicago studio | 11.002 | 6.574 | 8.594 | 5.469 | 18.575 | 16.406 |
| kısım dişli | 1523.410 | 307.053 | 1378.125 | 275.781 | 607.224 | 535.938 |
| güzeli zafer | 60.203 | 37.143 | 49.219 | 29.688 | 66.456 | 54.688 |
| İnternetten hazinem | 329.956 | 98.502 | 283.594 | 90.625 | 252.898 | 228.906 |
| oyuncudur atlanta | 280.721 | 73.352 | 238.281 | 65.625 | 140.264 | 116.406 |
| kendisine çıkacağını | 1357.517 | 226.754 | 1231.250 | 197.656 | 447.089 | 368.750 |
| qzxkvwnever | 0.022 | 0.021 | 0.000 | 0.000 | 0.245 | 0.000 |
| transubstantiationz | 2.853 | 2.596 | 0.781 | 1.562 | 7.757 | 8.594 |

## Bellek ve ek kapılar

Arama process peak katalog/model ve37 beklenen owned sayfayı da içerir;
motorun tek sorgu heap alanı değildir. Idle yeni proseste oracle yüklenmeden ölçülür.

| Okuyucu | Idle private max MiB | Arama process peak private max MiB | Peak WS max MiB |
|---|---:|---:|---:|
| baseline | 2.227 | 34.840 | 39.152 |
| candidate | 2.129 | 34.633 | 38.859 |
| sqlite | 3.012 | 33.453 | 37.801 |

Negatif kontrol<1ms ise mutlak +2ms, aksi +%10. Tarihsel qzxkvwnever
0,3685→0,612ms (+0,2435ms) yeni kuralla GEÇTİ; eski ham rapor değiştirilmedi.

| Negatif sorgu | Yeni cap4 p95 ms | Yeni melez p95 ms | Mutlak fark ms | İzin verilen ms | Kapı |
|---|---:|---:|---:|---:|---|
| qzxkvwnever | 0.2960 | 0.0319 | -0.2641 | 2.2960 | GEÇTİ |
| transubstantiationz | 0.5406 | 0.4994 | -0.0412 | 2.5406 | GEÇTİ |

| Pozitif CPU (2000 çağrı/yol) | Yeni serial toplam ms | Yeni melez toplam ms | Fark % | Fark ms/çağrı | +%5 kapısı |
|---|---:|---:|---:|---:|---|
| olduğu | 3453.125 | 3515.625 | +1.81 | +0.03125 | GEÇTİ |
| büyük | 3093.750 | 3281.250 | +6.06 | +0.09375 | KALDI |
| ayrıca | 3562.500 | 3703.125 | +3.95 | +0.07031 | GEÇTİ |

Büyük sorgusunda +%6,06 / +0,09375ms CPU sınırı KALDI. Bu fark otomatik
ölçüm gürültüsü sayılmadı veya yeni toleransla kapatılmadı. Melez opt-in kalır.

## Aynı50k üzerinde maliyet tanısı

Bir warmup+bir diagnostic örnek/yol; normal p95 tablosuna karışmaz.
Thread span toplamları örtüşür, CPU/wall yüzdesi olarak toplanmaz.
Aşağıda serial capped; iki okuyucu aynı adayları doğrular.

| Sorgu | Eski 4B probe | Yeni 4B probe | Yeni sayfa okuması | Eski read_at isteği | Yeni read_at isteği | Aday eski/yeni |
|---|---:|---:|---:|---:|---:|---|
| çalışmalarıyla | 171964 | 0 | 159 | 173022 | 1217 | 888/888 |
| ormanların | 234247 | 0 | 134 | 236731 | 2618 | 2225/2225 |
| yanıklarda | 556367 | 0 | 250 | 562206 | 6089 | 5013/5013 |
| külliyatı | 392478 | 0 | 145 | 396493 | 4160 | 3180/3180 |
| geldiği buldu | 51552 | 0 | 84 | 52162 | 694 | 457/457 |
| kısım dişli | 242740 | 0 | 93 | 245561 | 2914 | 2403/2403 |
| İnternetten hazinem | 87662 | 0 | 230 | 88805 | 1373 | 646/646 |
| oyuncudur atlanta | 79495 | 0 | 273 | 80407 | 1185 | 503/503 |
| kendisine çıkacağını | 453919 | 0 | 496 | 457254 | 3831 | 2519/2519 |

Tarihsel 380033 probe 66k/12 segment tanısına aitti; burada 50k/3 segment
kontrolünde yanıklarda 556367 probe vardı. Bunlar aynıN gibi karşılaştırılmadı.
Sayfa + ilerleyen konum, sayfa-only denemedeki 109753 tekrar okumasını da azaltır.
Payload okuma/CRC/metin doğrulama korunur; aday azaltan ön filtre uygulanmadı.

## Kurtarma, uyumluluk, paralel test

Kesilme/bozulma/ID-sıra ve normal kurtarma testleri, fmt/clippy ve feature
kontrolleri geçti. Nihai varsayılan-paralel all-targets paketinde **243 test**
geçti. [Test kilit çevrimi ve ikinci admission-testi yarışı](PARALLEL-TEST-DEADLOCK-20261029.md)
kontrollü üretildi; test-only path pause ve doğru dört-permit tutuşuyla çözüldü.
Seri moda kaçılarak kapatılmadı; production bütçesi/kilitleri değiştirilmedi.

Son DB'de Pending açılış process kill → strict reopen/full GET oracle ve
donmuş 24 Ekim okuyucuyla 50k GET +41 exact sorgu geçti. Kaynak hash'leri aynı;
power-loss iddiası yok. `.fdb/.ready` otorite, `.idx` türetilmiş cache olarak kalır.
256KiB giriş sınırı ile format/manifest/payload kaynak SHA'ları eskiyle aynı.

[Ham tasarım, program SHA ve sıra](benchmarks/posting-pages-50k-20261029-v3-design.json),
[bütün istatistikler/kapılar](benchmarks/posting-pages-50k-20261029-v3-analysis.json),
[CRC/limit/kill/eski okuyucu kanıtı](benchmarks/posting-pages-50k-20261029-v3-verification.json),
[fmt/clippy/test/feature komutları](benchmarks/snapshot-10k-20261026-validation-posting-pages-20261029-v3.json).

100k kullanıcı tarafından belge başına 200KB ile hazırlanacak yeni corpus'u
bekliyor; eski 444KB belgeye owned fallback uygulanmadı, entry limiti artırılmadı.
Yeni dosyada audit/oracle tekrar kurulmadan 100k ingest başlamayacak.
Yazma/bakım profili, dirty500,4R+yazıcı,100'lük commit/RAM, uzun snapshot ve
gerçek soğuk cache açık. Case34/parçalı geniş tarama ve signed-zero normal
light/owned farkı izleniyor. Ön filtre, format değişikliği, Blok3/4, C ABI veya
dış proje entegrasyonu yapılmadı. [Güncel açık işler](OPEN-WORK.md).

## Son kapı tablosu

| Madde | Durum | Kanıt |
|---|---|---|
|37 sorgu: melez N50 p95 ortak≤30ms / diğer≤150ms|GEÇTİ|20 örnek/sorgu; exact ayrıca raporlandı|
|Negatif melez: kontrol<1ms ise +2ms, aksi +%10|GEÇTİ|İki negatif; tarihsel qzxkvwnever da yeniden sınıflandı|
|Pozitif melez CPU≤serial+%5|KALDI|büyük+%6,06, +0,09375ms/çağrı; tolerans eklenmedi|
|Temiz idle private≤15MiB|GEÇTİ|Yeni2,129MiB; proses sayacı|
|Sorgu kontrollü posting bütçeleri|GEÇTİ|256KiB /8MiB /512KiB sınırları; optional alan sonradan|
|Exact/capped full payload/version oracle|GEÇTİ|Normal ölçülen bütün sayfalar, tüm37 sorgu|
|Kesilme/CRC/ID sırası/kill-reopen/eski okuyucu|GEÇTİ|Unit+integration, Pending kill,50k GET+41 sorgu|
|Paralel test takılması|ÇÖZÜLDÜ|Kontrollü wait-cycle, DB başına pause; default-paralel243 + diagnostic149 test|
|256KiB entry /kalıcı biçim /varsayılan exact-serial|KORUNDU|Kaynak SHA/old reader; melez opt-in|
|100k /yeni max200KB corpus|BEKLİYOR|Kullanıcı yeni dosyayı sağlayacak;100k başlatılmadı|
|Blok3/4 /C ABI /ön filtre|BAŞLATILMADI|Kapsam dışında|

Diagnostic clippy ve [16-thread diagnostic149 test](benchmarks/posting-pages-20261029-v3-diagnostic-tests.log)
nihai kodda geçti. Pozitif CPU kapısı KALDI olduğundan bunu tüm kapılar
geçti diye sunmuyoruz. Normal exact/serial okuma iyileştirmesi ve melezin
ölçülmüş kazanımları geri alınmadı; bir sonraki adım kullanıcı kararıdır.
