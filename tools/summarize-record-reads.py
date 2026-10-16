"""Aggregate existing REF01 benchmark logs only; never touch databases."""
import json
import math
import sys
from pathlib import Path
from statistics import median

root = Path(__file__).resolve().parent.parent
prefix = root / (sys.argv[1] if len(sys.argv) > 1 else 'docs/benchmarks/record-reads-10k-20261013')


def read(suffix):
    return {d['phase']: d for d in map(json.loads, Path(f'{prefix}-{suffix}.jsonl').read_text(encoding='utf-8-sig').splitlines())}


def aggregate(timings):
    samples = sorted(v for t in timings for v in t['samples'])
    return {'n': len(samples), **{f'p{p}_ms': samples[math.ceil(len(samples) * p/100)-1] for p in (50, 95, 99)},
            'run_p95': [t['p95_ms'] for t in timings]}


opens = {e: [read(f'first-use-{e}-{n}')['opened']['open_ms'] for n in range(5)] for e in ('control', 'fdb')}
result = {'cache': 'warm OS cache, no eviction', 'open_ms': opens,
          'median_open_ms': {e: median(v) for e, v in opens.items()},
          'paired_open_change_percent': [100*(new/old-1) for old, new in zip(opens['control'], opens['fdb'])],
          'first_use': {}, 'warm_reads': {}}
for e in opens:
    probes = [read(f'first-use-{e}-{n}') for n in range(5)]
    result['first_use'][e] = {k: median(p['deferred_first_use'][k] for p in probes)
                            for k in ('first_get_ms', 'first_search_ms', 'warm_search_ms', 'pending_on_open', 'pending_after_get')}
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
if Path(f'{prefix}-preparation.jsonl').exists():
    result['preparation'] = read('preparation')['record_reads_prepared']
result['full_integrity'] = read('integrity')['integrity_checked']
if Path(f'{prefix}-strict-control-0.jsonl').exists():
    strict = {e: [read(f'strict-{e}-{n}')['opened']['open_ms'] for n in range(5)] for e in opens}
    result['strict_open_ms'] = strict
    result['median_strict_open_ms'] = {e: median(values) for e, values in strict.items()}
    if Path(f'{prefix}-strict-repeat-control-0.jsonl').exists():
        extra = {e: [read(f'strict-repeat-{e}-{n}')['opened']['open_ms'] for n in range(5)] for e in opens}
        combined = {e: strict[e] + extra[e] for e in opens}
        result['strict_repeat'] = {'extra_open_ms': extra,
                                   'extra_median_open_ms': {e: median(v) for e,v in extra.items()},
                                   'combined_10_median_open_ms': {e: median(v) for e,v in combined.items()},
                                   'candidate_faster_pairs_out_of_10': sum(a < b for a,b in zip(combined['fdb'], combined['control']))}
if Path(f'{prefix}-matrix-check.jsonl').exists():
    result['matrix_check'] = read('matrix-check')['matrix_checked']
if Path(f'{prefix}-first-use-control-0.jsonl.memory.json').exists():
    result['first_use_process_memory'] = {}
    for e in opens:
        rows = [json.loads(Path(f'{prefix}-first-use-{e}-{n}.jsonl.memory.json').read_text(encoding='utf-8-sig')) for n in range(5)]
        assert all(row['samples'] > 0 for row in rows)
        result['first_use_process_memory'][e] = {
            'sample_interval_ms': 10, 'samples': [row['samples'] for row in rows],
            'scope': rows[0]['scope'], 'short_lived_peaks_may_be_missed': True,
            **{key: {'runs_bytes': [row[key] for row in rows], 'median_bytes': median(row[key] for row in rows)}
               for key in ('observed_private_peak_bytes', 'observed_os_peak_working_set_bytes')}}
if Path(f'{prefix}-new-caches.json').exists():
    vfy = json.loads(Path(f'{prefix}-new-caches.json').read_text(encoding='utf-8-sig'))
    result['new_cache_bytes'] = sum(f['bytes'] for f in vfy)
if Path(f'{prefix}-reads-control-0.jsonl.memory.json').exists():
    result['warm_process_memory'] = {}
    for e in opens:
        rows = [json.loads(Path(f'{prefix}-reads-{e}-{n}.jsonl.memory.json').read_text(encoding='utf-8-sig')) for n in range(3)]
        assert all(row['samples'] > 0 for row in rows)
        result['warm_process_memory'][e] = {'scope': rows[0]['scope'],
            'samples': [row['samples'] for row in rows],
            'sample_interval_ms': 10, 'short_lived_peaks_may_be_missed': True,
            **{key: {'runs_bytes': [row[key] for row in rows], 'median_bytes': median(row[key] for row in rows)}
               for key in ('observed_private_peak_bytes', 'observed_os_peak_working_set_bytes')}}
if Path(f'{prefix}-old-reader-compatibility.jsonl').exists():
    result['old_reader_open_ms'] = read('old-reader-compatibility')['opened']['open_ms']
Path(f'{prefix}-summary.json').write_text(json.dumps(result, indent=2, ensure_ascii=False), encoding='utf-8')
print(json.dumps({k:result[k] for k in ['median_open_ms','first_use','preparation','full_integrity','new_cache_bytes'] if k in result}, indent=2, ensure_ascii=False))
