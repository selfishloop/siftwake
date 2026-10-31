"""Render isolated-process RAM evidence; never subtract benchmark RAM."""
import json
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'hybrid-serial-ram50-20261030'
MIB = 1024 * 1024


def main():
    d = json.loads((L / f'{P}-design.json').read_text(encoding='utf-8'))
    assert d['stage'] == 'completed' and len(d['completed']) == 16
    rows = []
    for x in d['completed']:
        q = x['measurement']
        assert q['documents'] == 50000
        m = q['memory']
        row = dict(arm=x['arm'], role=x['role'], mode=x['mode'],
                   private=m['private_bytes'] / MIB,
                   peak_private=m['peak_private_commit_bytes'] / MIB,
                   working_set=m['working_set_bytes'] / MIB,
                   peak_working_set=m['peak_working_set_bytes'] / MIB,
                   query_counts=q['query_counts'],
                   writer_mutations=q['writer_mutations'])
        if x['mode'] == 'ingest':
            row['gate'] = row['peak_private'] <= 60
        elif x['mode'] in ['idle', 'postloaded_idle']:
            row['gate'] = row['private'] <= 15
        else:
            assert len(row['query_counts']) == 4 and min(row['query_counts']) > 0
            assert row['writer_mutations'] == 16384
            row['gate'] = None
        rows.append(row)
    comparisons = []
    for before_arm, after_arm in [(0, 1), (3, 2)]:
        a = next(x for x in rows if x['arm'] == before_arm and x['mode'] == 'loaded')
        b = next(x for x in rows if x['arm'] == after_arm and x['mode'] == 'loaded')
        comparisons.append(dict(before_arm=before_arm, after_arm=after_arm,
                                before=a['peak_private'], after=b['peak_private'],
                                difference=b['peak_private'] - a['peak_private']))
    summary = dict(rows=rows, loaded_comparisons=comparisons,
                   ingest_idle_gate=all(x['gate'] for x in rows if x['gate'] is not None))
    (L / f'{P}-summary.json').write_text(json.dumps(summary, indent=2), encoding='utf-8')
    lines = [
        '# Hibrit seri devam: motor belleği — 30 Ekim 2026', '',
        'Aynı normal ikili, before/after/after/before sırası. Her ingest için yalnız '
        'sabit FDB owner slotu sıfırlandı; kabul ve CPU koşuları bundan önce tamamlandı. '
        'Aynı 881756DA… kaynağının ilk 50.000 belgesi, 100 × 500 akışlı commit. '
        'SQLite slotu ve corpus değişmedi.', '',
        'Ölçülen sınır Windows motor **prosesidir**: runtime, en fazla bir giriş belgesi, '
        '8 KiB stdin buffer ve API sonucu dahildir. Corpus/katalog, model, oracle, history '
        've profil ayrı ebeveyn prosestedir. Bellekten tahmini pay çıkarılmadı. '
        'Loaded çocukta dört okuyucu ve bir yazıcı aynı prosestedir; 41 küçük sorgu tanımı '
        've aynı anda yaşayan API sonuçları da dahildir. Okuyucular hibrit N=50/bütçe 200; '
        'yazıcı 16.384 tekli değişiklik yapar. Oracle ölçüm çocuğunda çalışmaz: '
        'her kol sonrasında bütün 50k GET ve 41 sorgu bağımsız doğrulanır.', '',
        'Private = OS PrivateUsage; peak private = OS PeakPagefileUsage (private commit '
        'yüksek su seviyesi). Working set ayrıca gösterilir. 60 MiB kapısı ingest tepesine, '
        '15 MiB kapısı temiz boşta private değerine uygulanır. Loaded için yeni mutlak '
        'eşik eklenmedi.', '',
        '|Kol|Rol|Faz|Private MiB|Peak private MiB|WS MiB|Peak WS MiB|Kapı|',
        '|---|---|---|---:|---:|---:|---:|---|',
    ]
    for x in rows:
        gate = 'bilgi' if x['gate'] is None else ('GEÇTİ' if x['gate'] else 'KALDI')
        lines.append(f"|{x['arm']}|{x['role']}|{x['mode']}|{x['private']:.3f}|"
                     f"{x['peak_private']:.3f}|{x['working_set']:.3f}|"
                     f"{x['peak_working_set']:.3f}|{gate}|")
    lines += ['', '## Aynı proses 4R + yazıcı', '',
              '|Dengeli eş|Before peak private MiB|After|Fark MiB|',
              '|---|---:|---:|---:|']
    for x in comparisons:
        lines.append(f"|{x['before_arm']} / {x['after_arm']}|{x['before']:.3f}|"
                     f"{x['after']:.3f}|{x['difference']:+.3f}|")
    lines += ['',
              'Bu fark bütün 4R+yazıcı prosesinin yüksek su seviyesidir; yalnız seri '
              'fallback nesnelerine ait tahsis farkı değildir. Önce/sonra okuyucu ilerlemesi '
              've yaşayan snapshot sayısı farklı olabilir. Kabul koşusunun katalog/model/'
              'history içeren bellek rakamları D kapısı olarak kullanılmadı.', '',
              '60/15 MiB ingest/temiz-idle kapıları GEÇTİ. Buna karşılık her iki '
              '4R+yazıcı eşinde peak private yükseldi: RAM kötüleşmeme koşulu KALDI. '
              'Loaded satırlarının bilgi etiketi yalnız yeni bir mutlak loaded '
              'eşik eklenmediğini belirtir; göreli artışı geçer saymaz.', '',
              '|Kol/rol|Her okuyucunun sorgu adedi|Yazma adedi|',
              '|---|---|---:|']
    for x in rows:
        if x['mode'] == 'loaded':
            lines.append(f"|{x['arm']}/{x['role']}|{x['query_counts']}|{x['writer_mutations']}|")
    lines += ['',
              'İlk reset öncesi ve son kol sonrası ayrı process-kill/reopen, canlı kaynak '
              'hash ve eski okuyucu kontrolleri çalıştırıldı. Native RAM ölçümü sorgu '
              'gecikmesi kabul koşusunun yerine geçmez; kontrolün eski bekleme politikası '
              've adayın tek try/seri politikası thread-bound scope ile seçildi.', '',
              f'[Ham RAM ve SHA/kol dizisi](benchmarks/{P}-design.json) · '
              f'[Makine okunur tablo](benchmarks/{P}-summary.json) · '
              f'[Süreç olayları](benchmarks/{P}-lifecycle.jsonl).', '']
    (R / 'docs/HYBRID-SERIAL-RAM-50K-20261030.md').write_text('\n'.join(lines), encoding='utf-8')
    print(json.dumps(summary))


if __name__ == '__main__':
    main()
