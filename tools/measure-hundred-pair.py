"""One matched100-write pair per backend, after accepted30round gates."""
import importlib
import json
import os

b=importlib.import_module('measure-churn')
P=b.PREFIX='hundred-pair-10k-20261027'
b.PROGRAMS={'fdb':b.ROOT/'output/wal-profile-fdb-20261027-v1.exe',
            'sqlite':b.ROOT/'output/wal-profile-sqlite-20261027-v1.exe'}


def main():
    audit=json.loads((b.LOGS/'reclaim-idle30-10k-20261027-audit.json').read_text(encoding='utf-8'))
    assert audit['gate_pass']
    target=b.LOGS/f'{P}-design.json';assert not target.exists()
    d=dict(programs={k:dict(path=str(p),sha256=b.sha(p)) for k,p in b.PROGRAMS.items()},protected_before=b.protected(),completed=[],
           order=['FDB control','SQLite load','SQLite control','FDB load'],
           gate='one pair each;FDB loaded/control>=75% and greater than SQLite ratio',
           count='128commits x100 =12800updates in EACH arm/backend; initial1100stable body donors;continuous4readers/38cases until writer finishes;normal/revision interlude',
           cache='warm OS; no eviction/CPU/power isolation; oracle outside engine timer still consumes CPU; reader minimum2rounds FDB/1SQLite',
           scope='streamed primary, product WAL/reclaim defaults unchangedOFF;full oracle both arms;source/WAL/manifest formats unchanged')
    b.write(target,d)
    for flag in ['FDB_RECLAMATION_CONTROL','FDB_BENCH_SNAPSHOTS','FDB_COST_LEGACY','FDB_QUERY_VISIBILITY_PRUNING','FDB_SMALL_OPERATIONS','FDB_SEQUENCE_PROFILE']:os.environ.pop(flag,None)
    os.environ['FDB_STREAMED_WAL_LAYER']='1';os.environ['FDB_SNAPSHOT_FIXED_COMMITS']='128'
    for engine,arm in [('fdb','control'),('sqlite','load'),('sqlite','control'),('fdb','load')]:
        os.environ['FDB_SNAPSHOT_ROUNDS']='2' if engine=='fdb' else '1'
        label=f'{engine}-{arm}';state=b.LOGS/f'{P}-{label}-state.json'
        b.reset(engine);b.write(b.LOGS/f'{P}-slot-{engine}.json',dict(state=str(state),arm=arm))
        for mode in ['seed','snapshot-100' if arm=='load' else 'snapshot-control-100','verify']:
            d['completed'].append(b.run(engine,mode,'streamed',state,label));b.write(target,d)
    d['results']={}
    for engine in ['fdb','sqlite']:
        parts={}
        for arm in ['load','control']:
            mode='snapshot-100' if arm=='load' else 'snapshot-control-100'
            p=b.LOGS/f'{P}-{engine}-{arm}-{engine}-streamed-{mode}.jsonl'
            parts[arm]=next(r for r in map(json.loads,p.read_text(encoding='utf-8').splitlines()) if r['phase']=='snapshot_load')
        assert parts['load']['commits']==parts['control']['commits']==128
        d['results'][engine]=dict(ratio=parts['load']['ops_s']/parts['control']['ops_s'],control_ops_s=parts['control']['ops_s'],loaded_ops_s=parts['load']['ops_s'],oracle=True)
    d['gate_pass']=d['results']['fdb']['ratio']>=.75 and d['results']['fdb']['ratio']>d['results']['sqlite']['ratio']
    d['protected_after']=b.protected();assert d['protected_before']==d['protected_after'];d['complete']=True;b.write(target,d)
    print(json.dumps(d['results']),flush=True)


if __name__=='__main__':main()
