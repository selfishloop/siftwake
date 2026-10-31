"""Accepted revised gates; delta+mask candidate, one balanced FDB/SQLite pair."""
import argparse
import importlib.util
import json
import math
import os
import subprocess
import time
from collections import defaultdict
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks'
spec=importlib.util.spec_from_file_location('write',R/'tools/measure-write50.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
B=m.B
def read(p):return m.read(p)
def rows(r):return [json.loads(v) for v in (L/r['log']).read_text(encoding='utf-8').splitlines() if v]
def pct(xs,p=.95):return sorted(xs)[math.ceil(p*len(xs))-1]
def records(x):return [r for reader in x['results'] for r in reader['records']]

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--batch',type=int,choices=[1,100],default=1);args=ap.parse_args();batch=args.batch
    P=f'delta-load50-20261030-b{batch}';design=L/f'{P}-design.json';assert not design.exists()
    assert read(L/'dirty50-20261029-v1-gate.json')['passed'] and read(L/'dirty50-20261029-v1-recovery.json')['old_reader']['oracle']
    if batch==100:
        assert read(L/'delta-load50-20261030-b1-gate.json')['passed'], 'Stop at single-writer gate'
        assert read(L/'write50-revised-20261029-v1-gate.json')['passed']
        assert read(L/'write50-revised-20261029-profile4-complete.json')['completed']
    programs={e:(R/'output/delta-cap-20261030.exe' if e=='fdb' else R/'output/hybrid4r50-20261029-v2-sqlite.exe') for e in ['fdb','sqlite']}
    assert B.sha(m.CORPUS)==read(L/'trwiki-cap200k-20261029-v3-audit.json')['payload_sha256']
    d=dict(batch=batch,documents=50000,source_sha256=B.sha(m.CORPUS),programs={e:dict(path=str(p.relative_to(R)),sha256=B.sha(p)) for e,p in programs.items()},
        protected_before=B.protected(),completed=[],states={},stage='4R_single' if batch==1 else '4R_100',
        commits=16384 if batch==1 else 1024,rounds=3,
        sequence=[['fdb','control'],['sqlite','control'],['sqlite','loaded'],['fdb','loaded'],],
        query='FDB Snapshot.search_first_hybrid budget200/N50; SQLite capped N50; exact totals not requested',
        latency='admission+search+release; full payload/model oracle outside interval; oracle CPU/I/O still competes',
        waits='hybrid internal read/permit waits not independently instrumented; total latency includes them; admission/release measured',
        body='1100 eligible first50k donors2..199960B; same body, unique revision token changes; stable41 query ID sets',
        throughput='pooled equal-op control and loaded, same fixed commit count and interlude; two counterbalanced arm orders',
        cache='warm OS/engine after full payload oracle + idle queries; no cold eviction',optimization=True)
    def save():B.write(design,d)
    save();life=L/f'{P}-lifecycle.jsonl';stream=life.open('x',encoding='utf-8')
    def event(kind,**v):stream.write(json.dumps(dict(utc=m.utc(),kind=kind,**v),ensure_ascii=False)+'\n');stream.flush();os.fsync(stream.fileno())
    stop=L/f'{P}-watch.stop'
    watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(L/f'{P}-process-events.jsonl'),'-StopPath',str(stop),'-LifecyclePath',str(life)],cwd=R,stdout=(L/f'{P}-watch.stdout').open('x'),stderr=(L/f'{P}-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    event('driver_start',pid=os.getpid(),parent_pid=os.getppid())
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_BENCH_CORPUS_PATH=str(m.CORPUS),FDB_BENCH_CORPUS_INDEX=str(m.CATALOG),FDB_BENCH_ORIGINAL_BODIES='1',FDB_BENCH_50K_WRITE='1',FDB_SCALE_DOCUMENTS='50000',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',FDB_SNAPSHOT_FIRST_HYBRID='1',FDB_SNAPSHOT_ROUNDS='3',FDB_SNAPSHOT_FIXED_COMMITS=str(d['commits']),FDB_SNAPSHOT_MAX_COMMITS=str(d['commits']))
    for key in ['FDB_SEQUENCE_PROFILE','FDB_SQL_WRITE_PROFILE','FDB_SEED_PHASE_PROFILE','FDB_BENCH_SNAPSHOTS']:env.pop(key,None)
    def run(e,mode,state,label,options=None):
        options=options or env;log=L/f'{P}-{label}-{e}-{mode}.jsonl';assert not log.exists();at=time.monotonic()
        with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
            child=subprocess.Popen([str(programs[e]),mode,'streamed',str(state)],cwd=R,env=options,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
            event('worker_start',pid=child.pid,label=label,mode=mode);code=child.wait();event('worker_exit',pid=child.pid,exit_code=code,label=label)
        result=dict(engine=e,mode=mode,label=label,state=str(state),log=log.name,exit_code=code,wall_s=time.monotonic()-at);B.write(log.with_suffix('.process.json'),result);d['completed'].append(result);save();print(json.dumps(result),flush=True)
        if code:raise RuntimeError(f'Worker failed: {log.name}')
        return result
    try:
        for ordinal,(e,arm) in enumerate(d['sequence']):
            pair=ordinal//4;label=f'pair{pair}-{arm}';B.reset(e);state=L/f'{P}-{label}-{e}-state.json';assert not state.exists()
            se=env.copy()
            if e=='fdb':se['FDB_SEED_IDLE_CLEANUP']='1'
            run(e,'scale-seed',state,label+'-seed',se)
            mode=f'snapshot-control-{batch}' if arm=='control' else ('snapshot-one' if batch==1 else 'snapshot-100')
            run(e,mode,state,label)
            d['states'][e]=str(state);save()
            B.write(m.s.CURRENT,dict(corpus='trwiki-cap200k-hybrid4r50',source=str(m.CORPUS),sha256=d['source_sha256'],original_bodies=True,runtime_revision_mode='FDB_BENCH_50K_WRITE=1',slots={engine:dict(documents=50000,status='ready',state=path,model_sha256=B.sha(Path(path))) for engine,path in d['states'].items()}))
        # Same capped executable, old/full replacement masks: isolate mask effect
        # under actual4R+writer. Kept separate from candidate/SQLite acceptance.
        state=L/f'{P}-mask-reference-fdb-state.json';B.reset('fdb')
        se=env.copy();se['FDB_SEED_IDLE_CLEANUP']='1'
        run('fdb','scale-seed',state,'mask-reference-seed',se)
        reference=env.copy();reference['FDB_SNAPSHOT_MASK_REFERENCE']='1'
        run('fdb','snapshot-one' if batch==1 else 'snapshot-100',state,'mask-reference',reference)
        d['states']['fdb']=str(state);save()
        pointer=read(m.s.CURRENT);pointer['slots']['fdb'].update(state=str(state),documents=50000,status='ready',model_sha256=B.sha(state));B.write(m.s.CURRENT,pointer)
        if batch==100:
            state=L/f'{P}-profile-fdb-state.json';B.reset('fdb')
            se=env.copy();se['FDB_SEED_IDLE_CLEANUP']='1';run('fdb','scale-seed',state,'profile-seed',se)
            prof=env.copy();prof.update(FDB_SEQUENCE_PROFILE='1',FDB_WRITE_PROFILE_CAPACITY='4096',FDB_THREAD_PROFILE='1')
            run('fdb','snapshot-100',state,'profile',prof)
            d['states']['fdb']=str(state);save()
            pointer=read(m.s.CURRENT);pointer['slots']['fdb'].update(state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,pointer)
        idle=defaultdict(list);loaded=defaultdict(list);through=defaultdict(list);writer=defaultdict(list);memory=[];maintenance=[]
        for r in d['completed']:
            if r['label'].startswith(('mask-reference','profile')):continue
            if not r['mode'].startswith('snapshot'):continue
            for x in rows(r):
                if 'oracle' in x:assert x['oracle']
                if x['phase']=='snapshot_idle_four':
                    for v in records(x):idle[r['engine'],v['case_id']].append(v['engine_ms'])
                if x['phase']=='snapshot_load':
                    assert x['commits']==d['commits'] and x['failure'] is None
                    arm='control' if x['control'] else 'loaded';through[r['engine'],arm].append(dict(ops=x['mutations'],elapsed_ms=x['writer_elapsed_ms'],ops_s=x['ops_s']))
                    if arm=='loaded':
                        for v in records(x):
                            if v['writer_active']:loaded[r['engine'],v['case_id']].append(v['engine_ms'])
                        writer[r['engine']].extend(v['commit_ms'] for v in x['writes'])
                    memory.append(dict(engine=r['engine'],arm=arm,label=r['label'],memory=x['memory']))
                    maintenance.append(dict(engine=r['engine'],arm=arm,label=r['label'],maintenance=x['maintenance']))
        gate=dict(batch=batch,cases=[],throughput=[],failures=[],passed=False)
        for e in ['fdb','sqlite']:
            for i in range(41):
                a=idle[e,i];z=loaded[e,i]
                assert a and z,'Missing idle or active query coverage'
                initial=pct(a);observed=pct(z)
                limit=30 if initial<=30 else initial*2
                row=dict(engine=e,case_id=i,idle_n=len(a),loaded_n=len(z),idle_p95_ms=initial,loaded_p95_ms=observed,loaded_max_ms=max(z),limit_ms=limit,passed=observed<=limit)
                gate['cases'].append(row)
                if e=='fdb' and not row['passed']:gate['failures'].append(dict(kind='query',**row))
        ratios={}
        for e in ['fdb','sqlite']:
            rates={arm:sum(x['ops'] for x in through[e,arm])*1000/sum(x['elapsed_ms'] for x in through[e,arm]) for arm in ['control','loaded']}
            ratios[e]=rates['loaded']/rates['control'];gate['throughput'].append(dict(engine=e,rates=rates,loaded_control_ratio=ratios[e],samples={arm:through[e,arm] for arm in ['control','loaded']}))
        if ratios['fdb']<.75 or ratios['fdb']<=ratios['sqlite']:gate['failures'].append(dict(kind='throughput',fdb_ratio=ratios['fdb'],sqlite_ratio=ratios['sqlite'],minimum=.75))
        # The user changed the existing single-commit tail rule, not a SQLite max gate.
        gate['writer_tail']={e:dict(p95_ms=pct(xs),p99_ms=pct(xs,.99),p999_ms=pct(xs,.999),over100=sum(v>100 for v in xs),over100_ratio=sum(v>100 for v in xs)/len(xs),max_ms=max(xs)) for e,xs in writer.items()}
        if batch==1:
            x=gate['writer_tail']['fdb']
            if x['p95_ms']>20 or x['p99_ms']>50 or x['p999_ms']>100:gate['failures'].append(dict(kind='single_writer_tail',**x))
        gate['passed']=not gate['failures'];B.write(L/f'{P}-gate.json',gate);d['gate']=gate;d['memory']=memory;d['maintenance']=maintenance;d['stage']='passed' if gate['passed'] else 'STOP_failed';save();event('gate',passed=gate['passed'],failures=gate['failures'])
        d.update(protected_after=B.protected(),source_after=B.sha(m.CORPUS));assert d['protected_before']==d['protected_after'] and d['source_sha256']==d['source_after'];d['protected_unchanged']=d['source_unchanged']=True;save()
    except BaseException as error:
        d['error']=dict(type=type(error).__name__,message=str(error));save();event('driver_error',**d['error']);raise
    finally:
        stop.write_text('driver stopped',encoding='utf-8');watcher.wait(timeout=20);event('driver_finish',stage=d['stage']);stream.close()

if __name__=='__main__':main()
