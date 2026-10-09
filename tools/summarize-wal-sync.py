"""Verify paired commit corpora/WAL identity, then summarize sync-only timing."""
import hashlib
import json
import statistics
import sys
from pathlib import Path

prefix = sys.argv[1] if len(sys.argv) > 1 else 'wal-sync-20261009'
results = []
for batch in [1, 50, 500]:
    modes = {mode: [] for mode in ['on', 'off']}
    wal_hashes = []
    for run in range(3):
        hashes = []
        for mode in modes:
            value = json.loads(Path(f'docs/benchmarks/{prefix}-commit-{batch}-{mode}-{run}.json').read_text(encoding='utf-8-sig'))
            assert value['count'] == 5000 and value['text_bytes'] == 256
            assert value['batch_size'] == batch and value['wal_sync'] == mode
            assert value['wal_enabled'] and value['no_flush_or_merge_in_timer']
            assert value['full_reopen_oracle_verified'] and not value['power_failure_tested']
            files = sorted(Path(f'data/{prefix}-commit-{batch}-{mode}-{run}').glob('*.fdb'))
            assert len(files) == 1, 'unexpected flush/source count'
            hashes.append(hashlib.sha256(files[0].read_bytes()).hexdigest())
            modes[mode].append(value)
        assert hashes[0] == hashes[1], 'paired WAL bytes differ'
        wal_hashes.append(hashes[0])
    row = {'batch_size': batch, 'wal_sha256_by_pair': wal_hashes}
    for mode, values in modes.items():
        row[mode] = {
            'ingest_ms': statistics.median(value['ingest_ms'] for value in values),
            'commit_p50_ms': statistics.median(value['batches']['p50_ms'] for value in values),
            'commit_p95_ms': statistics.median(value['batches']['p95_ms'] for value in values),
            'runs': values,
        }
    row['ingest_reduction_percent'] = 100 * (1 - row['off']['ingest_ms'] / row['on']['ingest_ms'])
    results.append(row)
print(json.dumps(results, indent=2))
