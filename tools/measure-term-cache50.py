"""One final K/A/SQLite/D Williams block. Existing owner slots; never reset."""
import ctypes,importlib.util,json,os,shutil,subprocess,threading,time
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='term-cache50-20261102'
spec=importlib.util.spec_from_file_location('pool',R/'tools/measure-frunner-key50.py')
pool=importlib.util.module_from_spec(spec);spec.loader.exec_module(pool)
c,m,B=pool.c,pool.m,pool.B
ORDER=[['K','A','S','D'],['A','D','K','S'],['D','S','A','K'],['S','K','D','A']]
EXE=R/f'output/{P}-fdb.exe';POOL=R/f'output/{P}-pool-fdb.exe';RAM=R/f'output/{P}-ram-fdb.exe'
SQL=R/'output/estimator50-20261031-sqlite.exe';SQLPOOL=R/f'output/{P}-pool-sqlite.exe'

def main():
    destination=L/f'{P}-design.json';assert not destination.exists(),'Inspect state; never silently replay'
    ptr=m.read(m.s.CURRENT);assert all(x['status']=='ready' and x['documents']==50000 for x in ptr['slots'].values())
    assert B.sha(m.CORPUS)==ptr['sha256']=='881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12'
    d=dict(stage='running',completed=[],order=ORDER,corpus_sha256=ptr['sha256'],protected_before=B.protected(),
           initial_slots=ptr,programs={str(p):B.sha(p) for p in [EXE,POOL,RAM,SQL,SQLPOOL]},
           engine_sources={str(p.relative_to(R)):B.sha(p) for p in sorted((R/'src').rglob('*.rs'))},
           default='K, try/serial',reset=False,ingest=False,new_db_copy=False,
           samples='normal41 N60; pool4096+1/arm, sampled profiles stride64 excluded from primary wall',
           timing='wall counters disabled except explicitly attached sampled request; bounded cache counters identical K/A/D',
           cache='warm/uncontrolled OS, no eviction',gates='unchanged; complete fixed block unless correctness fails; no second round')
    B.write(destination,d)
    life=(L/f'{P}-lifecycle.jsonl').open('x',encoding='utf-8')
    def event(kind,**values):life.write(json.dumps(dict(utc=m.utc(),kind=kind,**values))+'\n');life.flush();os.fsync(life.fileno())
    def env(arm):
        e=c.env()
        for key in ['FDB_HYBRID_WAIT_REFERENCE','FDB_QUERY_PROFILE','FDB_SEQUENCE_PROFILE','FDB_SNAPSHOT_CASES','FDB_SNAPSHOT_IDLE_ONLY','FDB_SNAPSHOT_MASK_REFERENCE','FDB_HYBRID_WORKERS','POOL_VISIBILITY_PROFILE']:
            e.pop(key,None)
        e.update(FDB_TERM_CACHE_ARM='K' if arm=='S' else arm,FDB_TIMING_DISABLED='1',FDB_THREAD_PROFILE='1',FDB_SNAPSHOT_ROUNDS='15',FDB_SNAPSHOT_MIN_ACTIVE_ROUNDS='15',FDB_SNAPSHOT_MAX_COMMITS='262144',FDB_SNAPSHOT_ESTIMATOR_PROGRESS='1')
        return e
    def process(program,args,label,e):
        log=L/f'{P}-{label}.jsonl';assert not log.exists();start=time.monotonic();samples=[];done=threading.Event()
        with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
            child=subprocess.Popen([str(program),*map(str,args)],cwd=R,env=e,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
            event('worker_start',pid=child.pid,label=label)
            def observe():
                while not done.is_set():
                    try:
                        samples.append(dict(elapsed_s=time.monotonic()-start,threads=c.thread_count(child.pid)))
                    except OSError:pass
                    done.wait(.05)
            watcher=threading.Thread(target=observe);watcher.start();code=child.wait();done.set();watcher.join()
            times=[ctypes.c_uint64() for _ in range(4)];k=ctypes.WinDLL('kernel32',use_last_error=True)
            k.GetProcessTimes.argtypes=[ctypes.c_void_p,*([ctypes.POINTER(ctypes.c_uint64)]*4)]
            assert k.GetProcessTimes(int(child._handle),*[ctypes.byref(x) for x in times])
            cpu_ms=(times[2].value+times[3].value)/10000
            event('worker_exit',pid=child.pid,label=label,exit_code=code)
        record=dict(log=log.name,exit_code=code,wall_s=time.monotonic()-start,cpu_ms_all_process_threads=cpu_ms,
                    logical_processors=os.cpu_count(),thread_peak=max((x['threads'] for x in samples),default=0),thread_samples=samples)
        B.write(log.with_suffix('.process.json'),record)
        if code:raise RuntimeError(f'correctness/process failure; stop: {log.name}')
        return record
    def save(run):
        d['completed'].append(run);B.write(destination,d)
        subprocess.run([os.sys.executable,str(R/'tools/report-term-cache50.py')],cwd=R,check=True)
        print(json.dumps({k:v for k,v in run.items() if k in ['phase','pair','arm','log','wall_s']}),flush=True)
    def recovery(arm,state,label):
        subprocess.run([os.sys.executable,str(R/'tools/verify-profile-recovery.py'),'--prefix',P+'-'+label,'--state',str(state),'--program',str(EXE)],cwd=R,env=env(arm),check=True)
    queryfile=L/f'{P}-ram-queries.json';B.write(queryfile,[x['text'] for x in m.read(m.CATALOG)['cases']])
    try:
        event('driver_start',pid=os.getpid());recovery('A',Path(ptr['slots']['fdb']['state']),'pre-A')
        # Existing reader-free reference for the unchanged write retention gate.
        for arm in ORDER[0]:
            engine='sqlite' if arm=='S' else 'fdb';ptr=m.read(m.s.CURRENT)
            state=L/f'{P}-control-{arm}-state.json';shutil.copyfile(ptr['slots'][engine]['state'],state)
            ptr['slots'][engine].update(status='term_cache_control',state=str(state));B.write(m.s.CURRENT,ptr)
            r=process(SQL if arm=='S' else EXE,['snapshot-control-1','streamed',state],f'control-{arm}',env(arm))
            r.update(phase='control',pair=-1,arm=arm,state=str(state));ptr=m.read(m.s.CURRENT);ptr['slots'][engine].update(status='ready',model_sha256=B.sha(state));B.write(m.s.CURRENT,ptr);save(r)
        for pair,order in enumerate(ORDER):
            for arm in order:
                engine='sqlite' if arm=='S' else 'fdb';ptr=m.read(m.s.CURRENT)
                state=L/f'{P}-normal-{pair}-{arm}-state.json';shutil.copyfile(ptr['slots'][engine]['state'],state)
                ptr['slots'][engine].update(status='term_cache_normal',state=str(state));B.write(m.s.CURRENT,ptr)
                r=process(SQL if arm=='S' else EXE,['snapshot-one','streamed',state],f'normal-{pair}-{arm}',env(arm))
                # Existing N60 coverage + oracle auditor, unchanged estimator.
                es=importlib.util.spec_from_file_location('est',R/'tools/measure-estimator50.py');est=importlib.util.module_from_spec(es);es.loader.exec_module(est)
                r['coverage']=est.coverage(L/r['log'],False)
                r.update(phase='normal',pair=pair,arm=arm,state=str(state));ptr=m.read(m.s.CURRENT);ptr['slots'][engine].update(status='ready',model_sha256=B.sha(state));B.write(m.s.CURRENT,ptr);save(r)
                if arm!='S':
                    e=env(arm);e['FDB_RAM_QUERY_LIST']=str(queryfile)
                    for mode in ['idle','idle-warm']:
                        x=process(RAM,[mode],f'ram-{pair}-{arm}-{mode}',e);x.update(phase='ram',pair=pair,arm=arm,mode=mode);save(x)
        ptr=m.read(m.s.CURRENT)
        mainstates={engine:Path(ptr['slots'][engine]['state']) for engine in ['fdb','sqlite']}
        recovery('A',mainstates['fdb'],'post-normal-A')
        donors=m.read(L/'frunner-key50-20261102-design.json')['donors'];d['donors']=donors;B.write(destination,d)
        for pair,order in enumerate(ORDER):
            for arm in order:
                engine='sqlite' if arm=='S' else 'fdb';e=env(arm)
                e.update(POOL_VISIBILITY_PROFILE='1',POOL_PROFILE_STRIDE='64')
                state=L/f'{P}-pool-{pair}-{arm}-state.json';config=L/f'{P}-pool-{pair}-{arm}-config.json'
                B.write(config,dict(variant='frunner',entries=donors,output_state=str(state)))
                ptr=m.read(m.s.CURRENT);ptr['slots'][engine]['status']='term_cache_pool';B.write(m.s.CURRENT,ptr)
                r=process(SQLPOOL if arm=='S' else POOL,[config],f'pool-{pair}-{arm}',e)
                r.update(phase='pool',pair=pair,arm=arm,state=str(state))
                if arm!='S':r['old_reader']=pool.old_pool(state,f'{P}-pool-{pair}-{arm}')
                cleanup=process(SQLPOOL if arm=='S' else POOL,['cleanup'],f'cleanup-{pair}-{arm}',e)
                logs=[json.loads(x) for x in (L/cleanup['log']).read_text().splitlines()]
                seq=next(x['sequence'] for x in logs if x['phase']=='cleanup')
                model=m.read(mainstates[engine]);model['sequence']=seq;B.write(mainstates[engine],model)
                ptr=m.read(m.s.CURRENT);ptr['slots'][engine].update(status='ready',state=str(mainstates[engine]),model_sha256=B.sha(mainstates[engine]));B.write(m.s.CURRENT,ptr)
                r['cleanup']=cleanup;save(r)
        for arm in ['K','A','D']:recovery(arm,mainstates['fdb'],'final-'+arm)
        d.update(stage='completed',protected_after=B.protected(),corpus_after=B.sha(m.CORPUS))
        assert d['protected_after']==d['protected_before'] and d['corpus_after']==d['corpus_sha256']
        assert d['engine_sources']=={str(p.relative_to(R)):B.sha(p) for p in sorted((R/'src').rglob('*.rs'))}
        assert all(B.sha(Path(p))==sha for p,sha in d['programs'].items())
    except BaseException as error:
        d.update(stage='stopped',error=repr(error));event('failure',error=repr(error));raise
    finally:
        B.write(destination,d);event('driver_finish',stage=d['stage']);life.close()
        subprocess.run([os.sys.executable,str(R/'tools/report-term-cache50.py')],cwd=R,check=True)
if __name__=='__main__':main()
