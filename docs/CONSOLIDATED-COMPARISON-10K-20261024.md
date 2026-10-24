# Aynı 10k ile toplu karşılaştırma — 24 Ekim 2026

Eski FDB, güncel FDB ve native SQLite aynı oturumda, aynı mevcut ilk10k
üzerinde yeniden ölçüldü. Eski FDB **8a92384**, güncel **2a9fc2c** normal
`bounded-engine` release sürümleridir. Eski sürüm 24 Ekim gram sıralama,
RAM posting ve merge cursor adımlarından hemen öncedir; hızlı intake fold
dahildir. Bu ilk motor sürümünden bugüne kıyas değildir. Önceki raporların
yüzdeleri toplanmadı. İki çift, istatistiksel anlamlılık veya genel hız garantisi değildir.

Akışlı yazma+bakım 17.386→14.944s
(**-14.04%**), owned 14.791→12.289s
(**-16.92%**); ikisinde2/2 çift düşük. Buna karşılık ayrı son bakım
**+4.27%/+4.85% ve iki API’de2/2 çift yüksek**: bakım işi açık kalır.

## Yazma, son bakım, CPU ve OS bellek

Her API için iki dengeli üçlü: eski/güncel/SQLite, ardından SQLite/güncel/eski.
Güncel motor üçlülerin ortasındadır; bütün altı sıra permütasyonu ölçülmedi.
500 belge/transaction, 20 synced WAL commit; input CRC/JSON ve40 sınır get
yazma içindedir. FDB son bakım flush/merge drain, SQLite FTS optimize+WAL
checkpoint yapar; aynı algoritma değildir. Worker yazma ile örtüşebilir.
Toplam medyanı süreç başına yazma+bakımdan hesaplanır.

**Owned tablosunda SQLite daima tek belgeyle akışlı beslenir.** FDB caller
Vec maliyetini içerir; bu simetrik input RAM karşılaştırması değildir.

| API / medyan | Eski FDB | Güncel FDB | SQLite streaming | Güncel/eski fark | Düşük çift |
|---|---:|---:|---:|---:|---:|
| streamed yazma | 16.825 s | 14.360 s | 31.221 s | -14.65% | 2/2 |
| streamed son bakım | 0.560 s | 0.584 s | 1.066 s | +4.27% | 0/2 |
| streamed yazma+bakım | 17.386 s | 14.944 s | 32.287 s | -14.04% | 2/2 |
| streamed proses CPU | 15.836 s | 13.195 s | 31.008 s | -16.67% | 2/2 |
| streamed OS peak private | 10.133 MiB | 10.154 MiB | 8.418 MiB | +0.21% | 1/2 |
| streamed OS peak working set | 13.869 MiB | 14.072 MiB | 13.137 MiB | +1.46% | 0/2 |
| streamed retained core disk | 349.806 MB | 349.806 MB | 583.860 MB | +0.00% | 0/2 |
| owned yazma | 12.061 s | 9.426 s | 31.022 s | -21.85% | 2/2 |
| owned son bakım | 2.730 s | 2.863 s | 1.028 s | +4.85% | 0/2 |
| owned yazma+bakım | 14.791 s | 12.289 s | 32.050 s | -16.92% | 2/2 |
| owned proses CPU | 13.617 s | 11.141 s | 31.000 s | -18.19% | 2/2 |
| owned OS peak private | 80.029 MiB | 79.955 MiB | 8.385 MiB | -0.09% | 2/2 |
| owned OS peak working set | 83.477 MiB | 83.408 MiB | 12.736 MiB | -0.08% | 1/2 |
| owned retained core disk | 349.806 MB | 349.806 MB | 583.860 MB | +0.00% | 0/2 |

OS lifetime private commit fiziksel veya toplam sistem RAM’i değildir;
working set paylaşılan image sayfalarını içerir. Bunlar yazma süreçleridir;
query/get veya oracle RAM’i değildir. Faz örneklemesi kısa tepeleri kaçırabilir.

Bu oturumda akışlı toplam güncel FDB/SQLite oranı 0.4629
(-53.71%); owned/SQLite-streaming
oranı 0.3834
(-61.66%). Yalnız bu ortak20×500 belge iş yükü içindir.

## Okuma ve get

Aynı son sabit DB yeniden kullanıldı; sıfırlama/kopyalama yok. İki süreç çifti,
iki blok,25 tur: **38 durumun her birinde100 örnek/motor**. Sıralama ortak
seed ile aynıdır;8 sayfa da her tur çalışır.256 get anahtarı ve3 eksik anahtar
her tur araya girer. CPU maskesi4; strict açılış, bağımsız corpus oracle ve
her-case/get ısınması süre dışında. İki FDB oracle worker resident kalır,
blok başına DB bırakılır; SQLite blok başına yeni süreçtir. Sorgular ardışıktır.
Süre count+tam sahipli payload sonucu içerir; CLI JSON/IPC değildir.

Güncel/eski pooled p95 **29/38 düşük**,
güncel/SQLite **38/38 düşük**.
Güncel/eski için 8 yüksek, 1 eşit p95 vardır. Her durum aşağıda gösterilir.
Pooled p95 ve blok medyanı ayrı dağılımlardır; tek özet bütün sorguları temsil etmez.

| Sorgu / offset / limit | Eski p50/p95 ms | Güncel p50/p95 ms | SQLite p50/p95 ms | Güncel/eski p95 | Düşük çift | Güncel/SQLite p95 | Düşük çift |
|---|---:|---:|---:|---:|---:|---:|---:|
| quixote /0/50 | 2.0060/3.3463 | 2.0363/3.2272 | 12.9029/19.5254 | -3.56% | 1/2 | -83.47% | 2/2 |
| zeuglodon /0/50 | 2.3036/3.5179 | 2.2639/3.2434 | 7.5265/11.2009 | -7.80% | 2/2 | -71.04% | 2/2 |
| beagle /0/50 | 19.1294/28.4060 | 19.0452/28.0555 | 134.5762/173.6782 | -1.23% | 2/2 | -83.85% | 2/2 |
| mesopotamia /0/50 | 2.5682/4.0134 | 2.5978/3.9970 | 9.8175/14.9927 | -0.41% | 2/2 | -73.34% | 2/2 |
| transubstantiation /0/50 | 49.0418/68.4181 | 49.3518/68.3474 | 507.6046/644.0430 | -0.10% | 1/2 | -89.39% | 2/2 |
| harpoon /0/50 | 56.1937/81.8402 | 57.4769/79.8563 | 474.9058/600.0064 | -2.42% | 1/2 | -86.69% | 2/2 |
| ishmael /0/50 | 6.2358/9.1518 | 6.3595/8.9369 | 51.0451/66.3838 | -2.35% | 0/2 | -86.54% | 2/2 |
| invertebrata /0/50 | 10.4157/16.7825 | 10.8070/16.4694 | 91.6323/124.2911 | -1.87% | 1/2 | -86.75% | 2/2 |
| darcy /0/50 | 47.2265/62.4834 | 48.4160/62.4223 | 394.4614/522.4310 | -0.10% | 1/2 | -88.05% | 2/2 |
| elizabeth /0/50 | 45.3406/61.7339 | 45.9701/62.0655 | 349.7888/494.4418 | +0.54% | 1/2 | -87.45% | 2/2 |
| watson /0/50 | 43.2287/61.9257 | 43.1439/60.0494 | 316.3736/442.5059 | -3.03% | 1/2 | -86.43% | 2/2 |
| holmes /0/50 | 25.3060/36.0584 | 25.3782/34.9616 | 171.0043/213.7346 | -3.04% | 1/2 | -83.64% | 2/2 |
| frankenstein /0/50 | 14.2089/20.7540 | 14.4758/21.1505 | 109.9925/148.4855 | +1.91% | 1/2 | -85.76% | 2/2 |
| galapagos /0/50 | 9.3039/14.1199 | 9.1086/13.7137 | 60.8227/87.1009 | -2.88% | 2/2 | -84.26% | 2/2 |
| fossil /0/50 | 26.8147/37.3166 | 27.5764/37.9912 | 243.1289/321.1212 | +1.81% | 1/2 | -88.17% | 2/2 |
| geologist /0/50 | 16.0373/23.6801 | 16.1874/21.6056 | 123.4609/162.7371 | -8.76% | 2/2 | -86.72% | 2/2 |
| the /0/50 | 0.9386/1.6094 | 0.9910/1.5141 | 1078.9815/1343.9993 | -5.92% | 2/2 | -99.89% | 2/2 |
| man /0/50 | 1.1570/1.7634 | 1.2075/1.8603 | 1085.0466/1325.9942 | +5.50% | 0/2 | -99.86% | 2/2 |
| he /0/50 | 0.9002/1.3711 | 0.9350/1.3625 | 1052.0671/1340.4067 | -0.63% | 1/2 | -99.90% | 2/2 |
| zzxqnever /0/50 | 0.0347/0.0515 | 0.0341/0.0533 | 0.3443/1.5596 | +3.50% | 0/2 | -96.58% | 2/2 |
| zx /0/50 | 136.6429/185.6176 | 140.9985/185.3592 | 1471.4686/1869.3331 | -0.14% | 1/2 | -90.08% | 2/2 |
| the zx /0/50 | 136.0076/200.2768 | 138.6920/189.2755 | 1542.6847/2051.6107 | -5.49% | 1/2 | -90.77% | 2/2 |
| quixote whale /0/50 | 2.3098/3.7553 | 2.3018/3.6138 | 12.6766/20.2872 | -3.77% | 0/2 | -82.19% | 2/2 |
| holmes elementary /0/50 | 34.7219/48.0309 | 34.5228/45.7588 | 312.1617/417.1567 | -4.73% | 1/2 | -89.03% | 2/2 |
| fossil geologist /0/50 | 10.9410/16.3886 | 10.9220/16.3231 | 92.8057/130.0155 | -0.40% | 2/2 | -87.45% | 2/2 |
| whale harpoon /0/50 | 57.0070/80.6453 | 58.0928/76.1514 | 406.9821/512.7509 | -5.57% | 1/2 | -85.15% | 2/2 |
| QUİXOTE /0/50 | 2.1347/3.6844 | 2.1516/3.1205 | 12.9624/20.7701 | -15.31% | 2/2 | -84.98% | 2/2 |
| İSHMAEL /0/50 | 6.1146/9.6626 | 6.3669/8.9360 | 51.2754/69.3621 | -7.52% | 1/2 | -87.12% | 2/2 |
| uzun zx /0/50 | 0.0272/0.0462 | 0.0290/0.0462 | 1.4513/2.4484 | +0.00% | 1/2 | -98.11% | 2/2 |
| presumption /0/50 | 73.9316/111.4338 | 72.9082/98.6551 | 550.9247/703.1575 | -11.47% | 2/2 | -85.97% | 2/2 |
| the /0/1 | 0.1648/0.2944 | 0.1649/0.3166 | 1069.3038/1432.7577 | +7.54% | 1/2 | -99.98% | 2/2 |
| the /0/200 | 3.9002/6.3601 | 4.0133/5.9861 | 1071.3664/1366.7390 | -5.88% | 1/2 | -99.56% | 2/2 |
| the /5000/50 | 1.2546/2.0963 | 1.2980/1.9632 | 1081.2298/1400.9622 | -6.35% | 1/2 | -99.86% | 2/2 |
| the /9950/50 | 1.1148/1.8321 | 1.1201/1.8305 | 1072.0829/1448.6194 | -0.09% | 0/2 | -99.87% | 2/2 |
| ishmael /130/1 | 5.6562/8.2461 | 5.7759/8.6959 | 47.8343/64.0107 | +5.45% | 1/2 | -86.41% | 2/2 |
| ishmael /60/200 | 11.2130/17.7693 | 11.4123/16.2845 | 61.7605/88.0162 | -8.36% | 2/2 | -81.50% | 2/2 |
| quixote /28/10 | 1.7575/2.7933 | 1.7326/3.1388 | 10.5649/15.1549 | +12.37% | 0/2 | -79.29% | 2/2 |
| quixote /57/50 | 1.5484/2.7625 | 1.5823/2.6784 | 9.7347/14.4307 | -3.04% | 1/2 | -81.44% | 2/2 |

### Güncel FDB’nin kendi kontrolüne göre açık kalan artışlar

| Sorgu / offset / limit | Pooled p95 farkı | Çift 1 / çift 2 p95 farkı | Düşük çift | Tipik blok farkı |
|---|---:|---:|---:|---:|
| quixote /28/10 | +12.37% | +8.60% / +5.61% | 0/2 | +1.57% |
| the /0/1 | +7.54% | +4.83% / -15.02% | 1/2 | +0.72% |
| man /0/50 | +5.50% | +13.43% / +2.26% | 0/2 | -1.19% |
| ishmael /130/1 | +5.45% | +10.23% / -4.75% | 1/2 | +1.12% |
| zzxqnever /0/50 | +3.50% | +3.54% / +2.63% | 0/2 | -3.52% |
| frankenstein /0/50 | +1.91% | +38.77% / -5.35% | 1/2 | +1.25% |
| fossil /0/50 | +1.81% | +11.98% / -1.72% | 1/2 | +0.53% |
| elizabeth /0/50 | +0.54% | +1.93% / -2.78% | 1/2 | +1.47% |
| uzun zx /0/50 | +0.00% | -3.47% / +6.15% | 1/2 | +5.68% |

Uzun zx pooled p95 tam eşittir; tipik blok artışı açıkça korunur.
Buradaki farklar gözlemdir; bu iki çift kod kaynaklı regresyonu veya anlamlılık eşiğini kanıtlamaz.
Ek özel ölçüm yapılmadı. quixote /28/10, man ve zzxqnever iki çiftte de yüksek p95 verdi.
Eski dar raporlardaki farklı kontrol/oturum p95 ve CLI sorunları bu kıyasla kapanmaz.

| Get gövde boyutu | Örnek/motor | Eski p50/p95 ms | Güncel p50/p95 ms | SQLite p50/p95 ms |
|---|---:|---:|---:|---:|
| <4KiB | 1600 | 0.0139/0.0284 | 0.0146/0.0295 | 0.0276/0.0617 |
| 4–16KiB | 6600 | 0.0168/0.0336 | 0.0177/0.0346 | 0.0411/0.0843 |
| 16–32KiB | 8300 | 0.0219/0.0421 | 0.0227/0.0423 | 0.0641/0.1234 |
| >=32KiB | 9100 | 0.0265/0.0470 | 0.0278/0.0497 | 0.0885/0.1610 |
| eksik key | 300 | 0.0003/0.0010 | 0.0004/0.0010 | 0.0060/0.0123 |

Get p95 güncel/eski dört dolu boyut grubunda yüksek, eksik key’de eşittir;
güncel/SQLite beş grupta da düşüktür. Get artışları da kapanmış sayılmaz.

## Sözleşmeler, doğruluk ve sınırlar

Mevcut `data/rkyv-corpus-20261009/json.payload-bench` ilk10.000 belge,
264.842.858 özgün metin byte’ı. Yeni corpus/generator/download yok;
yalnız sahipliği doğrulanan `data/benchmark-work/{fdb,sqlite}` yazma için
`benchmark-workspace.ps1` ile sıfırlandı. Her12 yazma sürecinde tam10k
payload doğrulaması; FDB’de ayrıca4 contains sayfa oracle’ı geçti.
Aynı API eski/güncel source/index/ready byte’ları eşit. Okuma bütün38 case
total/tam payload/sıra/version ve get oracle’larını geçti. Okuma öncesi/sonrası
DB dosya SHA’ları aynı; corpus ve0.1 kaynak fixture SHA’ları korundu.

FDB strict open kaynak/index CRC/schema/key/version doğrular; SQLite açılışı
tam integrity taraması yapmaz. Native SQLite3.53.2 bundled FTS5 trigram+LIKE
alt-dize/AND; count ve tam payload tek snapshot transaction içindedir.
FULL WAL,2MiB page cache,mmap0,tempFILE,64 prepared-statement cache;
FDB’nin genel field/link/guard kapsamının tamamını sunmaz. SQLite yalnız
bağımsız benchmark paketidir; motor bağımlılığı/entegrasyonu değildir.

OS cache sıcak, eviction/güç/donanım izolasyonu yok. Query/get OS RAM,
açılış karşılaştırması, gerçek soğuk cache, eşzamanlı yük, başka corpus ve
dinamik geçici disk tepesi ölçülmedi. Genel RAM,owned caller maliyeti,
ayrı bakım,arama/CLI regresyonları,açılış,disk sıkıştırma/geçici alan ve
eşzamanlı/soğuk yük işleri [açık listede](OPEN-WORK.md) korunur.

## Tekrar üretim ve kanıtlar

[Tam commit/binary/source bağlantısı](benchmarks/consolidated-10k-20261024-identity.json),
[yazma tasarım/medyan/çift farkları](benchmarks/consolidated-write-10k-20261024-summary.json),
[12 yazma sürecinin kayıtları](benchmarks/consolidated-write-10k-20261024-records.json),
[okuma tasarım/100 örnek özetleri](benchmarks/consolidated-read-10k-20261024-summary.json),
[tüm 38 durumun CSV’si](benchmarks/consolidated-read-10k-20261024-table.csv),
[bağımsız rapor aritmetiği/oracle/binary kontrolü](benchmarks/consolidated-report-proof-20261024.json).
Aynı prefix altındaki `.jsonl`, `.memory.json`, `-files.json`,
`-verify.jsonl`, korunan önce/sonra SHA dosyaları ham kanıttır.

İlk rapor özeti SQLite logunu Windows cp1252 ile okuduğu için iki Unicode
sorgu adında kimlik kontrolünde durdu. Dört blokta toplam 8 text alanı
etkilendi. UTF-8 ham JSONL’de sorgu adları doğrudur; gerçek sorgular,
oracle ve süreler değişmedi. Özet yalnız kayıtlardan yeniden üretildi;
benchmark tekrar çalıştırılmadı. Her değişen adın ters cp1252→UTF-8
dönüşümü doğrulandı; diğer bütün alanlar ve timing dizileri derin eşitlik
kontrolünden geçti. Özgün records korunur; düzeltilen türetilmiş kayıt ayrıdır.
[8 alanlık kurtarma denetimi ve SHA’lar](benchmarks/consolidated-read-10k-20261024-recovery-audit.json),
[özgün kayıt](benchmarks/consolidated-read-10k-20261024-records.json),
[UTF-8 ham kaynağa eşlenen türetilmiş kayıt](benchmarks/consolidated-read-10k-20261024-recovered-records.json).

Raporu salt okunur kayıtlardan üretmek için `tools/measure-consolidated-reads.py
--summarize-only`, ardından `tools/summarize-consolidated.py` çalıştırılır.
Bu yol DB/corpus yazmaz; mevcut kaynak, corpus ve 0.1 fixture SHA’larını da
önce/sonra manifestlerine karşı yeniden kontrol eder.

Normal güncel write ve interleaved-read yeniden derlemeleri frozen SHA ile
birebir eşleşti. Ortak read_matrix/oracle/worker kaynakları8a92384→2a9fc2c
arasında değişmedi. Yalnız SQLite comparator’ına optional tur/sayfa seçimi
eklendi; eski20/5 varsayılanı korunur. Motor/format/feature/WAL değişmedi.
Mevcut dört SQLite bloğu optional 25/all-rounds yolunu çalıştırdı ve tam
payload oracle’ını geçti. Root ve native comparator için fmt ve tüm-target
Clippy warnings-as-errors kontrolleri geçti; depolama kodu değişmedi.
[Harness doğrulama çıktısı](benchmarks/consolidated-checks-20261024.log).
