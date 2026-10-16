"""Compare folding in the same binary/corpus/codec; never reuse old timings."""
import json
import statistics
import sys
from pathlib import Path

prefix = sys.argv[1] if len(sys.argv) > 1 else 'docs/benchmarks/fold-20261011'
queries = ['common', 'rare', 'short', 'and']
report = {}
for codec in ['json', 'rkyv']:
    strategies = {}
    for strategy in ['baseline', 'reuse']:
        runs = []
        for run in range(3):
            name = f'{prefix}-{codec}-{strategy}-{run}'
            rows = [json.loads(line) for line in Path(name+'.jsonl').read_text(encoding='utf-8-sig').splitlines() if line.strip()]
            memory = json.loads(Path(name+'.memory.json').read_text(encoding='utf-8-sig'))
            opening = next(row for row in rows if row['phase'] == 'open')
            measured = {row['query']: row for row in rows if row['phase'] == 'query'}
            assert memory['exit_code'] == 0 and memory['strategy'] == strategy
            assert opening['count'] == 100_000 and not opening['profiled']
            assert set(measured) == set(queries)
            assert all(q['full_page_oracle_verified'] and not q['profiled'] and q['strategy'] == strategy for q in measured.values())
            runs.append({'opening': opening, 'queries': measured, 'memory': memory})
        strategies[strategy] = {
            'runs': runs,
            'median': {
                'disk_bytes': statistics.median(r['opening']['disk_bytes'] for r in runs),
                'peak_working_set_bytes': statistics.median(r['memory']['peak_working_set_bytes'] for r in runs),
                'sampled_peak_private_bytes': statistics.median(r['memory']['sampled_peak_private_bytes'] for r in runs),
                'queries_ms': {q: statistics.median(r['queries'][q]['p50_ms'] for r in runs) for q in queries},
            },
        }
    for run in range(3):
        left, right = [strategies[s]['runs'][run] for s in ['baseline', 'reuse']]
        assert left['memory']['executable'] == right['memory']['executable']
        assert left['memory']['corpus'] == right['memory']['corpus']
        assert left['opening']['validation'] == right['opening']['validation']
        for q in queries:
            assert all(left['queries'][q][k] == right['queries'][q][k] for k in ['codec', 'text', 'candidates', 'total', 'warmup', 'cache'])
    before, after = [strategies[s]['median'] for s in ['baseline', 'reuse']]
    gates = {
        'repeatable_query_gain': all(strategies['reuse']['runs'][r]['queries'][q]['p50_ms'] < strategies['baseline']['runs'][r]['queries'][q]['p50_ms'] for r in range(3) for q in queries),
        'disk_unchanged': before['disk_bytes'] == after['disk_bytes'],
        'working_set_no_regression': after['peak_working_set_bytes'] <= before['peak_working_set_bytes'],
    }
    report[codec] = {
        'strategies': strategies, 'gates': gates,
        'query_change_percent': {q: 100*(after['queries_ms'][q]/before['queries_ms'][q]-1) for q in queries},
        'decision': 'passed_payload_experiment' if all(gates.values()) else 'rejected',
    }
print(json.dumps({'codecs': report, 'default_engine_changed': False,
                  'scope': 'same-codec payload query; warm cache; candidates precomputed'}, indent=2))
