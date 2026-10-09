# Yazma bileşenlerini ayırma — 08.10.2026

Önceki V2 sonuçlarındaki hız gerilemesi için üç bileşen birbirinden ayrıldı.
Kalıcı format ve varsayılan build değişmedi. Yeni Cargo feature'ları:

| Feature | İşlev |
| --- | --- |
| experimental-write-merge-policy | Dört benzer run bekleyen merge seçimi |
| experimental-write-record-spool | Builder kayıt metadata'sını geçici diske taşıma |
| experimental-write-maintenance-slots | Bir kaynak yazıcısı + bir builder sınırı |
| experimental-write-merge-slot | Merge'de key lookup yerine doğrudan slot okuma |

`experimental-write-pipeline` güncel seçimde merge-policy + record-spool +
merge-slot açar; maintenance-slots ayrı opt-in'dir. Ölçümdeki full önceki
dört bileşen birleşimidir; güncel umbrella ölçümdeki no-slots ile eşleşir.
Bileşenler kendi başlarına streaming-open/paged-metadata yolunu içerir. Full ve üç kontrol aynı güncel
koddan derlenir; eski binary ile farkları tek bileşene bağlama hatası yapılmaz.
Spills tek entry araması ve merge-slot bütün kontrol derlemelerinde aynıdır.
Her kontrolde full'den yalnız bir bileşen çıkarılır; bu deney etkileşimleri
bütün kombinasyonlarda ayrıştıran bir factorial test değildir.

## Sözleşme

Yeni DB'ler, 100.000 belge, 256 UTF-8 bayt aynı Türkçe corpus, batch 500;
[önceki sorgu ve oracle](WORKLOAD-COST-VALIDATION.md) aynıdır. Exact total,
sıralı tam StoredDocument, 50 güncelleme/silme, reopen doğrulanır. Normal
release binary'lerinde diagnostic-timing ve allocation sayaçları kapalıdır.
Sıcak OS/file cache; cold-cache, gerçek uygulama yükü, milyon kayıt,
çoklu DB/client, çeşitli/büyük metin bu turda ölçülmez.

Dört varyant dört koşuda dönen sıra ile ölçülür; her varyant her sıra
pozisyonunu bir kez alır. Build veya test ölçümlerle eşzamanlı çalışmaz.
Dar/exact/range/ikinci sayfa 30; son sayfa/geniş metin üç timed tekrar,
öncesinde bir untimed warmup. Dört koşu medyanı güven aralığı değildir;
özellikle üç tekrarlı geniş sorguda kuyruk ve koşu dalgalanması sınırlı
ölçülür. Whole-process peak working set ve 20 ms istenen aralıkla private
committed örneklemesi kullanılır; idle RAM veya toplam sistem belleği değildir.

## Dört koşu medyanları

| Ölçüm | Full | Merge politikası yok | Spool yok | Çalışma kilitleri yok |
| --- | ---: | ---: | ---: | ---: |
| Ingest | 9.5281 s | 10.6605 s | 9.8180 s | 7.7566 s |
| Son bakım | 4.6558 s | 5.0863 s | 4.8799 s | 4.8357 s |
| Yazma + bakım | 14.1604 s | 15.7143 s | 14.6838 s | 12.5679 s |
| Tepe working set | 67.0859 MiB | 76.7520 MiB | 67.5254 MiB | 68.3438 MiB |
| Örneklenen private tepe | 67.8867 MiB | 70.4785 MiB | 67.5977 MiB | 62.6094 MiB |

Sorgu p50 / p95 medyanları (ms):

| Sorgu | Full | Politika yok | Spool yok | Kilitler yok |
| --- | ---: | ---: | ---: | ---: |
| Dar metin | 2.6151 / 3.0388 | 2.7901 / 3.1195 | 2.7741 / 3.0888 | 2.7142 / 3.1728 |
| Exact ilk sayfa | 0.1908 / 0.2346 | 0.1953 / 0.2869 | 0.2026 / 0.2933 | 0.1909 / 0.2559 |
| İkinci sayfa | 0.1895 / 0.2547 | 0.1968 / 0.3075 | 0.2015 / 0.3099 | 0.1908 / 0.2386 |
| Range | 0.3993 / 0.5000 | 0.4044 / 0.5804 | 0.4160 / 0.6236 | 0.3943 / 0.4919 |
| Son sayfa | 1.2951 / 1.5680 | 1.2734 / 1.4639 | 1.2462 / 1.2995 | 1.2887 / 1.3925 |
| Geniş metin | 204.4385 / 206.4151 | 228.0659 / 230.9446 | 207.1633 / 208.7573 | 203.8388 / 208.4638 |
| 100 key okuma | 1.4608 / 1.5207 | 1.5563 / 1.6917 | 1.5139 / 1.6134 | 1.5227 / 1.7191 |

## Karar

Çalışma kilitlerini kaldırmak full'e göre toplam yazmayı %11,2 azaltıyor;
medyan working set yalnız 1,258 MiB yükseliyor. Her dört
koşuda da no-slots toplam yazması full'den düşük. Bu corpus'ta kilitlerin bellek
faydası maliyetine göre küçük; güncel deneysel umbrella'dan çıkarıldı. Ayrı
maintenance-slots özelliği ölçüm ve ilerideki gerçek byte bütçesi için korunur.
Bu seçim sınırsız veya her corpus'ta aynı RAM garantisi vermez.

Merge politikasını çıkarmak toplam yazmayı yaklaşık %11 ve working set'i
9,666 MiB artırıyor; politika korunur. Spool'u çıkarmak toplam yazmayı yaklaşık
%3,7 artırıyor, working set kazandırmıyor; spool da korunur. Private committed
sayaçları working set ile aynı eğilimi zorunlu olarak göstermez; allocatör ve
sayfalama etkileri vardır, bunlardan builder'ın tek başına byte tüketimi çıkarılmaz.

Kilitleri çıkarmak geniş metin p50'sini 204,4385 → 203,8388 ms yaptı.
Önceki V2'nin sorgu gerilemesini yalnız bu kilitlere bağlamak doğru değildir:
sorgu sırasında bu kilitler tutulmaz ve dört varyant aynı sorgu kodunu kullanır.
Bu turdaki daha düşük süreler önceki tur ile doğrudan karşılaştırılmaz; farklı
koşu koşulları/dalgalanma etkileri ayrıştırılmadı. Baseline ile yeni çiftler
aynı turda ayrıca ölçülür.

## Seçilen V3: aynı turda baseline karşılaştırması

Üç bağımsız çiftin sırası baseline/candidate, candidate/baseline, baseline/candidate.
Baseline frozen D84D82... önceki streaming-open binary; V3, ölçülen no-slots
binary'sinin byte düzeyinde kopyasıdır. Corpus/query/cache sözleşmesi aynıdır.

| Ölçüm | Baseline | V3 | Değişim |
| --- | ---: | ---: | ---: |
| Ingest | 8.4186 s | 7.7034 s | -8.5% |
| Son bakım | 7.0722 s | 4.7644 s | -32.6% |
| Yazma + bakım | 15.4908 s | 12.4678 s | -19.5% |
| Working set tepe | 86.4961 MiB | 68.0000 MiB | -21.4% |
| Private tepe | 86.7188 MiB | 68.3633 MiB | -21.2% |

| Sorgu | Baseline p50 / p95 ms | V3 p50 / p95 ms |
| --- | ---: | ---: |
| Dar metin | 2.7710 / 3.7165 | 2.6826 / 3.1907 |
| Exact ilk sayfa | 0.1906 / 0.2318 | 0.1883 / 0.2181 |
| İkinci sayfa | 0.2013 / 0.2767 | 0.1995 / 0.3300 |
| Range | 0.3989 / 0.4917 | 0.3959 / 0.5371 |
| Son sayfa | 1.3971 / 1.7820 | 1.2434 / 1.2611 |
| Geniş metin | 220.0136 / 224.5822 | 198.8911 / 203.9442 |
| 100 key | 1.4996 / 1.6271 | 1.4062 / 1.4817 |

Yazma+bakım %19,5, working set %21,4 azaldı. Bu üç çiftte bütün sorgu p50
medyanları baseline'dan düşük. Fakat ikinci sayfa p95 0,2767 → 0,3300 ms,
range p95 0,4917 → 0,5371 ms yükseldi. Bütün kuyruk hız kapıları geçti denmez;
çok küçük sürelerin OS/I/O dalgalanması daha fazla tekrarla ayrıştırılmalıdır.
SQLite bu turda çalıştırılmadı; SQLite'tan düşük toplam RAM hedefi tamamlanmadı.
Yeni tam byte bütçesi de henüz uygulanmadı. Düzeltilen konu, kaba bakım
kilitlerinin ölçülen hız/RAM dengesidir; normal default feature seti değişmedi.

Ham çiftler `write-selected-v3-20261008-{baseline,candidate}-{0,1,2}` JSONL/memory;
medyanlar `write-selected-v3-20261008.summary.json`. Bileşen ölçümlerinin
medyanları `write-components-20261008.summary.json`. Klasör boyutları eski,
inaktif run/cache'leri de içerir ve canlı veri boyutu olarak kullanılmaz.



## Doğrulama

- `cargo fmt --check`: geçti.
- `cargo test --all-targets --features experimental-write-pipeline`: 68 test geçti.
- `cargo test --all-targets --all-features`: 68 test geçti.
- `cargo test --all-targets`: 50 test geçti.
- Dört ölçüm feature setinin her biri için `cargo clippy --all-targets --features ... -- -D warnings`: geçti.
- `cargo clippy --all-targets --all-features -- -D warnings`: geçti.
- `cargo clippy --all-targets -- -D warnings`: geçti.

Bozulma, kesilme, süreç sonlandırma, cache rebuild, worker sürerken yazma,
görünürlük/sıralama/version ve değişmeden açılan 0.1 fixture testleri korunur.
Bu turdaki 22 native benchmark prosesinin tamamı başarıyla bitti ve bütün
oracle kontrollerini geçti. Windows elektrik kesintisinde dizin metadata
kalıcılığı ölçülmedi; yeni bir kalıcı format tanımlanmadı.

- `cargo build --release --features experimental-write-pipeline --bin fdb`: geçti;
  deneysel CLI `output/fdb-write-pipeline-cli-v3-20261008.exe` olarak saklandı.
- `cargo build --release --bin fdb`: geçti; `target/release/fdb.exe` normal
  default build olarak yeniden oluşturuldu.

Ölçülen native V3 explicit bileşen feature setiyle derlenmiştir; umbrella CLI
aynı davranışı içerir ancak ayrı binary'dir. CLI/IPC performansı bu native
rakamlarla ölçülmüş sayılmaz.

## Derleme setleri

- full: merge-policy + record-spool + maintenance-slots + merge-slot.
- no-policy: record-spool + maintenance-slots + merge-slot.
- no-spool: merge-policy + maintenance-slots + merge-slot.
- no-slots: merge-policy + record-spool + merge-slot.

Bütün adlar tabloda belirtilen `experimental-write-` önekiyle kullanılır.

## Yeniden üretim

Derleme feature setleri rapordaki tabloda tanımlıdır; frozen binary'ler
`output/fdb-ablation-20261008-{full,no-policy,no-spool,no-slots}.exe`.
`tools/measure-write-components.ps1` her biri için yeni DB dizini açar ve
ham JSONL/memory JSON dosyalarını saklar. Eski ölçümler değiştirilmez.

```powershell
./tools/measure-write-components.ps1 -Prefix write-components-20261008
python tools/summarize-native-pairs.py docs/benchmarks/write-components-20261008 --engines full no-policy no-spool no-slots --runs 4
```

Frozen native SHA-256:

- `output/fdb-ablation-20261008-full.exe`: `15EDCED8664ACCD2FD5F548D45AB0F4FD9C098769E3CE5E49092A6C99962ED6C`
- `output/fdb-ablation-20261008-no-policy.exe`: `6C6548699C4D6865298B5AB0A65FFF2A2FB856853DD2093D2EEFD9B302B3DF72`
- `output/fdb-ablation-20261008-no-spool.exe`: `762CC44B9914CE68035E9E72D9E7B0370B7E5947F58728939849D147CDFBFA0A`
- `output/fdb-ablation-20261008-no-slots.exe`: `BBA24EB5C3B6DAC78CA69F9DE8292209639693D7546795C53F9508E3A498009B`


Frozen V3 native `output/fdb-write-pipeline-v3-20261008.exe` SHA-256: `BBA24EB5C3B6DAC78CA69F9DE8292209639693D7546795C53F9508E3A498009B`.

Sonraki iş gerçek delta/builder/worker byte bütçesidir. Bu turda kaldırılan
kaba eşzamanlılık sınırı, allocation tüketimine göre kaynak ayıran bir bütçe
yerine geçmiyordu. Posting tekrarını azaltma hedefi de devam ediyor.

`output/fdb-write-pipeline-cli-v3-20261008.exe` SHA-256: `D990FEC341A60D9F4FB65B867683423603E03DA981FC4DF46CEF1171F219FD27`.

`target/release/fdb.exe` SHA-256: `B0B4A112F1B51D5F84D23F1D32E7A1D992AF25D1D0EAEF8E09C6F5B4BE0A4E81`.
