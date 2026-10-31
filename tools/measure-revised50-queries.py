"""Same50k narrow query traces and counterbalanced1/2/4-worker diagnostics."""
import importlib.util
import json
import os
import subprocess
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='write50-revised-20261029-v1-queries'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m=c.m;B=c.B

def main():
    dest=L/f'{P}-design.json';assert not dest.exists()
    normal=R/'output/write50-revised-20261029-v1-normal.exe';diag=R/'output/write50-revised-20261029-v1-query.exe'
    state=Path(m.read(m.s.CURRENT)['slots']['fdb']['state']);d=dict(stage='narrow',state=str(state),programs={str(p.relative_to(R)):B.sha(p) for p in [normal,diag]},source_sha256=B.sha(m.CORPUS),protected_before=B.protected(),completed=[],optimization=False,worker_limit='diagnostic thread-local ceiling; process-wide four permits unchanged',logical_processors=os.cpu_count(),thread_count='ToolHelp snapshot every50ms; sampled peak is a lower bound')
    B.write(dest,d);life=L/f'{P}-lifecycle.jsonl';stream=life.open('x',encoding='utf-8')
    def event(kind,**v):stream.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');stream.flush();os.fsync(stream.fileno())
    stop=L/f'{P}-watch.stop';watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(L/f'{P}-events.jsonl'),'-StopPath',str(stop),'-LifecyclePath',str(life)],stdout=(L/f'{P}-watch.stdout').open('x'),stderr=(L/f'{P}-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    def run(program,mode,options,label):
        record=c.run_child(program,mode,state,options,P+'-'+label,event);record['label']=label;d['completed'].append(record);B.write(dest,d)
        for x in map(json.loads,(L/record['log']).read_text().splitlines()):
            if x.get('phase') in ['snapshot_load','snapshot_idle_probe']:
                assert x['oracle']
                for reader in x['results']:
                    for q in reader['records']:
                        assert q['oracle']
                        if q['query_profile'] is not None:assert q['query_profile']['counters']['first.total']['calls']==1
    try:
        event('driver_start',pid=os.getpid());options=c.env();options.update(FDB_SNAPSHOT_CASES='0,3,39,40',FDB_SNAPSHOT_ROUNDS='8',FDB_SNAPSHOT_FIXED_COMMITS='4096',FDB_SNAPSHOT_MAX_COMMITS='4096')
        # Same donor body/query sets; each mode performs its checked setup+maintenance.
        for label,program,profile in [('normal0',normal,False),('trace0',diag,True),('trace1',diag,True),('normal1',normal,False)]:
            e=options.copy()
            if profile:e['FDB_QUERY_PROFILE']='1'
            run(program,'snapshot-one',e,label)
        # Prepare once, then all idle measurements reuse exactly this layout/model.
        e=c.env();e['FDB_SNAPSHOT_FIXED_COMMITS']='0';run(normal,'snapshot-control-1',e,'idle-clean')
        model_before=B.sha(state);physical_before={p.name:B.sha(p) for p in (R/'data/benchmark-work/fdb').iterdir() if p.suffix in ['.fdb','.ready']}
        e=c.env();e.update(FDB_SNAPSHOT_IDLE_ONLY='1',FDB_SNAPSHOT_CASES='25,31,33,32',FDB_SNAPSHOT_ROUNDS='8',FDB_QUERY_PROFILE='1')
        sequence=[(4,1),(4,4),(2,4),(2,1),(1,1),(1,4)]
        for ordinal,(workers,readers) in enumerate(sequence+list(reversed(sequence))):
            z=e.copy();z.update(FDB_HYBRID_WORKERS=str(workers),FDB_SNAPSHOT_IDLE_READERS=str(readers))
            run(diag,'snapshot-idle-only',z,f'idle{ordinal}-w{workers}-r{readers}')
        assert B.sha(state)==model_before
        physical_after={p.name:B.sha(p) for p in (R/'data/benchmark-work/fdb').iterdir() if p.suffix in ['.fdb','.ready']}
        # Strict open may remove only pre-existing obsolete sources, not modify retained files.
        assert all(physical_before[k]==v for k,v in physical_after.items())
        d.update(stage='completed',model_unchanged_in_idle=True,physical_unchanged_in_idle=physical_before==physical_after,source_after=B.sha(m.CORPUS),protected_after=B.protected());assert d['source_after']==d['source_sha256'] and d['protected_before']==d['protected_after'];B.write(dest,d)
        pointer=m.read(m.s.CURRENT);pointer['slots']['fdb'].update(state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,pointer)
    finally:stop.write_text('normal stop');watcher.wait(timeout=20);event('driver_finish',stage=d['stage']);stream.close()

if __name__=='__main__':main()
