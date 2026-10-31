"""Resume idle-only worker experiment; never repeat completed loaded profiles."""
import importlib.util
import json
import os
import subprocess
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='write50-revised-20261029-v1-idle'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);B=c.B;m=c.m

def main():
    dest=L/f'{P}-design.json';assert not dest.exists()
    state=Path(m.read(m.s.CURRENT)['slots']['fdb']['state']);program=R/'output/write50-revised-20261029-v1-query.exe'
    before=lambda:{f.name:B.sha(f) for f in (R/'data/benchmark-work/fdb').iterdir() if f.suffix in ['.fdb','.ready']}
    d=dict(stage='running',state=str(state),model_sha256=B.sha(state),sources_before=before(),program_sha256=B.sha(program),completed=[],logical_processors=os.cpu_count(),optimization=False)
    B.write(dest,d);life=L/f'{P}-lifecycle.jsonl';stream=life.open('x',encoding='utf-8')
    def event(kind,**v):stream.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');stream.flush();os.fsync(stream.fileno())
    stop=L/f'{P}-watch.stop';watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(L/f'{P}-events.jsonl'),'-StopPath',str(stop),'-LifecyclePath',str(life)],stdout=(L/f'{P}-watch.stdout').open('x'),stderr=(L/f'{P}-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    try:
        event('driver_start',pid=os.getpid());e=c.env();e.update(FDB_SNAPSHOT_IDLE_ONLY='1',FDB_SNAPSHOT_CASES='25,31,33,32',FDB_SNAPSHOT_ROUNDS='8',FDB_QUERY_PROFILE='1')
        sequence=[(4,1),(4,4),(2,4),(2,1),(1,1),(1,4)]
        for i,(w,r) in enumerate(sequence+list(reversed(sequence))):
            z=e.copy();z.update(FDB_HYBRID_WORKERS=str(w),FDB_SNAPSHOT_IDLE_READERS=str(r))
            result=c.run_child(program,'snapshot-one',state,z,P+f'-arm{i}-w{w}-r{r}',event);result.update(workers=w,readers=r);d['completed'].append(result);B.write(dest,d)
            x=next(x for x in map(json.loads,(L/result['log']).read_text().splitlines()) if x['phase']=='snapshot_idle_probe')
            assert x['oracle'] and all(v['oracle'] and v['query_profile']['counters']['first.total']['calls']==1 for rr in x['results'] for v in rr['records'])
        assert B.sha(state)==d['model_sha256'];d.update(stage='completed',sources_after=before());assert d['sources_after']==d['sources_before'];B.write(dest,d)
    finally:stop.write_text('normal stop');watcher.wait(timeout=20);event('driver_finish',stage=d['stage']);stream.close()

if __name__=='__main__':main()
