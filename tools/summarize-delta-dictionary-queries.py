"""Matched unflushed delta query probes; no DB/corpus access."""
import json
import math
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
prefix = root / (sys.argv[1] if len(sys.argv) > 1 else 'docs/benchmarks/delta-dictionary-query-20261013')
series = {}
layouts = {}
for variant in ('control', 'candidate'):
    series[variant] = []
    for run in range(3):
        rows = [json.loads(line) for line in Path(f'{prefix}-{variant}-{run}.jsonl').read_text(encoding='utf-8-sig').splitlines()]
        result = next(row for row in rows if row['phase'] == 'result')
        assert result['documents'] == 1000 and result['reverse_ingest_and_delete_reinsert_verified']
        layout = {'streamed_input': result.get('streamed_input', False),
                  'delta_keys': result.get('physical_delta_keys', 'not reported')}
        if variant in layouts:
            assert layouts[variant] == layout
        layouts[variant] = layout
        series[variant].append([row for row in rows if row['phase'] == 'query'])
queries = []
for n, row in enumerate(series['control'][0]):
    identity = {key: row[key] for key in ('state', 'text', 'total')}
    query = dict(identity)
    for variant in series:
        values = [run[n] for run in series[variant]]
        assert all(all(value[key] == expected for key, expected in identity.items()) for value in values)
        assert all(value['full_page_oracle'] for value in values)
        samples = sorted(sample for value in values for sample in value['samples_ms'])
        query[variant] = {'n': len(samples), 'p95_ms': samples[math.ceil(len(samples)*.95)-1],
                          'run_p95_ms': [value['p95_ms'] for value in values]}
    query['change_p95_percent'] = 100*(query['candidate']['p95_ms']/query['control']['p95_ms']-1)
    query['faster_pairs'] = sum(a < b for a, b in zip(query['candidate']['run_p95_ms'], query['control']['run_p95_ms']))
    queries.append(query)
result = {'queries': queries, 'lower_p95_cases': sum(query['change_p95_percent'] < 0 for query in queries),
          'higher_p95_cases': sum(query['change_p95_percent'] > 0 for query in queries),
          'physical_layouts': layouts,
          'scope': 'first 1000 of existing 10k corpus; 40 delete/reinsert mutations, reopen; logical state names do not identify physical layout; warm query, no cache eviction; no SQLite comparison or process RAM measurement'}
Path(f'{prefix}-summary.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
print(json.dumps(result, indent=2))
