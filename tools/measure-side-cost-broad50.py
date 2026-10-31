"""Repeat the original six-query broad-scan mix with real N60, four pairs."""
import importlib.util
import json
import os
import shutil
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'side-cost-broad50-20261031'
spec = importlib.util.spec_from_file_location('c', R / 'tools/measure-revised50-profile.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
m, B = c.m, c.B


def main():
    dest = L / f'{P}-design.json'
    assert not dest.exists()
    prior = m.read(L / 'side-cost-cpu50-20261031-design.json')
    assert prior['stage'] == 'completed'
    normal = R / 'output/estimator50-20261031-fdb.exe'
    diag = R / 'output/side-cost-cpu50-20261031-diag.exe'
    aux = L / 'hybrid-serial-extra50-20261030-aux-catalog.json'
    assert B.sha(aux) == prior['aux_sha256']
    state = L / f'{P}-state.json'
    shutil.copyfile(m.read(m.s.CURRENT)['slots']['fdb']['state'], state)
    d = dict(stage='running', completed=[], source_sha256=B.sha(m.CORPUS), protected_before=B.protected(),
             normal_sha256=B.sha(normal), diagnostic_sha256=B.sha(diag),
             pairs=[['before', 'after'], ['after', 'before']] * 2,
             cases=[34, 35, 36, 37, 38, 41],
             scope='same six-query mix as prior broad scan; normal latency and separate diagnostic CPU; 1R60 and4R15rounds/actor=60; no writes, the-zx info only')
    B.write(dest, d)
    life = (L / f'{P}-lifecycle.jsonl').open('x', encoding='utf-8')

    def event(kind, **v):
        life.write(json.dumps(dict(utc=m.utc(), kind=kind, **v)) + '\n')
        life.flush()
        os.fsync(life.fileno())

    def live():
        root = R / 'data/benchmark-work/fdb'
        marker = max(root.glob('generation-*.ready'), key=lambda p: int(p.stem.split('-')[1], 16))
        manifest = json.loads(marker.read_bytes()[20:-4])
        names = [marker.name, f'generation-{manifest["active"]["id"]:016x}.fdb']
        names += [f'{"run" if x["kind"] == "Run" else "generation"}-{x["id"]:016x}.fdb' for x in manifest['runs']]
        return {n: B.sha(root / n) for n in names}

    try:
        event('driver_start', pid=os.getpid())
        d['live_before'] = live()
        for profiled, program in [(False, normal), (True, diag)]:
            for readers in [1, 4]:
                for pair, order in enumerate(d['pairs']):
                    for role in order:
                        e = c.env()
                        for key in ['FDB_HYBRID_WAIT_REFERENCE', 'FDB_SNAPSHOT_MASK_REFERENCE', 'FDB_QUERY_PROFILE', 'FDB_HYBRID_WORKERS', 'FDB_SEQUENCE_PROFILE']:
                            e.pop(key, None)
                        if role == 'before':
                            e['FDB_HYBRID_WAIT_REFERENCE'] = '1'
                        e.update(FDB_BENCH_CORPUS_INDEX=str(aux), FDB_SNAPSHOT_IDLE_ONLY='1',
                                 FDB_SNAPSHOT_IDLE_READERS=str(readers), FDB_SNAPSHOT_CASES='34,35,36,37,38,41',
                                 FDB_SNAPSHOT_ROUNDS='60' if readers == 1 else '15')
                        if profiled:
                            e['FDB_QUERY_PROFILE'] = '1'
                        x = c.run_child(program, 'snapshot-one', state, e, f'{P}-{int(profiled)}-{readers}R-p{pair}-{role}', event)
                        phases = [json.loads(v) for v in (L / x['log']).read_text(encoding='utf-8').splitlines()]
                        row = next(v for v in phases if v['phase'] == 'snapshot_idle_probe')
                        n = {i: 0 for i in d['cases']}
                        for reader in row['results']:
                            for q in reader['records']:
                                assert q['oracle']
                                n[q['case_id']] += 1
                                if profiled:
                                    assert all(t['cpu_100ns'] is not None for t in q['query_profile']['threads'])
                        assert set(n.values()) == {60}
                        x.update(role=role, pair=pair, readers=readers, profiled=profiled, counts=n)
                        d['completed'].append(x)
                        B.write(dest, d)
        d.update(stage='completed', live_after=live(), source_after=B.sha(m.CORPUS), protected_after=B.protected())
        assert d['live_before'] == d['live_after']
        assert d['source_sha256'] == d['source_after'] == prior['source_sha256']
        assert d['protected_before'] == d['protected_after']
        B.write(dest, d)
    finally:
        event('driver_finish', stage=d['stage'])
        life.close()


if __name__ == '__main__':
    main()
