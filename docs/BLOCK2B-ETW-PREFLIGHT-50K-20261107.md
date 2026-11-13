# Blok 2b ETW ön testi — 7 Kasım 2026

**Gerçek cold kanıtlanamadı.** Ön test, yönetici bloğundan başlangıç sinyali
gelmeden kapandı. Ana koşu **0/8**.
[Onaylı ön test tasarımı](BLOCK2B-COLD-ETW-DESIGN-50K-20261107.md)

| Aşama | Sonuç |
|---|---|
| Başlangıç ve deadline | **07:59:59 UTC → 08:09:59 UTC**, en fazla 10 dk |
| Dosya kimliği | Canlı FDB run 1.853 B. Manifest CRC geçti. SQLite `sqlite.db` 477.908.992 B. |
| Normal kullanıcı okuyucusu | 08:00:48 UTC'de hazır, PID 82748, Windows oturumu 1 |
| Yönetici PowerShell bloğu | WPR ve iki RAMMap çağrısı için bir blok sunuldu. Bekleme, çözümleme ve kapanış rezervi nedeniyle 08:07:29 UTC'de sonlandı. Okuyucu `Operator start window expired` ile çıktı. |
| Ön ısıtma ve sıcak/cold okumaları | Uygulama okuma fazı 0/3. `reads.jsonl` oluşmadı. |
| Purge ve ETW | 0 purge, 0 kayıt. Kapanış WPR durumunda kayıt yok, ETL oluşmadı. |
| Dosya/PID atfı ve kayıp olay | ÖLÇÜLMEDİ |
| Ana performans | İlk sonuç ve 30. arama yakınsaması ÖLÇÜLMEDİ |

Kayıt için hazırlanan akış tek kullanımlık geçici faz sinyallerine bağlıydı.
Başlangıç sinyalinin neden ulaşmadığı bilinmiyor. WPR veya RAMMap hatası
ölçülmedi. Bu pencerenin bütçesi içinde aynı deneme tekrarlanmadı.
2b açık. Önceki yazma, bakım ve arama açıkları OPEN-WORK kaydında duruyor.

[UTC ve sayaçlar](benchmarks/cold-etw-20261107/summary.json) ·
[Dosya kimliği](benchmarks/cold-etw-20261107/identity.json) ·
[Okuyucu hazırlığı](benchmarks/cold-etw-20261107/reader-ready.json) ·
[Zaman aşımı](benchmarks/cold-etw-20261107/reader-error.txt)
Rapor kapanışı **08:08:27 UTC**, toplam **508 s**.
