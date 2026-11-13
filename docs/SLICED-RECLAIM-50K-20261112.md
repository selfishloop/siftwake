# Yazıcıda dilimli revizyon temizliği — 50k prototip

İkinci seçenek geçici derlemede denendi. Frunner olay kuyruğu doluyken
yazıcı önce commit'i işler. Kuyruk boşken bir revizyon adımı çalıştırır.
100'lük yazı yükünde adım, commit grupları arasına girer. Revizyon işçisi
açılmaz. Normal WAL prefix ve merge işçileri mevcut davranışını sürdürür.
Üretim ayarı, CRC sözleşmesi ve kalıcı biçim aynı.

## Dilim sınırı ve doğruluk

- Planlama ve paged metadata taraması 8 ms hedefi ve 4.096 kayıt üst sınırı
  ile imleci saklar. Sıralı taramada en yeni kaydı ve aynı anahtarın eski
  immutable run'da bulunup bulunmadığını aynı geçişten alır. Revizyon
  seçimi ve tombstone bariyeri ilk prototiple aynı kurallara dayanır.
- Kaynak CRC'si 64 KiB bloklarla, adım başına en çok 4 MiB. Payload kopyası
  adım başına yaklaşık 1 MiB ve 8 ms hedefi. Son kayıt ve frame encode/write
  bu hedefi aşabilir. Aktif okuyucu pacing'i 32 MiB/s, beklemek yerine sonraki
  hizmet turuna dönülür. Hazırlanan standart checkpoint frame'leri manifestten
  önce yalnız aday dosyada. Kaynak bir kere fsync edilir.
- Giriş indeks CRC'si, kaynak fsync, çıktı indeksini bitirme ve manifest
  yayımı ayrı adımlar. İndeks bitirme, mevcut strict native kaynak doğrulaması,
  posting yeniden eşleme, indeks fsync ve açmayı kapsar. Bu adımlar başladıktan
  sonra tek çağrıda tamamlanır. 8 ms bunlar için garanti değildir.
- Mevcut read pin ve bütçe kuralları korunur. Normal prefix/merge çalışırken
  kopya, indeks bitirme ve yayım ertelenir. Planın dayandığı immutable
  run öneki değişirse aday atılır. Yeni run eklenebilir. Kaynaklar ve aday
  dosya iş sürerken GC'ye karşı korunur. Yazıcı tek başına manifest yayımlar.

## Ölçüm düzeni

Aynı 881756DA… Türkçe korpusun ilk 50.000 belgesi, aynı FDB sabit slotu,
1 kısa snapshot okuyucusu + 1 akışlı yazıcı, 41 sorgu döngüsü, doğal
sıcak cache. Yeni corpus, kopya DB, WPR, ETW ve purge yok.

Olay ölçümü, mevcut 50k slotu sequence 3101'den. Altı çevrim, her çevrimde
%20 churn ve eşlenik 50/500/1.500/3.000 ms dosya olayları. Dilimli aday ve
bakımsız kontrol sırası dönüşümlü, kollar aynı slotta ardışık. Aday
görünürlük hedefi 24/24 olay ≤1 s ve gerçek revizyon adımı örtüşmesi.
Olaydan ilk doğru sonuç p95 ve max; kuyruk, commit, snapshot edinme ve ilk
sorgu ayrı. Ham kayıttaki `background` ve `worker_phase` isimleri ortak
harness'a ait. Bu koşuda dilimli aday ile olay geldiğinde yazıcıdaki adımı
gösterir. Birinci adayın kuyruk başlangıcı sequence 3750 idi. İki aday
arasında olay karşılaştırması tarihsel.

Olay ve doğruluk kapısı geçerse aynı sabit slot, önceki adayın disk koşusu
gibi temiz 50k başlangıçtan 30 tur. Her turda %20 churn ve 100 adet
100'lük commit, ardından 2 s yazmasız pencere. Her tur sonu retained disk
kapısı ≤1,3×. Geçici tepe ayrıca. Süre sınırı 50 dakika, 48 olay, en
çok 30 tur. Birinci aday ve SQLite'ın önceki aynı yük sonuçları referans.
[Birinci aday](BACKGROUND-RECLAIM-50K-20261112.md) ·
[Pencere](benchmarks/sliced-reclaim-50k-20261112/window.json) ·
[Sürücü](benchmarks/sliced-reclaim-50k-20261112/run.py)

## Dosya olayının görünmesi

| Kol | N | Görünürlük p50 / p95 / max (ms) | Kuyruk p95 / max (ms) | Commit p95 / max (ms) | >1 s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Bakımsız kontrol | 24 | 57,63 / 124,59 / 130,30 | 0,06 / 0,07 | 18,92 / 32,79 | 0 |
| Yazıcıda dilimli aday | 24 | 74,81 / 159,42 / 165,96 | 5,01 / 16,51 | 15,76 / 16,27 | 0 |
| Aday, bir hizmet adımıyla örtüşen | 11 | 52,62 / 104,76 / 104,76 | 16,51 / 16,51 | 13,83 / 13,83 | 0 |
| Aday, revizyon hazırlığı veya yayımıyla örtüşen | 3 | 61,66 / 104,76 / 104,76 | 16,51 / 16,51 | 12,46 / 12,46 | 0 |
| Birinci aday, tarihsel | 24 | 43,59 / 127,43 / 129,02 | 43,60 / 44,39 | 12,79 / 14,91 | 0 |

**Görünürlük kapısı GEÇTİ.** 29 plan başlatıldı, 27 çıktı yayımlandı,
2 plan iş bulmadı. İndeks bitirme ile örtüşen sequence 3533, kuyruk
2,28 ms. Kaynak CRC ile örtüşen 3635, kuyruk 0,73 ms. Yayım ile
örtüşen 3749, kuyruk 16,51 ms. Bunlar olay geldiğinde adımın kalan
süresini görür. En uzun indeks adımının başında gelen bir olay örneği yok.

| Yazıcı adımı | Çağrı | Toplam (ms) | En uzun çağrı (ms) |
| --- | ---: | ---: | ---: |
| Plan ve metadata | 844 | 4.305,62 | 24,14 |
| Payload kopyası | 18 | 108,14 | 13,81 |
| Kaynak CRC | 59 | 72,47 | 2,29 |
| Kaynak fsync | 10 | 22,82 | 6,39 |
| Giriş indeks CRC | 27 | 111,75 | 16,36 |
| Çıktı indeksi bitirme | 10 | 1.445,21 | **393,24** |
| Manifest ve snapshot yayımı | 27 | 1.376,00 | 69,14 |
| GC | 27 | 95,54 | 12,29 |

**Kanıt.** Parçalanan kopya ve plan çağrıları bazı örneklerde 8 ms
hedefini aştı. En uzun ölçülen tek yazıcı adımı indeks bitirme.
Görünürlük testi onun tamamını kuyruğa ekleyen bir örnek içermedi.
İlk adayla p95 farkı aynı başlangıç yerleşiminden eşleştirilmedi.
[Ham olay ve sequence'ler](benchmarks/sliced-reclaim-50k-20261112/queue.jsonl) ·
[Aşama özeti](benchmarks/sliced-reclaim-50k-20261112/summary.json)

## 30 tur ve iki seçeneğin karşılaştırması

| Ölçüm | Arka plan hazırlığı | Yazıcıda dilimli |
| --- | ---: | ---: |
| 1 saniyede görünür olan dosya olayları | 24/24 | 24/24 |
| En yüksek / son tur sonu retained disk | **1,290× / 1,183×** | **1,519× / 1,29985×** |
| Disk kapısı, tur sonu ≤1,3× | **GEÇTİ 30/30** | **KALDI 27/30** |
| Örnekli geçici disk tepesi | 449,01 MB / 1,493× | 466,31 MB / 1,550× |
| Son retained disk, başlangıç 300,84 MB | 355,83 MB | 391,04 MB |
| Son run / ölü payload / tombstone | 53 / 8,74 MB / 0 | 92 / 23,89 MB / 4.125 |
| 100'lük commit p95 / p99 / p99,9 / max (ms) | 47,62 / 1.012,51 / 1.794,25 / 2.075,47 | 49,93 / 1.151,78 / 1.335,61 / 1.766,53 |
| 100'lük commit >1 s | 33/3.000 | 80/3.000 |
| Yazı duvarı / 2 s aralar dahil (işlem/s) | 1.976 / 1.416 | 1.780 / 1.307 |
| 41 sorgu karışımı p95, yazı / yazmasız pencere (ms) | 98,59 / 91,12 | 146,65 / 136,79 |
| Tepe OS private commit / working set (MiB) | 89,73 / 90,25 | 101,85 / 101,72 |

Dilimli aday 4, 7 ve 29. turlarda sınırı aştı. Sırasıyla **1,403×,
1,519×, 1,368×**. Son tur 1,3× çizgisinin yaklaşık 44,7 kB altında.
Disk ölçümü ve 100'lük commit karşılaştırması iki adayda aynı başlangıç
boyutu, iş yükü ve sabit slotla ayrı koşularda. Adayda toplam 168,57 s
yazı duvarı ve 60,98 s yazmasız pencere. Yazı duvarı gruplar arası
bakım adımlarını ve model güncellemelerini içerir. Commit dağılımı
yalnız commit çağrıları. SQLite'ın önceki aynı yükte 100'lük commit p95'i
316–322 ms, max 849–914 ms ve >1 s adet 0. Yeni SQLite kolu yok.
Sorgu değerleri karışık 41 sorgu havuzu, ayrı sorgu kapısı uygulanmadı.
Bellek harness, model ve okuyucuyu içeren proses ölçümü.
[Ham disk turları](benchmarks/sliced-reclaim-50k-20261112/disk-30.jsonl) ·
[Disk oranları](benchmarks/sliced-reclaim-50k-20261112/disk-performance.json) ·
[SQLite referansı](LIVE-DISK-50K-20261111.md)

**Kanıt.** Dilimli aday 30 turda 163 çıktı yayımladı. İlk aday 256.
Dilimli adımda en uzun plan 21,87 ms, kopya 17,26 ms, kaynak fsync
33,05 ms, giriş indeks CRC 36,73 ms, çıktı indeksini bitirme **876,36 ms**,
yayım 82,13 ms. 100'lük commit max'i sequence 924, 9. turda 1.766,53 ms.
İndeks bitirme commit çağrısı dışında ölçüldü. Sequence 924'ün iç
aşamaları bilinmiyor. Daha az yayım ve daha fazla run disk farkıyla
birlikte gözlendi. Normal işçilerle yarışın ve dilim bütçesinin ayrı
etkileri ölçülmedi. Bu koşular uzun snapshot, kesintisiz yazı veya gerçek
Frunner anahtarı altında disk tavanı belirlemiyor.
[Aşama sayaçları ve tüm uç commit'ler](benchmarks/sliced-reclaim-50k-20261112/summary.json)

## Doğrulama ve kapanış

| Kapı | Sonuç |
| --- | --- |
| Dosya olayı ≤1 s, gerçek revizyon adımı örtüşmesi | **GEÇTİ** · 24/24 ve üç örtüşme |
| Her tur sonu retained disk ≤1,3× | **KALDI** · 27/30 |
| 50k process-kill → strict reopen ve 41 sorgu | **GEÇTİ** · olay sequence 3750, disk sequence 3101 |
| Dondurulmuş eski okuyucu | **GEÇTİ** · iki kolda GET/version ve 41 digest eşit |
| Dilimli plan, snapshot, eski plan, bozuk kaynak ve process-kill | **GEÇTİ** · dört hedefli test, mevcut plan eşitliği, araya yeni yazı, eski snapshot, CRC reddi ve yayım öncesi kesinti |
| Biçim, Clippy ve tam test paketi | **GEÇTİ** · fmt, Clippy `-D warnings`, 241 test geçti, 17 yardımcı test doğrudan çağrıda atlandı |

Ölçüm ve 50k doğrulaması 19:54:24–20:02:11 UTC. FDB sabit slotu
son modelle `ready`, SQLite slot kaydı aynı. Korpus, 0.1 fixture,
başlangıç modeli ve üretim `src` SHA'ları eşit. Üretim reclaim
varsayılanı **KAPALI**. İki aday için karar açık. Geçici klasör
`%TEMP%/fdb-sliced-reclaim-20261112` ölçüm bitiminde 564,90 MiB,
tam test derlemesi sonrasında 1,81 GiB. Klasör korundu.
[Yürütme, oracle ve SHA'lar](benchmarks/sliced-reclaim-50k-20261112/status.json) ·
[Adayın birinci prototipe göre kod farkı](benchmarks/sliced-reclaim-50k-20261112/candidate.diff.txt) ·
[Dört hedefli test](benchmarks/sliced-reclaim-50k-20261112/targeted-tests.log) ·
[Tam test paketi](benchmarks/sliced-reclaim-50k-20261112/full-tests.log) ·
[Fmt ve Clippy](benchmarks/sliced-reclaim-50k-20261112/compile.log)
