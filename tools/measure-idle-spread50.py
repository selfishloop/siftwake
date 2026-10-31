"""Four counterbalanced engine pairs, existing50k slots; never rebuild DBs."""
import importlib.util,json,os,shutil
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='idle-spread50-20261030'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m=c.m;B=c.B
def main():
    dest=L/f'{P}-design.json';assert not dest.exists();ptr=m.read(m.s.CURRENT)
    assert all(s['documents']==50000 and s['status']=='ready' for s in ptr['slots'].values())
    programs={'fdb':R/'output/delta-cap-20261030.exe','sqlite':R/'output/hybrid4r50-20261029-v2-sqlite.exe'}
    d=dict(stage='running',pairs=[['fdb','sqlite'],['sqlite','fdb'],['fdb','sqlite'],['sqlite','fdb']],completed=[],programs={k:dict(path=str(v),sha256=B.sha(v)) for k,v in programs.items()},source_sha256=B.sha(m.CORPUS),protected_before=B.protected(),same_db_no_reset=True,cache='warm OS, normal acceptance idle1R then idle4R then writer16k; all41 oracle',note='DBs reused; native maintenance/updates continue; physical layout and sequence recorded per arm; no pristine-layout equivalence claim')
    B.write(dest,d);life=(L/f'{P}-lifecycle.jsonl').open('x',encoding='utf-8')
    def event(kind,**v):life.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');life.flush();os.fsync(life.fileno())
    try:
        event('driver_start',pid=os.getpid());e=c.env()
        for pair,order in enumerate(d['pairs']):
            for engine in order:
                ptr=m.read(m.s.CURRENT);state=L/f'{P}-pair{pair}-{engine}-state.json';shutil.copyfile(ptr['slots'][engine]['state'],state)
                B.write(L/f'{P}-pair{pair}-{engine}-before-model.json',m.read(state))
                x=c.run_child(programs[engine],'snapshot-one',state,e,f'{P}-pair{pair}-{engine}',event);x.update(pair=pair,engine=engine,state=str(state));d['completed'].append(x)
                ptr['slots'][engine].update(state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,ptr);B.write(dest,d)
        d.update(stage='completed',source_after=B.sha(m.CORPUS),protected_after=B.protected());assert d['source_after']==d['source_sha256'] and d['protected_before']==d['protected_after'];B.write(dest,d)
    finally:event('driver_finish',stage=d['stage']);life.close()
if __name__=='__main__':main()
