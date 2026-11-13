# Reclaim — 100’lük commit kuyruk tanısı, 50k

**Kanıt.** Reclaim açıkken profillenen >1 s commitlerin 62/66’sında en
büyük aşama worker join’i. En kötü commit iki revizyon işçisini toplam
3,92 s bekledi. Kapalı profilli kolda 1.300 commit içinde >1 s örnek
çıkmadı. [Ham aşama dökümü](benchmarks/reclaim-profile-50k-20261112/analysis.json)

## Ölçüm düzeni

Onay penceresi 15:16:13–16:01:13 UTC, 12 Kasım 2026. Aynı Cap200k
Türkçe korpus SHA `881756DA…`, ilk 50.000 belge, sayısal anahtarlar, aynı
bir okuyucu ve bir akışlı yazıcı. Tur başına 100 adet 100’lük commit ve
aynı tur sonu bakımı. FDB’nin sahipliği doğrulanmış sabit slotu kullanıldı.
Her kol aynı temiz durumdan kuruldu. SQLite için önceki LIVE-DISK ölçümü
referans olarak korundu. 10. turda aynı cached I/O ve proses geçişi var.
Üretim motoru, format ve seçenekler için kaynak değişikliği yok.
[Onaylı tasarım](RECLAIM-DIAGNOSIS-DESIGN-50K-20261111.md) ·
[Ölçüm penceresi](benchmarks/reclaim-profile-50k-20261112/window.json)

A1/B1/B2/A2 reclaim açık, B1 ve B2 profilli. Ayrı C kolu aynı düzenle
reclaim kapalı ve profilli. Aynı geçici release ikilisi. Yazma izindeki
`operation_sequence`, bağımsız geçici zaman çizgisindeki sequence ve
benchmark commit dönüşü eşleştirildi. 128 commit ve 512 işçi kaydı/tur.
Çizgideki iç içe fazlar ayrık süreye çevrildi. İşçinin süresi yalnız
commitin gerçekten beklediği join aralığında foreground’a yazıldı.
İşçi ayrıntıları output ID ile bağlandı ve alt fazları örtüşebilir.
Thread CPU Windows user+kernel zamanı ve kısa aralıkta kaba çözünürlüklü.
GC/snapshot ve workspace/bütçe beklemeleri ayrı izlenir. Tüm işletim
sistemi beklemeleri veya her metadata cache mutex’i kapsanmaz.
[Geçici kod farkı](benchmarks/reclaim-profile-50k-20261112/probe.diff.txt) ·
[İşletim ve sequence kontrolü](benchmarks/reclaim-profile-50k-20261112/run.py) ·
[Ayrık faz hesabı](benchmarks/reclaim-profile-50k-20261112/analyze.py)

## ABBA ve tarihsel sequence eşlemesi

| Kol | Reclaim / profil | N | p50 / p95 / p99 / max (ms) | >1 s |
| --- | --- | ---: | ---: | ---: |
| A1 | Açık / kapalı | 1.300 | 10,64 / 204,35 / 1.958,06 / 3.723,62 | 42 |
| B1 | Açık / açık | 1.300 | 10,43 / 161,45 / 1.715,65 / 5.452,84 | 35 |
| B2 | Açık / açık | 1.300 | 9,71 / 126,59 / 1.735,23 / 3.378,56 | 31 |
| A2 | Açık / kapalı | 1.300 | 10,08 / 130,24 / 1.804,24 / 2.827,79 | 35 |
| C, ayrı blok | Kapalı / açık | 1.300 | 9,64 / 32,75 / 742,15 / 959,54 | 0 |

Profille birlikte p50 yakın kaldı, p99 ve >1 s sayısı daha düşük, max
iki eşleşmede de daha yüksek çıktı. İşçi takvimi ve kuyruk dağılımı
değiştiği için ayrı bir sabit profil maliyeti çıkarılamıyor. Tek ikili
ve ters sıra, fazların hangi sürede işlediğini gösterir.

| Sequence / tarihsel tur | Önceki açık (ms) | A1 / A2 profilsiz (ms) | B1 profilli | B2 profilli | C kapalı profilli |
| --- | ---: | ---: | --- | --- | --- |
| 693 / tur 6, grup 93 | 3.938,73 | 1.068,46 / 8,74 | 1.332,64 ms. Run indeksi 661,64, revizyon planı 514,51, prefix join 61,17 ms. | 830,86 ms. Run indeksi 461,03, revizyon planı 264,27, prefix join 33,70 ms. | 566,16 ms. Run indeksi 459,76, prefix join 32,14 ms. |
| 1324 / tur 13, grup 24 | 4.585,44 | 1.387,57 / 1.009,94 | 1.612,90 ms. Prefix join 656,54, revizyon planı 476,13, run indeksi 397,95 ms. | 1.605,09 ms. Prefix join 760,91, revizyon planı 463,12, run indeksi 291,86 ms. | 754,70 ms. Prefix join 430,25, run indeksi 259,00 ms. |

**Sinyal.** Seq 1324 iki profilli tekrarda aynı üç fazı benzer sırayla
gösterdi. Seq 693’ün aynı yazı grubu iki profilde de run indeksi ve
revizyon planı ağırlıklı, birinde 1 s altı. **Bilinmiyor.** Tarihsel 3,94
ve 4,59 s olayları aynı sequence’lerde yeniden görülmediği için eski
olaylara bu aşama süreleri atfedilemez. [Tarihsel 30 tur](LIVE-DISK-50K-20261111.md) ·
[Sequence başına ham eşleme](benchmarks/reclaim-profile-50k-20261112/analysis.json)

## Reclaim açık — uçların mekanizması

**Kanıt, ölçülen 66 adet >1 s commit.** En büyük tek aşama 42’sinde
revizyon join’i, 20’sinde prefix join’i ve 4’ünde foreground indeks
hazırlığı. İki join türü toplam duvar süresinin **%59,27’sini** aldı.
Revizyon planlaması **%18,78**, run indeks işi **%16,12**. Ölçülen WAL
fsync **%0,05**, GC **%0,62**, manifest **%0,44**, ayrı ölçülen kilitler
**%0,02**. WAL fsync toplamı en kötü 8,56 ms/commit, GC 19,29 ms ve
ölçülen kilit beklemeleri 14,99 ms. Her commitin dökümü ham sonuçta.
[Açık kol aşama verisi](benchmarks/reclaim-profile-50k-20261112/analysis.json)

**Kanıt, B1 seq 724.** 5.452,84 ms’lik committe prefix job 670 için
531,71 ms bekleme görüldü. Prefix install sırasında başlatılan revizyon
job 673, aynı `stream.worker_drain` içinde 2.492,51 ms beklendi. Delta
flush sırasında başlatılan job 677, `stream.post_flush_drain` içinde
1.423,17 ms beklendi. İki revizyon işi de aynı committe başladı ve
iptal edilmiş iş olarak dönmedi. Bu iki join aralığında işçi pacing’i
yaklaşık 1.487 ms, indeks kurma/açma 1.961 ms. Bu süreler işçilerin
join içindeki parçalarıdır. Uçlar arasında en az iki anlamlı revizyon
join’i olan 7 commit var. [B1 tur 7 ham iz](benchmarks/reclaim-profile-50k-20261112/B1-churn-01-10.jsonl)

### Açık ve kapalı karşılaştırması

Yüzdeler belirtilen grubun toplam commit duvar süresindeki ayrık paylar.
C kolunda >1 s aşama dağılımı yok. En yavaş beş örnek **sinyal** olarak
ayrı sütunda, 810–960 ms aralığında gösteriliyor.

| Aşama | Açık >1 s, N=66 | Kapalı >1 s, N=0 | Kapalı en yavaş 5 |
| --- | ---: | ---: | ---: |
| Revizyon worker join | %39,62 | — | %0 |
| Prefix worker join | %19,65 | — | %53,19 |
| Normal merge worker join | %0 | — | %0 |
| Foreground revizyon planlama | %18,78 | — | %0 |
| Foreground run indeks hazırlama/açma | %16,12 | — | %37,32 |
| Foreground run kaynak yazma/CRC ve hazırlık | %0,60 | — | %1,21 |
| WAL fsync | %0,05 | — | %0,13 |
| Diğer dosya sync | %0,26 | — | %0,96 |
| Manifest | %0,44 | — | %0,70 |
| Dosya GC | %0,62 | — | %1,27 |
| Ölçülen kilitler | %0,02 | — | <%0,01 |
| Snapshot maskesi | %2,25 | — | %0,95 |

**Sinyal, kapalı kol.** Eski kapalı koşuların ilk 13 turunda 26 ve 22 adet
>1 s commit vardı. İkisinde de ilk 13 turun en uzun commit’i seq 624,
1.395 ve 1.272 ms. Bu tekrarda aynı sequence yine en uzun, **959,54 ms**.
Prefix join **693,90 ms**, foreground run indeksi **204,80 ms**. C’nin
en yavaş beş örneğinin dördünde prefix join, birinde indeks hazırlığı
en büyük aşama. Tarihsel >1 s örneklerin tam aşama payı **bilinmiyor**.
[Kapalı ham iz](benchmarks/reclaim-profile-50k-20261112/C-churn-01-10.jsonl) ·
[Tarihsel kapalı örnekler](benchmarks/live-disk-50k-20261111/summary.json)

**Sonuç.** Açık kolun yeni uçlarında başlıca neden join, ardından
foreground revizyon planlama ve indeks hazırlığı. WAL fsync, ölçülen
GC ve kilitler uçların baskın fazı değildi. Seq 724, aynı commit içinde
iki yeni revizyon işinin kuyruğa girip beklendiğini doğruluyor. Tarihsel
iki açık olayın tam nedeni ile eski kapalı >1 s olaylarının aşama payları
bilinmiyor. C, bir ayrı blok ve yeni nedensel ON/OFF performans kapısı
değil.

## Yazı yolundan idle’a taşıma görüşü

`light_flush_ready` ve streamed fallback, normal merge seçimi için
`start_compaction_mode(true)` çağırabilir. Küçük WAL katmanı açıkken ve
uygun normal merge yokken bu yol senkron `start_revision_reclamation`
planlamasına girer. Planlayıcı `revision_masks` ile eski slotları ayırır,
tombstone’un daha eski bir run’da bariyer olarak gerekip gerekmediğini
arar, liveness tarar ve aday seçer. Revizyon işi başlatılmışsa kaynak
doğrulama/kopyalama/indeks işçidedir. Fallback çağıran thread’de iptal
sinyali verebilir ve `wait_for_compaction()` ile bekleyebilir. Hazır bir
revizyon sonucu da committe install, manifest, snapshot ve dosya GC’si
yapabilir. Normal merge ve prefix bu işlerden bağımsız başka bekleme
yollarıdır. [Streamed fallback](../src/engine/streamed.rs#L403) ·
[Otomatik seçim](../src/engine.rs#L2631) ·
[Revizyon planlayıcı](../src/engine/revisions.rs#L95) ·
[Kurulum](../src/engine.rs#L2783)

**Kod görüşü.** Otomatik seçimdeki revizyon fallback’i explicit
`maintain_idle()` yoluyla sınırlandırılabilir. Bu, otomatik revizyon
planlamasını ve bu kaynaktan doğan işçi beklemelerini committen taşımaya
adaydır. `maintain_idle()` içindeki explicit döngü zaten
`start_compaction_mode(false)` çağırır. Normal prefix/merge beklemeleri
ve yeni commitin kendi manifesti ayrı kalır. Dışarıdan async reclaim işi
başlatılırsa onun install/join koşulları ayrıca ele alınır. Taşımada
görünürlük maskesi ve tombstone bariyerleri korunmalı, bakım slotu
gerçekten ayrılmalı ve disk tavanı aynı canlı yükte yeniden ölçülmeli.
Yoğun yazıda yeterli idle yoksa diskin aynı sınırda kalacağı bilinmiyor.
Dosya GC’sini ayrıca taşımak, pinli dosyaların bırakma ve yeniden deneme
sözleşmesini koruyan ayrı bir davranış kararı olur.
[Idle döngüsü](../src/engine.rs#L2884) ·
[Dosya GC ve pinler](../src/engine/snapshot.rs#L108)

## Doğrulama ve kapanış

ABBA 52 tur ve 5.200 commit. Ayrı C kolu 13 tur ve 1.300 commit. C
kararında yaklaşık 23 dakika kalmıştı, güvenli ayırma tahmini 8,2 dakika.
Beş kol 15:21:52–15:42:09 UTC arasında tamamlandı. Profilli kollarda
**3.900/3.900** commit, kayıp çerçeve 0 ve kayıp işçi kaydı 0. Her kolda
50.000 tam GET/version, 32.500 silinen anahtar, 41 exact sorgu ve 24
örnekli sürüm işareti geçti. Beş model SHA’sı ve 41 sorgu imzası aynı.
Sabit FDB slotu son C kolunda `ready`, sequence 1400. Üretim kaynakları,
0.1 fixture’ı ve korpus SHA’ları eşit. Bu açılış kontrolleri sıcak koşu
normalizasyonuna aittir. 23 run açılış farkının yeni faz ölçümü veya uzun
snapshot deneyi alınmadı. [Yürütme ve oracle durumu](benchmarks/reclaim-profile-50k-20261112/status.json) ·
[Önceki 23 run ayrımı](RECLAIM-DIAGNOSIS-DESIGN-50K-20261111.md)

Geçici derlemede `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
ve `cargo test --lib` geçti, 162 test geçti ve 13 ignore. Yeni geçici
klasör `C:\Users\Serkan\AppData\Local\Temp\fdb-reclaim-profile-20261112`
**572.910.618 B (546,37 MiB)**. Kanıt klasörü 41,52 MiB. Önceki geçici
klasörler korunuyor. Genel varsayılan **kapalı** ve karar kullanıcıda.
[Birim testleri](benchmarks/reclaim-profile-50k-20261112/unit-tests.log) ·
[Clippy](benchmarks/reclaim-profile-50k-20261112/clippy.log)
