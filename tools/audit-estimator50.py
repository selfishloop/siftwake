"""Run after all timed arms and witnessed recovery; never while timing."""
import hashlib
import json
import math
import re
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'estimator50-20261031'


def read(path):
    return json.loads(path.read_text(encoding='utf-8'))


def sha(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest().upper()


def main():
    d = read(L / f'{P}-design.json')
    s = read(L / f'{P}-summary.json')
    assert d['stage'] == s['stage'] == 'completed'
    assert len(d['completed']) == 15 and len(s['rows']) == 492
    samples = read(L / f'{P}-samples.json')
    assert len(samples) == 12 * 41 * 3
    for x in samples:
        assert x['n'] == len(x['values']) >= 60
        assert x['p95_rank'] == math.ceil(.95 * x['n']) < x['n']
        assert x['p95'] == sorted(x['values'])[x['p95_rank'] - 1]
        assert x['max'] == max(x['values'])
    for x in s['rows']:
        limit = 30 if x['idle_p95'] <= 30 else 2 * x['idle_p95']
        assert x['limit'] == limit and x['passed'] == (x['loaded_p95'] <= limit)
    sources = {str(p.relative_to(R)): sha(p) for p in sorted((R / 'src').rglob('*.rs'))}
    assert sources == d['engine_sources_before'] == d['engine_sources_after']
    assert d['corpus_sha256'] == d['corpus_after'] == sha(Path('D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench'))
    assert d['protected_before'] == d['protected_after']
    for f in d['protected_after']:
        assert sha(R / f['path']) == f['sha256'] and (R / f['path']).stat().st_size == f['bytes']
    for p in d['programs'].values():
        assert sha(Path(p['path'])) == p['sha256']
    recovery = read(L / f'{P}-recovery.json')
    assert len(recovery['kills']) == 3 and recovery['old_reader']['oracle']
    assert recovery['live_sources_unchanged']
    assert recovery['protected_before'] == recovery['protected_after']
    text = (L / f'{P}-tests.log').read_text(encoding='utf-8')
    tests = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', text)
    assert tests and all(f == '0' for _, f, _ in tests)
    events = [json.loads(x) for x in (L / f'{P}-lifecycle.jsonl').read_text().splitlines()]
    exits = [x for x in events if x['kind'] == 'worker_exit']
    assert len(exits) == 15 and all(x['exit_code'] == 0 for x in exits)
    assert events[-1]['kind'] == 'driver_finish' and events[-1]['stage'] == 'completed'
    assert 'Cell::new(true)' in (R / 'src/engine/first.rs').read_text(encoding='utf-8')
    out = dict(role_gates=s['role_gates'], rows=492, phase_rows=len(samples),
               minimum_samples=min(x['n'] for x in samples), maximum_samples=max(x['n'] for x in samples),
               all_p95_ranks_below_max_rank=True, numerical_p95_equals_max=sum(x['p95_equals_max'] for x in samples),
               gates_unchanged=True, engine_sources_unchanged=True, frozen_programs_equal=True,
               corpus_and_v1_fixture_hashes_equal=True, prototype_default=False,
               bounded_wait_implemented=False, kills=3, old_reader_oracle=True,
               live_sources_unchanged=True, all15_children_normal_exit=True,
               tests_passed=sum(int(p) for p, _, _ in tests), tests_ignored=sum(int(i) for _, _, i in tests))
    (L / f'{P}-final-audit.json').write_text(json.dumps(out, indent=2), encoding='utf-8')
    print(json.dumps(out))


if __name__ == '__main__':
    main()
