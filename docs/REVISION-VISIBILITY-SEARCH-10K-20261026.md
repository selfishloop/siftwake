# Revizyon görünürlüğü / arama — 10k, 26 Ekim 2026

**37 sorgu kapısı: KALDI.** Referansa göre +%5 aşan 6/37. Case34 izleniyor. Revizyon geri kazanımı ve yeni sorgu deneyi varsayılan kapalıdır.

Bu oturumda eski aday da referansa göre 6/37 durumda +%5 üstünde; yeni ek pruning kendi eski adayına göre 14/37 durumda +%5 üstünde. Önceki oturum24→bu oturum6 farkını deney kazancı diye sunmuyoruz. **Ek pruning deneyi reddedildi; varsayılan yapılmadı.**

Kullanıcı önceki bakım sonucunu kabul etti: eski politika yaklaşık36s, aday13,7s, SQLite24–25s. 12s ölçüm hedefi geçmiş gibi yeniden etiketlenmedi. Bu ayrı iş geri kazanma eşiklerini, WAL/manifest/payload/SEG biçimini değiştirmez.

## Ölçülen kök ayrım

Görünürlük maskesi mevcut kodda seed ID döngüsünde, posting kesişimi ve payload doğrulamasından ÖNCE uygulanıyordu. Dolayısıyla ölü kayıtların belge olarak okunup doğrulandığı varsayımı doğrulanmadı. Profil hidden-skip sayısını doğrudan ölçer. Okuma pencereleri komşu byte aralıklarını önceden getirebilir; fiziksel read-ahead içindeki her fazla byte ölü payload diye sınıflandırılmadı.

| Sürüm / sorgu | Seed / maskede elenen | Payload adayı | Okunan / istenen MB | Maske hazırlığı / diğer ms | Posting setup / intersection ms | Payload verify ms | Read / CRC / schema / text ms |
|---|---|---:|---|---|---|---:|---|
| candidate-before / darcy | 3022 / 499 | 2146 | 67.082 / 67.082 | 0.012 / 0.413 | 0.385 / 0.310 | 51.100 | 19.333 / 6.073 / 6.512 / 15.838 |
| candidate-before / the zx | 11951 / 1951 | 10000 | 286.171 / 268.304 | 0.016 / 1.547 | 0.428 / 0.770 | 160.306 | 58.061 / 25.985 / 26.846 / 40.854 |
| candidate-before / quixote whale | 60 / 9 | 41 | 1.573 / 1.573 | 0.006 / 0.074 | 0.641 / 0.012 | 1.201 | 0.297 / 0.156 / 0.139 / 0.173 |
| candidate-before / holmes elementary | 1684 / 247 | 925 | 31.781 / 31.781 | 0.011 / 0.255 | 1.164 / 0.407 | 36.091 | 9.657 / 3.003 / 3.672 / 17.560 |
| candidate-after / darcy | 3022 / 499 | 2146 | 67.082 / 67.082 | 0.010 / 0.342 | 0.360 / 0.253 | 42.749 | 17.094 / 5.023 / 5.092 / 12.789 |
| candidate-after / the zx | 11951 / 1951 | 10000 | 286.171 / 268.304 | 0.013 / 1.586 | 0.442 / 0.776 | 157.673 | 55.437 / 26.309 / 26.774 / 40.459 |
| candidate-after / quixote whale | 60 / 9 | 41 | 1.573 / 1.573 | 0.005 / 0.068 | 0.670 / 0.010 | 1.073 | 0.253 / 0.153 / 0.135 / 0.158 |
| candidate-after / holmes elementary | 1684 / 247 | 925 | 31.781 / 31.781 | 0.009 / 0.250 | 1.243 / 0.400 | 35.942 | 9.709 / 3.000 / 3.705 / 17.274 |
| reference / darcy | 2567 / 44 | 2146 | 67.082 / 67.082 | 0.008 / 0.335 | 0.290 / 0.290 | 43.610 | 17.383 / 5.118 / 5.199 / 13.206 |
| reference / the zx | 10183 / 183 | 10000 | 287.355 / 268.304 | 0.011 / 1.478 | 0.340 / 0.801 | 151.983 | 50.350 / 26.129 / 26.623 / 40.426 |
| reference / quixote whale | 52 / 1 | 41 | 1.573 / 1.573 | 0.002 / 0.049 | 0.450 / 0.011 | 1.012 | 0.261 / 0.144 / 0.125 / 0.153 |
| reference / holmes elementary | 1463 / 26 | 925 | 31.781 / 31.781 | 0.006 / 0.250 | 0.960 / 0.411 | 36.254 | 9.970 / 3.014 / 3.680 / 17.398 |

Her profil hücresi5 gözlem ortalamasıdır; normal p95 değildir. Verify parent span; read/CRC/schema/text onun içindedir, süreler üst üste eklenmez. Setup posting lookup ve küçük cache hazırlığını içerir. Bütün38 durumun ham sayaçları ve süreleri profiles CSV içinde.

`the zx` için aday 2.013 read /7.987 window hit, referans 1.112 read /8.888 hit yapıyor. İkisi de10.000 canlı adayı ve268,304MB payload'ı istiyor; aday286,171MB, referans287,355MB fiziksel byte okuyor. Byte toplamından çok istek parçalanması artıyor. Kod üç fiziksel komşu istekten sonra read-ahead açar; maskede atlanan kaynak aralıkları komşuluğu böler. Bu, bu sorguda ölçülmüş I/O bedelidir; bütün p95 farkları için tek neden diye sunulmaz. Darcy, holmes elementary, quixote whale canlı aday/payload byte sayıları referansla eşit.

Maske hazırlığı engine'deki cache/mask kurulumudur. Prefix visible_count sayımı, ana seed okuma/decode döngüsü ve kalan filtreleme işi diğer sütunundadır; bunlar ayrı ayrı ölçülmüş gibi sunulmaz. Other, filter parent süresinden mask/setup/intersection/verify ayrık aralıklarının çıkarılmasıdır.

## Dar deney ve sınır

`set_query_visibility_pruning(true)` iki ek erken eleme uygular: görünür kaydı0 segmenti posting sözlüğüne girmeden atlar; küçük secondary posting cache'lerini maskeden geçirir ve boş grup varsa seed/payload işine başlamadan döner. Eski seed maskesi ve CRC/schema/identity/sıralı ID-bounds kontrolleri korunur. Ek corpus, ek kalıcı indeks veya eşik değişimi yok. Cache aynı256KiB üst bütçede, retain yerinde çalışır. Deney varsayılan kapalıdır.

Bu deney mantıksal ölü kayıt doğrulamasını kaldıran yeni bir yol olarak sunulmaz: o eleme zaten vardı. Canlı adayların payload maliyetini ve parçalı yerleşimin I/O bedelini maske tek başına ortadan kaldırmaz.

Ara cache'lerden elenen ID sayısı belge sayısı değildir: aynı belge farklı gram listelerinde tekrar sayılır. Örneğin holmes elementary için20.272 ara ID silindiği halde doğrulanan canlı aday sayısı925 olarak kaldı.

## Yöntem / normal ölçüm

Mevcut aynı ilk10000 belge. Mevcut420,644MB aday ilk çiftte doğrudan yeniden kullanıldı; kaynak SHA'ları okumalar boyunca aynı. Sonra sabit FDB slotunda eski %20/sabit pacing politikasıyla10 tur referans, ardından aynı10 tur aday kuruldu. Yeni corpus hazırlanmadı; DB kopyası tutulmadı. Reader sırası ilk adayda before→after, referansta before→after / after→before, son adayda after→before. Her sorgu30 zamanlı örnek ve eşlenmiş warmup; reader/layout başına pooled60. İki adayın mantıksal modelleri eşit doğrulandı.

SQLite mevcut10 tur DB'siyle, FTS5trigram/detailnone/FULLWAL/cache2MiB/mmap0/tempFILE ayarlarında iki30 örnekli okuma ve full oracle: yeniden ingest yok. OS cache sıcak; CPU/power/background izolasyonu yok. Oracle ve corpus okuması query timer dışında. Normal ve diagnostic derlemeler ayrıdır. Küçük negatif sorgularda yüksek yüzde için mutlak delta ayrıca verilir; küçük ms farkları kapıdan muaf tutulmaz.

| Case / sorgu / offset / limit | Ref p95 ms | Eski aday p95 | Yeni aday p95 | Δ ms / % | Yeni / eski aday % | SQLite p95 |
|---|---:|---:|---:|---|---:|---:|
| 0 / quixote / 0 / 50 | 2.256100 | 2.309900 | 2.343700 | +0.087600 / %+3.88 | %+1.46 | 15.516900 |
| 1 / zeuglodon / 0 / 50 | 1.466100 | 1.724900 | 1.633300 | +0.167200 / %+11.40 | %-5.31 | 6.850500 |
| 2 / beagle / 0 / 50 | 17.554700 | 19.571200 | 18.199100 | +0.644400 / %+3.67 | %-7.01 | 175.023900 |
| 3 / mesopotamia / 0 / 50 | 2.333300 | 2.639400 | 2.560300 | +0.227000 / %+9.73 | %-3.00 | 10.609500 |
| 4 / transubstantiation / 0 / 50 | 44.749000 | 47.636200 | 42.585700 | -2.163300 / %-4.83 | %-10.60 | 583.200600 |
| 5 / harpoon / 0 / 50 | 68.080100 | 70.917400 | 64.374900 | -3.705200 / %-5.44 | %-9.23 | 629.578700 |
| 6 / ishmael / 0 / 50 | 8.709200 | 9.094400 | 7.511800 | -1.197400 / %-13.75 | %-17.40 | 78.269000 |
| 7 / invertebrata / 0 / 50 | 11.949500 | 11.803300 | 14.028200 | +2.078700 / %+17.40 | %+18.85 | 134.227500 |
| 8 / darcy / 0 / 50 | 85.988300 | 56.519900 | 61.993500 | -23.994800 / %-27.90 | %+9.68 | 521.379200 |
| 9 / elizabeth / 0 / 50 | 68.577500 | 55.468600 | 58.520500 | -10.057000 / %-14.67 | %+5.50 | 478.611500 |
| 10 / watson / 0 / 50 | 58.671500 | 57.085700 | 47.341500 | -11.330000 / %-19.31 | %-17.07 | 377.136800 |
| 11 / holmes / 0 / 50 | 39.477900 | 35.038800 | 29.397000 | -10.080900 / %-25.54 | %-16.10 | 237.076700 |
| 12 / frankenstein / 0 / 50 | 20.599200 | 19.316200 | 14.575600 | -6.023600 / %-29.24 | %-24.54 | 137.213300 |
| 13 / galapagos / 0 / 50 | 14.526800 | 11.617500 | 9.864800 | -4.662000 / %-32.09 | %-15.09 | 77.137100 |
| 14 / fossil / 0 / 50 | 41.440600 | 29.575900 | 37.208100 | -4.232500 / %-10.21 | %+25.81 | 291.408400 |
| 15 / geologist / 0 / 50 | 26.099200 | 17.575800 | 21.355000 | -4.744200 / %-18.18 | %+21.50 | 169.059200 |
| 16 / the / 0 / 50 | 1.891200 | 1.365800 | 1.316100 | -0.575100 / %-30.41 | %-3.64 | 1306.917900 |
| 17 / man / 0 / 50 | 2.039900 | 1.532600 | 1.536800 | -0.503100 / %-24.66 | %+0.27 | 1322.800600 |
| 18 / he / 0 / 50 | 1.508800 | 1.277800 | 1.428900 | -0.079900 / %-5.30 | %+11.83 | 1281.244400 |
| 19 / zzxqnever / 0 / 50 | 0.024600 | 0.026300 | 0.023700 | -0.000900 / %-3.66 | %-9.89 | 0.157000 |
| 20 / zx / 0 / 50 | 157.563600 | 149.278900 | 173.816000 | +16.252400 / %+10.31 | %+16.44 | 1783.830000 |
| 21 / the zx / 0 / 50 | 169.575900 | 166.942900 | 172.416300 | +2.840400 / %+1.68 | %+3.28 | 1920.263400 |
| 22 / quixote whale / 0 / 50 | 2.649300 | 2.684100 | 2.936000 | +0.286700 / %+10.82 | %+9.38 | 18.168500 |
| 23 / holmes elementary / 0 / 50 | 51.593100 | 44.945800 | 40.217200 | -11.375900 / %-22.05 | %-10.52 | 375.203400 |
| 24 / fossil geologist / 0 / 50 | 13.779700 | 12.095200 | 14.031700 | +0.252000 / %+1.83 | %+16.01 | 132.263200 |
| 25 / whale harpoon / 0 / 50 | 75.343000 | 66.191300 | 67.984500 | -7.358500 / %-9.77 | %+2.71 | 486.250100 |
| 26 / QUİXOTE / 0 / 50 | 3.513300 | 2.563300 | 3.125700 | -0.387600 / %-11.03 | %+21.94 | 16.196000 |
| 27 / İSHMAEL / 0 / 50 | 10.436900 | 7.318500 | 10.242400 | -0.194500 / %-1.86 | %+39.95 | 62.230600 |
| 28 / zx ×150 / 0 / 50 | 0.022700 | 0.046400 | 0.052000 | +0.029300 / %+129.07 | %+12.07 | 1.622700 |
| 29 / presumption / 0 / 50 | 113.907900 | 98.510000 | 93.598800 | -20.309100 / %-17.83 | %-4.99 | 663.694900 |
| 30 / the / 0 / 1 | 0.462300 | 0.292800 | 0.221400 | -0.240900 / %-52.11 | %-24.39 | 1373.859500 |
| 31 / the / 0 / 200 | 9.442300 | 7.915100 | 6.273400 | -3.168900 / %-33.56 | %-20.74 | 1464.055300 |
| 32 / the / 5000 / 50 | 2.953600 | 1.979100 | 1.919600 | -1.034000 / %-35.01 | %-3.01 | 1295.855400 |
| 33 / the / 9950 / 50 | 2.759800 | 1.680300 | 1.959900 | -0.799900 / %-28.98 | %+16.64 | 1407.229700 |
| 34 izlenen / ishmael / 130 / 1 | 9.278000 | 10.933700 | 7.795600 | -1.482400 / %-15.98 | %-28.70 | 60.459300 |
| 35 / ishmael / 60 / 200 | 20.960200 | 19.372600 | 16.108400 | -4.851800 / %-23.15 | %-16.85 | 84.331200 |
| 36 / quixote / 28 / 10 | 2.473500 | 2.000000 | 1.957900 | -0.515600 / %-20.84 | %-2.11 | 12.606900 |
| 37 / quixote / 57 / 50 | 1.851600 | 1.297800 | 1.531300 | -0.320300 / %-17.30 | %+17.99 | 11.415700 |

## Bakım/disk ve doğruluk

Yeni aday10tur bakım toplamı 13.111s (önceki kabul edilmiş iki koşu13,500/13,950s); disk son tur 420.644MB. Ölçüm bir ek10tur tekrarıdır; yeni30tur koşusu yapılmadı. Query değişimi bakım kodunu veya kaynakları değiştirmez. Önceki30tur1,14× sonucu tarihsel kabul olarak korunur; yeni bir30tur kanıtı sayılmaz.

Bakımın önceki kabul edilmiş üst gözlemi13,9502s aşmaması ayrıca kontrol edildi. Bu kontrol eski12s hedefini geçmiş saymaz. Query-only değişimde bakım kodu ve eşikler aynıdır.

Oracle, yedi gözlenen emeklilik/iptal/iterator/flush/merge/cache kill-reopen ve eski frozen okuyucu doğrulaması tekrar geçti. Power-loss iddiası yok. Genel dört kill penceresinin owned/streaming kapsamı önceki rapordaki gibidir; bütün responsive worker evrelerinin kill kapsamı iddia edilmez. `cargo fmt --check`, default ve diagnostic clippy all-targets -Dwarnings, tam cargo test, no-default lib ve workload testleri geçti.

## Karar ve açık işler

37 sorgu kapısı geçmediği için snapshot Adım2 başlatılmadı. Maskenin zaten yaptığı işi ikinci kez çözmüş gibi sunmuyoruz; ek pruning deneyi kapalı kalır. Sonraki karar için gerçek fark canlı payload I/O/doğrulama tarafındadır.

Snapshot'ın ≤100ms mutlak maksimum kapısı için ayrıca başlangıç sınırı var: bu boşta matriste adayın en kötü tekil sorgusu 197.378ms. Snapshot kilit beklemesini azaltabilir; bu boşta tam contains taramasını kendiliğinden ≤100ms yapmış sayılmaz.

Adım3 bilinen sınırı: sürekli yazma revision worker'larını iptal ettirip geri kazanmayı boşta zamana erteleyebilir; bu yükte süresiz disk üst sınırı kanıtlanmadı. Genel RAM, dirty500 açılış, küçük commit, çok okuyucu/soğuk yük ve geçici dosyalar açık. C ABI uygulanmadı.

[Özet / kapılar](benchmarks/revision-search-10k-20261026-summary.json) · [Ham query CSV](benchmarks/revision-search-10k-20261026-queries.csv) · [Profil CSV](benchmarks/revision-search-10k-20261026-profiles.csv) · [Bakım CSV](benchmarks/revision-search-10k-20261026-maintenance.csv) · [Komutlar](benchmarks/revision-search-10k-20261026-checks.json) · [OPEN-WORK](OPEN-WORK.md).
