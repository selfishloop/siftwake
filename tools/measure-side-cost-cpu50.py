"""Four paired CPU repetitions, real N>=60; no engine changes."""
import importlib.util
import json
import os
import shutil
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'side-cost-cpu50-20261031'
spec = importlib.util.spec_from_file_location('c', R / 'tools/measure-revised50-profile.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
m, B = c.m, c.B


def main():
    dest = L / f'{P}-design.json'
    assert not dest.exists()
    prior = m.read(L / 'estimator50-20261031-design.json')
    assert prior['stage'] == 'completed'
    aux_prior = m.read(L / 'hybrid-serial-extra50-20261030-design.json')
    aux = L / 'hybrid-serial-extra50-20261030-aux-catalog.json'
    assert B.sha(aux) == aux_prior['aux_metadata_sha256']
    normal = R / 'output/estimator50-20261031-fdb.exe'
    diag = R / 'output/side-cost-cpu50-20261031-diag.exe'
    state = L / f'{P}-state.json'
    shutil.copyfile(m.read(m.s.CURRENT)['slots']['fdb']['state'], state)
    sources = {str(p.relative_to(R)): B.sha(p) for p in sorted((R / 'src').rglob('*.rs'))}
    assert sources == prior['engine_sources_after']
    assert 'Cell::new(true)' in (R / 'src/engine/first.rs').read_text()
    d = dict(stage='running', completed=[], source_sha256=B.sha(m.CORPUS),
             protected_before=B.protected(), engine_sources_before=sources,
             normal_sha256=B.sha(normal), diagnostic_sha256=B.sha(diag), aux_sha256=B.sha(aux),
             pairs=[['before', 'after'], ['after', 'before']] * 2,
             scope='normal41 latency/write reuses ESTIMATOR N60 raw data; new auxiliary the-zx normal+diagnostic 1R60/4R60; new full41 diagnostic1R60/4R60/loaded>=60; CPU parent+children GetThreadTimes',
             instrumentation='diagnostic counters have overhead; diagnostic wall p95 not substituted for normal latency; no newCPU thresholds',
             prototype_default=False, bounded_wait=False)
    B.write(dest, d)
    life = (L / f'{P}-lifecycle.jsonl').open('x', encoding='utf-8')

    def event(kind, **v):
        life.write(json.dumps(dict(utc=m.utc(), kind=kind, **v)) + '\n')
        life.flush()
        os.fsync(life.fileno())

    def options(role):
        e = c.env()
        for key in ['FDB_HYBRID_WAIT_REFERENCE', 'FDB_SNAPSHOT_CASES', 'FDB_SNAPSHOT_IDLE_ONLY',
                    'FDB_SNAPSHOT_MASK_REFERENCE', 'FDB_QUERY_PROFILE', 'FDB_SEQUENCE_PROFILE',
                    'FDB_HYBRID_WORKERS', 'FDB_SNAPSHOT_MIN_ACTIVE_ROUNDS']:
            e.pop(key, None)
        if role == 'before':
            e['FDB_HYBRID_WAIT_REFERENCE'] = '1'
        return e

    def run(program, label, role, pair, e):
        x = c.run_child(program, 'snapshot-one', state, e, f'{P}-{label}-p{pair}-{role}', event)
        x.update(label=label, role=role, pair=pair, state_sha256=B.sha(state))
        # Decode only after the timed child exits.
        phases = [json.loads(v) for v in (L / x['log']).read_text(encoding='utf-8').splitlines()]
        counts = {}
        for phase in phases:
            if phase['phase'] not in ['snapshot_idle_probe', 'snapshot_idle_single', 'snapshot_idle_four', 'snapshot_load']:
                continue
            assert phase['oracle']
            n = {}
            for reader in phase['results']:
                for q in reader['records']:
                    assert q['oracle']
                    if phase['phase'] == 'snapshot_load' and not q['writer_active']:
                        continue
                    n[q['case_id']] = n.get(q['case_id'], 0) + 1
                    if program == diag:
                        assert q['query_profile']['threads']
                        assert all(t['cpu_100ns'] is not None for t in q['query_profile']['threads'])
            assert min(n.values()) >= 60
            counts[phase['phase']] = n
        x['counts'] = counts
        d['completed'].append(x)
        if label == 'full41-profile':
            ptr = m.read(m.s.CURRENT)
            ptr['slots']['fdb'].update(state=str(state), model_sha256=B.sha(state), status='ready')
            B.write(m.s.CURRENT, ptr)
        B.write(dest, d)

    def live():
        root = R / 'data/benchmark-work/fdb'
        marker = max(root.glob('generation-*.ready'), key=lambda p: int(p.stem.split('-')[1], 16))
        manifest = json.loads(marker.read_bytes()[20:-4])
        names = [marker.name, f'generation-{manifest["active"]["id"]:016x}.fdb']
        names += [f'{"run" if x["kind"] == "Run" else "generation"}-{x["id"]:016x}.fdb' for x in manifest['runs']]
        return {n: B.sha(root / n) for n in names}

    try:
        event('driver_start', pid=os.getpid())
        d['before_readonly_sources'] = live()
        B.write(dest, d)
        for profiled, program in [(False, normal), (True, diag)]:
            for readers in [1, 4]:
                for pair, order in enumerate(d['pairs']):
                    for role in order:
                        e = options(role)
                        e.update(FDB_BENCH_CORPUS_INDEX=str(aux), FDB_SNAPSHOT_IDLE_ONLY='1',
                                 FDB_SNAPSHOT_IDLE_READERS=str(readers), FDB_SNAPSHOT_CASES='41',
                                 FDB_SNAPSHOT_ROUNDS='60' if readers == 1 else '15')
                        if profiled:
                            e['FDB_QUERY_PROFILE'] = '1'
                        run(program, f'aux-{readers}R-{int(profiled)}', role, pair, e)
        d['after_readonly_sources'] = live()
        assert d['before_readonly_sources'] == d['after_readonly_sources']
        B.write(dest, d)
        for pair, order in enumerate(d['pairs']):
            for role in order:
                e = options(role)
                e.update(FDB_QUERY_PROFILE='1', FDB_SNAPSHOT_ROUNDS='15',
                         FDB_SNAPSHOT_MIN_ACTIVE_ROUNDS='15', FDB_SNAPSHOT_MAX_COMMITS='262144',
                         FDB_SNAPSHOT_ESTIMATOR_PROGRESS='1')
                run(diag, 'full41-profile', role, pair, e)
        d.update(stage='completed', source_after=B.sha(m.CORPUS), protected_after=B.protected(),
                 engine_sources_after={str(p.relative_to(R)): B.sha(p) for p in sorted((R / 'src').rglob('*.rs'))})
        assert d['source_sha256'] == d['source_after'] == prior['corpus_sha256']
        assert d['protected_before'] == d['protected_after']
        assert d['engine_sources_before'] == d['engine_sources_after']
        B.write(dest, d)
    finally:
        event('driver_finish', stage=d['stage'])
        life.close()


if __name__ == '__main__':
    main()
