# FDB ve SQLite soğuk açılış karşılaştırması, 50k

Cap200k `881756DA…` korpusunun ilk 50 bin belgesi, iki motorun mevcut
sabit slotları. İki model SHA-256 ve sequence 3101 eşit, 41 sorgu
oracle'ı eşleşti. Zamanlanan sorgu TR case0 `olduğu`, offset 0 ve ilk
50 sonuç. FDB `ddb263b` kaynaklarından varsayılan bounded-engine ile
derlendi. 39 run'lı fiziksel yerleşim son disk deneyindeki aday
yazılarından kaldı. Açılan motor güncel üretim kodu. SQLite aynı FTS5
trigram sorgusu, bir salt okunur transaction, 2 MiB sayfa cache'i,
mmap kapalı ve temp FILE. Yeni DB veya corpus hazırlanmadı.
[Koşu sürücüsü](benchmarks/cold-open-paired-20261115/measure.py) ·
[İkili hash'leri](benchmarks/cold-open-paired-20261115/plan.json)

FDB normal strict-open'da birden fazla immutable run varsa ve süreç
içindeki iki ortak izin hemen alınabiliyorsa çağıran thread ve bir
yardımcı run açma ve doğrulamasını paylaşır. İzinler yoksa seri çalışır.
WAL replay ve açılış maskesi aynı seri aşamalardır. Bu koşuda FDB'ye
işçi override'ı verilmedi. Gerçek işçi sayısı zamanlanan probeda
ayrı sayaçla kaydedilmedi.
[Güncel açılış kodu](../src/engine.rs#L150)

Sıra SQLite → FDB → FDB → SQLite. Her deneme için RAMMap `-Es`,
`-Et` ve ilk sonuca kadar bir WPR izi. İlk sonuçtan sonra WPR
durduruldu, aynı oturumda kalan 29 sorgu ölçüldü. Ardından DB
dosyaları sırayla okunarak cache ısıtıldı ve WPR kapalıyken ayrı
proseste sıcak 30 sorgu çalıştı. Q30 tek sorgunun süresi.
Süreye göre hiçbir kol kesilmedi. SQLite'ın önceki 56 MiB collector
sınırına takılmasının ardından 96 MiB collector kullanıldı.
CSV diske yazılmadan süzüldü. Collector bazında kayıp ölçülemedi.
[Yönetici betiği](benchmarks/cold-open-paired-20261115/run-admin.ps1) ·
[İz doğrulama betiği](benchmarks/cold-open-paired-20261115/decode.ps1)

## Süreler

| Deneme | Motor | Koşul | Yalnız açılış | İlk sorgu | Açılıştan ilk sonuca | 30. sorgu | Q2–Q30 p95 |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | SQLite | Soğuk | 2,98 ms | 6.660,44 ms | 6.663,42 ms | 308,36 ms | 353,66 ms |
| 1 | SQLite | Sıcak | 1,73 ms | 295,03 ms | 296,76 ms | 298,71 ms | 308,83 ms |
| 2 | FDB | Soğuk | 727,38 ms | 181,23 ms | 908,62 ms | 164,37 ms | 185,31 ms |
| 2 | FDB | Sıcak | 472,48 ms | 167,76 ms | 640,24 ms | 172,05 ms | 177,55 ms |
| 3 | FDB | Soğuk | 714,20 ms | 171,04 ms | 885,25 ms | 165,71 ms | 169,43 ms |
| 3 | FDB | Sıcak | 458,47 ms | 168,14 ms | 626,61 ms | 167,85 ms | 186,64 ms |
| 4 | SQLite | Soğuk | 17,55 ms | 6.678,72 ms | 6.696,27 ms | 435,54 ms | 406,31 ms |
| 4 | SQLite | Sıcak | 1,42 ms | 338,05 ms | 339,47 ms | 300,84 ms | 343,14 ms |

Soğuk açılıştan ilk sonuç iki eşleşmede SQLite/FDB **7,33×** ve
**7,56×**. SQLite bağlantıyı birkaç milisaniyede açıyor ve soğuk
okuma maliyetinin büyük kısmı ilk sorguya geliyor. FDB açılışta
strict kaynak ve indeks kontrolü yapıyor. Sıcak açılış dahil ilk
sonuçta bu yerleşimde SQLite daha hızlı. Soğuk Q30 FDB'de iki
örnekte de sıcak Q30'a yakın. SQLite'ın ilk örneği de sıcak Q30'a
yakın, ikinci örnek 435,54 / 300,84 ms. Q30 için sayısal kabul
verilmedi. Açılış başına iki örnek, açılış p95'i çıkarılmadı.
WPR'nin mutlak süreye etkisi bu tur ayrıştırılmadı.
[Ham sorgular ve eşleşmiş özet](benchmarks/cold-open-paired-20261115/results.json)

## Soğuk kanıtı ve kapanış

| Deneme | Gerekli dosya/PID eşleşmesi | Hedefe atfedilen fiziksel okuma | Birleşik kayıp | Bağımsız FileIO eşleşmesi |
| --- | ---: | ---: | ---: | ---: |
| SQLite 1 | 1/1 | 207.220.736 B | 0 | 317.692/317.692 |
| FDB 2 | 41/41 | 72.555.520 B | 0 | 69.596/69.596 |
| FDB 3 | 41/41 | 71.506.432 B | 0 | 68.731/68.731 |
| SQLite 4 | 1/1 | 207.298.560 B | 0 | 297.486/297.486 |

Dört `-Es/-Et` ve WPR stop exit 0. Dört birleşik izde `tracerpt`
toplam kayıp 0. CSV tüm olay sayısı farkı her izde korunan zaman
penceresinin bağımsız `Get-WinEvent` FileIO sayımı ve SHA-256
eşleşmesiyle uzlaştırıldı. SQLite'ın bağımsız kontrolü ilk 50 saniyelik
çözümleme penceresini aştı. Eldeki iz üzerinde çevrimdışı kontrol
tamamlandı. Yeni deneme ya da purge yapılmadı. Nihai geçerlilik için
`results.json` esas, ilk denetleyici durumu ham süreç kaydı olarak
saklandı. Toplam ETL 26.468.404 B. WPR boşta.
[Nihai kanıt](benchmarks/cold-open-paired-20261115/results.json) ·
[İlk denetleyici kaydı](benchmarks/cold-open-paired-20261115/status.json)

İki motorun ön ve son 50k GET/version, 41 sorgu ve süreç sonlandırma
sonrası yeniden açılış doğrulamaları geçti. Dondurulmuş eski FDB
okuyucusunda 41 digest eşit. SQLite ana DB ve FDB kaynak/manifest
dosyalarının hash'leri eski okuyucudan sonra da aynıydı. SQLite
read-only bağlantısının doğal yan dosyaları izinli. 08:08:13–08:16:51
UTC. Dört soğuk ve dört sıcak deneme tamamlandı. Üretim kodu ve
veritabanı biçimi aynı.
[FDB oracle](benchmarks/cold-open-paired-20261115/fdb-after.jsonl) ·
[SQLite oracle](benchmarks/cold-open-paired-20261115/sqlite-after.jsonl) ·
[Eski okuyucu](benchmarks/cold-open-paired-20261115/frozen-fdb.jsonl)
