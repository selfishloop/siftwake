"""Check nine pages/full documents/counts and summarize three native pairs."""
import json
import statistics
import sys
from pathlib import Path

prefix = sys.argv[1] if len(sys.argv) > 1 else 'docs/benchmarks/long-sqlite-20261011'
expected = {'common:0': 100000, 'common:50': 100000, 'common:99950': 100000,
            'rare:0': 264, 'rare:50': 264, 'short:0': 100000, 'short:50': 100000,
            'and:0': 16537, 'and:50': 16537}
engines = {}
for engine in ['fdb', 'sqlite']:
    runs = []
    for run in range(3):
        path = f'{prefix}-{engine}-{run}'
        rows = [json.loads(line) for line in Path(path+'.jsonl').read_text(encoding='utf-8-sig').splitlines() if line.strip()]
        memory = json.loads(Path(path+'.memory.json').read_text(encoding='utf-8-sig'))
        assert memory['exit_code'] == 0 and memory['engine'] == engine
        opening = next(row for row in rows if row['phase'] == 'open')
        queries = {f"{row['query']}:{row['offset']}": row for row in rows if row['phase'] == 'query'}
        assert set(queries) == set(expected)
        for key, row in queries.items():
            assert row['total'] == expected[key] and row['full_page_oracle_verified'] and not row['profiled']
            assert len(row['samples_ms']) == (10 if key.startswith('rare:') else 1)
            if engine == 'sqlite':
                assert any('VIRTUAL TABLE' in s for s in row['plan'])
        if engine == 'fdb':
            assert opening['stats']['documents'] == 100000 and opening['stats']['segments'] == 1
            assert opening['stats']['resident_documents'] == 0
        runs.append({'opening': opening, 'queries': queries, 'memory': memory})
    engines[engine] = {'runs': runs, 'median': {
        'open_ms': statistics.median(r['opening']['open_ms'] for r in runs),
        'peak_working_set_bytes': statistics.median(r['memory']['peak_working_set_bytes'] for r in runs),
        'sampled_peak_private_bytes': statistics.median(r['memory']['sampled_peak_private_bytes'] for r in runs),
        'process_elapsed_s': statistics.median(r['memory']['elapsed_s'] for r in runs),
        'process_cpu_ms': statistics.median(r['memory']['cpu_ms'] for r in runs),
        'queries_ms': {key: statistics.median(r['queries'][key]['p50_ms'] for r in runs) for key in expected}}}
for run in range(3):
    a, b = [engines[e]['runs'][run] for e in ['fdb', 'sqlite']]
    assert all(a['memory'][k] == b['memory'][k] for k in ['fdb_database', 'sqlite_database'])
    for key in expected:
        assert all(a['queries'][key][k] == b['queries'][key][k] for k in ['text', 'offset', 'total'])
fdb, sqlite = [engines[e]['median']['queries_ms'] for e in ['fdb', 'sqlite']]
print(json.dumps({'engines': engines,
    'fdb_over_sqlite_time_ratio': {key: fdb[key]/sqlite[key] for key in expected},
    'fdb_faster_every_pair': {key: all(engines['fdb']['runs'][run]['queries'][key]['p50_ms'] < engines['sqlite']['runs'][run]['queries'][key]['p50_ms'] for run in range(3)) for key in expected},
    'sqlite_faster_every_pair': {key: all(engines['sqlite']['runs'][run]['queries'][key]['p50_ms'] < engines['fdb']['runs'][run]['queries'][key]['p50_ms'] for run in range(3)) for key in expected},
    'paired_time_ratios': {key: [engines['fdb']['runs'][run]['queries'][key]['p50_ms']/engines['sqlite']['runs'][run]['queries'][key]['p50_ms'] for run in range(3)] for key in expected},
    'scope': 'same 100k long passages, exact total + sorted 50 full StoredDocuments, warm protocol; query-only; native serial paired processes',
    'open_contract_equal': False}, indent=2))
