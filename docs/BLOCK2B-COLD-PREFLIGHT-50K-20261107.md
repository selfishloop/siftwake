# Blok 2b ön testi — 7 Kasım 2026

Onaylı plan, tek yönetici PowerShell penceresi ve daraltılmış case0 matrisiydi.
Başlangıç **07:28:53 UTC**, ön test sınırı **07:38:53 UTC**, toplam sınır
**08:08:53 UTC**. [Onaylı tasarım](BLOCK2B-COLD-CACHE-REDUCED-DESIGN-50K-20261104.md)

| Kapı | Sonuç | Kanıt |
|---|---|---|
| Küçük canlı kaynak | GEÇTİ | `generation-0000000000001ff9.ready` kimlik, uzunluk ve CRC kontrolünden geçti. `run-0000000000001c98.fdb` manifestte canlı, 1.853 B. |
| Pozitif okuma | GEÇTİ | Salt okunur buffered erişim 1.853 B okudu ve kapandı. UTC 07:29:56.095, PID 81756. |
| RAMMap pozitif → purge → negatif | TAMAMLANMADI | İstenen aday satırı ve `Total > 0` tanığı gelmedi. Kalan süre, negatif tanık ve ETW kontrolü için ayrılan süreye yetmeden duruldu. |
| ETW dosya/PID ve olay kaybı | ÖLÇÜLMEDİ | WPR durum kontrolü kayıt olmadığını gösterdi. Yerel tracerpt mevcut. ETW uygunluk kaydı için pozitif zincirin geçmesi bekleniyordu. |
| Ana case0 matrisi | BAŞLAMADI | 0/8 cold ve 0/8 warm. Open-start→ilk sonuç ve 30. arama yakınsaması ölçülmedi. |

Bu pencerenin toplamı **0 purge, 0 ETW kaydı**. Pozitif dosya okuması yalnız
ön test erişimidir. Kullanıcıya yönetici komut bloğu gönderilmesi pozitif
tanığa bağlıydı. Motor, sabit DB slotları ve model dosyalarında değişiklik
yapılmadı. 2b açık. 3a kısmi ve diğer açık işler önceki kapsamlarında duruyor.

[Başlangıç ve kapanış](benchmarks/cold-reduced-20261107/summary.json) ·
[Manifest ve okuma kanıtı](benchmarks/cold-reduced-20261107/positive-read.json)
Kapanış **07:38:10 UTC**, toplam **557,279 s**. Ön test sınırı aşılmadı.
