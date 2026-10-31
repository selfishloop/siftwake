"""Larger real samples, unchanged latency gates; no engine experiment added."""
import importlib.util
import json
import math
import os
import shutil
import subprocess
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'estimator50-20261031'
spec = importlib.util.spec_from_file_location('c', R / 'tools/measure-revised50-profile.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
m, B = c.m, c.B


def coverage(log, control):
    phases = [json.loads(x) for x in log.read_text(encoding='utf-8').splitlines()]
    load = next(x for x in phases if x['phase'] == 'snapshot_load')
    assert load['oracle'] and load['failure'] is None and load['commits'] >= 16384
    counts = {}
    if not control:
        for phase in ['snapshot_idle_single', 'snapshot_idle_four', 'snapshot_load']:
            row = next(x for x in phases if x['phase'] == phase)
            assert row['oracle']
            n = [0] * 41
            for reader in row['results']:
                assert reader['oracle']
                for q in reader['records']:
                    assert q['oracle']
                    if phase != 'snapshot_load' or q['writer_active']:
                        n[q['case_id']] += 1
            assert min(n) >= 60, (phase, n)
            counts[phase] = n
    return dict(counts=counts, commits=load['commits'], writer_ms=load['writer_elapsed_ms'])


def main():
    dest = L / f'{P}-design.json'
    assert not dest.exists(), 'Inspect an interrupted run; never silently replay it'
    pointer = m.read(m.s.CURRENT)
    assert all(s['documents'] == 50000 and s['status'] == 'ready' for s in pointer['slots'].values())
    programs = {engine: R / f'output/estimator50-20261031-{engine}.exe' for engine in ['fdb', 'sqlite']}
    sources = {str(p.relative_to(R)): B.sha(p) for p in sorted((R / 'src').rglob('*.rs'))}
    assert 'Cell::new(true)' in (R / 'src/engine/first.rs').read_text(encoding='utf-8')
    d = dict(stage='running', completed=[], programs={k: dict(path=str(p), sha256=B.sha(p)) for k, p in programs.items()},
             corpus_sha256=B.sha(m.CORPUS), protected_before=B.protected(), engine_sources_before=sources,
             pairs=[['before', 'sqlite', 'after'], ['after', 'sqlite', 'before']] * 2,
             idle_samples_per_query=60, loaded_minimum_per_query=60, rounds_per_4R_actor=15,
             estimator='nearest rank ceil(p*N), all real samples; p50/p95/max; no trim/bootstrap',
             gate='idle p95 <=30 ms: loaded p95 <=30 ms; otherwise loaded p95 <=2*idle',
             duration='16384 writer commit floor; extend only until all4 readers finish15 complete active rounds; hard cap262144',
             cache='warm/uncontrolled OS cache, no eviction, no query cache reset',
             reset=False, optimization=False, prototype_default=False,
             note='same fixed50k owner slots evolve with writes/maintenance; not identical physical layout; old N12 results retained')
    B.write(dest, d)
    life = (L / f'{P}-lifecycle.jsonl').open('x', encoding='utf-8')

    def event(kind, **values):
        life.write(json.dumps(dict(utc=m.utc(), kind=kind, **values)) + '\n')
        life.flush()
        os.fsync(life.fileno())

    def run(role, mode, pair):
        engine = 'sqlite' if role == 'sqlite' else 'fdb'
        pointer = m.read(m.s.CURRENT)
        state = L / f'{P}-pair{pair}-{role}-state.json'
        shutil.copyfile(pointer['slots'][engine]['state'], state)
        shutil.copyfile(state, state.with_name(state.stem + '-before.json'))
        pointer['slots'][engine].update(status='measuring_estimator', state=str(state))
        B.write(m.s.CURRENT, pointer)
        options = c.env()
        for key in ['FDB_HYBRID_WAIT_REFERENCE', 'FDB_QUERY_PROFILE', 'FDB_SEQUENCE_PROFILE',
                    'FDB_SNAPSHOT_CASES', 'FDB_SNAPSHOT_IDLE_ONLY', 'FDB_SNAPSHOT_MASK_REFERENCE']:
            options.pop(key, None)
        if role == 'before':
            options['FDB_HYBRID_WAIT_REFERENCE'] = '1'
        options.update(FDB_THREAD_PROFILE='1', FDB_SNAPSHOT_ROUNDS='15',
                       FDB_SNAPSHOT_MIN_ACTIVE_ROUNDS='15', FDB_SNAPSHOT_MAX_COMMITS='262144',
                       FDB_SNAPSHOT_ESTIMATOR_PROGRESS='1')
        x = c.run_child(programs[engine], mode, state, options, f'{P}-pair{pair}-{role}', event)
        x.update(role=role, pair=pair, state=str(state), coverage=coverage(L / x['log'], pair == -1))
        d['completed'].append(x)
        pointer['slots'][engine].update(status='ready', state=str(state), model_sha256=B.sha(state))
        B.write(m.s.CURRENT, pointer)
        B.write(dest, d)
        print(json.dumps(dict(completed=len(d['completed']), pair=pair, role=role, coverage=x['coverage'])), flush=True)
        subprocess.run([os.sys.executable, str(R / 'tools/report-estimator50.py')], cwd=R, check=True)

    try:
        event('driver_start', pid=os.getpid())
        for role in ['before', 'sqlite', 'after']:
            run(role, 'snapshot-control-1', -1)
        for pair, order in enumerate(d['pairs']):
            for role in order:
                run(role, 'snapshot-one', pair)
        d.update(stage='completed', protected_after=B.protected(), corpus_after=B.sha(m.CORPUS),
                 engine_sources_after={str(p.relative_to(R)): B.sha(p) for p in sorted((R / 'src').rglob('*.rs'))})
        assert d['protected_before'] == d['protected_after']
        assert d['corpus_sha256'] == d['corpus_after'] == '881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12'
        assert d['engine_sources_before'] == d['engine_sources_after']
        assert all(B.sha(Path(p['path'])) == p['sha256'] for p in d['programs'].values())
        B.write(dest, d)
        subprocess.run([os.sys.executable, str(R / 'tools/report-estimator50.py')], cwd=R, check=True)
    finally:
        event('driver_finish', stage=d['stage'])
        life.close()


if __name__ == '__main__':
    main()
