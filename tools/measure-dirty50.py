"""Dirty500 gate, same first50k, balanced slots, lifecycle evidence; no optimization."""
import importlib.util
import json
import os
import subprocess
import time
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='dirty50-20261029-v1'
spec=importlib.util.spec_from_file_location('write',R/'tools/measure-write50.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
B=m.B
PROGRAMS={e:R/f'output/tail-dirty50-20261029-{e}.exe' for e in ['fdb','sqlite','diagnostic']}
def read(p):return m.read(p)
def rows(r):return [json.loads(v) for v in (L/r['log']).read_text(encoding='utf-8').splitlines() if v]

def main():
    design=L/f'{P}-design.json';assert not design.exists()
    accepted=read(L/'write50-tail-acceptance-20261029.json');assert all(x['new_pass'] for x in accepted['results'] if x['engine']=='fdb')
    checks=read(L/'snapshot-10k-20261026-validation-tail-dirty50-20261029-v1.json');assert len(checks)==8 and all(x['exit_code']==0 for x in checks)
    assert B.sha(m.CORPUS)==read(L/'trwiki-cap200k-20261029-v3-audit.json')['payload_sha256']
    d=dict(documents=50000,source_sha256=B.sha(m.CORPUS),programs={e:dict(path=str(p.relative_to(R)),sha256=B.sha(p)) for e,p in PROGRAMS.items()},
        implementation_sha256={str(p.relative_to(R)):B.sha(p) for p in sorted((R/'src').rglob('*.rs'))},protected_before=B.protected(),
        completed=[],states={},stage='dirty500',orders=[['fdb','sqlite'],['sqlite','fdb']],
        preparation='five independent100-op streamed commits; foreground never installs completed prefix between parts; each process closes normally; verify delta500, do not estimate it',
        sqlite_preparation='short read transaction pins baseline during setup, then intentional process kill leaves acknowledged WAL; not a long-snapshot stress test',
        cache='warm/uncontrolled OS; strict CRC/source checks retained; first fresh-process open and idle without oracle',
        optimization=False,persistent_format_changed=False,entry_limit_changed=False)
    def save():B.write(design,d)
    save();life=L/f'{P}-lifecycle.jsonl';stream=life.open('x',encoding='utf-8')
    def event(kind,**v):stream.write(json.dumps(dict(utc=m.utc(),kind=kind,**v),ensure_ascii=False)+'\n');stream.flush();os.fsync(stream.fileno())
    stop=L/f'{P}-watch.stop'
    watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(L/f'{P}-process-events.jsonl'),'-StopPath',str(stop),'-LifecyclePath',str(life)],cwd=R,stdout=(L/f'{P}-watch.stdout').open('x'),stderr=(L/f'{P}-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    event('driver_start',pid=os.getpid(),parent_pid=os.getppid())
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_BENCH_CORPUS_PATH=str(m.CORPUS),FDB_BENCH_CORPUS_INDEX=str(m.CATALOG),FDB_BENCH_ORIGINAL_BODIES='1',FDB_BENCH_50K_WRITE='1',FDB_SCALE_DOCUMENTS='50000',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',FDB_FIRST_QUERY='olduğu',FDB_FIRST_VARIANT='cap-hybrid')
    for key in ['FDB_SEQUENCE_PROFILE','FDB_SEED_PHASE_PROFILE','FDB_SQL_WRITE_PROFILE','FDB_BENCH_SNAPSHOTS']:env.pop(key,None)
    keep=None;keeper_files=[]
    def run(e,mode,state,label,options=None):
        options=options or env;program=PROGRAMS[e];engine='fdb' if e=='diagnostic' else e
        log=L/f'{P}-{label}-{e}-{mode}.jsonl';assert not log.exists();at=time.monotonic()
        with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
            child=subprocess.Popen([str(program),mode,'streamed',str(state)],cwd=R,env=options,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
            event('worker_start',pid=child.pid,mode=mode,label=label);code=child.wait();event('worker_exit',pid=child.pid,exit_code=code,label=label)
        r=dict(engine=engine,program=e,mode=mode,label=label,log=log.name,exit_code=code,wall_s=time.monotonic()-at);B.write(log.with_suffix('.process.json'),r);d['completed'].append(r);save();print(json.dumps(r),flush=True)
        if code:raise RuntimeError(f'Worker failed: {log.name}')
        return r
    try:
        for pair,order in enumerate(d['orders']):
            states={}
            for e in order:
                B.reset(e);state=L/f'{P}-pair{pair}-{e}-state.json';assert not state.exists()
                se=env.copy()
                if e=='fdb':se['FDB_SEED_IDLE_CLEANUP']='1'
                run(e,'scale-seed',state,f'pair{pair}-seed',se);states[e]=state;d['states'][e]=str(state);save()
            assert B.sha(states['fdb'])==B.sha(states['sqlite'])
            for e in order:
                state=states[e]
                if e=='sqlite':
                    err=(L/f'{P}-pair{pair}-keeper.stderr').open('x',encoding='utf-8');keeper_files.append(err)
                    keep=subprocess.Popen([str(PROGRAMS[e]),'write50-hold-setup','streamed',str(state)],cwd=R,env=env,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW)
                    event('worker_start',pid=keep.pid,label=f'pair{pair}-sqlite-keeper')
                    witness=[]
                    while True:
                        line=keep.stdout.readline()
                        if not line:raise RuntimeError('SQLite setup keeper failed')
                        x=json.loads(line);witness.append(x)
                        if x['phase']=='setup_reader_ready':break
                    event('setup_read_transaction',pid=keep.pid,witness=witness)
                for part in range(5):
                    pe=env.copy();pe['FDB_DIRTY_PART']=str(part)
                    result=run(e,'write50-dirty-part',state,f'pair{pair}-part{part}',pe)
                    last=next(x for x in rows(result) if x['phase']=='dirty_part')
                    if e=='fdb':assert last['stats']['delta_keys']==(part+1)*100,last
                if keep is not None:
                    event('intentional_kill',pid=keep.pid,actor_pid=os.getpid(),reason='end setup reader without final SQLite close checkpoint');keep.kill();code=keep.wait(timeout=10);event('worker_exit',pid=keep.pid,exit_code=code,label=f'pair{pair}-sqlite-keeper');keep=None
                d.setdefault('before_open',[]).append(dict(pair=pair,engine=e,model_sha256=B.sha(state),disk=sorted((p.name,p.stat().st_size) for p in (R/f'data/benchmark-work/{e}').iterdir() if p.is_file())));save()
                result=run(e,'scale-idle',state,f'pair{pair}-dirty-open')
                opened=next(x for x in rows(result) if x['phase']=='opened')
                if e=='fdb':assert opened['stats']['delta_keys']==500 and opened['stats']['sequence']==105
                run(e,'first-strict' if e=='fdb' else 'first-sqlite',state,f'pair{pair}-oracle')
                if e=='fdb':run('diagnostic','open-profile',state,f'pair{pair}-replay-profile')
            assert B.sha(states['fdb'])==B.sha(states['sqlite'])
            B.write(m.s.CURRENT,dict(corpus='trwiki-cap200k-dirty500',source=str(m.CORPUS),sha256=d['source_sha256'],original_bodies=True,runtime_revision_mode='FDB_BENCH_50K_WRITE=1',slots={e:dict(documents=50000,status='ready',state=str(p),model_sha256=B.sha(p),sequence=105) for e,p in states.items()}))
        times=[];memory=[];replay=[]
        for r in d['completed']:
            if r['program']=='fdb' and r['mode']=='scale-idle':
                for x in rows(r):
                    if x['phase']=='opened':times.append(x['open_ms'])
                    if x['phase']=='scale_idle':memory.append(x['memory']['private_bytes'])
            if r['program']=='diagnostic':
                for x in rows(r):
                    if x['phase']=='strict_open_profile':
                        c=x['counters']['open.active_wal_replay'];assert c['calls']==1;replay.append(c['elapsed_ns']/1e6)
        assert len(times)==len(memory)==len(replay)==2
        gate=dict(stage='dirty500',strict_open_ms=times,idle_private_bytes=memory,diagnostic_wal_replay_ms=replay,passed=max(times)<=1000 and max(memory)<=15*2**20)
        d['gate']=gate;d['stage']='dirty_passed' if gate['passed'] else 'STOP_dirty_failed';save();B.write(L/f'{P}-gate.json',gate);event('gate',**gate)
        d.update(protected_after=B.protected(),source_after=B.sha(m.CORPUS));assert d['protected_before']==d['protected_after'] and d['source_sha256']==d['source_after'];d['protected_unchanged']=d['source_unchanged']=True;save()
    except BaseException as error:
        d['error']=dict(type=type(error).__name__,message=str(error));save();event('driver_error',**d['error']);raise
    finally:
        if keep is not None and keep.poll() is None:keep.kill();keep.wait(timeout=10)
        for f in keeper_files:f.close()
        stop.write_text('driver stopping',encoding='utf-8');watcher.wait(timeout=20);event('driver_finish',stage=d['stage']);stream.close()

if __name__=='__main__':main()
