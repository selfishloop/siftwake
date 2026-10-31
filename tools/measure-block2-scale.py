"""Block2 first checkpoint: balanced 10k/50k core measures, then stop for audit.

Only fixed owned DB slots; no document generation, no automatic 100k/cold/pin run.
"""
import importlib.util
import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOGS = ROOT / 'docs/benchmarks'
P = 'block2-scale-20261028-v1'
CATALOG = LOGS / f'{P}-corpus-index.json'
PROGRAMS = {'fdb': ROOT / 'output/block2-fdb-20261028-v1.exe',
            'sqlite': ROOT / 'output/block2-sqlite-20261028-v1.exe'}
spec = importlib.util.spec_from_file_location('fixed_slots', ROOT / 'tools/measure-churn.py')
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)


def run(engine, mode, state, label, count):
    log = LOGS / f'{P}-{label}-{engine}-{mode}.jsonl'
    if log.exists():
        raise RuntimeError(f'Refusing to overwrite evidence: {log}')
    env = os.environ.copy()
    env.update(FDB_SCALE_DOCUMENTS=str(count), FDB_BENCH_CORPUS_INDEX=str(CATALOG),
               FDB_STREAMED_WAL_LAYER='1', FDB_RECLAMATION_CONTROL='1',
               FDB_SCALE_QUERY_REPEATS='10')
    started = time.monotonic()
    with log.open('w', encoding='utf-8') as output, log.with_suffix('.stderr').open('w', encoding='utf-8') as error:
        child = subprocess.run([str(PROGRAMS[engine]), mode, 'streamed', str(state)],
                               cwd=ROOT, env=env, stdout=output, stderr=error,
                               creationflags=subprocess.CREATE_NO_WINDOW)
    result = dict(engine=engine, count=count, mode=mode, log=log.name, exit_code=child.returncode,
                  wall_s=time.monotonic() - started)
    base.write(log.with_suffix('.process.json'), result)
    print(json.dumps(result), flush=True)
    if child.returncode:
        raise RuntimeError(f'Failed worker: {log}')
    return result


def main():
    design_path = LOGS / f'{P}-design.json'
    if design_path.exists():
        raise RuntimeError('Design exists; inspect prior run instead of replaying')
    audit = json.loads((LOGS / 'block2-20261028-corpus-audit.json').read_text(encoding='utf-8'))
    assert audit['records'] == 100000 and audit['duplicate_bodies'] == 0
    assert CATALOG.exists()
    design = dict(programs={e: dict(path=str(p.relative_to(ROOT)), sha256=base.sha(p)) for e,p in PROGRAMS.items()},
                  protected_before=base.protected(), metadata_only_catalog=str(CATALOG.relative_to(ROOT)),
                  corpus='same existing first N, no duplicates or new documents; 8 books, overlapping windows',
                  cache='warm OS cache: full model get/oracle before query warmup, one unmeasured warmup/case; no eviction',
                  api='streamed, 500 entries/commit for ingest; active-WAL opt-in ON, revision reclaim OFF',
                  sqlite='FTS5 trigram detail=none, external content, FULL WAL, 2MiB cache, mmap0, tempFILE',
                  samples='10 measured samples/case/pair, 2 balanced pairs (20 pooled)',
                  memory='fresh-process idle before oracle/model; ingest full-process peak including catalog/model; Windows private and WS reported separately',
                  checkpoint='50k core gates first; do not proceed to 100k, 2b or 2c automatically',
                  order=[], completed=[])
    base.write(design_path, design)
    # Reverse engine order within each N; last slots remain 50k for diagnostics.
    tasks = [(0,10000,'fdb'),(0,10000,'sqlite'),(1,10000,'sqlite'),(1,10000,'fdb'),
             (0,50000,'fdb'),(0,50000,'sqlite'),(1,50000,'sqlite'),(1,50000,'fdb')]
    for pair,count,engine in tasks:
        label = f'n{count}-r{pair}'
        state = LOGS / f'{P}-{label}-{engine}-state.json'
        base.reset(engine)
        base.write(LOGS / f'{P}-slot-{engine}.json', dict(pair=pair,count=count,state=str(state)))
        design['order'].append(dict(pair=pair,count=count,engine=engine))
        for mode in ('scale-seed','scale-idle','scale-matrix'):
            design['completed'].append(run(engine,mode,state,label,count))
            base.write(design_path,design)
    design['protected_after'] = base.protected()
    assert design['protected_after'] == design['protected_before']
    design['protected_unchanged'] = True
    base.write(design_path, design)


if __name__ == '__main__':
    main()
