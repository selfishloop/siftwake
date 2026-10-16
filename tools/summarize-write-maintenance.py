"""Summarize fixed-slot 10k write/maintenance probes, without DB access."""
import json
import statistics
from pathlib import Path

root = Path(__file__).resolve().parent.parent
prefix = root / 'docs/benchmarks/write-maintenance-current-10k-20261013'

def read(path):
    return [json.loads(line) for line in path.read_text(encoding='utf-8-sig').splitlines()]

rows = []
for run in range(3):
    for engine in ('fdb', 'sqlite'):
        base = Path(f'{prefix}-{run}-{engine}-ingest-10000-0')
        result = next(row for row in read(base.with_suffix('.jsonl')) if row['phase'] == 'result')
        assert result['to'] == 10000 and result['all_input_crc_verified']
        memory = json.loads(base.with_suffix('.memory.json').read_text(encoding='utf-8-sig'))
        assert memory['exit_code'] == 0 and not memory['low_disk_stop']
        idle = json.loads(Path(f'{prefix}-{run}-{engine}-idle-10000-0.memory.json').read_text(encoding='utf-8-sig'))
        assert idle['exit_code'] == 0 and idle['idle']
        rows.append({
            'run': run, 'engine': engine,
            'ingest_s': result['ingest_ms'] / 1000,
            'maintenance_s': result['maintenance_ms'] / 1000,
            'combined_s': (result['ingest_ms'] + result['maintenance_ms']) / 1000,
            'observed_os_peak_ws_bytes': memory['os_peak_working_set_bytes'],
            'sampled_indexing_private_bytes': memory['phases']['indexing']['sampled_peak_private_bytes'],
            'sampled_maintenance_private_bytes': memory['phases']['maintenance']['sampled_peak_private_bytes'],
            'idle_private_bytes': idle['idle']['private_bytes'],
            'idle_ws_bytes': idle['idle']['working_set_bytes'],
            'retained_disk_bytes': memory['retained_disk_bytes'],
            'stats': result.get('stats'),
        })
metrics = [key for key in rows[0] if key not in ('run', 'engine', 'stats')]
summary = {'runs': rows, 'median': {
    engine: {metric: statistics.median(row[metric] for row in rows if row['engine'] == engine)
             for metric in metrics} for engine in ('fdb', 'sqlite')},
    'scope': 'same first 10k existing corpus, batch500, synced WAL; warm OS cache without eviction; whole-process sampled memory, no engine-only allocation claim'}
Path(f'{prefix}-summary.json').write_text(json.dumps(summary, indent=2), encoding='utf-8')
print(json.dumps(summary, indent=2))
