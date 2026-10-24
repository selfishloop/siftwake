"""Render the current three-engine comparison directly from captured records."""
import csv
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'docs/benchmarks'


def load(name):
    return json.loads((OUT / name).read_text(encoding='utf-8-sig'))


def main():
    writes = load('consolidated-write-10k-20261024-summary.json')
    reads = load('consolidated-read-10k-20261024-summary.json')
    identity = load('consolidated-10k-20261024-identity.json')
    audit = load('consolidated-read-10k-20261024-recovery-audit.json')
    assert identity['baseline_commit'].startswith('8a92384')
    assert identity['current_commit'].startswith('2a9fc2c')
    assert writes['protected_unchanged'] and writes['fdb_sources_indexes_ready_byte_equal']
    assert reads['files_unchanged'] and audit['all_other_fields_equal']
    assert audit['saved_and_live_manifests_equal'] and len(audit['corrections']) == 8
    streamed = writes['apis']['streamed']
    owned = writes['apis']['owned']
    lines = [
        '# Aynı 10k ile toplu karşılaştırma — 24 Ekim 2026', '',
        'Eski FDB, güncel FDB ve native SQLite aynı oturumda, aynı mevcut ilk10k',
        'üzerinde yeniden ölçüldü. Eski FDB **8a92384**, güncel **2a9fc2c** normal',
        '`bounded-engine` release sürümleridir. Eski sürüm 24 Ekim gram sıralama,',
        'RAM posting ve merge cursor adımlarından hemen öncedir; hızlı intake fold',
        'dahildir. Bu ilk motor sürümünden bugüne kıyas değildir. Önceki raporların',
        'yüzdeleri toplanmadı. İki çift, istatistiksel anlamlılık veya genel hız garantisi değildir.', '',
        f'Akışlı yazma+bakım {streamed["total_ms"]["medians"]["control"]/1000:.3f}→{streamed["total_ms"]["medians"]["candidate"]/1000:.3f}s',
        f'(**{streamed["total_ms"]["candidate_change_percent"]:+.2f}%**), owned {owned["total_ms"]["medians"]["control"]/1000:.3f}→{owned["total_ms"]["medians"]["candidate"]/1000:.3f}s',
        f'(**{owned["total_ms"]["candidate_change_percent"]:+.2f}%**); ikisinde2/2 çift düşük. Buna karşılık ayrı son bakım',
        f'**{streamed["maintenance_ms"]["candidate_change_percent"]:+.2f}%/{owned["maintenance_ms"]["candidate_change_percent"]:+.2f}% ve iki API’de2/2 çift yüksek**: bakım işi açık kalır.', '',
        '## Yazma, son bakım, CPU ve OS bellek', '',
        'Her API için iki dengeli üçlü: eski/güncel/SQLite, ardından SQLite/güncel/eski.',
        'Güncel motor üçlülerin ortasındadır; bütün altı sıra permütasyonu ölçülmedi.',
        '500 belge/transaction, 20 synced WAL commit; input CRC/JSON ve40 sınır get',
        'yazma içindedir. FDB son bakım flush/merge drain, SQLite FTS optimize+WAL',
        'checkpoint yapar; aynı algoritma değildir. Worker yazma ile örtüşebilir.',
        'Toplam medyanı süreç başına yazma+bakımdan hesaplanır.', '',
        '**Owned tablosunda SQLite daima tek belgeyle akışlı beslenir.** FDB caller',
        'Vec maliyetini içerir; bu simetrik input RAM karşılaştırması değildir.', '',
        '| API / medyan | Eski FDB | Güncel FDB | SQLite streaming | Güncel/eski fark | Düşük çift |',
        '|---|---:|---:|---:|---:|---:|']
    for api, metrics in writes['apis'].items():
        for name, label, factor, unit in [
            ('write_ms','yazma',1000,'s'), ('maintenance_ms','son bakım',1000,'s'),
            ('total_ms','yazma+bakım',1000,'s'), ('cpu_ms','proses CPU',1000,'s'),
            ('peak_private_bytes','OS peak private',1048576,'MiB'),
            ('peak_working_set_bytes','OS peak working set',1048576,'MiB'),
            ('disk_bytes','retained core disk',1000000,'MB')]:
            row = metrics[name]
            cells = [f'{row["medians"][e]/factor:.3f} {unit}' for e in ['control','candidate','sqlite']]
            lines.append(f'| {api} {label} | '+ ' | '.join(cells) + f' | {row["candidate_change_percent"]:+.2f}% | {row["candidate_lower_pairs"]}/2 |')
    lines += ['', 'OS lifetime private commit fiziksel veya toplam sistem RAM’i değildir;',
              'working set paylaşılan image sayfalarını içerir. Bunlar yazma süreçleridir;',
              'query/get veya oracle RAM’i değildir. Faz örneklemesi kısa tepeleri kaçırabilir.', '',
              f'Bu oturumda akışlı toplam güncel FDB/SQLite oranı {streamed["total_ms"]["medians"]["candidate"]/streamed["total_ms"]["medians"]["sqlite"]:.4f}',
              f'({100*(streamed["total_ms"]["medians"]["candidate"]/streamed["total_ms"]["medians"]["sqlite"]-1):+.2f}%); owned/SQLite-streaming',
              f'oranı {owned["total_ms"]["medians"]["candidate"]/owned["total_ms"]["medians"]["sqlite"]:.4f}',
              f'({100*(owned["total_ms"]["medians"]["candidate"]/owned["total_ms"]["medians"]["sqlite"]-1):+.2f}%). Yalnız bu ortak20×500 belge iş yükü içindir.', '',
              '## Okuma ve get', '',
              'Aynı son sabit DB yeniden kullanıldı; sıfırlama/kopyalama yok. İki süreç çifti,',
              'iki blok,25 tur: **38 durumun her birinde100 örnek/motor**. Sıralama ortak',
              'seed ile aynıdır;8 sayfa da her tur çalışır.256 get anahtarı ve3 eksik anahtar',
              'her tur araya girer. CPU maskesi4; strict açılış, bağımsız corpus oracle ve',
              'her-case/get ısınması süre dışında. İki FDB oracle worker resident kalır,',
              'blok başına DB bırakılır; SQLite blok başına yeni süreçtir. Sorgular ardışıktır.',
              'Süre count+tam sahipli payload sonucu içerir; CLI JSON/IPC değildir.', '',
              f'Güncel/eski pooled p95 **{reads["lower_p95_cases"]["baseline"]}/38 düşük**,',
              f'güncel/SQLite **{reads["lower_p95_cases"]["sqlite"]}/38 düşük**.',
              'Güncel/eski için 8 yüksek, 1 eşit p95 vardır. Her durum aşağıda gösterilir.',
              'Pooled p95 ve blok medyanı ayrı dağılımlardır; tek özet bütün sorguları temsil etmez.', '',
              '| Sorgu / offset / limit | Eski p50/p95 ms | Güncel p50/p95 ms | SQLite p50/p95 ms | Güncel/eski p95 | Düşük çift | Güncel/SQLite p95 | Düşük çift |',
              '|---|---:|---:|---:|---:|---:|---:|---:|']
    regressions = sorted(reads['queries'], key=lambda q:q['current_vs_baseline']['p95_change_percent'], reverse=True)
    for query in reads['queries']:
        cells = [f'{query[e]["p50_ms"]:.4f}/{query[e]["p95_ms"]:.4f}' for e in ['baseline','current','sqlite']]
        delta = query['current_vs_baseline']
        text = query['text'] if len(query['text']) < 35 else 'uzun zx'
        sql = query['current_vs_sqlite']
        lines.append(f'| {text} /{query["offset"]}/{query["limit"]} | '+ ' | '.join(cells) + f' | {delta["p95_change_percent"]:+.2f}% | {delta["faster_pairs"]}/2 | {sql["p95_change_percent"]:+.2f}% | {sql["faster_pairs"]}/2 |')
    lines += ['', '### Güncel FDB’nin kendi kontrolüne göre açık kalan artışlar', '',
              '| Sorgu / offset / limit | Pooled p95 farkı | Çift 1 / çift 2 p95 farkı | Düşük çift | Tipik blok farkı |',
              '|---|---:|---:|---:|---:|']
    for query in regressions:
        delta = query['current_vs_baseline']
        if delta['p95_change_percent'] < 0:
            continue
        text = query['text'] if len(query['text']) < 35 else 'uzun zx'
        paired = ' / '.join(f'{v:+.2f}%' for v in delta['paired_p95_changes_percent'])
        lines.append(f'| {text} /{query["offset"]}/{query["limit"]} | {delta["p95_change_percent"]:+.2f}% | {paired} | {delta["faster_pairs"]}/2 | {delta["median_block_change_percent"]:+.2f}% |')
    lines += ['', 'Uzun zx pooled p95 tam eşittir; tipik blok artışı açıkça korunur.',
              'Buradaki farklar gözlemdir; bu iki çift kod kaynaklı regresyonu veya anlamlılık eşiğini kanıtlamaz.',
              'Ek özel ölçüm yapılmadı. quixote /28/10, man ve zzxqnever iki çiftte de yüksek p95 verdi.',
              'Eski dar raporlardaki farklı kontrol/oturum p95 ve CLI sorunları bu kıyasla kapanmaz.', '',
              '| Get gövde boyutu | Örnek/motor | Eski p50/p95 ms | Güncel p50/p95 ms | SQLite p50/p95 ms |',
              '|---|---:|---:|---:|---:|']
    for query, label in zip(reads['gets'], ['<4KiB','4–16KiB','16–32KiB','>=32KiB','eksik key']):
        assert len({query[e]['n'] for e in ['baseline','current','sqlite']}) == 1
        lines.append(f'| {label} | {query["current"]["n"]} | '+ ' | '.join(f'{query[e]["p50_ms"]:.4f}/{query[e]["p95_ms"]:.4f}' for e in ['baseline','current','sqlite'])+' |')
    lines += ['', 'Get p95 güncel/eski dört dolu boyut grubunda yüksek, eksik key’de eşittir;',
              'güncel/SQLite beş grupta da düşüktür. Get artışları da kapanmış sayılmaz.', '',
              '## Sözleşmeler, doğruluk ve sınırlar', '',
              'Mevcut `data/rkyv-corpus-20261009/json.payload-bench` ilk10.000 belge,',
              '264.842.858 özgün metin byte’ı. Yeni corpus/generator/download yok;',
              'yalnız sahipliği doğrulanan `data/benchmark-work/{fdb,sqlite}` yazma için',
              '`benchmark-workspace.ps1` ile sıfırlandı. Her12 yazma sürecinde tam10k',
              'payload doğrulaması; FDB’de ayrıca4 contains sayfa oracle’ı geçti.',
              'Aynı API eski/güncel source/index/ready byte’ları eşit. Okuma bütün38 case',
              'total/tam payload/sıra/version ve get oracle’larını geçti. Okuma öncesi/sonrası',
              'DB dosya SHA’ları aynı; corpus ve0.1 kaynak fixture SHA’ları korundu.', '',
              'FDB strict open kaynak/index CRC/schema/key/version doğrular; SQLite açılışı',
              'tam integrity taraması yapmaz. Native SQLite3.53.2 bundled FTS5 trigram+LIKE',
              'alt-dize/AND; count ve tam payload tek snapshot transaction içindedir.',
              'FULL WAL,2MiB page cache,mmap0,tempFILE,64 prepared-statement cache;',
              'FDB’nin genel field/link/guard kapsamının tamamını sunmaz. SQLite yalnız',
              'bağımsız benchmark paketidir; motor bağımlılığı/entegrasyonu değildir.', '',
              'OS cache sıcak, eviction/güç/donanım izolasyonu yok. Query/get OS RAM,',
              'açılış karşılaştırması, gerçek soğuk cache, eşzamanlı yük, başka corpus ve',
              'dinamik geçici disk tepesi ölçülmedi. Genel RAM,owned caller maliyeti,',
              'ayrı bakım,arama/CLI regresyonları,açılış,disk sıkıştırma/geçici alan ve',
              'eşzamanlı/soğuk yük işleri [açık listede](OPEN-WORK.md) korunur.', '',
              '## Tekrar üretim ve kanıtlar', '',
              '[Tam commit/binary/source bağlantısı](benchmarks/consolidated-10k-20261024-identity.json),',
              '[yazma tasarım/medyan/çift farkları](benchmarks/consolidated-write-10k-20261024-summary.json),',
              '[12 yazma sürecinin kayıtları](benchmarks/consolidated-write-10k-20261024-records.json),',
              '[okuma tasarım/100 örnek özetleri](benchmarks/consolidated-read-10k-20261024-summary.json),',
              '[tüm 38 durumun CSV’si](benchmarks/consolidated-read-10k-20261024-table.csv),',
              '[bağımsız rapor aritmetiği/oracle/binary kontrolü](benchmarks/consolidated-report-proof-20261024.json).',
              'Aynı prefix altındaki `.jsonl`, `.memory.json`, `-files.json`,',
              '`-verify.jsonl`, korunan önce/sonra SHA dosyaları ham kanıttır.', '',
              'İlk rapor özeti SQLite logunu Windows cp1252 ile okuduğu için iki Unicode',
              'sorgu adında kimlik kontrolünde durdu. Dört blokta toplam 8 text alanı',
              'etkilendi. UTF-8 ham JSONL’de sorgu adları doğrudur; gerçek sorgular,',
              'oracle ve süreler değişmedi. Özet yalnız kayıtlardan yeniden üretildi;',
              'benchmark tekrar çalıştırılmadı. Her değişen adın ters cp1252→UTF-8',
              'dönüşümü doğrulandı; diğer bütün alanlar ve timing dizileri derin eşitlik',
              'kontrolünden geçti. Özgün records korunur; düzeltilen türetilmiş kayıt ayrıdır.',
              '[8 alanlık kurtarma denetimi ve SHA’lar](benchmarks/consolidated-read-10k-20261024-recovery-audit.json),',
              '[özgün kayıt](benchmarks/consolidated-read-10k-20261024-records.json),',
              '[UTF-8 ham kaynağa eşlenen türetilmiş kayıt](benchmarks/consolidated-read-10k-20261024-recovered-records.json).', '',
              'Raporu salt okunur kayıtlardan üretmek için `tools/measure-consolidated-reads.py',
              '--summarize-only`, ardından `tools/summarize-consolidated.py` çalıştırılır.',
              'Bu yol DB/corpus yazmaz; mevcut kaynak, corpus ve 0.1 fixture SHA’larını da',
              'önce/sonra manifestlerine karşı yeniden kontrol eder.', '',
              'Normal güncel write ve interleaved-read yeniden derlemeleri frozen SHA ile',
              'birebir eşleşti. Ortak read_matrix/oracle/worker kaynakları8a92384→2a9fc2c',
              'arasında değişmedi. Yalnız SQLite comparator’ına optional tur/sayfa seçimi',
              'eklendi; eski20/5 varsayılanı korunur. Motor/format/feature/WAL değişmedi.',
              'Mevcut dört SQLite bloğu optional 25/all-rounds yolunu çalıştırdı ve tam',
              'payload oracle’ını geçti. Root ve native comparator için fmt ve tüm-target',
              'Clippy warnings-as-errors kontrolleri geçti; depolama kodu değişmedi.',
              '[Harness doğrulama çıktısı](benchmarks/consolidated-checks-20261024.log).', '']
    (ROOT/'docs/CONSOLIDATED-COMPARISON-10K-20261024.md').write_text('\n'.join(lines),encoding='utf-8')
    with (OUT/'consolidated-read-10k-20261024-table.csv').open('w',newline='',encoding='utf-8-sig') as stream:
        writer=csv.writer(stream)
        writer.writerow(['text','offset','limit','total']+[f'{e}_{m}' for e in ['baseline','current','sqlite'] for m in ['n','p50_ms','p95_ms']]+['current_vs_baseline_p95_percent','current_vs_sqlite_p95_percent'])
        for q in reads['queries']:
            writer.writerow([q['text'],q['offset'],q['limit'],q['total']]+[q[e][m] for e in ['baseline','current','sqlite'] for m in ['n','p50_ms','p95_ms']]+[q['current_vs_baseline']['p95_change_percent'],q['current_vs_sqlite']['p95_change_percent']])


if __name__ == '__main__':
    main()
