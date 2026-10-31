"""Three long4R runs with the write trace disabled, same10k, no corpus prep."""
import importlib
import json
import os

b=importlib.import_module('measure-churn')
P=b.PREFIX='wal-qualified-10k-20261027'
b.PROGRAMS['fdb']=b.ROOT/'output/wal-profile-fdb-20261027-v1.exe'


def main():
    target=b.LOGS/f'{P}-design.json';assert not target.exists()
    d=dict(program=str(b.PROGRAMS['fdb']),sha256=b.sha(b.PROGRAMS['fdb']),protected_before=b.protected(),completed=[],
           scope='three long4096single-write4R runs;38case full model oracle;trace disabled;no handle optimization because historical419ms was not reproduced in correlated traces;warmOS,no CPU/power isolation')
    b.write(target,d)
    for flag in ['FDB_SEQUENCE_PROFILE','FDB_RECLAMATION_CONTROL','FDB_BENCH_SNAPSHOTS','FDB_COST_LEGACY','FDB_QUERY_VISIBILITY_PRUNING']:os.environ.pop(flag,None)
    os.environ['FDB_STREAMED_WAL_LAYER']='1';os.environ['FDB_SNAPSHOT_FIXED_COMMITS']='4096';os.environ['FDB_SNAPSHOT_ROUNDS']='2'
    for pair in range(3):
        label=f'r{pair}';state=b.LOGS/f'{P}-{label}-state.json'
        b.reset('fdb');b.write(b.LOGS/f'{P}-slot-fdb.json',dict(state=str(state)))
        for mode in ['seed','snapshot-one','verify']:
            d['completed'].append(b.run('fdb',mode,'streamed',state,label));b.write(target,d)
        rows=[json.loads(s) for s in (b.LOGS/f'{P}-{label}-fdb-streamed-snapshot-one.jsonl').read_text(encoding='utf-8').splitlines()]
        r=next(v for v in rows if v['phase']=='snapshot_load');assert r['commits']==4096 and r['write_profile'] is None and r['oracle']
        slow=[w for w in r['writes'] if w['commit_ms']>50]
        d.setdefault('trials',[]).append(dict(trial=pair,max_ms=max(w['commit_ms'] for w in r['writes']),over50=len(slow),slow=slow))
        b.write(target,d)
    d['gate_pass']=all(t['over50']==0 for t in d['trials'])
    d['protected_after']=b.protected();assert d['protected_before']==d['protected_after'];d['complete']=True;b.write(target,d)
    print(json.dumps(d['trials']),flush=True)


if __name__=='__main__':main()
