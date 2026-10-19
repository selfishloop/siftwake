# RAM metin sıkıştırma ön testi

Motorun RAM yapısını değiştirmeden özgün `Document.text` alanını LZ4 ile
sıkıştırıp tekrar sahipli UTF-8 `String` hazırlamanın maliyetini ölçer.
Bu araç hiçbir veritabanını açmaz, oluşturmaz, sıfırlamaz veya değiştirmez.
Engine Cargo dosyaları/feature'ları ve kalıcı formatlar aynı kalır; LZ4 yalnız
bu bağımsız manifestin bağımlılığıdır.

Yalnız mevcut `data/rkyv-corpus-20261009/json.payload-bench` ilk10k kullanılır.
En fazla200 ayrı kaynak metin tutulur;oracle,input ve sonuç kopyaları da
RAM'de bulunur. Yeni corpus/dosya içeriği üretilmez veya
indirilmez. Sıkıştırılmış metinler diske yazılmaz; her grup sonunda bırakılır.
Çıktıdaki allocation-capacity toplamları gruplar boyunca toplanır; toplam
proses/fiziksel RAM veya aynı anda resident veri boyutu değildir. Ham logdaki
`all_payloads_byte_equal` bu araçta yalnız metin byte eşitliğini ifade eder.

Her belge bağımsız LZ4 block'tur; ortak/eğitilmiş sözlük yoktur. En az1024B
metin denenir;64B'den fazla tasarruf yoksa raw String kalır.2MiB decoded limit,
uzunluk/sınır kontrolü ve UTF-8 doğrulaması vardır. Decoded CRC açılıp ayrıca
ölçülür. CRC modu, decoded metnin eşitliğini korur: LZ4'ün kullanılmayan token
bitindeki değişim aynı doğru metni üretirse kabul edilebilir. Bu bir disk
formatı veya `.fdb` doğrulama mekanizması değildir.

Çalışma dizini repo kökü:

```powershell
cargo test --manifest-path tools/ram_text_probe/Cargo.toml --target-dir target/ram-text-probe
cargo fmt --manifest-path tools/ram_text_probe/Cargo.toml --check
cargo clippy --manifest-path tools/ram_text_probe/Cargo.toml --target-dir target/ram-text-probe --all-targets -- -D warnings
cargo build --release --manifest-path tools/ram_text_probe/Cargo.toml --target-dir target/ram-text-probe
```

Windows runner'ı, Python ile `tools/measure-ram-text.py --prefix
docs/benchmarks/<kullanılmamış-log-prefixi>` şeklinde çalıştırılır. Dört ardışık
süreç, CPU maskesi4 ve iki yönlü mode sırası kullanılır. Mevcut DB/corpus/0.1
fixture SHA değerleri önce/sonra karşılaştırılır; DB reset çağrısı yoktur.
Build çıktıları root `target/` altında kalır; ayrı disk/test DB klasörü yoktur.

1/50/200 metin için yeni tahsis ve caller tamponunu tekrar kullanma ayrı
ölçülür. Her mode/page2 warmup+8 örnek.50'lik sayfalar dört alt grupla bütün
10k'yı kapsar;200'lük sayfalar da bütün10k'yı kapsar. Tek metin, her200'lük
grupta seed ile seçilmiş bir kayıttır. JSON/corpus CRC,girdi kopyası,çıktı Vec
rezervasyonu,oracle ve sonuç bırakma zaman dışında; sonuç String tahsisi,
LZ4 çözme,UTF-8 ve seçildiyse CRC zaman içindedir. Tampon tekrar kullanımında
çıktı allocation'ları korunur. Normalleştirme/contains/posting/fields/title/
links/WAL/bakım veya SQLite ölçümü yoktur. OS cache/allocator sıcaktır;
eviction,güç/arka plan izolasyonu yoktur.

`lz4_flex = 0.14.0`,safe-encode/safe-decode/checked-decode/std açık,frame
kapalıdır. API ve feature açıklaması: [resmi dokümantasyon](https://docs.rs/lz4_flex/latest/lz4_flex/).

20 Ekim sonucu: metin alanı -%34,17, fakat bütün geri-hazırlama durumları
dört süreçte de yavaş. **Ön kapı geçmedi; motora entegrasyon reddedildi.**
[Bütün koşullar ve sayılar](../../docs/RAM-TEXT-LZ4-10K-20261020.md).
