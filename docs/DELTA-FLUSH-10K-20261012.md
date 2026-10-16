# Delta/flush yeniden kullanımı — aynı 10 bin belge

## Sonuç ve karar

Flush artık belgeleri yeniden tarayarak indeks çıkarmıyor. RAM delta
dictionary'si ve posting listeleri checkpoint'in sıralı kayıt ID'lerine taşınıyor.
Silme kayıtlarının slot'ları korunuyor; tekrar kullanılan RAM slot ID'leri
disk sırası olarak kabul edilmiyor. Kayıt/payload yazımı, CRC, kaynak replay ve
publication doğrulaması devam ediyor. Kalıcı format/API/protokol değişmedi.
0.1 fixture değiştirilmedi; dış proje entegrasyonu yapılmadı.

Yeni feature yok: mevcut `experimental-merge-postings` flush ve merge
yeniden kullanımını kapsar. Delta ekleme maliyetini azaltmak için mevcut
`experimental-delta-bitmaps` ile birleşimi ölçüldü. İkisi de default kapalı.
Bu karşılaştırma **flush reuse + bitmap birleşiminin** kazancıdır; iki değişimin
ayrı ayrı normal benchmark katkısını göstermez. Bağımsız flush yolu da test edildi.

## Aynı oturumda ölçülen sonuçlar

Her sütun iki yeni proses koşusunun medyanıdır. Kontrol önceki merge-only
frozen executable; aday yeni flush reuse + bitmap. Önceki raporların mutlak
süreleri farklı koşu/OS koşullarına aittir, bu tablonun kontrolü yerine kullanılmaz.

| Ölçüm | Önceki merge-only FDB | Yeni birleşim FDB | SQLite |
|---|---:|---:|---:|
| Yazma | 58,98 s | **27,60 s** | 33,36 s |
| Son bakım | 3,89 s | **2,15 s** | 1,08 s |
| Yazma + bakım | 62,87 s | **29,75 s** | 34,44 s |
| Örneklenen tepe private | 165,15 MiB | 124,84 MiB | 8,55 MiB |
| Örneklenen tepe working set | 167,19 MiB | 127,92 MiB | 13,39 MiB |
| Boşta private, 3 açılış medyanı | 20,53 MiB | 21,02 MiB | 1,31 MiB |
| Açılış, 3 proses medyanı | 982,57 ms | 980,50 ms | Farklı doğrulama sözleşmesi |
| Tutulan DB dosyaları | 1.513.744.522 B | 1.513.744.522 B | 583.860.224 B |

FDB toplamı kontrole göre %52,68 azaldı; eşleşmeler ayrı ayrı %46,54 ve
%57,16 kazanç verdi. Bu SQLite düzeninde aday toplamı medyanda %13,62
daha kısa, iki eşleşmede %17,66 ve %9,56 daha kısa. Yazma aşaması da
iki eşleşmede kontrol ve SQLite'tan daha kısa çıktı. Son bakımda SQLite hâlâ hızlı.
Bu sonuç genel motor/arama üstünlüğü veya production kabulü değildir.

Tepe private yaklaşık %24,4 azaldı; idle private yaklaşık 0,49 MiB arttı.
Disk kazancı yoktur: iki FDB'nin canlı source/index boyutu ve tutulan
eski nesilleri aynıdır. RAM/disk hedefleri bütünüyle karşılanmış sayılmaz.
Bu yazma ölçümünde sorgu gecikmesi ölçülmediği için default etkinleştirilmedi.
Sonraki ayrı [mevcut DB okuma ölçümü](CONTENT-READS-10K-20261012.md) dört sorguyu
ölçer; 30 sorgu/50k/100k kabul kapısını tamamlamaz, default kararını değiştirmez.

## Corpus, cache ve düzen

Yeni corpus/belge indirme/üretimi yok. Mevcut
`data/rkyv-corpus-20261009/json.payload-bench` içindeki aynı ilk 10.000 kayıt:
264.842.858 bayt metin, belge başına 2.057–51.197 bayt. Sekiz kitabın çeşitli,
benzersiz, kısmen örtüşen pasajları; bağımsız 10 bin eser veya Türkçe corpus değil.
Header/record/payload CRC'leri kontrol edildi; 500 belge/batch, WAL sync açık.

Sıra kontrol0 → aday0 → SQLite0 → SQLite1 → aday1 → kontrol1.
FDB feature'ları kontrol `experimental-merge-postings` (eski binary),
aday `experimental-merge-postings experimental-delta-bitmaps` (yeni binary).
Release executable hash'leri koşu öncesi/sonrası kontrol edildi; kayıt
`benchmarks/delta-flush-10k-20261012-executables.json`.
OS cache boşaltılmadı; idle'da sorgu ısıtması yapılmadı. Örnekleme 20 ms,
faz marker kontrolü 100 ms. Örneklenen tepeler anlık tepeyi kaçırabilir;
private/working set farklı OS sayaçlarıdır, heap bütçesi veya file cache toplamı değildir.

SQLite comparator önceki aynı schema: JSON payload + folded text, FTS5
trigram external content, 2 MiB cache, mmap kapalı, temp FILE, WAL ve
synchronous FULL; optimize + checkpoint bakım süresine dahil. FDB alan/link
indeksleri ve bütün kaynakları doğrulayan açılış sözleşmesi farklıdır.

Yalnız `data/benchmark-work/fdb` ve `data/benchmark-work/sqlite` tekrar
kullanıldı; her sonuç doğrulanmadan slot sıfırlanmadı. Başka diske taşıma
ve koşu başına DB arşivi yok. Son DB'ler okuma testleri için korunur.
Corpus dahil `data/` toplamı 4.857.959.763 bayttır.

## Profil: tekrarın kaldırılması

Her varyantta ayrı bir `diagnostic-timing` koşusu; aynı slot ve aynı belgeler.
İç içe/worker span süreleri örtüşür; CPU payı olarak toplanmaz ve normal
koşu süreleriyle kıyaslanmaz. Ingest/maintenance snapshot çağrıları toplamı:

| İşlem | Merge-only kontrol | Yeni birleşim |
|---|---:|---:|
| Gövde fold | 20.000 | **10.000** |
| Gövde trigram çıkarımı | 20.000 | **10.000** |
| Builder terim çıkarımı | 10.000 | **0** |
| Builder normal posting ekleme | 10.000 | **0** |
| Delta dictionary export / finish | 0 | 10 |
| Builder payload/kayıt ekleme | 20.000 | 20.000 |
| Binary batch decode | 111 | 151 |
| WAL write / sync | 106 / 106 | 106 / 106 |

Decode artışı flush'ın ek kaynak replay doğrulamasıdır; doğrulama maliyeti
atılmamıştır. Aynı 19.221.340 disk posting girdisi, 10.000 belge, sequence20,
generation31 ve 5 segment elde edildi. Yeni yolda metin yalnız delta indeksi
kurulurken taranır; flush ve merge terimleri yeniden üretmez.

Kontrol profilde delta ekleme 36,33 s span kaydetti; adayda 16,17 s.
Aday gövde grams 7,30 s, fold 0,76 s; flush delta export toplam 5,61 s,
binary encode/CRC toplam 0,59 s. Bu ham span'lar örtüşür. Kalan maliyetin
önemli bölümü delta ekleme, grams üretimi ve RAM→disk kayıt ID remap'idir;
yalnız JSON çözümü değildir.

## Doğrulama

Bağımsız yeniden indekslemeyle `.idx` **bayt eşitliği**: güncelleme, silme,
geri kullanılan slot ID'leri, yoğun bitmap, karışık scope/collection,
Türkçe title/text, scalar/null/float/negative alan, nested alanlar ve duplicate
links. Eksik/eşleşmeyen remap reddedilir; başarısız export hedef cache yayımlamaz.

Dört normal ve iki profil FDB koşusunun her biri ayrı prosesle 10.000
tam belge `get` eşitliğini ve independent fold/contains scan oracle'ını geçti.
`the`, `quixote`, `he`, `the man`: toplam + sıralı ilk 50 tam belge/fields/
links/version kontrolü; toplamlar 10.000 / 56 / 10.000 / 9.932, sequence20.
İki SQLite koşusunun da veri + external-content FTS integrity kontrolü geçti.

Default63, yalnız reuse67, birleşim71, all-features117 test geçti.
Kesilme/bozulma, sağlam cache arkasındaki bozuk kaynak, process kill,
merge publication ve eski fixture okuma testleri kapsandı.
`cargo fmt --check`, default/birleşim/all-features clippy `-D warnings` geçti.

Ölçülmeyenler: arama/contains p95, cold cache, concurrent okuyucu/yazıcı,
DLL/derived profil, büyük corpus, geçici disk tepesi, flush ve bitmap'in ayrı
normal süre katkıları. Sonraki adım mevcut DB'lerde okuma/contains gecikmesi
karşılaştırmasıdır; yeni belge üretimi veya yazma benchmark'ı gerekmez.

Ham loglar ve özet: `benchmarks/delta-flush-10k-20261012-*.jsonl`,
`benchmarks/delta-flush-10k-20261012-*.memory.json`,
`benchmarks/delta-flush-10k-20261012-summary.json`.
