"""Only diagnose the failed single gate and verify recovery; no dependent workload."""
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
    assert not (L/f'{P}-postgate.json').exists()
    before=B.protected();assert before==d['protected_before']
    lifecycle=L/f'{P}-postgate-lifecycle.jsonl';stream=lifecycle.open('x',encoding='utf-8')
    def event(kind,**kw):stream.write(json.dumps(dict(utc=m.utc(),kind=kind,**kw),ensure_ascii=False)+'\n');stream.flush();os.fsync(stream.fileno())
    stop=L/f'{P}-postgate-watch.stop'
    watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(L/f'{P}-postgate-process-events.jsonl'),'-StopPath',str(stop),'-LifecyclePath',str(lifecycle)],cwd=R,stdout=(L/f'{P}-postgate-watch.stdout').open('x'),stderr=(L/f'{P}-postgate-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    event('driver_start',pid=os.getpid(),parent_pid=os.getppid())
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_BENCH_CORPUS_PATH=str(m.CORPUS),FDB_BENCH_CORPUS_INDEX=str(m.CATALOG),FDB_BENCH_ORIGINAL_BODIES='1',FDB_BENCH_50K_WRITE='1',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',FDB_SCALE_DOCUMENTS='50000')
    completed=[]
    def run(program,mode,state,label,options):
        log=L/f'{P}-{label}-fdb-{mode}.jsonl';assert not log.exists();at=time.monotonic()
        with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
            child=subprocess.Popen([str(program),mode,'streamed',str(state)],cwd=R,env=options,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
            event('worker_start',pid=child.pid,label=label,mode=mode);code=child.wait();event('worker_exit',pid=child.pid,exit_code=code,label=label)
        r=dict(engine='fdb',mode=mode,label=label,log=log.name,exit_code=code,wall_s=time.monotonic()-at);B.write(log.with_suffix('.process.json'),r);completed.append(r);print(json.dumps(r),flush=True);assert code==0
        return r
    try:
        B.reset('fdb');state=L/f'{P}-single-diagnostic-state.json';assert not state.exists()
        se=env.copy();se['FDB_SEED_IDLE_CLEANUP']='1'
        run(m.PROGRAMS['diag'],'scale-seed',state,'single-diagnostic-seed',se)
        pe=env.copy();pe['FDB_SEQUENCE_PROFILE']='1'
        run(m.PROGRAMS['diag'],'write50-single',state,'single-diagnostic',pe)
        assert B.sha(state)==B.sha(Path(d['states']['fdb'])),'Same logical commit sequence required'
        # Kill an opt-in Pending open, without changing any acknowledged transaction.
        ke=env.copy();ke.update(FDB_FIRST_QUERY='olduğu',FDB_FIRST_VARIANT='cap-hybrid')
        with (L/f'{P}-pending-kill.stderr').open('x',encoding='utf-8') as err:
            child=subprocess.Popen([str(m.PROGRAMS['fdb']),'first-background','streamed',str(state)],cwd=R,env=ke,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW)
            event('worker_start',pid=child.pid,label='pending-kill');witness=json.loads(child.stdout.readline());assert witness['status']=='pending'
            event('intentional_kill',actor_pid=os.getpid(),pid=child.pid,witness=witness);child.kill();code=child.wait(timeout=10);event('worker_exit',pid=child.pid,exit_code=code,label='pending-kill');assert code!=0
        run(m.PROGRAMS['fdb'],'first-strict',state,'reopen-after-kill',ke)
        sys.path.insert(0,str(R/'tools'));import importlib
        old=importlib.import_module('kill-block2-scale');old.P=P+'-compat';os.environ.update({k:v for k,v in env.items() if k.startswith('FDB_')})
        event('compat_begin');compat=old.old_reader(state);event('compat_end',oracle=compat['oracle'])
        B.write(m.s.CURRENT,dict(corpus='cap200k-write50',documents=50000,sha256=d['corpus_sha256'],slots={'fdb':str(state),'sqlite':d['states']['sqlite']},same_model_sha256=B.sha(state)))
        result=dict(completed=completed,model_sha256=B.sha(state),same_logical_workload=True,kill=dict(witness=witness,exit_code=code,intentional=True,power_loss_claim=False),old_reader=compat,protected_after=B.protected(),source_after=B.sha(m.CORPUS),no_optimization=True,dependent_stages_started=False)
        assert result['protected_after']==before and result['source_after']==d['corpus_sha256']
        B.write(L/f'{P}-postgate.json',result)
    finally:
        stop.write_text('postgate done',encoding='utf-8');watcher.wait(timeout=20);event('driver_finish');stream.close()

if __name__=='__main__':main()
