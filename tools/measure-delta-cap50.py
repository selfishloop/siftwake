"""Delta-only first-page ABBA on one dirty50k layout; not a mask gate."""
import importlib.util,json,os,subprocess
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='delta-cap50-20261030'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m=c.m;B=c.B
def main():
    dest=L/f'{P}-design.json';assert not dest.exists()
    programs={'control':R/'output/mask-stage1-same-binary-20261030.exe','candidate':R/'output/delta-cap-20261030.exe'}
    d=dict(stage='setup',source_sha256=B.sha(m.CORPUS),protected_before=B.protected(),programs={k:dict(path=str(p),sha256=B.sha(p)) for k,p in programs.items()},completed=[],cache='warm OS; no eviction',scope='delta optimization only; same dirty500 layout; not a mask read-only acceptance run',reset_reason='fixed slot held100k mask-only; rebuild authorized first50k, no copies/corpus generation')
    B.write(dest,d);state=L/f'{P}-state.json';life=(L/f'{P}-lifecycle.jsonl').open('x',encoding='utf-8')
    def event(kind,**v):life.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');life.flush();os.fsync(life.fileno())
    def run(role,mode,e,label):
        x=c.run_child(programs[role],mode,state,e,P+'-'+label,event);x.update(role=role,label=label);d['completed'].append(x);B.write(dest,d);return x
    try:
        event('driver_start',pid=os.getpid());e=c.env();B.reset('fdb');se=e.copy();se['FDB_SEED_IDLE_CLEANUP']='1';run('candidate','scale-seed',se,'seed');run('candidate','write50-dirty',e,'dirty500')
        ptr=m.read(m.s.CURRENT);ptr['slots']['fdb']=dict(documents=50000,status='ready',state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,ptr)
        for matrix in ['four','all41']:
            d['stage']=matrix;B.write(dest,d)
            for readers in [1,4]:
                for i,role in enumerate(['control','candidate','candidate','control']):
                    q=e.copy();q.update(FDB_SNAPSHOT_IDLE_ONLY='1',FDB_SNAPSHOT_IDLE_READERS=str(readers),FDB_SNAPSHOT_ROUNDS='5')
                    if matrix=='four':q['FDB_SNAPSHOT_CASES']='0,3,39,40'
                    x=run(role,'snapshot-one',q,f'{matrix}-{readers}R-{i}-{role}')
                    rows=[json.loads(v) for v in (L/x['log']).read_text(encoding='utf-8').splitlines()];probe=next(v for v in rows if v['phase']=='snapshot_idle_probe');assert probe['oracle']
        d.update(stage='completed',source_after=B.sha(m.CORPUS),protected_after=B.protected());assert d['source_after']==d['source_sha256'] and d['protected_before']==d['protected_after'];B.write(dest,d)
    finally:event('driver_finish',stage=d['stage']);life.close()
if __name__=='__main__':main()
