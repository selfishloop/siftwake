# 50k strict-open — sıralı anahtar taraması

**Aynı 52 run yerleşiminde sıcak açılış p50 1.350,65 → 730,71 ms (%45,9).** Açılış başlangıcından ilk sonuca p95 1.593,08 → 907,23 ms. Aday 12/12 dengeli blokta daha hızlı. Maske oracle’ı bit eşit. Bu sonuç geçici prototipe ait.

## Yöntem

- Cap200k TR ilk 50.000 belge (`881756DA…`), 52 run, 82.072 slot, generation 8185, sequence 529967. Önceki deneyin 159 dosyalı 467.167.854 B kopyası kullanıldı. Yeni ingest veya DB kopyası yok.
- Aynı release ikilisinde seri run-çifti kontrolü ve seri heap taraması, 24’er ayrı proses. Sıra ABBA ve BAAB dönüşümlü, toplam 12 blok. Her bloktan önce kopya cached I/O ile tamamen okundu. Gerçek cold, WPR ve purge kapsam dışı. Ölçüm penceresi 17:28:57–17:30:08 UTC, üst sınır 17:50:40 UTC.
- `olduğu`, melez N=50. Açılış saati `Database::open` çağrısından başlar. İlk sonuç saati aynı başlangıçtan başarılı sorgu dönüşüne kadardır ve aradaki hafif ölçüm çağrılarını içerir. Sorgu CRC oracle’ı zaman dışında. p95, 24 örneğin yukarı yuvarlanmış 23. sırası. CPU, süreç user+kernel. RAM, Windows süreç sayaçları ve OS dosya cache’i hariç.
- Aday her run’ın anahtarlarını slot sırasıyla bir kez okur. Heap aynı anahtarın run’larını manifest sırasıyla verir. Daha yeni kayıt görülünce önceki slot gizlenir. Tombstone da önceki slotu gizler. WAL shadow, maske cache’inin yazım sırası, görünür sayım ve strict CRC aynı kodla devam eder. Aday 52 ham bitseti aynı anda tutar, toplam 10.528 B. Heap run başına bir anahtar tutar. Daha büyük yerleşimlerde geçici bellek sınırı bu deneyle ölçülmedi.

## Sonuç

| Ölçüm | Seri kontrol p50 / p95 | Sıralı tarama p50 / p95 |
| --- | ---: | ---: |
| Strict-open (ms) | 1.350,65 / 1.462,01 | **730,71 / 792,59** |
| Görünürlük maskesi (ms) | 790,27 / 886,77 | **182,26 / 196,24** |
| Açılış → ilk sonuç (ms) | 1.458,61 / 1.593,08 | **837,91 / 907,23** |
| Yalnız ilk sorgu (ms) | 113,40 / 134,48 | 101,22 / 130,97 |
| Açılış CPU (ms) | 1.343,75 / 1.437,50 | **734,38 / 781,25** |
| Maske CPU (ms) | 781,25 / 859,38 | **187,50 / 187,50** |
| Açılış sonunda private (MiB) | 7,422 / 7,434 | 7,781 / 7,789 |
| Açılış tepe private (MiB) | 8,297 / 8,328 | 8,297 / 8,332 |
| İlk sonuca kadar tepe private (MiB) | 9,539 / 9,551 | 9,742 / 9,750 |
| İlk sonuca kadar tepe working set (MiB) | 13,980 / 13,988 | 14,266 / 14,273 |

Blok içi ortalamaların farkı medyanı açılışta −622,62 ms, ilk sonuçta −627,85 ms. İlk sonuca kadar tepe private p95 **+0,20 MiB**, açılış sonu private p95 **+0,36 MiB**. Sıcak 52 run ölçümü 2b gerçek soğuk kapısına girmez. Temiz yerleşim, başka belge ölçeği ve başka sorgu bu prototipte ölçülmedi.

## Doğruluk

- İki audit açılışında 97.306 B çıktı bit eşit ve önceki seri referansla aynı SHA-256. Shadow öncesi/sonrası bitmapler, run görünür sayıları ve cache sıra/içeriği dahil. Bu yerleşimde WAL shadow yeni bit eklemedi.
- 48/48 zamanlı sorgu ve 2/2 audit case0 oracle’ı geçti. 50 hit, CRC32 725765487, sequence 529967, 50.000 belge ve strict Verified.
- Geçici kopya, sabit FDB slotu ve model SHA kontrolleri ölçüm öncesi ve sonrası geçti. Üretim motoru, biçim ve varsayılan değişmedi.
- Adayın varsayılan açıldığı ayrı geçici doğrulama derlemesinde `cargo test --lib --tests` 230 geçti, 15 ignore. Process-kill, kaynak bozulması ve 0.1 fixture dahil. Ayrı eski ikiliyle yeniden okuma bu turda koşulmadı. İlk Clippy koşusu iç içe `if` biçeminde kaldı. Bu koşul `let` zincirine dönüştürülünce `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` ve 230 test geçti. Zaman tablosu ilk sabit release ikilisinden. [Son doğrulama](benchmarks/strict-open-key-sweep-50k-20261109/checks-style.json) · [Test günlüğü](benchmarks/strict-open-key-sweep-50k-20261109/test-style.log)

Otomatik çalıştırma politikası deneyin kendi `target` klasörünü silme komutunu engelledi. `C:\Users\Serkan\AppData\Local\Temp\fdb-open-key-sweep-20261109` altında yaklaşık **3,77 GiB** geçici derleme dosyası kaldı.

[Ham 48 örnek](benchmarks/strict-open-key-sweep-50k-20261109/samples.jsonl) · [Bloklar ve özet](benchmarks/strict-open-key-sweep-50k-20261109/analysis.json) · [İki audit](benchmarks/strict-open-key-sweep-50k-20261109/audit-results.jsonl) · [SHA ve zaman doğrulaması](benchmarks/strict-open-key-sweep-50k-20261109/validation.json) · [Prototip kaynağı](benchmarks/strict-open-key-sweep-50k-20261109/open-sweep-probe.rs.txt)
