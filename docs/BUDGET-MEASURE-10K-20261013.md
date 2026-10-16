# Mevcut bütçeli çalışma yolunun ölçümü

**Bu kombinasyon hızdan ödün vermeyen RAM çözümü olarak reddedildi.**
Üç çiftte indexing belleği azaldı; toplam yazma+bakım üç çiftte de yavaşladı.
Default feature ayarı değişmedi, motor çalışma kodunda bu tur değişiklik yok.

## Medyan sonuçlar

| Ölçüm | Normal yol | Bütçeli kombinasyon |
|---|---:|---:|
| Yazma | 33,091s | 32,000s |
| Son bakım | 2,076s | 6,155s |
| Yazma + son bakım | 35,167s | 38,593s |
| İndeksleme observed private peak | 110,00MiB | 102,70MiB |
| Bakım observed private peak | 99,86MiB | 96,07MiB |
| Tüm ingest prosesinin OS peak WS'i | 114,15MiB | 105,95MiB |
| Boşta private | 20,71MiB | 18,46MiB |
| Tutulan DB dosyaları | 1.513.744.522B | 1.514.639.487B |

Toplam medyan%9,74 yavaş; bakım yaklaşık2,96 kat süre aldı.
İndeksleme private%6,64, bakım private%3,79 düşük. Boşta fark yaklaşık2,25MiB.
Disk+894.965B (~0,85MiB); kaynak boyutu aynı, derived index/directory farkı var.
SQLite bu tur yeniden ölçülmedi; önceki8,25MiB bakım ölçümüne yaklaşılmadı.
Farklı oturumlardaki SQL sürelerinden güncel hız oranı çıkarılmaz.

| Çift / sıra | Normal toplam s | Bütçeli toplam s | Toplam değişim |
|---|---:|---:|---:|
| 0: normal → bütçeli | 34,461 | 38,593 | +%11,99 |
| 1: bütçeli → normal | 36,962 | 39,669 | +%7,32 |
| 2: normal → bütçeli | 35,167 | 37,535 | +%6,73 |

Bakım private pair farkları-%4,25/-%5,22/+%0,02. Dolayısıyla bakım RAM'i
her eşleşmede azalmış değildir. İndeksleme ve OS peak WS üç çiftte de düşük.
Medyanlar ve paired oranlar ayrı hesaplanır; en iyi koşu seçilmez.

## Gerçekte hangi bütçe ölçüldü?

Normal: güncel release merge-postings+delta-bitmaps; diagnostic kapalı.
Aday: bunlara existing `experimental-builder-budget` eklendi. Bu feature
write-buffer-budget → write-pipeline → merge-policy/record-spool/merge-slot →
streaming-open → paged-metadata → grouped-read/direct-slot yollarını da açar.
Streaming-metadata, WAL-delta ve maintenance-slots açılmadı.

Bu nedenle **tek başına bütçe etkisi** ölçülmüş değildir. Akışlı kaynak yazımı /
okuması, metadata disk yolu, merge politikası ve grouped hydration da değişir.
Süre farklarının hangi alt bileşenden geldiği bu tur profille ayrıştırılmadı.
Yeni feature eklenmedi, bağımlılık grafiği sadeleştirilmedi/değiştirilmedi.

Her aday ingest prosesinde son snapshot:

- Controlled write pool limit256KiB, peak reserved64KiB; waits0,reduced0.
- Posting pool limit2MiB, peak reserved1MiB; waits0,reduced0.
- İki pool da sonunda in_use0.

**Reserved byte gerçek allocation/RAM değildir.** Özellikle reusable-posting
builder'ı alan ayırma izni alabilir, fakat tamamını kullanmak zorunda değildir.
Bu sayaçlar256KiB veya2MiB toplam proses sınırı değildir. Pool sınırları bu
koşulda dolmamış, bekleme/lease küçültme gerçekleşmemiştir.

Delta'nın owned belgeleri, folded metinleri ve RAM indeks tabloları;
çağıranın500 belge batch'i; bazı encode/decode, checksum ve metadata
tahsisleri bu ortak pool'un kapsamında değildir. Bu yüzden yalnız mevcut
pool'u açmak97MiB bakım belleğini8–10MiB'ye indirmez. Mevcut API/benchmark
FDB'ye500 belgelik owned batch verir; SQLite comparator transaction içinde
input'tan tek belge okuyup insert eder. Transaction büyüklüğü aynı olsa da
caller belleği toplama biçimi aynı değildir; proses RAM farkının tamamı
cache boyutuna veya WAL'e atfedilemez.

Gerçek küçük toplam bellek hedefi, delta/body/metadata ve caller-batch
örtüşmesini de kapsayan bir tasarım ister. Bu henüz uygulanmış ve ölçülmüş
bir toplam bellek sınırı değildir; bu tur onu başardık iddiası yok.

## Corpus, cache, doğrulama ve sınırlar

Aynı mevcut ilk10k belge `data/rkyv-corpus-20261009/json.payload-bench`,
264.842.858 text byte,2.057–51.197B/doc. Yeni corpus/download/prepare yok.
Sabit FDB slotu yalnız ingest öncesinde sahiplik/path kontrolüyle sıfırlandı;
SQLite slotu değiştirilmedi, DB kopyaları saklanmadı. Logs docs/benchmarks.

Üç paired process/variant,500 belge/transaction,20 synced WAL commit.
Streamed CRC-checked JSON input decode + indeksleme/commit +40 batch-edge
full-document kontrolü ingest timer içinde; son flush/merge ayrı.
Cache OS cache, eviction yok; farklı donanım, cold cache ve concurrent
okuma/yazma yükü bu tur ölçülmez. RAM20ms sample, phase100ms log poll;
kısa peak/faz sınırları kaçabilir. OS peak WS tüm ingest prosesini kapsar;
engine quota veya sistem file-cache toplamı değildir. MiB1048576B.
Idle ayrı strict-open prosesinde5s bekleme sonrası, search warmup olmadan.

Her koşu sonrası bütün10k belge/version ve dört contains total/ilk50
full-page oracle zamanlama dışında doğrulandı. Her aday ayrıca normal
control exe ile okunup tam doğrulandı. Aynı sequence20,generation31,delta0,
5 run ve19.221.340 posting; source267.900.709B veWAL45B.
Disk index normal353.242.581B, aday353.688.186B.

Artifact SHA'ları ve ham logs `benchmarks/budget-measure-10k-20261013-*`;
summary `budget-measure-10k-20261013-summary.json`. Runner
`tools/measure-stream-checkpoints.ps1`, candidate features metadata ayrı
parametreyle kaydedilir. Summary script yalnız log okur. Bu tur yalnız
benchmark'a pool snapshot çıktıları ve runner feature etiketi parametresi eklendi.

## Karar ve açık işler

Bu existing feature kombinasyonu default kapalı kaldı. Ölçüm sonunda sabit
FDB slotu normal yol ile aynı10k belgede yeniden hazırlandı/doğrulandı;
experimental DB kopyası saklanmaz. Release examples normal merge+bitmap,
budget/grouped/diagnostic kapalı build'e döndürüldü.

Ölçülen exact feature setiyle `cargo test --all-targets` geçti; normal ve
aday için all-targets Clippy -D warnings, fmt --check geçti. Kesilme,
bozulma, acknowledged process-kill, merge publication ve0.1 fixture kontrolleri
bu test paketinde korundu. Normal yola dönüşte full10k document/version ve
dört-query oracle, ardından38-case full-page/version oracle geçti.
Son38-case kontrolü performans ölçümü değildir.
Loglar `budget-measure-{tests,fmt,clippy,clippy-feature}.log`,
`budget-measure-normal-verify.jsonl`, `budget-measure-normal-matrix.jsonl`.

[OPEN-WORK](OPEN-WORK.md): toplam bellek bütçesi, belge/folded-body/metadata
kopyaları, eski nesil disk alanı ve eşzamanlı yük açık. Önceki arama artışları
kullanıcı kararıyla kapalı; bu kombinasyon için yeni paired arama hız kapısı
çalıştırılmadı, yalnız veri/sorgu doğruluğu kontrol edildi.
