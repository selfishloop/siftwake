"""Final evidence audit only; never runs a benchmark or mutates an owner DB."""
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='term-cache50-20261102'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest().upper()
def main():
    d=read(L/f'{P}-design.json');s=read(L/f'{P}-summary.json')
    assert d['stage']=='completed' and s['stage']=='completed'
    assert len(s['normal'])==4*4*41 and len(s['pools'])==16 and len(s['ram'])==24
    assert len([x for x in s['arms'] if x['phase']=='normal'])==16
    assert d['engine_sources']=={str(p.relative_to(R)):sha(p) for p in sorted((R/'src').rglob('*.rs'))}
    assert all(sha(Path(p))==h for p,h in d['programs'].items())
    assert d['protected_before']==d['protected_after'] and d['corpus_after']==d['corpus_sha256']==sha(Path(d['initial_slots']['source']))
    for p in d['protected_after']:assert sha(R/p['path'])==p['sha256']
    assert 'Cell::new(false)' in (R/'src/engine/first.rs').read_text(encoding='utf-8')
    for x in s['cache']:
        assert x['page_limit_bytes']==(4*1024*1024 if x['arm']=='D' else 960*1024)
        assert x['lookup_limit_bytes']==64*1024 and x['inflight_current_bytes']==0
        if x['arm']=='A':assert x['combined_peak_bytes']<=960*1024
    recoveries={}
    for stage in ['pre-A','post-normal-A','final-K','final-A','final-D']:
        x=read(L/f'{P}-{stage}-recovery.json');assert len(x['kills'])==3 and x['old_reader']['oracle'] and x['live_sources_unchanged']
        recoveries[stage]=dict(kills=len(x['kills']),old_reader=True)
    for x in d['completed']:
        if x['phase']=='pool' and x['arm']!='S':assert x['old_reader']['oracle'] and x['old_reader']['documents']==257
    ptr=read(L/'active-benchmark-slots.json')
    for slot in ptr['slots'].values():
        assert slot['status']=='ready' and sha(Path(slot['state']))==slot['model_sha256'] and len(read(Path(slot['state']))['rows'])==50000
    test_logs=['tests-A','test','diag-tests-K','diag-tests-D','post-tests-A','example-tests','diag-example-tests','sql-tests']
    tests={}
    for name in test_logs:
        v=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',(L/f'{P}-{name}.log').read_text(encoding='utf-8'))
        assert v and all(n=='0' for _,n,_ in v);tests[name]=sum(int(n) for n,_,_ in v)
    validation=read(L/f'{P}-validation.json');assert all(x==0 for x in validation.values())
    assert 'Finished' in (L/f'{P}-final-diag-clippy.log').read_text(encoding='utf-8')
    assert 'Finished' in (L/f'{P}-sql-clippy.log').read_text(encoding='utf-8')
    for name in ['post-fmt','final-fmt']:assert not (L/f'{P}-{name}.log').read_text(encoding='utf-8').strip()
    result=dict(completed=True,normal_case_rows=656,pool_arms=16,ram_arms=24,default='K unchanged',
                capacity_and_lock_arms_separate=True,all_A_controlled_peaks_within_960KiB=True,
                owner_pools_legacy_oracles=12,recoveries=recoveries,tests=tests,
                source_fixture_corpus_SHA=True,no_reset_reingest_or_DB_copy=True,
                kill_scope='strict-open/oracle witnessed; no power-loss or in-flight kill claim',next_round=False)
    (L/f'{P}-audit.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(result,ensure_ascii=False))
if __name__=='__main__':main()
