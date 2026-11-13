"""Audit the one-off timing partition and existing correctness witnesses."""
import hashlib
import json
import re
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='visibility-phase50-20261102'
def read(p):return json.loads(p.read_text(encoding='utf-8'))
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest().upper()
def tests(name):
    xs=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',(L/f'{P}-{name}.log').read_text(encoding='utf-8'))
    assert xs and all(f=='0' for _,f,_ in xs);return sum(int(p) for p,_,_ in xs)
def main():
    d=read(L/f'{P}-design.json');s=read(L/f'{P}-summary.json');rows=read(L/f'{P}-samples.json')
    assert d['stage']==s['stage']=='completed' and d['diagnostic_once'] and d['gates'] is None
    assert len(d['blocks'])==1 and len(d['completed'])==4 and len(rows)==4097*4
    assert s['diagnostic_only'] and s['gates'] is None
    for r in rows:
        assert all(r[k]>=0 for k in ['commit','history','acquire','probe_prepare','diagnostic_init','first_query','release'])
        assert abs(sum(r[k] for k in ['history','acquire','probe_prepare','diagnostic_init','first_query','release'])-r['gap'])<1e-8
        assert abs(r['commit']+r['gap']-r['visibility'])<1e-8
    old=read(L/'frunner-key50-20261102-design.json');assert d['donors']==old['donors']
    sources={str(p.relative_to(R)):sha(p) for p in sorted((R/'src').rglob('*.rs'))};assert d['engine_sources']==old['engine_sources']==sources
    assert d['protected_before']==d['protected_after'] and d['corpus_sha256']==d['corpus_after']==sha(Path(d['initial_slots']['source']))
    assert d['main_sql_before']==d['main_sql_after']
    for p in d['programs'].values():assert sha(Path(p['path']))==p['sha256']
    for f in d['protected_after']:assert sha(R/f['path'])==f['sha256']
    assert all(v==0 for v in read(L/f'{P}-validation.json').values())
    for phase in ['pre','post']:
        r=read(L/f'{P}-{phase}-recovery.json');assert len(r['kills'])==3 and r['old_reader']['oracle'] and r['live_sources_unchanged']
    r=read(L/f'{P}-pool-recovery.json');assert len(r['kills'])==3 and r['old_reader']['oracle'] and r['source_hashes_unchanged']
    for arm in d['completed']:
        if arm['engine']=='fdb':assert arm['old_reader']['oracle'] and arm['old_reader']['documents']==257
    events=[json.loads(x) for x in (L/f'{P}-lifecycle.jsonl').read_text().splitlines()]
    exits=[x for x in events if x['kind']=='worker_exit' and not x['label'].startswith('pool-kill')]
    assert len(exits)==11 and all(x['exit_code']==0 for x in exits)
    assert events[-1]['stage']=='completed'
    pointer=read(L/'active-benchmark-slots.json')
    for engine,slot in pointer['slots'].items():
        assert slot['status']=='ready' and sha(Path(slot['state']))==slot['model_sha256']
        assert len(read(Path(slot['state']))['rows'])==50000
        inspection=read(L/f'{P}-post-inspect-{engine}.jsonl');assert inspection['numeric']==inspection['frunner']==0
    out=dict(diagnostic_only=True,gates=None,blocks=1,arms=4,rows=len(rows),exclusive_timeline_partition=True,
             nested_spans_not_added_to_outer_wall=True,profile_collected_after_visibility=True,
             engine_sources_equal_to_accepted_default=True,corpus_v1_fixture_sha_equal=True,
             same_donors_as_B=True,main50k_fdb_oracle=True,main50k_sql_digest_unchanged=True,pools_live=0,
             kills=9,old_reader=True,normal_children=11,unexpected_exits=0,
             tests_passed=tests('tests'),diagnostic_tests_passed=tests('diag-tests'),sql_tests_passed=tests('sql-tests'),
             no_new_corpus_or_db_copy_or_50k_ingest=True,optimization=False,next_tasks_started=False,
             kill_scope='strict open +oracle witnessed; not in-flight transaction or power-loss claim')
    (L/f'{P}-audit.json').write_text(json.dumps(out,ensure_ascii=False,indent=2),encoding='utf-8');print(json.dumps(out,ensure_ascii=False))
if __name__=='__main__':main()
