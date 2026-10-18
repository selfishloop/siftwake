# Doğrulanmış payload tekrar kullanımı varsayılan motorda — 18 Ekim 2026

Kullanıcının «varsayılan motora» talebiyle, daha önce ölçülen checked-page
algoritması normal `bounded-engine` profiline alındı. Standart derleme ve
`Database::search` bunu otomatik kullanır. Yeni feature, bağımlılık, public
API, payload codec veya kalıcı format yoktur; referans
`--no-default-features` motoru eski yolu kullanır.

Bu, önceki bütün-sorgu/OS RAM kabul kapısının geçtiği yönünde bir karar
değildir. Kalan ölçüm farkları açık tutularak kullanıcı talebi uygulandı.
Önceki reddetme raporları tarihçe olarak korunur; buradaki karar onları
varsayılan durum bakımından günceller. Başlangıç commit'i `440c52e`.
[Karar ve arşiv SHA](benchmarks/default-checked-page-20261018-decision.json).

## Değişen davranış ve sınırlar

Grouped hydration, offset=0, limit<=50, sorgu metni<=4096 UTF-8 byte ve en
az bir terimi üçten fazla Unicode scalar olan sorgular bu yolu kullanır.
Sorgu sözcüklerinin adına göre seçim yapılmaz. Contains doğrulaması pozitif
olan aynı payload slice'ı, sahipli ve değişmez bayt olarak saklanır; global
Key sırasında en fazla 50 kayıt/2 MiB kodlanmış payload baytı tutulur.
Map düğümleri, Key'ler ve çözülmüş sonuç belgeleri bu 2 MiB bütçeye dahil
değildir; bu bir toplam sorgu veya proses RAM sınırı değildir.

Sonuç hazırlanırken bu baytların ikinci dosya okuma/CRC'si kaldırılır;
schema/UTF-8 çözülürken tekrar kontrol edilir. Candidate CRC/schema/UTF-8/
key/version ve authoritative kaynak doğrulaması korunur. Yeni dosya
okumaları daima kendi doğrulamasından geçer; indeks sağlam olsa bile bozuk
kaynak kabul edilmez. Run/slot kimliği uyuşmazsa hata döner. Güncelleme,
silme ve delta görünürlüğü korunur. Sayfa dışı kopyalar fallback belge
hazırlığından önce bırakılır. Büyük payload, bütçe sınırı ve uygun olmayan
sorgular mevcut okuma yoluna döner.

Runtime ve beş test, önceki
[arşiv patch'in](benchmarks/checked-page-20261018-prototype.patch) aynı
algoritmasıdır; ters-patch kontrolü geçti. Sonuç Vec kapasitesindeki bilinen
fazlalık bu geçişte değiştirilmedi. Önceki aynı-program heap profilinde
50 sonuçta 52 yerine 66 kapasite, 14×176=2.464 B ek canlı heap ölçülmüştü.
Bu açıklama OS working set/private commit farkının tamamını kapsamaz.
[Önceki kapasite kanıtı](REUSE-CONTROLS-10K-20261018.md).

## Doğrulama

| Kontrol | Sonuç |
|---|---:|
| `cargo test --all-targets` | 185 geçti, 7 ignored |
| `cargo test --no-default-features --test engine` | 17 geçti |
| All-features checked-page birim testleri | 3 geçti |
| All-features bozulma/kesilme capture testi | 1 geçti |
| All-features sıra/görünürlük/filtre/sayfa testi | 1 geçti |
| `cargo fmt --check` | Geçti |
| Default, all-features ve no-default-features Clippy, all-targets, `-D warnings` | Geçti |

All-features ve referans profillerin tam test takımları bu geçişte yeniden
koşulmadı; yukarıdaki hedefli testler koşuldu. Default tam takımda mevcut
tek-byte bozulma, transaction kesilmesi, source/index bozulması, 0.1 fixture
ve süreç sonlandırma/kurtarma testleri de geçti.
[Komutlar, toplamlar ve log yolları](benchmarks/default-checked-page-20261018-checks.json).

Normal, sayaçsız release okuyucuda aynı ilk 10.000 belge bağımsız oracle'a
karşı 38 sorgunun bütün sayfaları/title/text/fields/links/version ve sırası
ile doğrulandı. Bu kontrolün süreleri performans sonucu sayılmaz. Normal
CLI stats ve quixote aramasında sequence 20, total 56, sıralı 50 sonuç ve
version kontrolü geçti. Yeni profilde varsayılan tekrar kullanımın gerçekten
çalıştığı ayrıca debug/diagnostic build'de doğrulandı: quixote üç tekrarın
her birinde 50 reuse, 56 candidate CRC ve 0 ikinci decode/CRC. The/he/the-man
fallback'lerinde reuse yoktur. Diagnostic debug süreleri normal release hız
sonucu olarak kullanılmaz.
[Runtime kanıtı](benchmarks/default-checked-page-20261018-runtime-proof.json),
[normal CLI/okuyucu SHA'ları](benchmarks/default-checked-page-20261018-binaries.json).

## Ölçüm düzeni

Mevcut `data/rkyv-corpus-20261009/json.payload-bench` içindeki aynı ilk 10.000
belge: sekiz kitap, 264.842.858 text byte, belge başına 2.057–51.197 byte.
Sabit `data/benchmark-work/fdb` yeniden kullanıldı; SQLite sabit dosyası
yalnız SHA kontrolünde yer aldı. Yeni corpus, hazırlama, DB kopyası,
reset/repack veya başka diske taşıma yapılmadı.

Normal release A/B, eski frozen sayaçsız okuyucu ile bu geçişin normal
okuyucusudur; aynı executable içindeki on/off deneyi değildir. Dört süreç
çifti, çift başına iki dengeli sıralı blok, blok başına 10 tekrar; aynı 38
sorgu her turda çalışır: sorgu ve motor başına 80 örnek. Affinity mask 4,
sıcak OS dosya cache, strict source/index açılışı ve ısınma saat dışında;
eviction yok. Query hazırlığı ve arama saat içinde, bağımsız tam belge/
version oracle kontrolü ve sonuç bırakma saat dışında. İki oracle worker
resident kalır, DB'yi bloklar sırayla ve tek tek açar; eşzamanlı DB sorgusu
yoktur. Worker/oracle belleği motor RAM ölçümü sayılmaz.

Ayrı OS bellek kontrolü dört fresh süreç çifti, aynı sırayla dönüşümlü
çalıştırılır. Oracle yüklenmez: süreç başına 24 arama/771 get; total/key/
order/version doğrulanır, tam payload eşitliği yukarıdaki ayrı 38 sorgu
oracle kontrolüyle sağlanır. OS lifetime peak private commit ve working
set kaydedilir; private commit fiziksel RAM değildir ve hiçbir sayaç
toplam sistem belleği olarak sunulmaz. Bu ölçümler yazma, eşzamanlı veya
gerçek-soğuk cache yükünü kapsamaz; donanım/güç/arka plan tam izolasyonu
iddiası yoktur.

## Bu varsayılan derlemenin normal release sonuçları

38 sorgunun 20'sinde pooled p95 düştü, 18'inde yükseldi. Bu, bütün sorgularda
hızlanma kabulü değildir. Aşağıdaki p95 ve eşlenmiş blok medyanı ayrı
dağılımları özetler; birinin düşük olması ötekinin artışını kapatmaz.

| Sorgu (offset/limit) | Önce p95 ms | Varsayılan p95 ms | p95 fark | Tipik blok farkı | Hızlı süreç çifti |
|---|---:|---:|---:|---:|---:|
| quixote (0/50) | 3,6434 | 3,4021 | -%6,62 | -%18,40 | 3/4 |
| quixote whale (0/50) | 3,7551 | 3,3411 | -%11,03 | -%11,20 | 4/4 |
| holmes (0/50) | 32,8859 | 32,9209 | +%0,11 | -%2,38 | 1/4 |
| beagle (0/50) | 24,9539 | 28,5300 | +%14,33 | -%2,01 | 0/4 |
| harpoon (0/50) | 68,9153 | 79,1913 | +%14,91 | -%0,75 | 1/4 |
| he (0/50) | 1,2446 | 1,3041 | +%4,78 | +%1,46 | 1/4 |
| zx (0/50) | 169,7235 | 173,5388 | +%2,25 | +%0,33 | 3/4 |
| uzun negatif (0/50) | 0,0509 | 0,0480 | -%5,70 | +%11,42 | 2/4 |
| the (0/200) | 5,1788 | 5,6386 | +%8,88 | +%6,38 | 1/4 |
| ishmael (130/1) | 7,3728 | 8,3400 | +%13,12 | -%0,37 | 1/4 |

Quixote tipik blokların 8/8'inde hızlı. Beagle p95 dört çiftte de yüksek;
negatif ve sayfa maliyetleri dahil bu artışlar «dalgalanma» diye kapatılmadı.
Uzun negatifte pooled p95 düşük olmasına rağmen tipik blok 8/8 yüksek.
İki farklı executable'ın bu sonucu, önceki tek-program/off0/off1/on
[sakin tekrarın](REUSE-REPEAT-10K-20261018.md) yüzdeleriyle birleştirilmez;
kod yerleşimi, sorgu geçmişi ve ortam payları tam ayrılmamıştır.

[38 sorgunun bütün sonuçları](benchmarks/default-checked-page-reads-10k-20261018-summary.json),
[ölçüm düzeni ve executable SHA'ları](benchmarks/default-checked-page-reads-10k-20261018-design.json).

| Ayrı oracle'sız proses ölçümü, dört çiftin medyanı | Önce | Varsayılan | Fark |
|---|---:|---:|---:|
| Lifetime peak private commit | 8,7090 MiB | 8,7363 MiB | +28 KiB (+%0,31) |
| Lifetime peak working set | 12,5859 MiB | 12,6797 MiB | +96 KiB (+%0,74) |

Private commit 3/4, working set 4/4 çiftte arttı. Bu fresh ölçüm önceki
+64 KiB/+170 KiB sonucunun yerine geçer; RAM gerilememesi kapısı geçti
denmez. Vec kapasitesinin 2.464 B etkisi ayrı önceki requested-heap ölçümüdür;
bu OS farkıyla aynı sayaç değildir.
[Ham sayaçlar ve karşılaştırma](benchmarks/default-checked-page-memory-10k-20261018-summary.json).

## Veri ve kaynakların korunması

Başlangıç/son 21 dosya SHA-eşit: 18 FDB dosyası 349.805.698 B, SQLite 583.860.224 B
ve 0.1 fixture'ın iki dosyası aynı. Corpus'un tamamı 2.740.862.279 B ve
SHA-256 `11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B`
ile önceki kanıta eşit. Tam corpus hash taraması performans koşuları bittikten sonra
yapıldı. Cargo/feature/dependency ve fixture değişikliği yok; runtime
arşiv algoritmasına eşit. Sadece mevcut iki veri klasörü vardır.
[Final kanıtı, kaynak/binary SHA'ları ve ortam](benchmarks/default-checked-page-20261018-proof.json).

## Açık işler

Öncelikli küçük takip: sonuç Vec kapasitesindeki 2.464 B fazlalığı azaltıp
ortak boş/negatif yolun maliyetini ölçmek. Bunlar bu geçişte çözülmüş
sayılmaz. Beagle/harpoon ve sayfa p95 artışlarının payı da açık. Yazma/bakım,
diğer sorgu regresyonları, normal Vec/query/caller
RAM, genel açılış, disk sıkıştırma ve eşzamanlı/gerçek-soğuk yük açık.
[Güncel iş listesi](OPEN-WORK.md).
