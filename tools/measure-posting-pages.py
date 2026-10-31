"""Same first50k, single FDB/SQLite slot, frozen-reader A/B and full oracle."""
import importlib.util
import json
import os
import subprocess
import time
from pathlib import Path

R=Path(__file__).resolve().parent.parent
L=R/'docs/benchmarks';P='posting-pages-50k-20261029-v3'
spec=importlib.util.spec_from_file_location('scale',R/'tools/measure-trwiki-scale.py')
s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)
programs={'baseline':R/'output/trwiki-scale-20261028-v2-fdb.exe',
          'candidate':R/'output/posting-pages-20261029-v3-fdb.exe',
          'sqlite':R/'output/trwiki-scale-20261028-v2-sqlite.exe'}

def run(e,mode,state,label,env):
    log=L/f'{P}-{label}-{e}-{mode}.jsonl'
    assert not log.exists(),log
    at=time.monotonic()
    with log.open('w',encoding='utf-8') as out,log.with_suffix('.stderr').open('w',encoding='utf-8') as err:
        code=subprocess.run([str(programs[e]),mode,'streamed',str(state)],cwd=R,env=env,
                            stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW).returncode
    result=dict(engine=e,mode=mode,label=label,log=log.name,exit_code=code,wall_s=time.monotonic()-at)
    s.b.write(log.with_suffix('.process.json'),result)
    print(json.dumps(result),flush=True)
    assert code==0,log
    return result

def main():
    design=L/f'{P}-design.json';assert not design.exists(),'Preserve existing evidence'
    checks=json.loads((L/'snapshot-10k-20261026-validation-posting-pages-20261029-v3.json').read_text(encoding='utf-8-sig'))
    assert len(checks)==8 and all(c['exit_code']==0 for c in checks)
    audit=json.loads((L/'trwiki-provided-20261028-v1-audit.json').read_text(encoding='utf-8'))
    assert s.b.sha(s.CORPUS)==audit['payload_sha256'] and s.b.sha(s.CATALOG)==audit['catalog_sha256']
    cases=list(range(35))+[39,40]
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_SCALE_DOCUMENTS='50000',
        FDB_BENCH_CORPUS_PATH=str(s.CORPUS),FDB_BENCH_CORPUS_INDEX=str(s.CATALOG),
        FDB_BENCH_ORIGINAL_BODIES='1',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',
        FDB_QUERY_CASES=','.join(map(str,cases)),FDB_SCALE_QUERY_REPEATS='10')
    d=dict(programs={e:dict(path=str(p.relative_to(R)),sha256=s.b.sha(p)) for e,p in programs.items()},
        cases=cases,documents=50000,main='cap-hybrid N50 budget200; exact serial measured separately',
        orders=[['baseline','sqlite','candidate'],['candidate','sqlite','baseline']],samples_per_case_variant=20,
        cache='Warm OS/engine: full GET/version oracle+one query warmup. No OS eviction.',
        cpu='GetProcessTimes user+kernel all query threads; quantized individual calls; positive blocks separately',
        negative_gate='control cap-four p95<1ms => absolute increase<=2ms; otherwise +10%',
        scope='No format/filter/WAL/limit/default API changes; 100k waits for user-capped replacement corpus',
        protected_before=s.b.protected(),provided_source_sha256=s.b.sha(s.CORPUS),completed=[])
    d['implementation_source_sha256']={str(p.relative_to(R)):s.b.sha(p) for p in sorted((R/'src').rglob('*.rs'))}
    s.b.write(design,d)
    # Reuse the completed preparation from v1. Its interrupted timing is never
    # pooled with this run. In particular, no corpus/DB is generated again.
    prior=json.loads((L/'posting-pages-50k-20261029-v1-design.json').read_text(encoding='utf-8'))
    states={e:Path(prior['models'][e]) for e in ['fdb','sqlite']}
    pointer=json.loads(s.CURRENT.read_text(encoding='utf-8'))
    for slot in states:
        assert pointer['slots'][slot]['documents']==50000
        assert s.b.sha(states[slot])==pointer['slots'][slot]['model_sha256']
    d['preparation']=[x for x in prior['completed'] if x['label']=='prepare']
    d['discarded_timing']='v1 stopped to preserve mandatory cursor admission; no v1 timings pooled'
    page_only=json.loads((L/'posting-pages-50k-20261029-v2-design.json').read_text(encoding='utf-8'))
    assert all(s.b.sha(programs[e])==page_only['programs'][e]['sha256'] for e in ['baseline','sqlite'])
    d['reused_control_scope']='Completed oracle-passing pair0 baseline/SQLite only; same frozen binaries/models/maintained DB; build gap disclosed. V2 candidate never pooled.'
    d['completed']=[r for r in page_only['completed'] if r['engine'] in ['baseline','sqlite']]
    assert len(d['completed'])==6 and all(r['exit_code']==0 for r in d['completed'])
    assert s.b.sha(states['fdb'])==s.b.sha(states['sqlite'])
    d['models']={e:str(p) for e,p in states.items()};d['model_sha256']=s.b.sha(states['fdb'])
    before=s.sources()
    def measured(e,mode,label):
        return any(x['engine']==e and x['mode']==mode and x['label']==label for x in d['completed'])
    for pair,order in enumerate(d['orders']):
        for e in order:
            if measured(e,'scale-query-experiments',f'pair{pair}-matrix'):continue
            state=states['sqlite' if e=='sqlite' else 'fdb']
            qe=env.copy();qe['FDB_QUERY_VARIANTS']='legacy,cap' if e=='sqlite' else 'legacy,cap-hybrid'
            result=run(e,'scale-query-experiments',state,f'pair{pair}-matrix',qe)
            d['completed'].append(result);s.b.write(design,d)
        for e in order:
            if measured(e,'scale-idle',f'pair{pair}-idle'):continue
            result=run(e,'scale-idle',states['sqlite' if e=='sqlite' else 'fdb'],f'pair{pair}-idle',env)
            d['completed'].append(result)
        # High-repeat CPU blocks make the cheap positive paths measurable.
        for e in [x for x in order if x!='sqlite']:
            ce=env.copy();ce.update(FDB_QUERY_CASES='0,1,2',FDB_QUERY_VARIANTS='cap,cap-four,cap-hybrid' if pair==0 else 'cap-hybrid,cap-four,cap',FDB_CPU_BLOCK_REPEATS='200')
            if not measured(e,'scale-query-cpu-blocks',f'pair{pair}-cpu'):
                d['completed'].append(run(e,'scale-query-cpu-blocks',states['fdb'],f'pair{pair}-cpu',ce))
            ne=env.copy();ne.update(FDB_QUERY_CASES='39,40',FDB_QUERY_VARIANTS='cap-four,cap-hybrid')
            if not measured(e,'scale-query-experiments',f'pair{pair}-negative'):
                d['completed'].append(run(e,'scale-query-experiments',states['fdb'],f'pair{pair}-negative',ne))
        assert before==s.sources()
        s.b.write(design,d)
    d.update(protected_after=s.b.protected(),provided_source_after=s.b.sha(s.CORPUS),authoritative_sources_unchanged=before==s.sources())
    assert d['protected_before']==d['protected_after'] and d['provided_source_sha256']==d['provided_source_after']
    s.b.write(design,d)
    print('Both balanced pairs, full payload and exact/capped oracles complete',flush=True)

if __name__=='__main__':main()
