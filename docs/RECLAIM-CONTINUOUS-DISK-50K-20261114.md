# Kesintisiz yazıcıyla varsayılan reclaim — 50k

**Tur sonu disk kapısı GEÇTİ 30/30.** En yüksek ve son tur sonu
retained disk **1,275×**. Tur içi örnekli tepe **1,528×**.

| Ölçüm | Kesintisiz yazıda | Önceki kabul edilen 2 s aralı koşu |
| --- | ---: | ---: |
| Başlangıç | 300,84 MB | 300,84 MB |
| En yüksek tur sonu oranı | 1,275× | 1,281× |
| Son tur | 383,66 MB / 1,275× | 364,70 MB / 1,212× |
| Son manifest-canlı disk | 382,84 MB | — |
| Örnekli geçici tepe | 459,82 MB / 1,528× | 453,33 MB / 1,507× |
| İlk 1,3× tur sonu aşımı | Yok | Yok |
| Son run sayısı | 92 | 52 |
| 100'lük commit p95 / p99 / max | 94,69 / 1.164,27 / 2.605,41 ms | 53,31 / 898,93 / 2.093,48 ms |
| Commit >1 s | 78/3.000 | 23/3.000 |
| Yazı ve doğal aralar, işlem/s | 1.665 | 1.518 |

0.4.0, aynı 881756DA… 50k öneği ve FDB slotu. 1R+1W,
41 sorgu, doğal sıcak cache. 30 tur × 10.000 değişiklik, 100'lük
commit, varsayılan reclaim. Yazıcı tur aralarında uyumadı, bakım
tiki veya senkron bakım çağırmadı. Yazar, model güncellemesi ve
bir sonraki churn listesinin hazırlanması doğal işin parçası.
Dosya envanteri ayrı proses. Tur sonu zamanları son grup ACK'si,
nearest disk örneğinin en büyük sapması **28,40 ms**. 30 örneğin
26'sında manifest kuşağı tarama boyunca sabit kaldı. Diğer dört
örneğin canlı baytı yaklaşık. Son tur örneği ACK'ten 13,28 ms
sonra, manifest kuşağı sabit. Bu, 30 turluk ölçüm ve yaklaşık
tur sonu örneklemesidir.

Kapanıştan önce 225 revizyon kurulmuştu, bir revizyon işçisi
çalışıyordu. Retained örneklerine sonraki doğrulama ve cleanup
eklenmedi. Önceki düzen başka oturumda ölçüldü. Arayı kaldırmanın
tek başına neden olduğu gecikme veya disk farkı çıkarılamıyor.
[Ham tur saatleri ve commit'ler](benchmarks/reclaim-followup-20261114/03-continuous/continuous-30.jsonl) ·
[Disk örnekleri](benchmarks/reclaim-followup-20261114/03-continuous/disk-samples.jsonl) ·
[Tur eşlemesi ve uçlar](benchmarks/reclaim-followup-20261114/03-continuous/disk-summary.json) ·
[Önceki kabul edilen koşu](BACKGROUND-RECLAIM-DEFAULT-50K-20261112.md)

**Doğruluk GEÇTİ.** Onaylanmış commit sonrasında process-kill,
strict reopen ve dondurulmuş okuyucu 50.000 belge/41 digest
ile geçti. FDB slotu hazır, sequence 3101. Korpus, önceki model
ve 0.1 fixture korundu. 22:40:20–22:44:59 UTC, ölçülen yazı
180,13 s. Kanıt 7,97 MiB. Süre, slot ve boş alan korumaları içinde.
[Doğrulama ve kaynak kimliği](benchmarks/reclaim-followup-20261114/03-continuous/status.json)
