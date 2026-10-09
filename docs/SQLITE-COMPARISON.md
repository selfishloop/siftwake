# SQLite karşılaştırması — bir milyon kayıt

Bu belge 0.3 başlangıç ölçümünü korur. Güncel uygulama ve yeniden ölçüm:
[0.3.1 doğrulaması](VALIDATION-0.3.1.md).

Kullanıcının RAM karşılaştırması isteği için SQLite **ayrı benchmark prosesinde**
kullanıldı. FDB motoruna SQLite bağımlılığı eklenmedi; Rust kaynakları,
kalıcı formatlar ve dış projeler değiştirilmedi.

Windows 11 / Ryzen 7 7840HS üzerinde, önceki FDB 0.3 ölçümüyle aynı makinede
SQLite 3.53.1, Python 3.12.14 standart sqlite3 modülü kullanıldı. SQLite çalışması
sırasında FDB benchmark'ı çalıştırılmadı. FDB'nin önceki ölçümü karşılaştırma
referansıdır; iki motor aynı anda ölçülmedi.

## Aynı iş yükü

1.000.000 kayıt, shared/contents, 10 haneli ID, benzersiz rapor başlığı,
kayıt başına aynı 256 bayt Türkçe metin, source=synthetic, day=id%30,
500 kayıtlık 2.000 kalıcı commit. Başlık ve orijinal içerik/metadata/links/sürüm
JSON payload içinde saklandı. İndeks kurulumu bütün yazmalar sırasında yapıldı.
Sorgular toplam eşleşme sayısı ve anahtar sıralı 50 payload döndürdü;
son sayfa ve 50 update/50 delete yeniden açılış sonrası ayrıca doğrulandı.

SQLite tablo/indeksleri:

- UNIQUE(scope,collection,id), source ve day range B-tree indeksleri.
- Türkçe katlanmış başlık+metin için external-content FTS5 trigram indeksi,
  detail=none ve columnsize=0. Folded metin ayrıca docs tablosunda tutuldu;
  tam kaynak içerik orijinal JSON'da korundu.
- Insert/update/delete trigger'ları içerik ve arama indeksini aynı işlemde değiştirdi.
- LIKE `%alt-dize%` FTS5 indeksi üzerinden çalıştı. Query plan'da
  `VIRTUAL TABLE INDEX 0:L0` ve rowid lookup doğrulandı.
- WAL + synchronous=FULL; batch commit'leri sync kapatılarak hızlandırılmadı.
- cache_size=-2048 (yaklaşık 2 MiB sayfa cache hedefi), mmap_size=0,
  temp_store=FILE. **2 MiB toplam proses RAM limiti değildir.**

Trigram/LIKE ve detail=none seçimi [SQLite FTS5 belgesine](https://www.sqlite.org/fts5.html#the_trigram_tokenizer),
kalıcılık ayarı [synchronous belgesine](https://www.sqlite.org/pragma.html#pragma_synchronous)
dayanır. Bu araç yalnız ölçülen sorgu ve alanlar için yazıldı; bütün FDB
API'sinin, scope yetkilendirmesinin veya genel guarded batch sözleşmesinin
SQLite uygulaması değildir. Corpus'taki links boş olduğundan link indeksi ölçülmedi.

## Sonuçlar

| Ölçüm | FDB 0.3 | SQLite + FTS5 trigram |
| --- | --- | --- |
| Gözlenen tepe working set | 416,70 MiB | 20,21 MiB |
| Örneklenen tepe private committed bytes | 517,48 MiB | 13,82 MiB |
| Bir milyon kayıt ingest, otomatik bakım dahil | 276,530 s | 137,404 s |
| Son bakım | 15,983 s | 3,672 s |
| 500 kayıt commit medyan / p95 | 67,450 / 482,561 ms | 42,113 / 80,156 ms |
| Commit maksimum | 2.041,788 ms | 1.032,933 ms |
| Dar başlık alt-dize araması, 30 tekrar medyan | 96,900 ms | 214,597 ms |
| source eşitliği, 5 tekrar medyan | 176,629 ms | 114,262 ms |
| source + day 2..5, 5 tekrar medyan | 38,346 ms | 39,790 ms |
| Geniş `teknoloji` alt-dize araması, 3 tekrar medyan | 9,779 s | 5,346 s |
| Referans verilen kaynak+cache / SQLite dosyası | 1.326.670.053 bayt | 1.035.628.544 bayt |
| Arşivler dahil toplam klasör, sentinel öncesi | 3.986.712.575 bayt | 1.035.628.544 bayt |

Bu konfigürasyon ve corpus'ta FDB'nin tepe working set'i SQLite prosesinin
yaklaşık **20,6 katı**. SQLite sonucu Python interpreter'ını da içerir.
Dar başlık sorgusunda FDB daha hızlı; metadata range sonuçları yakın.
SQLite bütün sorgularda daha hızlıdır sonucu çıkarılamaz.

FDB anahtar/konum ve posting sözlüklerini RAM'de tutar. SQLite bu testte
diskteki B-tree/FTS sayfalarını küçük sayfa cache'iyle işler. Bu mimari fark
FDB'de bellek azaltma için anahtar/sözlüklerin diskte tutulmasını araştırmak
gerektiğine işaret eder; sadece delta metinlerini diske geçirmek yeterli değildir.

SQLite son bakım FTS optimize + WAL checkpoint(TRUNCATE); FDB son bakım
flush + mevcut ve bütün uygun sınırlı birleştirme işleridir. Aynı algoritmalar
değildir. FDB eski kaynakları arşiv olarak saklar; SQLite WAL checkpoint sonrası
eski WAL'ı küçültür. Dolayısıyla toplam disk farkı saklama politikasını da içerir.

## Açılış ve gerçek boşta RAM

SQLite normal reopen + bir anahtar lookup **3,915 ms** sürdü. 50 update/delete
sonrası reopen + count **43,757 ms**. Bunlar bütün dosyayı doğrulamaz.
PRAGMA integrity_check ve FTS5 external-content integrity-check birlikte,
ayrı ölçümde **21,507 saniye** sürdü ve geçti.

FDB'nin önceki **11,753 saniyelik** açılışı kaynak/cache checksum + frame
doğrulamasını içerir. SQLite normal açılışıyla aynı kontrol kapsamı değildir;
bu iki açılış değerinden doğrudan hız oranı çıkarılmamalıdır. İki bütünlük
kontrolünün algoritma ve garanti kapsamları da birebir aynı değildir.

Ayrıca milyon kayıt FDB'si yeni bir CLI prosesinde açıldı. 999.951 kayıt,
15 segment ve 2002 sequence doğrulandı; stats yanıtından 1 saniye sonra,
hiç sorgu veya yazma yapmadan:

| FDB yeni proses | Ölçüm |
| --- | --- |
| Açılış + stats | 10,534 s |
| Boşta working set | **176,29 MiB** |
| Boşta private committed bytes | 263,10 MiB |
| Açılış sırasında gözlenen tepe working set | 201,44 MiB |

Dolayısıyla 416,70 MiB sürekli boşta kullanım değildir; önceki uzun yazma/bakım
çalışmasının tepesidir. 176,29 MiB de yeni açılmış prosesin tek anlık ölçümüdür;
uzun yaşayan ve daha önce yazmış bir prosesin boşta belleğini temsil etmez.

## Bellek ve sorgu ölçümünün sınırları

SQLite ana prosesi 200 ms aralıkla, 784 örnek ve 170,085 saniye boyunca izlendi.
OS PeakWorkingSet64'ün gözlenen maksimumu ve örneklenen PrivateMemorySize64
raporlandı; FDB ana ölçümünde de aynı sayaçlar/yöntem kullanılmıştı.
Working set paylaşılan fiziksel sayfaları içerir; private committed bytes resident
bellek değildir. OS toplam file cache'i, son örnek sonrası kaçabilecek tepe ve
farklı cache bütçeleri ölçülmedi. Python runtime belleği SQLite working set'ine dahildir.

Sorgular sıcak yerel OS/file cache ile yapıldı. Her iki corpus'ta çok tekrarlı
256 bayt metin var; gerçek uzun/çeşitli belge koleksiyonunu temsil etmez.
SQLite dar sorgu 1, metadata sorgu 1.000.000, range sorgu 133.336 eşleşme verdi.
SQLite SQL count+page tek okuma transaction'ında; FDB kendi native API'sinde
sayfa+count hesapladı. Metin sorgularında SQLite önceden katlanmış metni diskte
saklar; FDB payload'u okurken katlar. Bu tasarım tercihi de süre farkına dahildir.

Geniş metin sorgusu ana bellek ölçümünden sonra ayrı kurtarma prosesinde çalıştı;
FDB'de de bu sorgu ayrı proses ölçümüdür. SQLite geniş sorgu count+50 payload
SQL/Python süresini, FDB ölçümü stdio istemci serileştirmesini de içerir.
Soğuk I/O, cache büyüklükleri matrisi, tekrar eden tam ingest koşuları ve gerçek
corpus ölçülmedi; sonuç genel bir SQLite/FDB üstünlük iddiası değildir.

## Kurtarma ve kanıt

İki motorda 50 update/50 delete, ardından sentinel commit yanıtı alındıktan
sonra gerçek owner prosesi kill edildi. Yeni proses sentinel ve önceki 100
değişikliği doğruladı; 999.951 kayıt kaldı. Geniş metin sorgusu 999.900 eşleşme
verdi. Bu elektrik kesintisi testi değildir. SQLite bütünlük kontrolü kill'den
önce çalıştırıldı; kill sonrası kayıt/arama doğrulaması yapıldı.

SQLite küçük 1.000 kayıt denemesi de ingest/query/update/delete/integrity/kill
kontrollerinden geçti. Rust kaynakları değişmedi; cargo fmt ve clippy kontrolleri geçti.

Ham dosyalar: [SQLite JSON Lines](benchmarks/sqlite-million.jsonl),
[SQLite bellek](benchmarks/sqlite-million.memory.json),
[SQLite kurtarma](benchmarks/sqlite-million.recovery.json),
[FDB yeni proses/boşta bellek](benchmarks/fdb-idle-million.json).
Önceki FDB kanıtı: [0.3 doğrulama](VALIDATION-0.3.md).

Tekrarlama (boş hedef klasörü, PowerShell 7):

```powershell
.\tools\measure-sqlite.ps1 -DatabasePath data/sqlite-new -PythonPath C:/path/to/python.exe -OutputPrefix output/sqlite-new
python -X utf8 tools/verify_sqlite_recovery.py data/sqlite-new 1000000
.\tools\measure-fdb-idle.ps1 -DatabasePath data/scale-v03-20261007
```

Python'ın sqlite3 derlemesinde FTS5 trigram desteği gerekir. Motorun Cargo
bağımlılıkları değişmedi. Araçlar tools/measure_sqlite.py,
tools/measure-sqlite.ps1, tools/verify_sqlite_recovery.py ve tools/measure-fdb-idle.ps1.
