# Blok 2b ana koşu sonucu — 7 Kasım 2026

**İlk denemede duruldu.** FDB cold/warm ölçümü ham süreleri üretti. Collector
dosya sınırı izi geçersiz yaptı. Geçerli cold deneme **0/8**, başlatılan
deneme **1/8**. SQLite performans kolu başlamadı.
[Onaylı plan](BLOCK2B-COLD-ETW-MAIN-PLAN-50K-20261107.md)

Başlangıç **11:51:04 UTC**. Yeni deneme sınırı **12:08:04 UTC**,
son doğrulama sınırı **12:16:04 UTC**, toplam deadline **12:21:04 UTC**.
Ekrandaki ilk erken betik çağrısı arm kaydı bulunmadan çıktı. Onu izleyen
tek hazır çağrı 12:01:10.461 UTC'de başladı ve 12:01:30.724 UTC'de kapandı.
Ana koşuda **1 purge, 1 ETW**. Kabul edilen ETW ön testi dahil **2 / 2**.

## Ham süreler — geçersiz FDB denemesi

Aynı 881756DA… corpus'un ilk 50k slotu, normal strict open, akışlı WAL
profili, TR case0 `olduğu`, melez200 ve ilk N=50. Cold purge sonrası yeni
proses, warm 467.167.854 B stream-read sonrası ayrı proses. WPR iki fazda
da çalıştı. FDB 30 cold ve 150 warm örnek döndürdü. Tüm sonuçlarda
sequence 529967 ve case0 CRC32 725765487 doğrulandı.

| Ölçüm | Cold ham, ms | Warm ham, ms |
|---|---:|---:|
| Açılış | 1674,72 | 1287,27 |
| İlk sorgu | 95,60 | 97,31 |
| Open-start → ilk sonuç | **1770,33** | **1384,58** |
| Q30 | 119,13 | 125,53 |

| Sıcak A/A | p50 ms | p95 ms | Max ms |
|---|---:|---:|---:|
| A, 60 örnek | 126,16 | 173,70 | 181,64 |
| B, 60 örnek | 122,43 | 140,82 | 157,11 |

Cold ham değer 1 s çizgisinin **770,33 ms** üstünde. İz geçersiz olduğu
için bu koşudan kabul edilebilir cold kapı kararı çıkmaz. Yakınsama için
sayısal karar verilmedi. WPR maliyeti ve diğer farkların nedeni ayrıştırılmadı.
[180 ham örnek](benchmarks/cold-etw-main-20261107/trial-01/measurement.json)

## İz ve disk kapısı

`-Es`, `-Et`, WPR start ve stop exit 0. Stop isteği **13.408 ms**'de
verildi. Yönetici betiği bir collector dosyasının sınırına ulaştığını
gördü. Hangi collector olduğunu bu sürüm kaydetmedi. Birleşik ETL özeti
**740.731 olay, toplam kayıp 0**. **Collector bazı ölçülemedi.**

CSV çözümlemesi 180 MiB erken durdurma sınırına ulaştı. Son geçici boyut
188.864.296 B. Elde kalan bölümde beklenen 54 FileIO kaynağının **50'si**
eşlendi. Okuyucuya **40.930.816 B** storage okuması bağlandı. Dört kaynak
için FileIO kanıtı eksik. Bunların yokluğunun kayıt, kısmi çözümleme veya
atıf kapsamından hangisine bağlı olduğu bilinmiyor. Bu nedenle kayıp 0
sonucu tek başına yeterli kabul edilmedi.

Süzülen 74.842 satır ve tracerpt özeti saklandı. Yalnız yürütücünün açtığı
`trial-01/decoded.tmp.csv` silindi. Ön test dahil izlenen en yüksek toplam
**410,9 MiB**. Geçici CSV silindikten ve son doğrulamalar bittikten sonra
yaklaşık **231,0 MiB** toplam ve **61,8 MiB ETL**. Ana sıkıştırılmış ETL
12.378.698 B. İzlenen değerler 512 MiB ve 256 MiB tavanlarının altında.
Bu tek geçersiz izin boyutu sekiz geçerli izin boyut tahmini değildir.
[Yönetici kaydı](benchmarks/cold-etw-main-20261107/trial-01/admin-result.json) ·
[İz özeti](benchmarks/cold-etw-main-20261107/trial-01/tracerpt-summary.txt) ·
[Süzülmüş olaylar](benchmarks/cold-etw-main-20261107/trial-01/target-events.jsonl) ·
[Geçersizlik kararı](benchmarks/cold-etw-main-20261107/trial-01/verdict.json) ·
[Disk ve CSV temizliği](benchmarks/cold-etw-main-20261107/trial-01/disk-evidence.json)

## Doğrulama ve kapanış

| Kapı | Sonuç |
|---|---|
| Corpus SHA, model kimliği, başlangıç iki motor 50k GET/version ve 41 vaka | **GEÇTİ** |
| 180 zamanlı case0 sonuç oracle'ı | **GEÇTİ** |
| Okuyucu process kill → reopen ve tam oracle, iki motor | **GEÇTİ** |
| Eski FDB okuyucusu ve eski SQLite karşılaştırıcısı | **GEÇTİ** |
| Kaynak/indeks, SQLite ana DB ve model SHA karşılaştırması | **GEÇTİ**, mevcut kaynaklar |
| SQLite dosya envanteri | **KALDI**, başlangıçtaki 0 B WAL ve 32.768 B SHM kapanışta yok |
| Birleşik kayıp 0 + eksiksiz okumalar ve geçerli collector | **KALDI**, deneme geçersiz |
| Geçerli cold ≤1 s ve 30. arama yakınsaması | **ÖLÇÜLMEDİ** |

Eski SQLite doğrulamasında kullanılan frozen ikili bağlantıyı read-write
açar. Son dosya karşılaştırması yan dosyaların hangi doğrulamada kaybolduğunu
belirlemez. Ana SQLite DB SHA256 aynı, sequence ve tüm 50k GET/version
oracle'ı geçti. FDB kaynak ve indekslerinin dosya kimliği, boyutu ve hash'i
eşleşti. Fiziksel yerleşim kapısı ayrı KALDI olarak korunur.

Root `cargo fmt --check` ve `cargo clippy --offline --all-targets -- -D warnings`,
izole SQLite yürütücü clippy ve yönetici/normal yürütücü AST kontrolleri geçti.
Yeni ölçüm araçları yalnız benchmark giriş noktalarına eklendi. 2b açık.
Sonraki deneme için kullanıcı kararı beklenir.
[Kill/reopen ve eski okuyucu özeti](benchmarks/cold-etw-main-20261107/post-verification.json) ·
[Son envanter farkı](benchmarks/cold-etw-main-20261107/final-inventory-check.json)
Rapor kapanışı **12:09:25 UTC**, toplam **1101 s**. [UTC ve sayaçlar](benchmarks/cold-etw-main-20261107/summary.json)
