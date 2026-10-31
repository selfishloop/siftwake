# 100'lük tek throughput çifti — 27 Ekim 2026

**Yeni throughput kapısı GEÇTİ:** FDB%76,59 ≥%75 ve SQLite%61,23 üstünde.
Bu sonuç arama/RAM veya100'lü transaction kuyruk uçlarını kapatmaz.

| Motor | Kontrol işlem/sn |4 okuyucu ile işlem/sn | Yük/kontrol oranı |
|---|---:|---:|---:|
|FDB|724,137|554,609|76,589%|
|SQLite|242,723|148,628|61,233%|

Her motorda yalnız bir kontrol/yük çifti, her kolda128 adet100-entry commit
(12800 update). Tek çiftin pooled oranı kendi oranıdır;4 çift/uzun dönem
güven aralığı sonucu varmış gibi sunulmaz. Ham gate sonucu
`hundred-pair-10k-20261027-design.json` içindedir.

## Düzen

Sıra FDB kontrol→SQLite yük→SQLite kontrol→FDB yük; her kol aynı mevcut ilk10k
ile bitmiş bakımdan başlar, sabit sahiplik kontrollü slotları kullanır. İlk1100
donor'un gövdesi sabit/revision token değişir;38-case sonuç kümeleri sabittir.
Bu,30tur adımındaki farklı donor'la gövde değiştirme yükünden ayrıdır.

Akışlı API esas;4 okuyucu writer bitene kadar38-case döngüsünü sürdürür.
FDB en az2tur/okuyucu, SQLite1tur; writer-active örnek n en az55 ve14/sorgu.
İki backend'de commit sayısı eşittir. Normal merge/revision interlude aynı
işlemde, yazıcı timer'ında yer alır. FDB explicit interlude2 normal merge ve
1 rewrite tamamladı; okuyucular sürüyordu. Kontrol/yük oranı toplam eşit işlem
sürelerinden hesaplanır. Model oracle query timer dışında olsa da CPU/IO kullanır.
Writer-active sorgu sayıları FDB2128/SQLite544'tür; bu4-thread döngü testidir,
eşit QPS deneyi değildir. Hızlı okuyucu daha fazla oracle işi de üretir.

Sıcak OS cache; tahliye/CPU/background/power izolasyonu yok. SQLite native
FTS5trigram/detailnone,FULL WAL,2MiB cache,mmap0,tempFILE. Yeni corpus,kalıcı
format veya handle optimizasyonu yok. Nihai binary trace varsayılan kapalıdır;
bu ölçümde profile capture açık değildir. Owned/Vec performansı ölçülmedi.

## Kötü sonuçlar ve sınırlar

| Motor/kol |100'lü commit p50 /p95 /max ms | OS peak private /WS MiB |
|---|---:|---:|
|FDB kontrol|40,013 /750,672 /944,471|25,38 /29,69|
|FDB yük|45,349 /1003,783 /1109,992|77,94 /80,71|
|SQLite kontrol|383,392 /499,835 /1322,952|11,05 /15,53|
|SQLite yük|621,508 /805,331 /2035,394|69,00 /71,11|

Bunlar100 mutation'lı atomik commit süreleridir; tek belge latency'si değildir.
FDB p95≈1,004s/max1,110s olduğu için toplu commit uçları açık kalır. Bu süre
alt aşamalara ayrılmadı; worker wait/flush olduğu kesin diye sunulmaz.

OS private/WS sayacı tüm proses ve harness/model/oracle'ı içerir; motor tek
başına77,94MiB kullandı denmez. Bu yükte60MB/MiB proses hedefi geçmiş değildir.
İndeksleme ve sorgu bellek bileşenleri ayrıca profillenmeden blame atfedilmez.

Önceki4R arama sınırı FDB'de20/38 sorguda aşılır:
0,1,3,6,7,12,13,16,17,18,22,24,26,27,30,32,33,34,36,37.
Örnek `quixote whale` p95 5,260→32,364ms; `the /5000/50`
2,645→11,455ms. Native SQLite'ta case19 ve28 sınırı aştı.
Tüm38 sorgunun örnek sayısı, p95/max ve mutlak farkları
[ham tablolarda](benchmarks/hundred-pair-10k-20261027-tables.md); kötü örnekler
silinmedi. Tek çift ve8 idle örneği/sorgu kalıcı regresyon büyüklüğü garantisi
değildir; bu oturumdaki aşım gözlemi nettir.

FDB2128 writer-active okumada517 okuma writer-owned/admission/cache/release
beklemesi bildirdi; tekil en uzun0,6515ms, bir sorgudaki toplam en çok0,6566ms.
Bu wait tüm read latency'si değildir ve tek başına büyük p95 artışını açıklamaz.
Diğer CPU/IO/normalleştirme payları bu adımda ayrılmadı. Arama/RAM için yeni
optimizasyon veya eski kapıdan feragat yapılmadı.

## Doğrulama

Bütün kollarda payload/version modeli ve38-case oracle'ı geçti; her eşzamanlı
okuma bir commit epoch'una aitti. Bu adımda ayrıca7/7 process-kill/reopen,
eski frozen okuyucu, pin/son-reader GC ve0byte input/build/referanssız kaynak
testi tekrar geçti. Son kod233 test/fmt/clippy, no-default/all-features/native
comparator kontrollerini geçti.0.1 fixture/corpus SHA korundu. Güç kesintisi
iddiası yok. Kurtarma adımı aktif oracle ledger'ını ilerletir; DB kopyası tutulmaz.

Kanıt: `hundred-pair-10k-20261027-*`,
`hundred-pair-kill-10k-20261027-summary.json`; araçlar
`measure-hundred-pair.py`, `report-hundred-pair.py`, `kill-hundred-pair.py`.
Yeni iş/Blok2/3/4/C ABI başlatılmadı. Varsayılan WAL/reclaim kapalıdır.
