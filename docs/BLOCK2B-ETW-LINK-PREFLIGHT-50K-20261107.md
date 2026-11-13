# Blok 2b ETW ön testi — bağlantı provası sonrası

**Sonraki kullanıcı kararı.** Sıcak/cold zinciri kabul edildi. Ana koşu
kuralı birleşik ETL tracerpt kayıp 0 ve beklenen okumaların eksiksiz olması.
“Collector bazı ölçülemedi” etiketi korunur. Aşağıdaki ilk karar tarihsel.
[Ana koşu planı](BLOCK2B-COLD-ETW-MAIN-PLAN-50K-20261107.md)

**Disk erişimi ve dosya/PID atfı doğrulandı. Ön testin collector bazında
kayıp olay kapısı açık.** Bir purge ve bir ETW kaydı alındı. Ana ölçüm 0/8.
Kabul kararı **gerçek cold kanıtlanamadı** olarak açık kalır.

## Bağlantı ve süre

Çok satırlı yapıştırmada satır sırası ters döndü ve PowerShell `>>`
isteminde bekledi. Tek satırlık bağlantı provasında okuyucu PID 65004,
11:03:39.303 UTC'de PING aldı ve ACK verdi. Kullanıcı `BAGLANTI HAZIR`
çıktısını doğruladı. İlk tek satırlık ETW komutunda son `}` eksikti.
Kullanıcı eksik karakteri ekleyince aynı bekleyen komut çalıştı. Düzeltilen
satır PowerShell AST ayrıştırıcısıyla kontrol edildi.

Başlangıç **11:04:46 UTC**, deadline **11:14:46 UTC**. Normal okuyucu
PID **21552**, Windows oturumu 1. Üç fazda toplam 202.167 uygulama baytı.
Yönetici ekranında `-Es` exit 0, `-Et` exit 0 ve `STOP` exit 0 görüldü.
Stop isteği başlangıç çağrısından **1.601 ms** sonra. ETL başlık zamanları
11:07:10.2110384–11:07:13.0189867 UTC, yaklaşık **2,808 s**. Kapanışta WPR
boşta ve nihai ETL **50 MiB**.
[Yönetici ekranı satırları](benchmarks/cold-etw-link-20261107/admin-console.json)

## Dosya ve okuma kanıtı

| Dosya | Faz başına uygulama baytı | Sıcak FileIO / eşlenen DiskIO | Purge sonrası eşlenen DiskIO |
|---|---:|---:|---:|
| `run-0000000000001c98.fdb` | 1.853 | 1 okuma / 0 B | 1 okuma, **2.048 B** |
| `sqlite.db` | 65.536 | 1 okuma / 0 B | 2 okuma, **65.536 B** (20.480 + 45.056) |

Sıcak ve cold FileIO ilk offset 0. Küçük FDB dosyasında buffered handle'ın
FileIO isteği 4.096 B, uygulamaya dönen miktar 1.853 B. `FileIo/Create`
satırları tam hedef yolu ve PID 21552'yi, `FileIo/Read` satırları aynı
FileObject ve FileKey'i gösteriyor. `DiskIo/ReadInit` istek kimliği
`DiskIo/Read` ile eşleşiyor. Başlatan TID 81084, okuyucu PID 21552.
FDB için 158083 → 158091, SQLite için 158509 → 158547 ve 158515 → 158578
satırları. Tamamlanma olayının PID alanı `0xFFFFFFFF` olduğu için PID
başlatma olayından bağlandı. [DiskIO alanları](https://learn.microsoft.com/en-us/windows/win32/etw/diskio-typegroup1)

SQLite için ek 65.536 B disk okuması PID 4 / TID 86384'e bağlı ve dosya
offset'i 65.536. Okuyucunun 64 KiB sonucundan ayrı tutuldu. Trace, iki
dosyada seçilen aralığa ilişkin erişimi gösteriyor.
[Seçili ham satırlar](benchmarks/cold-etw-link-20261107/target-evidence.txt) ·
[Okuyucu fazları](benchmarks/cold-etw-link-20261107/reads.jsonl)

## Kayıp olay ve alan bütçesi

Yerel tracerpt özeti **380.067 olay, 50 tampon, toplam kayıp olay 0**
gösteriyor. Elde tek `Relogger` başlıklı birleşik ETL var. WPR stop/merge
sonunda kaynak collector ETL'leri artık görünmüyordu. Collector'ların ayrı
kayıp sayaçları kaydedilmedi ve birleşik toplamın kaynak sayaçlarını
koruduğu doğrulanmadı. Onaylı tasarımın collector bazında 0 şartı açık.
[Tracerpt özeti](benchmarks/cold-etw-link-20261107/trace-summary.txt) ·
[ETW oturum başlığı](https://learn.microsoft.com/en-us/windows/win32/etw/trace-logfile-header)

Mevcut tracerpt ile açılan CSV 158.083.124 B. Bu dosya ve ETL dahil
kanıt klasörü yaklaşık **200,8 MiB**. Ana matris için 256 MiB toplam ETL
bütçesine sığma da henüz gösterilmedi. Bu kaydın 50 MiB boyutu sekiz ana
iz için tahmin olarak kullanılırsa ön test dahil 450 MiB olur. Bu bir
ölçülmüş ana koşu boyutu değildir.

Ana koşunun ilk sonuç ve yakınsama kapıları **ÖLÇÜLMEDİ**. 2b açık. Sonraki
adım için ayrı ana koşu onayı ve açık kanıt şartının çözümü gerekir.
[UTC ve sayaçlar](benchmarks/cold-etw-link-20261107/summary.json)
Kapanış **11:12:29 UTC**, toplam **463 s**.
