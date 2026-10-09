# Doğrudan kayıt konumundan okuma — 08.10.2026

Planın ilk adımı uygulanıp doğrulandı. Yeni yol `experimental-direct-slot` feature'ı
ve `Database::search_direct_slots` ile denenebilir. **Varsayılan `search` ve CLI
anahtar üzerinden okuma yolunu korur.** Aynı proses içinde alan filtresi/ikinci
sayfa için yaklaşık %10 kazanç sinyali var; bütün iş yükleri için hız kabulü
tamamlanmadığından varsayılan davranış değiştirilmedi. Toplam RAM veya SQLite
üstünlüğü iddiası yoktur; kayıt/ID tabloları hâlâ RAM'dedir.

## Değişiklik

Sorgunun run/slot bilgisi payload okumaya kadar korunur. Deneysel yolda disk
sonucu seçilen run'ın `read_selected_document(slot)` metodundan, delta sonucu
RAM `docs` tablosundan alınır. Tekrar global `get(key)` ile run/key araması yapılmaz.
Yeni kayıtlar ve silmeler sorgunun mevcut görünürlük maskeleriyle çözümlenir.
Slot bounds, tombstone kontrolü, payload CRC ve metadata ile anahtar/sürüm
eşitliği korunur. Tombstone kontrolü seçilmiş slot girişindedir; ortak metin
aday doğrulama döngüsüne ilave kontrol eklenmez. API'nin `get(key)` yolu bağımsızdır.

Kaynak/cache biçimleri, kalıcı yazma ve kurtarma kodu değişmedi. `.fdb`/`.ready`
kaynak, `.idx` türetilmiş cache olmaya devam eder. 0.1 fixture'ları değiştirilmedi;
DB/arama bağımlılığı eklenmedi, dış projelere bağlantı veya entegrasyon yapılmadı.

## Ölçüm

Aynı bir milyonluk healthy corpus: `data/scale-v031-optimized-20261008`, 15 aktif
run, sequence 2002, 999.951 görünür belge; tekrarlı 256 bayt içerik, güncellemeler,
silmeler ve sentinel. Sayfa limit 50. Alan filtresi `source=synthetic` total 999.950;
range `day=2..5` total 133.336; dar başlık `rapor-0000500001` total 1; geniş metin
`teknoloji` total 999.900; ikinci sayfa offset 50, son sayfa offset 999900.

İlk binary karşılaştırmaları beşer bağımsız proses çiftiyle, aynı DB üzerinde
sırayla ve değişen motor sırasıyla yapıldı. İkinci seride geniş metin sorgusu
çıkarıldı. Tam JSON sonuç, sequence, total, sıra, sürüm ve payload eşitliği her
istekte kontrol edildi. Bu iki seride bazı sorgularda gerileme ve yüksek koşu
varyansı görüldü; hız kabulünü sağlamış sayılmadı. İlk serinin ilk kısa sorguları
release build'in bitişiyle örtüştü; bu da kabul kanıtı olarak kullanılmamasının
ek nedenidir. Uygulama kesintisinde tamamlanmış 0..3 koşuları korundu; yarım 4.
koşu ayrı dosyada saklanıp tamamen tekrar edildi (`--start-run 4`).

Son deney `compare_slot_hydration`: **bir proses, tek açık DB, beş tur**. İki okuma
yolu her istekte dönüşümlü çalışır; başlangıç sırası da değişir. Kaynak doğrulaması
normal `Database::open` ile yapılır, zamanlamaya dahil değildir. Her sorgu için
iki yol önceden ısıtılır. Alan/range/dar başlık/ikinci sayfada yol başına turda 30,
son sayfada 3, geniş metinde 1 ölçüm vardır. JSON eşitlik kontrolü zamanlama dışında;
engine çağrısı ölçülür, JSON serialization ve stdio dahil değildir. Allocation
instrumentation yoktur. Bu beş tur bağımsız proses tekrarı gibi sunulmaz.

| Sorgu | Anahtar yolu medyanı | Slot yolu medyanı | Eşlenmiş tur oranının medyanı |
| --- | ---: | ---: | ---: |
| Alan filtresi, ilk sayfa | 0,5413 ms | 0,4624 ms | 0,897 |
| İkinci sayfa | 0,5407 ms | 0,4854 ms | 0,899 |
| Sayı aralığı | 3,1425 ms | 3,1618 ms | 1,006 |
| Dar başlık | 10,2255 ms | 10,5618 ms | 1,007 |
| Son sayfa | 223,9895 ms | 223,1337 ms | 0,996 |
| Geniş metin | 4552,8903 ms | 4378,6120 ms | 0,983 |

İlk iki sütun tur p50'lerinin medyanı, son sütun her turdaki slot/key p50
oranlarının medyanıdır; aynı istatistik değildir. 1'in altı slot yolu lehine.
Alan filtresi ve ikinci sayfanın beş turunun tamamında p50 daha düşüktü. Diğer
sorgularda küçük farklar veya iki yönde değişim var; iyileşme/gerilemesizlik
garantisi değildir. Geniş metinde yalnız toplam beş örnek/yol bulunduğundan
tur p95 etiketi anlamlı bir kuyruk gecikmesi ölçümü değildir. Diğer sorguların
tur p95 ve en kötü tur değerleri özet dosyasında bulunur.

Cache sıcak OS cache'idir; proses restart soğuk disk testi sayılmaz. Soğuk cache,
uzun/tekrarsız içerik, rastgele `get`, ingest/maintenance, proses WS/private bytes
bu adımda ölçülmedi. Bellek bütçesi düşürülmedi. Son sayfanın tüm ID listesi ve
anahtar birleştirme maliyeti devam eder. Varsayılan yola geçiş ancak sonraki sayfalı
metadata/gruplu okuma uygulamasının tam kabul matrisiyle yapılır.

## Kontroller

- `cargo test --all-targets`: 44 motor + 2 prototip testi geçti.
- `cargo test --all-targets --all-features`: aynı 46 test geçti; deneysel yol
  çoklu run/delta, güncelleme/silme, sıralama, reopen, offset sınırı/taşması ve
  payload bozulması kontrollerine dahil edildi.
- `cargo fmt --check`, default ve all-features Clippy `-D warnings` geçti.
- Mevcut kesilme/bozulma/process-kill ve 0.1 kaynak uyumu testleri geçti.
- Bir milyonluk corpus'taki tüm karşılaştırmalarda sonuç eşitliği korundu.

```powershell
cargo build --release --example compare_slot_hydration --features experimental-direct-slot
target/release/examples/compare_slot_hydration.exe data/scale-v031-optimized-20261008
```

Ham/özet dosyaları:

- [Aynı proses, dönüşümlü karşılaştırma](benchmarks/fdb-direct-slot-alternating-20261008.jsonl)
  ve [özet](benchmarks/fdb-direct-slot-alternating-20261008.summary.json).
- [İlk binary karşılaştırması](benchmarks/fdb-direct-slot-reads-20261008.jsonl)
  ve [özet](benchmarks/fdb-direct-slot-reads-20261008.summary.json).
- [Kısa sorgu tekrarı](benchmarks/fdb-direct-slot-short-reads-20261008.jsonl)
  ve [özet](benchmarks/fdb-direct-slot-short-reads-20261008.summary.json).

İlk candidate SHA256: `613FBFC0503DD64864AD63A08A389FB7D4A0D6488EB3BAD977C219BB6D349AED`.
Baseline: `97A92D73691421615DDEB963742AEBFB913DD26B2973A21CCE55D79F3CFE8511`.
Dönüşümlü ölçüm binary'si: `60B298B2129FCA03FACB4943F8D3D0E46F87E5DFC21D983A24C6AD92E6127A60`.
İlk candidate artık default binary değildir. Default binary yeniden derlendi.

Sonraki adım: seçilen slot'ların metadata okumalarını sayfa başına gruplamak,
scope/collection tekrarlarını azaltmak ve iki seviyeli disk diziniyle fence
belleğinin kayıt sayısıyla doğrusal büyümesini kaldırmak. Bu yol için temel okuma
API'si artık mevcut; SEG04 veya proses RAM hedefi tamamlanmış sayılmaz.
