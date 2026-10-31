"""Witnessed process kills and frozen-reader oracle for a measured50k model."""
import argparse
import importlib
import importlib.util
import json
import os
import subprocess
import zlib
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks'
spec=importlib.util.spec_from_file_location('measure',R/'tools/measure-write50.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
B=m.B

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--prefix',required=True);ap.add_argument('--state',required=True);ap.add_argument('--program',required=True);ap.add_argument('--delta',type=int,default=-1);args=ap.parse_args()
    p=args.prefix;state=Path(args.state).resolve();program=Path(args.program).resolve();assert state.parent==L
    destination=L/f'{p}-recovery.json';assert not destination.exists()
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_BENCH_CORPUS_PATH=str(m.CORPUS),FDB_BENCH_CORPUS_INDEX=str(m.CATALOG),FDB_BENCH_ORIGINAL_BODIES='1',FDB_BENCH_50K_WRITE='1',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',FDB_SCALE_DOCUMENTS='50000',FDB_FIRST_QUERY='olduğu',FDB_FIRST_VARIANT='cap-hybrid')
    for k in ['FDB_SEQUENCE_PROFILE','FDB_SEED_PHASE_PROFILE','FDB_SQL_WRITE_PROFILE']:env.pop(k,None)
    life=L/f'{p}-recovery-lifecycle.jsonl';stream=life.open('x',encoding='utf-8')
    def event(kind,**v):stream.write(json.dumps(dict(utc=m.utc(),kind=kind,**v),ensure_ascii=False)+'\n');stream.flush();os.fsync(stream.fileno())
    stop=L/f'{p}-recovery-watch.stop'
    watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(L/f'{p}-recovery-events.jsonl'),'-StopPath',str(stop),'-LifecyclePath',str(life)],cwd=R,stdout=(L/f'{p}-recovery-watch.stdout').open('x'),stderr=(L/f'{p}-recovery-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    event('driver_start',pid=os.getpid(),parent_pid=os.getppid());kills=[]
    def sources():
        root=R/'data/benchmark-work/fdb'
        all_files={f.name:B.sha(f) for f in root.iterdir() if f.suffix in ('.fdb','.ready')}
        marker=max(root.glob('generation-*.ready'),key=lambda f:int(f.stem.split('-')[1],16))
        raw=marker.read_bytes();assert raw[:8]==b'FDBMAN03' and len(raw)==int.from_bytes(raw[16:20],'little')+24
        assert zlib.crc32(raw[:-4])==int.from_bytes(raw[-4:],'little')
        manifest=json.loads(raw[20:-4]);live=[marker.name,f'generation-{manifest["active"]["id"]:016x}.fdb']
        live.extend(f'{"run" if s["kind"]=="Run" else "generation"}-{s["id"]:016x}.fdb' for s in manifest['runs'])
        return dict(all_files=all_files,manifest=manifest,live={name:all_files[name] for name in live})
    before=sources();protected=B.protected();inventories=[dict(stage='before',**before)]
    def inventory(stage):
        current=sources();inventories.append(dict(stage=stage,**current));B.write(L/f'{p}-recovery-inventory.json',inventories)
        assert current['live']==before['live'],'Live source or current manifest changed'
        removed=set(before['all_files'])-set(current['all_files'])
        assert not removed.intersection(before['live'])
        assert all(before['all_files'][k]==v for k,v in current['all_files'].items()),'Source added/modified'
        return current
    B.write(L/f'{p}-recovery-inventory.json',inventories)
    try:
        for i in range(3):
            with (L/f'{p}-kill{i}.stderr').open('x',encoding='utf-8') as err:
                child=subprocess.Popen([str(program),'write50-open','streamed',str(state)],cwd=R,env=env,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW)
                event('worker_start',pid=child.pid,label=f'kill{i}');line=child.stdout.readline()
                if not line:event('worker_exit',pid=child.pid,exit_code=child.wait());raise RuntimeError('open failed before witness')
                witness=json.loads(line);assert witness['phase']=='opened'
                if args.delta>=0:assert witness['stats']['delta_keys']==args.delta
                event('intentional_kill',actor_pid=os.getpid(),pid=child.pid,witness=witness);child.kill();code=child.wait(timeout=10);event('worker_exit',pid=child.pid,exit_code=code,label=f'kill{i}');assert code!=0;kills.append(dict(witness=witness,exit_code=code));inventory(f'after_kill{i}')
        log=L/f'{p}-recovery-first-strict.jsonl'
        with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
            child=subprocess.Popen([str(program),'first-strict','streamed',str(state)],cwd=R,env=env,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW);event('worker_start',pid=child.pid,label='reopen');code=child.wait();event('worker_exit',pid=child.pid,exit_code=code,label='reopen');assert code==0
        assert any(json.loads(v)['phase']=='first_result_oracle' for v in log.read_text(encoding='utf-8').splitlines())
        inventory('after_reopen')
        import sys;sys.path.insert(0,str(R/'tools'));old=importlib.import_module('kill-block2-scale');old.P=p+'-compat';os.environ.update({k:v for k,v in env.items() if k.startswith('FDB_')})
        event('compat_begin');compatible=old.old_reader(state);event('compat_end',oracle=compatible['oracle']);after=inventory('after_old_reader')
        result=dict(kills=kills,scope='after strict open witnessed; no append-in-progress or power-loss claim',reopen_log=log.name,old_reader=compatible,model_sha256=B.sha(state),sources_unchanged=before['all_files']==after['all_files'],live_sources_unchanged=True,unreferenced_removed=sorted(set(before['all_files'])-set(after['all_files'])),inventory_log=f'{p}-recovery-inventory.json',protected_before=protected,protected_after=B.protected(),source_sha256=B.sha(m.CORPUS));assert result['protected_before']==result['protected_after'];B.write(destination,result)
        print(json.dumps(dict(prefix=p,kills=3,old_reader=True,oracle=True)),flush=True)
    finally:
        stop.write_text('normal stop',encoding='utf-8');watcher.wait(timeout=20);event('driver_finish');stream.close()

if __name__=='__main__':main()
