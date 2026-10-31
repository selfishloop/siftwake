"""Exactly one mask pair at100k; setup emits counts, not ingest/open/RAM metrics."""
import importlib.util,json,os,subprocess
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='mask-only100k-20261030'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m=c.m;B=c.B
def main():
    target=L/f'{P}-result.json';assert not target.exists()
    assert m.read(L/'mask-stage1-20261030-50k-gate.json')['proceed_to_mask100k']
    options=c.env();options['FDB_SCALE_DOCUMENTS']='100000';normal=R/'output/mask-stage1-candidate-20261030.exe';diag=R/'output/mask-stage1-probe-20261030.exe'
    before=B.protected();source=B.sha(m.CORPUS);B.reset('fdb')
    pointer=m.read(m.s.CURRENT);pointer['slots']['fdb']=dict(documents=100000,status='preparing_mask_only',state=None);B.write(m.s.CURRENT,pointer)
    life=L/f'{P}-lifecycle.jsonl';stream=life.open('x',encoding='utf-8')
    def event(kind,**v):stream.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');stream.flush();os.fsync(stream.fileno())
    event('driver_start',pid=os.getpid());stop=L/f'{P}-watch.stop'
    watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(L/f'{P}-events.jsonl'),'-StopPath',str(stop),'-LifecyclePath',str(life)],stdout=(L/f'{P}-watch.stdout').open('x'),stderr=(L/f'{P}-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    done=[]
    try:
        for program,mode in [(normal,'mask-scale-prepare'),(diag,'mask-scale-pair')]:
            log=L/f'{P}-{mode}.jsonl';assert not log.exists()
            with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
                child=subprocess.Popen([str(program),mode,'streamed','unused'],cwd=R,env=options,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
                event('worker_start',pid=child.pid,mode=mode);code=child.wait();event('worker_exit',pid=child.pid,mode=mode,exit_code=code)
            done.append(dict(program_sha256=B.sha(program),mode=mode,exit_code=code,log=log.name));B.write(L/f'{P}-processes.json',done);assert code==0
            print(json.dumps(dict(mode=mode,exit_code=code,log=log.name)),flush=True)
        rows=list(map(json.loads,(L/done[-1]['log']).read_text().splitlines()));assert len(rows)==1 and rows[0]['sample']['bitwise_equal']
        result=dict(measurement=rows[0],source_sha256=B.sha(m.CORPUS),protected_before=before,protected_after=B.protected(),completed=done,scope='one same-state full-vs-incremental mask pair; counts-only setup, no other100k benchmark, no corpus generated')
        assert result['source_sha256']==source and result['protected_after']==before;B.write(target,result)
        pointer=m.read(m.s.CURRENT);pointer['slots']['fdb'].update(status='mask_only_ready_requires_50k_reset_for_next_stage');B.write(m.s.CURRENT,pointer)
    finally:stop.write_text('normal stop');watcher.wait(timeout=20);event('driver_finish');stream.close()
if __name__=='__main__':main()
