# Akışlı açılış ve serileştirme — 08.10.2026

Bağımsız FDB motorunda açılış/bakım sırasında üst üste yaşayan JSON ve belge
nesnelerini azaltan adım uygulandı. `experimental-streaming-open`, paged-metadata
yolunu da açar; default build bu deneysel açılışı kullanmaz. Frame ve directory
serileştirmesinde gereksiz ikinci output buffer'ın kaldırılması tüm build'lerde
geçerlidir. SQLite'tan düşük toplam proses RAM'i ve bütün iş yüklerinde hız
korunması hedefinin tamamlandığı iddia edilmez.

## Uygulama ve uyumluluk

- Frame doğrudan tek Vec'e serileştirilir. SEG03 directory doğrudan bounded
  BufWriter'a yazılır; bütün directory JSON'u için ikinci Vec ayrılmaz.
- Deneysel SEG03 okuyucu her Record'u tek tek çözüp packed tabloya taşır.
  Tekil metadata entry sınırı 256 KiB'dir. Büyük exact alan token'ları mevcut
  256 MiB directory sınırına kadar desteklenir; token sözlüğü RAM'dedir.
  Eski bulk BTreeMap yerleşimi korunur.
- Değişmez kaynakta frame CRC/footer önce 64 KiB bloklarla doğrulanır.
  Sonrasında Change nesneleri tek tek çözülür ve doğrulanır. Schema hatasında
  geçici builder yayımlanmaz. Sağlam `.idx` bozuk `.fdb` kaynağını kabul ettirmez.
- WAL toplu atomik Batch replay'ini korur; deserialize sonrası serialized
  payload, Batch uygulanmadan önce bırakılır.
- FDBLOG01/03, MAN03 ve SEG03 byte formatları değişmedi. Tek-buffer frame
  üretimi eski encoder referansıyla byte byte karşılaştırılır. 0.1 fixture
  kaynak baytları değiştirilmedi.

Giriş buffer'ı 64 KiB'dir; tek Change scratch'i eski 64 MiB frame sınırına kadar
büyüyebilir. Büyük token scratch'i de directory sınırına kadar büyüyebilir.
Packed record/ID tabloları bir run açılırken geçici olarak hâlâ ayrılır ve
sidecar doğrulamasından sonra bırakılır. Builder tabloları, caller batch'i,
WAL DOM'u, token sözlüğü ve büyük sorgu çıktıları ortak RAM bütçesine bağlı
değildir. Bu adım genel bir sabit RAM garantisi sağlamaz.

## Ölçüm ortamı ve açılış

Windows release CLI; aynı sağlıklı `data/scale-v031-optimized-20261008` corpus'u:
999.951 görünür belge, 1.000.100 fiziksel kayıt, sequence 2002, 15 aktif run.
On basamaklı ID, tekrarlı 256 bayt Türkçe metin, `day` ve `source` alanları.
Yerel OS/file cache sıcaktır; cold-cache testi değildir. Eski paged v4 ve son
streaming v4 ayrı proseslerde üç kez, ikinci çiftte ters sırayla çalıştırıldı.
Stats sonrası bir saniye beklenip boşta sayaçlar alındı.

| Üç tekrarın medyanı | Önceki paged v4 | Streaming v4 |
| --- | ---: | ---: |
| Açılış + stats | 6,1608 s | 5,9218 s |
| Boşta working set | 11,2500 MiB | 8,2539 MiB |
| Boşta private committed | 7,9531 MiB | 4,6914 MiB |
| OS açılış tepe working set | 43,0000 MiB | 12,9141 MiB |

Açılış tepe working set'i yaklaşık %70 azaldı. Üç tekrarın açılış süresi medyanı
yaklaşık %4 düşük; kesin veya bütün koşullarda hız kazancı değildir.
Working set/private sayaçları gerçek FDB prosesinden alınır; sistem genelindeki
file cache dahil değildir. Bunlar yazma/bakım tepe RAM'i yerine kullanılamaz.
Açılış maliyeti özellikle en büyük run'a ve token çeşitliliğine bağlıdır.

İlk directory-only v1 açılış tepesini düşürmedi. İlk kayıt bazlı v2 RAM'i
azalttı ancak açılışı yavaşlattı. Blok taraması eklenen v3'ün beş sorgu çiftinde
geniş metin p50 medyanı %10 yavaş çıktı; hız kapısı geçmiş sayılmadı. Bu ara
ölçümler `docs/benchmarks/fdb-streaming-v1-*`, `fdb-streaming-v2-*` ve
`fdb-streaming-v3-*` dosyalarında korunur. Son v4 ayrıca büyük exact alan
uyumluluğunu ve eski token ağacı yerleşimini korur.

## Sorgu doğrulaması

Beş bağımsız proses çifti, çiftler arasında ters sıra; her sorguda bir untimed
warmup. Dar metin/eşitlik/aralık sorguları proses başına 30, geniş metin/ikinci
sayfa/son sayfa üç timed tekrar. Tablo run p50'lerinin medyanıdır; CLI JSON
Lines/IPC süresi dahildir. Her cevabın bütün sayfa JSON'u eski/yeni sürüm ve
tüm tekrarlar arasında eşitlik denetiminden geçti.

| Sorgu | Önceki paged v4 | Streaming v4 |
| --- | ---: | ---: |
| Dar metin `rapor-0000500001` | 11,8906 ms | 11,6093 ms |
| `source=synthetic`, ilk 50 | 1,0672 ms | 0,9930 ms |
| Aynı filtre, offset 50 | 1,1449 ms | 1,0351 ms |
| `source=synthetic`, day 2..5 | 3,6362 ms | 3,6472 ms |
| Geniş metin `teknoloji` | 2691,1249 ms | 2670,5905 ms |
| Aynı eşitlik filtresi, offset 999900 | 23,0562 ms | 22,2126 ms |

Geniş metin/aralık medyanı bu son seride yaklaşık aynı düzeydedir. İlk/ikinci
sayfa medyanları %7/%10 düşük çıktı; bütün sorgu veya tail gecikmelerinde hız
garantisi değildir. Örneğin ikinci sayfanın run p95 medyanı 1,1469 → 1,4139 ms,
son sayfanınki 23,5136 → 24,3914 ms oldu. Dar metinde en kötü run p95 de
15,9228 → 16,3686 ms. Default'a geçiş ve tüm hız kabulü tamamlanmadı.

Ham veri: `docs/benchmarks/fdb-streaming-v4-reads-20261008.jsonl`,
`fdb-streaming-v4-reads-20261008.summary.json`; üç açılış çiftinin dosyaları
`fdb-{paged-v4,streaming-v4}-open-final-v4-20261008-{1,2,3}.json`.

## Yazma/bakım ve allocation profili

Üç bağımsız çiftte yeni dizinlere 100.000 kayıt, 256 bayt aynı metin, 500'lük
batch yazıldı. Çift sırası baseline/candidate, candidate/baseline, baseline/candidate.
Commit sequence'ları ve son count doğrulandı; flush/bekleme ve mümkün olan
merge'ler tamamlandı. Her DB ayrıca yeniden açılıp ilk/orta/son belgenin bütün
içeriği ve sürümü doğrulandı. Bu, bütün corpus'un byte byte taraması değildir.

| Üç tekrarın medyanı | Önceki paged v4 | Streaming v4 |
| --- | ---: | ---: |
| Ingest, IPC dahil | 11,3248 s | 11,6456 s |
| Son bakım | 9,7138 s | 9,1528 s |
| Ingest + son bakım | 21,0386 s | 20,7984 s |
| Tepe working set | 98,8320 MiB | 92,3242 MiB |
| Örneklenen private committed tepe | 98,4688 MiB | 90,6289 MiB |

OS working-set tepe sayacı; private bytes için 20 ms istenen örnekleme aralığı.
Ölçüm penceresi open + ingest + bakım; reopen doğrulaması bellek ölçümünün
dışındadır. Corpus ve batch JSON'u dış Python istemcisinde önceden hazırlanır;
bu istemcinin RAM'i FDB proses sayacına dahil değildir. Süreler IPC içerir.
Cache yeni DB yazımından oluşur; cold-cache karşılaştırması değildir.

Yazma/bakım tepe working set'i yaklaşık %7 azaldı; ingest medyanı yaklaşık %3
daha yavaş. Toplam medyan yaklaşık %1 düşük ama üç tekrar ve background merge
varyasyonu genel hız üstünlüğü kanıtı değildir. Candidate working-set tepeleri
89,27..97,07 MiB aralığındaydı. **Yazma RAM'i hedefin hâlâ çok üzerindedir.**
Bir milyon kayıtlı açılış sayacı ile bu 100 bin kayıtlı yazma tepesi farklı
iş yükleridir; birbirinin yerine sunulmaz.

Ham veri: `docs/benchmarks/fdb-streaming-v4-writes-20261008.jsonl` ve `.summary.json`.

Aynı bir milyon fiziksel kayıt corpus'unda opt-in Rust allocation sayacı,
açılış requested peak'ini önceki 37.880.100 bayttan 8.848.942 bayta indirdi.
İş bitimindeki requested live 2.445.630 baytla aynı kaldı. Bunlar RSS/private
değildir; allocator metadata, runtime ve sistem cache dahil değildir. Ölçüm
binary'sindeki atomic sayaç overhead'i nedeniyle süreler performans kanıtı
olarak kullanılmaz. Offset 5000 yolunda ek requested peak 7.400.506 bayt,
toplam 9.848.600 bayt kaldı; tam ID listesi henüz kaldırılmadı.

Profil: `docs/benchmarks/fdb-streaming-v4-heap-20261008.jsonl`.
Profile binary SHA256:
`A817402624AF80A964DCDE3375CAD2081FE169C8F9F56C3A516F15E1B43F8A78`.

Ayrı native Rust `profile_writes` koşusunda 100.000 kayıt/500'lük batch için
cumulative requested peak 81.323.150 bayt (77,56 MiB), son bakım sonrası
requested live 164.396 bayt oldu. Tepe ilk kez 80.500 kayıtlık commit raporunda
görüldü; background compaction çalışıyordu. Son bakım bu cumulative tepeyi
artırmadı. Sayaç eşzamanlı worker'ı içerir; tek bileşene kesin attribution
vermez ve CLI'nin JSON giriş nesnelerini içermez. Zamanları kullanılmaz.
Bu koşu, yalnız boşta dizin cache'ini küçültmenin yazma tepesini çözmediğini
gösterir; delta ile merge/builder'ın ortak transient bütçesi gerekir.

Ham profil: `docs/benchmarks/fdb-streaming-v4-write-heap-20261008.jsonl`.
Write profile SHA256:
`63C8F46A12E426BB233486A02791A3FB6F4328EF841B7B2777845C0E73A66107`.

## Doğrulama

`cargo test --all-targets --all-features`: 64 test geçti. Ek kapsamlar:
frame byte uyumluluğu, checkpoint frame'inde her kesilme/bayt bozulması,
CRC'den önce callback verilmemesi, JSON alan sırası/duplicate/unknown/trailing
kontrolleri, küçük giriş pencerelerinde escape/UTF-8, büyük exact alanlı kaynak
ve cache rebuild/reopen. Süreç sonlandırma, WAL atomikliği, bozuk kaynak + sağlam
cache ve 0.1 fixture testleri de bu koşunun parçasıdır.

`cargo clippy --all-targets --all-features -- -D warnings` geçti.
Varsayılan `cargo test --all-targets`: 49 test geçti;
`cargo fmt --check` ve `cargo clippy --all-targets -- -D warnings` geçti.
İş sonunda default release CLI tekrar derlendi; deneysel binary ayrı korundu.

## Tekrarlama ve binary kimliği

```powershell
cargo build --release --features experimental-streaming-open --bin fdb --example profile_memory
tools/measure-fdb-idle.ps1 -DatabasePath data/scale-v031-optimized-20261008 -BinaryPath output/fdb-streaming-open-v4-20261008.exe
python tools/compare-fdb-reads.py output/fdb-paged-metadata-v4-20261008.exe output/fdb-streaming-open-v4-20261008.exe data/scale-v031-optimized-20261008 --runs 5
python tools/compare-fdb-write-memory.py output/fdb-paged-metadata-v4-20261008.exe output/fdb-streaming-open-v4-20261008.exe data/streaming-write-new --count 100000 --runs 3
```

Yazma aracı yeni dizin gerektirir; mevcut veritabanına yazmaz.

Önceki paged v4 SHA256:
`353EFB832545B9AB487C66215B7181FC2E010E6C312541005911945FC3B2E313`.
Son streaming v4 SHA256:
`26AFF1C5708B1FF38F935FCF837B9B9166EA0D5CABC65A76876082A4404F9AAF`.
Default release CLI SHA256:
`5105D3A11043D428761C93878213DDA29620EA5DA3885996F9B556A1BF29F38F`.
Default snapshot: `output/fdb-default-serialization-20261008.exe`.

## Kalan hedef

Native SQLite'ın önceki ingest ölçümü 9,48 MiB tepe working set / 5,22 MiB
örneklenen private committed idi. Bu turda SQLite yeniden ölçülmedi; FDB'nin
boşta sayacı ile SQLite'ın ingest tepesi eşdeğer karşılaştırma değildir.
FDB açılış tepesi bile halen bu tarihsel SQLite tepesinden yüksek.

Token sözlüğü, run açılışındaki geçici packed tablo, delta/builder/worker için
ortak bütçe, büyük offset'in tam ID listesi, büyük giriş/çıkış API'si ve cold-cache
kapıları kalan işlerdir. Sonraki adım yazma/bakım allocation baskısını ortak
bütçeye almak ve token sözlüğünü sayfalamaktır. Güncel, eşlenmiş native SQLite
karşılaştırması bu sınırlar ele alındıktan sonra tekrar yapılmalıdır.
