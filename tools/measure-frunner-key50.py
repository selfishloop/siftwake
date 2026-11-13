"""Four balanced numeric/Frunner pool blocks; existing50k owner DBs only."""
import importlib.util
import hashlib
import json
import os
import subprocess
import sqlite3
import time
import zlib
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='frunner-key50-20261102'
spec=importlib.util.spec_from_file_location('base',R/'tools/measure-revised50-profile.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m,B=c.m,c.B


def path(slot):
    size=1000 if slot==256 else [40,80,160,300][slot//16%4]
    prefix=f'C:\\pool\\ş\\{slot:04}\\';tail='\\file.txt'
    return prefix+'a'*(size-len(prefix.encode())-len(tail))+tail


def key(variant,row):
    ident=f'{row["identity"]:010}' if variant=='numeric' else f'{(~row["mtime"]) & ((1<<64)-1):016x}|{path(row["slot"])}'
    return dict(scope='shared',collection='keybench-'+('numeric' if variant=='numeric' else 'frunner'),id=ident)


def old_pool(state,prefix):
    saved=m.read(state);binary=R/'output/maintenance-candidate-cli-20261024.exe'
    with (L/f'{prefix}-old-reader.stderr').open('x',encoding='utf-8') as err:
        child=subprocess.Popen([str(binary),str(R/'data/benchmark-work/fdb'),'--stdio'],cwd=R,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW)
        def request(value):
            child.stdin.write(json.dumps(value)+'\n');child.stdin.flush();response=json.loads(child.stdout.readline());assert response['ok'],response;return response['result']
        expected=[]
        try:
            with m.CORPUS.open('rb') as corpus:
                for row in saved['rows']:
                    entry=saved['entries'][row['slot']%256];corpus.seek(entry['offset']);raw=corpus.read(entry['size']);assert zlib.crc32(raw)==entry['crc']
                    doc=json.loads(raw)['document'];doc.setdefault('title','');doc.setdefault('fields',{});doc.setdefault('links',[]);doc['key']=key(saved['variant'],row)
                    doc['text']=doc['text'].encode()[:199936].decode('utf-8',errors='ignore')+f' fdbkeypoolmark zqkeyp{row["slot"]:04}r{row["revision"]:08}zq'
                    wanted=dict(document=doc,version=row['version']);assert request(dict(op='get',key=doc['key']))==wanted;expected.append(wanted)
            page=request(dict(op='search',query=dict(scope='shared',collection=key(saved['variant'],saved['rows'][0])['collection'],text='fdbkeypoolmark',offset=0,limit=50)))
            assert page['sequence']==saved['sequence'] and page['total']==257
            assert page['hits']==sorted(expected,key=lambda x:x['document']['key']['id'].encode())[:50]
            child.stdin.close();assert child.wait(timeout=20)==0
        finally:
            if child.poll() is None:child.kill();child.wait()
    result=dict(oracle=True,documents=257,first50_order=True,first50_text_bytes=sum(len(x['document']['text'].encode()) for x in sorted(expected,key=lambda x:x['document']['key']['id'].encode())[:50]),long_path_bytes=1000,program_sha256=B.sha(binary))
    B.write(L/f'{prefix}-old-reader.json',result);return result


def main(diagnostic_once=False):
    dest=L/f'{P}-design.json';assert not dest.exists(),'Inspect existing run; never replay silently'
    pointer=m.read(m.s.CURRENT);assert all(x['documents']==50000 and x['status']=='ready' for x in pointer['slots'].values())
    assert B.sha(m.CORPUS)==pointer['sha256']=='881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12'
    programs={k:R/f'output/{P}-{k}.exe' for k in ['fdb','sqlite']}
    catalog=m.read(m.CATALOG);selected=[]
    for lo,hi in [(2048,4095),(4096,16383),(16384,65535),(65536,200000)]:
        entries=[(i,e) for i,e in enumerate(catalog['entries'][:50000]) if lo<=e['text_bytes']<=hi]
        assert len(entries)>=64
        # Include a longest donor, keeping donor identity and size distribution explicit.
        longest=max(entries,key=lambda x:x[1]['text_bytes'])
        chosen=[x for x in entries if x[0]!=longest[0]][:63]+[longest]
        assert len({i for i,_ in chosen})==64
        selected.extend(dict(body=i,**{k:e[k] for k in ['offset','size','crc','text_bytes']}) for i,e in chosen)
    order=[('fdb','numeric'),('sqlite','numeric'),('sqlite','frunner'),('fdb','frunner')]
    d=dict(stage='running',completed=[],initial_slots=pointer,donors=selected,corpus_sha256=B.sha(m.CORPUS),protected_before=B.protected(),
           programs={k:dict(path=str(p),sha256=B.sha(p)) for k,p in programs.items()},
           engine_sources={str(p.relative_to(R)):B.sha(p) for p in sorted((R/'src').rglob('*.rs'))},
           blocks=[order] if diagnostic_once else [order,list(reversed(order))]*2,reset=False,new_corpus=False,full_50k_rekey=False,
           default='single try/serial; no diagnostic override',key_collections=['keybench-numeric','keybench-frunner'],
           samples='64/type/path/body cell;4096 normal+1 edge commits/arm; idle1R60,idle4R15/actor,loaded>=60 active/query',
           estimator='nearest rank ceil(p*N); raw p50/p95/p99/max; no trim/bootstrap',
           gates=dict(short_commit_visibility='p95<=20,p99<=50,max<=100 ms',reads='idle<=30:loaded<=30;else loaded<=2*idle'),
           cache='warm/uncontrolled OS; no eviction',ram='process includes pool templates/folded oracle/history/results; not engine-only',note='selected pool only; existing50k texts never rekeyed/ingested; physical layout evolves')
    report_script=R/'tools'/('report-visibility-phase50.py' if diagnostic_once else 'report-frunner-key50.py')
    if diagnostic_once:
        d.update(diagnostic_once=True,gates=None,note='single diagnostic block, not an acceptance run; same pool/cohort/load; no optimization')
    B.write(dest,d);life=(L/f'{P}-lifecycle.jsonl').open('x',encoding='utf-8')
    def main_sql_digest():
        digest=hashlib.sha256();n=0
        with sqlite3.connect(f'file:{(R/"data/benchmark-work/sqlite/sqlite.db").as_posix()}?mode=ro',uri=True) as db:
            for ident,payload in db.execute("SELECT id,payload FROM docs WHERE scope='shared' AND collection='texts' ORDER BY id"):
                digest.update(ident.encode()+b'\0'+payload.encode()+b'\0');n+=1
        assert n==50000;return dict(documents=n,payload_sha256=digest.hexdigest().upper(),scope='read-only audit of existing docs table; not performance SQLite engine')
    def event(kind,**v):life.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');life.flush();os.fsync(life.fileno())
    env=c.env()
    for k in list(env):
        if k.startswith('FDB_') and k not in ['FDB_BENCH_CORPUS_PATH']:env.pop(k)
    env['FDB_BENCH_CORPUS_PATH']=str(m.CORPUS)
    if diagnostic_once:env['POOL_VISIBILITY_PROFILE']='1'
    mainstates={}
    for engine in programs:
        mainstates[engine]=L/f'{P}-main-{engine}-state.json';B.write(mainstates[engine],m.read(Path(pointer['slots'][engine]['state'])))
    def child(engine,args,label):
        log=L/f'{P}-{label}.jsonl';assert not log.exists();start=time.monotonic()
        with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
            p=subprocess.Popen([str(programs[engine]),*map(str,args)],cwd=R,env=env,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
            event('worker_start',pid=p.pid,engine=engine,label=label);code=p.wait();event('worker_exit',pid=p.pid,engine=engine,label=label,exit_code=code)
        record=dict(engine=engine,label=label,log=log.name,exit_code=code,wall_s=time.monotonic()-start)
        B.write(log.with_suffix('.process.json'),record)
        if code:raise RuntimeError(f'Child failed; retain all evidence: {log.name}')
        return record
    def clean(engine,label):
        rec=child(engine,['cleanup'],label);seq=json.loads((L/rec['log']).read_text(encoding='utf-8'))['sequence']
        model=m.read(mainstates[engine]);model['sequence']=seq;B.write(mainstates[engine],model)
        ptr=m.read(m.s.CURRENT);ptr['slots'][engine].update(state=str(mainstates[engine]),model_sha256=B.sha(mainstates[engine]),status='ready');B.write(m.s.CURRENT,ptr)
        return rec
    pool_kills=False
    try:
        event('driver_start',pid=os.getpid())
        d['main_sql_before']=main_sql_digest();B.write(dest,d)
        for engine in programs:
            rec=child(engine,['inspect'],engine+'-initial-inspect');inspection=json.loads((L/rec['log']).read_text());assert inspection['numeric']==inspection['frunner']==0,'Pool not owned/empty'
        # Same main50k oracle and witnessed recovery as previous steps.
        subprocess.run([os.sys.executable,str(R/'tools/verify-profile-recovery.py'),'--prefix',P+'-pre','--state',str(mainstates['fdb']),'--program',str(R/'output/hybrid-default50-20261102-fdb.exe')],cwd=R,env={**os.environ,'PYTHONUTF8':'1'},check=True)
        for block,arms in enumerate(d['blocks']):
            for engine,variant in arms:
                label=f'b{block}-{engine}-{variant}';state=L/f'{P}-{label}-pool-state.json';cfg=L/f'{P}-{label}-config.json'
                B.write(cfg,dict(variant=variant,entries=selected,output_state=str(state)))
                ptr=m.read(m.s.CURRENT);ptr['slots'][engine]['status']='measuring_key_pool';B.write(m.s.CURRENT,ptr)
                run=child(engine,[cfg],label);run.update(block=block,variant=variant,state=str(state))
                if engine=='fdb':
                    run['old_reader']=old_pool(state,f'{P}-{label}')
                    if variant=='frunner' and not pool_kills:
                        kills=[]
                        source_before={p.name:B.sha(p) for p in (R/'data/benchmark-work/fdb').iterdir() if p.suffix in ('.fdb','.ready')}
                        for n in range(3):
                            with (L/f'{P}-pool-kill{n}.stderr').open('x',encoding='utf-8') as err:
                                p=subprocess.Popen([str(programs['fdb']),'hold',str(state)],cwd=R,env=env,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW)
                                event('worker_start',pid=p.pid,label=f'pool-kill{n}');witness=json.loads(p.stdout.readline());assert witness['phase']=='pool_verified' and witness['oracle']
                                event('intentional_kill',pid=p.pid,witness=witness);p.kill();code=p.wait(timeout=10);event('worker_exit',pid=p.pid,exit_code=code,label=f'pool-kill{n}');assert code!=0;kills.append(dict(witness=witness,exit_code=code))
                        child('fdb',['verify',state],'pool-reopen')
                        source_after={p.name:B.sha(p) for p in (R/'data/benchmark-work/fdb').iterdir() if p.suffix in ('.fdb','.ready')}
                        assert source_after==source_before
                        B.write(L/f'{P}-pool-recovery.json',dict(kills=kills,source_hashes_unchanged=True,old_reader=old_pool(state,P+'-pool-postkill'),scope='after strict open +257 GET +5 query oracle; no in-flight transaction/power-loss claim'))
                        pool_kills=True
                run['cleanup']=clean(engine,label+'-cleanup');d['completed'].append(run);B.write(dest,d)
                subprocess.run([os.sys.executable,str(report_script)],cwd=R,check=True)
                print(json.dumps(dict(completed=len(d['completed']),block=block,engine=engine,variant=variant)),flush=True)
        subprocess.run([os.sys.executable,str(R/'tools/verify-profile-recovery.py'),'--prefix',P+'-post','--state',str(mainstates['fdb']),'--program',str(R/'output/hybrid-default50-20261102-fdb.exe')],cwd=R,env={**os.environ,'PYTHONUTF8':'1'},check=True)
        d.update(stage='completed',protected_after=B.protected(),corpus_after=B.sha(m.CORPUS),main_sql_after=main_sql_digest())
        assert d['main_sql_before']==d['main_sql_after']
        assert d['protected_after']==d['protected_before'] and d['corpus_after']==d['corpus_sha256']
        assert d['engine_sources']=={str(p.relative_to(R)):B.sha(p) for p in sorted((R/'src').rglob('*.rs'))}
        assert all(B.sha(Path(p['path']))==p['sha256'] for p in d['programs'].values())
        B.write(dest,d);subprocess.run([os.sys.executable,str(report_script)],cwd=R,check=True)
    except BaseException as error:
        d.update(stage='failed',error=repr(error));B.write(dest,d);event('failure',error=repr(error));raise
    finally:event('driver_finish',stage=d['stage']);life.close()


if __name__=='__main__':main()
