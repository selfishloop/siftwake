# Güncel yazma, bakım, RAM ve disk karşılaştırması

Üç dönüşümlü process çiftinde medyan yazma + son bakım FDB37,73s,
SQLite46,41s: FDB medyanı%18,71 daha kısa. FDB2/3 çiftte hızlı;
ikinci çiftte%1,04 yavaş. Bütün koşularda üstünlük gösterilmedi.
Son bakım, RAM ve toplam tutulan disk alanında SQLite daha küçük/hızlıdır.

## Sonuçlar

| İşlem | FDB medyan | SQLite medyan |
|---|---:|---:|
| 10k belge yazma | 35,807s | 44,955s |
| Son bakım | 1,919s | 1,458s |
| Yazma + son bakım | 37,726s | 46,412s |
| İndeksleme gözlenen özel bellek tepesi | 124,32MiB | 8,33MiB |
| Bakım gözlenen özel bellek tepesi | 98,73MiB | 8,25MiB |
| Tüm ingest prosesinin OS peak working set'i | 127,90MiB | 12,63MiB |
| Boşta özel bellek | 21,19MiB | 1,23MiB |
| Boşta working set | 22,91MiB | 5,32MiB |
| Tutulan DB dosyaları | 1513,74MB | 583,86MB |

| Çift / sıra | FDB yazma + bakım s | SQLite yazma + bakım s | FDB süre farkı |
|---|---:|---:|---:|
| 0: FDB → SQLite | 26,782 | 35,213 | -%23,94 |
| 1: SQLite → FDB | 48,613 | 48,113 | +%1,04 |
| 2: FDB → SQLite | 37,726 | 46,412 | -%18,71 |

Değişim aralığı geniştir. Arama işindeki kullanıcı dalgalanma kabulü bu
yazma sonuçlarının tamamını otomatik kapatmaz. Medyanların oranı ile her
çiftin oranı ayrı değerlendirilir; en iyi süre seçilmez. SQLite son bakım
medyanı FDB'den%24,05 kısa, FDB yazma medyanı SQLite'tan%20,35 kısadır.

FDB canlı kaynak267.900.709B + index353.242.581B + WAL45B =621.143.335B
(621,14MB). Tutulan bütün nesiller1513,74MB; yaklaşık892,60MB canlı set
dışındaki nesillerden gelir. SQLite583.860.224B. Dolayısıyla yalnız eski
nesilleri kaldırmak canlı source/index alanının SQLite'tan büyük olmasını
çözmez. Bu tur runtime temizleme/format değişikliği yapılmadı.

## Ölçüm sözleşmesi

- Aynı mevcut `data/rkyv-corpus-20261009/json.payload-bench` ilk10k belge:
  264.842.858 text byte,2.057–51.197B/doc, sekiz Gutenberg kitabının
  benzersiz/örtüşen pasajları. Yeni corpus/download/prepare yok.
- Yalnız `data/benchmark-work/fdb` ve `.../sqlite` sabit slotları. Her ingest
  öncesinde yalnız sahiplik işaretli ilgili benchmark slotu sıfırlandı.
  Önceki DB kopyası saklanmadı; son çiftin iki DB'si kullanıma hazır tutuldu.
- FDB güncel release `measure_content_memory`, merge-postings+delta-bitmaps
  açık; grouped-read/diagnostic kapalı. Bu opt-in write özellikleriyle
  ölçümdür, bütün experimental özellikler veya defaults-only değildir.
- SQLite ayrı comparator; docs JSON payload + folded text + external-content
  FTS5 trigram, detail=none,columnsize=0; cache2MiB,mmap0,tempFILE,
  WAL+synchronous FULL. SQLite motor bağımlılığı değildir.
- İki tarafta20 transaction,500 belge/transaction, senkron WAL. İnput stream
  CRC/JSON çözme, indeks oluşturma, commit ve batch başı/sonu40 full-document
  get kontrolü ingest timer içindedir. Oracle kitapları RAM'de tutulmaz.
- FDB son bakım: flush, background compaction bekleme ve kalan compaction'ları
  bitirme. SQLite: FTS optimize + WAL checkpoint(TRUNCATE). Bunlar birebir
  aynı bakım algoritmaları değildir; ingest sırasında yapılan bakım da
  ingest süresine dahildir. Toplam süreyi birlikte değerlendirmek gerekir.
- OS cache, eviction yok; soğuk cache veya izole donanım/arka plan yükü
  garantisi yok. Üçer ayrı ingest ve üçer idle process. Idle strict open
  sonrası5s bekleyip örneklenir; search warmup yok. SQLite open full-integrity
  yapmaz, FDB strict open kaynaklarını doğrular; open eşitliği iddiası yok.
- Proses özel belleği ve working set20ms poll; faz100ms log poll ile
  belirlenir. Faz sınırları/kısa tepeler kaçabilir. OS peak WS bütün proses
  (opening/ingest/bakım) içindir; engine allocation veya sistem file cache
  toplamı değildir. MiB=1048576B, disk MB=1000000B.

Prefix `benchmarks/write-maintenance-current-10k-20261013`;
`tools/summarize-write-maintenance.py` yalnız logları okur. Her run için
ingest/idle `.jsonl` ve `.memory.json` ayrı saklanır; DB kopyaları yok.
Güncel executable hash'leri `benchmarks/write-maintenance-current-executables.json`.

## Doğrulama ve araç düzeltmesi

Her FDB koşusu sonrası ayrı, zamanlama dışı proses bütün10k belgenin tam
içeriğini/version'ını ve dört sorgunun total/ilk50 tam sayfasını kontrol etti.
Her SQLite koşusunda PRAGMA integrity_check ve FTS external-content integrity
geçti. İlk SQLite koşusunda mevcut dört sorgu benchmark'ının tam sonuçları
doğrulandı. İkinci/üçüncü koşularda ayrıca bütün10k JSON payload kaynakla
kayıpsız karşılaştırıldı ve dört sorgu total/page/version kontrolü yapıldı.

SQLite read aracında `--matrix-check` desteklenmediği için çağrı normal
dört-query benchmark'a düşüyordu. İkinci koşudaki gereksiz read benchmark
sonlandırıldı; o kısmi log performans sonucu olarak kullanılmadı.
`--check-only` ve content_memory `verify` seçenekleri eklendi. İkinci SQLite
ingest sonucu yeniden üretilmedi; üçüncü ingest yeni verify destekli build
ile çalıştı. Timed ingest/bakım işlemlerinin tanımı aynı kaldı; iki farklı
comparator build kullanılmış olması ölçümün sınırlamasıdır.

FDB çalışma kodunda bu tur değişiklik yok; comparator doğrulama seçenekleri
ve ölçüm script'inde SQLite executable parametresi eklendi. Source/WAL/
manifest biçimi ve0.1 fixture korunur. Storage kurtarma davranışı değiştirilmedi.
FDB ve ayrı SQLite paketinde `cargo fmt --check` ve
`cargo clippy --all-targets -- -D warnings` geçti. Comparator content_memory /
content_reads test target'ları geçti; ek10k live doğrulama yukarıda belirtilir.
Loglar `benchmarks/write-maintenance-current-{fmt,clippy,sqlite-fmt,sqlite-clippy,sqlite-tests}.log`.

## Kalan işler

Arama artışları2026-10-13 kullanıcı kararıyla ölçüm dalgalanması kabul edilerek
kapatıldı; ham ölçümler [önceki raporda](REGRESSION-READS-10K-20261013.md).
Güncel yazma/bakım karşılaştırması artık ölçülmüş durumda. Sıradaki başlıklar:
indeksleme/bakım RAM tepesini düşürmek, eski nesil alanını güvenli yönetmek,
bakım sürerken okuma/yazma gecikmesi ve soğuk cache. Bu rapor bu işleri kapatmaz.
