"""Facts-only paired cost tables. Run after all timed children finish."""
import json
import math
import statistics
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'side-cost50-20261031'
ROLES = ['before', 'after', 'sqlite']


def read(path):
    return json.loads(path.read_text(encoding='utf-8'))


def pct(v, p=.95):
    return sorted(v)[math.ceil(p * len(v)) - 1]


def spread(v):
    return dict(min=min(v), median=statistics.median(v), max=max(v))


def delta(a, b):
    return 100 * (b / a - 1) if a else None


def number(x, digits=3):
    return '—' if x is None else f'{x:.{digits}f}'


def records(run):
    for line in (L / run['log']).read_text(encoding='utf-8').splitlines():
        x = json.loads(line)
        if x['phase'] not in ['snapshot_idle_single', 'snapshot_idle_four', 'snapshot_load', 'snapshot_idle_probe']:
            continue
        for t in x['results']:
            for q in t['records']:
                assert q['oracle']
                if x['phase'] != 'snapshot_load' or q['writer_active']:
                    yield x['phase'], q


def cpu_stat(v):
    assert len(v) >= 60
    cpus = [x['cpu_ms'] for x in v]
    return dict(n=len(v), cpu_sum=sum(cpus), cpu_mean=statistics.mean(cpus), cpu_p95=pct(cpus),
                parent_mean=statistics.mean(x['parent_ms'] for x in v),
                child_mean=statistics.mean(x['child_ms'] for x in v),
                wall_p95=pct([x['wall_ms'] for x in v]), samples=v)


def wall_stat(v):
    assert len(v) >= 60
    return dict(n=len(v), p50=pct(v, .5), p95=pct(v), max=max(v), samples=v)


def main():
    est = read(L / 'estimator50-20261031-summary.json')
    design = read(L / 'estimator50-20261031-design.json')
    cpu = read(L / 'side-cost-cpu50-20261031-design.json')
    broad = read(L / 'side-cost-broad50-20261031-design.json')
    ram = read(L / 'side-cost-ram50-20261031-design.json')
    assert all(x['stage'] == 'completed' for x in [est, design, cpu, broad, ram])
    cases = [x['text'] for x in read(L / 'trwiki-cap200k-20261029-v3-catalog.json')['cases']] + ['the zx']
    writes, values, times = [], {}, {}
    for run in design['completed']:
        phases = [json.loads(x) for x in (L / run['log']).read_text(encoding='utf-8').splitlines()]
        x = next(x for x in phases if x['phase'] == 'snapshot_load')
        v = [w['commit_ms'] for w in x['writes']]
        role, pair = run['role'], run['pair']
        values.setdefault((role, pair == -1), []).extend(v)
        times.setdefault((role, pair == -1), []).append((x['mutations'], x['writer_elapsed_ms']))
        writes.append(dict(role=role, pair=pair, n=len(v), ops_s=x['mutations'] * 1000 / x['writer_elapsed_ms'],
                           p95=pct(v), p99=pct(v, .99), p999=pct(v, .999), max=max(v), over100=sum(t > 100 for t in v)))
    controls = {x['role']: x['ops_s'] for x in writes if x['pair'] == -1}
    pooled = []
    for role in ROLES:
        for control in [False, True]:
            v, t = values[role, control], times[role, control]
            ops = sum(x[0] for x in t) * 1000 / sum(x[1] for x in t)
            pooled.append(dict(role=role, control=control, ops_s=ops, retention=ops / controls[role],
                               n=len(v), p95=pct(v), p99=pct(v, .99), p999=pct(v, .999), max=max(v)))
    for w in writes:
        sql = next(x for x in writes if x['role'] == 'sqlite' and x['pair'] == w['pair'])
        w.update(retention=w['ops_s'] / controls[w['role']], sqlite_ops_ratio=w['ops_s'] / sql['ops_s'],
                 sqlite_tail_ratios={k: w[k] / sql[k] for k in ['p95', 'p99', 'p999', 'max']})
    write_spread = {role: {k: spread([x[k] for x in writes if x['role'] == role and x['pair'] >= 0])
                           for k in ['ops_s', 'retention', 'p95', 'p99', 'p999', 'max']} for role in ROLES}
    write_deltas = [delta(next(x for x in writes if x['pair'] == p and x['role'] == 'before')['ops_s'],
                          next(x for x in writes if x['pair'] == p and x['role'] == 'after')['ops_s']) for p in range(4)]
    cpubins, wallbins = {}, {}
    for d, source in [(cpu, 'full'), (broad, 'six')]:
        for run in d['completed']:
            label = run.get('label', '')
            for phase, q in records(run):
                case = q['case_id']
                if case not in [34, 35, 36, 37, 38, 41]:
                    continue
                if source == 'six':
                    context = f"six-{run['readers']}R"
                elif label.startswith('aux'):
                    context = 'single-the-zx-' + label.split('-')[1]
                else:
                    context = {'snapshot_idle_single':'full41-1R', 'snapshot_idle_four':'full41-4R', 'snapshot_load':'full41-4R+W'}[phase]
                key = (context, run['pair'], run['role'], case)
                profile = q['query_profile']
                if isinstance(profile, dict) and 'threads' in profile:
                    ts = profile['threads']
                    assert ts and len({t['thread_id'] for t in ts}) == len(ts)
                    assert all(t['cpu_100ns'] is not None for t in ts)
                    # Request::finish drops caller scope after all children join.
                    parent = ts[-1]['cpu_100ns'] / 10000
                    child = sum(t['cpu_100ns'] for t in ts[:-1]) / 10000
                    cpubins.setdefault(key, []).append(dict(cpu_ms=parent + child, parent_ms=parent, child_ms=child,
                                                           wall_ms=q['engine_ms'], sequence=q['sequence'],
                                                           thread=q['thread'], round=q['round']))
                else:
                    wallbins.setdefault(key, []).append(q['engine_ms'])
    cpus = [dict(context=k[0], pair=k[1], role=k[2], case_id=k[3], **cpu_stat(v)) for k, v in sorted(cpubins.items())]
    walls = [dict(context=k[0], pair=k[1], role=k[2], case_id=k[3], **wall_stat(v)) for k, v in sorted(wallbins.items())]
    cpu_pooled, wall_pooled = [], []
    for context, case in sorted({(x['context'], x['case_id']) for x in cpus}):
        for role in ['before', 'after']:
            v = [s for x in cpus if x['context'] == context and x['case_id'] == case and x['role'] == role for s in x['samples']]
            cpu_pooled.append(dict(context=context, case_id=case, role=role, **cpu_stat(v)))
    for context, case in sorted({(x['context'], x['case_id']) for x in walls}):
        for role in ['before', 'after']:
            v = [s for x in walls if x['context'] == context and x['case_id'] == case and x['role'] == role for s in x['samples']]
            wall_pooled.append(dict(context=context, case_id=case, role=role, **wall_stat(v)))
    memory = []
    for x in ram['completed']:
        m = x['measurement']['memory']
        memory.append(dict(arm=x['arm'], role=x['role'], mode=x['mode'],
                           private=m['private_bytes'] / 1048576, peak=m['peak_private_commit_bytes'] / 1048576,
                           ws=m['working_set_bytes'] / 1048576, peak_ws=m['peak_working_set_bytes'] / 1048576))
    ram_pairs = []
    for pair, (b, a) in enumerate([(0, 1), (3, 2), (4, 5), (7, 6)]):
        for mode in ['ingest', 'idle', 'loaded', 'postloaded_idle']:
            left = next(x for x in memory if x['arm'] == b and x['mode'] == mode)
            right = next(x for x in memory if x['arm'] == a and x['mode'] == mode)
            field = 'private' if mode in ['idle', 'postloaded_idle'] else 'peak'
            limit = 60 if mode == 'ingest' else 15 if mode in ['idle', 'postloaded_idle'] else None
            ram_pairs.append(dict(pair=pair, mode=mode, before=left[field], after=right[field], difference=right[field]-left[field],
                                  limit=limit, passed=None if limit is None else left[field] <= limit and right[field] <= limit))
    normal_pooled = []
    sample_rows = read(L / 'estimator50-20261031-samples.json')
    for case in range(41):
        for role in ROLES:
            for phase in ['snapshot_idle_single', 'snapshot_idle_four', 'snapshot_load']:
                v = [v for x in sample_rows if x['case_id']==case and x['role']==role and x['phase']==phase for v in x['values']]
                normal_pooled.append(dict(case_id=case, role=role, phase=phase, **wall_stat(v)))
    out = dict(writes=writes, pooled_writes=pooled, write_spread=write_spread, write_pair_deltas=write_deltas,
               cpu=cpus, cpu_pooled=cpu_pooled, broad=walls, broad_pooled=wall_pooled,
               memory=memory, ram_pairs=ram_pairs, normal41=normal_pooled)
    (L / f'{P}-summary.json').write_text(json.dumps(out, ensure_ascii=False), encoding='utf-8')
    lines = ['# Prototip yan maliyetleri — 31 Ekim 2026', '',
             'Varsayılan REFERENCE_WAIT=true. Prototip diagnostics opt-in; 2 ms bekleme uygulanmadı. '
             'Aynı 881756DA… corpus ilk50k. Yeni corpus/DB kopyası yok; yalnız sabit FDB/SQLite '
             'slotları. Yazma/normal41 okuma verisi ESTIMATOR N60 ham koşularıdır; bu tur '
             'yeniden ingest/yazma matrisi diye etiketlenmedi. CPU ve RAM bu tur yeniden '
             'ölçüldü. Normal p95 ile diagnostic CPU ayrı ikili/scope verisidir. '
             'Warm OS cache, eviction yok; fiziksel yerleşim/sequence yazıyla ilerler. '
             'Eşikler aynı. Kullanıcı yan maliyetlerin tümü tamamlanıp birlikte raporlansın dedi.', '',
             '## Yazma: dört dengeli çift', '',
             'Kontrol paydası her rol için bir okuyucusuz16k koldur; dört ayrı kontrol '
             'ölçümü olarak sunulmaz. Loaded süre N60 için uzayabilir; commit adedi tabloda. '
             'Yüzdelikler raw commit örneklerinden hesaplandı. SQLite oranı FDB/SQLite; '
             'throughputta1 üstü, gecikmede1 altı farklı yönleri ifade eder.', '',
             '|Çift/rol|N|Ops/s|Yazmasız kontrole %|p95/p99/p99,9/max ms|Ops SQLite oranı|Tail SQLite oranları|',
             '|---|---:|---:|---:|---|---:|---|']
    for w in writes:
        lines.append(f"|{w['pair']}/{w['role']}|{w['n']}|{w['ops_s']:.3f}|{100*w['retention']:.3f}|" + '/'.join(number(w[k]) for k in ['p95','p99','p999','max']) + f"|{w['sqlite_ops_ratio']:.3f}|" + '/'.join(number(w['sqlite_tail_ratios'][k]) for k in ['p95','p99','p999','max']) + '|')
    lines += ['', '|Rol/ölçüt|Çift min|Medyan|Max|', '|---|---:|---:|---:|']
    for role in ROLES:
        for metric, v in write_spread[role].items():
            factor = 100 if metric == 'retention' else 1
            lines.append(f'|{role}/{metric}|' + '|'.join(number(v[k]*factor) for k in ['min','median','max']) + '|')
    lines += ['', '|Çift|After/before throughput Δ%|', '|---|---:|']
    for i, value in enumerate(write_deltas):
        lines.append(f'|{i}|{value:+.4f}|')
    ds = spread(write_deltas)
    lines += ['', '|Paired throughput Δ% min|Medyan|Max|', '|---:|---:|---:|',
              f"|{ds['min']:.4f}|{ds['median']:.4f}|{ds['max']:.4f}|"]
    old = read(L / 'hybrid-serial50-20261030-summary.json')
    lines += ['', '|SQLite serisi|Yazmasız ops/s|Loaded pooled ops/s|Korunum %|', '|---|---:|---:|---:|',
              '|CONCURRENT-DELTA-1|280.49|199.42|71.10|']
    x = next(x for x in old['throughput'] if x['role']=='sqlite')
    y = next(x for x in pooled if x['role']=='sqlite' and not x['control'])
    lines += [f"|HYBRID-SERIAL N12|{x['control']:.3f}|{x['loaded']:.3f}|{x['ratio']*100:.3f}|",
              f"|ESTIMATOR N60|{controls['sqlite']:.3f}|{y['ops_s']:.3f}|{y['retention']*100:.3f}|", '',
              '## Geniş tarama: normal p95 ve diagnostic CPU', '',
              'Altılı mix: kendisine çıkacağını,ve,if,id,zx,the zx. Her pair/rol/1R/4R '
              'için60 örnek. the zx bilgi amaçlı; ana41/kapı dışında. CPU ayrı diagnostic '
              'requestte caller+join edilmiş çocuk thread GetThreadTimes toplamı. '
              'CPU mean=ΣCPU/N; thread saatinin kısa aralıklarda15,625ms nicemlenmesi '
              'vardır. Nested wall sayaçları CPU diye toplanmadı. Diagnostic wall '
              'p95 normal gecikme yerine kullanılmadı. Full41 CPU ayrı bağlamdır; '
              'loaded eski +%10,6 ile aynı request ölçüm türüdür.', '',
              '|Mix/çift|Case|Normal p50/p95/max before→after ms|N B/A|', '|---|---|---|---|']
    for context, pair, case in sorted({(x['context'],x['pair'],x['case_id']) for x in walls}):
        b = next(x for x in walls if (x['context'],x['pair'],x['case_id'],x['role'])==(context,pair,case,'before'))
        a = next(x for x in walls if (x['context'],x['pair'],x['case_id'],x['role'])==(context,pair,case,'after'))
        lines.append(f"|{context}/{pair}|{cases[case]}|" + '/'.join(number(b[k]) for k in ['p50','p95','max'])+' → '+'/'.join(number(a[k]) for k in ['p50','p95','max'])+f"|{b['n']}/{a['n']}|")
    lines += ['', '|Mix/çift|Case|N B/A|CPU mean B→A ms|Δ%|Parent mean B/A|Child mean B/A|CPU p95 B/A|', '|---|---|---|---|---:|---|---|---|']
    for context, pair, case in sorted({(x['context'],x['pair'],x['case_id']) for x in cpus}):
        b = next(x for x in cpus if (x['context'],x['pair'],x['case_id'],x['role'])==(context,pair,case,'before'))
        a = next(x for x in cpus if (x['context'],x['pair'],x['case_id'],x['role'])==(context,pair,case,'after'))
        lines.append(f"|{context}/{pair}|{cases[case]}|{b['n']}/{a['n']}|{b['cpu_mean']:.6f}→{a['cpu_mean']:.6f}|{number(delta(b['cpu_mean'],a['cpu_mean']))}|{b['parent_mean']:.6f}/{a['parent_mean']:.6f}|{b['child_mean']:.6f}/{a['child_mean']:.6f}|{b['cpu_p95']:.3f}/{a['cpu_p95']:.3f}|")
    historic_cpu = read(L / 'hybrid-serial-extra50-20261030-summary.json')
    old_b = next(x for x in historic_cpu['aux'] if x['readers']=='4R' and x['case_id']==38 and x['role']=='before')['p95']
    old_a = next(x for x in historic_cpu['aux'] if x['readers']=='4R' and x['case_id']==38 and x['role']=='after')['p95']
    new_b = next(x for x in wall_pooled if x['context']=='six-4R' and x['case_id']==38 and x['role']=='before')['p95']
    new_a = next(x for x in wall_pooled if x['context']=='six-4R' and x['case_id']==38 and x['role']=='after')['p95']
    old_cb = next(x for x in historic_cpu['loaded'] if x['phase']=='snapshot_load' and x['case_id']==38 and x['role']=='before')['cpu_mean']
    old_ca = next(x for x in historic_cpu['loaded'] if x['phase']=='snapshot_load' and x['case_id']==38 and x['role']=='after')['cpu_mean']
    new_cb = next(x for x in cpu_pooled if x['context']=='full41-4R+W' and x['case_id']==38 and x['role']=='before')['cpu_mean']
    new_ca = next(x for x in cpu_pooled if x['context']=='full41-4R+W' and x['case_id']==38 and x['role']=='after')['cpu_mean']
    lines += ['', '|zx ölçütü|30 Ekim B→A|Δ%|Yeni N60 B→A|Δ%|', '|---|---|---:|---|---:|',
              f'|Altılı4R normal p95 ms|{old_b:.3f}→{old_a:.3f}|{delta(old_b,old_a):+.4f}|{new_b:.3f}→{new_a:.3f}|{delta(new_b,new_a):+.4f}|',
              f'|Full41 loaded diagnostic CPU ms/query|{old_cb:.3f}→{old_ca:.3f}|{delta(old_cb,old_ca):+.4f}|{new_cb:.3f}→{new_ca:.3f}|{delta(new_cb,new_ca):+.4f}|', '',
              'Eski/new koşular aynı fiziksel yerleşim veya oturum değildir. CPU '
              'normal p95 yerine konmadı; altılı/read-only,tek-sorgu ve41+writer '
              'bağlamları birbirine katılmadı. SQLite sorgu CPU ölçümü yok; SQLite '
              'normal41 ve yazma referansı ESTIMATOR hamlarında verilmiştir.']
    lines += ['', '## RAM: dört eşleşmiş çift', '',
              'OS motor prosesi: runtime+tek giriş belgesi+8KiB stdin buffer; loaded '
              'modda41 küçük query ve API sonuçları dahil. Model/oracle/history/katalog/'
              'profil ayrı ebeveyn proseste; tahmini pay çıkarılmadı. Ingestte100×500 '
              'streamed commit ve bakım; loaded modda4R+16.384 tekli writer,8192de '
              'maintain_idle. Her ingest için yalnız sabit FDB owner reset; bütün '
              'arama/CPU ölçümleri önceden bitmişti. Oracle çocuk dışındaki50k GET/41 '
              'query kontrolüdür. Peak private=OS PeakPagefileUsage, WS ayrıca rawda. '
              'Pair0=arm0/1,pair1=3/2,pair2=4/5,pair3=7/6.', '',
              '|Çift/faz|Before MiB|After MiB|Δ MiB|Sınır MiB|Kapı|', '|---|---:|---:|---:|---:|---|']
    for x in ram_pairs:
        lines.append(f"|{x['pair']}/{x['mode']}|{x['before']:.3f}|{x['after']:.3f}|{x['difference']:+.3f}|{number(x['limit'])}|{'—' if x['passed'] is None else 'GEÇTİ' if x['passed'] else 'KALDI'}|")
    old_ram = read(L / 'hybrid-serial-ram50-20261030-summary.json')
    lines += ['', '|Loaded peak private serisi|Çift|Before MiB|After MiB|Δ MiB|', '|---|---|---:|---:|---:|']
    for i,x in enumerate(old_ram['loaded_comparisons']):
        lines.append(f"|30 Ekim|{i}|{x['before']:.3f}|{x['after']:.3f}|{x['difference']:+.3f}|")
    for x in ram_pairs:
        if x['mode']=='loaded':
            lines.append(f"|Yeni|{x['pair']}|{x['before']:.3f}|{x['after']:.3f}|{x['difference']:+.3f}|")
    lines += ['', 'Loaded Δ bütün aynı proses yüksek su seviyesidir; yalnız fallback '
              'nesnelerinin tahsisi diye etiketlenmedi. Bütün kollar ve eşleşmeler '
              'gösterildi; kötü sonuç elenmedi.']
    audit_path = L / f'{P}-final-audit.json'
    if audit_path.exists():
        audit = read(audit_path)
        labels = {'write_retention_75_and_above_sqlite':'Yazma korunum ≥%75 ve SQLite pooled üstü',
                  'single_commit_p95_p99_p999':'Tekli commit p95/p99/p99,9 ≤20/50/100ms; max bilgi',
                  'ram_ingest60_idle15':'D ingest peak≤60 / temiz idle≤15MiB',
                  'ram_loaded_nonincrease':'Eşleşmiş4R+W peak private artmama',
                  'broad_pooled_nonincrease':'Altılı normal1R/4R pooled p95 artmama (the zx hariç)',
                  'cpu_pooled_nonincrease':'Altılı1R/4R ve full41 loaded CPU mean artmama (the zx hariç)'}
        lines += ['', '## Mevcut kapı hesabı ve doğrulama', '',
                  '|Mevcut koşul|Sonuç|', '|---|---|']
        for key, value in audit['gates'].items():
            lines.append(f"|{labels[key]}|{'GEÇTİ' if value else 'KALDI'}|")
        lines += ['|Normal41 okuma A, ESTIMATOR|Kontrol39/40/41/41; aday41/41/40/40; SQLite41/40/40/40; KALDI|', '',
                  'Artmama hesapları0 artış sınırını kullanır; yeni tolerans eklenmedi. '
                  'CPU/normal tarama kontrolleri tanımlı bağlamda gerçek örneklerin pooled '
                  'değerleridir; her pair ayrıca yukarıda. Query A kapısı her pairde '
                  'idle≤30ms ise loaded≤30ms, diğerlerinde≤2×idle olarak korunur.', '',
                  '251 default all-targets test (15 ignored),159 diagnostic lib test '
                  '(13 ignored),fmt ve default/diagnostic all-targets clippy -D warnings '
                  'geçti. Corpus/0.1 fixture ve engine kaynak SHA eşit. 40 CPU,32 altılı '
                  'tarama,48 RAM/oracle child normal exit0. Reset öncesi/sonrası3er '
                  'kill,reopen ve frozen eski okuyucu oracle geçti. Ek6 kill strict-open '
                  'tanığı sonrasıdır; commit ortası veya elektrik kesintisi iddiası yok. '
                  'Test paketi ayrı append/build/yayım/bozulma/kesilme/kill senaryolarını içerir.', '',
                  f'[Son gate/hash/test/süreç denetimi](benchmarks/{P}-final-audit.json) · '
                  f'[Default test](benchmarks/{P}-tests.log) · '
                  f'[Diagnostic test](benchmarks/{P}-diag-tests.log) · '
                  f'[Fmt](benchmarks/{P}-fmt.log) · [Clippy](benchmarks/{P}-clippy.log) · '
                  f'[Diagnostic clippy](benchmarks/{P}-diag-clippy.log).']
    lines += ['',
              '[Tek sayfa](SIDE-COST-SUMMARY-50K-20261031.md) · '
              f'[Ham sayı/samples JSON](benchmarks/{P}-summary.json) · '
              '[N60 normal41 raw](ESTIMATOR-50K-20261031.md) · '
              '[CPU SHA/ham loglar](benchmarks/side-cost-cpu50-20261031-design.json) · '
              '[Altılı mix SHA/loglar](benchmarks/side-cost-broad50-20261031-design.json) · '
              '[RAM SHA/loglar](benchmarks/side-cost-ram50-20261031-design.json).']
    (R / 'docs/SIDE-COST-50K-20261031.md').write_text('\n'.join(lines)+'\n', encoding='utf-8')
    summary = ['# Okuma ve yan maliyetler — 31 Ekim 2026', '',
               'B=REFERENCE_WAIT=true; A=diagnostics anında try/seri. Varsayılan B; '
               '2ms yok. Normal okuma/yazma=ESTIMATOR N60 dört çift. CPU/RAM=yeni '
               'dört eşleşmiş çift. Pooled p95 gerçek örneklerden; kapı kararları '
               'pair tablolarındadır, pooled değerler kapının yerine geçmez.', '',
               '|Sorgu|4R idle B→A p95 ms|4R+W B→A p95 ms|Sorgu|4R idle B→A p95 ms|4R+W B→A p95 ms|',
               '|---|---|---|---|---|---|']
    def cell(i):
        if i>=41:return ['','','']
        cols=[cases[i]]
        for phase in ['snapshot_idle_four','snapshot_load']:
            b=next(x for x in normal_pooled if (x['case_id'],x['role'],x['phase'])==(i,'before',phase))
            a=next(x for x in normal_pooled if (x['case_id'],x['role'],x['phase'])==(i,'after',phase))
            cols.append(f"{b['p95']:.2f}→{a['p95']:.2f}")
        return cols
    for i in range(21):summary.append('|'+ '|'.join(cell(i)+cell(i+21))+'|')
    summary += ['', '|Yazma pooled|B|A|SQLite|', '|---|---:|---:|---:|']
    for metric in ['ops_s','retention','p95','p99','p999','max']:
        factor=100 if metric=='retention' else 1
        label={'ops_s':'Ops/s','retention':'Korunum %','p95':'p95 ms','p99':'p99 ms','p999':'p99,9 ms','max':'Max ms'}[metric]
        summary.append('|'+label+'|'+ '|'.join(number(next(x for x in pooled if x['role']==role and not x['control'])[metric]*factor) for role in ROLES)+'|')
    summary += ['', 'Ops/s min/medyan/max: ' + '; '.join(role+' '+ '/'.join(number(write_spread[role]['ops_s'][k]) for k in ['min','median','max']) for role in ROLES)+'.',
                'Paired A/B throughput Δ% min/medyan/max: '+ '/'.join(number(ds[k]) for k in ['min','median','max'])+'. SQLite korunum serisi: 71,10% → 82,739% → '+number(y['retention']*100)+'%.']
    summary += ['', '|Altılı mix CPU ms/query|1R B→A|4R B→A|Full41 loaded CPU B→A|', '|---|---|---|---|']
    for case in [34,35,36,37,38,41]:
        cols=[]
        for context in ['six-1R','six-4R','full41-4R+W']:
            bs=[x for x in cpu_pooled if x['case_id']==case and x['context']==context and x['role']=='before']
            aa=[x for x in cpu_pooled if x['case_id']==case and x['context']==context and x['role']=='after']
            cols.append('—' if not bs else f"{bs[0]['cpu_mean']:.3f}→{aa[0]['cpu_mean']:.3f}")
        summary.append('|'+cases[case]+'|'+'|'.join(cols)+'|')
    summary += ['', '|RAM çift|Ingest peak B→A MiB|Idle private B→A MiB|4R+W peak B→A MiB|', '|---|---|---|---|']
    for pair in range(4):
        summary.append('|'+str(pair)+'|'+ '|'.join(f"{x['before']:.2f}→{x['after']:.2f}" for mode in ['ingest','idle','loaded'] for x in ram_pairs if x['pair']==pair and x['mode']==mode)+'|')
    summary += ['', 'A normal41 geçen sayısı: B39/40/41/41; A41/41/40/40; SQLite41/40/40/40. Motor kaynakları değişmedi. '
                'CPU diagnostic sayaç overheadini içerir; oracle hariç parent+child toplamıdır. '
                'RAM model/oracle/profil hariç motor prosesidir. the zx bilgi amaçlı. '
                '[Dört çift, spread, SQLite oranları, bütün raw ve sınırlar](SIDE-COST-50K-20261031.md).']
    (R / 'docs/SIDE-COST-SUMMARY-50K-20261031.md').write_text('\n'.join(summary)+'\n', encoding='utf-8')
    print(json.dumps(dict(write_pair_deltas=write_deltas, ram_pairs=ram_pairs, cpu_cells=len(cpus))))


if __name__ == '__main__':
    main()
