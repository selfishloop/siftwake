"""Fixed-slot paired stream checkpoint results; reads logs only."""
import json
import statistics
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
prefix = root / (sys.argv[1] if len(sys.argv) > 1 else 'docs/benchmarks/ram-stream-10k-20261013')

def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

rows = []
input_modes = {}
for run in range(3):
    for variant in ('control', 'candidate'):
        base = f'{prefix}-{variant}-{run}'
        events = [json.loads(line) for line in Path(f'{base}-fdb-ingest-10000-0.jsonl').read_text(encoding='utf-8-sig').splitlines()]
        result = next(row for row in events if row['phase'] == 'result')
        budget = next((row for row in events if row['phase'] == 'controlled_posting_budget'), {})
        streamed = budget.get('streamed_input', False)
        if variant in input_modes:
            assert input_modes[variant] == streamed
        input_modes[variant] = streamed
        memory = read(Path(f'{base}-fdb-ingest-10000-0.memory.json'))
        idle = read(Path(f'{base}-fdb-idle-10000-0.memory.json'))
        assert memory['exit_code'] == 0 and idle['exit_code'] == 0
        assert not memory['low_disk_stop'] and result['to'] == 10000
        rows.append({'variant': variant, 'run': run,
                     'ingest_s': result['ingest_ms'] / 1000,
                     'maintenance_s': result['maintenance_ms'] / 1000,
                     'combined_s': (result['ingest_ms'] + result['maintenance_ms']) / 1000,
                     'indexing_private_bytes': memory['phases']['indexing']['sampled_peak_private_bytes'],
                     'maintenance_private_bytes': memory['phases']['maintenance']['sampled_peak_private_bytes'],
                     'os_peak_ws_bytes': memory['os_peak_working_set_bytes'],
                     'idle_private_bytes': idle['idle']['private_bytes'],
                     'disk_bytes': memory['retained_disk_bytes']})
metrics = [key for key in rows[0] if key not in ('variant', 'run')]
medians = {variant: {key: statistics.median(row[key] for row in rows if row['variant'] == variant)
                     for key in metrics} for variant in ('control', 'candidate')}
pairs = [{'run': run, **{key: 100 * (rows[run * 2 + 1][key] / rows[run * 2][key] - 1)
                        for key in metrics}} for run in range(3)]
summary = {'rows': rows, 'median': medians, 'paired_change_percent': pairs,
           'streamed_input': input_modes,
           'scope': 'same first 10k corpus records, paired release, synced WAL; feature labels and executable hashes in logs; warm OS cache without eviction; sampled whole-process private bytes and working set'}
Path(f'{prefix}-summary.json').write_text(json.dumps(summary, indent=2), encoding='utf-8')
print(json.dumps(summary, indent=2))
