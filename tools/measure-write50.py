"""First50k write profiling, observed process lifecycle, strict stop gates."""
import importlib.util
import json
import math
import os
import subprocess
import time
from collections import defaultdict
from datetime import datetime,timezone
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='write50-20261029-v1'
spec=importlib.util.spec_from_file_location('scale',R/'tools/measure-trwiki-scale.py');s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)
B=s.b
CORPUS=Path('D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench')
CATALOG=L/'trwiki-cap200k-20261029-v3-catalog.json'
PROGRAMS={'fdb':R/'output/write50-20261029-v3-fdb.exe','sqlite':R/'output/write50-20261029-v3-sqlite.exe','diag':R/'output/write50-20261029-v3-diagnostic.exe','old':R/'output/trwiki-scale-20261028-v2-fdb.exe','oldsqlite':R/'output/trwiki-scale-20261028-v2-sqlite.exe'}

def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def utc():return datetime.now(timezone.utc).isoformat()
def rows(r):return [json.loads(v) for v in (L/r['log']).read_text(encoding='utf-8').splitlines() if v]
def percentile(xs):return sorted(xs)[math.ceil(.95*len(xs))-1]

def main():
    design=L/f'{P}-design.json';assert not design.exists(),'Inspect saved stages; never replay/reset silently'
    accepted=read(L/'cap200k-scale-20261029-v1-final-audit.json')
    assert accepted['source_sha256']=={str(p.relative_to(R)):B.sha(p) for p in sorted((R/'src').rglob('*.rs'))}
    audit=read(L/'trwiki-cap200k-20261029-v3-audit.json');assert B.sha(CORPUS)==audit['payload_sha256']
    assert B.sha(CATALOG)==audit['catalog_sha256']
    checks=read(L/'snapshot-10k-20261026-validation-write50-20261029-v4.json');assert len(checks)==8 and all(x['exit_code']==0 for x in checks)
    d=dict(started_utc=utc(),documents=50000,corpus_sha256=audit['payload_sha256'],catalog_sha256=audit['catalog_sha256'],
        programs={k:dict(path=str(v.relative_to(R)),sha256=B.sha(v)) for k,v in PROGRAMS.items()},
        source_sha256=accepted['source_sha256'],protected_before=B.protected(),completed=[],states={},gates=[],
        stage='ingest_ab',persistent_format_changed=False,optimization=False,cache='OS warm/uncontrolled, normal strict open; no forced eviction',
        corpus_policy='same provided first50k; runtime revisions keep donor body, append unique oracle token, cap UTF8 text200000B; corpus never written',
        ab='same session old/current FDB binaries, same WAL-on and500 commit; SQLite old/current comparator; extra current WAL-off control',
        single='256/type x4 types x2 balanced orders; equal new-body size bands2-4/4-16/16-64/64-200KB; outgoing sizes recorded separately')
    B.write(design,d)
    lifecycle=L/f'{P}-lifecycle.jsonl';assert not lifecycle.exists()
    stream=lifecycle.open('x',encoding='utf-8')
    def event(kind,**values):
        stream.write(json.dumps(dict(utc=utc(),kind=kind,**values),ensure_ascii=False)+'\n');stream.flush();os.fsync(stream.fileno())
    stop=L/f'{P}-watch.stop';watchlog=L/f'{P}-process-events.jsonl';assert not stop.exists() and not watchlog.exists()
    watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(watchlog),'-StopPath',str(stop),'-LifecyclePath',str(lifecycle)],cwd=R,stdout=(L/f'{P}-watch.stdout').open('x'),stderr=(L/f'{P}-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    event('driver_start',pid=os.getpid(),parent_pid=os.getppid(),watcher_pid=watcher.pid)
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_BENCH_CORPUS_PATH=str(CORPUS),FDB_BENCH_CORPUS_INDEX=str(CATALOG),
        FDB_BENCH_ORIGINAL_BODIES='1',FDB_BENCH_50K_WRITE='1',FDB_SCALE_DOCUMENTS='50000',FDB_RECLAMATION_CONTROL='1',FDB_STREAMED_WAL_LAYER='1')
    for k in ['FDB_SEQUENCE_PROFILE','FDB_SEED_PHASE_PROFILE','FDB_SQL_WRITE_PROFILE','FDB_BENCH_SNAPSHOTS']:env.pop(k,None)
    def save():B.write(design,d)
    def run(e,mode,state,label,options,program=None):
        program=program or PROGRAMS[e];log=L/f'{P}-{label}-{e}-{mode}.jsonl';assert not log.exists()
        started=time.monotonic()
        with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
            child=subprocess.Popen([str(program),mode,'streamed',str(state)],cwd=R,env=options,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
            event('worker_start',pid=child.pid,parent_pid=os.getpid(),engine=e,mode=mode,label=label,program=str(program.relative_to(R)))
            code=child.wait()
        result=dict(engine=e,mode=mode,label=label,log=log.name,state=str(state),exit_code=code,wall_s=time.monotonic()-started)
        B.write(log.with_suffix('.process.json'),result);event('worker_exit',pid=child.pid,**result)
        d['completed'].append(result);save();print(json.dumps(result),flush=True)
        if code:raise RuntimeError(f'Worker failed; evidence retained: {log.name}')
        return result
    def seed(e,label,program=None,layer=True,profile=False):
        B.reset(e);state=L/f'{P}-{label}-{e}-state.json';assert not state.exists()
        options=env.copy()
        if not layer:options.pop('FDB_STREAMED_WAL_LAYER',None)
        if e=='fdb':options['FDB_SEED_IDLE_CLEANUP']='1'
        if profile:options['FDB_SEED_PHASE_PROFILE' if e=='fdb' else 'FDB_SQL_WRITE_PROFILE']='1'
        B.write(s.CURRENT,dict(corpus='cap200k-write50',documents=50000,sha256=audit['payload_sha256'],slots=d['states'],resetting=e,label=label))
        result=run(e,'scale-seed',state,label,options,program)
        d['states'][e]=str(state);save();B.write(s.CURRENT,dict(corpus='cap200k-write50',documents=50000,sha256=audit['payload_sha256'],slots=d['states']))
        return state,result
    try:
        # Both historical configurations are actually WAL-on, synced,500/group.
        seed('fdb','ab-old',PROGRAMS['old']);seed('sqlite','ab-old',PROGRAMS['oldsqlite'])
        seed('sqlite','ab-current');seed('fdb','ab-current')
        seed('fdb','ab-current-wal-off',layer=False)
        d['stage']='diagnostic500';save()
        fs,_=seed('fdb','diag500',PROGRAMS['diag'],profile=True)
        ss,_=seed('sqlite','diag500',profile=True)
        d['stage']='single';save()
        for pair,order in enumerate([['fdb','sqlite'],['sqlite','fdb']]):
            if pair:
                for e in order:seed(e,f'single-pair{pair}-seed')
            states={e:Path(d['states'][e]) for e in order}
            assert B.sha(states['fdb'])==B.sha(states['sqlite'])
            for e in order:
                run(e,'scale-idle',states[e],f'single-pair{pair}-clean-idle',env)
                run(e,'write50-single',states[e],f'single-pair{pair}',env)
            assert B.sha(states['fdb'])==B.sha(states['sqlite'])
        samples=defaultdict(list)
        for r in d['completed']:
            if r['engine']=='fdb' and r['mode']=='write50-single':
                for x in rows(r):
                    if x['phase']=='write50_commit':samples[x['kind']].append(x)
        gate=dict(stage='single',passed=True,per_type=[],failures=[])
        for kind,xs in sorted(samples.items()):
            assert len(xs)==512
            value=dict(kind=kind,n=len(xs),commit_p95_ms=percentile([x['commit_ms'] for x in xs]),commit_max_ms=max(x['commit_ms'] for x in xs),
                visibility_p95_ms=percentile([x['visibility_ms'] for x in xs]),visibility_max_ms=max(x['visibility_ms'] for x in xs),
                peak_private_bytes=max(x['memory']['os_lifetime_peak_private_commit_bytes'] for x in xs))
            value['passed']=value['commit_p95_ms']<=10 and value['commit_max_ms']<=50 and value['visibility_max_ms']<=50 and value['peak_private_bytes']<=60*2**20
            gate['per_type'].append(value)
            if not value['passed']:gate['failures'].append(value)
        idle=[x['memory']['private_bytes'] for r in d['completed'] if r['engine']=='fdb' and r['mode']=='scale-idle' for x in rows(r) if x['phase']=='scale_idle']
        gate['clean_idle_private_bytes']=idle
        if max(idle)>15*2**20:gate['failures'].append(dict(kind='clean_idle',max_bytes=max(idle)))
        gate['passed']=not gate['failures'];d['gates'].append(gate);B.write(L/f'{P}-single-gate.json',gate)
        d['stage']='single_passed_next_pending' if gate['passed'] else 'STOP_single_failed';save()
        event('gate',**gate)
        d.update(protected_after=B.protected(),corpus_after=B.sha(CORPUS));assert d['protected_before']==d['protected_after'] and d['corpus_sha256']==d['corpus_after']
        d['protected_unchanged']=d['corpus_unchanged']=True;save()
    except BaseException as error:
        d['execution_error']=dict(type=type(error).__name__,message=str(error),utc=utc());save();event('driver_error',**d['execution_error']);raise
    finally:
        stop.write_text('normal driver cleanup\n',encoding='utf-8')
        try:watcher.wait(timeout=15)
        except subprocess.TimeoutExpired:event('watcher_timeout',pid=watcher.pid)
        event('driver_finish',stage=d['stage']);stream.close()

if __name__=='__main__':main()
