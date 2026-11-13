# Blok 3a çalışma alanı onarımı — 3 Kasım 2026

**TAMAMLANDI.** Yalnız kullanıcıca onaylanan temizlik/doğrulama yapıldı.
Yeni ölçüm, başarısız harness'in düzeltilmesi/yeniden koşulması, bakım çağrısı
veya payload optimizasyonu yok. Sabit FDB/SQLite slotları **ready**.

Her motorda `shared/keybench-numeric/0000000000..0000000015` için **önce 16
tam GET** yapıldı. Gövde, başlık, fields, links, key ve version mevcut corpus
donörü ve tanı marker'ıyla doğrulandı; tamamı silme öncesi kaydedildi.
Yalnız bu16 anahtar, motor başına tek atomik silme commit'iyle silindi.
FDB'de version guard; SQLite transaction'ında ilk DELETE'ten önce16 tam
kaydı tekrar karşılaştırma kullanıldı. Ana50k'ya Put/Delete gönderilmedi.

| Motor | Silme öncesi gerçek reopen sequence | Hazırlık sequence'i | Gözlenen hazırlık-sonrası commit | Silme sonrası / model sequence | Havuz |
|---|---:|---:|---:|---:|---|
| FDB | 529966 | 529902 | 64 | **529967** | 0 |
| SQLite | **1026310** | 1026309 | **1** | **1026311** | 0 |

SQLite'taki bir güncelleme tahmin değildir: reopen GET'te slot0 marker1 ve
version1026310; diğer15 kayıt marker2 ve version1026309 çıktı. FDB'nin16
version'ı önceki başarılı commit kayıtlarıyla birebir eşleşti.
Model dosyalarında yalnız global sequence değişti; rows/dead/next ve bütün
belge version'larının modeldeki değerleri korunmuş olarak doğrulandı.

| Doğrulama | FDB | SQLite |
|---|---|---|
| Silme öncesi16 GET / silme sonrası yokluk | GEÇTİ | GEÇTİ |
| Yeni açılışta ana50k tam GET/version +41-case oracle/digest | GEÇTİ | GEÇTİ |
| Process kill → yeniden açılışta aynı tam kontroller | GEÇTİ | GEÇTİ |
| Eski okuyucu / karşılaştırıcı | GEÇTİ: 50k GET/version +41 sorgu | GEÇTİ: 50k GET/version +41-case oracle |
| Model sequence = gerçek reopened DB sequence | GEÇTİ | GEÇTİ |

41-case mantıksal hit digest'i kill öncesi/sonrası aynı:
FDB `5A57ECDC5C15AE71…`, SQLite `207A0FCCB8A935C3…`.
FDB eski okuyucusunda aynı hit digest'i elde edildi; eski SQLite karşılaştırıcısı
41 exact-total oracle'ını da geçti. İki motorun DF digest'i aynı
(`C596DE96F954E061…`); hit version'ları motorlara ait olduğundan çapraz hit
digest eşitliği istenmedi. Tam SHA256'lar ve hesaplanan tablolar
[özet kanıtta](benchmarks/block3a-repair-20261103-summary.json).

Kill tanıkları silme commit'i ve tam oracle sonrasındadır; in-flight veya
güç kesintisi iddiası yok. Eski FDB okuyucusu24 Ekim, SQLite karşılaştırıcısı31
Ekim frozen ikilileridir; SHA'ları kayıtta. Oracle çalışmaları performans
ölçümü değildir; N60/A-A/p95 veya yeni performans kararı üretilmedi.

Root fmt ve all-targets clippy `-D warnings`; izole SQLite repair tool clippy
`fdb/bounded-engine` ile geçti. İlgili gerçek DB doğrulama/kurtarma kontrolleri
yukarıdadır. Engine,0.1 fixture ve eski tanı helper/driver dahil80 korunan
dosyanın SHA'ları ve881756DA… corpus'u değişmedi. Ayrı onarım aracı eklendi;
başarısız ölçüm kodunun pin/client davranışına dokunulmadı.

**3a = KISMEN, tanı yeterli sinyal verdi; özgün kapı (zx/the zx ≥%30,
diğer36 ±%5 + A/A) ÖLÇÜLMEDİ.** Eski44→7→52 run /12→32ms gap gözlemi ve
52-run tanısındaki posting hazırlığı/izin-kilit süreleri ilişkiyi araştırmak
için sinyaldir; A/A ve run değişimli eşleştirme tamamlanmadığından nedensel
kanıt değildir. zx profilinde50000 read/171,044MB,49282 yakın komşu aralık ve
0 window hit görülmesi de payload pencereleri için **sinyal, kanıt değil**;
%30 kazanç veya diğer36 sorguda ±%5 garantisi çıkarılamaz. Bu tur uygulanmadı.

Blok2b/2c/4 ve diğer açık yazma/bakım, arama, RAM/disk/eşzamanlı yük işleri
aynı kalır. Sonraki blok kararı kullanıcıya bırakıldı; başka işe geçilmedi.
Temizlik mantıksal yokluktur; bakım/fiziksel alan geri kazanımı iddiası yok.

Kanıt: [FDB silme öncesi16 tam kayıt](benchmarks/block3a-repair-20261103-fdb-before-delete.json),
[SQLite silme öncesi16 tam kayıt](benchmarks/block3a-repair-20261103-sqlite-before-delete.json),
[bütün aşamalar/SHA/oracle](benchmarks/block3a-repair-20261103-result.json),
[kill/çıkış kaydı](benchmarks/block3a-repair-20261103-lifecycle.jsonl),
[temiz slot kaydı](benchmarks/active-benchmark-slots.json).
