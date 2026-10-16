"""Matched 10k write runs and separate 1k delta queries; no extrapolation."""
import json
import re
import statistics
from pathlib import Path

BASE = Path('docs/benchmarks')


def rows(path):
    return [json.loads(line) for line in path.read_text(encoding='utf-8-sig').splitlines() if line.strip()]


def write_run(variant, run):
    stem = BASE / f'content-memory-{variant}-10k-20261012-{run}-fdb-ingest-10000-0'
    memory = json.loads(Path(str(stem)+'.memory.json').read_text(encoding='utf-8-sig'))
    result = next(r for r in rows(Path(str(stem)+'.jsonl')) if r['phase'] == 'result')
    assert memory['exit_code'] == 0 and not memory['low_disk_stop']
    assert result['all_input_crc_verified'] and result['sampled_full_document_checks'] == 40
    assert result['from'] == 0 and result['to'] == 10000
    assert result['stats']['documents'] == 10000 and result['stats']['resident_documents'] == 0
    assert result['stats']['sequence'] == 20
    assert memory['features'] == ('experimental-delta-bitmaps' if variant == 'bitmap-reserve' else 'default only')
    phases = memory['phases']
    return dict(ingest_ms=result['ingest_ms'], maintenance_ms=result['maintenance_ms'],
                total_ms=result['ingest_ms']+result['maintenance_ms'],
                peak_private_bytes=max(phases[p]['sampled_peak_private_bytes'] for p in ['indexing', 'maintenance']),
                peak_working_set_bytes=max(phases[p]['sampled_peak_working_set_bytes'] for p in ['indexing', 'maintenance']),
                text_bytes=result['text_bytes'], stats=result['stats'],
                retained_disk_bytes=memory['retained_disk_bytes'], executable=memory['executable'])


output = {'scope': 'first 10000 long corpus records; two fresh ingests per FDB variant, reverse order; OS cache not evicted; sampled Windows counters',
          'variants': {}, 'queries': {}, 'tests': {},
          'limits': ['not 50k acceptance', 'not cold cache', 'delta queries use only 1000 documents',
                     'no 30-query SQLite comparison', 'no concurrent readers/writer',
                     'no derived profile or DLL', 'no temporary disk peak']}
reference = None
for variant in ['control', 'reserve', 'bitmap-reserve']:
    samples = [write_run(variant, n) for n in range(2)]
    for sample in samples:
        identity = (sample['text_bytes'], sample['stats'], sample['retained_disk_bytes'])
        if reference is None:
            reference = identity
        assert reference == identity, variant
    output['variants'][variant] = {'runs': samples, 'median': {
        k: statistics.median(s[k] for s in samples)
        for k in ['ingest_ms', 'maintenance_ms', 'total_ms', 'peak_private_bytes', 'peak_working_set_bytes']}}
    idle = []
    for n in range(3):
        path = BASE / f'content-memory-{variant}-10k-20261012-0-fdb-idle-10000-{n}.memory.json'
        item = json.loads(path.read_text(encoding='utf-8-sig'))
        assert item['exit_code'] == 0 and item['idle']['search_calls'] == 0
        idle.append(item['idle']['private_bytes'])
    output['variants'][variant]['idle_private_samples'] = idle
    output['variants'][variant]['idle_private_median'] = statistics.median(idle)

for variant in ['keys', 'bitmap']:
    records = [rows(BASE / f'delta-queries-{variant}-20261012-{n}.jsonl') for n in range(2)]
    for run in records:
        last = run[-1]
        assert last['documents'] == 1000 and last['sequence'] == 3
        assert last['crc_checked_input'] and last['reverse_ingest_and_delete_reinsert_verified']
    for state in ['delta', 'reused_slots', 'reopened_delta']:
        output['queries'].setdefault(state, {})[variant] = {}
        for text in ['the', 'quixote', 'he', 'the man', 'istanbul', 'updatedunique']:
            found = [next(r for r in run if r.get('state') == state and r.get('text') == text) for run in records]
            assert all(r['full_page_oracle'] for r in found)
            assert found[0]['total'] == found[1]['total']
            samples = [v for r in found for v in r['samples_ms']]
            output['queries'][state][variant][text] = dict(total=found[0]['total'],
                p50_ms=statistics.median(samples), p95_ms=sorted(samples)[18], samples_ms=samples)
for state, variants in output['queries'].items():
    for text in variants['keys']:
        assert variants['keys'][text]['total'] == variants['bitmap'][text]['total'], (state, text)
for mode in ['default', 'all']:
    text = (BASE / f'content-optimization-20261012-tests-{mode}.log').read_text(encoding='utf-8-sig')
    assert 'test result: FAILED' not in text
    output['tests'][mode] = sum(int(n) for n in re.findall(r'test result: ok\. (\d+) passed', text))
Path(BASE / 'content-optimization-20261012-summary.json').write_text(json.dumps(output, indent=2), encoding='utf-8')
print(json.dumps({v: {k: x for k, x in item.items() if k != 'runs'} for v, item in output['variants'].items()}, indent=2))
