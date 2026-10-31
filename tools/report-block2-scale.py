"""Write a failed first-checkpoint report only from audited captured evidence."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOGS = ROOT / 'docs/benchmarks'
P = 'block2-scale-20261028-v1'


def main():
    d = json.loads((LOGS/f'{P}-audit-utf8-v2.json').read_text(encoding='utf-8'))
    assert not d['core_gates_pass'], 'Passing checkpoint requires continuing Block2'
    kill = json.loads((LOGS/'block2-kill-50k-20261028-v1-summary.json').read_text(encoding='utf-8'))
    assert all(r['coverage_observed'] and r['reopen_full_payload_matrix_oracle_exit_code']==0 for r in kill['results'])
    assert kill['old_reader']['oracle'] and kill['protected_unchanged']
    queries = d['queries']
    failed = [q for q in queries if not q['linear_target_pass']]
    fdb50 = [t for t in d['trials'] if t['engine']=='fdb' and t['n']==50000]
    common = [q for q in queries if q['case']['base'] in (16,17,18)]
    catalog = json.loads((LOGS/f'{P}-corpus-index.json').read_text(encoding='utf-8'))
    out = [
        '# Blok 2 — 50k ölçek kontrol kapısı, 28 Ekim 2026', '',
        f'**KALDI.** İki dengeli tekrarın pooled p95 sonucunda {len(failed)}/38 sorgu,',
        '10k süresinin 5 katını aşıyor. Aşağıdaki RAM/ortak kelime kapıları ayrı',
        'değerlendirildi. Kullanıcının durma kuralı uygulanarak 100k, 2b gerçek-soğuk,',
        '2c uzun snapshot ve kalan 50k yazma yükleri başlatılmadı. Motor optimizasyonu,',
        'kalıcı biçim/varsayılan değişikliği, Blok3/4 ve C ABI yapılmadı.', '',
        '| Kapı | Ölçülen | Durum |', '|---|---|---|',
        f"| Temiz idle private ≤15 MB | max {max(t['idle_private_bytes'] for t in fdb50)/1e6:.3f} MB ({max(t['idle_private_bytes'] for t in fdb50)/1048576:.3f} MiB) | {'GEÇTİ' if d['gates']['idle_15_decimal_MB'] else 'KALDI'} |",
        f"| Akışlı ingest tepe private ≤60 MB | max {max(t['ingest_peak_private_bytes'] for t in fdb50)/1e6:.3f} MB ({max(t['ingest_peak_private_bytes'] for t in fdb50)/1048576:.3f} MiB) | {'GEÇTİ' if d['gates']['ingest_peak_60_decimal_MB'] else 'KALDI'} |",
        f"| Ortak kelimeler p95 ≤30 ms | the/man/he ve bütün ortak kelime sayfalarında max {max(q['fdb_50000_p95_ms'] for q in common):.3f} ms | {'GEÇTİ' if d['gates']['common_p95_30_ms'] else 'KALDI'} |",
        f"| 10k→50k arama artışı ≤5× | {len(failed)}/38 aşıyor; tam tablo aşağıda | KALDI |",
        '| 50k tam payload/version + 38-case oracle | İki FDB/iki SQLite koşusu, her zamanlı tekrar sonrası kontrol | GEÇTİ |',
        '| FDB 50k process kill/reopen | Iterator, flush, merge, cache; dört gözlenmiş pencere | GEÇTİ |',
        f"| Donmuş eski okuyucu | {kill['old_reader']['full_payload_version_gets']} canlı get, {kill['old_reader']['deleted_gets']} silinmiş get, 38 sorgu; kaynak byte'ları aynı | GEÇTİ |",
        '| Tekli commit / dirty500 / snapshot+yazıcı / 100\'lü commit profili | Ölçek kontrol kapısında duruldu | ÖLÇÜLMEDİ |',
        '| 100k, gerçek-soğuk, 10dk snapshot / maks ömür API\'si | Sonraki koşulara geçilmedi | BAŞLANMADI |', '',
        '## Yöntem ve sınırlar', '',
        '- Aynı mevcut `json.payload-bench` havuzu; ilk10k/50k. Tam havuzda100000',
        '  kayıt CRC\'si doğrulandı,100000 farklı gövde mevcut kitap pasajlarıyla',
        '  eşleşti. Yeni corpus/indirme/çoğaltma yok. Sekiz kitabın örtüşen pasajları;',
        ' 100k bağımsız dosya temsilciliği iddiası yok. İlk10k descriptor/38-case',
        '  listesi eski metadata ile birebir aynı; yeni dosya yalnız oracle metadata\'sı.',
        '- Her N\'de FDB→SQLite ve SQLite→FDB; her koşu yalnız sabit kendi DB slotunu',
        '  sahiplik/reparse kontrolüyle sıfırlar. DB kopyası saklanmaz. Her iki N\'de',
        '  iki ingest ve iki query koşusu/backend;10 zamanlı tekrar/case/koşu,',
        '  pooled20 örnek, nearest-rank p95 (19. örnek). Tek uç örnekler ham logdadır.',
        '- Akışlı yazma500 kayıt/commit. Küçük aktif-WAL opt-in açık, reclaim kapalı;',
        '  kalıcı WAL ve FULL sync değişmedi. Ürün varsayılanları değişmedi.',
        '- SQLite FTS5 trigram/detail=none, external-content+triggers, FULL WAL,',
        '  cache2MiB, mmap0, tempFILE. Aynı sonuç sırası, total ve payload/version.',
        '  Önceki referansın LIKE/AND yolu korunur: count(*) ve sıralı sayfa için',
        '  iki sorgu; bütün olası SQLite MATCH planlarının en hızlısı iddiası yok.',
        '- **Sıcak OS cache.** Query öncesi bütün get oracle\'ı, ardından her case için',
        '  bir zamanlanmayan warmup; sonuç oracle\'ı zaman penceresi dışında fakat',
        '  OS cache/CPU\'yu etkiler. Cache eviction, güç kesintisi ve arka plan',
        '  süreç izolasyonu yapılmadı. Bu koşular cold cache sayılmaz.',
        '- Idle ayrı yeni proseste corpus/model yüklenmeden,2sn bekleyerek alınır.',
        '  Ağır arama/yazma sonrası aynı proseste idle geri dönüşü ölçülmedi.',
        '  Ingest tepesi bütün proses: catalog/model dahil, resident corpus gövdeleri',
        '  yok. OS lifetime peak private commit ve peak working set ayrı; working',
        '  set paylaşılan sayfaları da içerir. Motor bütçe sayaçları RSS sayılmaz.',
        '  OS dosya önbelleği/sistem toplam RAM\'i bu proses sayaçlarına dahil değildir.',
        '  Kapılarda MB=1000000 byte, tablolarda MiB=1048576 byte.', '',
        '## Her koşunun kaynak ölçüleri', '',
        '| N | Motor / tekrar | Ingest+bakım s | Commit toplam s | Bakım s | Idle private / WS MiB | Tepe private / WS MiB | Disk MB | Temiz strict-open* ms |',
        '|---:|---|---:|---:|---:|---:|---:|---:|---:|']
    for t in d['trials']:
        out.append(f"| {t['n']} | {t['engine']} / {t['pair']} | {t['ingest_ms']/1000:.3f} | {t['commits_ms']/1000:.3f} | {t['maintenance_ms']/1000:.3f} | {t['idle_private_bytes']/1048576:.3f} / {t['idle_ws_bytes']/1048576:.3f} | {t['ingest_peak_private_bytes']/1048576:.3f} / {t['ingest_peak_ws_bytes']/1048576:.3f} | {t['retained_bytes']/1e6:.3f} | {t['open_ms']:.3f} |")
    out += ['', '*FDB strict-open kaynak/CRC kontrolünü korur. SQLite açılışı aynı strict',
            'doğrulama işi değildir; sözleşme farkı saklanmadı. İki motor da temizdir;',
            'dirty500 ve cache\'i düşürülmüş açılış sonucu değildir.', '',
            '## 38-case pooled p95 — bütün iyi/kötü sonuçlar', '',
            '| Case / sorgu / offset / limit | FDB10k ms | FDB50k ms | Artış | Mutlak fark ms | SQLite10k ms | SQLite50k ms | ≤5× |',
            '|---|---:|---:|---:|---:|---:|---:|---|']
    for q in queries:
        c=q['case']; text=c['text'] if len(c['text'])<40 else '(zx)×150'
        out.append(f"| {q['case_id']} / {text} / {c['offset']} / {c['limit']} | {q['fdb_10000_p95_ms']:.4f} | {q['fdb_50000_p95_ms']:.4f} | {q['fdb_growth']:.3f}× | {q['fdb_absolute_delta_ms']:+.4f} | {q['sqlite_10000_p95_ms']:.4f} | {q['sqlite_50000_p95_ms']:.4f} | {'GEÇTİ' if q['linear_target_pass'] else 'KALDI'} |")
    out += ['', '## Aşımın ne kadarı açıklandı?', '',
            'Belge sayısı5×, özgün gövde byte\'ı5,0067×, segment sayısı5→25.',
            'Sabit ilk-N havuzunda sorgu başına eşleşme dağılımı birebir aynı değildir:', '',
            '| Sorgu | Eşleşme10k | Eşleşme50k | Eşleşme artışı | Zaman artışı |',
            '|---|---:|---:|---:|---:|']
    for i in (2,5,6,7,12,19,21,34,37):
        q=queries[i]; c=q['case']; bit=1<<c['base']
        a=sum(bool(e['mask']&bit) for e in catalog['entries'][:10000])
        b=sum(bool(e['mask']&bit) for e in catalog['entries'][:50000])
        growth=f'{b/a:.3f}×' if a else '0→0'
        out.append(f"| {i}: {c['text']} / {c['offset']} / {c['limit']} | {a} | {b} | {growth} | {q['fdb_growth']:.3f}× |")
    out += ['', 'Eşleşme dağılımı bazı artışları kısmen açıklar; bütün aşımı açıklamaz.',
            f"Case37/quixote offset57 ilk10k'da {queries[37]['page_hits_10000']} hit döndürürken50k'da {queries[37]['page_hits_50000']} hit döndürür; bu sayfada sonuç payload okuma işi de değişir.",
            'Segmentlerin artışı, sabit sayfa/sözlük bütçesi ve aday doğrulama yolunun',
            'payları bu koşuda aşama aşama ölçülmedi. Tek kök neden ilan edilmiyor.',
            'Özellikle kısa negatif sorguların oranı mutlak ms farkıyla birlikte',
            'okunmalı; bu yeni ölçek kapısına kendiliğinden +2ms istisnası eklenmedi.',
            'Sonraki dar öneri: aynı10k/50k\'da posting/metadata/sözlük cache miss,',
            'payload okuma/CRC ve text doğrulama sayı/süre profili; ölçümden sonra',
            'format değiştirmeyen çözüm önerisi. Bu rapor optimizasyon yaptı iddiası taşımaz.', '',
            'Koşular arası dalgalanma da korunur: SQLite50k matrix proses süresi',
            '745,328s ve1020,297s; FDB50k ingest+bakım76,628s ve65,365s.',
            'Case23 yalnız5,010× ile sınıra çok yakındır; bütün15 aşımın aynı',
            'önemde veya gürültüden bağımsız olduğu söylenmez. Buna karşılık',
            'beagle/harpoon/invertebrata gibi sorguların artışı belirgindir.', '',
            '100\'lü commit p95≈1sn/78MiB önceki10k bulgusudur;50k\'ya taşınmış',
            'veya kök nedeni kanıtlanmış sayılmaz. İstenen50k profili, tekli commit,',
            'dirty500 ve4R+yazıcı ölçümü henüz yapılmadı. Durma kapısının ardından',
            'devam yönünü kullanıcı belirler.', '',
            '## Doğrulama ve ham kanıt', '',
            f"- Yeni cargo paketinde {d['tests']['passed']} test geçti, {d['tests']['failed']} başarısız, {d['tests']['ignored']} subprocess yardımcısı ignored. fmt --check, clippy --all-targets -- -D warnings, no-default/all-features ve native SQLite kontrolleri geçti.",
            '- Aktif-WAL/prefix yayım, kaynak kesilme/bozulma, process-kill, eski0.1',
            '  fixture ve snapshot/pin regresyonları yeniden çalıştı. Ayrıca gerçek',
            ' 50k DB\'de dört gözlenmiş process-kill penceresi sonrası tam oracle geçti.',
            '  Cache onarımında kaynak hash\'leri aynı; frozen24Ekim okuyucu bütün50k',
            '  canlı payload/version ve38-case sonuçlarını doğruladı. Güç kesintisi',
            '  iddiası yok; SQLite50k process-kill bu tur ölçülmedi.',
            '- Motorun64 Rust kaynak SHA\'sı kabul edilmiş Blok1 ile aynı. Corpus ve',
            ' 0.1 fixture hash\'leri korunuyor. Kullanıcı verisi/entegrasyon yok.',
            f'- [Havuz CRC/benzersizlik kanıtı](benchmarks/block2-20261028-corpus-audit.json).',
            f'- [Sıra/ayarlar/program hash\'leri ve24 proses sonucu](benchmarks/{P}-design.json).',
            f'- [Kapı denetimi, bütün trial/query değerleri ve kaynak SHA\'ları](benchmarks/{P}-audit-utf8-v2.json).',
            f'- Her raw log: `docs/benchmarks/{P}-n{{10000|50000}}-r{{0|1}}-{{fdb|sqlite}}-scale-{{seed|idle|matrix}}.jsonl`; aynı kökte `.stderr` ve `.process.json`.',
            '- [Dört50k kill/old-reader kanıtı](benchmarks/block2-kill-50k-20261028-v1-summary.json).',
            '- [Yeni doğrulama komutları](benchmarks/snapshot-10k-20261026-validation-block2-scale-20261028-v1.json).', '',
            'Kill sonrası tek maintain_idle() çağrısı sırasında kısa snapshot pini',
            'tutuldu; eski snapshot50k payload/version ve yeni durum geçti. Son pin',
            'bırakılınca unreferenced kaynak ve input/build/spill artık dosyaları0byte.',
            'Son temiz50k durum eski okuyucuyla tekrar tam doğrulandı.',
            '[Temizlik/pin kanıtı](benchmarks/block2-cleanup-50k-20261028-v1-summary.json).',
            'Bu yalnız kurtarma temizliği kontrolüdür;4R+yazıcı veya10dk snapshot',
            'performans kapısı sayılmadı.', '',
            '## Ölçüm aracındaki iki düzeltme — eski kanıtlar silinmedi', '',
            '- İlk merge-kill denemesinde `start_compaction()` öncesi ek100 kayıt',
            '  aktif WAL\'da kaldı; ikinci küçük immutable run oluşmadığı için merge',
            '  hiç başlamadı.30sn timeout sonrası oracle geçti fakat coverage=false;',
            '  bu deneme merge-kill geçti sayılmadı. Retry yalnız fixture\'da ürün',
            '  varsayılanı küçük aktif-WAL katmanı kapalıyken iki küçük run oluşturdu.',
            '  Kalıcı WAL/sync açık, motor aynı; gerçek merge-build penceresinde kill',
            '  ve oracle geçti. Raw missed/retry dosyaları ve ayar notu korunur.',
            '- Python\'ın cp1252 varsayılanı metadata\'daki `İ` harfini yanlış',
            '  çözdü. İlk eski-CLI kontrolünde case26/27 için yanlış sorgu gönderildi;',
            '  50k get doğrulaması geçti ama bu iki query kontrolü reddedildi. Bu bir',
            '  motor/format hatası olarak sunulmaz. Tüm ilgili dosya okumaları açık',
            '  UTF-8 yapıldı; eski okuyucu50k get ve38 doğru sorguyla yeniden geçti.',
            '  İlk audit\'in iki query etiketi düzeltilmiş UTF8-v2 audit ile',
            '  değiştirildi; v1 dosyası korunur. Bütün sayısal trial/query/gate',
            '  değerlerinin birebir aynı kaldığı v2 audit\'te assert edildi.',
            '- [Encoding düzeltme kanıtı](benchmarks/block2-scale-20261028-v1-python-encoding-correction.json),',
            '  [kaçan merge](benchmarks/block2-kill-50k-20261028-v1-merge-attempt.json),',
            '  [retry ayarı](benchmarks/block2-kill-50k-20261028-v1-merge-retry1-scope.json).', '',
            'Kill koşuları son FDB slotunun modelini ilerletir; zamanlı trial logları',
            'ayrıdır. Slot pointer çalışma durumunu gösterir, geçmiş zamanlı DB kopyası',
            'saklanmaz. Önceki case34, geniş zx/the zx, parçalı okuma1112→2013,',
            '100\'lü4R RAM/gecikme ve bakımın kalan sınırları OPEN-WORK\'te açıktır.', '']
    target=ROOT/'docs/BLOCK2-SCALE-50K-20261028.md'
    assert not target.exists()
    target.write_text('\n'.join(out),encoding='utf-8')
    print(target)


if __name__=='__main__':
    main()
