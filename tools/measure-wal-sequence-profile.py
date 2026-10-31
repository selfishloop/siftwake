"""Sequence-correlated trace; separate opt-in wall phases, not global diagnostic spans."""
import argparse
import importlib
import json
import os

b=importlib.import_module('measure-churn')
P='wal-sequence-profile-10k-20261027'


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--attempt',required=True);args=parser.parse_args()
    b.PREFIX=f'{P}-{args.attempt}'
    b.PROGRAMS['fdb']=b.ROOT/'output/wal-profile-fdb-20261027-v1.exe'
    target=b.LOGS/f'{b.PREFIX}-design.json';assert not target.exists()
    audit=json.loads((b.LOGS/'reclaim-idle30-10k-20261027-audit.json').read_text(encoding='utf-8'));assert audit['gate_pass']
    before=b.protected()
    d=dict(protected_before=before,program=str(b.PROGRAMS['fdb']),sha256=b.sha(b.PROGRAMS['fdb']),completed=[],
           scope='profiling only;existing10k;one4R4096single-write run;bounded8192frame capture enabled solely during writer;no global diagnostic-timing feature;warmOS,not CPU/power isolated',
           query='same38case matrix;reader min2rounds continues until writer done;model oracle excluded from query timer but consumes CPU')
    b.write(target,d)
    for flag in ['FDB_RECLAMATION_CONTROL','FDB_BENCH_SNAPSHOTS','FDB_COST_LEGACY','FDB_QUERY_VISIBILITY_PRUNING']:os.environ.pop(flag,None)
    os.environ['FDB_STREAMED_WAL_LAYER']='1';os.environ['FDB_SEQUENCE_PROFILE']='1'
    os.environ['FDB_SNAPSHOT_FIXED_COMMITS']='4096';os.environ['FDB_SNAPSHOT_ROUNDS']='2'
    label='fdb-load';state=b.LOGS/f'{b.PREFIX}-state.json'
    b.reset('fdb');b.write(b.LOGS/f'{P}-slot-fdb.json',dict(state=str(state)))
    for mode in ['seed','snapshot-one','verify']:
        d['completed'].append(b.run('fdb',mode,'streamed',state,label));b.write(target,d)
    rows=[json.loads(s) for s in (b.LOGS/f'{b.PREFIX}-{label}-fdb-streamed-snapshot-one.jsonl').read_text(encoding='utf-8').splitlines()]
    load=next(r for r in rows if r['phase']=='snapshot_load');trace=load['write_profile']
    assert trace['dropped_frames']==0 and len(trace['frames'])==load['commits']==4096
    frames={f['operation_sequence']:f for f in trace['frames']}
    assert len(frames)==4096
    linked=[]
    for i,w in enumerate(load['writes'],1):
        f=frames[w['sequence']]
        assert f['entry_sequence']+1==w['sequence']
        linked.append(dict(ordinal=i,sequence=w['sequence'],commit_ms=w['commit_ms'],profile_ms=f['elapsed_ns']/1e6,phases=f['phases']))
    d['slow_commits']=[r for r in linked if r['commit_ms']>50]
    d['worst']=max(linked,key=lambda r:r['commit_ms'])
    d['wal_opens']=[dict(sequence=r['sequence'],ordinal=r['ordinal'],commit_ms=r['commit_ms'],phase=p) for r in linked for p in r['phases'] if p['name']=='snapshot.wal_open']
    d['protected_after']=b.protected();assert before==d['protected_after'];d['complete']=True;b.write(target,d)
    print(json.dumps(dict(max_ms=d['worst']['commit_ms'],over50=len(d['slow_commits']),worst=d['worst'])),flush=True)


if __name__=='__main__':main()
