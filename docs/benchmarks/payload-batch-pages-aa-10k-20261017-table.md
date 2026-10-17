# Aynı SHA / yoğun sekiz sayfa A/A

4 süreç çifti x8 strict blok x25 tur =800 örnek/sayfa/variant. İki tarafta aynı kontrol programı. Sıcak OS cache; kaynak doğrulaması ve tam-belge oracle timing dışında. Fark=B/A-1. Bu oranlar A/B sonuçlarından çıkarılmaz.

| Sorgu | offset/limit | A p95 ms | B p95 ms | Fark | B hızlı çift | Tipik blok fark | B hızlı blok |
|---|---:|---:|---:|---:|---:|---:|---:|
| the |0/1|0.2912|0.2881|-1.06%|1/4|-0.16%|16/32|
| the |0/200|6.3115|6.4183|+1.69%|2/4|-3.74%|19/32|
| the |5000/50|2.3972|2.4577|+2.52%|2/4|-5.00%|20/32|
| the |9950/50|2.0886|2.1338|+2.16%|2/4|-2.54%|18/32|
| ishmael |130/1|7.7369|8.1476|+5.31%|2/4|-0.63%|17/32|
| ishmael |60/200|15.8304|16.6865|+5.41%|1/4|-1.01%|17/32|
| quixote |28/10|2.4966|2.6081|+4.47%|1/4|-0.02%|16/32|
| quixote |57/50|2.1990|2.3182|+5.42%|2/4|-0.81%|18/32|
