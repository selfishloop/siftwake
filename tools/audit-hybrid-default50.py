"""Audit completed default promotion without changing historical evidence."""
import hashlib
import json
import math
import re
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'hybrid-default50-20261102'


def read(path):
    return json.loads(path.read_text(encoding='utf-8'))


def sha(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest().upper()


def tests(name):
    text = (L / f'{P}-{name}.log').read_text(encoding='utf-8')
    found = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', text)
    assert found and all(failed == '0' for _, failed, _ in found)
    for case in ['hybrid_default_and_nested_diagnostic_rollback_are_thread_local',
                 'diagnostic_rollback_still_waits_cancels_and_returns_the_same_page',
                 'exhausted_hybrid_continues_same_frontier_with_no_permit_or_new_worker',
                 'capped_selected_wal_payload_still_checks_crc']:
        assert f'{case} ... ok' in text
    return sum(int(p) for p, _, _ in found), sum(int(i) for _, _, i in found)


def main():
    d, s = [read(L / f'{P}-{name}.json') for name in ['design', 'summary']]
    assert d['stage'] == s['stage'] == 'completed' and len(d['completed']) == 15
    assert len(s['rows']) == 492 and d['prototype_default'] and not d['reset'] and not d['ingest']
    samples = read(L / f'{P}-samples.json')
    assert len(samples) == 1476
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
    assert d['corpus_sha256'] == d['corpus_after'] == sha(Path(d['initial_slots']['source']))
    assert d['protected_before'] == d['protected_after']
    for item in d['protected_after']:
        path = R / item['path']
        assert sha(path) == item['sha256'] and path.stat().st_size == item['bytes']
    for program in d['programs'].values():
        assert sha(Path(program['path'])) == program['sha256']
    prior = read(L / 'estimator50-20261031-design.json')
    assert d['programs']['sqlite']['sha256'] == prior['programs']['sqlite']['sha256']
    recovery = {}
    for stage in ['pre', 'post']:
        r = read(L / f'{P}-{stage}-recovery.json')
        assert len(r['kills']) == 3 and r['old_reader']['oracle'] and r['live_sources_unchanged']
        assert r['protected_before'] == r['protected_after']
        recovery[stage] = dict(kills=3, old_reader_oracle=True, live_sources_unchanged=True)
    events = [json.loads(x) for x in (L / f'{P}-lifecycle.jsonl').read_text(encoding='utf-8').splitlines()]
    exits = [x for x in events if x['kind'] == 'worker_exit']
    assert len(exits) == 15 and all(x['exit_code'] == 0 for x in exits)
    assert events[-1]['kind'] == 'driver_finish' and events[-1]['stage'] == 'completed'
    policies = [x for x in events if x['kind'] == 'arm_policy']
    assert len(policies) == 15
    assert all(x['override'] == ('1' if x['role'] == 'before' else None) for x in policies)
    code = (R / 'src/engine/first.rs').read_text(encoding='utf-8')
    assert 'Cell::new(false)' in code
    default, diagnostic = tests('tests'), tests('diag-tests')
    validation = read(L / f'{P}-validation.json')
    assert all(v == 0 for v in validation['exit_codes'].values())
    out = dict(role_gates=s['role_gates'], rows=492, phase_rows=len(samples),
               minimum_samples=min(x['n'] for x in samples), maximum_samples=max(x['n'] for x in samples),
               all_p95_ranks_below_max_rank=True, numerical_p95_equals_max=sum(x['p95_equals_max'] for x in samples),
               gates_unchanged=True, frozen_engine_sources_equal=True, frozen_programs_equal=True,
               corpus_and_v1_fixture_hashes_equal=True, prototype_default=True, rollback_true_tested=True,
               default_measured_without_override=True, bounded_wait_implemented=False,
               sqlite_same_frozen_estimator_program=True,
               recovery=recovery, all15_children_normal_exit=True,
               tests_passed=default[0], tests_ignored=default[1], diagnostic_tests_passed=diagnostic[0], diagnostic_tests_ignored=diagnostic[1],
               next_tasks_started=False, known_cost_gates='KALDI; SONRADAN REVIZE user accepted known costs; thresholds unchanged')
    (L / f'{P}-final-audit.json').write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(out, ensure_ascii=False))


if __name__ == '__main__':
    main()
