# Revizyon temizliğini `maintain_idle()` yoluna alma — 50k

**Aday iki tekrarda disk kapısını geçti.** 1 s üstü commit **211/6.000
→ 15/6.000**, p99 **1,91–2,11 s → 0,79–0,82 s**. Yazı ve bakım duvarı
birlikte yaklaşık **%37 daha yüksek throughput** verdi. Aday geçici
derlemede. [Özet ve commit sequence’leri](benchmarks/reclaim-idle-only-50k-20261112/summary.json)

## Değişiklik ve düzen

Geçici derlemede adayın revizyon planlayıcısı yalnız `maintain_idle()`
içindeki explicit döngüden çalışabiliyor. Committeki otomatik seçim normal
prefix ve normal merge planlama/kurulumunu sürdürüyor. Revizyon planı,
başlatılan iş ve join sayısı, yazı turu ile bakım çağrısında ayrı sayılıyor.
Çalışan snapshot ve görünürlük kuralları aynı. Üretim kaynağı ve varsayılan
kapalı reclaim ayarı değişmedi. [Geçici kod farkı](benchmarks/reclaim-idle-only-50k-20261112/candidate.diff.txt) ·
[Önceki kuyruk tanısı](RECLAIM-PROFILE-50K-20261112.md)

Tek release ikilisi. İki runtime kolunda da küçük akışlı WAL ve reclaim
açık. Sıra **mevcut → aday → aday → mevcut**, aynı 881756DA… Cap200k ilk
50.000 belge ve aynı sahipliği doğrulanmış sabit FDB slotu. Her kol aynı
bakımlı başlangıçtan. Sayısal anahtar, bir kısa snapshot okuyucusu ve bir
akışlı yazıcı. 30 tur × 10.000 değişiklik, 100’lük commit, tur sonunda
bir `maintain_idle()` ve iki saniye yazmasız ara. Okuyucu bakım sırasında
da sorgular. Sıcak açılış karşılaştırması 0/10/20/30. turlarda 10’ar yeni
proses, `olduğu` melez N=50. SQLite’ın önceki LIVE-DISK koşusu referans.
Yeni korpus, WPR/ETW/purge ve uzun snapshot yok.
[Önceki 50k iş yükü](LIVE-DISK-50K-20261111.md) ·
[Yürütme penceresi](benchmarks/reclaim-idle-only-50k-20261112/window.json)

Kapı, her tur bakım ve iki saniye sonrası retained disk **≤1,3×**.
Geçici disk tepesi 50 ms örnekli ve ayrıca raporlanır. Commit p95/p99/max,
1 s üstü adet, yazı throughput’u ve bakım dahil throughput ayrı.
`maintain_idle()` senkron olduğundan bakım süresini toplam maliyete
ekliyoruz. Bu düzen her tur bakım fırsatı veriyor. Sürekli, hiç boşluk
vermeyen yazıcının disk tavanını ölçmüyor. Pencere 16:06:49–17:06:49 UTC,
120 tur ve 12.000 commit üst sınırı. [Sürücü](benchmarks/reclaim-idle-only-50k-20261112/run.py)

Geçici derlemede `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
ve 8 hedefli revizyon testi geçti. Yeni test, adayın explicit idle dışında
revizyon işini başlatamadığını, idle’da tombstone temizliğini ve eski
snapshot görünümünü koruduğunu doğruladı. [Test günlüğü](benchmarks/reclaim-idle-only-50k-20261112/tests.log)

## Yazma ve bakım

| Kol | Commit p50 / p95 / p99 / p99,9 / max (ms) | >1 s / 3.000 | Yazı / bakım toplamı (s) | Yazı / bakım dahil (işlem/s) |
| --- | ---: | ---: | ---: | ---: |
| 1 · Mevcut | 14,11 / 201,20 / 2.113,81 / 3.210,74 / 3.964,18 | 108 | 261,47 / 164,49 | 1.147 / 704 |
| 2 · Aday | 10,79 / 36,14 / 824,19 / 1.080,17 / 1.302,09 | 14 | 110,83 / 194,77 | 2.707 / 982 |
| 3 · Aday | 10,28 / 34,49 / 787,87 / 846,12 / 1.031,06 | 1 | 102,30 / 180,44 | 2.932 / 1.061 |
| 4 · Mevcut | 12,18 / 167,71 / 1.910,67 / 3.209,33 / 4.087,80 | 103 | 238,46 / 139,90 | 1.258 / 793 |

Yazı oranı commit gruplarının yazı duvarı üzerinden, bakım dahil oran
yazı ve bakım toplamı üzerinden. Her turdaki iki saniye boşluğu ile
ölçüm arası strict reopen ve doğrulama bu oranların dışında. p95 ve
diğer yüksek yüzdelikler en yakın üst sıra.

İki kol havuzunda yazı **500 → 213 s**, bakım **304 → 375 s**. Yazı
oranı **1.200 → 2.815 işlem/s** (+%135). Bakım dahil **746 → 1.020
işlem/s** (+%37). Bakım toplamı yaklaşık %23 arttı. Adayın kalan 15
uzun commit’i var. İki adayda da en uzun sequence **624**, sırasıyla
1.302 ve 1.031 ms. Önceki kapalı profil aynı sequence’te prefix join
sinyali verdi. Bu prototipte 624’ün aşama süreleri ölçülmedi.
[Önceki 624 profili](RECLAIM-PROFILE-50K-20261112.md)

| Sequence | Mevcut 1 / 4 (ms) | Aday 2 / 3 (ms) |
| --- | ---: | ---: |
| 693 | 3.964,18 / 1.080,15 | 68,36 / 553,26 |
| 1324 | 1.464,41 / 1.296,88 | 831,48 / 764,41 |

| Kol | Yazı içinde plan / başlangıç / join çağrısı | Bakım içinde plan / başlangıç / join çağrısı |
| --- | ---: | ---: |
| 1 · Mevcut | 430 / 153 / 148 | 72 / 7 / 12 |
| 2 · Aday | 0 / 0 / 0 | 110 / 80 / 80 |
| 3 · Aday | 0 / 0 / 0 | 111 / 81 / 81 |
| 4 · Mevcut | 446 / 145 / 142 | 75 / 2 / 5 |

Join sayısı çağrıyı sayar. Hazır işçiyi devralma da bu sayaca girer.
Adayda 6.000 commit boyunca hiçbir revizyon işi çağrı içinde join
edilmedi. Bakımda başlayan 161 revizyon işi aynı bakım çağrıları
içinde join edildi. Bakım senkron. Yazılar explicit bakım sürerken
servis kuyruğuna gelirse ne kadar bekleyecekleri bu düzenin dışında.

## Disk ve açılış

| Kol | Son retained (MB) | Son / en yüksek bakım sonrası oran | Geçici tepe | Son run / ölü payload (MB) | Son sıcak strict açılış / ilk sonuç p50 (ms) | Kapı |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| 1 · Mevcut | 380,75 | 1,266× / 1,266× | 1,499× | 25 / 32,97 | 285,25 / 290,98 | GEÇTİ |
| 2 · Aday | 337,27 | 1,121× / 1,266× | 1,591× | 25 / 8,89 | 265,63 / 271,38 | GEÇTİ |
| 3 · Aday | 337,27 | 1,121× / 1,266× | 1,591× | 25 / 8,89 | 263,61 / 269,46 | GEÇTİ |
| 4 · Mevcut | 348,97 | 1,160× / 1,248× | 1,488× | 23 / 16,00 | 270,23 / 275,74 | GEÇTİ |

Sıcak ilk sonuç saati strict açılışın başlangıcından ilk sayfanın dönüşüne.
Geçici disk tepesi 50 ms örnekleme, bakım sonrası 1,3× ayrı ölçü.
Adayın en yüksek geçici tepesi yaklaşık **478,5 MB**. İki adayın son
disk, ölü payload ve tombstone sayısı bire bir eşit. Son on turda ölü
payload 8,89–34,05 MB ve tombstone 0–3.824, 30. turda tombstone 0.
Yazı altındaki karışık 41 sorgu p95’i mevcut 98–119 ms, aday 84–90 ms.
Tepe private proses belleği mevcut 67,17–68,26 MiB, aday 65,68–69,68 MiB.
Bu son iki ölçüm 41 sorguya ayrı kapı veya motor-only RAM testi değildir.

SQLite’ın önceki aynı iş yükündeki referansı commit p95 **316–322 ms**,
en uzun commit **849–914 ms**, son disk **516–519 MB**. Ara `busy`
checkpoint’lerde bakım sonrası disk 3,671–4,633×. Bu tur eşzamanlı yeni
SQLite kolu çalıştırılmadı. [SQLite referansı](LIVE-DISK-50K-20261111.md)

## Kapılar ve doğrulama

| Kontrol | Sonuç |
| --- | --- |
| Her tur bakım sonrası disk ≤1,3× | **GEÇTİ 2/2 aday**, en yüksek 1,266×. Mevcut 2/2. |
| Aday commit yolunda revizyon planı / başlatma / join | **0 / 0 / 0** iki tekrarda. |
| Yazma gecikmesi ve throughput | Her iki dengeli karşılaştırmada iyileşti. Adayın 6.000 commitinde 15 adet >1 s kaldı. |
| Commit sonrası process-kill → reopen | **GEÇTİ 4/4**. Her kolda 50.000 tam GET/version, 75.001 silinen anahtar, 41 exact sorgu ve 24 örnekli marker. |
| Dondurulmuş eski FDB okuyucu | **GEÇTİ 4/4**. 50.000 GET/version ve 41 digest güncel okuyucuyla eşleşti. Dört kolun model SHA ve 41 sorgu imzası aynı. |

Ölçüm 16:09:22–16:43:26 UTC. 120 tur, 12.000 commit. Korpus, 0.1
fixture ve üretim kaynak SHA’ları eşit. Sabit FDB slotu son kontrol
kolunda `ready`, sequence 3101. SQLite slotuna yazılmadı. Genel reclaim
varsayılanı kapalı. [Yürütme ve oracle durumu](benchmarks/reclaim-idle-only-50k-20261112/status.json)

Geçici derlemede `cargo test --lib` **163 geçti, 13 ignore**. Geçici
klasör `C:\Users\Serkan\AppData\Local\Temp\fdb-reclaim-idle-only-20261112`
**572.224.458 B (545,72 MiB)**. Kanıt dosyaları 47,97 MiB. Önceki geçici
klasörler korunuyor. [Birim testleri](benchmarks/reclaim-idle-only-50k-20261112/unit-tests.log)
