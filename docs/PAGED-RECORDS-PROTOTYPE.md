# Sayfalı kayıt tablosu deneyi — 08.10.2026

Karar: ilk sayfalı kayıt prototipi bellek yönünden umut veriyor, fakat hız kabul
kapısını geçmedi. Üretim motoruna alınmadı. `.fdb`, `.ready`, `.idx` biçimleri ve
normal motorun RAM kullanımı bu deneyle değişmedi. Bu belge aşama 2'nin tamamlandığı
veya FDB'nin SQLite'tan düşük proses RAM'ine ulaştığı anlamına gelmez.

## Deney

`examples/probe_paged_records.rs` normal `Database::open` ile kaynakları doğrular,
aktif run listesini alır; ardından SEG03 kayıt dizinlerini okur. Aktif 15 run'da
1.000.100 fiziksel kayıt vardır; eski sürümler ve tombstone'lar bu sayıya dahildir.
Görünür belge sayısı ile karıştırılmaz. Corpus `data/scale-v031-optimized-20261008`:
bir milyon başlangıç belgesi, tekrarlı 256 bayt içerik, sonraki güncelleme/silmeler
ve kurtarma testinin sentinel kaydı.

64 kayıtlık değişken boyutlu sayfalar; sayfa başına RAM'de ilk anahtar/offset fence'i,
sayfa içinde offset tablosu ve ikili arama; tüm run'lar için ortak 256 KiB sayfa
payload cache'i uygulanır. Scope/collection/id Unicode dizeleri korunur; kayıt
offset/version/len/checksum/deleted alanları birebir karşılaştırılır. Sayfalar yeni
`output/paged-records-binary-probe-20261008-*` klasörlerine yazılır. Deney dosyaları
motorda okunmaz; restart/publication/recovery sözleşmesi olan SEG04 değildir.
Deneyin FNV kontrolü üretim CRC/bütünlük tasarımının yerine geçmez.

Her biri yeni proses olan beş tekrar, tek makinede sırayla çalıştırıldı. Dönüşüm
dosya cache'ini ısıtır; cache temizlenmiş soğuk disk testi yapılmadı. Her iş yükünün
başında yalnız prototipin uygulama cache'i temizlenir. İki yol da aynı 20.000
anahtarı bulur ve aynı metadata tuple'ını doğrular. Beklenen sonuçların oluşturulması
ölçüm dışında; lookup ve sonuç karşılaştırması ölçüm içindedir. Resident referans,
deserialized kayıtların RAM'de ikili aranmasıdır; mevcut motorun PackedRecord/grup
havuzu uygulamasının end-to-end süresi değildir. Sıra resident→paged; ters sıra
deneyi yapılmadığından küçük farkları kabul kanıtı sayamayız.

| Seçilmiş alan | Miktar |
| --- | ---: |
| Mevcut motor record Vec + ID String capacity | 50.005.000 bayt / 47,69 MiB |
| Prototip fence Vec + anahtar String capacity | 1.875.720 bayt / 1,79 MiB |
| Ortak cached sayfa payload bütçesi | 262.144 bayt / 0,25 MiB |
| Fence capacity + payload bütçesi | yaklaşık 2,04 MiB |

Bu tablo **heap veya proses RAM ölçümü değildir**. Cache yönetim düğümleri,
allocation overhead, okunmakta olan geçici sayfa, deletion bitmap, token sözlüğü,
delta, builder/replay ve OS cache dahil değildir. Deney aynı anda resident referansı
ve JSON dizinlerini de ayırır; deney prosesinin RSS'ini motorun gelecekteki RAM'i
gibi kullanmak yanlış olur. Fence sayısı hâlâ kayıt sayısıyla doğrusal büyür.
Uzun anahtarlı tek sayfanın geçici allocation'ı da bu bütçeyle sınırlanmamıştır.

| 20.000 metadata lookup, beş koşunun medyanı | Resident | Sayfalı |
| --- | ---: | ---: |
| Run'lara dağılan mevcut anahtarlar | 51,610 ms | 294,844 ms |
| Her run'ın ilk 50 kaydında tekrarlı erişim | 8,872 ms | 15,451 ms |

Dağınık erişim her koşuda 18.709 uygulama sayfası okudu; lokal erişim yalnız 15.
Fark, tam sorgunun bu oranlarda yavaşladığını göstermez: payload, posting,
görünürlük, count ve sorgu planı bu ölçümde yoktur. İlk doğrusal sayfa taraması
ikili aramayla değiştirildi; eski denemenin ham dosyaları `paged-records-probe-*`
adlarıyla korunur, güncel karşılaştırmaya karıştırılmaz.

## Sonraki uygulamanın gereksinimleri

1. Sorgu heap'inde run/slot zaten biliniyorsa payload için tekrar global key araması
   yapılmamalı. Aynı sayfadaki metadata/payload okumaları gruplanmalı; slot erişimi
   ve key araması ayrı yollar olmalı. Tam sorgu kapısı bu düzen üzerinde ölçülmeli.
2. Scope/collection sözlüğü, sıkıştırılmış anahtar blokları ve iki seviyeli sayfalı
   fence dizini gerekir. Bu prototipin her 64 kayda bir owned Key'i nihai ölçek
   çözümü değildir. Tombstone kontrolü için posting döngüsünde her aday başına
   metadata sayfası okumak yerine sınırlı bitmap/visibility yolu gerekir.
3. Ortak cache bütçesi foreground ve merge worker arasında uygulanmalı; geçici
   okuma/sayfa decode ve uzun anahtarlar hesaba katılmalı. Prototip tek iş parçacıklıdır.
4. SEG04 tasarımı kaynak binding, CRC, bounds, key sıra/UTF-8, kesilme, bozulma,
   cache rebuild, eski cache/kaynak uyumu ve process-kill kapılarıyla tamamlanmalı.
   Disk hataları fallible index API/merge iterator üzerinden yayılmalı.
5. Aynı stage 1 binary'sine karşı metadata/range/text/get/son sayfa, count+payload
   eşitliği ve proses WS/private peak ölçülmeden varsayılan motor değiştirilmemeli.
   Açılış ve ingest peak'i için replay/builder'ın bütçelendirilmesi ayrıca gereklidir.

## Doğrulama ve tekrar

Motorun 44 testi `cargo test --all-targets` ile geçti; deneyin iki ek testi de geçti.
Son ikili sayfa düzeni ayrıca `cargo test --example probe_paged_records` ile denendi.
Unicode, eksik anahtar, sayfa sınırı, tombstone metadata, cache eviction/bütçe,
bozuk sayfa ve truncated dosya hata dönüşü kapsandı. `cargo fmt --check` ve
`cargo clippy --all-targets -- -D warnings` geçti. Kalıcı depolama yolu değişmedi;
üretim kurtarma testleri mevcut test paketinde geçti, deney publication formatı
olmadığından deney için yayınlama/process-kill iddiası yoktur.

```powershell
cargo build --release --example probe_paged_records
target/release/examples/probe_paged_records.exe data/scale-v031-optimized-20261008 output/new-paging-probe
```

Output klasörü yeni olmalı. Beş koşunun özeti
[`paged-records-binary-probe-20261008.summary.json`](benchmarks/paged-records-binary-probe-20261008.summary.json);
ham veriler aynı dizinde `paged-records-binary-probe-20261008-1.jsonl` … `-5.jsonl`.
`tools/summarize-paged-probe.py <dosya-prefix>` bu özeti yeniden üretir.
