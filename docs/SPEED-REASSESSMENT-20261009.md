# Okuma, yazma ve içerir araması — 2026-10-09

Kullanıcı önceliği değiştirdi: bu aşamada seçim ölçütü okuma/yazma ve içerir
sorgusunun sonuç getirme hızıdır. RAM azaltımı optimizasyon hedefi değildir.
SDB/Fundops entegrasyonu kapsam dışıdır; motor bağımsız kalır.

## Yeni karşılaştırma

Üç dönüşümlü çift FDB/SQLite, SQLite/FDB, FDB/SQLite. Her koşu yeni dizin:
100k belge, tekrarlı256 UTF-8 byte Türkçe gövde, benzersiz title/key, source/day,
batch500. Native Rust API; IPC yok. Sorgu tam toplam + key sıralı50 tam
StoredDocument döndürür; bütün payload/sürüm/sıra bağımsız corpus oracle'ıyla
timer dışında doğrulanır. 50update/50delete ve reopen doğrulanır. Sıcak yerel OS
cache, sorgu öncesi untimed warmup. Ölçüm sırasında build/test/profile yok.

FDB frozen `output/fdb-speed-baseline-20261009.exe` seçilmiş deneysel composite:
merge-postings, shared-documents, shared-delta-text, delta-bitmaps. Transitively
streaming/paged metadata, grouped-read ve write-pipeline yolları açıktır.
Builder-budget/WAL-delta/diagnostic kapalıdır. Bu normal default binary değildir.
SHA256 `3792BE204D89221385B77A60B629FBE7F94246DB5B28741C1B93C9D9FF473E93`.

SQLite frozen native-v2 SHA256
`28E9707EA778BBCE75E17E5030D605E2CCA51A3612FBAD145D9A17D33A69E181`:
WAL/FULL,2048KiB requested cache,mmap0,temp_storeFILE, UNIQUE key, alan B-tree ve
external-content FTS5 trigram. Hazır engine yalnız ayrı karşılaştırma paketindedir.
Şema bütün FDB dinamik alan/link/guard/Unicode API'sinin eşdeğeri değildir.
FDB kaynakları açılışta tam doğrular; SQLite normal open full integrity değildir.
SQLite full integrity ayrıca çalışır ve sıradan yazma timer'ına dahil değildir.
Batch timer'ında serialization hazırlığı farklıdır; toplam ingest esas alınır.

Üç koşunun p50 medyanları:

| İşlem | Deneysel FDB | SQLite |
| --- | ---: | ---: |
| 100k yazma, corpus üretimi dahil | 5.2650 s | 6.2779 s |
| Son flush/indeks bakım | 3.3413 s | 0.2085 s |
| Yazma + bakım | 8.5893 s | 6.4864 s |
| Dar metin, tek sonuç | 2.7732 ms | 13.5522 ms |
| Alan eşitliği, ilk50 + toplam | 0.1962 ms | 11.1129 ms |
| Alan + sayısal aralık | 0.4004 ms | 1.9852 ms |
| Yaygın içerir: teknoloji,99900 sonuç | 199.7970 ms | 326.2392 ms |
| 100 farklı key ile tam belge | 1.5703 ms | 0.9654 ms |
| Alan eşitliği, son sayfa | 1.4030 ms | 21.3483 ms |

Ingest tek başına hızlı, fakat bakım dahil FDB %32.4 yavaş. Key get100 %62.7
yavaş. Dar metin4.89x, yaygın metin1.63x hızlı; bu configured comparator'a aittir,
genel SQLite üstünlük sınırı değildir. Sorgu sonuçlarını sadece ID getirerek veya
tam toplamı kaldırarak hızlandıran farklı sözleşme ölçülmedi.
Raw: `benchmarks/speed-review-20261009-{fdb,sqlite}-{0,1,2}.{jsonl,memory.json}`;
oracle doğrulamalı özet `speed-review-20261009.summary.json`.

## İçerir matrisi

`examples/measure_contains.rs` aynı tamamlanmış100k DB'yi read-only açar. Açılış
ve oracle corpus üretimi timer dışında; her sorgu ilk çağrı +20 tekrar. Her
sayfanın tam içerik/sürüm/sıra ve toplamı bağımsız normalizasyon oracle'ıyla
kontrol edilir. İlk çağrı cold-disk ölçümü değildir. Tek proses/tek DB matrisi
üç çift kadar güçlü istatistiksel kanıt değildir.

| Sorgu | Toplam | p50 |
| --- | ---: | ---: |
| rapor-0000050001 | 1 | 2.7737 ms |
| teknoloji | 99900 | 197.1159 ms |
| teknoloji bilanço (AND alt-dize) | 99900 | 283.6915 ms |
| Şİ (iki karakter) | 99900 | 216.6781 ms |
| bulunmayan-xyz | 0 | 0.0141 ms |
| 000000000000 (trigram adayı, gerçek eşleşme yok) | 0 | 224.7742 ms |
| teknoloji + day2..5 | 13336 | 115.5164 ms |
| teknoloji, offset50 | 99900 | 217.8228 ms |
| teknoloji, offset99850 | 99900 | 225.3284 ms |

Raw `benchmarks/contains-review-20261009-baseline.jsonl`. Başlık/gövde ve Türkçe
eşleştirme korunur; whitespace ayrı terimleri AND yapar, tırnaklı phrase API'si
değildir. Kısa sorgu trigramla elenemez. Eksik gram hızlı boş cevap sağlar;
gramların bulunması gerçek substring garantisi değildir. Sıfır sonuçlu false-gram
sorgusunun da yavaş olması darboğazın sayfa getirmeden önce olduğunu gösterir.
Exact total korunduğu için limit50 olsa da tüm adaylar doğrulanır.

## Uygulama sırası

### Varsayılan motor ve profil ayrımı

Default bağımsız bir yeni100k native koşuda yazma+bakım14.2000s, get1000.7009ms,
dar metin3.3729ms, yaygın metin413.6083ms çıktı. Bu tek koşu farklı bir zaman
penceresidir; üç çiftin yerine geçmez ve doğrudan kesin hız oranı kurulmaz.
Raw `benchmarks/speed-review-default-20261009.{jsonl,memory.json}`.
Default aynı tamamlanmış DB'de contains matrisini de geçti; yaygın kelime
p50399.6047ms, iki terim413.6845ms, kısa iki karakter408.3323ms.
`benchmarks/contains-review-20261009-default.jsonl`.

Default'un key get ölçümü deneysel paged metadata yolundan daha iyi; grouped
payload/text pencere okuması ise yaygın aramada deneysel yolun avantajıdır.
Bu nedenle bellek kaygısıyla paged metadata yolunu her okuma için seçmek hız
hedefiyle tutarlı değildir. Resident metadata + grouped text I/O kombinasyonu
ayrı prototipte ve aynı koşulda ölçülmelidir; bu tur uygulanmadı.

Seçilmiş composite + diagnostic-timing ayrı binary, bir ilk +bir tekrar:

* Yaygın teknoloji: toplam search464.875ms, JSON220.929ms (%47.5),199800
  text decode çağrısı,6550 positional read. Sayfa hydration0.521ms.
* Teknoloji + range: search239.950ms, JSON32.742ms,26672 decode,25465 read;
  nested read span124.895ms. JSON tek darboğaz değildir.
* Dar title: search6.681ms,3422 read,24 text decode, JSON0.066ms.

Nested span süreleri toplanamaz, CPU attribution değildir; profil binary'sinin
süreleri performans sonucu gibi sunulmaz. Her sorgu sonrası counters sıfırlanır,
open ve oracle hazırlığı hariçtir. Raw
`benchmarks/contains-profile-20261009-{common,range,rare}.jsonl`.

1. İçerir doğrulamasında tüm Document/fields/links yerine yalnız gerekli
   title/text/key/version çözümleyen bir yol prototiple. Escaped JSON ve Unicode
   için doğru çözüm sürsün; CRC, uzunluk, kimlik ve authoritative kaynak doğrulaması
   korunmalı. Hem ilk çağrı hem tekrarlar, unique ve büyük gövdeler ölçülmeli.
2. Dar metin/alan birleşiminde posting erişimlerini bloklar halinde kesiştir;
   aday başına tekrar binary-search/positional-read sayısını azalt. False gram,
   çok terim, aralık ve derin sayfa oracle testleri geçmeli.
3. Key okumada metadata lookup ve payload okuma/çözümleme ayrı profillensin.
   RAM önceliği kalktığından resident metadata ile grouped text okumasını
   birleştiren seçenek de hız kapısına alınmalı.
   100-key ölçümü toplu API değildir; yeni batch-get varsa ayrı sözleşme olarak
   ölçülmeli. Hızlı görünen sayfa okuma tekil get açığını gizlememeli.
4. Yazmada final bakım3.34s maliyetini kaynak yazımı, CRC, source replay ve indeks
   üretimine ayır; aynı kalıcılıkta gereksiz tekrarları kaldır. Yalnız ingest
   üzerinden SQLite'tan hızlı olunduğu ilan edilmesin.

Bu tur ölçüm/harness ekler, motor davranışı veya kalıcı format değiştirmez.
İleride hız değişikliği correctness/corruption/process-kill ve query oracle
testleriyle, aynı binary/iş yükü ve dönüşümlü koşularla kabul edilir. RAM sonucu
bu aşamanın kapısı değildir. Cold I/O,1M,unique/uzun gerçek dosya içerikleri ve
eşzamanlı okuyucu/yazıcı hızları bu tekrarın kapsamı dışındadır.

Yeni harness varsayılan ve composite ile dokuz sorgunun tam oracle kontrolünü
geçti. Üç native FDB/SQLite çifti ve ayrı default native koşu full oracle,
update/delete ve reopen kontrolünü geçti. `cargo fmt --check` ve default,
composite+diagnostic, all-features için `cargo clippy --all-targets -- -D warnings`
geçti. Motor kodu değişmediğinden depolama suite'leri tekrar çalıştırılmadı;
önceki suite sonuçları ayrı builder doğrulama belgesindedir.
