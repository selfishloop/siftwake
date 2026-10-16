# 10k genişletilmiş okuma ve contains matrisi

## Sonuç ve karar

Beşer süreç tamamlandı. Bu **2 MiB cache / mmap kapalı SQLite comparator'ında**
30 ana query girişinin tamamında ve 8 sayfa senaryosunun tamamında pooled p95
FDB lehine. Bu başka SQLite schema/plan/cache ayarlarının en iyi sonucu veya
genel DB üstünlüğü değildir. Tüm sorguların tablosu aşağıdadır; yavaş sorgular
ve sıfır sonuçlar hariç tutulmadı.

Asıl bulgu nadir/dar sorguların aynı hızda olmaması:

| Sorgu | Sonuç | FDB p95 ms | SQLite p95 ms |
|---|---:|---:|---:|
| zeuglodon | 32 | 4,76 | 8,95 |
| beagle | 46 | **61,27** | 153,27 |
| invertebrata | 43 | 27,07 | 104,44 |
| holmes elementary | 65 | **110,73** | 329,84 |
| transubstantiation | 0 | **155,27** | 542,05 |
| presumption | 883 | **247,36** | 639,19 |
| the | 10.000 | 404,09 | 1.199,17 |
| zx | 0 | **711,00** | 1.554,52 |
| the zx | 0 | **709,79** | 1.610,39 |

`beagle` run p95'leri ilk üç koşuda63,69 / 50,36 / 44,40 ms;
30 ms üstündeki maliyet tek en yüksek örnekten ibaret değildir. Az veya
sıfır gerçek eşleşme hızlı sonuç garantisi vermiyor. Sonraki profil önceliği
`beagle`, `holmes elementary`, `transubstantiation` ve negatif kısa sorgularda
aday sayısı, posting kesişimi/IO ve metin/CRC doğrulama maliyetini ayrı ölçmek.
Bu turda nedenleri alt sürelerle ayırmadık; yalnız normalleştirmeye bağlamıyoruz.

### Tam gövde kontrolünün sınırlı regresyon değerlendirmesi

| Stres sorgusu | Tam gövde kontrolü p95 ms (40 örnek) | Yeni p95 ms (100 örnek) |
|---|---:|---:|
| zx | 787,82 | 711,00 |
| the zx | 776,68 | 709,79 |
| presumption | 278,97 | 247,36 |
| 300 bayt negatif terim | 0,1019 | 0,1082 |

Pooled sayılar tek başına "hiç regresyon yok" kanıtı sayılmaz. İlk zamansal
eşleşmede zx792,31→737,10 / the-zx800,63→766,86 ms; son eşleşmede
zx559,05→570,99 (%2,1 artış) / the-zx563,47→577,62 ms (%2,5 artış).
Kontrollerde yalnız dört stres query'si, yeni motorda geniş matris çalışır;
iş yükü sırası ve tekrar sayısı birebir aynı değildir. Bu küçük negatif fark
ile büyük koşu değişkenliğini ayırmak için daha sıkı eşleşmiş tekrar gerekir.
Garantili negatif tarama kazancı iddiası yok. Uzun negatif terimdeki artış
0,0063 ms (~%6,2); bu sorgu boş trigram adayında elenir, chunk dalına girmez.
Nedensel regresyon veya yokluğu bu örneklerle kesinleştirilmedi.

### Get: metin boyutuna göre

Her hücre p50 / p95 / p99, milisaniye. 25.600 başarılı, 300 olmayan kayıt
okuması/motor; tüm returned StoredDocument'lar kaynağa eşit:

| Metin boyutu | Örnek/motor | FDB | SQLite |
|---|---:|---:|---:|
| 2–4 KiB | 1.600 | 0,0144 / 0,0305 / 0,0480 | 0,0338 / 0,0786 / 0,1183 |
| 4–16 KiB | 6.600 | 0,0222 / 0,0425 / 0,0638 | 0,0470 / 0,0997 / 0,1365 |
| 16–32 KiB | 8.300 | 0,0374 / 0,0640 / 0,0858 | 0,0693 / 0,1323 / 0,1774 |
| 32–50 KiB | 9.100 | 0,0511 / 0,0846 / 0,1180 | 0,0930 / 0,1665 / 0,2148 |
| Olmayan kayıt | 300 | 0,0004 / 0,0015 / 0,0017 | 0,0093 / 0,0227 / 0,0334 |

FDB açılış beş örnekte918,67–973,49 ms, medyan958,91 ms;
SQLite bağlantı/pragma8,45–10,13 ms, medyan9,53 ms. SQLite burada tam kaynak
doğrulaması yapmaz, aynı açılış sözleşmesi karşılaştırılmıyor.

Ana iki motorun **6.400 zamanlı sorgu ve 51.800 get** doğrulaması geçti.
Ek kontrol iki prosesinde160 sorgu / 10.360 get de doğrulandı. Bunlar ölçümsüz
warmup'ları içermez. Tüm mevcut `.fdb/.ready/.idx` ve `sqlite.db` dosyalarının
SHA256 ve boyutları önce/sonra aynı. Motor kodu, kalıcı format, 0.1 fixture
ve dış projeler değiştirilmedi. RAM yeniden ölçülmedi.

Ölçüm aracı testleri FDB örneğinde4/4, izole SQLite bin'de4/4 geçti.
İki crate'te `cargo fmt --check` ve `cargo clippy --all-targets -- -D warnings`
geçti. Depolama kodu değişmediği için bu turda kesilme/kill testleri tekrar
çalıştırılmadı. Geniş benchmark daha sağlam okuma kanıtıdır; 50k/100k, soğuk
cache, yazarken arama veya production kabul kapısının geçtiği anlamına gelmez.

## Kapsam ve yöntem

Önceki dört sorgu ölçümü genel performans kapısı değildi. Bu tur 30 ana sorgu,
8 ek sayfalama senaryosu, 256 rastgele seçilmiş mevcut anahtar, dört metin boyutu
grubu ve 3 olmayan anahtar içerir. Yeni belge/corpus veya DB üretilmedi;
aynı ilk 10.000 kaynak kayıt, mevcut `data/benchmark-work/fdb` ve
`data/benchmark-work/sqlite/sqlite.db` kullanıldı. Metin toplamı264.842.858 bayt,
2.057–51.197 bayt/belge; sekiz kitaptan benzersiz, kısmen örtüşen pasajlar.
Corpus gerçek Türkçe dosyalar veya 10 bin bağımsız eser değildir.

Mevcut FDB 5 immutable segment, boş delta; release feature'ları
`experimental-merge-postings experimental-delta-bitmaps`, diagnostic feature yok.
SQLite read-only FTS5 trigram external-content, cache2 MiB, mmap kapalı,
temp FILE; eski optimize/checkpoint edilmiş DB. Query text Türkçe/Unicode
kurallarıyla katlanır; LIKE jokeri içermeyen sabit terimler kullanılır.
Query scope shared, collection texts, AND alt-dize, kesin toplam,
anahtar sırası, tam StoredDocument/versiyon döndürme. SQL statement hazırlama,
count ve sayfa query'si, tek read transaction ve JSON çözümü süreye dahildir.
FDB Query oluşturma, CRC/biçim kontrolleri, hit get ve sequence20 kontrolü dahildir.
Kaynak oracle taraması ve eşitlik kontrolleri süre dışında.

Ana sorgular 20 tekrar/proses × 5 proses = **100 örnek/sorgu/motor**.
Her sayfa 5 tekrar/proses × 5 = **25 örnek/senaryo/motor**. Sayfa seçimi:
yaygın sorguda limit1/200, offset5000/9950; orta sorguda orta/son sayfa ve
limit1/200; dar sorguda orta sayfa/limit10 ve sonucun dışındaki offset.
Get:256 anahtar × 20 tekrar × 5 =25.600 başarılı okuma/motor;
3 olmayan anahtar × 20 × 5 =300 negatif okuma/motor. İlk/son kayıt ve
dört boyut grubundan en az birer kayıt örnek anahtar havuzuna dahildir.
PRNG reproducible xorshift; her koşuda seed0–4, aynı seed aynı sorgu/get
sırası, FDB ve SQLite birbirinden bağımsız zamanda çalışır.

Sıra: full-scan-control0 → FDB0 → SQLite0 → SQLite1 → FDB1 → FDB2 →
SQLite2 → SQLite3 → FDB3 → FDB4 → SQLite4 → full-scan-control1.
Her query/key bir ölçümsüz ısıtma ile başlar. OS cache boşaltılmadı;
hash kontrolleri ve corpus okumaları da cache'i etkiler. **Sıcak OS cache**
ölçümüdür; SQLite'ın sayfa cache'i veritabanının tamamını içermez.

Rapor p50/p95/p99'u tüm koşulardaki örnekleri birleştirip nearest-rank ile
hesaplar. Önceki rapordaki proses percentile medyanı ile aynı toplama yöntemi
değildir. Her sorgu için ayrıca koşu p95 minimum/maksimumu kaydedilir.
100 örnekte p99 ikinci en yüksek, 25 sayfa örneğinde p99 en yüksek örnektir;
sayfa p95/p99 kuyruk tahminleri kabadır. Örnekler bağımsız rastgele deneyler
sayılmaz; güven aralığı veya kararlı production p99 iddiası yoktur.

## Doğruluk oracle'ı ve sorgu seçimi

Kaynak streaming okunur; header/record/payload CRC ve key/version kontrolleri
yapılır. Motor normalizer'ını kullanmayan bağımsız Unicode/Türkçe fold +
başlık/gövde alt-dize taraması tüm 10k kayıt üzerinde tam eşleşme ID listelerini
kurar. İkinci geçişte sadece sayfalarda/get havuzunda kullanılan benzersiz
StoredDocument'lar tutulur; tüm corpus RAM'e alınmaz. RAM/working set bu
ölçümde ölçülmedi; oracle belleği motor belleği diye sunulmaz.

Ön taramada sıfır eşleşen nadir kelime adayları gerçek eşleşen kelimelerle
değiştirildi: zeuglodon32, beagle46, mesopotamia31, invertebrata43.
Dar AND `quixote whale`44, `holmes elementary`65; orta `fossil geologist`293.
Sıfır sonuçları gizlemek yerine ayrı negatif sorgular vardır. `QUİXOTE` ve
`İSHMAEL`, mevcut İngilizce eşleşmeler üzerinden sorgu katlama testleridir;
Türkçe corpus performansı sayılmaz. 30 farklı query girişinin iki çifti aynı
katlanmış terimleri üretir: toplam28 farklı katlanmış terim kümesi vardır.

`zx` ve `the zx` hiçbir belgede yoktur, fakat iki karakter ve yaygın diğer
terim nedeniyle aday metni sona kadar tarama gerektirir. Uzun negatif sorgu
300 bayt `zx` tekrarından oluşur; trigramla erken aday eleme de ölçülür.
Tail-token `presumption` ilk mevcut belgenin sonundan seçildi; 883 eşleşmenin
196'sında ilk gerekli gövde eşleşmesi metnin son çeyreğindedir. Bu sorgunun
tamamı geç eşleşen bir corpus gibi sunulmaz.

## Eski tam gövde yolu için ek kontrol

Negatif/tail/uzun sorgu türlerinde iki ayrı kontrol prosesi çalışır. Kontrol,
mevcut motor kaynaklarının küçük, yalnız kod içeren snapshot'ıdır; tek fark
`text.len()>1024` chunk dalının kapalı olmasıdır. Böylece önceki tam gövde
dönüşümü ve bounded tekrar cache'i çalışır. Canonical kaynaklar değiştirilmez.
Yeni feature, protokol veya kalıcı format yok; DB veya corpus snapshot'a
kopyalanmaz. `tools/build-read-matrix-control.ps1` yalnız bu kontrol executable'ını
oluşturur. Kontrolde 2×20=40 örnek/sorgu, get de çalışır; ana iki motorun
beş koşusu ile aynı istatistik gücüne sahip değildir. Kontrol p95 ve koşular
ayrı kaydedilir; farklı zamanlama nedeniyle küçük farklara kesin regresyon denmez.

## Araçlar ve sınırlar

`tools/measure-content-reads.ps1 -Matrix` mevcut DB'leri kullanır, reset/ingest/
maintenance çağırmaz. Önce/sonra `.fdb/.ready/.idx` ve `sqlite.db` SHA256/boyut
kontrolleri yapılır; lock/SQLite shm dosyaları hariçtir. Ham kayıt/hash prefix'i
`benchmarks/read-matrix-10k-20261012`; her örnek JSONL'de korunur.
`tools/summarize-read-matrix.py` yalnız bu raporları okur, summary JSON ve
tam 38 satırlı tablo üretir.

İş yükü hâlâ10k, sıcak cache ve boş delta'dır. 50k/100k, soğuk cache,
yazarken arama, güncelleme/silme görünürlüğü, gerçek Türkçe corpus,
RAM/disk optimizasyonu ve production p99 bu turun dışında.
Bu görev motor davranışını değiştirmez; yalnız benchmark kapsamını genişletir.

## Tüm sorgular — pooled p50/p95/p99

FDB/SQLite p95 oranı 1'den küçükse FDB daha kısa; sayfa p99'u 25 örnekle kabadır.

| Sorgu | Tür | Toplam | Offset/limit | FDB p50/p95/p99 ms | SQLite p50/p95/p99 ms | FDB/SQLite p95 |
|---|---|---:|---|---:|---:|---:|
| `quixote` | word | 56 | 0/50 | 6.087/8.820/9.258 | 13.308/16.611/18.550 | 0.53 |
| `zeuglodon` | word | 32 | 0/50 | 3.771/4.760/5.139 | 7.576/8.948/9.605 | 0.53 |
| `beagle` | word | 46 | 0/50 | 44.403/61.269/63.691 | 130.489/153.267/168.656 | 0.40 |
| `mesopotamia` | word | 31 | 0/50 | 4.227/5.679/6.343 | 9.901/12.493/14.015 | 0.45 |
| `transubstantiation` | word | 0 | 0/50 | 116.522/155.270/169.015 | 475.926/542.048/691.171 | 0.29 |
| `harpoon` | word | 1006 | 0/50 | 134.481/177.163/197.407 | 455.332/510.781/564.236 | 0.35 |
| `ishmael` | word | 260 | 0/50 | 15.815/20.620/22.687 | 50.280/57.912/66.961 | 0.36 |
| `invertebrata` | word | 43 | 0/50 | 20.654/27.075/30.503 | 88.837/104.438/115.034 | 0.26 |
| `darcy` | word | 1220 | 0/50 | 111.085/146.927/155.943 | 386.915/460.953/508.682 | 0.32 |
| `elizabeth` | word | 2280 | 0/50 | 87.166/116.146/122.176 | 343.579/382.203/433.156 | 0.30 |
| `watson` | word | 1707 | 0/50 | 102.776/143.240/148.796 | 312.536/371.931/425.218 | 0.39 |
| `holmes` | word | 1222 | 0/50 | 51.498/69.265/72.325 | 170.438/198.635/216.880 | 0.35 |
| `frankenstein` | word | 607 | 0/50 | 31.119/40.207/45.066 | 109.533/125.265/135.281 | 0.32 |
| `galapagos` | word | 179 | 0/50 | 19.206/25.967/26.310 | 59.798/69.532/74.160 | 0.37 |
| `fossil` | word | 595 | 0/50 | 60.355/81.728/88.951 | 235.033/274.139/305.975 | 0.30 |
| `geologist` | word | 306 | 0/50 | 34.871/46.572/48.786 | 122.721/138.127/163.777 | 0.34 |
| `the` | common | 10000 | 0/50 | 295.354/404.086/413.262 | 1064.236/1199.167/1266.571 | 0.34 |
| `man` | common | 9932 | 0/50 | 305.013/395.710/427.922 | 1094.809/1175.836/1228.636 | 0.34 |
| `he` | short | 10000 | 0/50 | 295.721/391.386/407.776 | 1041.749/1172.451/1228.298 | 0.33 |
| `zzxqnever` | absent | 0 | 0/50 | 0.087/0.133/0.143 | 0.349/0.519/0.543 | 0.26 |
| `zx` | short-negative | 0 | 0/50 | 544.346/710.997/737.104 | 1425.770/1554.518/1605.227 | 0.46 |
| `the zx` | negative-and | 0 | 0/50 | 544.809/709.792/766.858 | 1485.804/1610.387/1687.627 | 0.44 |
| `quixote whale` | and | 44 | 0/50 | 5.787/8.109/9.002 | 12.462/14.387/16.881 | 0.56 |
| `holmes elementary` | and | 65 | 0/50 | 78.456/110.725/112.312 | 295.816/329.845/365.804 | 0.34 |
| `fossil geologist` | and | 293 | 0/50 | 21.408/28.707/30.810 | 90.561/110.992/114.843 | 0.26 |
| `whale harpoon` | and | 995 | 0/50 | 129.185/180.420/185.005 | 394.699/447.261/491.548 | 0.40 |
| `QUİXOTE` | fold | 56 | 0/50 | 6.218/8.646/9.272 | 13.305/16.104/17.899 | 0.54 |
| `İSHMAEL` | fold | 260 | 0/50 | 15.595/21.340/23.252 | 50.748/59.222/63.210 | 0.36 |
| `zx × 150 (300 bayt)` | long-negative | 0 | 0/50 | 0.062/0.108/0.147 | 1.269/2.063/2.209 | 0.05 |
| `presumption` | tail-token | 883 | 0/50 | 177.692/247.355/257.583 | 539.880/639.190/678.764 | 0.39 |
| `the` | page | 10000 | 0/1 | 292.925/391.495/406.964 | 1064.492/1193.663/1512.145 | 0.33 |
| `the` | page | 10000 | 0/200 | 301.927/388.501/416.848 | 1082.502/1177.252/1490.746 | 0.33 |
| `the` | page | 10000 | 5000/50 | 297.071/384.309/399.243 | 1067.913/1204.331/1443.873 | 0.32 |
| `the` | page | 10000 | 9950/50 | 298.390/385.207/385.845 | 1079.626/1172.282/1229.572 | 0.33 |
| `ishmael` | page | 260 | 130/1 | 13.521/19.704/21.562 | 47.974/59.361/61.103 | 0.33 |
| `ishmael` | page | 260 | 60/200 | 23.392/29.836/31.957 | 61.363/70.269/72.086 | 0.42 |
| `quixote` | page | 56 | 28/10 | 4.418/6.315/7.008 | 10.931/12.402/14.229 | 0.51 |
| `quixote` | page | 56 | 57/50 | 3.567/5.200/5.264 | 10.056/12.631/12.779 | 0.41 |

