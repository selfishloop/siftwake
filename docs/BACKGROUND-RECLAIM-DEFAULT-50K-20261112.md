# Arka planda revizyon geri kazanımı — varsayılan karar

12 Kasım kullanıcı kararıyla ilk aday varsayılan oldu. Önceki iki
geçici adayın karşılaştırmasında arka plan hazırlığı 30 turun tamamında
disk kapısını geçti, yazıcıda dilimli aday üç turda kaldı.
[İlk aday](BACKGROUND-RECLAIM-50K-20261112.md) ·
[İki adayın karşılaştırması](SLICED-RECLAIM-50K-20261112.md)

## Motor davranışı

- Yeni yazıcı oturumunda otomatik reclaim açık. `set_revision_reclamation(false)`
  oturum için kapatır ve yayımlanmamış otomatik işi iptal eder.
- Tek revizyon işçisi immutable run'lardan planı çıkarır, kaynak ve indeks
  doğrulamasını yapıp standart çıktıyı hazırlar. Aktif WAL tombstone
  atmak için kanıt oluşturmaz. Arka plan sonucu yalnız planın sıralı
  run öneki hâlâ aynıysa yayımlanır. Sonradan eklenen run kabul edilir.
- Normal commit bitmemiş revizyon işçisini join etmez. Hazır çıktı yazıcı
  tarafından manifest ve snapshot üzerinden kurulur. Kaynak ve indeks
  çalışma alanı bütçeleri normal işler ve revizyon arasında ortak.
- Canlı döngüde yazı yokken `poll_maintenance()` bir adım ilerletir.
  Tamamlanmış işin yayımı ve emekli dosya temizliği yazıcı adımıdır.
  `wait_for_compaction()` ve `maintain_idle()` açıkça beklemeli işlemler.
  Eski snapshot'lar kaynak pin'lerini bırakana kadar dosyalar korunur.
- WAL, manifest, payload ve indeks biçimleri aynı. Normal merge, WAL
  prefix ve revizyon işleri ayrı. Frunner uygulamasına entegrasyon yok.

## Doğrulama

Mevcut 881756DA… Türkçe korpusun ilk 50 bin belgesi, mevcut sahipli
FDB benchmark slotu, bir kısa snapshot okuyucusu ve bir akışlı yazıcı.
Temiz başlangıç seed'inde kontrol amacıyla reclaim kapalı, ölçüm kolu
motoru yeniden açar ve reclaim setter'ı çağırmadan varsayılanı kullanır.
Her turda %20 churn, 100 adet 100'lük commit ve 2 saniye yazmasız
pencere. 41 sorgu döngüsü, doğal sıcak cache. Yeni veri korpusu veya
SQLite koşusu yok. Önceki prototip ayrı koşu, karşılaştırma bağlamı.
[Sürücü](benchmarks/background-reclaim-default-50k-20261112/run.py)

| 50k sonucu | Üretim varsayılanı | İlk prototip |
| --- | ---: | ---: |
| Tur sonu retained disk ≤1,3× | **GEÇTİ 30/30** | GEÇTİ 30/30 |
| En yüksek / son tur oranı | 1,281× / 1,212× | 1,290× / 1,183× |
| Son disk / örnekli geçici tepe | 364,70 / 453,33 MB | 355,83 / 449,01 MB |
| Son run / ölü payload / tombstone | 52 / 16,50 MB / 1.745 | 53 / 8,74 MB / 0 |
| 100'lük commit p95 / p99 / p99,9 / max (ms) | 53,31 / 898,93 / 1.617,96 / 2.093,48 | 47,62 / 1.012,51 / 1.794,25 / 2.075,47 |
| 100'lük commit >1 s | 23/3.000 | 33/3.000 |
| Yazı / yazı ve boşluk dahil (işlem/s) | 2.180 / 1.518 | 1.976 / 1.416 |
| Yazı altında 41 sorgu karışımı p95 (ms) | 89,16 | 98,59 |
| Proses tepe private / working set (MiB) | 85,50 / 86,06 | 89,73 / 90,25 |

Başlangıç retained disk 300,84 MB. En yüksek tur sonu 13. turda
385,27 MB. Geçici tepe 50 ms örneklemeli. Yazı duvarı 137,65 s,
yazmasız pencereler 60,04 s. 25. tur sequence 2511 en uzun 100'lük
commit. Aşama profili alınmadı, uç nedeni bilinmiyor. Bellek
harness ve model dahil proses ölçümü. Sorgu karışımı ölçümü her
sorgu için ayrı bir kapı uygulamıyor. Bu tur yeni dosya olayı
görünürlük serisi içermiyor. Uzun snapshot ve hiç boşluk vermeyen
yazıcı altında disk tavanı açık.
Ham tur ve sorgu örnekleri yerel `disk-30.jsonl` kaydında tutulur.
[Özet](benchmarks/background-reclaim-default-50k-20261112/summary.json)

| Doğrulama | Sonuç |
| --- | --- |
| Paused revizyon işçisiyle varsayılan commit | **GEÇTİ** · 129 işlemli akışlı fallback, işçi serbest bırakılmadan döndü |
| Bayat plan, opt-out ve eski snapshot | **GEÇTİ** · değişen run öneki ve iptal edilmiş çıktı atıldı, eski snapshot pin'i korundu |
| Yayımdan önce process-kill | **GEÇTİ** · eski manifest ve silme bariyeri korundu, yetim çıktı bakımda temizlendi |
| 50k commit sonrası process-kill → strict reopen | **GEÇTİ** · sequence 3101, tam GET/version, silinen kayıtlar, 41 sorgu ve marker oracle |
| Dondurulmuş eski okuyucu | **GEÇTİ** · 50.000 belge ve 41 sorgu digest'i eşit |
| Fmt, Clippy ve tam test paketi | **GEÇTİ** · Clippy `-D warnings`, 237 test geçti, 16 yardımcı test doğrudan çağrıda atlandı |

Ölçüm ve yeniden açılış 20:36:59–20:41:49 UTC. FDB slotu
`ready`, SQLite slot kaydı, korpus, 0.1 fixture ve önceki model
hash'leri aynı. Üç eski testin sabit iş sırasını korumak için
otomatik revizyonu kendi fixture'larında açıkça kapattık. Yeni
varsayılanın akışlı yazı, iptal, pin ve kesinti testleri ayrı.
Varsayılan özellikler olmadan kütüphane kontrolü de geçti.
Bu yapılandırmada `Pages::empty` için kullanılmayan işlev uyarısı var.
[Yürütme ve oracle](benchmarks/background-reclaim-default-50k-20261112/status.json) ·
[Hedefli testler](benchmarks/background-reclaim-default-50k-20261112/targeted-tests.log) ·
[Tam test paketi](benchmarks/background-reclaim-default-50k-20261112/tests.log) ·
[Fmt ve Clippy](benchmarks/background-reclaim-default-50k-20261112/final-checks.log)
