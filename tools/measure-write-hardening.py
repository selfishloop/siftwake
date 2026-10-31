"""Block1 evidence, existing corpus and ownership-checked fixed slots only."""
import importlib
import json
import os
import sys

b = importlib.import_module('measure-churn')
P = b.PREFIX = 'write-hardening-profile-10k-20261027'

PROGRAMS = dict(candidate=b.ROOT / 'output/write-hardening-fdb-20261027-v4.exe',
                control=b.ROOT / 'output/small-commit-fdb-20261026-v7.exe',
                sqlite=b.ROOT / 'output/write-hardening-sqlite-20261027-v4.exe')


def acceptance(section):
    b.PREFIX = f'write-hardening-{section}-v4-10k-20261027'
    target = b.LOGS / f'{b.PREFIX}-design.json'
    assert not target.exists(), 'Never overwrite evidence'
    d = dict(section=section, protected_before=b.protected(), programs={k: dict(path=str(p), sha256=b.sha(p)) for k, p in PROGRAMS.items()}, completed=[],
             corpus='same existing first10000, no corpus preparation; ownership checked fixed slots',
             cache='warm OS; no eviction/background or CPU/power isolation',
             scope='streamed primary; no persistent format changes; full model oracle', orders=[])
    b.write(target, d)

    os.environ['FDB_STREAMED_WAL_LAYER'] = '1'
    for flag in ['FDB_COST_LEGACY', 'FDB_QUERY_VISIBILITY_PRUNING', 'FDB_BENCH_SNAPSHOTS', 'FDB_SMALL_OPERATIONS', 'FDB_RECLAMATION_CONTROL']:
        os.environ.pop(flag, None)
    if section == 'boundary':
        orders = [('control', 'candidate', 'sqlite'), ('sqlite', 'candidate', 'control'), ('candidate', 'control', 'sqlite'), ('sqlite', 'control', 'candidate')]
        jobs = [(f'r{i}-{arm}', arm, ['small-boundary', 'verify']) for i, order in enumerate(orders) for arm in order]
    elif section == 'churn':
        jobs = [(f'r{i}-{arm}', arm, ['write-churn-30', 'verify']) for i, order in enumerate([('candidate', 'sqlite'), ('sqlite', 'candidate')]) for arm in order]
    else:
        raise ValueError(section)
    for label, arm, modes in jobs:
        engine = 'sqlite' if arm == 'sqlite' else 'fdb'
        b.PROGRAMS[engine] = PROGRAMS[arm]
        state = b.LOGS / f'{b.PREFIX}-{label}-state.json'
        b.reset(engine)
        b.write(b.LOGS / f'write-hardening-10k-20261027-slot-{engine}.json', dict(state=str(state), arm=arm))
        d['orders'].append(label)
        for mode in ['seed'] + modes:
            d['completed'].append(b.run(engine, mode, 'streamed', state, label))
            b.write(target, d)
    d['protected_after'] = b.protected()
    assert d['protected_before'] == d['protected_after']
    d['complete'] = True
    b.write(target, d)


def throughput():
    b.PREFIX = 'write-hardening-throughput-v4-10k-20261027'
    target = b.LOGS / f'{b.PREFIX}-design.json'
    assert not target.exists()
    d = dict(protected_before=b.protected(), programs={k: dict(path=str(p), sha256=b.sha(p)) for k,p in PROGRAMS.items() if k!='control'},
             workload='fixed identical commit count per load/control arm, continuous4readers until writer completes; maintenance drained before arm; full oracle outside engine timer still consumes CPU',
             counts='FDB4096 single/128hundred; SQLite512single/64hundred informative reference;4pairs each; reader minimum2rounds FDB/1SQLite, keeps looping under load',
             cache='warmOS; no eviction/background or CPU/power isolation', completed=[], orders=[])
    b.write(target,d)
    os.environ['FDB_STREAMED_WAL_LAYER']='1'
    for flag in ['FDB_RECLAMATION_CONTROL','FDB_BENCH_SNAPSHOTS','FDB_COST_LEGACY','FDB_QUERY_VISIBILITY_PRUNING']:os.environ.pop(flag,None)
    for batch in ['one','100']:
        for pair in range(4):
            for engine in (['fdb','sqlite'] if pair%2==0 else ['sqlite','fdb']):
                b.PROGRAMS[engine]=PROGRAMS['candidate' if engine=='fdb' else 'sqlite']
                count=(4096 if batch=='one' else 128) if engine=='fdb' else (512 if batch=='one' else 64)
                os.environ['FDB_SNAPSHOT_FIXED_COMMITS']=str(count)
                os.environ['FDB_SNAPSHOT_ROUNDS']='2' if engine=='fdb' else '1'
                order=['control','load'] if pair in [0,3] else ['load','control']
                d['orders'].append(dict(engine=engine,batch=batch,pair=pair,order=order,commits=count))
                for arm in order:
                    label=f'{engine}-{batch}-r{pair}-{arm}'
                    state=b.LOGS/f'{b.PREFIX}-{label}-state.json'
                    b.reset(engine)
                    b.write(b.LOGS/f'write-hardening-10k-20261027-slot-{engine}.json',dict(state=str(state),arm=arm))
                    for mode in ['seed',f'snapshot-{batch}' if arm=='load' else f'snapshot-control-{batch}','verify']:
                        d['completed'].append(b.run(engine,mode,'streamed',state,label));b.write(target,d)
    d['protected_after']=b.protected();assert d['protected_before']==d['protected_after'];d['complete']=True;b.write(target,d)


def main():
    if sys.argv[1:] == ['throughput']:
        return throughput()
    if len(sys.argv) == 2 and sys.argv[1] in ['boundary', 'churn']:
        return acceptance(sys.argv[1])
    if sys.argv[1:] == ['smoke']:
        b.PREFIX = 'write-hardening-smoke-v1-10k-20261027'
        b.PROGRAMS['fdb'] = b.ROOT / 'output/write-hardening-fdb-20261027-v1.exe'
        pointer = b.LOGS / 'write-hardening-10k-20261027-slot-fdb.json'
        state = b.ROOT / json.loads(pointer.read_text(encoding='utf-8'))['state']
        before = b.protected()
        results = [b.run('fdb', mode, 'streamed', state, 'same-slot') for mode in ['small-boundary', 'verify']]
        assert before == b.protected()
        b.write(b.LOGS / f'{b.PREFIX}-design.json', dict(completed=results, protected_before=before, protected_after=before, sha256=b.sha(b.PROGRAMS['fdb'])))
        return
    assert sys.argv[1:] == ['profile']
    b.PROGRAMS['fdb'] = b.ROOT / 'output/write-hardening-profile-20261027.exe'
    target = b.LOGS / f'{P}-design.json'
    assert not target.exists(), 'Never overwrite evidence'
    state = b.LOGS / f'{P}-fdb-state.json'
    design = dict(protected_before=b.protected(), program=str(b.PROGRAMS['fdb']),
                  sha256=b.sha(b.PROGRAMS['fdb']), completed=[],
                  scope='diagnostic only, unchanged512-key16MiB threshold; same first10k; warm OS cache; full model oracle; not acceptance timing')
    b.write(target, design)
    b.reset('fdb')
    b.write(b.LOGS / 'write-hardening-10k-20261027-slot-fdb.json', dict(state=str(state)))
    for mode in ['seed', 'small-boundary', 'verify']:
        design['completed'].append(b.run('fdb', mode, 'streamed', state, 'baseline'))
        b.write(target, design)
    design['protected_after'] = b.protected()
    assert design['protected_before'] == design['protected_after']
    design['complete'] = True
    b.write(target, design)
    data = [json.loads(line) for line in (b.LOGS / f'{P}-baseline-fdb-streamed-small-boundary.jsonl').read_text(encoding='utf-8').splitlines()]
    records = next(r['records'] for r in data if r['phase'] == 'small-boundary')
    ordinal, record = max(enumerate(records, 1), key=lambda p: p[1]['commit_ms'])
    print(json.dumps(dict(ordinal=ordinal, record=record), ensure_ascii=False), flush=True)


if __name__ == '__main__':
    main()
