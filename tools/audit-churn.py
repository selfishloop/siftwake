"""Validate saved evidence only; never runs a workload or mutates a database."""
import csv
import hashlib
import json
import math
import statistics
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOGS = ROOT / 'docs/benchmarks'
PREFIX = 'churn-concurrent-10k-20261024'


def read(name):
    return json.loads((LOGS / name).read_text(encoding='utf-8-sig'))


def quantiles(values):
    ordered = sorted(values)
    return dict(n=len(values), p50=statistics.median(values),
                p95=ordered[math.ceil(len(values) * .95) - 1], maximum=ordered[-1])


def sha(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(chunk)
    return result.hexdigest().upper()


def main():
    design = read(f'{PREFIX}-all-design.json')
    summary = read(f'{PREFIX}-summary.json')
    finalizer = read('churn-finalizer-20261025.json')
    kills = read('churn-kill-10k-20261025-summary.json')
    assert finalizer['status'] == 'complete' and finalizer['main_exit_code'] == 0
    assert all(step['exit_code'] == 0 for step in finalizer['steps'])
    assert len(design['completed']) == 92
    assert all(row['exit_code'] == 0 for row in design['completed'])
    assert len(summary['input_files']) == 96 and not summary['failures']
    assert design['protected_before'] == design['protected_after']
    assert design['protected_unchanged'] and kills['protected_unchanged']
    protected = []
    for row in design['protected_after']:
        path = ROOT / row['path']
        assert path.stat().st_size == row['bytes'] and sha(path) == row['sha256']
        protected.append(row)
    for program in design['programs'].values():
        assert sha(ROOT / program['path']) == program['sha256']
    assert subprocess.run(['git', 'diff', '--quiet', '828d637', '--', 'src', 'Cargo.toml', 'Cargo.lock'],
                          cwd=ROOT, check=False).returncode == 0

    assert len(summary['latency']) == 64
    for group in summary['latency']:
        assert not group['errors'] and len(group['runs']) == 2
        assert {row['run'] for row in group['runs']} == {0, 1}
        assert group['commit']['n'] == {1: 64, 10: 24, 100: 12, 1000: 6}[group['batch']]
        assert quantiles(group['commit_samples_ms']) == group['commit']
        assert quantiles(group['visible_upper_bound_samples_ms']) == group['visibility_upper_bound']
    assert len(summary['churn']) == 40
    for engine in ('fdb', 'sqlite'):
        for run in (0, 1):
            rounds = [r for r in summary['churn'] if r['engine'] == engine and r['run'] == run]
            assert sorted(r['round'] for r in rounds) == list(range(1, 11))
            assert all(not row['errors'] for row in rounds)
    assert len(summary['delta']) == 8 and len(summary['concurrent']) == 8
    for result in summary['concurrent']:
        rows = [json.loads(line) for line in (LOGS / result['file']).read_text(encoding='utf-8').splitlines()]
        raw = next(row for row in rows if row['phase'] == 'concurrent')
        assert raw['oracle'] and len(raw['read_records']) == 380
        for case_id, case in enumerate(result['cases']):
            points = [r for r in raw['read_records'] if r['case_id'] == case_id]
            assert len(points) == 10
            assert quantiles([r['total_ms'] for r in points]) == case['all_reads']
            assert quantiles([r['wait_ms'] for r in points]) == case['read_wait']
            assert case['p95_ratio'] == case['all_reads']['p95'] / case['idle']['p95']
        if result['engine'] == 'fdb':
            assert result['all_cases_active'] and result['writer']['previous_jobs_drained'] > 0
    for name in ('churn-concurrent-20261025.csv', 'churn-burst-search-20261025.csv'):
        with (LOGS / name).open(encoding='utf-8', newline='') as stream:
            assert len(list(csv.DictReader(stream))) == 304
    assert {r['stage'] for r in kills['results']} == {'iterator', 'flush', 'merge', 'cache'}
    for result in kills['results']:
        assert result['coverage_observed']
        assert result['reopen_full_payload_matrix_oracle_exit_code'] == 0
        assert result['pending_old_new_marker_oracle']['all_passed']
        if result['stage'] == 'cache':
            assert result['source_hash_before'] == result['source_hash_after']
    proof = dict(ordinary_worker_processes=92, recovery_verification_processes=4,
                 latency_groups=64, churn_rounds=40, delta_matrices=8, concurrent_runs=8,
                 cases_per_concurrent_run=38, samples_per_concurrent_case=10,
                 all_quantiles_and_wait_ratios_recomputed=True,
                 fdb_merge_completed_while_reader_loop_active=True,
                 all_four_process_kill_windows_observed=True,
                 recovered_full_payload_matrix_and_revision_oracles_passed=True,
                 cache_repair_did_not_change_authoritative_sources=True,
                 live_binary_sha_matches_design=True, protected_files_rehashed=protected,
                 production_source_and_root_cargo_unchanged_from='828d637',
                 checks=finalizer['steps'],
                 scope='Saved normal-engine observations; no new benchmark, power-loss, cold-cache or native SQLite independent-connection claim')
    (LOGS / 'churn-report-proof-20261025.json').write_text(
        json.dumps(proof, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in proof.items() if isinstance(v, (int, bool))}))


if __name__ == '__main__':
    main()
