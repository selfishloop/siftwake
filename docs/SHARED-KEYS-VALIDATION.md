# Ortak delta anahtarları — 2026-10-09

## Değişiklik

`experimental-shared-documents` resident yolunda docs/indeks/shadow aynı
Arc<Key>'i kullanır; key'in scope/collection/id String'leri tekrar tutulmaz.
Yeni key shadow'a alınır, sonraki update/delete aynı kimliği kullanır. Recovery
shadow/docs'u kurar, indekse aynı Arc'ı verir. İndeks paylaşılan key ve public
document key eşitliğini mutation'dan önce kontrol eder. Public get/search
caller-owned Key/StoredDocument döndürür. Arc ve HashMap API'ye sızmaz.

Docs standart Rust HashMap, shadow sıralı BTreeSet, mevcut posting indeksleri
korunur. Key Hash türetir; hash kalıcı formata yazılmaz. Hash iterasyonu recovery
slot sırasını değiştirebilir; query key sırasıyla sıralar, flush shadow key sırasını
kullanır. Başarılı flush docs tablosunun kapasitesini de bırakır. HashMap içinde
kalıcı veri tutulmaz; normal kaynak checksum/schema/length doğrulaması sürer.

WAL-delta aynı anda açıksa eski WAL yerleşimi seçilir. Default belge yerleşimi
değişmez. Kalıcı codec/0.1 fixture kaynakları değişmedi. Yeni DB/arama kütüphanesi
yok; std HashMap bir hazır DB motoru değildir. SDB/Fundops entegrasyonu yapılmadı.
Ortak toplam delta/builder/worker RAM kotası hâlâ tamamlanmadı.

## V1 hız kapısı

Docs B-tree + ortak Arc ilk V1 deneyi, 100k/256byte/batch500 üç çiftte
tepe working set22.8203→19.0117MiB; yazma+bakım9.1448→9.1344s.
Ancak unflushed unique10k get100 p50 0.1046→0.1297ms, broad text
0.8927→1.4112ms. Unique üç koşunun hepsinde yavaşladı. RAM karşılığında hız
kaybı kabul edilmedi; bu yerleşim final olarak seçilmedi. Ham V1 korundu.

V2 docs'u HashMap'e taşıdı. 10k unique get100 0.0946→0.0793ms, broad text
0.7772→0.7473ms. Aynı anda sorted shadow/index anahtar paylaşımı sürer.

## Ölçüm sözleşmesi

Native 100k: yeni DB, tekrarlı256 UTF-8 byte body, batch500, watermarks10k /
16MiB; whole-process open/ingest/maintenance/reopen/query/50update/50delete
PeakWorkingSet64. Private tepe hedef20ms örneklenir. Sistem RAM/OS-wide cache
dahil değildir. Sıcak OS cache; full payload/version/count/ordered-page oracle
timer dışında. Build/test/profile native ölçüm sırasında çalıştırılmadı.

Delta query: ayrı yeni10k, watermarks10001/64MiB,20×500,seq20,0segment;
flush yok, open/yazma query timer'ından çıkarılmış. Repeated aynı256byte body,
unique her gövdeye unique ID soneki + nested metadata, mixed çift ID'ler unique.
256byte temel gövdedir; unique gövdeler sonekle daha uzun. 10 warmup; küçük
sorgular200, last/get100, text20 timed tekrar. Get100 n*97+1, dar rapor5000,
source exact, day2..5 range, teknoloji text, son offset9950. Her sonuç total,
sıralı StoredDocument/sürüm independent corpus oracle'ıyla kontrol edilir.
Bu harness proses RAM ölçmez; resident text sayacı RSS değildir.

Her karşılaştırma B/C,C/B,B/C üç çift. Baseline frozen shared-documents-v2;
delta baseline frozen shared-documents-delta-v3-candidate. İkisi de önceki
payload paylaşımı + Arc query keys + merge-postings/shared-delta-text/bitmaps
kombinasyonudur. Candidate aynı kombinasyonda resident canonical key + HashMap
ekler. WAL/diagnostic features native ölçümde kapalıdır. Raw:
`docs/benchmarks/shared-keys-{v1,v2}-20261009-*` ve
`shared-keys-delta-{v1,v2}-20261009-*`.

## Delta V2 sorgu sonucu

Medyan p50 ms:

| Corpus / sorgu | Baseline | Candidate |
| --- | ---: | ---: |
| repeated first | 0.3005 | 0.2619 |
| repeated range | 0.0884 | 0.0788 |
| repeated last | 0.2788 | 0.2624 |
| repeated text | 0.7410 | 0.7017 |
| repeated get100 | 0.0748 | 0.0525 |
| unique first | 0.2784 | 0.2901 |
| unique range | 0.0992 | 0.0918 |
| unique last | 0.2899 | 0.2734 |
| unique text | 0.7772 | 0.7473 |
| unique get100 | 0.0946 | 0.0793 |
| mixed first | 0.2797 | 0.2813 |
| mixed second | 0.2813 | 0.2979 |
| mixed last | 0.2981 | 0.2748 |
| mixed text | 1.0282 | 0.7271 |
| mixed get100 | 0.0901 | 0.0673 |

Get100 %16–30 daha kısa. Unique first p50%4.2, mixed second p50%5.9 arttı;
bu veri her sorguda kesin hız korunması anlamına gelmez. p95 ve tüm diğer
query sonuçları `shared-keys-delta-v2-20261009.summary.json` içinde.
Reproduction: `python tools/summarize-delta-pairs.py
docs/benchmarks/shared-keys-delta-v2-20261009`.

## Native V2 sonucu

Medyanlar (sum her koşunun toplamının medyanıdır):

| Ölçüm | Baseline | Candidate |
| --- | ---: | ---: |
| Yazma | 5.6592s | 5.5718s |
| Son bakım | 3.4396s | 3.4232s |
| Yazma + bakım | 9.0989s | 9.0440s |
| Tepe working set | 22.5195MiB | 18.4805MiB |
| Örneklenen private tepe | 19.6211MiB | 15.7227MiB |

Working set %17.94 azaldı. Yazma+bakım %0.60 daha kısa; bu küçük fark büyük
bir hız kazanımı olarak sunulmaz. Altı koşu oracle/50update/50delete/reopen'dan
geçti. Final marker, iki fdb ve iki idx üç çiftte byte-identical. Kanıt:
`shared-keys-v2-20261009.byte-equality.json`; read-only tekrar:
`python tools/verify-native-publications.py data/shared-keys-v2-20261009`.

Flushed query p50/p95 ms:

| Sorgu | Baseline | Candidate |
| --- | ---: | ---: |
| narrow | 2.7704 / 3.6584 | 2.7531 / 3.6958 |
| broad | 0.2070 / 0.3510 | 0.1897 / 0.3675 |
| range | 0.4195 / 0.6446 | 0.4131 / 0.5969 |
| second | 0.2046 / 0.2829 | 0.1969 / 0.2746 |
| last | 1.2769 / 1.8168 | 1.1906 / 1.7864 |
| text | 222.5689 / 246.4981 | 228.6001 / 229.1590 |
| get100 | 1.5533 / 1.6264 | 1.4952 / 1.9927 |

Text p50%2.71 ve get100 p95%22.52 arttı. Son/text/get yalnız üç timed repeat
içerir; p95 örnek sayısı azdır, genel tail-hız güvencesi vermez. Bu ölçümde
delta flush edilmiş ve immutable kayıtlar okunmuştur; delta-only kazancı ayrı
harness'tadır. Genel hız/RAM hedefi ve default promotion kapısı açık kalır.

Frozen native SHA256:

- `output/fdb-shared-keys-v2-20261009.exe`: `5EFBF8BF1762EF9ED9150CB0D8A1A0D00A135C51763E0F0541A80A4C7AE7A1D9`
- `output/fdb-shared-keys-delta-v2-20261009.exe`: `0F2706AAC9B3EC242DFB72B2BF7B1147A2074F4B8C23A13229A16CF4F8ED3362`

## Son doğrulama

Final runtime koduyla seçilmiş kombinasyon89, standalone shared-documents56,
default53, all-features91 test geçti. Dört kombinasyonda strict
`cargo clippy --all-targets -- -D warnings` ve `cargo fmt --check` başarılı.
Standalone Clippy ilk kez BTreeMap import'unun kullanılmadığını bildirdi;
import yalnız gereken feature'lara bağlandı, dört Clippy tekrar geçti.
Test logları `shared-keys-v2-{selected,standalone,default,all-features}-tests-20261009.log`.

Yeni unit testi indekste paylaşılan key Arc kimliğini, yanlış key ile add/remove
işleminin mutation öncesi reddini doğrular. Full suite source/cache/manifest
bozulması, her frame cut, acknowledged process-kill, update/delete/guard,
interleaved pagination, random model, source byte eşitliği ve 0.1 uyumluluğu
kapsar. Standalone noncompact ve seçilmiş compact/bitmap yolları ayrı çalıştı.
All-features WAL yolunu seçer, resident key paylaşımını tek başına doğrulamaz.
Normal CLI default `cargo build --release --bin fdb` ile üretildi; SHA256:
`F63EC59AA1EB5FD1A04D698472EB5D44AE1D68C61C299974EEA59096EE642582`.

Yeni ayrı origin allocator profili: total requested allocation tepe11062412
byte (10.55MiB), önceki shared-documents profili14169896 byte. Bu RSS/hız
ölçümü değildir. Profilin en yüksek sampled-live noktasında (100000 kayıt)
delta_tables3440896, delta_index1783666, packed_metadata930015 byte.
Bağımsız builder origin tepe3233677 byte kaldı. Origin ilk allocation sahibini
gösterir; key allocation'ları index'ten delta_tables'a kaymıştır. Origin
tepeleri aynı anda oluşmaz ve toplanamaz. Kontrollü I/O reservation tepe
131072/limit262144 byte; ortak toplam heap bütçesi değildir. Ham ve özet:
`shared-keys-origins-v2-20261009.jsonl`,
`shared-keys-v2-20261009.diagnostic-summary.json`.

## Kalan hedef

SQLite bu aşamada yeniden ölçülmedi; SQLite'tan daha düşük RAM / daha yüksek
hız hedefi tamamlanmadı. Unique/büyük corpus için full-process bellek, cold-cache,
1m ve genel hız kapıları ölçülecek. Anahtar paylaşımı toplam heap kotası değildir.
