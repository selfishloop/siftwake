"""Summarize fresh unflushed delta queries; timings exclude writes and open."""
import argparse
import json
from pathlib import Path
import statistics

parser = argparse.ArgumentParser()
parser.add_argument('prefix')
parser.add_argument('--runs', type=int, default=3)
args = parser.parse_args()
assert args.runs > 0
results = {}
for mode in ['repeated', 'unique', 'mixed']:
    results[mode] = {}
    for label in ['baseline', 'candidate']:
        rows = []
        for run in range(args.runs):
            path = Path(f'{args.prefix}-{mode}-{label}-{run}.jsonl')
            row = json.loads(path.read_text(encoding='utf-8-sig'))
            assert row['corpus_mode'] == mode and row['counts_verified']
            assert not row['flushed'] and row['open_excluded'] and row['writes_before_queries']
            assert row['count'] == row['stats']['documents'] == row['stats']['delta_keys'] == 10000
            assert row['stats']['segments'] == 0 and row['stats']['sequence'] == 20
            assert row['text_bytes'] == 256 and row['warmup'] == 10
            assert row['small_repeats'] == 200 and row['get_repeats'] == row['last_repeats'] == 100
            assert row['text_repeats'] == 20
            rows.append(row)
        results[mode][label] = {
            query: {metric: statistics.median(row[query][metric] for row in rows)
                    for metric in ['p50_ms', 'p95_ms']}
            for query in ['narrow', 'broad', 'range', 'second_page', 'last_page', 'broad_text', 'get_100']
        }
print(json.dumps(results, indent=2))
