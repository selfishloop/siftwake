"""One fixed16k control diagnostic, then narrow query diagnostics. No optimization."""
import ctypes
import argparse
import importlib.util
import json
import os
import subprocess
import threading
import time
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='write50-revised-20261029-v1-control'
spec=importlib.util.spec_from_file_location('m',R/'tools/measure-write50.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);B=m.B

class ThreadEntry(ctypes.Structure):
    _fields_=[('size',ctypes.c_uint32),('usage',ctypes.c_uint32),('tid',ctypes.c_uint32),('pid',ctypes.c_uint32),('priority',ctypes.c_int32),('delta',ctypes.c_int32),('flags',ctypes.c_uint32)]

def thread_count(pid):
    k=ctypes.WinDLL('kernel32',use_last_error=True)
    k.CreateToolhelp32Snapshot.argtypes=[ctypes.c_uint32,ctypes.c_uint32];k.CreateToolhelp32Snapshot.restype=ctypes.c_void_p
    k.Thread32First.argtypes=k.Thread32Next.argtypes=[ctypes.c_void_p,ctypes.POINTER(ThreadEntry)]
    k.CloseHandle.argtypes=[ctypes.c_void_p]
    h=k.CreateToolhelp32Snapshot(4,0)
    if h==ctypes.c_void_p(-1).value:raise ctypes.WinError(ctypes.get_last_error())
    try:
        row=ThreadEntry();row.size=ctypes.sizeof(row);ok=k.Thread32First(h,ctypes.byref(row));n=0
        while ok:
            n+=row.pid==pid;ok=k.Thread32Next(h,ctypes.byref(row))
        return n
    finally:k.CloseHandle(h)

def run_child(program,mode,state,env,prefix,event):
    log=L/f'{prefix}-{mode}.jsonl';assert not log.exists();samples=[];done=threading.Event();at=time.monotonic()
    with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
        child=subprocess.Popen([str(program),mode,'streamed',str(state)],cwd=R,env=env,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
        event('worker_start',pid=child.pid,mode=mode)
        def observe():
            while not done.is_set():
                try:samples.append(dict(elapsed_s=time.monotonic()-at,threads=thread_count(child.pid)))
                except OSError as error:samples.append(dict(elapsed_s=time.monotonic()-at,error=str(error)))
                done.wait(.05)
        observer=threading.Thread(target=observe);observer.start();code=child.wait();done.set();observer.join();event('worker_exit',pid=child.pid,mode=mode,exit_code=code)
    record=dict(log=log.name,mode=mode,exit_code=code,wall_s=time.monotonic()-at,pid=child.pid,logical_processors=os.cpu_count(),thread_sample_interval_ms=50,thread_peak=max((x.get('threads',0) for x in samples),default=0),thread_samples=samples)
    B.write(log.with_suffix('.process.json'),record);print(json.dumps({k:v for k,v in record.items() if k!='thread_samples'}),flush=True)
    if code:raise RuntimeError(log.name)
    return record

def env():
    options=os.environ.copy();options.update(PYTHONUTF8='1',FDB_BENCH_CORPUS_PATH=str(m.CORPUS),FDB_BENCH_CORPUS_INDEX=str(m.CATALOG),FDB_BENCH_ORIGINAL_BODIES='1',FDB_BENCH_50K_WRITE='1',FDB_SCALE_DOCUMENTS='50000',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',FDB_SNAPSHOT_FIRST_HYBRID='1',FDB_SNAPSHOT_ROUNDS='3',FDB_SNAPSHOT_FIXED_COMMITS='16384',FDB_SNAPSHOT_MAX_COMMITS='16384')
    for key in ['FDB_SEQUENCE_PROFILE','FDB_SQL_WRITE_PROFILE','FDB_SEED_PHASE_PROFILE','FDB_BENCH_SNAPSHOTS']:options.pop(key,None)
    return options

def main():
    global P
    ap=argparse.ArgumentParser();ap.add_argument('--prefix',default=P);ap.add_argument('--program',default='output/write50-revised-20261029-v1-profile.exe');args=ap.parse_args();P=args.prefix
    dest=L/f'{P}-design.json';assert not dest.exists();assert m.read(L/'write50-revised-20261029-v1-gate.json')['passed']
    program=R/args.program;assert program.exists()
    d=dict(program_sha256=B.sha(program),source_sha256=B.sha(m.CORPUS),protected_before=B.protected(),stage='control_profile',completed=[],optimization=False)
    B.write(dest,d);life=L/f'{P}-lifecycle.jsonl';stream=life.open('x',encoding='utf-8')
    def event(kind,**v):stream.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');stream.flush();os.fsync(stream.fileno())
    stop=L/f'{P}-watch.stop';watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(L/f'{P}-events.jsonl'),'-StopPath',str(stop),'-LifecyclePath',str(life)],stdout=(L/f'{P}-watch.stdout').open('x'),stderr=(L/f'{P}-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    event('driver_start',pid=os.getpid());state=L/f'{P}-state.json';options=env()
    try:
        B.reset('fdb');se=options.copy();se['FDB_SEED_IDLE_CLEANUP']='1'
        d['completed'].append(run_child(program,'scale-seed',state,se,P,event));B.write(dest,d)
        options.update(FDB_SEQUENCE_PROFILE='1',FDB_WRITE_PROFILE_CAPACITY='32768',FDB_THREAD_PROFILE='1')
        d['completed'].append(run_child(program,'snapshot-control-1',state,options,P,event));B.write(dest,d)
        x=next(x for x in map(json.loads,(L/d['completed'][-1]['log']).read_text().splitlines()) if x['phase']=='snapshot_load')
        assert x['commits']==16384 and x['oracle'] and x['write_profile']['dropped_frames']==0 and len(x['write_profile']['frames'])==16384
        assert B.sha(state)==B.sha(Path(m.read(L/'hybrid4r50-20261029-v1-b1-design.json')['states']['fdb']))
        d.update(stage='completed',source_after=B.sha(m.CORPUS),protected_after=B.protected(),state=str(state));assert d['source_after']==d['source_sha256'] and d['protected_after']==d['protected_before'];B.write(dest,d)
        pointer=m.read(m.s.CURRENT);pointer['slots']['fdb'].update(state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,pointer)
    finally:stop.write_text('normal stop');watcher.wait(timeout=20);event('driver_finish',stage=d['stage']);stream.close()

if __name__=='__main__':main()
