"""Accepted50k book fixture: hybrid continuation/cancel gate before slot reuse."""
import importlib.util
import json
import os
import subprocess
import time
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks'
P='hybrid-cancel-book50k-20261028-v1'
spec=importlib.util.spec_from_file_location('bench',R/'tools/measure-churn.py')
b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
programs={n:R/f'output/hybrid-cancel-20261028-v2-{n}.exe' for n in ('normal','sqlite','diagnostic')}
def sources():return {p.name:b.sha(p) for p in (R/'data/benchmark-work/fdb').iterdir() if p.suffix in ('.fdb','.ready')}

def run(program,mode,state,label,env):
    log=L/f'{P}-{label}-{program}-{mode}.jsonl';assert not log.exists()
    at=time.monotonic()
    with log.open('w',encoding='utf-8') as out,log.with_suffix('.stderr').open('w',encoding='utf-8') as err:
        child=subprocess.run([str(programs[program]),mode,'streamed',str(state)],cwd=R,env=env,
            stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
    result=dict(log=log.name,exit_code=child.returncode,wall_s=time.monotonic()-at)
    b.write(log.with_suffix('.process.json'),result);print(json.dumps(result),flush=True)
    assert child.returncode==0,result
    return result

def main():
    design=L/f'{P}-design.json';assert not design.exists()
    fs=Path(json.loads((L/'block2-scale-20261028-v1-slot-fdb.json').read_text(encoding='utf-8'))['state'])
    ss=Path(json.loads((L/'block2-query-open-50k-20261028-v1-slot-sqlite.json').read_text(encoding='utf-8'))['state'])
    assert b.sha(fs)==b.sha(ss)
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_SCALE_DOCUMENTS='50000',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',FDB_BENCH_CORPUS_INDEX=str(L/'block2-scale-20261028-v1-corpus-index.json'),FDB_QUERY_CASES='5,29,8,4,23')
    for key in ['FDB_BENCH_CORPUS_PATH','FDB_BENCH_ORIGINAL_BODIES']:env.pop(key,None)
    variants='cap,cap-four,cap-hybrid,cap-cancel,cap-four-cancel,cap-hybrid-cancel'
    d=dict(programs={n:dict(path=str(p.relative_to(R)),sha256=b.sha(p)) for n,p in programs.items()},
        protected_before=b.protected(),sources_before=sources(),model_sha256=b.sha(fs),
        corpus='same existing50k post-recovery sequence104, no reset/copies/new bodies',
        cache='Warm OS procedure: full get/version oracle plus variant warmup; no OS eviction',
        order=['FDB/SQLite','SQLite/FDB'],normal_samples_per_case_variant=40,
        sqlite_samples_per_case=6,sqlite_p95_limit='n6 exploratory reference; original n20 remains separately documented',
        cpu_blocks='2 balanced orders x5 rotated blocks x100 queries for3 positive cases; query+owned-result drop, no oracle inside CPU interval',
        gates='negative hybrid p95<=four-worker p95; positive hybrid CPU totals compared literally with serial, no new tolerance silently added',
        completed=[])
    b.write(design,d)
    for pair,order in enumerate([('normal','sqlite'),('sqlite','normal')]):
        for program in order:
            e=env.copy();e.update(FDB_QUERY_VARIANTS=variants if program=='normal' else 'legacy,cap',FDB_SCALE_QUERY_REPEATS='20' if program=='normal' else '3')
            d['completed'].append(run(program,'scale-query-experiments',ss if program=='sqlite' else fs,f'pair{pair}',e));b.write(design,d)
    for pair in range(2):
        e=env.copy();e.update(FDB_QUERY_CASES='5,29,8',FDB_QUERY_VARIANTS='cap,cap-four,cap-hybrid' if pair==0 else 'cap-hybrid,cap-four,cap',FDB_CPU_BLOCK_REPEATS='100')
        d['completed'].append(run('normal','scale-query-cpu-blocks',fs,f'cpu{pair}',e));b.write(design,d)
    d['completed'].append(run('normal','scale-cancel-probe',fs,'cancel',env));b.write(design,d)
    e=env.copy();e.update(FDB_QUERY_VARIANTS=variants,FDB_SCALE_QUERY_REPEATS='2')
    d['completed'].append(run('diagnostic','scale-query-experiments',fs,'counters',e));b.write(design,d)
    d['completed'].append(run('normal','scale-first-oracle',fs,'oracle38',env))
    d.update(protected_after=b.protected(),sources_after=sources())
    assert d['protected_before']==d['protected_after'] and d['sources_before']==d['sources_after']
    assert b.sha(fs)==b.sha(ss)==d['model_sha256']
    d.update(protected_unchanged=True,sources_unchanged=True,model_unchanged=True)
    b.write(design,d)

if __name__=='__main__':main()
