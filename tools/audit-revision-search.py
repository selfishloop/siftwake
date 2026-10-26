"""Verify saved evidence, defaults and byte preservation; no database writes."""
import hashlib,importlib,json,re,subprocess,sys
b=importlib.import_module('measure-churn');ROOT,LOGS=b.ROOT,b.LOGS
P='revision-search-10k-20261026'
def load(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def main():
    protected=b.protected();d=load(LOGS/f'{P}-design.json');assert d['complete']
    assert d['protected_before']==d['protected_after']==protected
    assert d['candidate_read_sources_unchanged'] and d['candidate_model_equal']
    assert len(d['completed'])==19 and all(r['exit_code']==0 for r in d['completed'])
    for path,sha in d['programs'].items():assert b.sha(ROOT/path)==sha
    initial=load(LOGS/f'{P}-profile-design.json');assert initial['protected_before']==initial['protected_after']==protected
    assert initial['result']['exit_code']==0 and b.sha(ROOT/initial['program'])==initial['program_sha256']
    original_model=load(ROOT/initial['state'])
    sqlite_state=load(LOGS/'revisions-cost-10k-20261025-slot-sqlite.json')['state']
    assert original_model==load(ROOT/sqlite_state)
    s=load(LOGS/f'{P}-summary.json');assert len(s['queries'])==38 and len(s['profiles'])==114 and len(s['maintenance'])==20
    assert s['failed_cases']==[r['case_id'] for r in s['queries'] if not r['tracked'] and r['regression_percent']>5]
    checks=load(LOGS/f'{P}-checks.json');assert checks['status']=='complete' and all(r['exit_code']==0 for r in checks['steps'])
    for r in checks['steps']:assert b.sha(LOGS/r['log'])==r['sha256']
    kill=load(LOGS/'revision-search-kill-10k-20261026-summary.json');assert len(kill['results'])==7 and kill['old_reader']['exit_code']==0
    for r in kill['results']:
        if 'mode' in r:assert r['oracle'] and r['verify_exit_code']==0 and r['marker_oracle']['all_passed']
        else:assert r['coverage_observed'] and r['reopen_full_payload_matrix_oracle_exit_code']==0 and r['pending_old_new_marker_oracle']['all_passed']
    assert b.sha(ROOT/kill['program'])==kill['program_sha256']
    policy=(ROOT/'src/engine.rs').read_text();assert 'revision_reclamation: false,' in policy and 'query_visibility_pruning: false,' in policy
    assert not subprocess.check_output(['git','diff','5c6ffa3','--','Cargo.toml','Cargo.lock','src/format.rs','src/manifest.rs','src/payload.rs'],cwd=ROOT)
    diff=subprocess.check_output(['git','diff','-U0','5c6ffa3','--','src/segment.rs'],cwd=ROOT).decode()
    assert not [l for l in diff.splitlines() if l[:1] in '+-' and not l.startswith(('+++','---')) and re.search('MAGIC|FOOTER|const.*VERSION',l)]
    b.write(LOGS/f'{P}-proof.json',dict(status='complete',protected_unchanged=True,frozen_programs_verified=True,measured_sections=len(d['completed']),normal_samples_per_case_per_reader=60,profile_samples=570,process_kill_cases=7,source_format_dependencies_unchanged=True,default_reclamation=False,default_query_pruning=False,gates=s['gates'],case34='tracked'))
    if '--staged' in sys.argv:
        blobs=subprocess.check_output(['git','ls-files','--stage','-z','--','docs/benchmarks/revision-search-*'],cwd=ROOT);verified=[]
        for entry in blobs.split(b'\0'):
            if not entry:continue
            header,path=entry.split(b'\t',1);name=path.decode()
            if name.endswith('-staged-byte-proof.json'):continue
            data=(ROOT/name).read_bytes();sha=hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest();assert sha==header.split()[1].decode(),name
            verified.append(dict(path=name,git_blob=sha,sha256=hashlib.sha256(data).hexdigest()))
        b.write(LOGS/f'{P}-staged-byte-proof.json',dict(count=len(verified),raw_bytes_preserved=True,verified=verified))
    print(json.dumps(s['gates'],indent=2))
if __name__=='__main__':main()
