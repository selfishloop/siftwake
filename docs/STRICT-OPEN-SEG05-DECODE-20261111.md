# Strict-open SEG05 — terim tamponlarını yeniden kullanma

**Kanıt, sıcak 52 run’lı 50k yerleşim.** İki dönüşümlü tamponla strict açılış
medyanı **385,51 → 365,74 ms** (%5,1), açılıştan ilk sonuç **483,92 → 462,33 ms**
oldu. Her ikisi de **10/10** eşleşmiş blokta hızlandı. Kontrol, önceki sayfa
sınırı iyileştirmesini içeriyor. İki yol da aynı geçici release ikilisinde.
[Önceki deney](STRICT-OPEN-SEG05-TERMS-20261111.md)

## Karşılaştırma

Mevcut Cap200k TR ilk 50.000 belge kopyası, 52 run, 16 aktif delta anahtarı,
159 dosya, generation 8185, sequence 529967. Her blok başında aynı dosyalar
normal cached I/O ile okundu. 10 ABBA/BAAB blokta kol başına 20 yeni proses.
Sorgu `olduğu`, melez N=50. Tüm örneklerde strict Verified, 50 sonuç ve
CRC32 725765487. WPR, ETW ve purge kapalı. Kopya, aktif slot ve model SHA’ları
önce/sonra eşit. Ölçüm 16:10:44–16:11:09 UTC. p95 en yakın üst sıra, 20
örnekte 19. gözlem. [Plan ve ikili SHA](benchmarks/strict-open-seg05-decode-20261111/plan.json) ·
[Ham örnekler](benchmarks/strict-open-seg05-decode-20261111/reuse-samples.jsonl)

| Ölçüm | Sayfa sınırı kontrolü p50 / p95 | + Tampon kullanımı p50 / p95 | Eşleşmiş blok farkı medyanı |
| --- | ---: | ---: | ---: |
| Strict-open, ms | 385,51 / 395,46 | **365,74 / 386,56** | −20,44 |
| Açılış → ilk sonuç, ms | 483,92 / 492,94 | **462,33 / 483,35** | −22,50 |
| Yalnız ilk sorgu, ms | 96,69 / 102,19 | 95,94 / 98,91 | −0,44 |
| Açılış CPU, ms | 609,38 / 625,00 | 562,50 / 593,75 | −39,06 |
| İlk sonuca kadar CPU, ms | 703,13 / 718,75 | 664,06 / 687,50 | −35,16 |
| Terim döngüsü, iki işçide biriken ms | 163,03 / 168,84 | 120,59 / 124,95 | −42,38 |
| Açılış tepe private, MiB | 8,562 / 8,805 | 8,594 / 9,203 | +0,281 |
| İlk sonuca kadar tepe private, MiB | 9,654 / 9,734 | 9,635 / 9,773 | +0,009 |
| Açılış tepe working set, MiB | 11,752 / 12,023 | 11,791 / 12,211 | +0,201 |
| İlk sonuca kadar tepe working set, MiB | 13,619 / 13,715 | 13,646 / 13,699 | +0,021 |

Açılış blok farkı −30,04…−4,62 ms, ilk sonuç −28,13…−5,30 ms. Açılış
tepe private p95 **+0,398 MiB**, ilk sonuca kadar **+0,039 MiB**. CPU süreç
user+kernel toplamıdır. Terim döngüsü işçi sürelerinin toplamıdır. Bellek
göstergeleri yalnız ölçüm prosesine aittir. İlk sorgu blok farkı iki yöne
gitti. Temiz az-run ve gerçek soğuk yerleşim bu ölçümün dışında.
[Blok özetleri ve farklar](benchmarks/strict-open-seg05-decode-20261111/reuse-summary.json)

## Aday ve doğruluk

Önceki terimin artık kullanılmayan `String` ayırmaları güvenli `into_bytes`
ile dönüşümlü bankaya geri alınır. Sınırdaki son terim mevcut fence yoluna
taşınır. İki banka, dört alan ve tampon başına 4 KiB saklama sınırıyla işçi
başına en fazla 32 KiB yeniden kullanım kapasitesi tutar. Daha büyük alanlar
mevcut uzunluk kontrolünden geçer ve tampon bankasında saklanmaz. Dosya biçimi,
CRC, UTF-8 ve posting kontrolleri ile arama semantiği aynı. Aday geçici
derlemede. [Kod farkı](benchmarks/strict-open-seg05-decode-20261111/compact.diff.txt)

Zamansız doğrulamada **789.189/789.189** terim, posting ve son bayt konumu
kontrol çözümleyicisiyle bire bir karşılaştırıldı. **6.174/6.174** sayfa
sınırı alan alan eşit. [Doğrulama kaydı](benchmarks/strict-open-seg05-decode-20261111/fence-and-decode-audit.json) ·
[Envanter kontrolü](benchmarks/strict-open-seg05-decode-20261111/status.json)

Adayla process-kill sonrası reopen ve dondurulmuş eski okuyucuyla 50.000
GET/version ile 41 sorgu digest’i geçti. Geçici derlemede `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` ve üç hedefli SEG05 testi geçti.
İki subprocess yardımcı testi normal listede ignore. Yeniden kullanılan
geçici derleme klasörü `C:\Users\Serkan\AppData\Local\Temp\fdb-seg05-terms-20261111`
doğrulama sonunda 897.217.804 B (855,65 MiB). [Okuyucu doğrulaması](benchmarks/strict-open-seg05-decode-20261111/verification.json) ·
[SEG05 test günlüğü](benchmarks/strict-open-seg05-decode-20261111/test.log)
