# Aynı 10 bin belge: FDB / SQLite okuma ve contains

## Sonuç

Üç ayrı prosesin p50/p95 değerlerinin medyanı, milisaniye:

| İşlem | Eşleşme | FDB p50 | SQLite p50 | FDB p95 | SQLite p95 |
|---|---:|---:|---:|---:|---:|
| Tek belge get | — | 0,0388 | 0,0835 | **0,0743** | 0,1731 |
| Yaygın `the` | 10.000 | 732,69 | 1.002,79 | **773,21** | 1.345,56 |
| Dar `quixote` | 56 | 8,99 | 13,32 | **10,63** | 19,70 |
| İki karakter `he` | 10.000 | 708,46 | 974,56 | **770,80** | 1.343,26 |
| AND `the man` | 9.932 | 713,92 | 1.053,14 | **770,70** | 1.421,32 |

Bu düzen ve dört sorguda FDB p95 gecikmesi SQLite'a göre %42,5–46,0 düşük;
get p95 %57,1 düşük. Üç numaralı eşleşmenin her birinde dört sorgu da FDB
lehine. Bu farklı SQL/schema/cache ayarlarına veya genel arama üstünlüğüne
genellenmez. Geniş sorguların 770 ms civarında olması hâlâ ciddi sorun;
dar sorgunun iyi sonucu geniş sorgu hedefini karşılamaz. 10k dar sorguda
30 ms'nin altında olmak 50k/100k hedefinin geçtiğini kanıtlamaz.

Koşular arasında oynama var: FDB dar p95 7,94 / 11,91 / 10,63 ms;
SQLite 19,70 / 19,88 / 14,58 ms. FDB yaygın p95 581,26 / 830,41 / 773,21 ms;
SQLite 1.345,56 / 1.455,75 / 1.042,66 ms. İlk koşu tek başına sonuç seçilmedi.

Açılış medyanı FDB **936,40 ms**, SQLite bağlantı/pragma **9,85 ms**;
SQLite burada tam kaynak bütünlüğü doğrulamıyor. Bu iki sayı aynı açılış
sözleşmesini ölçmüyor; FDB'nin yaklaşık bir saniyelik açılış maliyeti mevcut.
RAM ve disk optimizasyonu bu görevde yapılmadı, RAM tekrar ölçülmedi.

Altı koşuda tüm tam toplamlar, ilk 50 StoredDocument/versiyon ve 100 farklı
get anahtarı kaynakla eşleşti; toplam 18.000 zamanlı get ve 720 zamanlı sorgu
doğrulandı (ısıtma hariç). Tüm `.fdb/.ready/.idx` ve `sqlite.db` boyut/hash'leri
önce/sonra aynı. Kaynak/indeks oluşturma veya bakım yapılmadı.

Yeni ölçüm aracı testleri FDB'de 3/3, izole SQLite crate'te 3/3 geçti
(CRC referans vektörü, Türkçe oracle, percentile hesabı). Her iki crate'te
`cargo fmt --check` ve `cargo clippy --all-targets -- -D warnings` geçti.
Motor depolama/kurtarma kodu değişmedi; bu görev için süreç sonlandırma veya
bozulma testi tekrar çalıştırılmadı. 0.1 fixture değiştirilmedi.

Feature'lar default kapalı kalır. Öncelikli sonraki araştırma geniş sorguda
tam toplam/aday doğrulaması maliyetidir; bunu alt süreler ölçülmeden yalnız
normalleştirmeye veya payload çözümüne bağlamak doğru olmaz.

## Ölçüm kapsamı

Yeni corpus veya veritabanı üretilmedi; indeks yeniden kurulmadı. Mevcut
`data/benchmark-work/fdb` ve `data/benchmark-work/sqlite/sqlite.db` kullanıldı.
FDB son flush/bitmap adayının oluşturduğu 5 segmentli, delta'sı boş veritabanı;
SQLite optimize/checkpoint edilmiş FTS5 trigram external-content veritabanı.
FDB release feature'ları `experimental-merge-postings experimental-delta-bitmaps`;
diagnostic instrumentation yok, default feature kararı değişmedi. Bu feature'ların
yazma kazancı ölçüldü; bu rapor boş delta üzerindeki mevcut okuma yolunu ölçer.

Kaynak mevcut `data/rkyv-corpus-20261009/json.payload-bench` ilk 10.000 kaydı:
264.842.858 bayt metin, belge başına 2.057–51.197 bayt. Sekiz kitabın benzersiz,
kısmen örtüşen pasajları; Türkçe veya bağımsız 10 bin eser corpus'u değildir.
Kaynak CRC'leri doğrulandı. Bağımsız Unicode/Türkçe katlama + alt-dize taraması
oracle'ı her proses için ölçüm dışında kuruldu; yalnız ilk 50 eşleşme ve dağıtılmış
100 get örneği tutuldu. Tüm corpus RAM'e alınmadı.

## Yöntem

- Sıra FDB0 → SQLite0 → SQLite1 → FDB1 → FDB2 → SQLite2; motorlar eşzamanlı çalışmadı.
- OS cache boşaltılmadı. Başlangıç dosya hash kontrolü ve oracle okumaları da cache'i
  etkiler. Sonuçlar **sıcak OS cache**, sorgu/anahtar başına bir tam ölçümsüz ısıtma
  sonrasıdır; SQLite'ın 2 MiB sayfa cache'i tüm veritabanını içermez.
- Her proseste sorgu başına 30 tekrar; sorgu sırası her tur döndürülür. Get için
  ilk/son dahil 100 dağıtılmış anahtar × 30 tur = 3.000 tek belge okuması.
- p50/p95 nearest-rank; rapor üç prosesin p50/p95 değerlerinin medyanıdır,
  birleştirilmiş 90 örneğin p95'i değildir. Ham örnekler JSONL dosyalarındadır.
- Contains: `scope=shared`, `collection=texts`, offset=0, limit=50, tam toplam,
  anahtar ID sırası, tüm terimler AND ve alt-dize. Full StoredDocument/versiyon
  dönüşümü ve doğrulama/CRC dahil mevcut motor yolu ölçülür; oracle eşitlik
  kontrolleri süre dışında. FDB sequence=20 kontrolü ince wrapper içinde yapılır.
- SQL: `LIKE '%term%'` terim başına AND, FTS5 trigram üzerinden count + sıralı
  ilk 50 seçimi, JSON payload çözümü, tek read transaction. Statement hazırlama
  ve sorgu oluşturma süreye dahil; FDB Query oluşturma da dahil. LIKE jokeri
  içermeyen sabit dört sorgu kullanılır.
- SQLite read-only açılır; `cache_size=-2048`, `mmap_size=0`, `temp_store=FILE`.
  Açılış sadece bağlantı/pragma süresidir; FDB açılışı kaynak bütünlüğünü de
  doğrular. Açılış rakamları eşdeğer doğrulama sözleşmesi sayılmaz.
- `.fdb/.ready/.idx` ve `sqlite.db` SHA256 + boyutları önce/sonra karşılaştırılır.
  Kilit ve SQLite shm dosyaları kalıcı içerik karşılaştırmasına dahil değildir.

Bu dar ölçüm 30 kullanıcı tipi sorgu kapısı, 50k/100k ölçek, soğuk cache, Türkçe
corpus, yazarken arama, güncelleme görünürlüğü, derin sayfalama veya RAM ölçümü
değildir. İki karakter sorgusu mevcut API'nin tarama davranışını gösterir;
Frunner gereksinimindeki TERM_TOO_SHORT sözleşmesi burada uygulanmadı.
Normalleştirme/tarama/CRC alt süreleri ayrı profillenmedi; rakamlar uçtan uca
gecikmelerdir. Kalıcı format, runtime API veya dış proje değişikliği yok.

## Tekrarlama ve kanıt

`cargo build --release --example measure_content_reads --features
"experimental-merge-postings experimental-delta-bitmaps"`
ve `cargo build --release --manifest-path tools/native_sqlite/Cargo.toml
--bin content_reads` sonrasında `tools/measure-content-reads.ps1` çalıştırılır.
Bu script reset/ingest/maintenance çağırmaz ve mevcut DB'ler yoksa başarısız olur.
`tools/summarize-content-reads.py` yalnız rapor JSONL'lerini özetler.

Ham kayıt prefix'i `benchmarks/content-reads-10k-20261012`; executable hash'leri,
önce/sonra içerik hash'leri, altı JSONL ve summary JSON bu prefix altındadır.
