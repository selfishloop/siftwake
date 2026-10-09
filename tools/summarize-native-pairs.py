"""Validate and summarize complete, independently verified native runs."""
import argparse
import json
from pathlib import Path
import statistics

parser = argparse.ArgumentParser()
parser.add_argument('prefix', help='prefix followed by -{label}-{run}')
parser.add_argument('--runs', type=int, default=3)
parser.add_argument('--engines', nargs='+', default=['fdb', 'sqlite'], help='labels for before/after or component comparisons')
args = parser.parse_args()
assert len(args.engines) >= 2 and len(set(args.engines)) == len(args.engines), 'at least two unique labels required'
results = {}
identities = {}
for engine in args.engines:
    samples = []
    for run in range(args.runs):
        prefix = Path(f'{args.prefix}-{engine}-{run}')
        memory = json.loads(Path(str(prefix) + '.memory.json').read_text(encoding='utf-8-sig'))
        assert memory['exit_code'] == 0, (engine, run, 'process failed')
        rows = [json.loads(line) for line in Path(str(prefix) + '.jsonl').read_text(encoding='utf-8-sig').splitlines() if line.strip()]
        final = [row for row in rows if row['phase'] == 'result']
        assert len(final) == 1, (engine, run, 'missing/duplicate result')
        final = final[0]
        assert final['verified_updates'] == final['verified_deletes'] == 50
        assert final['query_verification'].startswith('full StoredDocument oracle')
        if identities:
            assert (final['count'], final['text_bytes']) == next(iter(identities.values()))
        identities[engine] = (final['count'], final['text_bytes'])
        metrics = {name: final[name] for name in ['ingest_ms', 'maintenance_ms', 'reopen_ms']}
        metrics['ingest_plus_maintenance_ms'] = final['ingest_ms'] + final['maintenance_ms']
        for name in ['narrow', 'broad', 'range', 'broad_text', 'last_page', 'second_page', 'get_100']:
            metrics[name + '_p50_ms'] = final[name]['p50_ms']
            metrics[name + '_p95_ms'] = final[name]['p95_ms']
        for name in ['peak_working_set_bytes', 'sampled_peak_private_bytes', 'total_directory_bytes']:
            metrics[name] = memory[name]
        samples.append(metrics)
    results[engine] = dict(median={key: statistics.median(row[key] for row in samples) for key in samples[0]}, runs=samples)
print(json.dumps(dict(runs=args.runs, count=identities[args.engines[0]][0], text_bytes=identities[args.engines[0]][1],
    cache='warm local OS cache from new-DB ingest and per-query untimed warmup',
    memory_window='entire native benchmark process including queries' + (' and SQLite final integrity checks' if 'sqlite' in args.engines else ''),
    shared_query_contract='exact total + ordered decoded StoredDocument page; independent corpus oracle',
    open_contract_differs='sqlite' in args.engines, results=results), indent=2))
