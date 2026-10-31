"""RAM from a dedicated engine child; corpus/catalog/model stay in parent."""
import importlib.util,json,os,subprocess,zlib,shutil,sys
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='engine-only50-20261030'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m=c.m;B=c.B
def main():
    dest=L/f'{P}-design.json';assert not dest.exists();exe=R/'output/engine-only50-20261030.exe'
    d=dict(stage='running',program_sha256=B.sha(exe),source_sha256=B.sha(m.CORPUS),protected_before=B.protected(),completed=[],roles=['control','candidate','candidate','control'],scope='dedicated engine process + runtime + one bounded input document; no model/oracle/catalog/history/profile in child; no baseline subtraction',control='same binary, full replacement mask reference; candidate incremental masks; snapshot reader enabled, no retained snapshot',idle='fresh strict-open child after maintained ingest, not allocator retained after ingest')
    B.write(dest,d);catalog=m.read(m.CATALOG)['entries'];life=(L/f'{P}-lifecycle.jsonl').open('x',encoding='utf-8')
    def event(kind,**v):life.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');life.flush();os.fsync(life.fileno())
    try:
        event('driver_start',pid=os.getpid())
        previous=Path(m.read(m.s.CURRENT)['slots']['fdb']['state'])
        subprocess.run([sys.executable,str(R/'tools/verify-profile-recovery.py'),'--prefix',P+'-profile-before-reset','--state',str(previous),'--program',str(R/'output/delta-cap-20261030.exe')],cwd=R,check=True)
        for i,role in enumerate(d['roles']):
            B.reset('fdb');ptr=m.read(m.s.CURRENT);ptr['slots']['fdb'].update(status='preparing_engine_only',state=None);B.write(m.s.CURRENT,ptr)
            env=os.environ.copy();env.pop('FDB_QUERY_PROFILE',None);env.pop('FDB_SEQUENCE_PROFILE',None);env.pop('FDB_SNAPSHOT_MASK_REFERENCE',None)
            if role=='control':env['FDB_SNAPSHOT_MASK_REFERENCE']='1'
            for mode in ['ingest','idle']:
                path=L/f'{P}-{i}-{role}-{mode}.jsonl';assert not path.exists()
                with path.open('x',encoding='utf-8') as out,path.with_suffix('.stderr').open('x',encoding='utf-8') as err:
                    child=subprocess.Popen([str(exe),mode],cwd=R,env=env,stdin=subprocess.PIPE if mode=='ingest' else subprocess.DEVNULL,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
                    event('worker_start',pid=child.pid,role=role,mode=mode)
                    if mode=='ingest':
                        with m.CORPUS.open('rb') as corpus:
                            for n,e in enumerate(catalog[:50000]):
                                corpus.seek(e['offset']);raw=corpus.read(e['size']);assert zlib.crc32(raw)==e['crc'];doc=json.loads(raw)['document'];doc['key']=dict(scope='shared',collection='texts',id=f'{n:010}')
                                child.stdin.write((json.dumps(doc,ensure_ascii=False,separators=(',',':'))+'\n').encode('utf-8'))
                        child.stdin.close()
                    code=child.wait();event('worker_exit',pid=child.pid,exit_code=code,role=role,mode=mode);assert code==0
                row=json.loads(path.read_text(encoding='utf-8'));d['completed'].append(dict(arm=i,role=role,mode=mode,log=path.name,measurement=row));B.write(dest,d)
            state=L/f'{P}-{i}-{role}-state.json';B.write(state,dict(rows={str(n):dict(body=n,revision=0,version=n//500+1) for n in range(50000)},dead={},next=50000,sequence=100))
            ptr=m.read(m.s.CURRENT);ptr['slots']['fdb'].update(documents=50000,status='ready',state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,ptr)
            # Oracle outside engine-RAM child and outside its memory counters.
            c.run_child(R/'output/delta-cap-20261030.exe','write50-open',state,c.env(),f'{P}-{i}-{role}-oracle',event)
            oracle=c.env();oracle.update(FDB_SNAPSHOT_IDLE_ONLY='1',FDB_SNAPSHOT_IDLE_READERS='1',FDB_SNAPSHOT_ROUNDS='1')
            c.run_child(R/'output/delta-cap-20261030.exe','snapshot-one',state,oracle,f'{P}-{i}-{role}-query-oracle',event)
        subprocess.run([sys.executable,str(R/'tools/verify-profile-recovery.py'),'--prefix',P+'-final','--state',str(state),'--program',str(R/'output/delta-cap-20261030.exe')],cwd=R,check=True)
        d.update(stage='completed',protected_after=B.protected(),source_after=B.sha(m.CORPUS));assert d['protected_after']==d['protected_before'] and d['source_after']==d['source_sha256']
        d['gates']=[dict(arm=x['arm'],role=x['role'],mode=x['mode'],mib=(x['measurement']['memory']['peak_private_commit_bytes'] if x['mode']=='ingest' else x['measurement']['memory']['private_bytes'])/1048576,limit_mib=60 if x['mode']=='ingest' else 15) for x in d['completed']]
        for x in d['gates']:x['passed']=x['mib']<=x['limit_mib']
        d['passed']=all(x['passed'] for x in d['gates']);B.write(dest,d)
    finally:event('driver_finish',stage=d['stage']);life.close()
if __name__=='__main__':main()
