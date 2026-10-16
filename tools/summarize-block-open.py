"""Aggregate existing VFY01 benchmark logs only; never touch databases."""
import json
import math
from pathlib import Path
from statistics import median

root = Path(__file__).resolve().parent.parent
prefix = root / 'docs/benchmarks/block-open-10k-20261012'


def read(suffix):
    return {d['phase']: d for d in map(json.loads, Path(f'{prefix}-{suffix}.jsonl').read_text(encoding='utf-8-sig').splitlines())}


def aggregate(timings):
    samples = sorted(v for t in timings for v in t['samples'])
    return {'n': len(samples), **{f'p{p}_ms': samples[math.ceil(len(samples) * p/100)-1] for p in (50, 95, 99)},
            'run_p95': [t['p95_ms'] for t in timings]}


opens = {e: [read(f'{e}-{n}')['opened']['open_ms'] for n in range(5)] for e in ('control', 'fdb')}
result = {'cache': 'warm OS cache, no eviction', 'open_ms': opens,
          'median_open_ms': {e: median(v) for e, v in opens.items()},
          'paired_open_change_percent': [100*(new/old-1) for old, new in zip(opens['control'], opens['fdb'])],
          'first_use': {}, 'warm_reads': {}}
for e in opens:
    probes = [read(f'first-use-{e}-{n}') for n in range(3)]
    result['first_use'][e] = {k: median(p['deferred_first_use'][k] for p in probes)
                            for k in ('first_get_ms', 'first_search_ms', 'warm_search_ms')}
    result['first_use'][e]['open_plus_get_ms'] = median(p['opened']['open_ms'] + p['deferred_first_use']['first_get_ms'] for p in probes)
    result['first_use'][e]['open_plus_get_plus_search_ms'] = median(p['opened']['open_ms'] + p['deferred_first_use']['first_get_ms'] + p['deferred_first_use']['first_search_ms'] for p in probes)
    rows = [read(f'reads-{e}-{n}')['reads'] for n in range(3)]
    assert all(r['full_pages_and_versions_verified'] for r in rows)
    queries = []
    for n, query in enumerate(rows[0]['queries']):
        identity = {k: query[k] for k in ('text', 'offset', 'limit', 'total')}
        assert all(all(row['queries'][n][k] == v for k,v in identity.items()) for row in rows)
        queries.append({**identity, 'timing': aggregate([r['queries'][n]['timing'] for r in rows])})
    result['warm_reads'][e] = {'get': aggregate([r['get'] for r in rows]), 'queries': queries}
result['preparation'] = read('preparation')['open_checks_prepared']
result['full_integrity'] = read('integrity')['integrity_checked']
vfy = json.loads(Path(f'{prefix}-block-caches.json').read_text(encoding='utf-8-sig'))
result['vfy_bytes'] = sum(f['bytes'] for f in vfy)
result['old_reader_open_ms'] = read('old-reader-compatibility')['opened']['open_ms']
Path(f'{prefix}-summary.json').write_text(json.dumps(result, indent=2, ensure_ascii=False), encoding='utf-8')
print(json.dumps(result, indent=2, ensure_ascii=False))
