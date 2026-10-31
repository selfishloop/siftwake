# Yeni cap200k corpus audit — 29 Ekim 2026

**CRC/biçim/anahtar/tekrarsızlık denetimi GEÇTİ. Düzeltilmiş SHA kullanıcıca ONAYLANDI.**
İlk bildirilen SHA `8510408E…` dosyası incelenirken corpus değiştirildi.
O ilk dosyada ilk10 başlık/metinler eski kaynakla örtüşmüyor, sorguların yalnız
8/41'i ortaktı. Şu an dosya ve manifest farklı, aşağıdaki düzeltilmiş sürüm
üzerinde tam audit yapıldı. İlk veriyle ölçüm yapılmadı; DB slotları resetlenmedi.

Gerçek payload SHA256:
`881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12`.
İlk kullanıcı mesajındaki `8510408E6F943B3DB0CF5EC816739D4CF1FB52A3922EEAC6C01F281ABAC38467`
ile farklı olduğundan ayrıca soruldu; kullanıcı881756DA… sürümünü açıkça onayladı.
Tam salt-okuma audit ve doğrulama komutları tamamlandı;50k ingest başlıyor.

Kaynak `D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench`;
`private/` okunmadı. PLOADB01 v1 başlık+her kayıt CRC'si, record-id sırası,
JSON shape/duplicate-key/UTF-8 ve version1, beklenen scope/collection/10-haneli
anahtar kimliği doğrulandı.100000 farklı body SHA'sı var. Corpus üretilmedi,
kopyalanmadı veya değiştirilmedi; katalog yalnız offset/CRC/mask/uzunluk içerir.

Önceki onaylı kaynakla sırayla stream karşılaştırmasında100000 anahtar,
100000 başlık,100000 text-prefix ilişkisi,100000 link ve bytes harici fields
aynı. Yalnız45192 ve66498 metinleri değişti; bytes alanları yeni boyutla aynı.
41 sorgunun tamamı eski listeyle aynı. Yeni SHA için sıralama/kırpma iddiası
bağımsız olarak doğrulandı; eski50k hızlarıyla birebir kıyas yapılmayacak.

| Önek | Toplam text B | Min B | p50 B | p95 B | Max B |
|---|---:|---:|---:|---:|---:|
| 50000 | 157147725 | 511 | 1479 | 10690 | 199999 |
| 100000 | 312672211 | 511 | 1478 | 10673 | 200000 |

## 41-case exact contains oracle

Normalized title OR text; AND bütün terimler için, token DF değildir.
Delta yalnız exact eşleşme sayımıdır; performans kıyası değildir.

| Sorgu | Sınıf | Yeni50k total | Yeni100k total | Eski50k delta | Eski100k delta |
|---|---|---:|---:|---:|---:|
| olduğu | very_common_20pct_plus | 14299 | 28407 | +0 | +0 |
| büyük | very_common_20pct_plus | 15316 | 30642 | +0 | +0 |
| ayrıca | very_common_20pct_plus | 14965 | 29882 | +0 | -1 |
| arasında | very_common_20pct_plus | 15854 | 31555 | +0 | +0 |
| birlikte | very_common_20pct_plus | 10219 | 20347 | +0 | +0 |
| sanat | common_5_to_20pct | 6171 | 12436 | +0 | +0 |
| görenler | common_5_to_20pct | 3379 | 6572 | +0 | +0 |
| millî | common_5_to_20pct | 4603 | 9256 | +0 | +0 |
| ikinci | common_5_to_20pct | 9031 | 17915 | +0 | +0 |
| toplam | common_5_to_20pct | 5685 | 11405 | +0 | +0 |
| isimli | common_5_to_20pct | 2763 | 5458 | +0 | +0 |
| öldürür | mid_0_5_to_5pct | 402 | 812 | +0 | +0 |
| ölümü | mid_0_5_to_5pct | 7952 | 15800 | +0 | +0 |
| akarsu | mid_0_5_to_5pct | 515 | 960 | +0 | +0 |
| demokratik | mid_0_5_to_5pct | 904 | 1806 | +0 | +0 |
| çalışmalarıyla | mid_0_5_to_5pct | 249 | 505 | +0 | +0 |
| üreticisi | mid_0_5_to_5pct | 424 | 815 | +0 | +0 |
| joseph | mid_0_5_to_5pct | 1027 | 2051 | +0 | +0 |
| leyla | mid_0_5_to_5pct | 287 | 591 | +0 | +0 |
| protest | rare_under_0_5pct | 912 | 1822 | +0 | +0 |
| furlong | rare_under_0_5pct | 4 | 9 | +0 | +0 |
| korkaklığa | rare_under_0_5pct | 2 | 3 | +0 | +0 |
| ormanların | rare_under_0_5pct | 194 | 404 | +0 | +0 |
| yanıklarda | rare_under_0_5pct | 4 | 8 | +0 | +0 |
| külliyatı | rare_under_0_5pct | 27 | 80 | +0 | +0 |
| timmy | rare_under_0_5pct | 22 | 42 | +0 | +0 |
| yüklük | rare_under_0_5pct | 508 | 983 | +0 | +0 |
| ilçesine köyleri | and_pairs_mid | 1313 | 2590 | +0 | +0 |
| geldiği buldu | and_pairs_mid | 365 | 726 | +0 | +0 |
| chicago studio | and_pairs_mid | 33 | 55 | +0 | +0 |
| kısım dişli | and_pairs_mid | 110 | 212 | +0 | +0 |
| güzeli zafer | and_pairs_mid | 16 | 37 | +0 | +0 |
| İnternetten hazinem | and_pairs_mid | 1 | 1 | +0 | +0 |
| oyuncudur atlanta | and_pairs_mid | 10 | 18 | +0 | +0 |
| kendisine çıkacağını | and_pairs_mid | 17 | 34 | +0 | +0 |
| ve | fixed_short | 47523 | 94949 | +0 | +0 |
| if | fixed_short | 22579 | 45182 | +0 | +0 |
| id | fixed_short | 38276 | 76428 | +0 | +0 |
| zx | fixed_short | 28 | 47 | +0 | +0 |
| qzxkvwnever | fixed_negative | 0 | 0 | +0 | +0 |
| transubstantiationz | fixed_negative | 0 | 0 | +0 | +0 |

## En uzun belge seçimi

| Ordinal ID | Text B | bytes field | Aynı bytes değerli belge |
|---|---:|---:|---:|
| 66498 | 200000 | 200000 | 1 |
| 45192 | 199999 | 199999 | 1 |
| 92149 | 193957 | 193957 | 1 |

66498 (200000B) ve45192 (199999B) mevcut cancel helper'ın benzersiz bytes
selector'ıyla seçilebilir.256KiB entry sınırı değişmeyecek.100k kapısı izin
verirse, candidate-start tanığıyla serial/4/melez/exact4 iptal ölçülecek;
200KB gecikmesi henüz ölçülmüş sayılmaz.

İki audit araç hatası saklandı: v1 preflight sırasında dosyanın manifest/SHA'sı
bildirilen sürümden farklıydı; v2 bütün kayıtları okuduktan sonra yeni manifestte
opsiyonel `unique_bodies` alanı bulunmadığı için KeyError verdi. Bu bir veri
bozulması değildi. v3 gerçek body hash'leriyle100000 tekrarsızlığı doğruladı.

[Ham audit](benchmarks/trwiki-cap200k-20261029-v3-audit.json),
[41-case katalog](benchmarks/trwiki-cap200k-20261029-v3-catalog.json),
[audit stdout](benchmarks/trwiki-cap200k-20261029-v3-audit.stdout),
[yeni CPU kabul kuralı](benchmarks/cpu-gate-acceptance-20261029-cap200k.json).

Pozitif CPU kuralı serial<10ms/çağrı ise +0,2ms, aksi +%5. Eski büyük
+0,09375ms artık GEÇTİ; eski ham rapor değiştirilmedi. Yazma profili,
uzun snapshot ve gerçek soğuk cache ayrı onay bekliyor. Ön filtre, biçim
ve256KiB değişimi, Blok3/4, C ABI ve dış proje entegrasyonu yok.
