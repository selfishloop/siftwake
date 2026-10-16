# Chunk kopyasını azaltma deneyi — reddedildi

İki prototip ölçüldü; ikisi de genel hız kapısını geçmedi ve runtime
değişiklikleri geri alındı. Önceki [WIDE-FOLD](WIDE-FOLD-10K-20261014.md)
uygulaması korunur. Yeni feature/kütüphane/kalıcı format yok; default aynı.
SDB/Fundops/Frunner entegrasyonu yok.

## Ne denendi?

Eski uzun-metin yolu 1024 baytlık UTF-8 parçasını önce `scratch.text` içine
normalleştirip sonra `scratch.window` overlap tamponuna kopyalıyordu.

1. `fold_append`, normalleştirilmiş parçayı doğrudan window'a ekledi. Eski
   prefix byte'larını korudu; ASCII lowercase sadece eklenen suffix'e uygulandı.
   Böylece uzun chunk'ta ara String ve ikinci kopya kalktı.
2. İlk varyantın AND gerilemesi üzerine doğrudan yol yalnız **tek kalan metin
   parçasına** uygulandı. Birden fazla residual parça eski iki-tampon yolunda
   kaldı. Ayrım kelime adına dayanmaz; covering sonrası kalan parça sayısıdır.
   `the zx`te `the` kanıtlanınca `zx` tek kalır; `quixote whale` iki kalır.

Kaynak/payload CRC, checked schema, key/version, visibility ve source guards
atlatılmadı. Normalize edilmiş çıktı bağımsız fold oracle'ıyla aynıydı.
Prototype Unicode/all-scalar, prefix preservation, UTF-8/chunk sınırı, uzun
term, AND ve reuse testleri geçti. İkinci varyantta tek parça için ara
`scratch.text` kapasitesinin sıfır kaldığı da doğrulandı. Doğruluk başarısı
performans başarısı olarak kabul edilmedi.

## İki ayrı 10k ölçümü

Her varyant kendi üç paired proses grubunda önceki aynı frozen wide-fold
reader'a karşı ölçüldü. İkisi release/experimental-builder-budget/record-read;
diagnostic kapalı. Aynı mevcut ilk 10.000 belge, aynı sabit FDB slotu, aynı
30 query + 8 page case. İkinci çift ters sıra; query/get sırası seed ile
karıştırılır. Temel case20 sample/proses (60 pooled), sayfa5 (15 pooled),
nearest-rank p95. Sıcak OS cache, manuel eviction yok. Tam total/sıralama/
belge/version oracle'ı geçti. FDB/cache ve SQLite DB hash'leri aynı.
SQLite bu adımda yeniden ölçülmedi. Oracle resident olduğu için bu
proseslerin RAM'i motorun proses RAM'i olarak sunulmaz.

### 1. Bütün uzun parçalar doğrudan window'a

`append-fold-matrix-10k-20261015`: **32 p95 düşük /6 yüksek.**

| Query / offset / limit | Kontrol p95 | Prototip p95 | Değişim | Hızlı çift |
|---|---:|---:|---:|---:|
| zx /0 /50 |380,4674 ms |342,7662 ms |-%9,91 |2/3 |
| the zx /0 /50 |389,8291 ms |339,3669 ms |-%12,94 |3/3 |
| quixote /0 /50 |6,6063 ms |5,2324 ms |-%20,80 |1/3 |
| quixote whale /0 /50 |7,0937 ms |7,5293 ms |+%6,14 |0/3 |
| man /0 /50 |3,0598 ms |3,2335 ms |+%5,68 |1/3 |
| quixote /57 /50 |4,4150 ms |5,7163 ms |+%29,47 |1/3 |

`the zx` üç çiftte hızlıydı; `quixote whale` üç çiftte de yavaştı.
Bu geniş varyant genel çözüm olarak reddedildi.
[Ham toplu sonuç](benchmarks/append-fold-matrix-10k-20261015-summary.json).

### 2. Yalnız tek residual metin parçası doğrudan window'a

`append-fold-single-matrix-10k-20261015`: **21 p95 düşük /17 yüksek.**

| Query / offset / limit | Kontrol p95 | Prototip p95 | Değişim | Hızlı çift |
|---|---:|---:|---:|---:|
| zx /0 /50 |278,3152 ms |270,3368 ms |-%2,87 |2/3 |
| the zx /0 /50 |281,4831 ms |272,7072 ms |-%3,12 |2/3 |
| quixote /0 /50 |4,9457 ms |4,4022 ms |-%10,99 |2/3 |
| quixote whale /0 /50 |5,1532 ms |5,6985 ms |+%10,58 |0/3 |
| frankenstein /0 /50 |20,9964 ms |24,0246 ms |+%14,42 |0/3 |
| the /0 /200 |5,6176 ms |6,1079 ms |+%8,73 |0/3 |
| the /9950 /50 |2,3144 ms |2,8199 ms |+%21,84 |0/3 |

AND eski buffer yoluna dönmesine rağmen üç çiftteki gerileme çözülmedi;
ek ayrım da genel hedefi karşılamadı. İkinci varyant reddedildi.
[Ham toplu sonuç](benchmarks/append-fold-single-matrix-10k-20261015-summary.json).

Kontrol executable'ı iki batch'te aynı olsa da kontrolün mutlak süreleri farklı:
örneğin `zx` pooled380,47 ve278,32ms. Bu yüzden ikinci batch'in mutlak
süresi ilkine karşı optimizasyon kazancı gibi sunulmaz. Karşılaştırmalar her
batch'in kendi eşleştirilmiş kontrolüyledir. OS/CPU/cache koşullarının tümü
sabitlenmedi; kesin varyans sebebi bu deneyde ölçülmedi. Covering nedeniyle
gövde taramayan `he` gibi case'lerin düşüşü chunk kopyası kazancı sayılmaz.
Eski kullanıcı dalgalanma kabulü yeni artışları kapatmak için kullanılmadı.

## Veri, RAM, disk ve sınırlar

Bu adımda DB sıfırlanmadı, yazma/indeks oluşturma veya yeni RAM checkpoint
koşusu yapılmadı. Mevcut 10k/seq20/delta0 DB yeniden kullanıldı; yeni corpus
üretilmedi/indirilmedi. Kalıcı boyutlar/hash'ler iki ölçüm boyunca aynı.
Önceki9,14MiB indexing private/13,34MiB WS tek teyidi yeni bir ölçüm değildir.
Query tepe, bakım/yazma, eşzamanlı ve soğuk-cache performansı ölçülmedi.

Ayrı read-only corpus sayımı (`append-fold-chunk-mix-20261015.json`), aynı
1024-bayt UTF-8 sınırlarıyla first10k/264.842.858 B metni inceledi; bütün row
CRC'leri kontrol edildi.59.305 ASCII chunk58.119.077 B (%21,94),204.322 mixed
chunk206.723.781 B (%78,06). Bu sayım benchmark ile eşzamanlı çalıştırılmadı.
Başka metin hazırlamadı. Corpus İngilizce ağırlıklıdır; sonuçlar bütün dil ve
iş yüklerine genellenmez.

Kesin çıkarım: ikinci kopyayı mekanik olarak kaldırmak, bütün contains yolunu
tekrarlanabilir biçimde hızlandırmadı. Kopya/normalleştirme/derleyici yerleşimi
payları ayrı paired mikro ölçümle ayrılmadı; AND gerilemesi tek nedene bağlanmaz.
Önceki profildeki büyük normalleştirme maliyeti hâlâ açık iş; bu deneyde o
maliyetin kaldırıldığı iddia edilmez.

## Geri alma ve doğrulama

`fold_append`, ortak helper refactor'u, parça-sayısı dalı ve sadece prototipe
ait buffer testi kaldırıldı; önceki fold_into ve iki-tampon chunk döngüsü
geri geldi. `src/index.rs` SHA256 önceki wide-fold snapshot'ıyla aynıdır.
Önceki kirli çalışma ağacı korunur; git reset/restore/init kullanılmadı.
Prototip executable'ları ve başarısız kapı logları kanıt olarak tutulur.

Prototip test logları `append-fold-unit-20261014.log` ve
`append-fold-single-unit-20261015.log`. Nihai rollback default/builder-budget/
all-features all-targets suites geçti; kesilme, tek bayt bozulması, process
kill/merge kill, kaynak-cache ve eski JSON fixture kontrolleri başarılı.
Default/all-features `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check` ve diff-check geçti.0.1 fixture değişmedi.
Loglar `append-fold-rollback-tests-{feature,default,all-final}-20261015.log`,
`append-fold-rollback-clippy-{default,all}-20261015.log`,
`append-fold-rollback-fmt-20261015.log`.

Normal builder-budget release examples geri alınan kodla yeniden build
edildi; son38-case correctness kontrolü geçti
(`append-fold-rollback-matrix-check-20261015.jsonl`). Source/normal executable
hash'leri `append-fold-rollback-final-artifacts-20261015.json` içindedir.
İki benchmark'ın ardından ve final reader sonrasında bütün FDB/cache/SQLite
hash'leri aynı; bütün mevcut corpus SHA256 değeri de önceki kayıtla aynıdır.
Final rebuild/check yeni performans veya RAM ölçümü değildir.

[OPEN-WORK](OPEN-WORK.md): chunk kopyası çözüm olarak kapatılmadı; kalan CPU
normalleştirme/tarama, mevcut arama artışları, yazma/bakım, normal API/genel
heap/query tepe RAM'i, disk/eski nesiller ve eşzamanlı/soğuk yük işleri açık.
