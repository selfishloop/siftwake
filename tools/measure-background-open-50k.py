"""Opt-in background/strict/SQLite first-result latency; warm fixed slots."""
import importlib
import importlib.util
import json
import os
import subprocess
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
L=ROOT/'docs/benchmarks'
P='block2-background-open-50k-20261028-v1'
spec=importlib.util.spec_from_file_location('bench',ROOT/'tools/measure-churn.py')
b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
programs={n:ROOT/f'output/{P}-{n}.exe' for n in ('normal','diagnostic','sqlite')}
queries=['harpoon','presumption','darcy','transubstantiation','holmes elementary']

def sources():
    return {p.name:b.sha(p) for p in (ROOT/'data/benchmark-work/fdb').iterdir() if p.suffix in ('.fdb','.ready')}

def run(program,mode,state,label,env):
    log=L/f'{P}-{label}-{program}-{mode}.jsonl'
    assert not log.exists()
    at=time.monotonic()
    with log.open('w',encoding='utf-8') as out,log.with_suffix('.stderr').open('w',encoding='utf-8') as err:
        child=subprocess.run([str(programs[program]),mode,'streamed',str(state)],cwd=ROOT,
            env=env,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
    result=dict(log=log.name,program=program,mode=mode,label=label,exit_code=child.returncode,wall_s=time.monotonic()-at)
    b.write(log.with_suffix('.process.json'),result)
    print(json.dumps(result),flush=True)
    assert child.returncode==0,result
    return result

def main():
    design=L/f'{P}-design.json';assert not design.exists()
    # The prior gate is an explicit prerequisite, never inferred from a fast handle.
    gate=json.loads((L/'block2-capped-exact-50k-20261028-v1-analysis.json').read_text(encoding='utf-8'))
    assert gate['exact_four_gate'] and gate['cap_four_gate']
    fs=Path(json.loads((L/'block2-scale-20261028-v1-slot-fdb.json').read_text(encoding='utf-8'))['state'])
    ss=Path(json.loads((L/'block2-query-open-50k-20261028-v1-slot-sqlite.json').read_text(encoding='utf-8'))['state'])
    assert b.sha(fs)==b.sha(ss)
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_SCALE_DOCUMENTS='50000',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',FDB_BENCH_CORPUS_INDEX=str(L/'block2-scale-20261028-v1-corpus-index.json'))
    record=dict(programs={n:dict(path=str(p.relative_to(ROOT)),sha256=b.sha(p)) for n,p in programs.items()},
        sources_before=sources(),protected_before=b.protected(),model_sha256=b.sha(fs),
        cache='Warm OS procedure; source/hash and prior full oracle; no forced eviction',
        order=['strict/background/sqlite','sqlite/background/strict']*2,queries=queries,
        samples_per_query_arm=4,first_result='elapsed since API invocation; includes validation wait and N50 hydration',
        prototype='all source, SEG05 directory and index CRC checks in <=4 worker background; no result before ALL strict validation',
        source_format_changed=False,completed=[])
    b.write(design,record)
    for repeat in range(4):
        order=[('normal','first-strict'),('normal','first-background'),('sqlite','first-sqlite')]
        if repeat%2:order.reverse()
        for pos in range(5):
            q=queries[(pos+repeat)%5];env['FDB_FIRST_QUERY']=q
            for program,mode in order:
                record['completed'].append(run(program,mode,ss if program=='sqlite' else fs,f'r{repeat}-q{queries.index(q)}',env))
                b.write(design,record)
    env['FDB_FIRST_QUERY']='harpoon'
    record['completed'].append(run('diagnostic','first-background',fs,'profile',env))
    record['completed'].append(run('diagnostic','first-strict',fs,'profile',env))
    record['completed'].append(run('normal','scale-first-oracle',fs,'all38',env))
    # Kill only a READ opening session. Existing process-kill commit/merge tests
    # run again in cargo test; do not manufacture new 50k churn or change ledgers.
    log=L/f'{P}-kill-opening.jsonl';assert not log.exists()
    with log.open('w',encoding='utf-8') as out,log.with_suffix('.stderr').open('w',encoding='utf-8') as err:
        child=subprocess.Popen([str(programs['normal']),'first-background','streamed',str(fs)],cwd=ROOT,
            env=env,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW)
        first=json.loads(child.stdout.readline());out.write(json.dumps(first)+'\n');out.flush()
        assert first['phase']=='session_return' and first['status']=='pending',first
        child.kill();child.wait(timeout=20)
        killed=dict(phase='kill_opening',exit_code=child.returncode,witness=first,coverage_observed=True,power_loss_claim=False)
        out.write(json.dumps(killed)+'\n')
    record['opening_kill']=killed
    record['completed'].append(run('normal','scale-first-oracle',fs,'after-kill',env))
    # Frozen old reader: full50k payload/version, all deleted keys and38 original cases.
    import sys
    sys.path.insert(0,str(ROOT/'tools'))
    old=importlib.import_module('kill-block2-scale');old.P=P+'-compat'
    os.environ['FDB_BENCH_CORPUS_INDEX']=env['FDB_BENCH_CORPUS_INDEX']
    record['old_reader']=old.old_reader(fs)
    record.update(protected_after=b.protected(),sources_after=sources())
    assert record['protected_before']==record['protected_after'] and record['sources_before']==record['sources_after']
    assert b.sha(fs)==b.sha(ss)==record['model_sha256']
    record.update(protected_unchanged=True,sources_unchanged=True,model_unchanged=True)
    b.write(design,record)

if __name__=='__main__':main()
