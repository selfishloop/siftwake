"""Frozen same-binary before/after, four matched pairs with SQLite reference."""
import importlib.util,json,os,shutil
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='hybrid-serial50-20261030'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m=c.m;B=c.B
def main():
    dest=L/f'{P}-design.json';assert not dest.exists();ptr=m.read(m.s.CURRENT);assert all(x['documents']==50000 and x['status']=='ready' for x in ptr['slots'].values())
    programs={'fdb':R/'output/hybrid-serial-20261030.exe','sqlite':R/'output/hybrid4r50-20261029-v2-sqlite.exe'}
    d=dict(stage='running',source_sha256=B.sha(m.CORPUS),protected_before=B.protected(),programs={k:dict(path=str(v),sha256=B.sha(v)) for k,v in programs.items()},completed=[],pairs=[['before','sqlite','after'],['after','sqlite','before'],['before','sqlite','after'],['after','sqlite','before']],cache='warm OS, no eviction; same1R12round/4R3round/16k writer acceptance schedule; full41 oracle',reset=False,note='one frozen FDB executable; before selects old blocking handoff; after nonblocking try+existing serial cursor; same slots progress physically with writes/maintenance, not identical layout')
    B.write(dest,d);life=(L/f'{P}-lifecycle.jsonl').open('x',encoding='utf-8')
    def event(kind,**v):life.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');life.flush();os.fsync(life.fileno())
    def run(role,mode,label,pair):
        engine='sqlite' if role=='sqlite' else 'fdb';ptr=m.read(m.s.CURRENT);state=L/f'{P}-{label}-{role}-state.json';shutil.copyfile(ptr['slots'][engine]['state'],state);B.write(L/f'{P}-{label}-{role}-before-model.json',m.read(state))
        e=c.env();e.pop('FDB_HYBRID_WAIT_REFERENCE',None);e['FDB_THREAD_PROFILE']='1'
        if role=='before':e['FDB_HYBRID_WAIT_REFERENCE']='1'
        x=c.run_child(programs[engine],mode,state,e,f'{P}-{label}-{role}',event);x.update(role=role,engine=engine,pair=pair,label=label,state=str(state));d['completed'].append(x)
        ptr['slots'][engine].update(state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,ptr);B.write(dest,d)
    try:
        event('driver_start',pid=os.getpid())
        for role in ['before','sqlite','after']:run(role,'snapshot-control-1','writer-control',-1)
        for pair,order in enumerate(d['pairs']):
            for role in order:run(role,'snapshot-one',f'pair{pair}',pair)
        d.update(stage='completed',protected_after=B.protected(),source_after=B.sha(m.CORPUS));assert d['protected_after']==d['protected_before'] and d['source_sha256']==d['source_after'];B.write(dest,d)
    finally:event('driver_finish',stage=d['stage']);life.close()
if __name__=='__main__':main()
