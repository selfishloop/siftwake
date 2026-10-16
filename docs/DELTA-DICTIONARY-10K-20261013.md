# Scope altında RAM parça sözlüğü

Delta gram sözlüğü `(scope, gram) → posting` yerine
`scope → (gram → posting)` olarak tutulur. Bir scope metni her gram için
ayrı saklanmaz. Mevcut posting güncellemelerinde gram/scope kopyası oluşturan
lookup kaldırıldı; gram yalnız ilk sözlük girdisi oluşurken kopyalanır.
Scope sözlüğü belge başına bulunur. Boş posting ve son boş scope silinir.

Bu RAM içi düzenlemedir. Kalıcı Term::Gram ve term sırası, native payload,
source/index codec, WAL/manifest/protokol aynı. Yeni feature/bağımlılık yok.
Belge ve normalleştirilmiş body kopyaları kaldırılmadı; genel RAM işi açık.

## 10k yazma, bakım, proses belleği

Üç paired process/variant, sıra kontrol→aday / aday→kontrol / kontrol→aday.
Release merge-postings+delta-bitmaps; diagnostic/grouped-read kapalı.
Kontrol önceki WAL-lifetime kodudur; yeni dictionary dışında aynı kod/özellikler.
Artifact hash'leri `benchmarks/delta-dictionary-10k-20261013-executables.json`.

| Ölçüm | Kontrol medyan | Aday medyan |
|---|---:|---:|
| Yazma | 40,556s | 34,722s |
| Son bakım | 2,439s | 2,081s |
| Yazma + son bakım | 42,995s | 36,803s |
| İndeksleme observed private peak | 111,75MiB | 110,36MiB |
| Bakım observed private peak | 100,54MiB | 97,58MiB |
| Tüm ingest prosesinin OS peak WS'i | 114,65MiB | 114,05MiB |
| Boşta private | 20,72MiB | 20,72MiB |
| Tutulan DB dosyaları | 1.513.744.522B | 1.513.744.522B |

Medyan toplam%14,40 kısa; üç çiftte de hızlı. İndeksleme private medyanı
yaklaşık1,39MiB /%1,24 düşük; bakım yaklaşık2,96MiB /%2,95 düşük.
Bellek kazancı küçük; bütün belge kopyaları veya RAM farkı çözüldü iddiası yok.

| Çift | Kontrol toplam s | Aday toplam s | Toplam fark | İndeksleme private farkı |
|---|---:|---:|---:|---:|
| 0 | 42,995 | 40,203 | -%6,49 | -%0,99 |
| 1 | 44,465 | 36,803 | -%17,23 | -%2,29 |
| 2 | 42,728 | 32,863 | -%23,09 | -%0,89 |

Medyanlar ve paired oranlar ayrı hesaplanır; en iyi süre seçilmez. Değişim
aralığı geniş, arka plan sistem yükü izole değil. SQLite bu tur ölçülmedi;
önceki oturumdaki SQL süreleriyle yeni oran çıkarılmaz.

Aynı mevcut ilk10k corpus `data/rkyv-corpus-20261009/json.payload-bench`:
264.842.858 text byte,2.057–51.197B/doc, sekiz Gutenberg kitabının
benzersiz/örtüşen pasajları. Yeni corpus/download/prepare yok.
Sabit FDB slotu yalnız ingest gerektiğinde sahiplik/path kontrolüyle
sıfırlandı. SQLite slotu değiştirilmedi, koşu başına DB kopyası tutulmadı.

500 belge/transaction,20 synced WAL commit. Streamed CRC input + JSON decode,
indeksleme/commit ve40 batch-edge full-doc get kontrolü ingest timer içinde.
Son flush/merge ayrı. Her koşu sonrası zamanlama dışında bütün10k belge/
version ve dört contains total/ilk50 full-page oracle doğrulandı. Her aday
ayrıca eski control reader ile açıldı/doğrulandı. Her koşu aynı sequence20,
generation31,delta0,5 run ve19.221.340 disk posting verdi.
Source267.900.709B,index353.242.581B,WAL45B; disk fark0B.

OS cache eviction yok, cold cache yok.20ms private/WS poll,100ms phase poll;
kısa tepeler/faz sınırları kaçabilir. OS peak WS bütün ingest prosesidir;
private/WS engine allocation veya toplam sistem file cache değildir.
MiB1048576B. Idle strict open +5s bekleme, search warmup yok.

Runner `tools/measure-stream-checkpoints.ps1` yeni control/candidate
parametreleriyle kullanıldı. Summary yalnız log okuyan
`tools/summarize-stream-checkpoints.py` ile üretildi.
Ham sonuç prefix `benchmarks/delta-dictionary-10k-20261013`.

## Flush edilmemiş delta araması

Aynı corpus'un ilk1000 belgesi, ayrı üç paired process/variant.
Bu alt örnek10k disk contains kapısının yerine geçmez. Options128MiB/2000key
ile flush yapılmaz. Reverse ingest,40 delete/reinsert ile aynı anahtarların
title/body değişimi ve WAL'den yeniden açılış karşılaştırılır. Testteki40
güncelleme yeni corpus değildir; var olan bounded delta doğrulama aracının
mutasyonlarıdır. Alt örnek koşuları sonunda aynı slot tekrar10k mevcut
corpus ile doldurulup tam doğrulandı; 1000 belge DB'si saklanmadı.

Queries: `the`, `quixote`, `he`, AND `the man`, negatif/sonradan pozitif
`istanbul`, `updatedunique`. Her üç state'te her query1 untimed warmup +10
timed sample/process; pooled30 sample/case, nearest-rank p95.
Total/order/ilk50 full StoredDocument/version independent scalar fold oracle
ile API timer dışında doğrulanır. Query normalize, posting lookup/intersection,
full-document get/hydration timer içindedir. RAM burada ölçülmez; caller/oracle
1000 belge tutar. OS cache eviction yok.

Kontrol query exe aynı feature setiyle yalnız dictionary değişikliği çıkarılmış
koddur. Build sırasında mevcut dirty index.rs `finally` ile bayt eşitliğiyle
geri yüklendi. SHA'lar `benchmarks/delta-dictionary-query-executables.json`.
Ham prefix `delta-dictionary-query-20261013`; ayrı summary script'i
`tools/summarize-delta-dictionary-queries.py`.

**18 case'in17'sinde pooled p95 düşük,1'inde yüksek.** Per-process farklar
karışıktır; tüm sorguların her çiftte hızlı olduğu iddiası yok.

| State / sorgu | Kontrol p95 ms | Aday p95 ms | Hızlı çift |
|---|---:|---:|---:|
| delta / the | 0,5977 | 0,3944 | 2/3 |
| delta / quixote | 0,1100 | 0,0347 | 1/3 |
| delta / he | 0,7883 | 0,3768 | 3/3 |
| delta / the man | 1,2567 | 0,7597 | 2/3 |
| reused / the | 0,9106 | 0,4085 | 3/3 |
| reused / he | 0,8600 | 0,3694 | 2/3 |
| reopened / the man | 2,3527 | 1,1985 | 3/3 |
| reused / updatedunique | 0,0466 | 0,0605 | 2/3 |

Tek artış0,0139ms (~14µs),+%29,83. Kontrol run p95
0,0558/0,0407/0,0466; aday0,0431/0,0612/0,0438.
Yeni delta arama artışı önceki38-case kullanıcı kabulünün ek istatistiksel
kanıtı değildir; bu fark ham sonuçta korunur. No-regression garantisi verilmez.
Bu dar RAM sözlüğü değişikliği yazma hızının3/3 çiftte iyileşmesi ve gözlenen
belleğin3/3 çiftte azalmasıyla korunur.

## Takip

Default ve all-features `cargo test --all-targets`, `cargo fmt --check`,
default ve all-features `cargo clippy --all-targets -- -D warnings` geçti.
Merge/delta export'un bağımsız yeniden indekslemeyle bayt eşitliği, karışık
scope/collection, slot reuse, update/delete, bozulma/kesilme/process-kill ve
0.1 fixture testleri korunur. Güncel release aynı10k DB'de38-case
total/page/full-document/version oracle kontrolünü geçti; bu son kontrol
zamanlama değildir (`delta-dictionary-matrix-check.jsonl`).
Loglar `delta-dictionary-{tests-default,tests-all-features,fmt,clippy,clippy-all-features}.log`.
Release examples sonunda güncel merge+bitmap, grouped/diagnostic kapalı
build olarak bırakıldı. Mevcut dirty çalışma korunur; eski query kontrolü
üretilirken yalnız bu değişiklik geçici çıkarıldı ve kaynak bayt eşitliğiyle
geri kondu. Git reset/init, dış proje entegrasyonu veya fixture değişikliği yok.

Belge/folded-body kopyaları, bakım RAM'i, eski nesil disk alanı ve concurrent
yük [OPEN-WORK](OPEN-WORK.md) içinde açık kalır. Önceki arama incelemesi
kullanıcı kabulüyle kapalıdır; yeni alt örnek delta farkı yukarıda ayrıca kayıtlıdır.
