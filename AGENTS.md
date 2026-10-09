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
