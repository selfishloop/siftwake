"""Auxiliary the-zx oracle, process CPU blocks and query-local4R CPU profiles."""
import importlib.util,json,os,shutil,zlib
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='hybrid-serial-extra50-20261030'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m=c.m;B=c.B
def main():
    dest=L/f'{P}-design.json';assert not dest.exists();assert m.read(L/'hybrid-serial50-20261030-design.json')['stage']=='completed'
    normal=R/'output/hybrid-serial-cpu-20261030.exe';diag=R/'output/hybrid-serial-diag-20261030.exe';state=L/f'{P}-state.json';shutil.copyfile(m.read(m.s.CURRENT)['slots']['fdb']['state'],state)
    catalog=m.read(m.CATALOG);catalog['entries']=catalog['entries'][:50000];catalog['cases'].append(dict(text='the zx',offset=0,limit=50,base=41));fold=lambda s:s.translate(str.maketrans({'I':'i','İ':'i','ı':'i','Ş':'s','ş':'s','Ğ':'g','ğ':'g','Ç':'c','ç':'c','Ö':'o','ö':'o','Ü':'u','ü':'u'})).lower();df=0
    with m.CORPUS.open('rb') as source:
        for e in catalog['entries']:
            source.seek(e['offset']);raw=source.read(e['size']);assert zlib.crc32(raw)==e['crc'];doc=json.loads(raw)['document'];text=fold(doc['title'])+'\n'+fold(doc['text'])
            if 'the' in text and 'zx' in text:e['mask']|=1<<41;df+=1
    aux=L/f'{P}-aux-catalog.json';B.write(aux,catalog)
    d=dict(stage='running',completed=[],source_sha256=B.sha(m.CORPUS),protected_before=B.protected(),aux_df=df,aux_metadata_sha256=B.sha(aux),aux_gate=False,normal_sha256=B.sha(normal),diagnostic_sha256=B.sha(diag),cpu='process GetProcessTimes blocks, parent+temporary workers; helper sampler included, no oracle in CPU interval; query-local loaded CPU separately coarse',state=str(state))
    B.write(dest,d);life=(L/f'{P}-lifecycle.jsonl').open('x',encoding='utf-8')
    def event(kind,**v):life.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');life.flush();os.fsync(life.fileno())
    def options(role):
        e=c.env();e.pop('FDB_HYBRID_WAIT_REFERENCE',None)
        if role=='before':e['FDB_HYBRID_WAIT_REFERENCE']='1'
        return e
    def run(program,mode,label,role,e):
        x=c.run_child(program,mode,state,e,P+'-'+label+'-'+role,event);x.update(role=role,label=label);d['completed'].append(x);B.write(dest,d)
    def live():
        root=R/'data/benchmark-work/fdb';marker=max(root.glob('generation-*.ready'),key=lambda p:int(p.stem.split('-')[1],16));manifest=json.loads(marker.read_bytes()[20:-4]);names=[marker.name,f'generation-{manifest["active"]["id"]:016x}.fdb'];names.extend(f'{"run" if x["kind"]=="Run" else "generation"}-{x["id"]:016x}.fdb' for x in manifest['runs']);return {n:B.sha(root/n) for n in names}
    try:
        event('driver_start',pid=os.getpid());d['before_readonly_sources']=live();B.write(dest,d)
        for readers in [1,4]:
            for j,role in enumerate(['before','after','after','before']):
                e=options(role);e.update(FDB_BENCH_CORPUS_INDEX=str(aux),FDB_SNAPSHOT_IDLE_ONLY='1',FDB_SNAPSHOT_IDLE_READERS=str(readers),FDB_SNAPSHOT_ROUNDS='10',FDB_SNAPSHOT_CASES='34,35,36,37,38,41')
                run(normal,'snapshot-one',f'aux-{readers}R-{j}',role,e)
        for j,role in enumerate(['before','after','after','before']):
            for group,ids,repeats in [('fast',list(range(11))+[35,36,37,39,40],200),('slow',list(range(11,35))+[38,41],5)]:
                e=options(role);e.update(FDB_BENCH_CORPUS_INDEX=str(aux),FDB_QUERY_CASES=','.join(map(str,ids)),FDB_QUERY_VARIANTS='cap-hybrid',FDB_CPU_BLOCK_REPEATS=str(repeats))
                run(normal,'scale-query-cpu-blocks',f'cpu-{j}-{group}',role,e)
        d['after_readonly_sources']=live();assert d['before_readonly_sources']==d['after_readonly_sources'];B.write(dest,d)
        for j,role in enumerate(['before','after','after','before']):
            e=options(role);e['FDB_QUERY_PROFILE']='1';run(diag,'snapshot-one',f'loaded-profile-{j}',role,e)
            ptr=m.read(m.s.CURRENT);ptr['slots']['fdb'].update(state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,ptr)
        d.update(stage='completed',protected_after=B.protected(),source_after=B.sha(m.CORPUS));assert d['protected_before']==d['protected_after'] and d['source_sha256']==d['source_after'];B.write(dest,d)
    finally:event('driver_finish',stage=d['stage']);life.close()
if __name__=='__main__':main()
