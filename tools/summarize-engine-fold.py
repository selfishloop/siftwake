"""Validate the actual engine API, pages and paired speed measurements."""
import json
import statistics
import sys
from pathlib import Path

prefix = sys.argv[1] if len(sys.argv) > 1 else 'docs/benchmarks/engine-fold-20261011'
expected = {'common:0':100000, 'common:50':100000, 'common:99950':100000,
            'rare:0':264, 'rare:50':264, 'short:0':100000, 'short:50':100000,
            'and:0':16537, 'and:50':16537}
variants = {}
for variant in ['baseline','candidate']:
    runs = []
    for run in range(3):
        name = f'{prefix}-{variant}-{run}'
        rows = [json.loads(line) for line in Path(name+'.jsonl').read_text(encoding='utf-8-sig').splitlines() if line.strip()]
        memory = json.loads(Path(name+'.memory.json').read_text(encoding='utf-8-sig'))
        assert memory['exit_code'] == 0 and memory['variant'] == variant
        opening = next(r for r in rows if r['phase'] == 'open')
        assert opening['stats']['documents'] == 100000
        assert opening['stats']['resident_documents'] == 0 and opening['stats']['segments'] == 1
        queries = {f"{r['query']}:{r['offset']}":r for r in rows if r['phase']=='query'}
        assert set(queries) == set(expected)
        for key,row in queries.items():
            assert row['api']=='Database::search' and row['full_page_oracle_verified'] and not row['profiled']
            assert row['total']==expected[key]
        runs.append({'opening':opening,'queries':queries,'memory':memory})
    variants[variant] = {'runs':runs,'median':{
        'open_ms':statistics.median(r['opening']['open_ms'] for r in runs),
        'peak_working_set_bytes':statistics.median(r['memory']['peak_working_set_bytes'] for r in runs),
        'sampled_peak_private_bytes':statistics.median(r['memory']['sampled_peak_private_bytes'] for r in runs),
        'queries_ms':{q:statistics.median(r['queries'][q]['p50_ms'] for r in runs) for q in expected},
    }}
for run in range(3):
    left,right = [variants[v]['runs'][run] for v in ['baseline','candidate']]
    assert left['memory']['database'] == right['memory']['database']
    assert left['opening']['stats'] == right['opening']['stats']
    for key in expected:
        assert all(left['queries'][key][k]==right['queries'][key][k] for k in ['text','total','offset','cache','api'])
before,after = [variants[v]['median'] for v in ['baseline','candidate']]
gates = {
    'all_pages_repeatably_faster':all(variants['candidate']['runs'][r]['queries'][q]['p50_ms'] < variants['baseline']['runs'][r]['queries'][q]['p50_ms'] for r in range(3) for q in expected),
    'working_set_no_regression':after['peak_working_set_bytes'] <= before['peak_working_set_bytes'],
}
print(json.dumps({'variants':variants,'gates':gates,
    'query_change_percent':{q:100*(after['queries_ms'][q]/before['queries_ms'][q]-1) for q in expected},
    'scope':'actual Database::search; one legacy-backed SEG03 disk segment; warm cache',
    'decision':'passed_measured_query_gate' if all(gates.values()) else 'gate_not_passed'},indent=2))
