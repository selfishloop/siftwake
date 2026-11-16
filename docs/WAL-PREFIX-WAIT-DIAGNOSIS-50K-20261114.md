# 50k akışlı yazıda WAL prefix beklemesi

Mevcut profilli 50k kayıtlarının çevrimdışı analizi. Cap200k korpusunun
ilk 50 bini, doğal sıcak cache, 1 snapshot okuyucu + 1 yazıcı, 41 sorgu,
100 işlem/commit, 30 tur × %20 churn, tur arasında 2 saniye bakım.
Üretim davranışındaki kontrol A2. Önceki oturumun B1/B2 kolları aynı
yazma ayarlarıyla ayrı tekrarlar. Her kolda 3.000 commit, kayıp frame ve
yardımcı kayıt 0. Yeni benchmark, SQLite kolu, oracle veya kill koşusu
yapılmadı. Önceki koşuların doğrulama kayıtları ilgili raporlarda.
[Kontrol koşusu](NORMAL-MERGE-DEFER-50K-20261114.md) ·
[Önceki tekrarlar](WAL-PREFIX-FOLLOWUP-20261114.md)

## Commit neden kısa yoldan çıktı?

**Kanıt.** A2'de 114 commit uzun yola girdi. En az 50 ms süren 97 prefix
join'in tamamı uzun yolun ilk `stream.worker_drain` aralığındaydı.
86'sında aynı tur içindeki WAL oluşturma ve append sırası üzerinden etkin
durum yeniden kurulabildi. Yirmi başarılı 100'lük grup 2.000 farklı
anahtar bırakıyor. Sıradaki 100'lük grup `shadow + stage = 2.100` ile
2.048 sınırını aşıyor. Her grubun benzersizliği ve ardışık başarılı kısa
commit'ler kontrol edildi. Dosya kopyası veya WAL replay başlatmadan,
mevcut korpustaki 49.437 ilgili girişin CRC'si kontrol edildi. OFF1 ve TXN2
kodunun uzunluk formülleri, harness'in anahtar değiştirme ve marker
ekleme işlemiyle birlikte çevrimdışı uygulandı. Rakamlar arşivden
yeniden hesaplandı.
[Commit koşulu](../src/engine/streamed.rs#L313) ·
[Delta sınırları](../src/engine.rs#L1671) ·
[Prefix kurulumu](../src/engine/light_flush.rs#L147) ·
[Yük üretimi](../tools/bench_support/live_disk50.rs#L43) ·
[OFF1](../src/payload.rs#L87) · [TXN2](../src/binary_batch.rs#L13)

| Kısa yol kontrolü | Sequence 124 | Yeniden kurulan 86 örnek | Sınır |
| --- | ---: | ---: | ---: |
| Grup boyutu | 100 | 100 | ≤128 |
| Shadow + grup anahtarı | 2.100 | 2.100 | ≤2.048 |
| Canlı delta payload | 6.941.087 B | 279.066–7.552.041 B | <16 MiB |
| Kodlanmış grup | 392.856 B | 3.805–471.562 B | ≤8 MiB |
| Mevcut WAL + kodlanmış grup | 7.344.728 B | 356.356–7.948.601 B | ≤64 MiB |

Bu 86 örnek 38,26 s prefix beklemesi içeriyor ve 63 commit 1 s üzerinde.
Dört örnekte etkin WAL'ın oluşturulması commit izinde yok, yedi örnek
bakım aralığını aşıyor. Bu 11 örnekte tetikleyici **bilinmiyor**.
[Sequence bazında yeniden kurulum](benchmarks/wal-prefix-wait-20261114/admission-by-sequence.csv) ·
[Kapsam ve istisnalar](benchmarks/wal-prefix-wait-20261114/admission-summary.json) ·
[Sequence 124 hesabı](benchmarks/wal-prefix-wait-20261114/seq124-admission.json)

## İşçi bekleme sırasında ne yapıyordu?

Prefix join, çıktı run kimliği ve zaman aralığıyla işçiye eşlendi.
İşçi aralıkları join başlangıcı ve sonuna kırpıldı. İç içe bütçe ölçümleri
kendi sütununa alındı ve her zaman dilimi tek kez sayıldı. Yüzdeler her
koldaki ≥50 ms prefix join toplamına ait. 50 ms burada tanı filtresi.

| Kontrol | ≥50 ms join | Join toplamı | Kaynak yazımından önce, ayrıştırılmamış | İndeks hazırlığı, bütçe hariç | Kaynak hazırlığı, bütçe hariç | Kaydedilen bütçe ve workspace | Diğer |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| A2 | 97 | 40,57 s | 21,82 s · %53,8 | 14,63 s · %36,1 | 2,19 s · %5,4 | 1,48 s · %3,7 | 0,45 s |
| B1 | 90 | 31,36 s | 15,29 s · %48,8 | 11,62 s · %37,0 | 1,75 s · %5,6 | 2,34 s · %7,5 | 0,36 s |
| B2 | 88 | 36,29 s | 18,86 s · %52,0 | 13,60 s · %37,5 | 2,15 s · %5,9 | 1,29 s · %3,6 | 0,39 s |

| A2 sequence / işçi run | Commit | Prefix join | Ön aralık | Kaynak | İndeks | Kaynak workspace beklemesi | Sonraki foreground delta flush |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 124 / 276 | 836,32 ms | 585,88 ms | 419,25 ms | 32,27 ms | 129,16 ms | ≈0 ms | 154,01 ms |
| 244 / 351 | 2.009,37 ms | 1.069,05 ms | 356,85 ms | 26,25 ms | 157,00 ms | 524,76 ms | 763,41 ms |
| 3.022 / 2.344 | 1.378,82 ms | 990,39 ms | 760,91 ms | 33,78 ms | 192,01 ms | ≈0 ms | 235,01 ms |

**Kanıt.** A2'de 1 s üstündeki 76 commit'in 71'inde ≥50 ms prefix join var.
Bu 71 commit'in prefix beklemesi toplam 33,75 s. A2'deki 97 anlamlı
beklemenin hiçbirinde işçi, stage hazırlığı sona erdiğinde bitmiş
değildi. Bu örneklerde stage sonuna bir hazır kontrolü eklemek beklemeyi
ortadan kaldırmazdı. Seq 2.511 üç kolda 15,05–19,24 ms ve kısa WAL
yolunda. Önceki 2.511 ucu bu izlerde tekrarlanmadı.
[Ham join eşlemesi](benchmarks/wal-prefix-wait-20261114/join-by-sequence.csv) ·
[Özet ve iz kaynakları](benchmarks/wal-prefix-wait-20261114/summary.json)

**Sinyal.** İlk aralıkta kod, dondurulan WAL satırlarını okuyup CRC,
payload, anahtar ve sürümü doğruluyor, ardından gram ve posting
tablosunu hazırlıyor. Kaynak oluşturma için aynı satırlar tekrar okunuyor.
Ön aralığın duvar süresi ölçüldü. İlk WAL okuma/doğrulama, normalleştirme,
posting ekleme, CPU ve zamanlanma payları ayrı ayrı **bilinmiyor**.
[İşçi döngüsü](../src/engine/light_flush.rs#L53) ·
[WAL satır doğrulaması](../src/wal_delta.rs#L21) ·
[İndeks hazırlığı](../src/index.rs#L575)

## Sonraki dar tanı

Önce işçi hazırlığının maliyetini ayırmak daha güçlü aday. Seq 124'te
işçi, prefix 120 kurulur kurulmaz kalan 1.700 anahtarın kopyasını işlemek
üzere başlatıldı. Yazıcı üç grup daha kabul etti ve dördüncüde sınıra
dayandı. Başlatma eşiğini düşürmek bu sırada ikinci işçi başlatamazdı.
Eşiği daha erken devreye almanın başka yerleşimlerde etkisi ve küçük run
sayısına maliyeti ölçülmedi.
[Başlatma eşiği](../src/engine/streamed.rs#L395) ·
[Tek prefix sınırı](../src/engine/light_flush.rs#L18)

Bir geçici ölçüm derlemesinde ilk üç tur yeterli bir başlangıç olur.
Sequence başına kısa yol koşullarını, prefix başlatma sequence ve
yakalanan anahtar/bayt sayısını, işçi duvar ve CPU sürelerini kaydet.
Belge başına log üretmeden ilk WAL okuma/CRC/decode ile
`add_builder` sürelerini işçi başına biriktir. Bütçe beklemelerini ve
sonraki foreground delta flush'ı ayrı tut. Bu ölçüm, hazırlığı kısaltmaya
mı yoksa eşik ile işçi zamanlamasını incelemeye mi öncelik verileceğini
belirler. Mevcut tanıdan hız kazancı veya yeni eşik çıkarılmadı.

100'lük commit uçları, yazma/bakım maliyeti, geniş arama uçları ve yoğun
okuma altında RAM ayrı açık işler. Eski okuyucu, CRC ve snapshot
doğruluğu olası bir sonraki aday için yeniden sınanmalı.
[Açık işler](OPEN-WORK.md)
