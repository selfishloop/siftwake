# Revizyon geri kazanımı — varsayılan karar özeti

**Mevcut ayar kapalı. Karar kullanıcıda.** Kanıt, Cap200k ilk 50.000 belge,
sayısal anahtar, tek okuyucu, tek akışlı yazıcı, kısa snapshot ve her tur
explicit bakım içeren iki ters sıralı tekrar için geçerli.
[Canlı disk ölçümü](LIVE-DISK-50K-20261111.md)

| Konu | Fayda ve maliyet |
| --- | --- |
| Disk | Açık kollar 30 tur sonunda 361–376 MB, kapalı kollar 1,72 GB. Açık kolların her tur bakım sonrası oranı ≤1,251×, örneklenen geçici tepesi 1,499–1,520×. |
| Yeniden açılış | Son sıcak strict-open 1.138–1.199 → 281–286 ms. İlk sonuç 4,77–5,05 s → 286–292 ms. Temiz başlangıç ve gerçek soğuk açılış ayrı ölçüm konuları. |
| Akışlı yazma | Bakım hariç yazı hızı 2.074 → 1.107 işlem/s, yaklaşık %47 düşüş. Bakım dahil 891 → 680 işlem/s. |
| Kuyruk uçları | 100’lük commit p95 44–48 → 197–204 ms, p99 1,08–1,20 → 2,02–2,39 s ve max 1,29–1,39 → 3,94–4,59 s. Açık kollarda 107–111/3.000 commit 1 s üstünde. En kötü uçların aşama nedeni bilinmiyor. |
| Doğruluk ve kıyas | Kill/reopen 6/6, eski okuyucu 4/4. SQLite 30. turda 516–519 MB. Aktif okuyucu varken `busy` checkpoint görülen turlarda bakım sonrası 3,67–4,63×. |
| Kapsam riski | Uzun snapshot emekli dosyaları tutabilir. 5/30/120 s disk etkisi, gerçek Frunner yol/mtime anahtarları ve 30 tur ötesi davranış ölçülmedi. Bu sonuç sürekli disk tavanı garantisi vermez. |

**Öneri.** Üretim varsayılanı kararından önce commit uçlarının foreground
planlama, worker join’i, I/O veya kilit beklemesi payını ayır. Frunner için
açık ayarın disk ve sıcak açılış kazancı tekrarlanmış durumda. Kuyruğun
en uzun beklemeleri ve uzun pin etkisi pilot ayarı kararına birlikte
girmeli. [Tanı tasarımı](RECLAIM-DIAGNOSIS-DESIGN-50K-20261111.md)

**12 Kasım tanı eki.** Açık profilde >1 s 66 commitin 62’sinde join
baskın. 5,45 s’lik örnek aynı commit içinde iki revizyon işçisi başlatıp
3,92 s bekledi. Ayrı kapalı 13 turda >1 s örnek çıkmadı. En uzun kapalı
örnekte 960 ms’nin 694 ms’si prefix join. Otomatik revizyon işini idle’a
taşıma kod düzeyinde mümkün görünüyor, disk ve gecikme etkisi ölçülmedi.
[Ölçülmüş kuyruk tanısı](RECLAIM-PROFILE-50K-20261112.md)
