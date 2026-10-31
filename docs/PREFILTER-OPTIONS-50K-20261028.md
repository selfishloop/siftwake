# Ön filtre seçenekleri — yalnız maliyet tahmini, 50k, 28 Ekim 2026

Son karar: ön filtre uygulanmayacak. Aşağıdaki uygulama önerileri tarihseldir;
[yeni Türkçe corpus'ta yalnız offline oranlar](TRWIKI-OFFLINE-20261028.md)
hazırlandı. Yeni bitset/4-gram/position codec'i veya SEG sürümü yok.


**Hiçbiri motora uygulanmadı.** Yeni kaynak/WAL/manifest/SEG sürümü veya cache
codec'i yazılmadı. Aşağıdaki aday sayıları mevcut belge gövdelerinin offline
analizidir; yeni corpus/DB/ön filtre oluşturulmadı. Uygulama için kullanıcı
onayı gerekir. Arama hızlarının tahmini, ölçülmüş p95 gibi sunulmaz.

## Ölçülen içerik seçiciliği

Aynı mevcut ilk50k ve sequence104 modeli; bütün payload'lar mevcut havuzdan
CRC kontrolüyle okunup aynı2–50KiB kırpma, revision marker ve metin eşleme
kurallarıyla analiz edildi. Bir term'in n-gramlarının hepsinin title veya text'te
bulunması aday sayılır; AND terimleri ayrı ayrı sınanır. Term'de whitespace
olmadığından n-gramlar whitespace üzerinden üretilmez. Son tam eşleşme sayıları
önceki model oracle'ıyla aynıdır. Bu analiz indekse bağlanmadı.

| Sorgu | Mevcut3-gram aday | 4-gram varsayımsal aday | 5-gram varsayımsal aday | Gerçek eşleşme |
|---|---:|---:|---:|---:|
| harpoon | 10651 | 5212 | 5212 | 5212 |
| presumption | 12635 | 5401 | 4447 | 4318 |
| darcy | 10584 | 5983 | 5983 | 5983 |
| transubstantiation | 7563 | 0 | 0 | 0 |
| holmes elementary | 4620 | 530 | 530 | 282 |

4-gram için payload aday azalması harpoon%51,1; presumption%57,3; darcy%43,5;
transubstantiation%100; holmes elementary%88,5. AND'de iki kelimenin aynı
belgede ayrı bulunması hâlâ yeterlidir: holmes elementary530 aday/282 gerçek.
4-gram bütün yanlış pozitifleri kaldırmaz.5-gram burada çoğunlukla4-gramla aynı;
presumption5401→4447 için ek yarar var. Yeni grama ait posting lookup/CRC/ingest
maliyeti bu sayılardan çıkarılamaz.

## Disk modeli ve ingest/RAM bütçesi

1000 belge deterministik örnek: mevcut anahtar sırasındaki her50. kayıt. Yeni
örnek corpus dosyası yok. Örnekte trigram DF1.939.533;4-gram DF2.994.819,
trigram occurrence12.206.463. Vocab3=11.743, vocab4=42.675 **yalnız örnekte
görülen alt sınır**, tüm havuzun vocabulary tahmini diye sunulmaz. Örnekler
kitap pencereleri olduğundan bağımsız rasgele dosya örneklemi veya güven
aralığı değildir.50× doğrusal genişletme yalnız kapasite planlama modelidir.
Mevcut50k indeks/cache414,33MB ve DB yaklaşık1,76GB.

| Seçenek | İlave disk tahmini @50k | Ingest planlama tahmini | RAM planlama tahmini | Beklenen aday etkisi |
|---|---|---|---|---|
| Belge başına **tam trigram presence bitset** | M vocabulary için N×ceil(M/8); örnekteki M alt sınırıyla≥73,4MB. M=16384 ayrılırsa102,4MB, M=32768 ise204,8MB; dictionary/headers ayrıca | Mevcut trigram extraction yeniden kullanılabilir. Ek bit-set ve sıralı yazma;65–77s bazına kabaca+%5–20 bütçe, **ölçülmedi** | Akışlı tek bitset2–4KiB/doc + vocabulary eşlemesi. Bütün bitsetler RAM'e alınırsa73–205MB ve hedefi bozar; diskten sayfa okuma şart | **Ek eleme0**: mevcut inverted trigram posting'leri zaten aynı presence AND bilgisini taşıyor |
| Trigram + **konum listeleri** | Deltalı position verisi yaklaşık1.152MB; liste count/field/header ile kabaca+1,25–1,55GB. Ham u32 position+2.441MB; mevcut doc ID posting'leri ayrıca korunur | Yeni occurrence/list üretimi, spool/sort/merge ve daha fazla I/O; bazın kabaca1,5–3×'i için bütçe ayırma modeli. Yeni codec ölçülmedi | Bütçeli spool ile worker başına ilave1–4MiB hedeflenebilir. Naif tam occurrence biriktirme GB'lar tutar;≤60MiB garanti değil | Tek kelimenin bütün komşu position zinciri, field ayrımıyla gerçek substring'e kadar indirebilir; teorik alt sınır tabloda gerçek eşleşme sütunu |
| **Mevcut3-gram yanında4-gram** inverted posting | Örnek DF oranıyla ilave≈599MB ham u32 doc ID; yeni dictionary/page/header ayrıca. Toplam DB kabaca2,36GB+; yaklaşık+%34. Replace yerine ekleme varsayıldı | Posting insert sayısı3-gramın1,544×'i kadar **ek** artar. Basit model Tnew/Told≈1+1,544×f; ingest'in grama ayrılan f=%35–70 varsayımıyla≈1,54–2,08× (yaklaşık100–160s). f bu tur ölçülmedi | Disk cache aynı bounded bütçede tutulabilir; builder vocabulary/flush sıklığı artar. İlave1–4MiB bounded spool hedefi; sabit RAM'in bedeli daha çok geçici I/O. Ölçülmeden60MiB geçti denmez | Yukarıdaki4-gram aday sayıları; pozisyon gerekmez;3 karakterli sorgular eski indeksi kullanabilir |
| Seçilmiş4-gram / Bloom4 özeti | p byte/doc için50k×p; ör.256B=12,8MB,1024B=51,2MB + binding/header. Boyut kesin olunca false-positive ölçülmeli | Her belgeyi en az bir kez ek tarama + hash/set; mevcut3-gram maliyetine doğrudan eşit sayılmaz. Özet taraması ve I/O için+%10–40 **planlama bütçesi**; uygulama ölçümü yok | Sabit256B–1KiB/doc scratch ve bounded disk cache; resident tüm özetleri tutmak12,8–51,2MB | Yoğun uzun metinde küçük Bloom doyar. Beklenen FP≈(1−e^(−k·d/m))^k; d≈2995, m=8192bit, k=2 ise≈%27. Bu tek4-gram FP'sidir; sorgu gramları bağımsız sayılmaz. Tablodaki tam4-gram kadar eleme garanti edilmez |

Ingest aralıkları **ölçülmüş performans sonucu değildir**, kapasite/risk bütçesidir.
Mevcut65–77s ingest'e CPU-only sayı ekleyerek kesin duvar süresi tahmin edilmez;
shared I/O, flush sayısı, mevcut builder bütçesi ve yeni compression bunu değiştirir.
Vocab alt sınırı, u32 doc-ID ve örnek DF/occurrence hesapları ham JSON'dan
tekrarlanabilir. Position delta hesabı title/text'i ayrı başlatır; field IDs,
list count, first position ve key/version binding codec'i henüz tasarlanmadı.

**Öncelik önerisi:** daha fazla persistent veriye geçilecekse önce sadece
4-gram için dar bir cache prototipi değerlendirilir: sıfır sonuçlu7563 adayı bu
corpus'ta0'a indiriyor. Bunun yaklaşık599MB ham posting maliyeti var; yeni
gerçek dosya corpus'unda seçicilik ve daha sıkı sparse/özet seçenekleri ayrıca
ölçülmeli. Trigram bitsetini eklemek false-positive sorununu çözmüyor.
Position listeleri daha güçlü fakat disk maliyeti bu iş yükü için yüksek.
Hiçbiri için uygulamaya başlanmadı; kullanıcı onayı beklenir.

Bir uygulama kararı çıkarsa yeni **SEG sürümü** (kaynağa konursa ayrıca payload/
WAL sürümü) ve eski okuyucu uyumluluk planı önce yazılır. SEG05'e sessiz alan
veya aynı sürümde yeni anlam eklenmez;0.1 fixture korunur.

Ham kanıt: [50k tam aday analizi ve1000 belge kapasite örneği](benchmarks/block2-prefilter-estimates-50k-20261028-v1.json).
Araç `tools/estimate-prefilters-50k.py`, yalnız mevcut havuz okur ve rapor metadata'sı yazar.
