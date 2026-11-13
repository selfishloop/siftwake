# Blok 2b — beş dakikalık ek preflight

Kullanıcı onayı: yalnız File Summary tanık yolu ve en fazla bir purge dizisi.
Başlangıç **2026-11-04 17:27:24 UTC**, üst sınır **17:32:24 UTC** (300 s).
Önceki iki preflight tarihsel olarak korunur; ana koşu/ETW yok.

File Summary ve Total/Standby/Modified/Modified No-Write/Active sütunları
görüldü. Ctrl+F yalnız Find Next açıyor; bütün sonuçları filtreleyen bir
kontrol görülmedi. Kullanıcının `\data\benchmark-work\` araması purge
öncesinde **Cannot find string** döndürdü. Aramanın tüm tam yolları/162 dosyayı
kapsadığı doğrulanmadı; bu sonuç resident=0 kanıtı sayılmadı.

Gözlem sırasında `no monitor found for window` ve `foreground window did not
report a process id` hataları alındı. Eski pencere kayboldu; süreç tanığı yeni
RAMMap PID'leri48176/53632 ve yeni pencere2360714 gösterdi. Yeniden başlama
nedeni **bilinmiyor**. Yeni pencere seçildi; eski koordinatlar kullanılmadı.
Yönetici pencereye klavye/tıklama otomasyonu güvenilir çalışmadı; kullanıcı
elle yardımcı oldu. Yeni helper veya gözlem aracı yazılmadı.

**Tek purge dizisi denendi (1/1):** yerel Usage'dan önce doğrulanan `-Es`
17:31:42.114–17:31:45.804 UTC, ardından `-Et`
17:31:45.873–17:31:48.503 UTC. İki RunAs parent prosesi **exit0** döndü.
Bu çıkış kodları hedef resident0 veya fiziksel cold kanıtı değildir.
İkinci purge yapılmaz. Sonrasında DB hash/GET/SQL/benchmark çalıştırılmadı.
Kullanıcıdan F5 ve aynı hedef araması istendi; taze tam-dosya tanığı henüz yok.

**Gerçek cold kanıtlanamadı; ana kapı ÖLÇÜLMEDİ.** ETW kaydı0; önceki WPR
yetenek kontrolü gerçek file/PID eşlemesi veya kayıp-event0 ispatı değildir.
SQLite önceki read-view önkoşulu bu ek turda tekrar çalıştırılmadı.
Kod/DB/model/varsayılan değişmedi.2b ana koşu ayrıca onay bekler;
3a KISMEN/özgün kapı ölçülmedi,2c/4 ve önceki açık işler aynı.

Kanıt: [süre/son durum](benchmarks/cold-preflight3-20261104-summary.json),
[purge çağrıları](benchmarks/cold-preflight3-20261104-purge.json),
[yeni süreç tanığı](benchmarks/cold-preflight3-20261104-process-end.json).

**Süre ihlali:** son pencere gözlemi17:32:34 UTC'de tamamlandı:310/300 s, **10 s aşım**. Bu kontrol sınır içinde bitti diye sunulmaz. Ardından yalnız bu rapor kaydı kapatıldı; yeni GUI/DB/purge işlemi yok. Son ekranda Find açıktı, File Summary sayıları önceki snapshot ile aynıydı; F5 tamamlanması/taze tam kapsam doğrulanmadı. Sonuç **yarım / cold kanıtlanamadı**, duruldu.

## Sonraki kullanıcı destekli kontrol / kabul — tarihsel süreyi değiştirmez

Daha sonraki ayrı konuşma adımlarında kullanıcı RAMMap'i açtı; görünen dosya
adı ve dar Path sütununda gizlenen yol kısmı pozitif bulundu. Kullanıcı F5
sonrası hedef-prefix aramasının baştan Down ve sondan Up sonuç vermediğini
bildirdi. Gönderdiği görüntüde prefix,Up ve kapalı iki kutucuk görüldü;
bulunamadı sonucu kullanıcı bildirimidir. Kullanıcı **o görüntüde hedef
klasöre ait RAM sayfası bulunmadı** bulgusunu kabul etti. Bu, önceki purge
anının geriye dönük kanıtı veya süre aşımının kaldırılması değildir.

Sonraki [tasarım revizyonu](BLOCK2B-COLD-CACHE-REDUCED-DESIGN-50K-20261104.md)
küçük DB dosyasını okuyup görünür kılma/purge sonrası kaybolma zincirini ve
8-trial ana matrisi teklif eder; yürütme onayı henüz yok.
