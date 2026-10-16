"""Compare diagnostic page work counts; nested wall spans are not CPU shares."""
import argparse
import json
import math
import statistics
from collections import defaultdict
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('prefix')
args = parser.parse_args()
prefix = Path(args.prefix)
design = json.loads(Path(f'{prefix}-design.json').read_text(encoding='utf-8-sig'))
runs = {engine: defaultdict(list) for engine in ('control', 'fdb')}
for engine in runs:
    for pair in range(design['pairs']):
        rows = [json.loads(line) for line in Path(f'{prefix}-{engine}-{pair}.jsonl').read_text(encoding='utf-8-sig').splitlines()]
        oracle = next(row for row in rows if row['phase'] == 'matrix_oracle')
        assert oracle['source_crc_verified'] and oracle['records'] == 10000
        rows = [row for row in rows if row['phase'] == 'selectivity_profile']
        assert len(rows) == 8 * 30, (engine, pair, len(rows))
        iterations = defaultdict(list)
        for row in rows:
            assert row['instrumented']
            key = row['text'], row['offset'], row['limit'], row['total']
            iterations[key].append(row['iteration'])
            assert row['thread_cycles'] is not None and row['thread_cycles'] > 0
            runs[engine][key].append(row)
        assert len(iterations) == 8 and all(sorted(values) == list(range(30)) for values in iterations.values())

assert runs['control'].keys() == runs['fdb'].keys()
queries = []
for key in runs['control']:
    samples = {engine: data[key] for engine, data in runs.items()}
    series = {engine: [row['counters'] for row in rows] for engine, rows in samples.items()}
    names = sorted({name for rows in series.values() for row in rows for name in row})
    stages = {}
    for name in names:
        entry = {}
        for engine, rows in series.items():
            calls = [row.get(name, {}).get('calls', 0) for row in rows]
            elapsed = [row.get(name, {}).get('elapsed_ns', 0) / 1e6 for row in rows]
            entry[engine] = dict(call_values=sorted(set(calls)), median_ms=statistics.median(elapsed), samples=len(rows))
        entry['all_call_counts_identical'] = entry['control']['call_values'] == entry['fdb']['call_values'] and len(entry['control']['call_values']) == 1
        stages[name] = entry
    diagnostics = {}
    for engine, rows in samples.items():
        diagnostics[engine] = {}
        for field in ('elapsed_ms', 'thread_cycles'):
            values = sorted(row[field] for row in rows)
            process_medians = [statistics.median(row[field] for row in rows[n:n + 30]) for n in range(0, len(rows), 30)]
            diagnostics[engine][field] = dict(median=statistics.median(values), p95=values[math.ceil(len(values) * .95) - 1], per_process_medians=process_medians)
    for field in ('elapsed_ms', 'thread_cycles'):
        diagnostics[f'{field}_median_change_percent'] = 100 * (diagnostics['fdb'][field]['median'] / diagnostics['control'][field]['median'] - 1)
        diagnostics[f'{field}_paired_median_change_percent'] = [100 * (a / b - 1) for a, b in zip(diagnostics['fdb'][field]['per_process_medians'], diagnostics['control'][field]['per_process_medians'])]
    queries.append(dict(text=key[0], offset=key[1], limit=key[2], total=key[3], all_work_counts_identical=all(s['all_call_counts_identical'] for s in stages.values()), diagnostics=diagnostics, stages=stages))

result = dict(design=design, queries=queries, all_query_work_counts_identical=all(q['all_work_counts_identical'] for q in queries), body_index_extraction_calls=sum(row['counters'].get('body.grams', {}).get('calls', 0) + row['counters'].get('index.gram_windows', {}).get('calls', 0) for data in runs.values() for rows in data.values() for row in rows), interpretation='Diagnostic nested wall spans overlap and include instrumentation. Thread cycles describe only the calling benchmark thread, not the whole process. Do not add nested spans, call them CPU shares, or substitute instrumented results for normal-release p95 comparisons.')
Path(f'{prefix}-summary.json').write_text(json.dumps(result, indent=2, ensure_ascii=False), encoding='utf-8')
print(json.dumps(dict(all_query_work_counts_identical=result['all_query_work_counts_identical'], body_index_extraction_calls=result['body_index_extraction_calls'], queries=[dict(text=q['text'], offset=q['offset'], limit=q['limit'], all_work_counts_identical=q['all_work_counts_identical'], search=q['stages']['search.total']) for q in queries]), indent=2))
