"""Exact-total/open profiling before API design approval. Reuses fixed50k DBs."""
import hashlib
import importlib.util
import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOGS = ROOT / 'docs/benchmarks'
P = 'block2-query-open-50k-20261028-v1'
spec = importlib.util.spec_from_file_location('fixed_bench', ROOT/'tools/measure-churn.py')
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)
PROGRAMS = {name:ROOT/f'output/{P}-{name}.exe' for name in ('normal','diagnostic','sqlite')}


def sources():
    return {p.name:b.sha(p) for p in (ROOT/'data/benchmark-work/fdb').iterdir()
            if p.suffix in ('.fdb','.ready')}


def run(program, mode, state, label, env):
    log = LOGS/f'{P}-{label}-{program}-{mode}.jsonl'
    if log.exists():
        raise RuntimeError(f'Never overwrite evidence: {log}')
    started=time.monotonic()
    with log.open('w',encoding='utf-8') as out, log.with_suffix('.stderr').open('w',encoding='utf-8') as err:
        child=subprocess.run([str(PROGRAMS[program]),mode,'streamed',str(state)],cwd=ROOT,
            env=env,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
    result=dict(program=program,mode=mode,label=label,log=log.name,exit_code=child.returncode,
                wall_s=time.monotonic()-started)
    b.write(log.with_suffix('.process.json'),result)
    print(json.dumps(result),flush=True)
    if child.returncode:
        raise RuntimeError(f'Worker failed: {log}')
    return result


def main():
    design=LOGS/f'{P}-design.json'
    if design.exists():
        raise RuntimeError('Existing design: inspect and resume explicitly instead of replaying')
    slot=lambda name:json.loads((LOGS/f'block2-scale-20261028-v1-slot-{name}.json').read_text(encoding='utf-8'))
    fslot,sslot=slot('fdb'),slot('sqlite')
    assert fslot['count']==sslot['count']==50000
    fstate=Path(fslot['state'])
    sstate=LOGS/f'{P}-sqlite-aligned-state.json'
    assert not sstate.exists()
    sstate.write_bytes(Path(sslot['state']).read_bytes())  # Ledger only, never a DB copy.
    env=os.environ.copy()
    env.update(PYTHONUTF8='1',FDB_SCALE_DOCUMENTS='50000',FDB_STREAMED_WAL_LAYER='1',
        FDB_RECLAMATION_CONTROL='1',FDB_SCALE_QUERY_REPEATS='10',
        FDB_BENCH_CORPUS_INDEX=str(LOGS/'block2-scale-20261028-v1-corpus-index.json'),
        FDB_SCALE_TARGET_MODEL=str(fstate))
    record=dict(programs={k:dict(path=str(p.relative_to(ROOT)),sha256=b.sha(p)) for k,p in PROGRAMS.items()},
        protected_before=b.protected(),fdb_sources_before=sources(),
        fdb_state=str(fstate),fdb_state_sha256=b.sha(fstate),sqlite_state=str(sstate),
        corpus='existing first50k pool and post-recovery logical model; no new corpus or DB reset',
        cache='warm OS procedure: full get/version oracle, one query warmup; no OS eviction',
        query_cases=[5,29,8,4,23],query_names=['harpoon','presumption','darcy','transubstantiation','holmes elementary'],
        timings='normal only for p95; diagnostic nested wall/call/byte counters are not normal performance',
        order=['FDB then SQLite','SQLite then FDB'],samples_per_case_per_arm=10,
        source_formats_changed=False,capped_or_deferred_open_implemented=False,completed=[])
    b.write(design,record)
    record['completed'].append(run('sqlite','scale-align-reference',sstate,'setup',env))
    assert b.sha(fstate)==b.sha(sstate),'Reference payload/version model must match byte-for-byte'
    record['aligned_model_sha256']=b.sha(sstate)
    b.write(LOGS/f'{P}-slot-sqlite.json',dict(count=50000,state=str(sstate)))
    for i in range(3):
        for program,mode in [('normal','scale-idle'),('diagnostic','open-profile')]:
            record['completed'].append(run(program,mode,fstate,f'open-r{i}',env))
    record['completed'].append(run('diagnostic','search-target-profile',fstate,'profile-r0',env))
    b.write(design,record)
    for pair,order in enumerate([('normal','sqlite'),('sqlite','normal')]):
        for program in order:
            state=sstate if program=='sqlite' else fstate
            record['completed'].append(run(program,'scale-target-matrix',state,f'pair-r{pair}',env))
            b.write(design,record)
    record['completed'].append(run('diagnostic','search-target-profile',fstate,'profile-r1',env))
    record.update(protected_after=b.protected(),fdb_sources_after=sources())
    assert record['protected_before']==record['protected_after']
    assert record['fdb_sources_before']==record['fdb_sources_after']
    assert record['fdb_state_sha256']==b.sha(fstate)==b.sha(sstate)
    record.update(protected_unchanged=True,fdb_sources_unchanged=True)
    b.write(design,record)


if __name__=='__main__':
    main()
