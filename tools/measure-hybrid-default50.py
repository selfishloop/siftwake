"""N60 acceptance of the promoted default; existing slots, no reset/ingest."""
import importlib.util
import json
import os
import shutil
import subprocess
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'hybrid-default50-20261102'
spec = importlib.util.spec_from_file_location('estimator', R / 'tools/measure-estimator50.py')
E = importlib.util.module_from_spec(spec)
spec.loader.exec_module(E)
c, B, m = E.c, E.B, E.m


def main():
    dest = L / f'{P}-design.json'
    assert not dest.exists(), 'Inspect interrupted run; do not silently replay'
    pointer = m.read(m.s.CURRENT)
    assert pointer['sha256'] == B.sha(m.CORPUS) == '881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12'
    assert all(s['documents'] == 50000 and s['status'] == 'ready' for s in pointer['slots'].values())
    programs = dict(fdb=R / f'output/{P}-fdb.exe', sqlite=R / 'output/estimator50-20261031-sqlite.exe')
    assert 'Cell::new(false)' in (R / 'src/engine/first.rs').read_text(encoding='utf-8')
    sources = {str(p.relative_to(R)): B.sha(p) for p in sorted((R / 'src').rglob('*.rs'))}
    d = dict(stage='running', completed=[], initial_slots=pointer,
             programs={k: dict(path=str(p), sha256=B.sha(p)) for k, p in programs.items()},
             corpus_sha256=B.sha(m.CORPUS), protected_before=B.protected(), engine_sources_before=sources,
             pairs=[['before', 'sqlite', 'after'], ['after', 'sqlite', 'before']] * 2,
             role_meaning=dict(before='diagnostic rollback REFERENCE_WAIT=true', after='new default, no policy override', sqlite='same frozen FTS5 trigram N60 reference'),
             idle_samples_per_query=60, loaded_minimum_per_query=60, rounds_per_4R_actor=15,
             estimator='nearest rank ceil(p*N), real samples, no trim/bootstrap',
             gate='idle p95 <=30 ms: loaded p95 <=30 ms; otherwise loaded p95 <=2*idle',
             duration='16384 commits floor, extend until4 readers each complete15 active rounds; hard cap262144',
             cache='warm/uncontrolled OS cache; no eviction/query cache reset', reset=False, ingest=False,
             prototype_default=True, bounded_wait_implemented=False,
             stop_rule='complete this four-pair performance block, report failures, no next task; abort immediately on oracle/process failure',
             note='same existing50k owner slots evolve; no claim of identical physical state across arms/sessions')
    B.write(dest, d)
    life = (L / f'{P}-lifecycle.jsonl').open('x', encoding='utf-8')

    def event(kind, **v):
        life.write(json.dumps(dict(utc=m.utc(), kind=kind, **v)) + '\n')
        life.flush()
        os.fsync(life.fileno())

    def run(role, mode, pair):
        engine = 'sqlite' if role == 'sqlite' else 'fdb'
        ptr = m.read(m.s.CURRENT)
        state = L / f'{P}-pair{pair}-{role}-state.json'
        shutil.copyfile(ptr['slots'][engine]['state'], state)
        shutil.copyfile(state, state.with_name(state.stem + '-before.json'))
        ptr['slots'][engine].update(status='measuring_hybrid_default', state=str(state))
        B.write(m.s.CURRENT, ptr)
        options = c.env()
        for key in ['FDB_HYBRID_WAIT_REFERENCE', 'FDB_QUERY_PROFILE', 'FDB_SEQUENCE_PROFILE',
                    'FDB_SNAPSHOT_CASES', 'FDB_SNAPSHOT_IDLE_ONLY', 'FDB_SNAPSHOT_MASK_REFERENCE']:
            options.pop(key, None)
        if role == 'before':
            options['FDB_HYBRID_WAIT_REFERENCE'] = '1'
        options.update(FDB_THREAD_PROFILE='1', FDB_SNAPSHOT_ROUNDS='15',
                       FDB_SNAPSHOT_MIN_ACTIVE_ROUNDS='15', FDB_SNAPSHOT_MAX_COMMITS='262144',
                       FDB_SNAPSHOT_ESTIMATOR_PROGRESS='1')
        event('arm_policy', pair=pair, role=role, override=options.get('FDB_HYBRID_WAIT_REFERENCE'))
        x = c.run_child(programs[engine], mode, state, options, f'{P}-pair{pair}-{role}', event)
        x.update(role=role, pair=pair, state=str(state), coverage=E.coverage(L / x['log'], pair == -1))
        d['completed'].append(x)
        ptr['slots'][engine].update(status='ready', state=str(state), model_sha256=B.sha(state))
        B.write(m.s.CURRENT, ptr)
        B.write(dest, d)
        print(json.dumps(dict(completed=len(d['completed']), pair=pair, role=role, coverage=x['coverage'])), flush=True)
        subprocess.run([os.sys.executable, str(R / 'tools/report-hybrid-default50.py')], cwd=R, check=True)

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
        assert d['corpus_sha256'] == d['corpus_after']
        assert d['engine_sources_before'] == d['engine_sources_after']
        assert all(B.sha(Path(p['path'])) == p['sha256'] for p in d['programs'].values())
        B.write(dest, d)
        subprocess.run([os.sys.executable, str(R / 'tools/report-hybrid-default50.py')], cwd=R, check=True)
    except BaseException as error:
        d.update(stage='failed', error=repr(error))
        B.write(dest, d)
        event('driver_error', error=repr(error))
        raise
    finally:
        event('driver_finish', stage=d['stage'])
        life.close()


if __name__ == '__main__':
    main()
