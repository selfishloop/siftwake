# Ertelenmiş kaynak kontrolü — ilk segment düzeyi uygulama

## API ve güvence

`Database::open_deferred(path)` opt-in Rust API'dir. Default `open` ve
`open_with_options` tam kaynak doğrulamasını korur. CLI/protokol değişmedi.
Yeni feature veya kalıcı format yok; 0.1 fixture korunur.

| Aşama | Kontrol |
|---|---|
| Ertelenmiş açılış | `.ready` CRC/manifest, immutable source boyut/magic, `.idx` tam CRC ve grammar/kimlik, aktif WAL replay/kurtarma |
| İlk `get` | Sırayla lookup'ta kullanılacak segmentlerin kaynakları tam doğrulanır; sonra index payload CRC/biçim/kimliği kontrol edilir |
| İlk `search` | Tüm immutable kaynaklar doğrulanmadan toplam/sayfa döndürülmez; boş posting/negatif sorgu bunu atlayamaz |
| Yazma/flush/merge | Kaynak doğrulamaları başarılı olmadan değişiklik/yayın yapılmaz |
| `check_integrity()` | Daha önce kontrol edilenler dahil bütün immutable kaynakları ve active WAL'i yeniden kontrol eder |

`pending_source_checks()` doğrulanmamış immutable kaynak sayısını verir.
Açılışın başarılı olması tam bütünlük kontrolünün tamamlandığı anlamına gelmez;
`stats`/count bu kaynaklar için provisional metadata'dır. Kaynak hatasında
get/search/commit/bakım hata döndürür; sağlıklı cache bozuk kaynaktan sonuç
döndürmeye izin vermez. Eksik/bozuk cache, eski kaynak biçimi ve legacy source
tam kontrollü eski açılışa döner. Büyük active WAL veya açılış flush ihtiyacı
ertelenmiş yolun kazancını azaltabilir; bu yol bütün DB'ler için sabit süre değildir.

Doğrulama başarılı olduktan sonra segment oturum içinde yeniden taranmaz;
dışarıdan dosya değiştirilmesi desteklenmez. `check_integrity` sonradan yapılan
source bozulmasını da tekrar tarar. Her read/index payload kontrolü korunur.
Integrity check bozuk/eksik türetilmiş cache'i yeniden oluşturabilir; source'u
onarmaz. Active WAL'in incomplete tail'i explicit check'te reddedilir,
truncate edilmez; normal açılış recovery sözleşmesi aynı kalır.

## İlk sürümün açık sınırı

Bu **belge veya sayfa düzeyinde lazy doğrulama değildir**. SEG04 cache payload'ı
source adresi taşımaz; sağlam index'ten okurken source bozulmasını saklamamak
için ilk kullanımda bütün segment source'u doğrulanır. Açılışta kontrol edilen
index ilk kullanımda yeniden taranmaz; kaynak CRC/replay/grammar ve kayıt sayısı
kontrol edilir.
İlk get/search'ın bedeli gösterilmeden yalnız open süresiyle hız iddiası yapılmaz.

İndeksin tam CRC taraması ve directory hazırlanması açılışta kalır. SQLite'ın
~9 ms bağlantısına eşdeğer açılış hedefinin tamamlandığı söylenmez. Daha küçük
doğrulama blokları/source-offset yerleşimi sonraki sürümlü format çalışmasıdır;
yeni format kararı bu prototiple alınmış değildir.

## Ölçüm yöntemi

Aynı mevcut ilk10.000 belge, seq20,5 segment,boş delta; source267.900.709 B,
index353.242.581 B. Metin264.842.858 B,2.057–51.197 B/belge,8 Gutenberg
eserinden benzersiz fakat kısmen örtüşen pasajlar. Yeni corpus/DB yok;
`data/benchmark-work/fdb` kullanılır. SQLite tekrar ölçülmez.

Tek release executable, experimental-merge-postings + experimental-delta-bitmaps,
diagnostic kapalı. Strict/deferred birer ölçümsüz open; ardından5 taze proses
çifti, sıra dönüşümlü. Sıcak OS cache; eviction yok, cold cache iddiası yok.
`open_ms` sadece API open; stats/log/hash dışarıda. Deferred timer içinde sadece
gerekli kontroller vardır; tamamlanmamış source kontrolleri explicit pending
olarak raporlanır. Eşit tamamlanmış doğrulama işi ölçüldüğü iddia edilmez.

Üç ayrı fresh process ilk get(son kaynakta9999/version20), ilk search(quixote,
total56,50 hits) ve aynı warm search'ı ayrı ölçer; source doğrulama sayıları
önce/sonra raporlanır. Sonra bir fresh process explicit full integrity tarar.
Kaynak oracle ile3.000 get ve dört sorguda120 timed search'ın toplam/sıralı tam
StoredDocument sayfaları ayrıca kontrol edilir; bu normal benchmark değil,
doğruluk smoke kontrolüdür. RAM/disk optimizasyonu, yazma hızı, concurrent yük,
38 senaryoda yeni hız karşılaştırması ve50k/100k ölçülmez.

## Testler

Default ve all-features testleri; CRC-valid frame ama wrong whole-source CRC,
corrupt source + sağlam cache, negatif get/search, commit/flush/merge engelleme,
eksik cache rebuild, explicit integrity recheck, active WAL partial-tail'in
değiştirilmeden reddi ve clean deferred commit/flush kontrolü kapsanır.
Var olan cut/byte-corruption/process-kill ve0.1 fixture testleri çalıştırılır.
Default ve all-features all-targets testleri geçti; son helper değişikliği ve
ek testler ayrıca native_binary testinde default/all-features5/5 geçti.
`cargo fmt --check`, default/all-features
`cargo clippy --all-targets -- -D warnings` geçti.

Araç `tools/measure-deferred-open.ps1`; mevcut raporu ezmez, yalnız sabit DB'yi
kullanır; executable ve bütün `.fdb/.ready/.idx` önce/sonra SHA256 kaydedilir.
Final ham kayıt prefix'i `docs/benchmarks/deferred-open-10k-20261012-final`.
İlk prototipin index'i tekrar açan sonuçları eski prefix'te korunur, final hız
kanıtı değildir. Yeni rapor prefix'i DB kopyası değil küçük JSONL loglarıdır.

## Açık işler

[İş listesi](OPEN-WORK.md): güncel yazma/bakım SQLite karşılaştırması,
arama regresyonları, RAM/disk ve bakım sırasındaki gecikme açık kalır.


## Final sonuç ve karar

Beş çift, aynı executable ve mevcut10k DB:

| Çift | Strict open ms | Deferred open ms |
|---|---:|---:|
| 0 | 762,4586 | 324,7874 |
| 1 | 743,9233 | 319,2395 |
| 2 | 751,7391 | 316,2177 |
| 3 | 745,2441 | 322,9253 |
| 4 | 755,6294 | 307,7177 |
| Medyan | **751,7391** | **319,2395** |

Open aşaması %57,53 azaldı; her çift %56,67–59,28 kazanç verdi.
Bu farklı tamamlanmış iş miktarıdır: deferred açılışta5 pending source vardır.
İlk son-key get bunlardan1'ini doğrular,4 kalır; ilk search kalan4'ü kontrol eder.

| İlk kullanım (üç fresh süreç medyanı) | Strict | Deferred |
|---|---:|---:|
| Get9999, API ms | 0,1510 | 91,9364 |
| İlk quixote search, API ms | 6,7889 | 359,8307 |
| Tekrar quixote search, API ms | 6,2918 | 5,3042 |

**Bu ilk kullanım maliyeti saklanmadı; deferred yolu warm query kazancı olarak
sunulmuyor.** Strict ilk-use control süreçleri sonraki sırada ölçüldü ve bu
süreçlerin açılışı~952 ms medyana çıktı (eşleşmiş open tablosunda~752 ms).
Deferred first-use süreçlerinde open+get medyan401,10 ms, open+get+search760,93 ms;
strict ilk-use süreçlerinde951,77 /959,08 ms. Bu zamansal/cpu/cache değişkenliği
nedeniyle toplam workload için nedensel yüzde kazancı iddia edilmez. SQLite
first-use veya yeni SQL karşılaştırması yapılmadı. Startup-only319 ms,9 ms
SQLite bağlantı hedefine ulaşmış değildir.

Explicit full integrity bir fresh deferred süreçte750,59 ms; sonunda pending0.
Kaynak oracle smoke3.000 get ve120 search'ın tam belge/sıra/version/toplam
kontrolleri geçti. Bütün source/index/ready boyut ve SHA256 değerleri aynı;
son strict-probe kontrolünden sonra da tekrar hash doğrulandı. Yeni corpus,
DB kopyası veya dış proje değişikliği yok. RAM yeniden ölçülmedi.

Karar: API opt-in olarak uygulanmıştır; default strict open korunur. Segment
seviyesinde gecikmiş kaynak doğrulaması başlangıç kullanımını değiştirdiğinden
normal açılışla aynı güvence/zamanlama sözleşmesi sayılmaz. İleride source
adresli per-record/page doğrulaması ve index'in küçük bloklarla kontrolü
sürüm/uyumluluk testiyle tasarlanmalı; sadece index CRC'sini atlamak yeterli
olmaz. [Açık işler](OPEN-WORK.md) aynı kalır.
