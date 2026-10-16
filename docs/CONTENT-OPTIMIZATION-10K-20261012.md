# 10 bin uzun belgede ilk hedefli optimizasyon

## Karar ve uygulanan değişiklik

Default yolun `TXN2` çerçeve tamponu artık payload uzunluğu önceden
kontrol edilerek header + payload + footer için tek seferde ayrılır.
Geometrik büyüme ve footer eklenirken tüm çerçevenin yeniden kopyalanması
önlenir. İki eşleşmiş koşuda tepe private yaklaşık %4,4 azaldı;
belirgin hız kazancı görülmedi. Working set kazancı da gösterilmedi.
Kalıcı bayt biçimi, fsync politikası, manifest, index directory ve API değişmedi.

Mevcut `experimental-delta-bitmaps` yolu ayrıca ölçüldü. Yeni feature
eklenmedi, bitmap default'a alınmadı. İndeksleme belleği ve süresi iyileşiyor;
boşta private hafif arttı, dar sorgu p95 kazancı gösterilmedi ve genel içerik
indeksi kabul hedefleri karşılanmadı. Bu sonuç bütün FDB'nin veya SQLite'a
karşı arama performansının kabul edildiği anlamına gelmez.

## Corpus ve yöntem

Performans geliştirme koşuları kullanıcı kararıyla **en fazla 10 bin belge**.
WAL/source/index için yeni format, entegrasyon veya veri taşıma yok.
İlk 10.000 kayıt `data/rkyv-corpus-20261009/json.payload-bench` dosyasından
CRC kontrol edilerek, 500 belgelik batch'lerle okunur. Metin toplamı
264.842.858 bayt; belge metinleri 2.057–51.197 bayt. Sekiz Gutenberg kitabının
çeşitli, benzersiz fakat kısmen örtüşen pencereleri; 10 bin bağımsız eser
veya Türkçe uzun metin corpus'u değildir. Kitaplar/full oracle RAM'e yüklenmez.

Üç FDB binary'si aynı kaynak kodundan farklı aşamalarda donduruldu:
eski kontrol, yalnız tampon düzeltmesi, bitmap + tampon düzeltmesi.
Sıra kontrol → tampon → bitmap ve bitmap → tampon → kontrol;
her ingest ayrı boş veritabanında. Arada tampon sürümüyle SQLite tekrar ölçüldü.
İlk turun her FDB varyantında üç ayrı boşta açılış prosesi vardır.
Derleme/test işleri süre ölçümleriyle örtüşmez. OS cache tahliye edilmez;
soğuk cache iddiası yok. SQL karşılaştırıcısı SQLite 3.53.2, WAL/FULL,
2 MiB cache, mmap=0, JSON + folded text + FTS5 trigram kullanır.
FDB fields/links/guard indeksleriyle SQLite şeması tam özellik eşdeğeri değildir.

Windows private ve working set 20 ms aralıkla örneklenir, faz adı 100 ms'de
okunur. Kısa pikler/faz sınırları kaçabilir. Tablodaki tepe, indeksleme ve
son bakım pencerelerinin maksimumudur; boşta ölçüm açılıştan 5 saniye sonra,
arama yapılmadan alınır. OS dosya cache'i private sayacına dahil değildir.
Süre/tepe değerleri iki ingest medyanı, boşta değerleri üç proses medyanıdır.
SQLite'ın burada tek yeni ingest koşusu vardır.

## Ölçülen sonuç

| Ölçüm | Eski FDB kontrolü | Default tampon düzeltmesi | Bitmap + tampon, opt-in | SQLite |
|---|---:|---:|---:|---:|
| İndeksleme | 60,97 s | 60,82 s | 49,70 s | 40,97 s |
| Son bakım | 14,20 s | 14,10 s | 13,96 s | 1,51 s |
| Yazma + bakım | 75,17 s | 74,92 s | 63,66 s | 42,48 s |
| Örneklenen tepe private | 188,16 MiB | 179,82 MiB | 138,94 MiB | 8,55 MiB |
| Örneklenen tepe working set | 180,71 MiB | 180,94 MiB | 141,93 MiB | 13,50 MiB |
| Boşta private | 21,43 MiB | 21,47 MiB | 21,69 MiB | 1,31 MiB |

Bitmap birleşimi kontrolün iki karşılıklı koşusunda da daha hızlı ve düşük
tepe private verdi: toplam süre %15,3, indeksleme süresi %18,5,
tepe private %26,2 azaldı. İki gözlem daha geniş corpus/cihaz güven aralığı değildir.
Tampon düzeltmesinin working set medyanı %0,13 arttı; burada working set
iyileşmesi iddia edilmez. Bitmap boşta private medyanı yaklaşık 0,26 MiB arttı.

FDB'nin tüm altı koşusunda disk boyutları/posting sayısı/kayıt sayısı aynı:
10.000 belge, 19.221.340 posting, 5 segment, sequence 20, resident 0.
Canlı kaynak + indeks + manifest **621.143.804 B**, tutulan eski nesillerle
**1.513.745.887 B**. SQLite **583.860.224 B**. Canlı FDB/SQLite 1,064;
eski nesiller dahil 2,593. Bu değişiklik disk kazancı sağlamadı.
Geçici dosyaların disk pikleri ölçülmedi.

## Delta araması ve doğruluk

Arama ayrı bir probe'da corpus'un ilk **1.000** uzun belgesiyle ölçüldü:
26.743.203 B metin; delta eşiği 128 MiB/2.000 key, flush/merge yapılmaz.
İki binary'de de tampon düzeltmesi var; yalnız posting temsili değişir.
Anahtarlar ters sırada eklenir. İlk 40 belge silinip `İSTANBUL updatedunique`
metniyle tekrar eklenir, ardından motor yeniden açılır. Her durumun altı
sorgusunda total, anahtar sırası, ilk 50 tam belge ve version referans taramayla
kontrol edilir. Oracle/folded belgeler bu probe'da RAM'dedir; bu prosesin
belleği 10k indeksleme belleğiyle karşılaştırılmaz.

Sıra keys → bitmap / bitmap → keys; her sorgu/durum/proseste bir ısınma,
10 ölçüm; tablo ilk delta durumunda iki prosesin toplam 20 örneğidir.
İki karakter mevcut motor API'sinde desteklenir; Frunner'ın TERM_TOO_SHORT
sözleşmesinin uygulanması değildir. SQLite arama kıyası bu probe'da yoktur.

| Sorgu | Eşleşme | Arc/key p50 / p95 ms | Bitmap p50 / p95 ms |
|---|---:|---:|---:|
| Yaygın `the` | 1.000 | 0,5609 / 0,7451 | 0,1649 / 0,2526 |
| Dar `quixote` | 7 | 0,0323 / 0,0507 | 0,0237 / 0,0537 |
| İki karakter `he` | 1.000 | 0,3720 / 0,4654 | 0,1587 / 0,2693 |
| AND `the man` | 993 | 1,0736 / 1,6478 | 0,2736 / 0,4978 |

Dar sorgu p50 iyileşse de p95 yaklaşık %5,9 arttı. Sıfır eşleşmeli ve
silme/yeniden ekleme/reopen durumları ham özette saklıdır. 10k/30 sorgulu
narrow p95, disk+delta karışımı, update visibility ve eşzamanlı okuyucular
burada ölçülmedi; bu küçük örnek üretim p95 garantisi değildir.

## Tahsis profili ve doğrulama

İlk 10k diagnostic probe'unda bağımsız origin pikleri: delta_index
67.996.450 B; frame_encode 49.234.688 B; source_batch 23.452.506 B;
caller_documents 43.060.207 B. Bu pikler aynı anda olmak zorunda değildir,
**toplanmaz**. Sayaçlar istenen heap tahsislerini takip eder; allocator
başlığı/ölçüm hook'u ve realloc sırasında kopyalama normal allocator'dan
farklıdır. Bunlar RSS/private veya normal hız ölçümü değildir.
Bakım sonu packed_metadata canlı 13.758.023 B; bu da toplam proses RAM'i değildir.

`cargo test --all-targets`: 63 başarılı; `--all-features`: 115 başarılı.
Kesilme, her bayt bozulması, process kill, eski fixture, source bozukken
sağlam cache'in reddi ve silme/merge doğruluğu mevcut testlerle geçti.
`cargo fmt --check`, default ve all-features clippy `-D warnings` geçti.
`tests/engine.rs` ve fixture'lar değişmedi. Her 10k koşusunda tüm input
CRC'leri ve 40 tam belge örneği doğrulandı; SQLite integrity ayrıca süre
ölçümü dışında çalıştırıldı. Dondurulmuş binary SHA-256'ları kontrol edilir.

## Sonraki hedef

10k sınırında devam: yaklaşık 14 saniyelik bakımda tekrar payload çözme,
fold/gram üretimi ile posting merge maliyetlerini ayırmak; mevcut posting
reuse yolunu izole değerlendirmek. Boşta bellekte packed metadata ana hedeftir.
60 MiB indexing / 15 MiB idle / 0,5 s open ve disk hedefleri bu adımda
geçilmedi. Kaynak+indeks metin kopyası, derived profil ve DLL henüz değiştirilmedi.

Ham kanıt: `docs/benchmarks/content-optimization-20261012-summary.json`,
`content-optimization-20261012-executables.json`, test/clippy logları,
`content-memory-{control,reserve,bitmap-reserve}-10k-20261012-{0,1}-*`,
`delta-queries-{keys,bitmap}-20261012-{0,1}.jsonl`.
Özet: `tools/summarize-content-optimization.py`; SQLite eşleşmiş özet:
`tools/summarize-content-memory.py docs/benchmarks/content-memory-reserve-10k-20261012-0`.
