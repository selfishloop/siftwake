"""Same-binary isolated engine RAM, before/after, including native4R+writer."""
import importlib.util,json,os,subprocess,zlib
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='side-cost-ram50-20261031'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m=c.m;B=c.B
def main():
    subprocess.run([os.sys.executable,str(R/'tools/measure-side-cost-broad50.py')],cwd=R,check=True)
    dest=L/f'{P}-design.json';assert not dest.exists();assert m.read(L/'side-cost-cpu50-20261031-design.json')['stage']=='completed';exe=R/'output/hybrid-serial-ram-20261030.exe';normal=R/'output/estimator50-20261031-fdb.exe';catalog=m.read(m.CATALOG);assert len(catalog['cases'])==41 and all(x['offset']==0 and x['limit']==50 for x in catalog['cases'])
    querylist=L/f'{P}-queries.json';B.write(querylist,[x['text'] for x in catalog['cases']]);entries=catalog['entries'][:50000]
    d=dict(stage='running',program_sha256=B.sha(exe),source_sha256=B.sha(m.CORPUS),protected_before=B.protected(),completed=[],roles=['before','after','after','before']*2,scope='isolated engine+runtime+one input doc+8KiB stdin;4R mode adds41 query strings and API-owned results; no model/oracle/history/profiling inside; no subtraction',reset_reason='ingest requires empty fixed owner slot; main acceptance/CPU run must finish first')
    B.write(dest,d);life=(L/f'{P}-lifecycle.jsonl').open('x',encoding='utf-8')
    def event(kind,**v):life.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');life.flush();os.fsync(life.fileno())
    def document(corpus,n):
        e=entries[n];corpus.seek(e['offset']);raw=corpus.read(e['size']);assert zlib.crc32(raw)==e['crc'];doc=json.loads(raw)['document'];doc['key']=dict(scope='shared',collection='texts',id=f'{n:010}');return doc
    try:
        event('driver_start',pid=os.getpid());previous=Path(m.read(m.s.CURRENT)['slots']['fdb']['state'])
        subprocess.run([os.sys.executable,str(R/'tools/verify-profile-recovery.py'),'--prefix',P+'-main-before-reset','--state',str(previous),'--program',str(normal)],cwd=R,check=True)
        for i,role in enumerate(d['roles']):
            B.reset('fdb');ptr=m.read(m.s.CURRENT);ptr['slots']['fdb'].update(status='preparing_ram',state=None);B.write(m.s.CURRENT,ptr)
            model=dict(rows={str(n):dict(body=n,revision=0,version=n//500+1) for n in range(50000)},dead={},next=50000,sequence=100)
            env=os.environ.copy();env.pop('FDB_QUERY_PROFILE',None);env.pop('FDB_SNAPSHOT_MASK_REFERENCE',None);env.pop('FDB_HYBRID_WAIT_REFERENCE',None);env['FDB_RAM_QUERY_LIST']=str(querylist)
            if role=='before':env['FDB_HYBRID_WAIT_REFERENCE']='1'
            for mode in ['ingest','idle','loaded','idle']:
                phase='postloaded_idle' if mode=='idle' and model['sequence']!=100 else mode;log=L/f'{P}-{i}-{role}-{phase}.jsonl';assert not log.exists()
                with log.open('x',encoding='utf-8') as out,log.with_suffix('.stderr').open('x',encoding='utf-8') as err:
                    child=subprocess.Popen([str(exe),mode],cwd=R,env=env,stdin=subprocess.PIPE if mode in ['ingest','loaded'] else subprocess.DEVNULL,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW);event('worker_start',pid=child.pid,mode=phase,role=role)
                    if mode in ['ingest','loaded']:
                        with m.CORPUS.open('rb') as corpus:
                            ids=list(range(50000)) if mode=='ingest' else [n for n,e in enumerate(entries) if 2048<=e['text_bytes']<=199960][:1100]
                            cache={} if mode=='ingest' else {n:document(corpus,n) for n in ids}
                            for at in range(50000 if mode=='ingest' else 16384):
                                n=ids[at] if mode=='ingest' else ids[at%len(ids)];doc=document(corpus,n) if mode=='ingest' else dict(cache[n])
                                if mode=='loaded':
                                    row=model['rows'][str(n)];row['revision']+=1;row['version']=101+at;model['sequence']=101+at;doc['text']=doc['text']+' '+f'zqv{n:010}r{row["revision"]:010}zq'
                                child.stdin.write((json.dumps(doc,ensure_ascii=False,separators=(',',':'))+'\n').encode('utf-8'))
                        child.stdin.close()
                    code=child.wait();event('worker_exit',pid=child.pid,mode=phase,role=role,exit_code=code);assert code==0
                d['completed'].append(dict(arm=i,role=role,mode=phase,log=log.name,measurement=m.read(log)));B.write(dest,d)
            state=L/f'{P}-{i}-{role}-state.json';B.write(state,model);ptr=m.read(m.s.CURRENT);ptr['slots']['fdb'].update(documents=50000,status='ready',state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,ptr)
            c.run_child(normal,'write50-open',state,c.env(),f'{P}-{i}-{role}-oracle',event)
            oracle=c.env();oracle.update(FDB_SNAPSHOT_IDLE_ONLY='1',FDB_SNAPSHOT_IDLE_READERS='1',FDB_SNAPSHOT_ROUNDS='1');c.run_child(normal,'snapshot-one',state,oracle,f'{P}-{i}-{role}-query-oracle',event)
        subprocess.run([os.sys.executable,str(R/'tools/verify-profile-recovery.py'),'--prefix',P+'-final','--state',str(state),'--program',str(normal)],cwd=R,check=True)
        d.update(stage='completed',protected_after=B.protected(),source_after=B.sha(m.CORPUS));assert d['protected_before']==d['protected_after'] and d['source_sha256']==d['source_after'];B.write(dest,d)
    finally:event('driver_finish',stage=d['stage']);life.close()
if __name__=='__main__':main()
