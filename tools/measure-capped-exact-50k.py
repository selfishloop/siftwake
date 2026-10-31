"""Read-only balanced query experiments; existing fixed DB slots only."""
import importlib.util
import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOGS = ROOT / 'docs/benchmarks'
P = 'block2-capped-exact-50k-20261028-v1'
spec = importlib.util.spec_from_file_location('fixed_bench', ROOT/'tools/measure-churn.py')
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)
PROGRAMS = {n: ROOT/f'output/{P}-{n}.exe' for n in ('normal','diagnostic','sqlite')}

def sources():
    return {p.name:b.sha(p) for p in (ROOT/'data/benchmark-work/fdb').iterdir()
            if p.suffix in ('.fdb','.ready')}

def run(program, state, label, variants, repeats):
    log=LOGS/f'{P}-{label}-{program}.jsonl'
    if log.exists():
        raise RuntimeError(f'Existing evidence: {log}')
    env=os.environ.copy()
    env.update(PYTHONUTF8='1',FDB_SCALE_DOCUMENTS='50000',FDB_STREAMED_WAL_LAYER='1',
        FDB_RECLAMATION_CONTROL='1',FDB_SCALE_QUERY_REPEATS=str(repeats),FDB_QUERY_VARIANTS=variants,
        FDB_BENCH_CORPUS_INDEX=str(LOGS/'block2-scale-20261028-v1-corpus-index.json'))
    at=time.monotonic()
    with log.open('w',encoding='utf-8') as out,log.with_suffix('.stderr').open('w',encoding='utf-8') as err:
        child=subprocess.run([str(PROGRAMS[program]),'scale-query-experiments','streamed',str(state)],
            cwd=ROOT,env=env,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
    result=dict(program=program,label=label,log=log.name,exit_code=child.returncode,
        wall_s=time.monotonic()-at,variants=variants,repeats=repeats)
    b.write(log.with_suffix('.process.json'),result)
    print(json.dumps(result),flush=True)
    if child.returncode:
        raise RuntimeError(f'Child failed: {log}')
    return result

def main():
    design=LOGS/f'{P}-design.json'
    if design.exists():
        raise RuntimeError('Inspect/resume existing evidence; do not replay')
    fstate=Path(json.loads((LOGS/'block2-scale-20261028-v1-slot-fdb.json').read_text(encoding='utf-8'))['state'])
    sstate=Path(json.loads((LOGS/'block2-query-open-50k-20261028-v1-slot-sqlite.json').read_text(encoding='utf-8'))['state'])
    assert b.sha(fstate)==b.sha(sstate)
    variants='legacy,exact,exact-two,exact-four,exact-join,exact-crc,exact-four-join-crc,cap,cap-four,cap-four-join-crc'
    record=dict(programs={n:dict(path=str(p.relative_to(ROOT)),sha256=b.sha(p)) for n,p in PROGRAMS.items()},
        fstate=str(fstate),sstate=str(sstate),model_sha256=b.sha(fstate),
        protected_before=b.protected(),sources_before=sources(),completed=[],
        corpus='Existing first50k and identical sequence104 payload/version ledgers; no reset, new corpus or DB copies',
        cache='Warm OS, full get/version oracle and all variant/case warmup; no forced eviction',
        cases=[5,29,8,4,23],limit=50,order=['FDB/SQLite','SQLite/FDB'],
        cpu='GetProcessTimes user+kernel all process threads; quantized per-query, aggregate CPU separately',
        normal_samples_per_variant_case=20,diagnostic_not_used_for_performance=True)
    b.write(design,record)
    for pair,order in enumerate([('normal','sqlite'),('sqlite','normal')]):
        for program in order:
            record['completed'].append(run(program,sstate if program=='sqlite' else fstate,
                f'pair{pair}', 'legacy,cap' if program=='sqlite' else variants,10))
            b.write(design,record)
    record['completed'].append(run('diagnostic',fstate,'counters',variants,2))
    record.update(protected_after=b.protected(),sources_after=sources())
    assert record['protected_before']==record['protected_after']
    assert record['sources_before']==record['sources_after']
    assert b.sha(fstate)==b.sha(sstate)==record['model_sha256']
    record.update(protected_unchanged=True,sources_unchanged=True,model_unchanged=True)
    b.write(design,record)

if __name__=='__main__':
    main()
