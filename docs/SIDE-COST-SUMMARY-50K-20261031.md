# Okuma ve yan maliyetler — 31 Ekim 2026

B=REFERENCE_WAIT=true; A=diagnostics anında try/seri. Varsayılan B; 2ms yok. Normal okuma/yazma=ESTIMATOR N60 dört çift. CPU/RAM=yeni dört eşleşmiş çift. Pooled p95 gerçek örneklerden; kapı kararları pair tablolarındadır, pooled değerler kapının yerine geçmez.

|Sorgu|4R idle B→A p95 ms|4R+W B→A p95 ms|Sorgu|4R idle B→A p95 ms|4R+W B→A p95 ms|
|---|---|---|---|---|---|
|olduğu|4.81→4.71|11.96→10.77|korkaklığa|42.03→41.71|59.02→47.22|
|büyük|4.10→3.94|9.57→9.27|ormanların|436.59→325.69|550.80→358.53|
|ayrıca|4.69→4.66|10.86→10.39|yanıklarda|452.64→383.28|510.91→411.83|
|arasında|6.07→5.97|13.05→12.98|külliyatı|354.84→304.37|439.11→311.22|
|birlikte|4.69→4.63|12.89→12.32|timmy|162.63→23.54|211.78→27.31|
|sanat|5.41→5.45|13.29→13.68|yüklük|178.07→36.28|197.54→37.84|
|görenler|11.07→11.40|25.03→24.13|ilçesine köyleri|9.72→9.40|16.02→15.39|
|millî|4.22→4.45|10.46→10.30|geldiği buldu|207.28→73.13|268.48→76.98|
|ikinci|4.51→4.58|12.96→11.79|chicago studio|11.49→11.20|18.01→16.23|
|toplam|4.99→4.98|14.13→13.50|kısım dişli|439.97→336.08|537.08→385.99|
|isimli|10.21→10.43|21.74→22.69|güzeli zafer|223.58→47.06|238.44→53.17|
|öldürür|238.82→42.11|236.55→50.40|İnternetten hazinem|259.19→125.11|305.13→132.68|
|ölümü|5.49→5.72|12.34→12.27|oyuncudur atlanta|219.28→92.90|261.67→100.79|
|akarsu|239.60→20.94|78.81→23.50|kendisine çıkacağını|336.08→283.01|431.99→302.56|
|demokratik|7.40→7.27|16.07→11.40|ve|2.67→2.86|4.26→4.17|
|çalışmalarıyla|314.50→172.05|419.53→209.18|if|4.38→4.30|6.23→6.02|
|üreticisi|183.03→54.90|206.85→64.80|id|3.28→3.36|4.89→4.69|
|joseph|4.10→4.36|8.22→7.33|zx|650.40→777.46|804.55→1095.56|
|leyla|260.51→92.16|327.93→109.19|qzxkvwnever|1.00→1.20|3.32→3.34|
|protest|10.08→10.44|16.24→13.70|transubstantiationz|4.30→4.12|11.12→10.09|
|furlong|11.28→11.23|16.16→15.07||||

|Yazma pooled|B|A|SQLite|
|---|---:|---:|---:|
|Ops/s|322.471|341.188|235.920|
|Korunum %|104.922|111.437|102.970|
|p95 ms|4.372|3.901|9.190|
|p99 ms|14.171|9.568|19.035|
|p99,9 ms|22.361|20.938|175.561|
|Max ms|260.221|195.251|1203.274|

Ops/s min/medyan/max: before 287.151/330.765/349.222; after 331.398/341.131/352.277; sqlite 211.389/244.651/247.349.
Paired A/B throughput Δ% min/medyan/max: 0.875/3.157/15.444. SQLite korunum serisi: 71,10% → 82,739% → 102.970%.

|Altılı mix CPU ms/query|1R B→A|4R B→A|Full41 loaded CPU B→A|
|---|---|---|---|
|kendisine çıkacağını|226.628→224.089|243.490→254.362|268.291→268.638|
|ve|1.302→1.432|2.539→2.474|2.542→2.349|
|if|2.865→2.214|4.232→2.734|3.720→3.881|
|id|1.562→1.823|3.385→3.060|2.790→3.579|
|zx|422.786→418.880|504.297→607.161|559.648→602.093|
|the zx|137.305→136.328|165.690→191.536|—|

|RAM çift|Ingest peak B→A MiB|Idle private B→A MiB|4R+W peak B→A MiB|
|---|---|---|---|
|0|16.11→16.61|2.27→2.25|34.00→35.52|
|1|14.81→15.08|2.28→2.29|33.90→36.04|
|2|15.64→14.63|2.28→2.28|32.08→36.26|
|3|14.73→15.40|2.28→2.26|31.84→35.17|

A normal41 geçen sayısı: B39/40/41/41; A41/41/40/40; SQLite41/40/40/40. Motor kaynakları değişmedi. CPU diagnostic sayaç overheadini içerir; oracle hariç parent+child toplamıdır. RAM model/oracle/profil hariç motor prosesidir. the zx bilgi amaçlı. [Dört çift, spread, SQLite oranları, bütün raw ve sınırlar](SIDE-COST-50K-20261031.md).
