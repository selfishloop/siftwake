# Gruplu payload okuması — 08.10.2026

Yakın kayıtların payload okumalarını birleştiren yol uygulandı. Yeni Cargo feature
`experimental-grouped-read`, `experimental-direct-slot` feature'ını içerir;
Rust API `Database::search_grouped_slots` üzerinden denenir. Default `search` ve
JSON Lines CLI mevcut anahtar yolunu korur. **Kayıt/ID tablosu hâlâ RAM'dedir.**
Bu aşama SEG04 veya SQLite'tan düşük toplam proses belleği hedefini tamamlamaz.

## Uygulama

Sorgu önce görünür küresel sayfanın run/slot seçimlerini toplar. Seçimler run'a
göre ayrılır; yakın payload'lar en fazla 64 KiB bir okuma penceresinde alınır.
Aradaki en büyük boşluk 4096 bayttır. Scratch Vec tek sorgunun bütün run'larında
yeniden kullanılır ve büyürken `reserve_exact` uygulanır. 64 KiB'den büyük belge
mevcut tekil slot okumasına döner; bu belgenin allocation'ı pencere bütçesi değildir.

Her seçilmiş belge kendi payload CRC, uzunluk ve key/version identity kontrolünden
geçer. Slot sırası/bounds ve tombstone kontrolleri vardır. Penceredeki seçilmemiş
belgeler parse edilmez. Sonuçlar saklanan küresel sayfa pozisyonuyla sıralanır;
RAM delta sonuçları aynı sırada birleştirilir. Okuma hatasında bütün sorgu hata
döndürür, kısmi sayfa kullanıcıya yayımlanmaz.

İlk koşulsuz prototip son sayfada gerileme gösterdi. Bu nedenle gruplama yalnız
mevcut bounded-prefix yolu, `offset+limit <= 4096`, için çalışır. Büyük offset
ve checked-addition taşması `search_direct_slots` yoluna döner. Tam ID listesi
ve bütün anahtarları birleştiren derin sayfa maliyeti henüz çözülmedi.

Bu 64 KiB sınırı payload pencere buffer'ıdır: sonuç belgeleri, seçim listeleri,
görünürlük maskeleri, büyük tekil payload ve toplam proses RAM'i dahil değildir.
Her eşzamanlı sorgunun kendi buffer'ı vardır; genel cache/worker bellek kotası
henüz uygulanmadı. Okumalar değişken boyutlu payload aralıklarında birleştirilir;
bu aşamada metadata sayfaları veya yeni kalıcı format yoktur. `.fdb`/`.ready`
kaynak, `.idx` türetilmiş cache olarak kalır; kaynak doğrulaması değiştirilmedi.
0.1 fixture'ları, dış projeler ve servisler değiştirilmedi; DB bağımlılığı eklenmedi.

## Performans

Corpus `data/scale-v031-optimized-20261008`: bir milyon başlangıç kaydı, tekrarlı
256 bayt içerik, güncelleme/silmeler ve sentinel; 15 run, sequence 2002 ve
999.951 görünür belge. Sorgu limiti 50; `source=synthetic` total 999.950,
`day=2..5` range total 133.336, başlık `rapor-0000500001` total 1, geniş metin
`teknoloji` total 999.900. İkinci sayfa offset 50, son sayfa offset 999900.

Karşılaştırma **tek proses/tek açık DB, altı tur**: key, tekil slot ve gruplu
okuma. İstek sırası dönüşümlü döndürülür/ters çevrilir. Her tur/sorgu öncesi
üç yol ısıtılır. Alan/range/dar başlık/ikinci sayfa yol başına 30 örnek/tur,
son sayfa 3, ilk geniş metin serisi 1 örnek/tur kullanır. Her isteğin tüm JSON
sonucu zamanlama dışında karşılaştırılır; sıra, total, sequence, sürüm ve payload
eşitliği korunmuştur. Engine çağrısı ölçülür; stdio ve JSON serialization hariçtir.
CPU benchmark'ında allocation sayaçları yoktur. OS cache sıcak; soğuk disk testi
yapılmadı. Altı tur bağımsız proses tekrarı değildir.

İlk koşulsuz seride gruplu yolun son sayfa eşlenmiş p50 oranı key'e göre 1,122,
tekil slot'a göre 1,069 idi; hız kabulü olarak kullanılmadı. Sınırlı gruplama ve
derin-offset fallback eklendikten sonra geniş metin hariç bütün kısa sorgular ve
son sayfa altı turla tekrar ölçüldü. Son kod için aşağıdaki tablo kullanılır.

| Sorgu | Key medyan p50 | Gruplu API medyan p50 | Gruplu/key eşlenmiş oran |
| --- | ---: | ---: | ---: |
| Alan filtresi, ilk sayfa | 0,5633 ms | 0,3716 ms | 0,688 |
| İkinci sayfa | 0,68165 ms | 0,4623 ms | 0,686 |
| Sayı aralığı | 3,7202 ms | 3,54835 ms | 0,957 |
| Dar başlık | 12,7496 ms | 12,7487 ms | 1,006 |
| Son sayfa, direct fallback | 299,1264 ms | 305,5593 ms | 1,036 |

İlk iki sütun tur p50'lerinin medyanı; son sütun tur içindeki p50 oranlarının
medyanıdır. Alan filtresi ve ikinci sayfanın altı turunun tamamında gruplu p50
daha düşüktü. Tekil slot'a göre eşlenmiş kazanç sırasıyla yaklaşık %32 ve %22.
İlk/ikinci sayfanın tur p95 medyanları key 0,98075/1,00735 ms, gruplu API
0,80645/0,7739 ms oldu. Bütün tur p95/worst değerleri özet dosyasındadır.

Son sayfanın gruplu API'si artık tekil slot kodunu çağırır. Tekrardaki eşlenmiş
oran direct'e göre 1,016, key'e göre 1,036; bu rakamlar gerilemesizlik garantisi
olarak sunulmaz. Default yol korunur. Koşulsuz ilk serinin geniş metin oranı
key'e göre 0,993 idi; altı örnek/yol kuyruk gecikmesi veya genel hız kazancı
kanıtı değildir. Son gate geniş metnin offset=0 yolunu değiştirmez, ancak son
kod için geniş metin tekrar ölçülmedi. Soğuk cache, farklı corpus, ingest/bakım,
random get, eşzamanlı okuyucular ve proses WS/private ölçülmedi.

## Geçici allocation profili

Son kod ayrı `profile_grouped_reads` binary'sinde opt-in Rust allocator sayacıyla
ölçüldü. Bu sayaç istenen allocation boyutlarıdır; allocator metadata/fragmentation,
stack, OS cache ve RSS dahil değildir. Instrumented süreler hız ölçümü değildir.
Ek peak, sorgudan hemen önceki requested live değerine göre hesaplanır; dönen
sayfa dahildir, JSON doğrulama/çıktı serialization'ı snapshot sonrasındadır.

| Sorgu | Key/slot ek requested peak | Gruplu ek requested peak | Allocation çağrıları key → gruplu |
| --- | ---: | ---: | ---: |
| Alan filtresi | 64.154 bayt | 84.901 bayt | 961 → 920 |
| Sayı aralığı | 64.129 bayt | 64.744 bayt | 1055 → 1014 |

İlk sayfada ek maliyet 20.747 bayt, yaklaşık 20,3 KiB'dir. Bellek azalması
iddiası yoktur. Gruplama tekrar tekrar payload buffer ayırmayı azaltır; kayıt/ID
tablosunun yaklaşık 47,69 MiB kapasitesini kaldıracak sonraki disk dizini
çalışmasının yerine geçmez. Küçük fixture'da üç yakın seçim için tek okuma
isteği doğrulandı; bu sayaç kernel syscall veya fiziksel disk I/O sayısı değildir.

## Doğrulama

- Default `cargo test --all-targets`: 46 test geçti.
- `cargo test --all-targets --all-features`: 51 test geçti, son gate sonrası tekrar edildi.
- `cargo fmt --check` ve default/all-features Clippy `-D warnings` geçti.
- Yeni testler: yakın/gapped slot'ların tek penceresi, pencere kapasitesi,
  oversized fallback, CRC bozulması, geçerli CRC ile yanlış key/version,
  tombstone/boş/tekrarlı/sırasız/geçersiz slot ve kısa dosya okuması.
- Entegrasyon: üç çakışan run, RAM delta update/delete, 1.000 kayıtlık sayfa,
  büyük belge, range, empty/delta-only, reopen ve offset/taşma eşitliği.
- Mevcut source-corruption/cache, frame kesilme, process-kill, guard/atomiklik
  ve 0.1 kaynak uyumu testleri geçti. Kalıcı format değişmedi.

Tekrar:

```powershell
cargo build --release --example compare_slot_hydration --example profile_grouped_reads --features experimental-grouped-read
target/release/examples/compare_slot_hydration.exe data/scale-v031-optimized-20261008 --grouped --skip-text
target/release/examples/profile_grouped_reads.exe data/scale-v031-optimized-20261008
```

- Son gate [ham veri](benchmarks/fdb-grouped-short-gated-20261008.jsonl),
  [özet](benchmarks/fdb-grouped-short-gated-20261008.summary.json) ve
  [requested heap profili](benchmarks/fdb-grouped-gated-heap-20261008.jsonl).
- Önceki koşulsuz [ham veri](benchmarks/fdb-grouped-reads-20261008.jsonl) ve
  [özet](benchmarks/fdb-grouped-reads-20261008.summary.json) ayrı korunur.
- `tools/summarize-grouped-hydration.py` eksik tur/sorgu setini reddederek özet üretir.

Son ölçüm binary SHA256:

- Compare: `0B44DE225FD181DF54193AB7803A5C08A12DA7D6A6B5C983A7E801A3519C84A6`.
- Heap profile: `515E2BB401391466E6E8DAD296AA9678A54145303E30C7D2DD1E386703610807`.
- Yeniden derlenen default CLI: `41EE5A5A01CBB44EC1E7725E89965DDC8238B8726C83D068414F172EAFBA6643`.

Sonraki esas RAM adımı, metadata/ID tablolarını sayfalı türetilmiş disk dizinine
taşımaktır: slot erişimi için bu batch yolu; key araması için sıkıştırılmış
anahtar blokları ve iki seviyeli dizin; ortak sınırlı cache; fallible I/O
arayüzleri gerekir. Builder/replay ve token sözlüğü ayrıca bütçelenecektir.
