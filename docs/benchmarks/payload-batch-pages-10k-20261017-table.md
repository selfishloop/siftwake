# Aynı mevcut10k / yoğun sekiz sayfa A/B

4 süreç çifti x8 strict blok x25 tur =800 örnek/sayfa/variant. Sıcak OS cache; kaynak doğrulaması ve tam-belge oracle timing dışında. Fark=aday/kontrol-1.32 blok32 bağımsız süreç değildir.

| Sorgu | offset/limit | Kontrol p95 ms | Aday p95 ms | Fark | Hızlı çift | Tipik blok fark | Hızlı blok |
|---|---:|---:|---:|---:|---:|---:|---:|
| the |0/1|0.3133|0.3098|-1.12%|2/4|-1.82%|20/32|
| the |0/200|7.1694|7.6012|+6.02%|2/4|+2.78%|14/32|
| the |5000/50|2.6346|2.6150|-0.74%|2/4|-1.22%|17/32|
| the |9950/50|2.2039|2.3591|+7.04%|2/4|-1.81%|18/32|
| ishmael |130/1|9.0156|9.1982|+2.03%|2/4|+0.60%|16/32|
| ishmael |60/200|18.7338|18.5048|-1.22%|3/4|+0.67%|16/32|
| quixote |28/10|2.7894|2.9048|+4.14%|3/4|-1.15%|18/32|
| quixote |57/50|2.4730|2.4844|+0.46%|3/4|-0.38%|18/32|
