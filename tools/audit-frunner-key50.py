"""Completed key-pool evidence; no extra measurement or engine changes."""
import hashlib
import json
import re
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='frunner-key50-20261102'
def read(p):return json.loads(p.read_text(encoding='utf-8'))
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest().upper()
def test_counts(name):
    rows=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',(L/f'{P}-{name}.log').read_text(encoding='utf-8'))
    assert rows and all(f=='0' for _,f,_ in rows)
    return dict(passed=sum(int(p) for p,_,_ in rows),ignored=sum(int(i) for _,_,i in rows))
def main():
    d=read(L/f'{P}-design.json');s=read(L/f'{P}-summary.json');assert d['stage']==s['stage']=='completed'
    assert len(d['completed'])==len(s['arms'])==16 and len(s['reads'])==80 and len(s['writes'])==80 and len(s['cells'])==1024
    assert len({x['body'] for x in d['donors']})==256 and all(x['body']<50000 for x in d['donors'])
    samples=read(L/f'{P}-read-samples.json');assert len(samples)==240 and all(x['n']>=60 for x in samples)
    assert all(x['n']==64 for x in s['cells'])
    for row in s['reads']:
        limit=30 if row['idle4']['p95']<=30 else 2*row['idle4']['p95']
        assert row['limit']==limit and row['passed']==(row['loaded']['p95']<=limit)
    for row in s['writes']:
        edge=row['kind'].startswith('edge')
        expected=all(v['max']<=100 and (edge or v['p95']<=20 and v['p99']<=50) for k,v in row.items() if k in ('commit_ms','visibility_ms'))
        assert row['passed']==expected
    assert d['engine_sources']=={str(p.relative_to(R)):sha(p) for p in sorted((R/'src').rglob('*.rs'))}
    accepted=read(L/'hybrid-default50-20261102-design.json')
    assert d['engine_sources']==accepted['engine_sources_after']
    for p in d['programs'].values():assert sha(Path(p['path']))==p['sha256']
    assert d['protected_before']==d['protected_after'] and d['corpus_sha256']==d['corpus_after']==sha(Path(d['initial_slots']['source']))
    assert d['main_sql_before']==d['main_sql_after']
    for item in d['protected_after']:assert sha(R/item['path'])==item['sha256']
    for name in ['validation','final-validation','post-validation']:assert all(v==0 for v in read(L/f'{P}-{name}.json').values())
    for stage in ['pre','post']:
        r=read(L/f'{P}-{stage}-recovery.json');assert len(r['kills'])==3 and r['old_reader']['oracle'] and r['live_sources_unchanged']
        assert r['protected_before']==r['protected_after']
    pool=read(L/f'{P}-pool-recovery.json');assert len(pool['kills'])==3 and pool['source_hashes_unchanged'] and pool['old_reader']['oracle']
    legacy=[x['old_reader'] for x in d['completed'] if x['engine']=='fdb']
    assert len(legacy)==8 and all(x['oracle'] and x['documents']==257 and x['first50_order'] for x in legacy)
    events=[json.loads(x) for x in (L/f'{P}-lifecycle.jsonl').read_text().splitlines()]
    exits=[x for x in events if x['kind']=='worker_exit'];normal=[x for x in exits if not x['label'].startswith('pool-kill')]
    assert len(normal)==35 and all(x['exit_code']==0 for x in normal)
    assert events[-1]['kind']=='driver_finish' and events[-1]['stage']=='completed'
    ptr=read(R/'docs/benchmarks/active-benchmark-slots.json')
    for engine,slot in ptr['slots'].items():
        assert slot['status']=='ready' and sha(Path(slot['state']))==slot['model_sha256']
        assert len(read(Path(slot['state']))['rows'])==50000
        inspection=read(L/f'{P}-post-inspect-{engine}.jsonl')
        assert inspection['numeric']==inspection['frunner']==0 and inspection['sequence']==read(Path(slot['state']))['sequence']
    out=dict(decision=s['decision'],main50k_fdb_full_get_and_41_queries=True,main50k_sql_payload_digest_unchanged=True,
             source_crc_oracle=True,default_try_serial_unchanged=True,engine_sources_equal_to_accepted_default=True,
             no_new_corpus=True,no_db_copy_or_50k_ingest=True,donors=256,pool=257,blocks=4,arms=16,
             read_phase_rows=240,read_min_n=min(x['n'] for x in samples),read_max_n=max(x['n'] for x in samples),
             write_samples=4097*16,cells=1024,cell_n=64,old_reader_pool_arms=8,kills=9,
             kill_scope='strict-open/full pool oracle witnessed, not in-flight commit or power-loss claim',
             normal_children=len(normal),unexpected_exits=0,tests=test_counts('tests'),diagnostic_tests=test_counts('diag-tests'),sql_tests=test_counts('sql-tests'),
             performance_thresholds_unchanged=True,next_tasks_started=False,preflight_failures_preserved=2,
             preflight_scope='donor selector duplicate and Python Path type; both before opening DB workers; logs retained')
    out['retained_after_bytes']={e:sum(p.stat().st_size for p in (R/'data/benchmark-work'/e).rglob('*') if p.is_file()) for e in ['fdb','sqlite']}
    out['pool_collections_live_records_after_cleanup']=0
    (L/f'{P}-audit.json').write_text(json.dumps(out,ensure_ascii=False,indent=2),encoding='utf-8');print(json.dumps(out,ensure_ascii=False))
if __name__=='__main__':main()
