"""Diagnostic runs; frozen acceptance binary and gates remain separate."""
import importlib.util,json,os,shutil
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='query-attribution50-20261030'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m=c.m;B=c.B
def main():
    dest=L/f'{P}-design.json';assert not dest.exists();assert m.read(L/'idle-spread50-20261030-design.json')['stage']=='completed'
    ptr=m.read(m.s.CURRENT);state=L/f'{P}-state.json';shutil.copyfile(ptr['slots']['fdb']['state'],state);exe=R/'output/query-attribution50-20261030.exe';d=dict(stage='running',program_sha256=B.sha(exe),protected_before=B.protected(),source_sha256=B.sha(m.CORPUS),completed=[],note='diagnostic wall/cpu/cache counter overhead; not acceptance timing; idle matrix same immutable source layout; loaded runs continue writes, not identical physical layout')
    B.write(dest,d);life=(L/f'{P}-lifecycle.jsonl').open('x',encoding='utf-8')
    def event(kind,**v):life.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');life.flush();os.fsync(life.fileno())
    def run(label,e):
        x=c.run_child(exe,'snapshot-one',state,e,P+'-'+label,event);x['label']=label;d['completed'].append(x);B.write(dest,d)
    try:
        event('driver_start',pid=os.getpid());base=c.env();base['FDB_QUERY_PROFILE']='1'
        def live_sources():
            root=R/'data/benchmark-work/fdb';marker=max(root.glob('generation-*.ready'),key=lambda p:int(p.stem.split('-')[1],16));raw=marker.read_bytes();manifest=json.loads(raw[20:-4]);names=[marker.name,f'generation-{manifest["active"]["id"]:016x}.fdb']
            names.extend(f'{"run" if s["kind"]=="Run" else "generation"}-{s["id"]:016x}.fdb' for s in manifest['runs']);return {n:B.sha(root/n) for n in names}
        d['before_idle_live_sources']=live_sources();B.write(dest,d)
        for j,(readers,matrix) in enumerate([(1,'41'),(4,'41'),(4,'few'),(4,'few'),(4,'41'),(1,'41')]):
            e=base.copy();e.update(FDB_SNAPSHOT_IDLE_ONLY='1',FDB_SNAPSHOT_IDLE_READERS=str(readers),FDB_SNAPSHOT_ROUNDS=('20' if readers==1 else '5') if matrix=='41' else '50')
            if matrix=='few':e['FDB_SNAPSHOT_CASES']='16,25,31,33'
            run(f'idle{j}-{readers}R-{matrix}',e)
        d['after_idle_live_sources']=live_sources();assert d['before_idle_live_sources']==d['after_idle_live_sources'];B.write(dest,d)
        for j,matrix in enumerate(['41','few']):
            e=base.copy()
            if matrix=='few':e['FDB_SNAPSHOT_CASES']='16,25,31,33'
            run(f'loaded{j}-4R-{matrix}',e)
            ptr=m.read(m.s.CURRENT);ptr['slots']['fdb'].update(state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,ptr)
        d.update(stage='completed',protected_after=B.protected(),source_after=B.sha(m.CORPUS));assert d['protected_before']==d['protected_after'] and d['source_sha256']==d['source_after'];B.write(dest,d)
    finally:event('driver_finish',stage=d['stage']);life.close()
if __name__=='__main__':main()
