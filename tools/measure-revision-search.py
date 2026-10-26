"""One narrow query experiment; existing 10k, fixed slots, no corpus preparation."""
import importlib,json,os
from pathlib import Path
b=importlib.import_module('measure-churn');ROOT,LOGS=b.ROOT,b.LOGS
P='revision-search-10k-20261026';b.PREFIX=P

def main():
    target=LOGS/f'{P}-design.json';assert not target.exists()
    protected=b.protected()
    design=dict(protected_before=protected,cache='warm OS; 1 warmup +30 timed samples/case; full page oracle outside timer',orders=[],completed=[],programs={},scope='case34 tracked; per-case pooled60; no absolute-ms waiver for tiny queries')
    def sources():return {p.name:b.sha(p) for p in (ROOT/'data/benchmark-work/fdb').iterdir() if p.suffix in ['.fdb','.ready']}
    def run(label,mode,state,variant='before',legacy=False,engine='fdb'):
        os.environ.pop('FDB_QUERY_VISIBILITY_PRUNING',None);os.environ.pop('FDB_COST_LEGACY',None);os.environ.pop('FDB_RECLAMATION_CONTROL',None)
        if variant.startswith('after'):os.environ['FDB_QUERY_VISIBILITY_PRUNING']='1'
        if legacy:os.environ['FDB_COST_LEGACY']='1'
        program=ROOT/f'output/revision-search-{variant}-20261026.exe'
        if engine=='sqlite':program=ROOT/'output/revision-search-sqlite-20261026.exe'
        b.PROGRAMS[engine]=program;design['programs'][str(program.relative_to(ROOT))]=b.sha(program)
        design['orders'].append(dict(label=label,mode=mode,variant=variant,legacy=legacy,engine=engine,state=str(state)))
        b.write(target,design)
        design['completed'].append(b.run(engine,mode,'streamed',state,label));b.write(target,design)
    # Existing 10th-round candidate is reused before any reset. Its earlier
    # diagnostic profile is recorded separately; it is not a timing gate sample.
    candidate=json.loads((LOGS/'revisions-cost-10k-20261025-slot-fdb.json').read_text())['state']
    model_before=json.loads(Path(candidate).read_text());source_before=sources()
    for v in ['before','after']:run('candidate-r0-'+v,'search-matrix',candidate,v)
    run('candidate-profile-after','search-profile',candidate,'after-profile')
    design['candidate_read_sources_unchanged']=source_before==sources();assert design['candidate_read_sources_unchanged']
    # Same corpus + workload, aggressive old policy as the physical reference.
    b.reset('fdb');ref=LOGS/f'{P}-reference-state.json'
    for mode in ['seed','churn','verify']:run('reference-prepare',mode,ref,'after',True)
    run('reference-profile','search-profile',ref,'before-profile',True)
    for pair,order in enumerate([['before','after'],['after','before']]):
        for v in order:run(f'reference-r{pair}-{v}','search-matrix',ref,v,True)
    # Rebuild candidate once to measure accepted maintenance cost unchanged;
    # pair reader order reverses around reference. Only the fixed slot resets.
    b.reset('fdb');candidate=LOGS/f'{P}-candidate-state.json'
    for mode in ['seed','churn','verify']:run('candidate-prepare',mode,candidate,'after')
    design['candidate_model_equal']=model_before==json.loads(candidate.read_text());assert design['candidate_model_equal']
    for v in ['after','before']:run('candidate-r1-'+v,'search-matrix',candidate,v)
    sqlite=json.loads((LOGS/'revisions-cost-10k-20261025-slot-sqlite.json').read_text())['state']
    for pair in [0,1]:run(f'sqlite-r{pair}','search-matrix',sqlite,engine='sqlite')
    run('sqlite-oracle','verify',sqlite,engine='sqlite')
    design['protected_after']=b.protected();assert protected==design['protected_after']
    design['complete']=True;b.write(target,design)

if __name__=='__main__':main()
