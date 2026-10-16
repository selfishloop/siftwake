# rkyv payload prototipi — 2026-10-09

Bu deney yalnız payload kodlamasını ölçer. `experimental-rkyv-payload` default
kapalıdır; `Database` bu modülü çağırmaz. JSON kaynak, WAL, manifest, SEG03,
protokol ve `.idx` yolları değişmedi. SDB/Fundops entegrasyonu yapılmadı.

**Karar: reddedildi.** Disk ve RAM kapıları geçti; bütün sorgularda üç çiftte
tekrarlanabilir contains kazancı kapısı geçmedi. Medyanların daha iyi olması
tek başına kabul edilmedi. Deney default'a veya üretim motoruna alınmadı.

## Birincil sonuçlar

100k gerçek uzun metin, sıcak cache, CRC ve yapısal/kimlik doğrulaması açık;
sorgular toplam sayım + ilk 50 tam belgeyi içerir. Üç prosesin medyanı:

| Ölçüm | JSON | rkyv | rkyv değişimi |
|---|---:|---:|---:|
| Payload export boyutu | 2.740.862.279 B | 2.711.444.148 B | −%1,07 |
| Tepe proses working set | 15,668 MiB | 11,980 MiB | −%23,53 |
| Örneklenen tepe private bytes | 13,281 MiB | 9,555 MiB | −%28,06 |
| Tam doğrulamalı export açılışı | 5.883,36 ms | 2.591,67 ms | −%55,95 |
| `the` | 15.359,66 ms | 13.160,09 ms | −%14,32 |
| `abbreviation` | 207,05 ms | 178,57 ms | −%13,76 |
| `of` | 15.219,70 ms | 12.997,25 ms | −%14,60 |
| `the whale` | 12.672,47 ms | 10.558,96 ms | −%16,68 |

Disk boyutu iki codec'te aynı framing'i içerir; ortak corpus/oracle/kaynak
dosyaları dahil değildir. Küçülme 29.418.131 bayttır. Açılış JSON'da
5.152–5.969 ms, rkyv'de 2.555–2.693 ms aralığındadır. Bu açılış payload
export'unun tamamını doğrular; FDB'nin indeksli açılışı değildir.

### Tekrar kontrolü

Her hücre JSON → rkyv, ms. Çift 0 ve 2 JSON önce, çift 1 rkyv önce çalışır.

| Sorgu | Çift 0 | Çift 1 | Çift 2 |
|---|---:|---:|---:|
| `the` | 11.597,38 → 13.160,09 | 15.359,66 → 12.756,63 | 15.800,39 → 13.629,33 |
| `abbreviation` | 148,76 → 178,57 | 211,30 → 175,31 | 207,05 → 195,64 |
| `of` | 11.928,80 → 13.075,40 | 15.219,70 → 12.786,97 | 16.229,59 → 12.997,25 |
| `the whale` | 10.158,87 → 11.175,46 | 12.672,47 → 10.558,96 | 13.969,53 → 8.236,27 |

İlk çiftte dört sorgunun tamamı geriledi; diğer iki çiftte iyileşti. JSON
proseslerinin CPU süresi de yaklaşık 76,55 → 93,00 → 98,19 saniye değişti;
rkyv 76,38–76,66 saniye aralığındaydı. Bu değişimin nedeni bu deneyde
izole edilmedi; CPU frekansı/ısıl durum kanıtı olarak yorumlanmaz. Daha düşük
medyanları kesin ve tekrarlanabilir hız artışı diye sunmuyoruz.

Kabul kuralı: disk medyanı ve proses working set medyanı gerilemeyecek;
dört sorgunun her biri eşleştirilmiş üç turda da daha hızlı olacak.
İlk iki kapı **geçti**, son kapı **kaldı**. Ham sonuçlar ve oracle/gate
kontrolü [özet JSON](benchmarks/rkyv-payload-v1-20261009.summary.json) içindedir;
özet ilgili altı `.jsonl`/`.memory.json` dosyasını içerir.

## Ayrı doğrulama maliyeti profili

Birincil altı proses tamamlandıktan sonra aynı binary ile `profile-json` ve
`profile-rkyv` çalıştırıldı. Her kayıtta ayrı saat okunur; bu profilli süreler
kabul kapısına katılmaz. Bir warmup ardından tek örnek (`repeat=1`), ms:

| Sorgu / codec | Okuma | CRC | JSON çözme veya bytecheck+graph+kimlik | Fold+contains | Tam sayfa dönüşümü | Toplam |
|---|---:|---:|---:|---:|---:|---:|
| `the` / JSON | 677,65 | 1.421,24 | 2.570,18 | 6.892,48 | 0* | 11.659,04 |
| `the` / rkyv | 654,74 | 1.396,28 | 589,88 | 6.732,45 | 0,18 | 9.394,88 |
| `abbreviation` / JSON | 75,05 | 9,98 | 19,75 | 47,73 | 0* | 153,14 |
| `abbreviation` / rkyv | 76,78 | 9,76 | 4,44 | 48,36 | 0,28 | 139,89 |
| `of` / JSON | 666,87 | 1.421,96 | 2.499,15 | 6.844,25 | 0* | 11.525,84 |
| `of` / rkyv | 651,13 | 1.400,09 | 583,87 | 6.707,55 | 0,20 | 9.364,03 |
| `the whale` / JSON | 647,81 | 1.217,88 | 2.017,79 | 5.930,75 | 0* | 9.879,58 |
| `the whale` / rkyv | 626,75 | 1.198,13 | 469,58 | 5.831,63 | 0,22 | 8.142,53 |

\* JSON zaten her adayı tam çözdüğü için ayrı sayfa decode işlemi yoktur;
bu maliyet JSON çözme sütunundadır. Toplam ayrıca döngü/sonuç toplama/bırakma
ve ölçüm overhead'ini içerir; sütunların toplamına tam eşit olması beklenmez.
CRC iki codec'te açıktır ve yapısal doğrulamadan ayrı ölçülür. Yapısal rkyv
doğrulaması tek başına bytecheck değildir; graph/alan sırası/key/version kontrolü
de bu sütundadır. CRC-only veya unchecked rkyv hız sonucu yayımlanmadı.

Bu profilde owned JSON çözmeyi kaldırmak doğrulama sütununu belirgin azaltır.
Fakat rkyv yaygın sorgu toplamının yaklaşık %72'si hâlâ normalizasyon+contains,
yaklaşık %15'i CRC'dir; dar sorgunun yaklaşık %55'i aday okumasıdır. Payload
codec'i bu maliyetleri ortadan kaldırmaz. Bu bulgular sonraki çalışmanın nerede
ölçülmesi gerektiğini gösterir; birincil koşulardaki farkın nedenini kanıtlamaz.
Profilde daha iyi çıkan toplamları kabul kararını değiştirmek için kullanmadık.
Ham [JSON profil](benchmarks/rkyv-validation-profile-20261009-json.jsonl) ve
[rkyv profil](benchmarks/rkyv-validation-profile-20261009-rkyv.jsonl) kayıtları
warmup ve measured satırlarını, oracle doğrulamasını içerir.

## Uygulama ve doğrulama

`src/rkyv_payload.rs` title/text, key, fields, links ve version'ı kodlar.
Fields, JSON DOM yerine düz postorder `null/bool/i64/f64/string/list/map`
düğümleridir. i64 varyantındaki unsigned etiketi pozitif u64 değerlerini,
`u64::MAX` dahil, bit kaybı olmadan korur. JSON sınırında geri dönüşüm vardır;
normal sorguda fields/links owned belgelere dönüştürülmez.

Her okumada payload CRC, güvenli `rkyv::access` bytecheck'i, graph sınırları,
derinlik, tek sahiplik, alan sıralaması ve key/version kimliği doğrulanır.
UTF-8 ve göreli işaretçiler bytecheck tarafından denetlenir. Contains yalnız
archived title/text/key/version'a erişir; ilk 50 sonuç tam belgeye çevrilir.
Doğrulama sırasında bütün archive yapısı incelenir: "tam belgeyi çözmeme",
doğrulamayı atlama anlamına gelmez. Güvenli API:
[rkyv 0.8.18 access](https://docs.rs/rkyv/0.8.18/rkyv/fn.access.html).

Benchmark'ın `PLOADB01` taşıyıcısı, ortak framing/CRC ile iki codec'i saklayan
atılabilir `.payload-bench` dosyasıdır. FDB kaynağı değildir; `.fdb`/`.ready`
yerine geçmez. Mevcut kalıcı format değişmediği için SEG04 yayımlanmadı.
Üretime geçiş ayrı kaynak sürümü ve uyumluluk çalışması gerektirir.
Tek yeni Cargo feature eklendi. Diğer deneyler bu codec yolunu kullanmadığından
onları kaldırmak veya birleştirmek bu dar deneyin kapsamını genişletirdi.

## Corpus ve sorgu sözleşmesi

Seed **20261009**, **100.000** belge, UTF-8 gövdeler **2.048–51.200 bayt**;
toplam gövde **2.655.160.287 bayt**. Sekiz farklı Project Gutenberg kitabından
alınan gerçek İngilizce metin pencereleridir. 100.000 gövdenin SHA-256'ı farklıdır;
pencereler örtüşebilir. Bunlar 100.000 bağımsız kitap/kaynak değildir ve karma
dilli üretim corpus'u temsil etmez. Türkçe/Unicode doğruluğu ayrıca test edilir.
Kaynak URL'leri, raw/body hash'leri ve uzunluklar
[provenance](benchmarks/rkyv-corpus-20261009.provenance.json) dosyasındadır.

Belge başlıkları `Passage {id}`; kaynak adı fields içindedir. Böylece yaygın
terimi başlıkta bularak uzun gövdeyi atlayan bir iş yükü oluşmaz. İki codec'in
key/title/text/fields/links/version değerleri aynıdır. Adaylar corpus hazırlama
sırasında ortak trigram koşullarıyla çıkarılır; iki karakterde bütün belgeler
adaydır. Bağımsız oracle toplam sayıyı, ilk 50 tam belgeyi, sırayı ve version'ı
doğrular. Nadir kelime sıfır sonuç vermeyecek şekilde corpus'tan seçilir.

| Sorgu | Metin | Aday | Gerçek sonuç |
|---|---|---:|---:|
| Yaygın | `the` | 100.000 | 100.000 |
| Dar | `abbreviation` | 529 | 264 |
| İki karakter | `of` | 100.000 | 100.000 |
| AND | `the whale` | 75.664 | 16.537 |

Contains normalizasyonu motorun aynı `index::fold` fonksiyonudur; tüm terimler
title veya text içinde bulunmalıdır. Özel hızlı ASCII yolu eklenmedi.

## Ölçüm yöntemi ve sınırlar

Release binary SHA-256:
`A5A34DD536AAD35BC20CF165DD2FF1C53BE4BB12C91327301C01328D32F4474A`.
JSON ve rkyv aynı binary'de, diğer experimental/diagnostic feature'lar kapalıdır.
Üç bağımsız proses çifti sırasıyla JSON/rkyv, rkyv/JSON, JSON/rkyv çalıştırılır.
Birincil ölçümler sırasında build/test veya profilli çalışma yapılmaz.
Makine: AMD Ryzen 7 7840HS (8 core/16 thread), Windows 11 Pro 10.0.26200,
yaklaşık 15,31 GiB görünür RAM. Sorgu yolu tek iş parçacığıdır.

Tam açılış bütün kayıtların doğrulanmasını ve konum listesinin kurulmasını
içerir. Sorgular sıcak OS cache amaçlı tam açılış + bir untimed sorgudan sonra
ölçülür; cache zorla boşaltılmaz. Bu sonuçlar soğuk disk gecikmesi değildir.
Yaygın/iki karakter/AND her proseste bir timed örnek, dar sorgu 20 timed örnek
üretir. Rapor üç prosesin medyanını kullanır; tek örnekli sorgulardaki raw p95
alanı kuyruk gecikmesi istatistiği olarak yorumlanmamalıdır.

Her sorgu aday payload okuma, CRC, codec doğrulama/JSON parse, contains, toplam
sayım ve 50 tam sonuç üretimini kapsar. İndeks/aday üretimi, DB açılışı, WAL,
flush/merge ve protokol maliyeti kapsam dışıdır. Sonuçlar tam FDB veya SQLite
hız karşılaştırması değildir. İki codec aynı 1 MiB okuma tamponunu kullanır.

RAM, Windows proses `PeakWorkingSet64` yüksek su seviyesidir; oracle/adaylar,
tam açılış ve sorgular dahildir. 20 ms hedef aralıkla ayrıca private bytes
örneklenir; bu sayaç tepeyi kaçırabilir. OS dosya cache'i proses working set'e
dahil değildir. Archive boyutu veya iç sayaç toplam proses RAM'i gibi sunulmaz.

Payload üretim ham süresi test çalışmalarıyla örtüştüğünden yazma/encode hız
kabulü için kullanılmaz. Bu deneyin yazma bulgusu dosya boyutuyla sınırlıdır.

## Doğruluk ve hata testleri

- `cargo test --features experimental-rkyv-payload`: **55 geçti**, 1 child helper
  normal suite'de ignored; ebeveyn process-kill testi tarafından ayrıca çağrıldı.
- `cargo test --all-features`: **98 geçti**, 2 ignored helper/test.
- `cargo fmt --check` geçti. Başlangıçta `tests/engine.rs` zaten bu kontrolü
  geçiyordu; bu dosyada değişiklik gerekmedi.
- `cargo clippy --all-targets -- -D warnings`, rkyv feature'lı ve all-features
  varyantları geçti.

JSON↔archive testi tüm değer türlerini, i64 sınırlarını, u64 maksimumunu,
negatif sıfırı, nested list/map, kaçışları, Türkçe/CJK/emoji, links ve version'ı
JSON byte eşitliğiyle kontrol eder. Bytecheck'ten geçebilen fakat anlamsal olarak
bozuk graph'lar da reddedilir. Her export byte kesilmesi ve her byte konumundaki
tek-bit bozulması her iki codec'te reddedilir; açılıştan sonra payload bozulması
CRC ile yakalanır. Ham archive kesilmesi ve yanlış hizalama güvenli erişimde
reddedilir.

Çocuk proses sync edilmiş tek kayıttan sonra öldürülür: beklenen tek kayıtlık
export okunur; iki kayıt bekleyen yarım export reddedilir. Bu güç kesintisi veya
WAL dayanıklılığı kanıtı değildir. 0.1 JSON fixture'ının kopyası gerçek DB ile
okunur, dört belge ve Türkçe sorgu doğrulanır; kaynak byte'ları ve repository'deki
fixture değişmez. Sağlam cache'e güvenerek bozuk kaynağı kabul eden yol eklenmedi.

Test ham kayıtları: [seçili feature](benchmarks/rkyv-selected-tests-20261009.log),
[bütün feature'lar](benchmarks/rkyv-all-tests-20261009.log).

## Yeniden çalıştırma

```powershell
python tools/prepare-payload-corpus.py
cargo build --release --example measure_rkyv_payload --features experimental-rkyv-payload
# Binary'yi output/fdb-rkyv-payload-v1-20261009.exe olarak kopyala ve hash'i kaydet.
./output/fdb-rkyv-payload-v1-20261009.exe generate data/rkyv-corpus-20261009
./tools/measure-payload-codecs.ps1 -Prefix yeni-olcum-adi
python tools/summarize-payload-codecs.py docs/benchmarks/yeni-olcum-adi
./output/fdb-rkyv-payload-v1-20261009.exe profile-json data/rkyv-corpus-20261009
./output/fdb-rkyv-payload-v1-20261009.exe profile-rkyv data/rkyv-corpus-20261009
```

Corpus hazırlayıcı mevcut row planını korur; uzak kaynakların değişmediğini
provenance hash'leriyle kontrol et. Generate `create_new` kullanır: mevcut export'u
ezmez; yeni dataset dizini seç veya yalnız bu atılabilir dosyaları açıkça taşı.
Ölçüm script'i de mevcut çıktı prefix'ini ezmez. Profilleri birincil ölçümlerden
sonra çalıştır. Kalıcı FDB dosyalarına bu komutlar yönlendirilmez.
