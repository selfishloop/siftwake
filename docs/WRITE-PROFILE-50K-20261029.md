# 50k yazma profili — 29 Ekim 2026

## Kullanıcı kapısı güncellemesi — 29 Ekim 2026

**Güncel karar: tekli yazma GEÇTİ.** Ham örnekler yeniden çalıştırılmadan p99 hesaplandı. Commit ve görünürlük için yeni kapı p95≤20ms,p99≤50ms,max≤100ms; bütün dört FDB tipi geçti. SQLite max şartı yoktur. mtime sil+ekle görünürlük max FDB76,633ms,SQLite65,796ms: FDB bu uçta10,837ms daha yüksek. Commit max karşılığı74,840/57,106ms. 74,840ms ucun nedeni hâlâ bilinmiyor; çözülmüş veya gürültü diye kapatılmadı.

| İşlem | Eski kapı | Yeni kapı |
|---|---|---|
| Sil | KALDI | GEÇTİ |
| Ekle | GEÇTİ | GEÇTİ |
| mtime: sil+ekle | KALDI | GEÇTİ |
| Güncelle | GEÇTİ | GEÇTİ |

Eski kapı: commit p95≤10ms/max≤50ms ve görünürlük max≤50ms. Yeni kapı: her iki metrikte p95≤20ms,p99≤50ms,max≤100ms. Yüzdelik tanımı nearest-rank; n=512/tip, iki dengeli kol pooled. Aşağıdaki ilk durma kararı tarihsel olarak korunur.

**KALDI; sonraki bağlı aşamalar durduruldu.** Tekli commit p95 dört işlem tipinde geçti. Ancak sil+ekle görünürlük max76,633ms, silme50,221ms; kabul edilen50ms sınırı aşıldı. Dirty500,4R+yazıcı ve100’lü commit profili çalıştırılmadı. Optimizasyon yapılmadı.

Ölçülen iki kök maliyet:500’lü fallback commit’te önceki merge worker’ını beklemek; tekli tanı tekrarında ise önceki sequence’in snapshot görünürlük maskesini hazırlamak. İlk74,840ms örneğin kesin aşaması profil kapalı koşuda kaydedilmedi; onu çözülmüş veya ölçüm gürültüsü diye sunmuyoruz.

## Kapsam ve yöntem

Kaynak SHA256 `881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12`; aynı sağlanan korpusun yalnız ilk50k belgesi. `private/` okunmadı;100k DB koşusu, yeni corpus, DB kopyası, format/256KiB sınır değişikliği yok. Sabit C: `data/benchmark-work/fdb` ve `sqlite` alanları ownership kontrolüyle yeniden kullanıldı. Varsayılan exact/serial, opt-in melez/cancel ve kapalı reclaim değişmedi. Akışlı API, synced WAL; SQLite FTS5 trigram/detail=none/columnsize=0, WAL/FULL,cache2MiB,mmap0,tempFILE.

Tekli yük:256 işlem/tip/kol; iki dengeli sıra FDB/SQLite ve SQLite/FDB; toplam512 örnek/tip/motor. Yeni gövdeler2–4/4–16/16–64/64–200KB boyut sınıflarından seçildi; ilk50k’daki en uzun199999B donör dahil. Gövdeler kaynak dosyada değişmedi; runtime revizyonlarında özgün gövdeye tekil oracle etiketi eklendi; 200000B sınırına sığması için gerekirse UTF-8 sınırında son birkaç bayt kesildi. En uzun yeni metin200000B. Fields/links donörden gelir. Salt donör tekrarları ve revision etiketleri yeni bir corpus dosyası değildir. Anahtarlar mevcut numeric kimliklerdir; mtime kalıbı farklı iki numeric anahtarda aynı transaction’da delete+put ile temsil edildi. Gerçek16hex|uzun-yol anahtar maliyeti ölçülmedi.

Ölçüm öncesinde çıkacak kayıtlar revision1 ile hazırlanıp bakım tamamlandı. SnapshotReader açık tutuldu; commit sonrası yeni snapshot sequence/payload ve eski/yeni tekil metin oracle’ı doğrulandı. Görünürlük, yazma başlangıcından o snapshot’ın ilk doğrulanmış arama cevabına kadar; tüm ek GET ve oracle kontrolleri `verified_visibility_ms` alanında ayrıca yer alır. Silinen anahtar GET=None ve eski etiket total=0; güncellenen yalnız yeni etikette bulunur. Tam50k payload/version GET oracle’ı her kol sonunda geçti. Sıcak OS cache, eviction yok; uygulama/CPU/enerji ortamı tamamen izole edilmedi.

Bellek Windows process private commit/working set; katalog/model, snapshot, worker ve oracle dahil. Interleaved işlem tiplerinin bellekleri ayrı süreçlerde izole edilmedi: current max tipteki gözlem, OS lifetime peak ise o süreçte o ana kadarki tepedir.10ms örnekleyiciye ek OS peak kullanılır. Boşta bellek yeni süreçte oracle yüklenmeden ölçüldü. Profil kapalı kabul sonuçları ile tanı süreleri karıştırılmaz.

## Tekli yazma ve görünürlük

| Motor | İşlem | n | Commit p95 ms | Commit p99 ms | Commit max ms | Görünürlük p95 ms | Görünürlük p99 ms | Görünürlük max ms | Current private max MiB | Process peak private MiB | Peak WS MiB |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| fdb | Sil |512| 2.491 | 11.845 | 50.028 | 3.035 | 12.054 | 50.221 | 20.328 | 21.824 | 26.184 |
| fdb | Ekle |512| 4.073 | 6.450 | 19.167 | 7.791 | 12.095 | 19.648 | 20.328 | 21.824 | 26.184 |
| fdb | mtime: sil+ekle |512| 4.444 | 11.132 | 74.840 | 11.309 | 17.909 | 76.633 | 20.410 | 21.824 | 26.184 |
| fdb | Güncelle |512| 4.232 | 11.365 | 17.877 | 7.112 | 12.748 | 18.935 | 20.324 | 21.824 | 26.184 |
| sqlite | Sil |512| 15.224 | 32.865 | 91.675 | 23.868 | 39.583 | 101.234 | 18.125 | 19.078 | 23.547 |
| sqlite | Ekle |512| 21.054 | 59.039 | 81.806 | 31.147 | 69.065 | 91.496 | 18.125 | 19.078 | 23.547 |
| sqlite | mtime: sil+ekle |512| 21.342 | 32.972 | 57.106 | 32.345 | 45.899 | 65.796 | 18.125 | 19.078 | 23.547 |
| sqlite | Güncelle |512| 20.616 | 32.497 | 143.922 | 32.449 | 44.730 | 150.856 | 18.172 | 19.078 | 23.547 |

Bütün kötü FDB örnekleri:

| Kol/tip | Sequence | Commit ms | Görünürlük ms | Yeni/eski donör B |
|---|---:|---:|---:|---|
| single-pair0 /mtime: sil+ekle | 1070 | 74.840 | 76.633 | [76037]/[37319] |
| single-pair1 /Sil | 1072 | 50.028 | 50.221 | []/[16918] |

## Ingest: kısa aynı-oturum A/B

Geçmiş TR38,44–39,13sn ve cap200k51,97–57,98sn koşularının ikisinde de WAL katmanı açık, 500 işlem/commit ve son idle cleanup aynıydı. Bu nedenle ayar değiştirilmiş gibi bir neden kurulmuyor. Bu tur aynı881756DA kaynağıyla eski/güncel donmuş ikililer birer kez FDB/SQLite ve SQLite/FDB sırasıyla çalıştırıldı. Ek güncel WAL-off FDB kontrolü yine500 işlem/commit. Tek örnek/ayar, düşük yüzdeler için istatistiksel üstünlük veya ortamın kesin alt nedeni değildir.

| Kol | Motor | Ingest+bakım sn | Commit toplam sn | Bakım sn |500lü p95/max ms | Peak private MiB |
|---|---|---:|---:|---:|---|---:|
| ab-old | fdb | 55.395 | 48.142 | 6.759 | 1438.858/1513.345 | 22.000 |
| ab-old | sqlite | 46.474 | 44.495 | 1.978 | 608.531/1122.016 | 18.008 |
| ab-current | sqlite | 46.841 | 44.849 | 1.992 | 628.088/1167.296 | 17.973 |
| ab-current | fdb | 56.167 | 49.053 | 6.699 | 1378.846/1559.844 | 20.160 |
| ab-current-wal-off | fdb | 55.303 | 48.074 | 6.846 | 1411.568/1539.351 | 19.012 |
| diag500 | fdb | 57.300 | 49.651 | 7.271 | 1460.985/1658.442 | 19.391 |
| diag500 | sqlite | 46.747 | 44.764 | 1.983 | 640.019/1124.752 | 17.973 |
| single-pair1-seed | sqlite | 66.085 | 56.064 | 10.021 | 1174.943/4006.220 | 17.988 |
| single-pair1-seed | fdb | 59.533 | 52.221 | 6.854 | 1460.303/1791.201 | 20.590 |

Tabloya kurulum/diagnostic ve ikinci tekli-kol seed’leri de dahil: ikinci SQLite seed83sn süreç süresine çıktı; kötü/yavaş seed gizlenmedi. Ingest kolonu Rust ölçümüdür, `.process.json` wall süresi başlatma ve ölçüm sonrası tam oracle’ı da içerir.

Eski ikili de bugün55,395sn; güncel56,167sn. WAL-off55,303sn. Eski/güncel fark≈%1,4, WAL-off fark≈%1,5: bu tek çiftle küçük kazanç iddiası yok. Tarihsel yaklaşık%45 artış yeni kod/ayar farkı olarak yeniden üretilmedi; SQLite da46–47sn ile eski36sn düzeyine inmedi. **Ortam/oturum koşulları güçlü adaydır (çıkarım); geçmiş CPU/frekans/I/O telemetrisi olmadığından kesin ortam nedeni kanıtlanmadı.** Kaynak ilk50k’da yalnız45192’nin637B kırpılması bakımından eski TR girdisinden farklıdır; eski girdiyi bu tur yeniden çalıştırıp birebir korpus A/B’si yapılmadı.

## 500’lü kuyruğun ölçülen aşamaları

100 diagnostic commit toplamı: 49.603sn. Aşağıdaki foreground span’leri sıralı kapsamları gösterir; alt/worker span’leriyle toplanmaz. Wall süresidir, CPU payı değildir.

| Foreground aşama | Toplam sn | Commit toplamının yüzdesi |
|---|---:|---:|
| stream.stage_entry | 18.439 | 37.17% |
| stream.finish_frame_crc | 0.071 | 0.14% |
| stream.fallback_worker_drain | 15.567 | 31.38% |
| stream.fallback_delta_flush | 0.078 | 0.16% |
| stream.fallback_post_flush_drain | 0.000 | 0.00% |
| stream.fallback_transaction_publish | 13.274 | 26.76% |

Stage giriş süresi entry kodlama/CRC ve indeks hazırlığını kapsar; caller donör JSON okuma/çözme ve diğer commit işleri kalan sürededir. Transaction publish; WAL, kaynak/index yazımı, manifest ve sync içerir. Aşağıdakiler iç içe veya worker ile örtüşebilir; yüzde olarak toplanmaz.

| İç/worker sayaç | Toplam sn |
|---|---:|
| merge.worker | 21.434 |
| merge.join_wait | 15.232 |
| index.change_total | 16.197 |
| builder.finish_delta | 8.385 |
| builder.finish_reused | 16.070 |
| source.encode_write_frames | 3.116 |
| segment.open | 7.718 |
| open.index_binary_directory_parse | 4.576 |
| stream.wal_copy_and_sync | 0.355 |
| stream.wal_sync | 0.178 |
| source.checkpoint_sync | 0.388 |
| builder.index_sync | 0.489 |
| manifest.publish | 0.310 |
| manifest.sync | 0.091 |

| En yavaş500lü sequence | Commit ms | Worker drain ms | Stage entry ms | Publish ms | WAL sync ms | Manifest sync ms |
|---|---:|---:|---:|---:|---:|---:|
| 98 | 1658.442 | 1299.135 | 197.125 | 137.091 | 6.122 | 1.534 |
| 66 | 1656.495 | 1296.685 | 209.562 | 126.645 | 1.266 | 1.828 |
| 18 | 1598.782 | 1217.466 | 222.471 | 133.692 | 1.534 | 2.762 |
| 50 | 1565.583 | 1188.187 | 219.312 | 132.559 | 1.497 | 1.994 |
| 34 | 1521.496 | 1188.323 | 171.846 | 139.569 | 1.314 | 1.775 |

En pahalı kuyruğun doğrudan kanıtı worker drain: örneklerde1,2–1,3sn. WAL sync1–6ms bu uçların baskın nedeni değil. İndeks hazırlığı toplam maliyette ayrıca büyüktür.100’lü commit aynı yola düşer varsayımı yapılmadı; o profil tekli kapısı nedeniyle ölçülmedi.

| SQLite500 iç aşama | Toplam sn |
|---|---:|
| donor_ns | 1.110 |
| fold_encode_ns | 1.093 |
| sql_fts_ns | 37.113 |
| tx_commit_sync_ns | 5.354 |

SQLite `sql_fts` INSERT+trigger/FTS işi; `tx_commit_sync` COMMIT ve içeride gerçekleşebilen sync/checkpoint işidir. Saf fsync süresi diye sunulmaz; SQLite iç C aşamaları ayrıca profillenmedi.

## Tekli aşımın tanısı

Aynı mantıksal model hash’ini üreten bir1024-commit diagnostic tekrar yapıldı; source formatı aynı. Normal74,840ms /sequence1070, tanıda3,448ms oldu: aynı nadir örnek tekrar oluşmadı. Buna rağmen tanıda sequence1068 silme50,171ms/visibility50,396ms ile sınır tekrar aşıldı. Ölçülen35,528ms `snapshot.mask`,35,577ms `snapshot.prepare`; WAL sync0,581ms. İlk maskenin visible_sequence’i1067, sonraki küçük maskenin1068: önceki sürümün yayım hazırlığı yeni commit’e yükleniyor. Worker/poll yayımından gelen bu hazırlık maliyeti somut bir sonraki tanı/düzeltme hedefidir; ilk74,840ms’nin kesin nedeni olduğu söylenmez.

Diğer tanı örnekleri: prefix install15–16ms; WAL reader açma6,9–9,3ms. Kaynak/manifest işlerini kaldırmak veya güvenlik kontrolünü atlamak önerilmiyor. Bu tur hiçbir optimizasyon uygulanmadı.

## Kapılar ve durma kararı

| Kapı | Ölçüm | Karar |
|---|---|---|
| Tekli commit p95≤10ms | dört tip2,492–4,444ms | GEÇTİ |
| Önceki max commit≤50ms |74,840ms; ayrıca50,028ms | KALDI |
| Yazma→görünürlük≤50ms |76,633ms ve50,221ms | KALDI |
| Temiz idle≤15MiB |2.172MiB | GEÇTİ |
| Ölçülen akışlı tepe private≤60MiB | tekli21,824MiB; tüm normal seed’ler üstte | GEÇTİ |
| Dirty500 strict≤1000ms /idle≤15MiB | öncül tekli kapısı kaldı | ÖLÇÜLMEDİ |
|4R+yazıcı, loaded p95≤2×idle; idle<1ms için+2ms | öncül kapı kaldı | ÖLÇÜLMEDİ |
| Throughput≥%75 ve SQLite pooled üstü | öncül kapı kaldı | ÖLÇÜLMEDİ |
|100lü p95/max/RAM ve kök neden | öncül kapı kaldı; yeni kapı uydurulmadı | ÖLÇÜLMEDİ |
| Oracle/kill/eski okuyucu | aşağıdaki kapsam | GEÇTİ |

## Kurtarma, uyumluluk ve süreç kapanmaları

Yazılmış model üzerinde üç kez normal strict-open tanığı alınıp dirty WAL açıkken child kill edildi. Tanıkta255 delta anahtarı ve önceki tüm commit’ler vardı. Sonra strict reopen +tam50k GET/version ve arama oracle’ı geçti. Bu255-kayıt kurtarma kontrolü, durdurulan dirty500 performans koşusu değildir. Append/flush/merge ortasında yeni kill tanığı yok; elektrik kesintisi iddiası yok. Dondurulmuş eski okuyucu50k tam GET,512 silinmiş GET ve41 exact sorguda geçti; read-only kaynak SHA’ları değişmedi.0.1 fixture ve korunmuş eski corpus hash’leri aynı; bütün motor `src/*.rs` hash’leri kabul edilen sürümle aynı.

17 ana ölçüm worker’ı normal exit0 ile kapandı. Sonraki diagnostic seed/tekli de exit0. Üç kurtarma kill’i ayrı `intentional_kill` kaydında actor/target PID ile işaretli. Bu tur ana koşuda açıklanamayan worker kaybı gözlenmedi. Önceki oturum kesintisinin kesin nedeni geçmişe dönük kanıtlanamadı.

v2 gözlemcisindeki `driver_disappeared` satırı normal kapanıştan sonradır: driver_finish 12:45:57.335Z (STOP_single_failed), handle_exit12:45:57.887Z/exit0, ardından poll12:45:57.906Z. Stop dosyası da mevcuttu. Bu kayıt açıklanamayan yarıda kesilme değildir.

WMI ProcessStart/StopTrace aboneliği Call cancelled/Access denied verdi; yükseltme yapılmadı. Wrapper fsync’li PID/start/exit kayıtları ve salt-okuma process handle exit denetimi kullanıldı. İlk izleyicide lifecycle dosyasını paylaşım izni olmadan okumak hataya neden oldu; FileShare.ReadWrite ile düzeltilip v2 loguna yeniden başlatıldı. Bu kısa gözlem boşluğunda wrapper kayıtları kaldı; benchmark durmadı. System/Application ve oturum olay sorgularında ölçüm aralığında eşleşen olay bulunmadı. İzleyici de aynı oturumla öldürülürse son aktörü saptama garantisi yoktur.

Bir yardımcı girişimi ise gerçekten hata ile bitti: kirli WAL üzerinde temiz-WAL zorunlu OpeningDatabase prototipi çağrıldı, native hata `background opening prototype requires a clean active WAL`; boş stdout’u JSON sayan Python yardımcısı JSONDecodeError verdi. Hata logları korunup yalnız bu doğrulama kısmı doğru strict yolla tamamlandı; ölçümler tekrar çalıştırılmadı, hata motor çökmesi diye sunulmadı.

## Ham kanıt ve açık iş

- [Tasarım, ikili/source hashları ve tamamlanan bütün komutlar](benchmarks/write50-20261029-v1-design.json).
- [p95/max/RAM, foreground/worker sayaçları ve olayların tamamı](benchmarks/write50-20261029-v1-analysis.json).
- [Tekli kapı](benchmarks/write50-20261029-v1-single-gate.json); [kurtarma ve eski okuyucu](benchmarks/write50-20261029-v1-postgate.json).
- [Ana süreç kayıtları](benchmarks/write50-20261029-v1-lifecycle.jsonl), [handle/oturum gözlemcisi](benchmarks/write50-20261029-v1-process-events-v2.jsonl), [kurtarma kill nedenleri](benchmarks/write50-20261029-v1-recovery-lifecycle.jsonl).
- [243 test/fmt/clippy/feature ve SQLite kontrolleri](benchmarks/snapshot-10k-20261026-validation-write50-20261029-v4.json). Önceki v1 unused-import ve v3 harness-fmt geliştirme hataları da silinmedi.

Sonraki onay için öneri: önce snapshot yayımındaki maske hazırlığını, ardından500’lü fallback’in worker beklemesini hedefleyen dar değişiklik. Dirty500,4R+yazıcı ve100lü profil açık kaldı. Uzun snapshot, gerçek-soğuk cache, Blok3/4,C ABI, ön filtre ve entegrasyon başlamadı. Eski case34 (ishmael /130/1),the zx parçalı okuma ve signed-zero maddeleri ölçümsüz kapanmadı.
