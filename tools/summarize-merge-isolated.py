"""Isolated posting reuse, fixed DB slots and the same existing 10k documents."""
import json
import re
import statistics
import sys
from pathlib import Path

BASE = Path('docs/benchmarks')
PREFIX = sys.argv[1] if len(sys.argv) > 1 else 'merge-reuse-10k-20261012'
TEST_PREFIX = 'merge-isolated-10k-20261012'


def read_json(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def rows(path):
    return [json.loads(s) for s in path.read_text(encoding='utf-8-sig').splitlines() if s.strip()]


def result(variant, run, engine='fdb'):
    tag = f'{variant}-{run}'
    stem = BASE / f'{PREFIX}-{tag}-{engine}-ingest-10000-0'
    memory = read_json(Path(str(stem)+'.memory.json'))
    event = next(r for r in rows(Path(str(stem)+'.jsonl')) if r['phase'] == 'result')
    assert memory['exit_code'] == 0 and not memory['low_disk_stop']
    assert memory['features'] == ('experimental-merge-postings' if engine == 'fdb' and variant == 'candidate' else 'default only')
    assert event['from'] == 0 and event['to'] == 10000 and event['all_input_crc_verified']
    assert event['sampled_full_document_checks'] == 40
    if engine == 'fdb':
        assert event['stats']['documents'] == 10000 and event['stats']['resident_documents'] == 0
        assert event['stats']['sequence'] == 20 and not event['stats']['compaction_running']
    else:
        assert event['documents'] == 10000
    phases = memory['phases']
    return dict(ingest_ms=event['ingest_ms'], maintenance_ms=event['maintenance_ms'],
                total_ms=event['ingest_ms']+event['maintenance_ms'],
                peak_private_bytes=max(phases[p]['sampled_peak_private_bytes'] for p in ['indexing', 'maintenance']),
                peak_working_set_bytes=max(phases[p]['sampled_peak_working_set_bytes'] for p in ['indexing', 'maintenance']),
                maintenance_private_bytes=phases['maintenance']['sampled_peak_private_bytes'],
                retained_disk_bytes=memory['retained_disk_bytes'], text_bytes=event['text_bytes'],
                stats=event.get('stats'), memory=memory)


output = {'scope': 'two fresh 10k long pairs, control/candidate and candidate/control; only merge algorithm differs; no bitmap or streaming/pipeline features; OS cache not evicted',
          'variants': {}, 'sqlite': {}, 'profiles': {}, 'tests': {},
          'unmeasured': ['cold cache', '30-query p95 benchmark', 'concurrent API readers/writer', 'temporary disk peak', 'DLL', 'derived profile']}
oracle = None
for variant in ['control', 'candidate']:
    samples = [result(variant, run) for run in range(2)]
    for run, sample in enumerate(samples):
        tag = f'{variant}-{run}'
        verified = rows(BASE / f'{PREFIX}-{tag}-verify.jsonl')[-1]
        assert verified['phase'] == 'verified' and verified['full_document_checks'] == 10000
        assert verified['first50_full_pages_verified'] and verified['input_crc_verified'] and verified['outside_timing']
        if oracle is None:
            oracle = (sample['text_bytes'], verified['queries'], verified['totals'])
        assert oracle == (sample['text_bytes'], verified['queries'], verified['totals'])
    median = {key: statistics.median(s[key] for s in samples)
              for key in ['ingest_ms', 'maintenance_ms', 'total_ms', 'peak_private_bytes', 'peak_working_set_bytes', 'maintenance_private_bytes', 'retained_disk_bytes']}
    idle = [read_json(BASE / f'{PREFIX}-{variant}-0-fdb-idle-10000-{n}.memory.json')['idle'] for n in range(3)]
    assert all(i['search_calls'] == 0 and i['after_open_wait_s'] == 5 for i in idle)
    median['idle_private_bytes'] = statistics.median(i['private_bytes'] for i in idle)
    opening = [next(r for r in rows(BASE / f'{PREFIX}-{variant}-0-fdb-idle-10000-{n}.jsonl') if r['phase'] == 'opened')['open_ms'] for n in range(3)]
    median['open_ms'] = statistics.median(opening)
    output['variants'][variant] = dict(median=median, runs=samples, oracle_queries=oracle[1], oracle_totals=oracle[2])
output['sqlite'] = result('candidate', 0, 'sqlite')
assert output['sqlite']['text_bytes'] == oracle[0]
for variant in ['control', 'isolated']:
    events = rows(BASE / f'{PREFIX}-profile-{variant}.jsonl')
    done = next(r for r in events if r['phase'] == 'result')
    assert done['to'] == 10000 and done['stats']['documents'] == 10000 and done['all_input_crc_verified']
    phases = [r for r in events if r['phase'] in ['profile_ingest', 'profile_maintenance']]
    assert len(phases) == 2 and all(r['instrumented'] and r['overlapping_spans_do_not_sum'] for r in phases)
    calls = {}
    for phase in phases:
        for key, counter in phase['counters'].items():
            calls[key] = calls.get(key, 0)+counter['calls']
    output['profiles'][variant] = dict(calls=calls, phases=phases,
        metadata=read_json(BASE / f'{PREFIX}-profile-{variant}.metadata.json'),
        note='Same volume and reusable DB slot; diagnostic spans overlap, never sum as CPU attribution or compare with uninstrumented times')
for mode, suffix in [('default', '-default'), ('isolated', ''), ('all', '-all')]:
    text = (BASE / f'{TEST_PREFIX}-tests{suffix}.log').read_text(encoding='utf-8-sig')
    assert 'test result: FAILED' not in text
    output['tests'][mode] = sum(int(n) for n in re.findall(r'test result: ok\. (\d+) passed', text))
assert rows(BASE / f'{PREFIX}-sqlite-integrity.jsonl')[-1]['ok']
output['storage_policy'] = 'same existing corpus; fixed FDB/SQLite slots; verification before reuse; no per-run DB archives'
Path(BASE / f'{PREFIX}-summary.json').write_text(json.dumps(output, indent=2), encoding='utf-8')
print(json.dumps({v: item['median'] for v, item in output['variants'].items()}, indent=2))
print(json.dumps({v: p['calls'] for v, p in output['profiles'].items()}, indent=2))
