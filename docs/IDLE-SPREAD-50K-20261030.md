# Idle tabanı ve4R+yazıcı — 30 Ekim2026

Dört çift; sırayla FDB→SQLite,SQLite→FDB,FDB→SQLite,SQLite→FDB. Aynı frozen ikili ve aynı sabit50k DB slotları yeniden kurulmadan kullanıldı. Her child kendi normal kabul düzeninde1R idle,4R idle,16384 tekli writer+4R ve oracle çalıştırdı. Mantıksal sorgu kümeleri aynı; revision token/version ilerler. Normal başlangıç bakımı ve yazmalar fiziksel yerleşimi değiştirir; bu yüzden yayılımın yalnız işletim sistemi gürültüsünden kaynaklandığı iddiası yok. No-reset, sequence/segment sayıları her opened logunda. Aynı anda başka benchmark/build/test çalıştırılmadı. Warm OS cache; cold eviction yok.

P95 nearest rank. Idle4R/case N12 olduğundan p95 o kolun maksimum örneğidir; loaded p95 kendi writer-active örneklerinden hesaplandı. Yeni eşik yok: idle≤30ms ise loaded≤30ms, diğerleri≤2×idle. SQLite aynı formülle bilgi amaçlı gösterilir; gate FDB içindir. Request edilen minimum3 yerine4 çiftle iki sıra eşit tekrarlandı.

## Çift kararları

|Çift|Motor|Geçti / kaldı|Kalan case ID|
|---|---|---|---|
|0|fdb|40 /1|[10]|
|0|sqlite|40 /1|[29]|
|1|sqlite|40 /1|[40]|
|1|fdb|37 /4|[6, 10, 11, 33]|
|2|fdb|39 /2|[6, 29]|
|2|sqlite|39 /2|[20, 40]|
|3|sqlite|38 /3|[21, 25, 31]|
|3|fdb|36 /5|[11, 16, 21, 31, 33]|

## Idle p95 yayılımı (çiftlerin p95 değerleri)

|Motor/case|Min ms|Max ms|Medyan ms|Dört değer ms|
|---|---:|---:|---:|---|
|fdb/13:akarsu|86.779|382.041|120.180|382.041, 135.245, 105.115, 86.779|
|fdb/16:üreticisi|50.693|397.118|227.175|202.190, 397.118, 252.160, 50.693|
|fdb/25:timmy|110.581|435.112|231.064|317.221, 435.112, 144.906, 110.581|
|sqlite/13:akarsu|148.335|235.429|226.281|235.429, 226.926, 225.637, 148.335|
|sqlite/16:üreticisi|241.373|393.491|368.604|393.491, 367.093, 370.115, 241.373|
|sqlite/25:timmy|23.532|38.031|33.403|38.031, 34.953, 31.853, 23.532|

## Her çiftte odak sorgular

|Çift/motor|Case|Idle p95 ms|Loaded p95 ms|Sınır ms|Karar|
|---|---|---:|---:|---:|---|
|0/fdb|akarsu|382.041|44.593|764.082|GEÇTİ|
|0/fdb|üreticisi|202.190|181.228|404.381|GEÇTİ|
|0/fdb|timmy|317.221|294.799|634.443|GEÇTİ|
|0/sqlite|akarsu|235.429|297.597|470.859|GEÇTİ|
|0/sqlite|üreticisi|393.491|479.015|786.982|GEÇTİ|
|0/sqlite|timmy|38.031|55.752|76.062|GEÇTİ|
|1/sqlite|akarsu|226.926|283.457|453.851|GEÇTİ|
|1/sqlite|üreticisi|367.093|502.943|734.186|GEÇTİ|
|1/sqlite|timmy|34.953|50.784|69.906|GEÇTİ|
|1/fdb|akarsu|135.245|45.639|270.489|GEÇTİ|
|1/fdb|üreticisi|397.118|253.963|794.237|GEÇTİ|
|1/fdb|timmy|435.112|203.716|870.225|GEÇTİ|
|2/fdb|akarsu|105.115|101.847|210.230|GEÇTİ|
|2/fdb|üreticisi|252.160|327.752|504.320|GEÇTİ|
|2/fdb|timmy|144.906|283.673|289.812|GEÇTİ|
|2/sqlite|akarsu|225.637|282.191|451.274|GEÇTİ|
|2/sqlite|üreticisi|370.115|457.779|740.230|GEÇTİ|
|2/sqlite|timmy|31.853|39.298|63.706|GEÇTİ|
|3/sqlite|akarsu|148.335|218.381|296.671|GEÇTİ|
|3/sqlite|üreticisi|241.373|372.888|482.746|GEÇTİ|
|3/sqlite|timmy|23.532|36.241|30.000|KALDI|
|3/fdb|akarsu|86.779|24.397|173.559|GEÇTİ|
|3/fdb|üreticisi|50.693|244.653|101.387|KALDI|
|3/fdb|timmy|110.581|173.705|221.161|GEÇTİ|

Bütün41 case, ham örnekler ve sayımlar [JSON tabloda](benchmarks/idle-spread50-20261030-summary.json); [program SHA/exit ve ham log dizini](benchmarks/idle-spread50-20261030-design.json). Hiçbir kötü kol çıkarılmadı. Oynaklık kaynak atfı değildir; cache,yerleşim,worker bekleme,CPU/I/O ayrımı sonraki ayrı diagnostics raporundadır. Process RAM motor-only diye sunulmaz; D ayrıdır. Throughput için bu tekrarlar yeni okuyucusuz control çalıştırmadı; önceki%91,4/%71,1 sonucu yeni ölçülmüş oran sayılmaz.

100lü/B/C/2,6sn ancak A41/41 sonrası; herhangi bir çiftte kalan sorgu sessizce havuzla kapatılmaz. Kullanıcıya bütün kararlar sunulur. Biçim/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k/WAL düzeltmesi yok.
