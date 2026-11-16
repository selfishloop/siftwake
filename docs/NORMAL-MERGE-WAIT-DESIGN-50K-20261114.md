# 100'lük akışlı commit'te normal merge beklemesi

14 Kasım 2026. İnceleme tabanı `ddb263b`. Bu adım mevcut kaynak
ve kayıtlı 50k trace üzerinde yapıldı. Yeni motor davranışı veya
benchmark yok. [Önceki ölçüm](WAL-PREFIX-FOLLOWUP-20261114.md)

## Beklemenin yeri

B1 ve B2 profilli kollarındaki 6.000 commit frame'i yeniden okundu.
`merge.join` aralıkları, başlangıç ve bitiş saatleri üzerinden
`stream.worker_drain` veya `stream.post_flush_drain` içine eşlendi.
Her kolun 3.000 commit'i ve 1 saniye üstü sınıfı önceki raporla aynı.

| Profilli kol | Uzun yola giren / toplam | >1 s olanların uzun yola gireni | İlk drain'de normal merge join | Flush sonrası normal merge join | Diğer normal merge join |
| --- | ---: | ---: | ---: | ---: | ---: |
| B1 | 117 / 3.000 | 28 / 28 | 13, toplam 5.382,89 ms | 3, toplam 2.242,73 ms | 19, toplam 0,16 ms |
| B2 | 116 / 3.000 | 72 / 72 | 11, toplam 5.243,27 ms | 2, toplam 1.987,27 ms | 10, toplam 0,09 ms |

| Sequence | Commit (ms) | İlk drain'de normal merge (ms) | Flush sonrası yeni normal merge (ms) |
| --- | ---: | ---: | ---: |
| B1 · 1422 | 2.314,87 | | 801,05 |
| B1 · 1597 | 1.476,92 | | 448,93 |
| B1 · 2019 | 2.858,11 | 762,95 | 992,75 |
| B2 · 298 | 1.920,61 | | 609,63 |
| B2 · 2599 | 3.109,18 | 514,01 | 1.377,64 |

İlk drain sütunu yalnız en uzun iki commit için açıldı. Bunlarda
ayrıca sırasıyla 124,94 ve 431,85 ms WAL prefix işçisi beklemesi var.
Flush sonrası beş beklemenin toplamı **4.230 ms**. Bu, beklemeye
atfedilen eski süre. İşçinin kaldırıldığı bir koşudan ölçülmüş kazanç
henüz yok. [Ham B1/B2 aşamaları](benchmarks/wal-prefix-remeasure-20261114/02-profile-abba/outlier-detail.json)

## Hangi bekleme hangi işi yapıyor?

| Yol | Kod ve bağımlılık | İlk aday |
| --- | --- | --- |
| Commit başındaki prefix ve merge yoklaması | Yalnız bitmiş işi devralır. Kısa WAL yolu, kapasite ve işlem boyutu uygunsa WAL'a yazar, sync eder ve snapshot yayımlar. | Aynı. [Commit başı](../src/engine/streamed.rs#L226) · [Kısa WAL yolu](../src/engine/streamed.rs#L313) |
| Uzun yolun ilk drain'i | `wait_for_merge()` önce pending WAL prefix'ini bitirir. Bu sırada frozen prefix ile tail yeni aktif WAL üzerinden kurulabilir. Ardından normal merge işini bekler. Normal merge çıktısının belge sırası için commit'ten önce gerekli olduğuna dair bir bağımlılık yok. İşçi sabit run penceresiyle çalışır. Ancak ilk drain'i kaldırmak, çalışan işçinin kaynak/yapıcı kilitleriyle ve commit'in tam çalışma alanı rezervasyonuyla çakışabilir. | İlk drain ve prefix rotasyonu bu deneyde korunur. [Çağrı](../src/engine/streamed.rs#L407) · [Birleşik bekleme](../src/engine.rs#L2906) · [WAL kurulumu](../src/engine/light_flush.rs#L130) |
| Delta flush sonrası drain | `flush()` eski delta'yı run'a çevirir, maskeleri yeniler ve dört uygun run varsa otomatik merge başlatır. Hemen sonraki çağrı bu yeni işçiyi bekler. Akışlı transaction, yeni run'ını bu merge sonucu olmadan da manifest sonuna ekleyebilir. Daha sonra merge yalnız kendi eşleşen giriş penceresini değiştirir ve yeni suffix'i korur. | Yalnız uzun yolun ilk delta flush'ının otomatik merge planlamasını erteler. Commit sonundaki mevcut otomatik planlama, başarılı yayım ve çalışma alanı bırakıldıktan sonra çalışır. [Flush sonu](../src/engine.rs#L2616) · [İkinci bekleme](../src/engine/streamed.rs#L420) · [Yayım ve son planlama](../src/engine/streamed.rs#L474) · [Merge kurulumu](../src/engine.rs#L2837) |
| Tam çalışma alanı içindeki ikinci flush ve son drain | Önceki adımlar bitince çağrılır. Mevcut akışta arada yeni delta kaydı üretilmez. `flush()` yine hazır iş yoklayabilir ve eski dosya toplamasına girebilir. `wait_for_merge()` yeni iş kalmışsa bloklayıcıdır. | İlk prototip bu iki çağrıyı korur ve profil altında ayrı maliyetlerini gözler. Ek davranış değişiklikleri bu veriye bağlı. [Çalışma alanı ve son çağrılar](../src/engine/streamed.rs#L442) |

**Kaynak bütçesi.** Süreç genelinde 16 MiB çalışma havuzu, 2 MiB
taban rezervasyonu, 6 MiB işlem alımı, 8 MiB builder ve uzun
akışlı yol için 14 MiB tam rezervasyon var. Kaynak yazımı ve builder
kilitleri ayrı. Bir normal merge'i aynı commit içinde erken başlatıp
yalnız `join`i silmek, beklemeyi bu havuza veya kilitlere taşıyabilir.
Daha sonra aynı worker'ı tam havuz tutulurken join etmek kilitlenme
riski doğurur. Erteleme bu nedenle işçinin başlangıç noktasında
yapılmalı. GC, çalışan işçinin kaynak kimliklerini, çıktısını ve
snapshot pinlerini hâlihazırda koruyor.
[Çalışma havuzu](../src/write_budget.rs#L87) ·
[Kaynak ve builder sırası](../src/engine.rs#L919) ·
[GC koruması](../src/engine.rs#L2461)

## Dar prototip ve kanıt sırası

1. **Yalnız geçici derlemede** birlikte ölçülebilen iki davranış hazırla.
   Kontrol bugünkü yolu çalıştırır. Aday, ortak flush uygulamasına
   iç kullanım için bir planlama seçeneği verip sadece
   `commit_streamed` uzun yolundaki delta flush sırasında otomatik merge
   başlatmayı erteler. İlk drain, aynı anda çalışan işçi sayısı, WAL
   sync, kısa yol, görünürlük maskesi ve public `flush()` davranışı
   korunur. CRC ve kalıcı biçim değişikliği gerekmiyor.
2. Küçük kontrollü testte üç benzer run ve dördüncü run'a dönüşecek dolu
   delta hazırla. Uzun yolu örneğin 129 geçerli işlemle seçtir. İşçiyi
   mevcut veritabanına özel test bariyerinde tut. Kontrolde commit'in
   yeni işçiyi beklediğini, adayda ACK ve sequence yayımının işçi
   serbest bırakılmadan geldiğini tanıkla. Hazır işçinin sonradan
   yayımlandığını da doğrula. Merge yeni run da eklendikten sonra
   planlandığı için seçilen pencere değişebilir.
3. Yazıcı ve güncel/eski snapshot'larda GET, exact/capped, tombstone ve
   sequence oracle'ı. Process-kill için commit ACK çevresi ve merge
   yayımından önce/sonra, ardından strict reopen ve dondurulmuş okuyucu.
   Kaynak CRC, fixture, fmt, Clippy ve tüm test paketi önce geçmeli.
4. Ancak bundan sonra mevcut Cap200k ilk 50k, aynı sabit slot, aynı 41
   sorgu, 1R+1W ve 100'lük gruplarla dengeli kontrol/aday. Önceki iki
   saniyelik aralı iş yükünde p95/p99/p99,9/max, 1 s üstü adet ve ops/s
   birlikte. Profile alınan ayrı kolda ilk drain, post-flush drain,
   rezervasyon, mutex ve worker başlangıç/bitim/yayım sequence'lerini
   eşle. Uçların komşu commit'e taşınıp taşınmadığı da ölçülür.
5. Tekli olay görünürlüğü ≤1 s, 30 tur kesintisiz retained disk ≤1,3×,
   motor-only ingest/idle private ≤60/15 MiB ve oracle/kurtarma
   kapıları aynı. Toplu yazıda 1 saniye üstü adet mevcut tanı göstergesi.
   Olumsuz kol da tutulur. Bekleme sadece başka aşamaya veya sonraki
   commit'e kayarsa aday varsayılan yapılmaz.

İlk ölçüm hedefi flush sonrası ikinci merge beklemesi. Daha sık görülen
ilk normal merge join ve WAL prefix beklemesi ayrı deneyler olarak açık.
