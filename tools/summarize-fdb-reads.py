"""Summarize independent-run medians; reject incomplete comparisons."""
import argparse
import json
import statistics

parser = argparse.ArgumentParser()
parser.add_argument('jsonl')
parser.add_argument('--runs', type=int, default=5)
args = parser.parse_args()
with open(args.jsonl, encoding='utf-8-sig') as stream:
    rows = [json.loads(line) for line in stream if line.strip()]
result = {}
for name in sorted({r['query'] for r in rows if r['phase'] == 'query'}):
    summaries = {}
    for engine in ['baseline', 'candidate']:
        selected = [r for r in rows if r['phase'] == 'query' and r['engine'] == engine and r['query'] == name]
        assert sorted(r['run'] for r in selected) == list(range(args.runs)), (engine,name,'incomplete runs')
        summaries[engine] = dict(
            median_run_p50_ms=statistics.median(r['timing']['p50_ms'] for r in selected),
            median_run_p95_ms=statistics.median(r['timing']['p95_ms'] for r in selected),
            worst_run_p95_ms=max(r['timing']['p95_ms'] for r in selected),
            total=selected[0]['total'],hits=selected[0]['hits'])
    summaries['candidate_to_baseline_p50_ratio'] = summaries['candidate']['median_run_p50_ms']/summaries['baseline']['median_run_p50_ms']
    result[name] = summaries
print(json.dumps(dict(runs=args.runs,cache='warm local OS cache',includes_stdio=True,queries=result),indent=2))
