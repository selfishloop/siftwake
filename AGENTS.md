# FDB çalışma kapsamı

Bu repository bağımsız, Rust ile yazılan DB motorudur. SDB ve Fundops yalnız
veri modeli ihtiyaçlarına referanstır. Kullanıcı açıkça proje entegrasyonunu
kapsam dışında bıraktı; bu görev kapsamında dış projeleri değiştirme, servislerine
bağlanma veya veri taşıma yapma.

Çalışma öncesi `README.md`, `docs/ARCHITECTURE.md`, güncel doğrulama belgesi ve
`git status` okunur. Mevcut değişiklikler korunur. Hazır DB veya arama motoru
(SQLite, RocksDB, LMDB, sled, Tantivy vb.) eklenmez. Genel serileştirme/runtime
kütüphaneleri bu yasağın parçası değildir.

`.fdb` ve `.ready` dosyaları kalıcı verinin kaynağıdır. `.idx` türetilmiş cache'tir;
bozuk kaynağı sağlam cache var diye kabul etme. Yeni kalıcı format değişiklikleri
sürüm ve uyumluluk testi gerektirir. Test fixture'larındaki 0.1 kaynağını değiştirme.

Değişiklikle ilgili testler, `cargo fmt --check` ve
`cargo clippy --all-targets -- -D warnings` çalıştırılır. Depolama/kurtarma
değişikliklerinde kesilme, bozulma ve süreç sonlandırma davranışı test edilir.
Performans sonuçlarında corpus, query, cache durumu ve ölçülmeyen sınırlar açıkça
belirtilir. RAM sayaçları toplam proses belleği gibi sunulmaz.

## Test verisi ve disk sınırı — 2026-10-12 kullanıcı kararı

Yeni belge corpus'u üretme veya indirme; indeks testlerinde mevcut
`data/rkyv-corpus-20261009/json.payload-bench` içindeki aynı ilk 10.000 belgeyi
kullan. Corpus hazırlama araçlarını yeniden çalıştırma. Testleri D: veya başka
bir diske taşıma. Tarihli/prefix'li yeni veritabanı klasörleri biriktirme.
Aktif ölçümlerde yalnız `data/benchmark-work/fdb` ve
`data/benchmark-work/sqlite` sabit çalışma alanları kullanılır. Okuma/arama
testleri mevcut veritabanını yeniden kullanır; yazma/indeks oluşturma ölçümü
gerektiğinde yalnız bu araçlara ait çalışma alanı sıfırlanır. Sonuç logları
`docs/benchmarks` altında tutulur; her koşunun veritabanı kopyası saklanmaz.
Geçmiş çoklu-corpus/büyük ölçek benchmark script'leri bu kuralla uyumlu hale
getirilmeden çalıştırılmaz. Kod, raporlar ve 0.1 fixture temizliğe dahil değildir.

## Açık işleri hatırlatma — kullanıcı isteği

`docs/OPEN-WORK.md` iş listesini koru. Sonraki performans sonuçlarında güncel
işin yanında yazma/bakım, arama regresyonları, RAM/disk ve eşzamanlı yük gibi
açık kalan başlıkların kısa durumunu hatırlat. İşi ölçüm/kanıt olmadan kapatma.
