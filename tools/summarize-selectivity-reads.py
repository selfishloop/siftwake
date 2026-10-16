"""Reports only: paired full-matrix comparisons and separate diagnostic spans."""
import json
import math
from pathlib import Path
from statistics import median

root = Path(__file__).resolve().parent.parent
prefix = root / 'docs/benchmarks/selectivity-reads-10k-20261012'


def load(path):
    return [json.loads(line) for line in path.read_text(encoding='utf-8-sig').splitlines()]


def aggregate(timings):
    samples = sorted(x for t in timings for x in t['samples'])
    return {'n': len(samples), **{f'p{p}_ms': samples[math.ceil(len(samples)*p/100)-1] for p in (50, 95, 99)},
            'run_p95': [t['p95_ms'] for t in timings]}


runs = {engine: [{row['phase']: row for row in load(Path(f'{prefix}-{engine}-{n}.jsonl'))}
                 for n in range(3)] for engine in ('control', 'fdb', 'sqlite')}
result = {'cache': 'warm OS cache; no eviction', 'runs_per_engine': 3, 'queries': [], 'gets': {}, 'profile': []}
lines = ['| Sorgu | Offset / limit | Sonuç | Eski p95 ms | Yeni p95 ms | SQLite p95 ms | Değişim % |',
         '|---|---:|---:|---:|---:|---:|---:|']
for n, case in enumerate(runs['fdb'][0]['matrix_reads']['queries']):
    fields = ('text', 'kind', 'offset', 'limit', 'total', 'late_matches')
    entry = {k: case[k] for k in fields}
    for engine in runs:
        rows = [run['matrix_reads']['queries'][n] for run in runs[engine]]
        assert all(all(row[k] == entry[k] for k in fields) for row in rows)
        entry[engine] = aggregate([row['timing'] for row in rows])
    entry['change_percent'] = 100*(entry['fdb']['p95_ms']/entry['control']['p95_ms']-1)
    entry['paired_change_percent'] = [100*(new/old-1) for new, old in zip(entry['fdb']['run_p95'], entry['control']['run_p95'])]
    result['queries'].append(entry)
    text = '300 bayt negatif terim' if len(entry['text']) > 100 else entry['text']
    lines.append(f"| {text} | {entry['offset']} / {entry['limit']} | {entry['total']} | {entry['control']['p95_ms']:.4f} | {entry['fdb']['p95_ms']:.4f} | {entry['sqlite']['p95_ms']:.4f} | {entry['change_percent']:+.2f} |")
for engine in runs:
    result['gets'][engine] = [aggregate([run['matrix_reads']['gets'][bin]['timing'] for run in runs[engine]]) for bin in range(5)]
    result.setdefault('open_ms', {})[engine] = [run['opened']['open_ms'] for run in runs[engine]]
profiles = {name: [row for row in load(root / f'docs/benchmarks/selectivity-profile-{name}.jsonl') if row['phase'] == 'selectivity_profile'] for name in ('before', 'after')}
for text in dict.fromkeys(row['text'] for row in profiles['before']):
    entry = {'text': text}
    for name, rows in profiles.items():
        selected = [row for row in rows if row['text'] == text]
        counters = selected[0]['counters']
        entry[name] = {k: {'calls': counters[k]['calls'], 'median_ms': median(row['counters'][k]['elapsed_ns'] for row in selected)/1e6} for k in counters}
    result['profile'].append(entry)
Path(f'{prefix}-summary.json').write_text(json.dumps(result, indent=2, ensure_ascii=False), encoding='utf-8')
Path(f'{prefix}-table.md').write_text('\n'.join(lines)+'\n', encoding='utf-8')
print(json.dumps({'queries': len(result['queries']), 'p95_faster': sum(q['change_percent'] < 0 for q in result['queries']), 'open_ms': result['open_ms']}, indent=2))
