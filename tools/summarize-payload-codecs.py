"""Validate paired workload/oracle/validation windows and enforce every gate."""
import json
import statistics
import sys
from pathlib import Path

prefix = sys.argv[1] if len(sys.argv) > 1 else 'docs/benchmarks/rkyv-payload-v1-20261009'
candidate = sys.argv[2] if len(sys.argv) > 2 else 'rkyv'
suffix = '-' + sys.argv[3] if len(sys.argv) > 3 else ''
assert candidate in ['rkyv', 'binary']
results = {}
for codec in ['json', candidate]:
    runs = []
    for run in range(3):
        name = f'{prefix}-{codec}{suffix}-{run}'
        rows = [json.loads(line) for line in Path(name+'.jsonl').read_text(encoding='utf-8-sig').splitlines() if line.strip()]
        memory = json.loads(Path(name+'.memory.json').read_text(encoding='utf-8-sig'))
        assert memory['exit_code'] == 0 and memory['codec'] == codec
        opening = next(row for row in rows if row['phase'] == 'open')
        assert opening['count'] == 100_000 and opening['codec'] == codec and not opening['profiled']
        queries = {row['query']: row for row in rows if row['phase'] == 'query'}
        assert set(queries) == {'common', 'rare', 'short', 'and'}
        assert all(q['codec'] == codec and q['full_page_oracle_verified'] and not q['profiled'] for q in queries.values())
        runs.append({'open':opening,'queries':queries,'memory':memory})
    medians = {
        'disk_bytes': statistics.median(r['open']['disk_bytes'] for r in runs),
        'open_ms': statistics.median(r['open']['open_ms'] for r in runs),
        'peak_working_set_bytes': statistics.median(r['memory']['peak_working_set_bytes'] for r in runs),
        'sampled_peak_private_bytes': statistics.median(r['memory']['sampled_peak_private_bytes'] for r in runs),
        'queries': {q:statistics.median(r['queries'][q]['p50_ms'] for r in runs) for q in ['common','rare','short','and']},
    }
    results[codec] = {'median':medians,'runs':runs}
for run in range(3):
    for q in ['common','rare','short','and']:
        left, right = [results[c]['runs'][run]['queries'][q] for c in ['json',candidate]]
        assert all(left[k] == right[k] for k in ['text','candidates','total','warmup','cache','strategy'])
        assert left['total'] > 0 and left['candidates'] >= left['total']
left, right = [results[c]['median'] for c in ['json',candidate]]
gates = {
    'disk_no_regression':right['disk_bytes'] <= left['disk_bytes'],
    'process_working_set_no_regression':right['peak_working_set_bytes'] <= left['peak_working_set_bytes'],
    # Every query must improve in all three matching process pairs, not just a
    # cherry-picked median. This is a conservative prototype criterion.
    'repeatable_contains_gain':all(results[candidate]['runs'][r]['queries'][q]['p50_ms'] < results['json']['runs'][r]['queries'][q]['p50_ms'] for r in range(3) for q in ['common','rare','short','and']),
}
print(json.dumps({'results':results,'gates':gates,'decision':'passed_codec_only' if all(gates.values()) else 'rejected','default_enabled':False,'engine_integration':False,'scope':'payload verification+count+50 full documents; candidate/index construction excluded'},indent=2))
