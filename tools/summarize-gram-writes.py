"""Only aggregate existing logs; never open, generate or reset databases."""
import argparse
import json
import statistics
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('prefix', nargs='?', default='docs/benchmarks/gram-key-write-10k-20261016')
parser.add_argument('tail', nargs='?')
parser.add_argument('--sqlite-prefix')
parser.add_argument('--sqlite-tail')
args = parser.parse_args()
prefix = root / args.prefix
tail = root / args.tail if args.tail else (
    root / 'docs/benchmarks/gram-key-write-tail-10k-20261016'
    if prefix.name == 'gram-key-write-10k-20261016' else None)
sql_prefix = root / args.sqlite_prefix if args.sqlite_prefix else prefix
sql_tail = root / args.sqlite_tail if args.sqlite_tail else tail


def source(pair):
    return tail if pair == 3 and tail else prefix


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def phases(path):
    return {row['phase']: row for row in map(json.loads, path.read_text(encoding='utf-8-sig').splitlines())}


def row(variant, pair):
    engine = 'sqlite' if variant == 'sqlite' else 'fdb'
    origin = (sql_tail if pair == 3 and sql_tail else sql_prefix) if variant == 'sqlite' else source(pair)
    stem = Path(f'{origin}-{variant}-{pair}-{engine}-ingest-10000-0')
    log = phases(Path(f'{stem}.jsonl'))
    memory = read(Path(f'{stem}.memory.json'))
    verify = phases(Path(f'{origin}-{variant}-{pair}-verify.jsonl'))['verified']
    assert verify['full_document_checks'] == 10000 and verify['input_crc_verified']
    if variant != 'sqlite':
        assert verify['first50_full_pages_verified']
    result = log['result']
    assert result['text_bytes'] == 264842858 and memory['exit_code'] == 0 and not memory['low_disk_stop']
    return {'variant': variant, 'pair': pair,
            'ingest_s': result['ingest_ms'] / 1000,
            'maintenance_s': result['maintenance_ms'] / 1000,
            'combined_s': (result['ingest_ms'] + result['maintenance_ms']) / 1000,
            'cpu_s': memory['cpu_ms'] / 1000,
            'sampled_indexing_private_bytes': memory['phases']['indexing']['sampled_peak_private_bytes'],
            'sampled_maintenance_private_bytes': memory['phases']['maintenance']['sampled_peak_private_bytes'],
            'os_lifetime_private_commit_bytes': memory['os_lifetime_peaks']['peak_private_commit_bytes'],
            'os_lifetime_working_set_bytes': memory['os_lifetime_peaks']['peak_working_set_bytes'],
            'retained_disk_bytes': memory['retained_disk_bytes']}


design = read(Path(f'{prefix}-design.json'))
first_pair = design.get('first_pair', 0)
pair_indices = list(range(first_pair, first_pair + design.get('fdb_pairs', 4)))
rows = [row(e, n) for n in pair_indices for e in ('control', 'fdb')]
sql_indices = (0, 3) if args.sqlite_prefix else design.get('sqlite_references_after_pairs', [0, 3])
sql = [row('sqlite', n) for n in sql_indices]
fields = [k for k in rows[0] if k not in ('variant', 'pair')]
medians = {e: {k: statistics.median(r[k] for r in group if r['variant'] == e) for k in fields}
           for e, group in (('control', rows), ('fdb', rows), ('sqlite', sql)) if group}
paired = {}
for field in fields:
    control = [next(r[field] for r in rows if r['pair'] == n and r['variant'] == 'control') for n in pair_indices]
    candidate = [next(r[field] for r in rows if r['pair'] == n and r['variant'] == 'fdb') for n in pair_indices]
    changes = [100 * (b / a - 1) for a, b in zip(control, candidate)]
    paired[field] = {'change_of_medians_percent': 100 * (medians['fdb'][field] / medians['control'][field] - 1),
                     'paired_change_percent': changes, 'median_paired_change_percent': statistics.median(changes),
                     'lower_pairs': sum(b < a for a, b in zip(control, candidate)),
                     'higher_pairs': sum(b > a for a, b in zip(control, candidate))}
files = [read(Path(f'{source(n)}-{e}-{n}-files.json')) for n in pair_indices for e in ('control', 'fdb')]
assert all(f == files[0] for f in files)
original_hashes = {r['engine']: r['sha256'] for r in read(Path(f'{prefix}-executables.json'))}
if tail:
    tail_hashes = {r['engine']: r['sha256'] for r in read(Path(f'{tail}-executables.json'))}
    assert original_hashes == tail_hashes
sql_hashes = {r['engine']: r['sha256'] for r in read(Path(f'{sql_prefix}-executables.json'))}
assert original_hashes['sqlite'] == sql_hashes['sqlite']
result = {'design': design, 'rows': rows, 'sqlite_unpaired_references': sql,
          'sqlite_reference_logs': {'prefix': str(sql_prefix.relative_to(root)),
                                    'tail': str(sql_tail.relative_to(root)) if sql_tail else None,
                                    'reused_from_previous_stage': sql_prefix != prefix},
          'resume': {'pair3_prefix': str(tail.relative_to(root)),
                     'reason': 'phase monitor failed after initial pair3 candidate; incomplete attempt excluded, complete pair3 rerun; initial pairs0..2 retained',
                     'same_frozen_executables_verified': True,
                     'combined_balanced_order': ['control/fdb', 'fdb/control', 'control/fdb', 'fdb/control']} if tail else None,
          'medians': medians, 'paired_fdb_changes': paired, 'all_fdb_core_file_hashes_equal': True,
          'all_eight_fdb_core_file_hashes_equal': len(files) == 8,
          'core_file_count_per_run': len(files[0]),
          'memory_scope': 'phase private peaks sampled20ms/phase100ms; OS lifetime high-water marks include startup/input/indexing/maintenance, not phase-specific; neither is total system RAM',
          'sqlite_scope': f'{len(sql)} unpaired references, not a paired SQLite/FDB performance gate; FULL WAL, cache2MiB/mmap0/tempFILE; maintenance FTS optimize+checkpoint differs from FDB flush/drain merge',
          'interpretation': 'Differences of medians and medians of paired ratios are separate; no statistical significance or default promotion is implied.'}
Path(f'{prefix}-summary.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
print(json.dumps({k: result[k] for k in ('medians', 'paired_fdb_changes', 'all_eight_fdb_core_file_hashes_equal')}, indent=2))
