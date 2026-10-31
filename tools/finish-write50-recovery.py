"""Recover the observer attempt without rerunning completed profiles."""
import importlib.util
import json
import os
import subprocess
import sys
import time
from pathlib import Path

R=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('measure',R/'tools/measure-write50.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
L=m.L;P=m.P;B=m.B

def main():
    d=m.read(L/f'{P}-design.json');assert d['stage']=='STOP_single_failed'
    result_path=L/f'{P}-postgate.json';assert not result_path.exists()
    state=L/f'{P}-single-diagnostic-state.json';assert B.sha(state)==B.sha(Path(d['states']['fdb']))
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_BENCH_CORPUS_PATH=str(m.CORPUS),FDB_BENCH_CORPUS_INDEX=str(m.CATALOG),FDB_BENCH_ORIGINAL_BODIES='1',FDB_BENCH_50K_WRITE='1',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',FDB_SCALE_DOCUMENTS='50000',FDB_FIRST_QUERY='olduğu',FDB_FIRST_VARIANT='cap-hybrid')
    life=L/f'{P}-recovery-lifecycle.jsonl';stream=life.open('x',encoding='utf-8')
    def event(kind,**v):stream.write(json.dumps(dict(utc=m.utc(),kind=kind,**v),ensure_ascii=False)+'\n');stream.flush();os.fsync(stream.fileno())
    stop=L/f'{P}-recovery-watch.stop'
    watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(L/f'{P}-recovery-process-events.jsonl'),'-StopPath',str(stop),'-LifecyclePath',str(life)],cwd=R,stdout=(L/f'{P}-recovery-watch.stdout').open('x'),stderr=(L/f'{P}-recovery-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    event('driver_start',pid=os.getpid(),parent_pid=os.getppid());kills=[]
    sources=lambda:{p.name:B.sha(p) for p in (R/'data/benchmark-work/fdb').iterdir() if p.suffix in ('.fdb','.ready')}
    before=sources()
    try:
        for i in range(3):
            with (L/f'{P}-dirty-open-kill{i}.stderr').open('x',encoding='utf-8') as err:
                child=subprocess.Popen([str(m.PROGRAMS['fdb']),'write50-open','streamed',str(state)],cwd=R,env=env,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW)
                event('worker_start',pid=child.pid,label=f'dirty-open-kill{i}')
                line=child.stdout.readline()
                if not line:event('worker_exit',pid=child.pid,exit_code=child.wait());raise RuntimeError('strict open did not return witness')
                witness=json.loads(line);assert witness['phase']=='opened' and witness['stats']['delta_keys']>0
                event('intentional_kill',pid=child.pid,actor_pid=os.getpid(),witness=witness);child.kill();code=child.wait(timeout=10);event('worker_exit',pid=child.pid,exit_code=code,label=f'dirty-open-kill{i}');assert code!=0
                kills.append(dict(witness=witness,exit_code=code,intentional=True,phase='strict-open completed, live dirty WAL, before oracle; no append-in-progress witness'))
        log=L/f'{P}-recovery-first-strict.jsonl';at=time.monotonic()
        with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
            child=subprocess.Popen([str(m.PROGRAMS['fdb']),'first-strict','streamed',str(state)],cwd=R,env=env,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
            event('worker_start',pid=child.pid,label='reopen');code=child.wait();event('worker_exit',pid=child.pid,exit_code=code,label='reopen');assert code==0
        reopened=dict(log=log.name,exit_code=code,wall_s=time.monotonic()-at)
        assert any(json.loads(v).get('phase')=='first_result_oracle' for v in log.read_text(encoding='utf-8').splitlines())
        sys.path.insert(0,str(R/'tools'));import importlib
        old=importlib.import_module('kill-block2-scale');old.P=P+'-compat';os.environ.update({k:v for k,v in env.items() if k.startswith('FDB_')})
        event('compat_begin');compatible=old.old_reader(state);event('compat_end',oracle=compatible['oracle']);assert before==sources()
        profiles=[]
        for f in L.glob(f'{P}-single-diagnostic*-fdb-*.process.json'):profiles.append(m.read(f))
        value=dict(completed=profiles,model_sha256=B.sha(state),same_logical_workload=True,kills=kills,reopen=reopened,old_reader=compatible,
            observer_attempt_error=dict(native_reason='background opening prototype requires a clean active WAL',driver_error='JSONDecodeError after empty stdout; harness corrected to strict dirty-compatible mode',raw_stderr=f'{P}-pending-kill.stderr'),
            protected_after=B.protected(),corpus_after=B.sha(m.CORPUS),power_loss_claim=False,append_in_progress_kill=False,no_optimization=True,dependent_stages_started=False)
        assert value['protected_after']==d['protected_before'] and value['corpus_after']==d['corpus_sha256']
        B.write(result_path,value);B.write(m.s.CURRENT,dict(corpus='cap200k-write50',documents=50000,sha256=d['corpus_sha256'],slots={'fdb':str(state),'sqlite':d['states']['sqlite']},same_model_sha256=B.sha(state)))
        print(json.dumps(dict(kills=3,old_reader=True,oracle=True)),flush=True)
    finally:
        stop.write_text('recovery complete',encoding='utf-8');watcher.wait(timeout=20);event('driver_finish');stream.close()

if __name__=='__main__':main()
