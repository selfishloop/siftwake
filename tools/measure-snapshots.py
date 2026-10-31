"""Fixed-slot snapshot experiment; no corpus preparation or retained DB copies."""
import argparse
import importlib
import json
import os
from pathlib import Path

b = importlib.import_module('measure-churn')
P = 'snapshot-10k-20261026'
b.PREFIX = P
b.PROGRAMS = {'fdb': b.ROOT / 'output/snapshot-fdb-20261026-v2.exe',
              'sqlite': b.ROOT / 'output/snapshot-sqlite-20261026-v2.exe'}


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--engine', choices=['fdb', 'sqlite'], required=True)
    p.add_argument('--batch', choices=['one', '100'], required=True)
    p.add_argument('--pair', type=int, choices=[0, 1], required=True)
    args = p.parse_args()
    engine, batch, pair = args.engine, args.batch, args.pair
    manifest = b.LOGS / f'{P}-{engine}-{batch}-r{pair}-design.json'
    assert not manifest.exists(), 'Evidence must not be overwritten'
    for flag in ['FDB_COST_LEGACY', 'FDB_QUERY_VISIBILITY_PRUNING',
                 'FDB_RECLAMATION_CONTROL', 'FDB_BENCH_SNAPSHOTS']:
        os.environ.pop(flag, None)
    os.environ['FDB_SNAPSHOT_ROUNDS'] = '5' if engine == 'fdb' else '1'
    before = b.protected()
    design = dict(engine=engine, batch=batch, pair=pair, protected_before=before,
                  program=str(b.PROGRAMS[engine].relative_to(b.ROOT)), sha256=b.sha(b.PROGRAMS[engine]),
                  corpus='existing first10000 only; fixed slots, 1100 stable body donors updated in place',
                  cache='warm OS; strict reopen; no eviction/background or power isolation',
                  readers='4 threads, one snapshot per query, independent oracle outside engine timer',
                  baselines='single idle and four-reader idle; both shown, not silently interchangeable',
                  samples_per_case=20 if engine == 'fdb' else 4,
                  sqlite_limit='4 samples/case/phase/trial, p95 is max; informative reference',
                  order=['load', 'control'] if pair == 0 else ['control', 'load'], completed=[])
    b.write(manifest, design)
    target = 9
    if pair == 1:
        prior = b.LOGS / f'{P}-{engine}-{batch}-r0-load-{engine}-streamed-snapshot-{batch}.jsonl'
        rows = [json.loads(x) for x in prior.read_text(encoding='utf-8').splitlines()]
        target = next(r['commits'] for r in rows if r['phase'] == 'snapshot_load')
    for arm in design['order']:
        label = f'{engine}-{batch}-r{pair}-{arm}'
        state = b.LOGS / f'{P}-{label}-state.json'
        b.reset(engine)
        b.write(b.LOGS / f'{P}-slot-{engine}.json', dict(state=str(state), label=label))
        design['completed'].append(b.run(engine, 'seed', 'streamed', state, label))
        os.environ['FDB_SNAPSHOT_CONTROL_COMMITS'] = str(target)
        mode = f'snapshot-{batch}' if arm == 'load' else f'snapshot-control-{batch}'
        result = b.run(engine, mode, 'streamed', state, label)
        design['completed'].append(result)
        rows = [json.loads(x) for x in (b.LOGS / result['log']).read_text(encoding='utf-8').splitlines()]
        load = next(r for r in rows if r['phase'] == 'snapshot_load')
        if arm == 'load':
            target = load['commits']
        design.setdefault('arms', {})[arm] = dict(commits=load['commits'], ops_s=load['ops_s'],
                                                  maintenance=load['maintenance'], oracle=load['oracle'])
        design['completed'].append(b.run(engine, 'verify', 'streamed', state, label))
        b.write(manifest, design)
    design['protected_after'] = b.protected()
    assert before == design['protected_after']
    design['complete'] = True
    b.write(manifest, design)


if __name__ == '__main__':
    main()
