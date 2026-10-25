"""Independent raw proof check; zero writes to DB/corpus/fixtures."""
import importlib,json,hashlib,re,subprocess,sys
b=importlib.import_module('measure-churn');ROOT,LOGS=b.ROOT,b.LOGS
P='revisions-cost-10k-20261025'
def load(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def main():
    protected=b.protected();designs=[]
    for suffix in ['before-profile','compare-fixed-v2','long','after-profile']:
        d=load(LOGS/f'{P}-{suffix}-design.json');assert d['complete']
        assert d['protected_before']==d['protected_after']==protected
        assert all(x['exit_code']==0 for x in d['completed'])
        for program in d['programs'].values():assert b.sha(ROOT/program['path'])==program['sha256']
        designs.append(suffix)
    sqlite=load(LOGS/f'{P}-compare-v2-design.json')
    assert sqlite['protected_before']==protected
    reference=sqlite['programs']['sqlite'];assert b.sha(ROOT/reference['path'])==reference['sha256']
    for pair in [0,1]:
        for mode in ['seed','churn','verify']:
            assert load(LOGS/f'{P}-compare-v2-r{pair}-sqlite-streamed-{mode}.process.json')['exit_code']==0
    s=load(LOGS/f'{P}-summary.json')
    assert len(s['profiles'])==20 and len(s['rounds'])==60 and len(s['long'])==30
    assert s['writer']['n']==64 and len(s['readload'])==len(s['queries'])==38
    assert all(r['oracle'] for r in s['rounds']+s['long'])
    for row in s['profiles']:
        assert abs(sum(v for k,v in row.items() if k.endswith('_percent'))-100)<1e-6
    finish=load(LOGS/f'{P}-finish.json');assert finish['status']=='complete'
    for r in finish['steps']:
        assert r['exit_code']==0 or any(x['label']==r.get('superseded_by') and x['exit_code']==0 for x in finish['steps']),r
    assert any(r['label']=='old-reader' and r['exit_code']==0 for r in finish['steps'])
    kills=load(LOGS/'revisions-cost-kill-10k-20261025-summary.json');assert len(kills['results'])==7
    for r in kills['results']:
        if 'mode' in r:
            assert r['oracle'] and r['verify_exit_code']==0 and r['marker_oracle']['all_passed']
        else:
            assert r['coverage_observed'] and r['reopen_full_payload_matrix_oracle_exit_code']==0 and r['pending_old_new_marker_oracle']['all_passed']
    for program in kills['programs']:assert b.sha(ROOT/program['path'])==program['sha256']
    assert b.protected()==protected
    assert not subprocess.check_output(['git','diff','f7788ca','--','Cargo.toml','Cargo.lock'],cwd=ROOT)
    diff=subprocess.check_output(['git','diff','-U0','f7788ca','--','src/format.rs','src/format','src/manifest.rs','src/segment.rs','src/segment'],cwd=ROOT).decode('utf-8')
    magic=[line for line in diff.splitlines() if line[:1] in '+-' and not line.startswith(('+++','---')) and re.search(r'const .*MAGIC|const .*FOOTER|b"FDB|b"TXN',line)]
    assert not magic,magic
    assert 'revision_reclamation: false,' in (ROOT/'src/engine.rs').read_text()
    assert 'SourceLease::immutable' in (ROOT/'src/engine/revisions.rs').read_text()
    assert 'Builder::from_native_records' in (ROOT/'src/engine/revisions.rs').read_text()
    proof=dict(status='complete',protected_unchanged=True,designs=designs,frozen_programs_verified=True,sqlite_frozen_two_runs_verified=True,source_magic_changes=magic,dependencies_unchanged=True,default_enabled=False,gates=s['gates'],case34='tracked',kill_cases=7,active_write_samples=64,loaded_read_samples=760,raw_profiles=20)
    b.write(LOGS/f'{P}-proof.json',proof)
    if '--staged' in sys.argv:
        blobs=subprocess.check_output(['git','ls-files','--stage','-z','--','docs/benchmarks/revisions-cost*'],cwd=ROOT)
        verified=[]
        for row in blobs.split(b'\0'):
            if not row:continue
            header,path=row.split(b'\t',1);name=path.decode('utf-8')
            if name.endswith('-staged-byte-proof.json'):continue
            content=(ROOT/name).read_bytes()
            actual=hashlib.sha1(b'blob '+str(len(content)).encode()+b'\0'+content).hexdigest()
            assert actual==header.split()[1].decode(),name
            verified.append(dict(path=name,git_blob=actual,sha256=hashlib.sha256(content).hexdigest()))
        b.write(LOGS/f'{P}-staged-byte-proof.json',dict(verified=verified,count=len(verified),raw_bytes_preserved=True))
    print(json.dumps(proof,indent=2))
if __name__=='__main__':main()
